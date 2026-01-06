use crate::game::SharedGame;
use crate::room::SharedRoomManager;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

pub type Clients = Arc<RwLock<HashMap<SocketAddr, mpsc::UnboundedSender<Message>>>>;

pub async fn ws_route(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(room_manager): State<SharedRoomManager>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, addr, room_manager))
}

async fn handle_socket(socket: WebSocket, addr: SocketAddr, room_manager: SharedRoomManager) {
    info!("New WebSocket connection from: {}", addr);

    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    let mut current_room_id: Option<String> = None;
    let mut current_game: Option<SharedGame> = None;
    let mut current_clients: Option<Clients> = None;

    // Spawn task to handle outgoing messages
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Main message handling loop
    loop {
        tokio::select! {
            // Messages from client
            Some(Ok(msg)) = receiver.next() => {
                match msg {
                    Message::Text(text) => {
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

                                        let _ = tx.send(Message::Text(response.to_string()));
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
                                            v.get("maxPlayers").and_then(|m| m.as_u64()).unwrap_or(2)
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

                                        let _ = tx.send(Message::Text(response.to_string()));
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
                                                let _ = tx.send(Message::Text(error.to_string()));
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
                                                    let _ = tx.send(Message::Text(error.to_string()));
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
                                                let _ = tx.send(Message::Text(welcome.to_string()));

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
                                                let _ = tx.send(Message::Text(error.to_string()));
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
                    Message::Close(_) => {
                        info!("Client {} sent close message", addr);
                        break;
                    }
                    Message::Ping(data) => {
                        let _ = tx.send(Message::Pong(data));
                    }
                    Message::Pong(_) => {}
                    Message::Binary(bin) => {
                        info!("Received {} bytes from {}", bin.len(), addr);
                    }
                }
            }

            else => {
                // Channel closed
                break;
            }
        }
    }

    // Cleanup on disconnect
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

    info!(
        "Client {} disconnected and removed from room {:?}",
        addr, current_room_id
    );
}
