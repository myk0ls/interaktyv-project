use crate::game::{GameState, SharedGame};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{error, info};

pub type Clients = Arc<RwLock<HashMap<SocketAddr, mpsc::UnboundedSender<Message>>>>;

/// Handle an individual TCP -> WebSocket connection.
/// This function parses incoming messages and delegates to game APIs (aim/shoot).
pub async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    clients: Clients,
    game: SharedGame,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("New WebSocket connection from: {}", addr);

    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Store the client sender so other tasks can broadcast to it
    clients.write().await.insert(addr, tx.clone());

    // create a player in game state
    {
        let mut gs = game.write().await;
        let player = gs.add_player(addr);
        info!("Added player {} for {}", player.id, addr);
    }

    // Spawn task to forward outgoing messages to the socket
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                info!("Received text from {}: {}", addr, text);
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(t) = v.get("type").and_then(|t| t.as_str()) {
                        match t {
                            "aim" => {
                                if let Some(yaw) = v.get("yaw").and_then(|y| y.as_f64()) {
                                    let mut gs = game.write().await;
                                    gs.handle_aim(&addr, yaw as f32);
                                }
                            }
                            "shoot" => {
                                let mut gs = game.write().await;
                                // handle_shoot returns and pushes the marble internally
                                gs.handle_shoot(&addr);
                            }
                            _ => {
                                // ignore other types for now
                            }
                        }
                    }
                }
            }
            Ok(Message::Binary(bin)) => {
                info!("Received {} bytes from {}", bin.len(), addr);
                // ignore for now
            }
            Ok(Message::Close(_)) => {
                info!("Client {} disconnected", addr);
                break;
            }
            Ok(Message::Ping(data)) => {
                if let Some(tx) = clients.read().await.get(&addr) {
                    tx.send(Message::Pong(data)).ok();
                }
            }
            Ok(Message::Pong(_)) => {}
            Err(e) => {
                error!("WebSocket error for {}: {}", addr, e);
                break;
            }
            _ => {}
        }
    }

    // Clean up
    send_task.abort();
    clients.write().await.remove(&addr);
    {
        let mut gs = game.write().await;
        if let Some(p) = gs.remove_player(&addr) {
            info!("Removed player {} for {}", p.id, addr);
        }
    }
    info!("Client {} removed", addr);

    Ok(())
}
