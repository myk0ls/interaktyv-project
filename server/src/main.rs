use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::hash::Hash;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::handshake::client;
use tokio_tungstenite::tungstenite::stream;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{error, info, warn};

type Clients = Arc<RwLock<HashMap<SocketAddr, tokio::sync::mpsc::UnboundedSender<Message>>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //init logging
    tracing_subscriber::fmt::init();

    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(&addr).await?;
    info!("WebSocket server listening on: {}", addr);

    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));

    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(stream, addr, clients.clone()));
    }

    Ok(())
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    clients: Clients,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("New WebSocket connection from: {}", addr);

    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Store the client
    clients.write().await.insert(addr, tx);

    // Spawn task to handle outgoing messages
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
                //broadcast_message(&clients, Message::Text(text), addr).await;
                return_message(&clients, Message::Text(text), addr).await;
            }
            Ok(Message::Binary(bin)) => {
                info!("Received {} bytes from {}", bin.len(), addr);
                broadcast_message(&clients, Message::Binary(bin), addr).await;
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
            Ok(Message::Frame(_)) => {}
            Err(e) => {
                error!("WebSocket error for {}: {}", addr, e);
                break;
            }
        }
    }

    // Clean up
    send_task.abort();
    clients.write().await.remove(&addr);
    info!("Client {} removed", addr);

    Ok(())
}

async fn broadcast_message(clients: &Clients, msg: Message, sender: SocketAddr) {
    let clients = clients.read().await;
    for (addr, tx) in clients.iter() {
        if *addr != sender {
            tx.send(msg.clone()).ok();
        }
    }
}

async fn return_message(clients: &Clients, msg: Message, sender: SocketAddr) {
    let client = clients.read().await;
    for (addr, tx) in client.iter() {
        if *addr == sender {
            tx.send(msg.clone()).ok();
        }
    }
}
