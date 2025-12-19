use crate::game::{GameState, SharedGame};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{Duration, timeout};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{error, info};

pub type Clients = Arc<RwLock<HashMap<SocketAddr, mpsc::UnboundedSender<Message>>>>;

/// Handle an individual TCP -> WebSocket connection.
/// Expects the client to send a join message first:
/// { "type": "join", "token": "<optional-token>" }
/// Server will reply with a welcome message: { "type": "welcome", "token": "...", "player": { ... } }
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

    // spawn task to forward outgoing messages to the socket
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Wait for a short time for a join message from the client
    let join_msg = match timeout(Duration::from_secs(5), ws_receiver.next()).await {
        Ok(Some(Ok(Message::Text(txt)))) => match serde_json::from_str::<serde_json::Value>(&txt) {
            Ok(v) => Some(v),
            Err(_) => None,
        },
        _ => None,
    };

    // If no valid join message, close the connection
    if join_msg.is_none() {
        let _ = tx.send(Message::Close(Some(
            tokio_tungstenite::tungstenite::protocol::frame::CloseFrame {
                code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Away,
                reason: "No join message received".into(),
            },
        )));
        send_task.abort();
        return Ok(());
    }

    let v = join_msg.unwrap();
    let token_opt = v
        .get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());

    // perform join / restore
    let (token, player) = {
        let mut gs = game.write().await;
        gs.join_with_token(token_opt, addr)
    };

    // Register client for broadcasting now that it's joined
    clients.write().await.insert(addr, tx.clone());

    // send welcome message (through tx so send_task sends it)
    let welcome = serde_json::json!({
        "type": "welcome",
        "token": token,
        "player": player,
    });
    tx.send(Message::Text(welcome.to_string())).ok();

    info!("Player {} joined from {}", player.id, addr);

    // Now continue handling messages coming from this client
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

    // Clean up after disconnect
    send_task.abort();
    clients.write().await.remove(&addr);
    {
        let mut gs = game.write().await;
        gs.disconnect_by_addr(&addr);
    }
    info!("Client {} removed", addr);

    Ok(())
}
