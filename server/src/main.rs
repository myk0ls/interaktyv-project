use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Player {
    id: u64,
    x: f32,
    y: f32,
    z: f32,
    yaw: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Marble {
    id: u64,
    x: f32,
    y: f32,
    z: f32,
    vx: f32,
    vy: f32,
    vz: f32,
    life: f32, // seconds remaining
}

type SharedGame = Arc<RwLock<GameState>>;

#[derive(Debug, Default)]
struct GameState {
    players: HashMap<SocketAddr, Player>,
    marbles: Vec<Marble>,
    next_player_id: u64,
    next_marble_id: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //init logging
    tracing_subscriber::fmt::init();

    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(&addr).await?;
    info!("WebSocket server listening on: {}", addr);

    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));
    let game: SharedGame = Arc::new(RwLock::new(GameState::default()));

    // spawn tick loop for game updates + broadcasting state (20 Hz)
    {
        let clients = clients.clone();
        let game = game.clone();
        tokio::spawn(async move {
            let tick_rate = tokio::time::Duration::from_millis(50); // 20 Hz
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                interval.tick().await;
                // advance game state
                {
                    let mut gs = game.write().await;
                    let dt = 0.05_f32; // consistent with 50 ms
                    // update marbles
                    gs.marbles.iter_mut().for_each(|m| {
                        m.x += m.vx * dt;
                        m.y += m.vy * dt;
                        m.z += m.vz * dt;
                        // simple gravity
                        //m.vy += -9.8 * dt;
                        m.life -= dt;
                    });
                    // remove dead marbles or ones far away
                    gs.marbles.retain(|m| {
                        m.life > 0.0 && m.x.abs() < 200.0 && m.y > -50.0 && m.z.abs() < 200.0
                    });
                }

                // build and broadcast state
                let payload = {
                    let gs = game.read().await;
                    let players: Vec<Player> = gs.players.values().cloned().collect();
                    let marbles: Vec<Marble> = gs.marbles.clone();
                    serde_json::json!({
                        "type": "state",
                        "players": players,
                        "marbles": marbles,
                    })
                    .to_string()
                };

                let clients_map = clients.read().await;
                for (_addr, tx) in clients_map.iter() {
                    // ignore send errors (closed channels)
                    let _ = tx.send(Message::Text(payload.clone()));
                }
            }
        });
    }

    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(
            stream,
            addr,
            clients.clone(),
            game.clone(),
        ));
    }

    Ok(())
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    clients: Clients,
    game: SharedGame,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("New WebSocket connection from: {}", addr);

    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Store the client
    clients.write().await.insert(addr, tx.clone());

    // create a player in game state
    {
        let mut gs = game.write().await;
        let id = gs.next_player_id;
        gs.next_player_id += 1;

        // spawn position - random small offset so players don't stack
        let mut rng = rand::thread_rng();
        let angle = (id as f32) * 0.618; // deterministic-ish
        let random_val: f32 = rng.random();
        let radius = 2.0 + (random_val * 2.0);
        let px = radius * angle.sin();
        let pz = radius * angle.cos();

        let player = Player {
            id,
            x: 0.0, //px
            y: 0.0,
            z: 0.0, //pz
            yaw: 0.0,
        };
        gs.players.insert(addr, player);
        info!("Added player {} for {}", id, addr);
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
                // try to parse json messages; expected types: aim { yaw }, shoot
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(t) = v.get("type").and_then(|t| t.as_str()) {
                        match t {
                            "aim" => {
                                if let Some(yaw) = v.get("yaw").and_then(|y| y.as_f64()) {
                                    let mut gs = game.write().await;
                                    if let Some(p) = gs.players.get_mut(&addr) {
                                        p.yaw = yaw as f32;
                                    }
                                }
                            }
                            "shoot" => {
                                // spawn marble at player's position with velocity based on yaw
                                let mut gs = game.write().await;
                                if let Some(p) = gs.players.get(&addr).cloned() {
                                    let mid = gs.next_marble_id;
                                    gs.next_marble_id += 1;
                                    let speed = 5.0_f32;
                                    let vx = yaw_sin(p.yaw) * speed;
                                    let vz = yaw_cos(p.yaw) * speed;
                                    let marble = Marble {
                                        id: mid,
                                        x: p.x,
                                        y: p.y + 0.1, // spawn slightly above player
                                        z: p.z,
                                        vx,
                                        vy: 0.0, //vy: 4.0, // small upward kick
                                        vz,
                                        life: 8.0, // seconds
                                    };
                                    gs.marbles.push(marble);
                                }
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
        if let Some(p) = gs.players.remove(&addr) {
            info!("Removed player {} for {}", p.id, addr);
        }
    }
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

// helper sin/cos where yaw is radians, with x = sin(yaw), z = cos(yaw)
fn yaw_sin(yaw: f32) -> f32 {
    yaw.sin()
}
fn yaw_cos(yaw: f32) -> f32 {
    yaw.cos()
}
