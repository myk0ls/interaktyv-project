use std::net::SocketAddr;
use std::sync::Arc;

mod game;
mod network;

use game::SharedGame;
use network::Clients;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // init logging
    tracing_subscriber::fmt::init();

    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(&addr).await?;
    info!("WebSocket server listening on: {}", addr);

    let clients: Clients = Arc::new(RwLock::new(std::collections::HashMap::new()));
    let game: SharedGame = Arc::new(RwLock::new(game::GameState::default()));

    // spawn accept loop
    let clients_accept = clients.clone();
    let game_accept = game.clone();
    tokio::spawn(async move {
        while let Ok((stream, addr)) = listener.accept().await {
            tokio::spawn(network::handle_connection(
                stream,
                addr,
                clients_accept.clone(),
                game_accept.clone(),
            ));
        }
    });

    // tick loop + broadcast snapshots (20Hz)
    let tick_clients = clients.clone();
    let tick_game = game.clone();
    tokio::spawn(async move {
        let tick_rate = tokio::time::Duration::from_millis(50); // 20 Hz
        let mut interval = tokio::time::interval(tick_rate);
        loop {
            interval.tick().await;
            {
                // advance game state (physics, aging, collisions eventually)
                let mut gs = tick_game.write().await;
                gs.update(0.05_f32);
            }

            // build snapshot
            let payload = {
                let gs = tick_game.read().await;
                gs.snapshot()
            };

            // broadcast to all clients
            let clients_map = tick_clients.read().await;
            for (_addr, tx) in clients_map.iter() {
                let _ = tx.send(Message::Text(payload.clone()));
            }
        }
    });

    // prevent main from exiting
    futures_util::future::pending::<()>().await;
    Ok(())
}
