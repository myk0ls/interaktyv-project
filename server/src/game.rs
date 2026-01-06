use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Shared game alias used by the networking layer
pub type SharedGame = Arc<RwLock<GameState>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
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
    pub life: f32,
    pub color: String,
    pub owner: Option<u64>,
}

/// Chain marble stored on the sampled path. `s` is fraction along total path length [0..1].
/// color == None indicates a gap.
#[derive(Debug, Clone)]
pub struct ChainMarble {
    pub id: Option<u64>,
    pub s: f32,                // fraction [0..1] along path
    pub color: Option<String>, // None => gap
}

#[derive(Debug)]
pub struct GameState {
    pub players: HashMap<SocketAddr, Player>,
    pub marbles: Vec<Marble>,
    pub chain: Vec<ChainMarble>,

    pub current_score: u32,

    // sampled path representation
    pub path_points: Vec<(f32, f32)>, // control description (for debug)
    pub samples: Vec<(f32, f32)>,     // dense samples along path in (x,z)
    pub cum_lengths: Vec<f32>,        // cumulative lengths at sample indices (starts at 0)
    pub total_length: f32,            // total arc length

    // tuning
    pub spawn_accum: f32,
    pub spawn_interval: f32,
    pub marble_diameter: f32,
    pub spacing_length: f32,
    pub chain_speed: f32, // fraction of total per second

    pub next_player_id: u64,
    pub next_marble_id: u64,

    pub token_map: HashMap<String, PersistentPlayer>,
}

#[derive(Debug, Clone)]
pub struct PersistentPlayer {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub loaded_color: String,
    pub next_color: String,
    pub connected: bool,
    pub addr: Option<SocketAddr>,
}

#[derive(Serialize, Deserialize)]
struct ChainPath {
    name: String,
    points: Vec<(f32, f32, f32)>,
}

impl Default for GameState {
    fn default() -> Self {
        let mut gs = GameState {
            players: HashMap::new(),
            marbles: Vec::new(),
            chain: Vec::new(),
            current_score: 0,
            path_points: Vec::new(),
            samples: Vec::new(),
            cum_lengths: Vec::new(),
            total_length: 2.0,
            spawn_accum: 0.0,
            spawn_interval: 1.5,
            marble_diameter: 0.4,
            spacing_length: 0.4 * 1.02,
            chain_speed: 0.02,
            next_player_id: 0,
            next_marble_id: 0,
            token_map: HashMap::new(),
        };

        // Generate circular two-row path within [-8,8] bounds; avoid players.
        let mut rng = rand::thread_rng();
        //gs.generate_two_ring_path(&mut rng, 6, (-8.0, 8.0), (-8.0, 8.0), 200);

        gs.read_path("paths/zuma_path.json");

        // initial chain
        let colors = ["red", "green", "blue", "yellow", "purple"];
        let chain_len = 15usize;
        for i in 0..chain_len {
            let mid = gs.next_marble_id;
            gs.next_marble_id += 1;
            let s = 0.0;
            let color_index = (rng.random::<f32>() * (colors.len() as f32)) as usize;
            let color = colors[color_index % colors.len()].to_string();
            gs.chain.push(ChainMarble {
                id: Some(mid),
                s,
                color: Some(color),
            });
        }

        gs
    }
}

impl GameState {
    pub fn new() -> Self {
        Self::default()
    }

    fn read_path(&mut self, path_json: &str) {
        let string_data = fs::read_to_string(path_json).unwrap();

        let path: ChainPath = serde_json::from_str(&string_data).unwrap();

        let mut samples: Vec<(f32, f32)> = Vec::new();

        for point in path.points {
            samples.push((point.0, point.2));
        }

        // build cumulative lengths
        let mut cum_lengths: Vec<f32> = Vec::with_capacity(samples.len());
        let mut acc = 0.0_f32;
        for i in 0..samples.len() {
            if i == 0 {
                cum_lengths.push(0.0);
                continue;
            }
            let (ax, az) = samples[i - 1];
            let (bx, bz) = samples[i];
            let d = ((bx - ax) * (bx - ax) + (bz - az) * (bz - az)).sqrt();
            acc += d;
            cum_lengths.push(acc);
        }
        let total_length = if let Some(&last) = cum_lengths.last() {
            last
        } else {
            0.0
        };

        // Save
        self.samples = samples;
        self.cum_lengths = cum_lengths;
        self.total_length = if total_length > 0.0 {
            total_length
        } else {
            1.0
        };
    }

    /// Map arc-fraction s in [0..1] to world x,z by linear interpolation in samples.
    fn chain_world_pos(&self, s: f32) -> (f32, f32) {
        if self.samples.is_empty() {
            return (0.0, 0.0);
        }
        let s_clamped = s.clamp(0.0, 1.0);
        let target = s_clamped * self.total_length;
        // binary search in cum_lengths
        let idx = match self
            .cum_lengths
            .binary_search_by(|v| v.partial_cmp(&target).unwrap_or(std::cmp::Ordering::Equal))
        {
            Ok(i) => i,
            Err(i) => i,
        };
        if idx == 0 {
            return self.samples[0];
        } else if idx >= self.samples.len() {
            return *self.samples.last().unwrap();
        } else {
            // interpolate between idx-1 and idx
            let i1 = idx - 1;
            let i2 = idx;
            let l1 = self.cum_lengths[i1];
            let l2 = self.cum_lengths[i2];
            let denom = (l2 - l1).max(1e-6);
            let t = (target - l1) / denom;
            let (x1, z1) = self.samples[i1];
            let (x2, z2) = self.samples[i2];
            let x = x1 * (1.0 - t) + x2 * t;
            let z = z1 * (1.0 - t) + z2 * t;
            return (x, z);
        }
    }

    /// Restore or create a player by token and bind it to addr.
    pub fn join_with_token(
        &mut self,
        token_opt: Option<String>,
        addr: SocketAddr,
    ) -> (String, Player) {
        if let Some(token) = token_opt {
            if let Some(pp) = self.token_map.get_mut(&token) {
                pp.connected = true;
                pp.addr = Some(addr);
                let player = Player {
                    id: pp.id,
                    x: pp.x,
                    y: pp.y,
                    z: pp.z,
                    yaw: pp.yaw,
                    loaded_color: pp.loaded_color.clone(),
                    next_color: pp.next_color.clone(),
                };
                self.players.insert(addr, player.clone());
                info!("Restored player id={} from token {}", pp.id, token);
                return (token, player);
            }
        }

        let mut rng = rand::thread_rng();
        let id = self.next_player_id;
        self.next_player_id += 1;
        let connected_count = self.token_map.values().filter(|pp| pp.connected).count();
        let (px, pz) = match connected_count {
            0 => (-2.0, 0.0),
            1 => (2.0, 0.0),
            _ => {
                let angle = (id as f32) * 0.618;
                let random_val: f32 = rng.random();
                let radius = 2.0 + (random_val * 2.0);
                (radius * angle.sin(), radius * angle.cos())
            }
        };
        let loaded = random_color_with_rng(&mut rng);
        let next = random_color_with_rng(&mut rng);
        let token = generate_token(&mut rng);
        let persistent = PersistentPlayer {
            id,
            x: px,
            y: 0.0,
            z: pz,
            yaw: 0.0,
            loaded_color: loaded.clone(),
            next_color: next.clone(),
            connected: true,
            addr: Some(addr),
        };
        self.token_map.insert(token.clone(), persistent.clone());
        let player = Player {
            id,
            x: px,
            y: 0.0,
            z: pz,
            yaw: 0.0,
            loaded_color: loaded,
            next_color: next,
        };
        self.players.insert(addr, player.clone());
        info!("Created new persistent player id={} token={}", id, token);
        (token, player)
    }

    pub fn disconnect_by_addr(&mut self, addr: &SocketAddr) {
        if let Some(p) = self.players.remove(addr) {
            for (_token, pp) in self.token_map.iter_mut() {
                if pp.id == p.id {
                    pp.connected = false;
                    pp.addr = None;
                    info!("Player id={} marked disconnected (addr={})", pp.id, addr);
                    break;
                }
            }
        }
    }

    pub fn handle_aim(&mut self, addr: &SocketAddr, yaw: f32) {
        if let Some(p) = self.players.get_mut(addr) {
            p.yaw = yaw;
            for (_token, pp) in self.token_map.iter_mut() {
                if Some(addr.clone()) == pp.addr {
                    pp.yaw = yaw;
                    break;
                }
            }
        }
    }

    pub fn handle_shoot(&mut self, addr: &SocketAddr) -> Option<Marble> {
        if let Some(p) = self.players.get_mut(addr) {
            let mid = self.next_marble_id;
            self.next_marble_id += 1;
            let speed = 8.0_f32;
            let vx = yaw_sin(p.yaw) * speed;
            let vz = yaw_cos(p.yaw) * speed;
            let color = p.loaded_color.clone();
            p.loaded_color = p.next_color.clone();
            let mut rng = rand::thread_rng();
            p.next_color = random_color_with_rng(&mut rng);
            for (_token, pp) in self.token_map.iter_mut() {
                if Some(addr.clone()) == pp.addr {
                    pp.loaded_color = p.loaded_color.clone();
                    pp.next_color = p.next_color.clone();
                    break;
                }
            }
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

    pub fn update(&mut self, dt: f32) {
        // update free marbles
        for m in self.marbles.iter_mut() {
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            m.z += m.vz * dt;
            m.life -= dt;
        }
        // remove dead/out-of-bounds marbles
        self.marbles
            .retain(|m| m.life > 0.0 && m.x.abs() < 200.0 && m.y > -50.0 && m.z.abs() < 200.0);

        // spawn new chain marbles at start (s=0)
        self.spawn_accum += dt;
        while self.spawn_accum >= self.spawn_interval {
            self.spawn_accum -= self.spawn_interval;
            let mut rng = rand::rng();
            let color = random_color_chain(&mut rng, &self.chain);
            let id = self.next_marble_id;
            self.next_marble_id += 1;
            self.chain.push(ChainMarble {
                id: Some(id),
                s: 0.0,
                color: Some(color),
            });
        }

        // advance chain by increasing s proportional to fraction speed = chain_speed * dt
        for cm in self.chain.iter_mut() {
            cm.s += self.chain_speed * dt;
        }
        // remove those past end (s >= 1.0)
        self.chain.retain(|cm| cm.s < 1.0);

        // equalize spacing per contiguous non-gap segments using arc-length (s * total_length)
        self.equalize_chain_spacing();

        // keep chain sorted
        self.chain
            .sort_by(|a, b| a.s.partial_cmp(&b.s).unwrap_or(std::cmp::Ordering::Equal));

        // collision detection & insertion (gaps ignored)
        let mut i = 0usize;
        while i < self.marbles.len() {
            let m = self.marbles[i].clone();
            if let Some(coll_idx) = self.find_collision_index(&m) {
                // don't insert if next is a gap
                if coll_idx + 1 < self.chain.len() && self.chain[coll_idx + 1].color.is_none() {
                    i += 1;
                    continue;
                }
                self.insert_into_chain(m, coll_idx);
                self.marbles.swap_remove(i);
                continue;
            }
            i += 1;
        }
    }

    /// Re-space contiguous non-gap segments to have equal arc-length spacing anchored at the head of each segment.
    fn equalize_chain_spacing(&mut self) {
        if self.chain.is_empty() || self.total_length <= 0.0 {
            return;
        }

        // sort indices by s ascending (tail->head)
        let mut order: Vec<usize> = (0..self.chain.len()).collect();
        order.sort_by(|&a, &b| {
            self.chain[a]
                .s
                .partial_cmp(&self.chain[b].s)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // find contiguous non-gap segments using ordered indices
        let mut segments: Vec<Vec<usize>> = Vec::new();
        let mut cur: Vec<usize> = Vec::new();
        for &idx in order.iter() {
            if self.chain[idx].color.is_some() {
                cur.push(idx);
            } else {
                if !cur.is_empty() {
                    segments.push(cur);
                    cur = Vec::new();
                }
            }
        }
        if !cur.is_empty() {
            segments.push(cur);
        }

        let spacing = self.spacing_length.max(0.001);

        for seg in segments.into_iter() {
            // seg: indices in ascending s (tail->head)
            // gather current s -> convert to arc length
            let seg_s: Vec<f32> = seg.iter().map(|&i| self.chain[i].s).collect();
            if seg_s.is_empty() {
                continue;
            }
            // head s
            let s_head = *seg_s
                .iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap();
            let L_head = s_head * self.total_length;
            let m = seg.len();
            // desired head->tail lengths
            let mut desired_Ls: Vec<f32> = (0..m).map(|i| L_head - (i as f32) * spacing).collect();
            // clamp min to 0
            for d in desired_Ls.iter_mut() {
                if *d < 0.0 {
                    *d = 0.0;
                }
            }
            // convert to s (tail->head)
            desired_Ls.reverse();
            // assign back s values
            for (j, &chain_idx) in seg.iter().enumerate() {
                let L = desired_Ls[j];
                let s_new = if self.total_length > 0.0 {
                    (L / self.total_length).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                self.chain[chain_idx].s = s_new;
            }
        }
    }

    /// Find nearest non-gap chain marble within collision distance; returns chain index or None.
    fn find_collision_index(&self, marble: &Marble) -> Option<usize> {
        if self.chain.is_empty() || self.samples.is_empty() {
            return None;
        }
        let collision_distance = (self.marble_diameter * 1.0).max(0.5);
        let collision_sq = collision_distance * collision_distance;
        let mut best: Option<(usize, f32)> = None;
        for (idx, cm) in self.chain.iter().enumerate() {
            if cm.color.is_none() {
                continue;
            }
            let (cx, cz) = self.chain_world_pos(cm.s);
            let dx = marble.x - cx;
            let dz = marble.z - cz;
            let d2 = dx * dx + dz * dz;
            if d2 <= collision_sq {
                let d = d2.sqrt();
                match best {
                    None => best = Some((idx, d)),
                    Some((_, bd)) => {
                        if d < bd {
                            best = Some((idx, d));
                        }
                    }
                }
            }
        }
        best.map(|(i, _)| i)
    }

    fn insert_into_chain(&mut self, marble: Marble, coll_idx: usize) {
        let new_id = marble.id;
        let color = marble.color.clone();
        if self.chain.is_empty() {
            self.chain.push(ChainMarble {
                id: Some(new_id),
                s: 0.0,
                color: Some(color),
            });
            return;
        }
        let cur_s = self.chain[coll_idx].s;
        let marble_spacing = 0.02_f32;

        // Insert the new marble right after the collision point
        let insert_s = (cur_s - marble_spacing).min(1.0);
        self.chain.push(ChainMarble {
            id: Some(new_id),
            s: insert_s,
            color: Some(color),
        });
        self.chain
            .sort_by(|a, b| a.s.partial_cmp(&b.s).unwrap_or(std::cmp::Ordering::Equal));
        let inserted_idx = self
            .chain
            .iter()
            .position(|c| c.id == Some(new_id))
            .unwrap_or(0);

        // Check if insertion point is connected to the main chain (from start)
        let is_connected_to_main_chain = {
            let mut connected = true;
            for i in 0..=coll_idx {
                if self.chain[i].color.is_none() {
                    connected = false;
                    break;
                }
            }
            connected
        };

        // Try to remove matches
        self.try_remove_matches(inserted_idx);

        // Check if the marble still exists (wasn't removed due to match)
        let marble_still_exists = self.chain.iter().any(|c| c.id == Some(new_id));

        // Only push chain forward if:
        // 1. Marble didn't match and still exists
        // 2. Insertion was into the main connected chain
        if marble_still_exists && is_connected_to_main_chain {
            // Find the current position of the inserted marble
            if let Some(idx) = self.chain.iter().position(|c| c.id == Some(new_id)) {
                // Push only consecutive connected marbles forward (stop at gaps)
                for i in (idx + 1)..self.chain.len() {
                    // Stop pushing if we hit a gap (disconnected marble)
                    if self.chain[i].color.is_none() {
                        break;
                    }
                    self.chain[i].s = (self.chain[i].s + marble_spacing).min(1.0);
                }
            }
        }
    }

    /// Mark contiguous matches as gaps (do not collapse).
    fn try_remove_matches(&mut self, idx: usize) {
        if self.chain.is_empty() {
            return;
        }
        let len = self.chain.len();
        if idx >= len {
            return;
        }
        if self.chain[idx].color.is_none() {
            return;
        }
        let color = self.chain[idx].color.clone().unwrap();
        // left
        let mut left = 0usize;
        let mut cur = idx;
        while cur > 0 {
            let prev = cur - 1;
            if let Some(ref c) = self.chain[prev].color {
                if c == &color {
                    left += 1;
                    cur = prev;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        // right
        let mut right = 0usize;
        cur = idx;
        while cur + 1 < len {
            let nxt = cur + 1;
            if let Some(ref c) = self.chain[nxt].color {
                if c == &color {
                    right += 1;
                    cur = nxt;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        let total = 1 + left + right;
        if total >= 3 {
            let start = if idx >= left { idx - left } else { 0 };
            let end = (idx + right).min(len - 1);
            for i in start..=end {
                self.chain[i].color = None;
                self.chain[i].id = None;
            }
            info!("Marked {} matching marbles as gaps color={}", total, color);
        }
    }

    /// Snapshot: convert chain to world positions (excluding gaps) and send path control points for debug.
    pub fn snapshot(&self) -> String {
        let players: Vec<Player> = self.players.values().cloned().collect();
        let mut marbles: Vec<Marble> = self.marbles.clone();
        for cm in self.chain.iter() {
            if cm.color.is_none() {
                continue;
            }
            let (x, z) = self.chain_world_pos(cm.s);
            marbles.push(Marble {
                id: cm.id.unwrap_or(0),
                x,
                y: 0.5,
                z,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                life: 9999.0,
                color: cm.color.clone().unwrap_or_else(|| "unknown".into()),
                owner: None,
            });
        }
        json!({
            "type":"state",
            "players": players,
            "marbles": marbles,
            "path": {
                "path_points": self.path_points,
                "total_length": self.total_length,
                "samples_len": self.samples.len()
            }
        })
        .to_string()
    }
}

/// Helpers

fn yaw_sin(yaw: f32) -> f32 {
    yaw.sin()
}
fn yaw_cos(yaw: f32) -> f32 {
    yaw.cos()
}

fn random_color_with_rng(rng: &mut impl Rng) -> String {
    let colors = ["red", "green", "blue", "yellow", "purple"];
    let idx = (rng.random::<f32>() * (colors.len() as f32)) as usize;
    colors[idx % colors.len()].to_string()
}

fn random_color_chain(rng: &mut impl Rng, chain: &[ChainMarble]) -> String {
    let colors = ["red", "green", "blue", "yellow", "purple"];

    // If chain is empty or very small, just use random
    if chain.len() < 3 {
        let idx = (rng.random::<f32>() * (colors.len() as f32)) as usize;
        return colors[idx % colors.len()].to_string();
    }

    // 60% chance to duplicate a color from the last 10 marbles in the chain
    if rng.random::<f32>() < 0.6 {
        // Look at the last 10 marbles (or fewer if chain is shorter)
        let look_back = chain.len().min(10);
        let recent_marbles = &chain[chain.len() - look_back..];

        // Collect colors from recent marbles
        let recent_colors: Vec<String> = recent_marbles
            .iter()
            .filter_map(|m| m.color.clone())
            .collect();

        if !recent_colors.is_empty() {
            let idx = (rng.random::<f32>() * (recent_colors.len() as f32)) as usize;
            return recent_colors[idx % recent_colors.len()].clone();
        }
    }

    // 40% chance (or fallback): completely random color
    let idx = (rng.random::<f32>() * (colors.len() as f32)) as usize;
    colors[idx % colors.len()].to_string()
}

fn generate_token(rng: &mut impl Rng) -> String {
    let n = rng.random::<u128>();
    format!("{:032x}", n)
}
