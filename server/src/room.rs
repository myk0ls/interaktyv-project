use crate::game::{GameState, SharedGame};
use crate::network::Clients;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub type SharedRoomManager = Arc<RwLock<RoomManager>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: String,
    pub name: String,
    pub players: usize,
    pub max_players: usize,
    pub created_at: i64,
}

pub struct Room {
    pub id: String,
    pub name: String,
    pub max_players: usize,
    pub created_at: i64,
    pub game: SharedGame,
    pub clients: Clients,
}

impl Room {
    pub fn new(id: String, name: String, max_players: usize) -> Self {
        let game = Arc::new(RwLock::new(GameState::default()));
        let clients = Arc::new(RwLock::new(HashMap::new()));
        let created_at = chrono::Utc::now().timestamp();

        info!("Created room: {} ({})", name, id);

        Self {
            id,
            name,
            max_players,
            created_at,
            game,
            clients,
        }
    }

    pub async fn player_count(&self) -> usize {
        self.game.read().await.players.len()
    }

    pub async fn is_full(&self) -> bool {
        self.player_count().await >= self.max_players
    }

    pub async fn info(&self) -> RoomInfo {
        RoomInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            players: self.player_count().await,
            max_players: self.max_players,
            created_at: self.created_at,
        }
    }
}

pub struct RoomManager {
    rooms: HashMap<String, Arc<RwLock<Room>>>,
    player_rooms: HashMap<SocketAddr, String>, // tracks which room each player is in
    next_room_id: u64,
}

impl RoomManager {
    pub fn new() -> Self {
        Self {
            rooms: HashMap::new(),
            player_rooms: HashMap::new(),
            next_room_id: 1,
        }
    }

    pub fn create_room(&mut self, name: String, max_players: usize) -> String {
        let id = format!("room-{}", self.next_room_id);
        self.next_room_id += 1;

        let room = Room::new(id.clone(), name, max_players);
        self.rooms.insert(id.clone(), Arc::new(RwLock::new(room)));

        info!("Room created: {}", id);
        id
    }

    pub fn get_room(&self, room_id: &str) -> Option<Arc<RwLock<Room>>> {
        self.rooms.get(room_id).cloned()
    }

    pub async fn list_rooms(&self) -> Vec<RoomInfo> {
        let mut rooms = Vec::new();
        for room_lock in self.rooms.values() {
            let room = room_lock.read().await;
            rooms.push(room.info().await);
        }
        rooms.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        rooms
    }

    pub fn assign_player_to_room(&mut self, addr: SocketAddr, room_id: String) {
        self.player_rooms.insert(addr, room_id);
    }

    pub fn get_player_room(&self, addr: &SocketAddr) -> Option<String> {
        self.player_rooms.get(addr).cloned()
    }

    pub fn remove_player(&mut self, addr: &SocketAddr) {
        self.player_rooms.remove(addr);
    }

    pub async fn cleanup_empty_rooms(&mut self) {
        let mut to_remove = Vec::new();

        for (id, room_lock) in &self.rooms {
            let room = room_lock.read().await;
            if room.player_count().await == 0 {
                to_remove.push(id.clone());
            }
        }

        for id in to_remove {
            self.rooms.remove(&id);
            info!("Removed empty room: {}", id);
        }
    }

    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }
}

impl Default for RoomManager {
    fn default() -> Self {
        Self::new()
    }
}
