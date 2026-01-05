use crate::game::SharedGame;
use crate::room::SharedRoomManager;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{error, info, warn};

pub type Clients = Arc<RwLock<HashMap<SocketAddr, mpsc::UnboundedSender<Message>>>>;

pub async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    room_manager: SharedRoomManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("New WebSocket connection from: {}", addr);

    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // spawn task to forward outgoing messages to the socket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut current_room_id: Option<String> = None;
    let mut current_game: Option<SharedGame> = None;
    let mut current_clients: Option<Clients> = None;

    // Handle incoming messages
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                info!("Received text from {}: {}", addr, text);

                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(msg_type) = v.get("type").and_then(|t| t.as_str()) {
                        match msg_type {
                            "list_rooms" => {
                                // Send list of available rooms
                                let rooms = {
                                    let rm = room_manager.read().await;
                                    rm.list_rooms().await
                                };

                                let response = serde_json::json!({
                                    "type": "rooms_list",
                                    "rooms": rooms,
                                });

                                tx.send(Message::Text(response.to_string())).ok();
                                info!("Sent room list to {}: {} rooms", addr, rooms.len());
                            }

                            "create_room" => {
                                // Create a new room
                                let name = v
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("Unnamed Room")
                                    .to_string();

                                let max_players =
                                    v.get("maxPlayers").and_then(|m| m.as_u64()).unwrap_or(4)
                                        as usize;

                                let room_id = {
                                    let mut rm = room_manager.write().await;
                                    rm.create_room(name.clone(), max_players)
                                };

                                let response = serde_json::json!({
                                    "type": "room_created",
                                    "roomId": room_id,
                                    "name": name,
                                    "maxPlayers": max_players,
                                });

                                tx.send(Message::Text(response.to_string())).ok();
                                info!("Created room {} for client {}", room_id, addr);
                            }

                            "join_room" => {
                                // Join a specific room
                                let room_id = match v.get("roomId").and_then(|r| r.as_str()) {
                                    Some(id) => id,
                                    None => {
                                        warn!("Client {} sent join_room without roomId", addr);
                                        let error = serde_json::json!({
                                            "type": "error",
                                            "message": "Missing roomId",
                                        });
                                        tx.send(Message::Text(error.to_string())).ok();
                                        continue;
                                    }
                                };

                                let token_opt = v
                                    .get("token")
                                    .and_then(|t| t.as_str())
                                    .map(|s| s.to_string());

                                // Get the room
                                let room_lock = {
                                    let rm = room_manager.read().await;
                                    rm.get_room(room_id)
                                };

                                match room_lock {
                                    Some(room_lock) => {
                                        let room = room_lock.read().await;

                                        // Check if room is full
                                        if room.is_full().await {
                                            warn!("Room {} is full, rejecting {}", room_id, addr);
                                            let error = serde_json::json!({
                                                "type": "error",
                                                "message": "Room is full",
                                            });
                                            tx.send(Message::Text(error.to_string())).ok();
                                            continue;
                                        }

                                        // Join the game
                                        let (token, player) = {
                                            let mut gs = room.game.write().await;
                                            gs.join_with_token(token_opt, addr)
                                        };

                                        // Register client in room's client list
                                        room.clients.write().await.insert(addr, tx.clone());

                                        // Track player's room
                                        {
                                            let mut rm = room_manager.write().await;
                                            rm.assign_player_to_room(addr, room_id.to_string());
                                        }

                                        // Store references for this connection
                                        current_room_id = Some(room_id.to_string());
                                        current_game = Some(room.game.clone());
                                        current_clients = Some(room.clients.clone());

                                        // Send welcome message
                                        let welcome = serde_json::json!({
                                            "type": "welcome",
                                            "token": token,
                                            "player": player,
                                            "roomId": room_id,
                                        });
                                        tx.send(Message::Text(welcome.to_string())).ok();

                                        info!(
                                            "Player {} joined room {} from {}",
                                            player.id, room_id, addr
                                        );
                                    }
                                    None => {
                                        warn!("Room {} not found for client {}", room_id, addr);
                                        let error = serde_json::json!({
                                            "type": "error",
                                            "message": "Room not found",
                                        });
                                        tx.send(Message::Text(error.to_string())).ok();
                                    }
                                }
                            }

                            "aim" => {
                                // Handle aim update (only if in a room)
                                if let (Some(game), Some(yaw)) =
                                    (&current_game, v.get("yaw").and_then(|y| y.as_f64()))
                                {
                                    let mut gs = game.write().await;
                                    gs.handle_aim(&addr, yaw as f32);
                                }
                            }

                            "shoot" => {
                                // Handle shoot (only if in a room)
                                if let Some(game) = &current_game {
                                    let mut gs = game.write().await;
                                    gs.handle_shoot(&addr);
                                }
                            }

                            _ => {
                                warn!("Unknown message type from {}: {}", addr, msg_type);
                            }
                        }
                    }
                }
            }
            Ok(Message::Binary(bin)) => {
                info!("Received {} bytes from {}", bin.len(), addr);
            }
            Ok(Message::Close(_)) => {
                info!("Client {} disconnected", addr);
                break;
            }
            Ok(Message::Ping(data)) => {
                tx.send(Message::Pong(data)).ok();
            }
            Ok(Message::Pong(_)) => {}
            Err(e) => {
                error!("WebSocket error for {}: {}", addr, e);
                break;
            }
            _ => {}
        }
    }

    // Clean up after disconnect
    send_task.abort();

    // Remove from room's client list
    if let Some(clients) = &current_clients {
        clients.write().await.remove(&addr);
    }

    // Disconnect from game
    if let Some(game) = &current_game {
        let mut gs = game.write().await;
        gs.disconnect_by_addr(&addr);
    }

    // Remove player from room manager
    {
        let mut rm = room_manager.write().await;
        rm.remove_player(&addr);
    }

    info!("Client {} removed from room {:?}", addr, current_room_id);

    Ok(())
}
