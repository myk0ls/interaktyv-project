use std::sync::Arc;

mod game;
mod network;
mod room;

use axum::{routing::get, Router};
use room::{RoomManager, SharedRoomManager};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // init logging
    tracing_subscriber::fmt::init();

    // Create room manager
    let room_manager: SharedRoomManager = Arc::new(RwLock::new(RoomManager::new()));

    // Create a default lobby room
    {
        let mut rm = room_manager.write().await;
        rm.create_room("Lobby".to_string(), 8);
        info!("Default lobby room created");
    }

    let app = Router::new()
        .route("/ws", get(network::ws_route))
        .with_state(room_manager.clone())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    // tick loop for all rooms + broadcast snapshots (20Hz)
    let rm_tick = room_manager.clone();
    tokio::spawn(async move {
        let tick_rate = tokio::time::Duration::from_millis(50); // 20 Hz
        let mut interval = tokio::time::interval(tick_rate);
        loop {
            interval.tick().await;

            // Get all rooms
            let rooms = {
                let rm = rm_tick.read().await;
                rm.list_rooms().await
            };

            // Update each room
            for room_info in rooms {
                let room_lock = {
                    let rm = rm_tick.read().await;
                    rm.get_room(&room_info.id)
                };

                if let Some(room_lock) = room_lock {
                    let room = room_lock.read().await;

                    // advance game state
                    {
                        let mut gs = room.game.write().await;
                        gs.update(0.05_f32);
                    }

                    // build snapshot
                    let payload = {
                        let gs = room.game.read().await;
                        gs.snapshot()
                    };

                    // broadcast to all clients in this room
                    let clients_map = room.clients.read().await;
                    for (_addr, tx) in clients_map.iter() {
                        let _ = tx.send(axum::extract::ws::Message::Text(payload.clone()));
                    }
                }
            }
        }
    });

    // Cleanup task - removes empty rooms every 60 seconds
    let rm_cleanup = room_manager.clone();
    tokio::spawn(async move {
        let cleanup_interval = tokio::time::Duration::from_secs(60);
        let mut interval = tokio::time::interval(cleanup_interval);
        loop {
            interval.tick().await;
            let mut rm = rm_cleanup.write().await;
            rm.cleanup_empty_rooms().await;
        }
    });

    let addr = "0.0.0.0:8080";
    let listener = TcpListener::bind(addr).await?;
    info!("HTTP server listening on: {}", addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
