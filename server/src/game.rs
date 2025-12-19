use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info; // debug stats/logging

/// Shared game alias used by the networking layer
pub type SharedGame = Arc<RwLock<GameState>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,

    // the color currently loaded in the shooter, and the next color in queue.
    pub loaded_color: String,
    pub next_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Marble {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
    pub life: f32, // seconds remaining
    pub color: String,
    pub owner: Option<u64>, // None for free marbles and chain marbles (shared chain)
}

/// Marble stored inside the shared chain. Position is derived from angle + chain_center.
#[derive(Debug, Clone)]
pub struct ChainMarble {
    pub id: u64,
    pub angle: f32,    // radians along the orbit
    pub distance: f32, // radius from chain center
    pub color: String,
}

#[derive(Debug)]
pub struct GameState {
    pub players: HashMap<SocketAddr, Player>,
    pub marbles: Vec<Marble>, // free marbles (shot by players and moving freely)
    pub chain: Vec<ChainMarble>, // single shared chain for all players (coop)
    pub chain_center_x: f32,
    pub chain_center_z: f32,
    pub next_player_id: u64,
    pub next_marble_id: u64,
}

impl Default for GameState {
    fn default() -> Self {
        let mut gs = GameState {
            players: HashMap::new(),
            marbles: Vec::new(),
            chain: Vec::new(),
            chain_center_x: 0.0,
            chain_center_z: 0.0,
            next_player_id: 0,
            next_marble_id: 0,
        };

        // initialize a simple shared chain orbiting around chain_center (0,0)
        // using the debugging/stats parameters you requested
        let mut rng = rand::thread_rng();
        let colors = ["red", "green", "blue", "yellow", "purple"];
        let chain_len = 30usize; // UPDATED: 30 marbles
        let chain_radius = 3.0_f32; // UPDATED: radius 3.0
        let spacing = TAU / (chain_len as f32);
        info!(
            "Initializing shared chain: len={}, radius={}, spacing={}",
            chain_len, chain_radius, spacing
        );

        for i in 0..chain_len {
            let mid = gs.next_marble_id;
            gs.next_marble_id += 1;
            let a = spacing * (i as f32);
            // use rng.random() per your request (rng.gen() replaced)
            let color_index = (rng.random::<f32>() * (colors.len() as f32)) as usize;
            let color = colors[color_index % colors.len()].to_string();
            gs.chain.push(ChainMarble {
                id: mid,
                angle: a,
                distance: chain_radius,
                color,
            });
        }

        gs
    }
}

impl GameState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a player for an address, returning the created Player.
    /// First two players receive fixed offsets so a two-player coop layout is stable.
    pub fn add_player(&mut self, addr: SocketAddr) -> Player {
        let id = self.next_player_id;
        self.next_player_id += 1;

        // spawn position:
        // - first connected player -> fixed left offset (-2.0, 0.0)
        // - second connected player -> fixed right offset (2.0, 0.0)
        // - further players -> small random offset as before
        let (px, pz) = match self.players.len() {
            0 => {
                // player 1
                (-2.0_f32, 0.0_f32)
            }
            1 => {
                // player 2
                (2.0_f32, 0.0_f32)
            }
            _ => {
                // fallback/random for additional players
                let mut rng = rand::thread_rng();
                let angle = (id as f32) * 0.618;
                // use rng.random() as requested
                let random_val: f32 = rng.random();
                let radius = 2.0 + (random_val * 2.0);
                (radius * angle.sin(), radius * angle.cos())
            }
        };

        // assign initial loaded/next colors using rng.random()
        let mut rng = rand::thread_rng();
        let loaded = random_color_with_rng(&mut rng);
        let next = random_color_with_rng(&mut rng);

        let player = Player {
            id,
            x: px,
            y: 0.0,
            z: pz,
            yaw: 0.0,
            loaded_color: loaded.clone(),
            next_color: next.clone(),
        };
        info!(
            "Added player id={} at ({:.2},{:.2}) loaded={} next={}",
            player.id, player.x, player.z, loaded, next
        );
        self.players.insert(addr, player.clone());
        player
    }

    /// Remove a player by address.
    pub fn remove_player(&mut self, addr: &SocketAddr) -> Option<Player> {
        self.players.remove(addr)
    }

    /// Update player's yaw for aiming.
    pub fn handle_aim(&mut self, addr: &SocketAddr, yaw: f32) {
        if let Some(p) = self.players.get_mut(addr) {
            p.yaw = yaw;
        }
    }

    /// Fire a marble from a player's position. Uses the player's loaded color.
    /// After shooting, the player's loaded_color is replaced by next_color, and next_color is randomized.
    pub fn handle_shoot(&mut self, addr: &SocketAddr) -> Option<Marble> {
        if let Some(p) = self.players.get_mut(addr) {
            let mid = self.next_marble_id;
            self.next_marble_id += 1;
            let speed = 8.0_f32;
            let vx = yaw_sin(p.yaw) * speed;
            let vz = yaw_cos(p.yaw) * speed;

            // use player's loaded color for the fired marble
            let color = p.loaded_color.clone();

            // rotate queue: loaded <- next, next <- random
            p.loaded_color = p.next_color.clone();
            // use rng.random() for next color
            let mut rng = rand::thread_rng();
            p.next_color = random_color_with_rng(&mut rng);

            info!("Player {} fired marble id={} color={}", p.id, mid, color);

            let marble = Marble {
                id: mid,
                x: p.x,
                y: p.y + 0.1,
                z: p.z,
                vx,
                vy: 0.0,
                vz,
                life: 8.0,
                color,
                owner: None,
            };
            self.marbles.push(marble.clone());
            Some(marble)
        } else {
            None
        }
    }

    /// Advance the simulation by dt seconds (physics, lifetime decay). Chain marbles move as a single shared chain.
    /// Also: detect collisions between free marbles and chain marbles; insert and run color-match removal.
    pub fn update(&mut self, dt: f32) {
        // update free marbles (physics)
        for m in self.marbles.iter_mut() {
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            m.z += m.vz * dt;
            // gravity placeholder (disabled by default)
            // m.vy += -9.8 * dt;
            m.life -= dt;
        }

        // remove dead/out-of-range free marbles
        self.marbles
            .retain(|m| m.life > 0.0 && m.x.abs() < 200.0 && m.y > -50.0 && m.z.abs() < 200.0);

        // update shared chain marbles: advance along angle (simple movement)
        let angular_speed = 0.6_f32; // radians per second: how fast chain moves
        for cm in self.chain.iter_mut() {
            cm.angle += angular_speed * dt;
            // normalize angle to 0..TAU
            if cm.angle > TAU {
                cm.angle -= TAU;
            } else if cm.angle < 0.0 {
                cm.angle += TAU;
            }
        }

        // Collision detection + insertion + match removal
        let mut i = 0usize;
        while i < self.marbles.len() {
            let m = self.marbles[i].clone(); // clone to work with it
            if let Some(coll_idx) = self.find_collision_index(&m) {
                // insert marble into chain at position after coll_idx
                self.insert_into_chain(m, coll_idx);
                // remove free marble (swap_remove)
                self.marbles.swap_remove(i);
                // do not increment i, since we've swapped in a new element at i
                continue;
            }
            i += 1;
        }
    }

    /// Determine index of closest chain marble within collision distance for a given free marble.
    fn find_collision_index(&self, marble: &Marble) -> Option<usize> {
        const COLLISION_DISTANCE: f32 = 0.7_f32; // tuning parameter (marble radius ~0.5)
        if self.chain.is_empty() {
            return None;
        }

        let mut best: Option<(usize, f32)> = None;
        for (idx, cm) in self.chain.iter().enumerate() {
            let (cx, cz) = self.chain_world_pos(cm);
            let dx = marble.x - cx;
            let dz = marble.z - cz;
            let dist2 = dx * dx + dz * dz;
            if dist2 <= COLLISION_DISTANCE * COLLISION_DISTANCE {
                let d = dist2.sqrt();
                match best {
                    None => best = Some((idx, d)),
                    Some((_, bestd)) => {
                        if d < bestd {
                            best = Some((idx, d));
                        }
                    }
                }
            }
        }
        best.map(|(idx, _)| idx)
    }

    /// Insert a free marble (world-coordinate) into the chain near collided index.
    /// Insertion index will be (coll_idx + 1) mod len. New marble angle is midpoint between collided and next marble.
    fn insert_into_chain(&mut self, marble: Marble, coll_idx: usize) {
        let new_id = marble.id;
        let color = marble.color.clone();

        if self.chain.is_empty() {
            // create chain with single marble
            self.chain.push(ChainMarble {
                id: new_id,
                angle: 0.0,
                distance: 3.0,
                color,
            });
            return;
        }

        let len = self.chain.len();
        let insert_after = coll_idx;
        let insert_idx = (insert_after + 1) % (len + 1); // if inserting at end, modulo handles it

        // compute angle between collided marble and next marble
        let cur_angle = self.chain[insert_after % len].angle;
        let next_angle = if len == 1 {
            cur_angle + 0.2 // arbitrary small offset when only one marble
        } else {
            // next is insert_after+1 mod len
            self.chain[(insert_after + 1) % len].angle
        };

        let mid_angle = angle_mid(cur_angle, next_angle);

        // keep same distance as collided marble
        let distance = self.chain[insert_after % len].distance;

        let new_cm = ChainMarble {
            id: new_id,
            angle: normalize_angle(mid_angle),
            distance,
            color,
        };

        // insert at insert_idx (if insert_idx == len, push)
        if insert_idx >= self.chain.len() {
            self.chain.push(new_cm);
            let idx = self.chain.len() - 1;
            self.try_remove_matches(idx);
        } else {
            self.chain.insert(insert_idx, new_cm);
            self.try_remove_matches(insert_idx);
        }
    }

    /// Attempt to remove contiguous match around index. Removes sequence if len >= 3.
    /// This will handle wrap-around.
    fn try_remove_matches(&mut self, idx: usize) {
        if self.chain.is_empty() {
            return;
        }
        let len = self.chain.len();
        // clone the color to avoid holding immutable borrow during mutation
        let color = self.chain[idx].color.clone();

        // Count left side (excluding idx)
        let mut left_count = 0usize;
        let mut cur = idx;
        for _ in 0..(len - 1) {
            let prev = if cur == 0 { len - 1 } else { cur - 1 };
            if self.chain[prev].color == color {
                left_count += 1;
                cur = prev;
            } else {
                break;
            }
        }

        // Count right side (excluding idx)
        let mut right_count = 0usize;
        cur = idx;
        for _ in 0..(len - 1) {
            let next = (cur + 1) % len;
            if self.chain[next].color == color {
                right_count += 1;
                cur = next;
            } else {
                break;
            }
        }

        let total = 1 + left_count + right_count;
        if total >= 3 {
            // determine start and end indices (inclusive), taking wrapping into account
            let start = (idx + len - left_count) % len;
            let end = (idx + right_count) % len;

            // mark removal flags
            let mut remove = vec![false; len];
            let mut i = start;
            loop {
                remove[i] = true;
                if i == end {
                    break;
                }
                i = (i + 1) % len;
            }

            // build new chain with elements not removed
            let mut new_chain = Vec::with_capacity(len - total);
            for (i, cm) in self.chain.drain(..).enumerate() {
                if !remove[i] {
                    new_chain.push(cm);
                }
            }
            info!("Removed {} matching marbles of color {}", total, color);
            self.chain = new_chain;
            // Note: after removal, you might want to apply chain collapse logic (neighbors come together)
            // or spawn score events. That can be added later.
        }
    }

    /// Compute world-space x,z of a chain marble
    fn chain_world_pos(&self, cm: &ChainMarble) -> (f32, f32) {
        let x = self.chain_center_x + cm.distance * cm.angle.cos();
        let z = self.chain_center_z + cm.distance * cm.angle.sin();
        (x, z)
    }

    /// Produce a JSON snapshot string of the current state to broadcast.
    /// This flattens both free marbles and shared chain marbles into a single "marbles" array.
    /// NOTE: Player structs now include loaded_color and next_color so the client can render them.
    pub fn snapshot(&self) -> String {
        // players
        let players: Vec<Player> = self.players.values().cloned().collect();

        // free marbles (clone)
        let mut marbles: Vec<Marble> = self.marbles.clone();

        // append chain marbles converted to Marble objects with computed world positions
        for cm in self.chain.iter() {
            let x = self.chain_center_x + cm.distance * cm.angle.cos();
            let z = self.chain_center_z + cm.distance * cm.angle.sin();
            let y = 0.5_f32; // slightly above ground
            marbles.push(Marble {
                id: cm.id,
                x,
                y,
                z,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                life: 9999.0,
                color: cm.color.clone(),
                owner: None,
            });
        }

        json!({
            "type": "state",
            "players": players,
            "marbles": marbles,
        })
        .to_string()
    }
}

// helper sin/cos where yaw is radians, with x = sin(yaw), z = cos(yaw)
fn yaw_sin(yaw: f32) -> f32 {
    yaw.sin()
}
fn yaw_cos(yaw: f32) -> f32 {
    yaw.cos()
}

/// Normalize angle to 0..TAU
fn normalize_angle(a: f32) -> f32 {
    let mut x = a % TAU;
    if x < 0.0 {
        x += TAU;
    }
    x
}

/// Compute midpoint angle going from a to b in angular sense (handles wrap)
fn angle_mid(a: f32, b: f32) -> f32 {
    // compute difference b-a in range 0..TAU
    let diff = (b - a + TAU) % TAU;
    let mid = a + diff * 0.5;
    normalize_angle(mid)
}

/// Small helper: random color chooser using rng.random()
fn random_color_with_rng(rng: &mut impl Rng) -> String {
    let colors = ["red", "green", "blue", "yellow", "purple"];
    let idx = (rng.random::<f32>() * (colors.len() as f32)) as usize;
    colors[idx % colors.len()].to_string()
}
