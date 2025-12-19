use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
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

/// Free-moving marble (shot by players)
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

/// Marble stored on the path. `s` is parameter along the path in [0,1].
/// color == None indicates a gap (removed marble spot).
#[derive(Debug, Clone)]
pub struct ChainMarble {
    pub id: Option<u64>,       // None for gap slots
    pub s: f32,                // parameter along the path [0..1]
    pub color: Option<String>, // None => gap
}

#[derive(Debug)]
pub struct GameState {
    pub players: HashMap<SocketAddr, Player>, // connected players keyed by addr
    pub marbles: Vec<Marble>,                 // free marbles (shot by players)
    pub chain: Vec<ChainMarble>, // ordered sequence from start (s small) -> end (s close to 1)

    // Bezier control points for horseshoe-shaped path (x,z coordinates)
    pub p0: (f32, f32),
    pub p1: (f32, f32),
    pub p2: (f32, f32),
    pub p3: (f32, f32),

    // spawn / spacing / movement tuning
    pub spawn_accum: f32,
    pub spawn_interval: f32, // seconds between spawns
    pub marble_diameter: f32,
    pub spacing_length: f32, // desired arc-length spacing between marbles
    pub next_player_id: u64,
    pub next_marble_id: u64,

    // persistent mapping: token -> persistent player (keeps identity across reconnects)
    pub token_map: HashMap<String, PersistentPlayer>,
}

/// Persistent player record mapped by token. Kept across disconnects.
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

impl Default for GameState {
    fn default() -> Self {
        // Define horseshoe-like cubic Bezier control points.
        // These are in (x,z) plane. You can tweak them to change shape/scale.
        // Start near top-left, curve down under players, end top-right.
        let p0 = (-8.0_f32, 6.0_f32); // start (top-left)
        let p1 = (-8.0_f32, -4.0_f32); // pulls downward on left side
        let p2 = (8.0_f32, -4.0_f32); // pulls downward on right side
        let p3 = (8.0_f32, 6.0_f32); // end (top-right)

        let mut gs = GameState {
            players: HashMap::new(),
            marbles: Vec::new(),
            chain: Vec::new(),
            p0,
            p1,
            p2,
            p3,
            spawn_accum: 0.0,
            spawn_interval: 0.6, // spawn one new chain marble every 0.6s (tunable)
            marble_diameter: 0.6,
            spacing_length: 0.6 * 1.02, // slightly larger than diameter
            next_player_id: 0,
            next_marble_id: 0,
            token_map: HashMap::new(),
        };

        // initialize chain along the bezier horseshoe path
        let mut rng = rand::thread_rng();
        let colors = ["red", "green", "blue", "yellow", "purple"];
        let chain_len = 30usize; // requested
        let spacing_s = 1.0_f32 / (chain_len as f32); // initial parameter spacing (not arc-accurate)
        info!(
            "Initializing shared chain (horseshoe) len={} spacing_s={}",
            chain_len, spacing_s
        );

        // place marbles with s spaced from 0..(chain_len-1)/chain_len (so endpoint is not immediately full)
        for i in 0..chain_len {
            let mid = gs.next_marble_id;
            gs.next_marble_id += 1;
            // s in [0, 1) but not including 1.0 to avoid immediate removal
            let s = (i as f32) * spacing_s;
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

    /// Restore or create a player by token and bind it to addr.
    /// Returns (token, Player) — token will be newly generated if not provided or not found.
    pub fn join_with_token(
        &mut self,
        token_opt: Option<String>,
        addr: SocketAddr,
    ) -> (String, Player) {
        // If token provided and exists, restore persistent player
        if let Some(token) = token_opt {
            if let Some(pp) = self.token_map.get_mut(&token) {
                // rebind to new addr
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

        // Otherwise, create a new persistent player
        let mut rng = rand::thread_rng();
        let id = self.next_player_id;
        self.next_player_id += 1;

        // spawn position:
        // - first connected persistent player -> fixed left offset (-2.0, 0.0)
        // - second connected persistent player -> fixed right offset (2.0, 0.0)
        // - further players -> small random offset
        let connected_count = self.token_map.values().filter(|pp| pp.connected).count();
        let (px, pz) = match connected_count {
            0 => (-2.0_f32, 0.0_f32),
            1 => (2.0_f32, 0.0_f32),
            _ => {
                let angle = (id as f32) * 0.618;
                let random_val: f32 = rng.random();
                let radius = 2.0 + (random_val * 2.0);
                (radius * angle.sin(), radius * angle.cos())
            }
        };

        // pick loaded/next colors
        let loaded = random_color_with_rng(&mut rng);
        let next = random_color_with_rng(&mut rng);

        // generate token
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

    /// Mark persistent player disconnected by addr (keeps token mapping so reconnect can restore).
    pub fn disconnect_by_addr(&mut self, addr: &SocketAddr) {
        if let Some(p) = self.players.remove(addr) {
            // find persistent entry with same id and mark disconnected
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

    /// Update player's yaw for aiming (addr refers to current connection address).
    pub fn handle_aim(&mut self, addr: &SocketAddr, yaw: f32) {
        if let Some(p) = self.players.get_mut(addr) {
            p.yaw = yaw;
            // also update persistent
            for (_token, pp) in self.token_map.iter_mut() {
                if Some(addr.clone()) == pp.addr {
                    pp.yaw = yaw;
                    break;
                }
            }
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
            let mut rng = rand::thread_rng();
            p.next_color = random_color_with_rng(&mut rng);

            // update persistent record too
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

    /// Advance the simulation by dt seconds (physics, lifetime decay). Chain marbles move along the bezier path.
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

        // spawn new chain marbles periodically at the start (they start at s=0.0)
        self.spawn_accum += dt;
        while self.spawn_accum >= self.spawn_interval {
            self.spawn_accum -= self.spawn_interval;
            let mut rng = rand::thread_rng();
            let color = random_color_with_rng(&mut rng);
            let id = self.next_marble_id;
            self.next_marble_id += 1;
            // push at start (s = 0.0) — will remain at start until equalization moves them within first segment
            self.chain.push(ChainMarble {
                id: Some(id),
                s: 0.0,
                color: Some(color),
            });
        }

        // advance chain along the path: increase `s` for each non-gap chain marble
        let chain_speed = 0.03_f32; // fraction of path per second
        for cm in self.chain.iter_mut() {
            // gaps still advance as placeholders so the whole path moves visually (you can change this if desired)
            cm.s += chain_speed * dt;
        }

        // remove placeholders or marbles that reached or passed end (s >= 1.0)
        // keep gaps that are beyond endpoint removed as well
        self.chain.retain(|cm| cm.s < 1.0);

        // Re-equalize spacing along the path only within contiguous non-gap segments so gaps persist
        self.equalize_chain_spacing();

        // keep chain sorted by s (ascending)
        self.chain
            .sort_by(|a, b| a.s.partial_cmp(&b.s).unwrap_or(std::cmp::Ordering::Equal));

        // Collision detection + insertion + match removal
        let mut i = 0usize;
        while i < self.marbles.len() {
            let m = self.marbles[i].clone(); // clone to work with it
            if let Some(coll_idx) = self.find_collision_index(&m) {
                // NEW: don't insert if the immediate next slot after coll_idx is a gap.
                // This prevents filling a hole by shooting into it — shots into gaps are treated as misses.
                if coll_idx + 1 < self.chain.len() && self.chain[coll_idx + 1].color.is_none() {
                    // it's a gap region; ignore this collision
                    i += 1;
                    continue;
                }

                // insert marble into chain near coll_idx
                self.insert_into_chain(m, coll_idx);
                // remove free marble (swap_remove)
                self.marbles.swap_remove(i);
                // do not increment i, since we've swapped in a new element at i
                continue;
            }
            i += 1;
        }
    }

    /// Re-sample chain so marbles are equally spaced in arc-length, but preserve gaps.
    /// For each contiguous non-gap segment, anchor by the head (largest s in that segment),
    /// and place each marble backward by spacing_length.
    fn equalize_chain_spacing(&mut self) {
        if self.chain.is_empty() {
            return;
        }

        const SAMPLE_STEPS: usize = 64;

        // helper: arc length from 0 to s
        let arc_len_to = |s: f32, gs: &GameState| -> f32 {
            if s <= 0.0 {
                return 0.0;
            }
            let steps = SAMPLE_STEPS;
            let mut length = 0.0_f32;
            let mut prev = gs.chain_world_pos(0.0);
            for i in 1..=steps {
                let t = (i as f32) / (steps as f32) * s;
                let p = gs.chain_world_pos(t);
                let dx = p.0 - prev.0;
                let dz = p.1 - prev.1;
                length += (dx * dx + dz * dz).sqrt();
                prev = p;
            }
            length
        };

        // sort indices by s ascending to process segments in path order (tail -> head)
        let mut order: Vec<usize> = (0..self.chain.len()).collect();
        order.sort_by(|&a, &b| {
            self.chain[a]
                .s
                .partial_cmp(&self.chain[b].s)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // find contiguous non-gap segments in the ordered list
        let mut segments: Vec<Vec<usize>> = Vec::new();
        let mut current: Vec<usize> = Vec::new();
        for &idx in order.iter() {
            if self.chain[idx].color.is_some() {
                current.push(idx);
            } else {
                if !current.is_empty() {
                    segments.push(current);
                    current = Vec::new();
                }
            }
        }
        if !current.is_empty() {
            segments.push(current);
        }

        // Process each segment separately
        let spacing = self.spacing_length.max(0.001);

        for seg in segments.into_iter() {
            // seg is indices in ascending s (tail->head)
            // build a vector of (s, id, color)
            let mut seg_marbles: Vec<(f32, Option<u64>, String)> = seg
                .iter()
                .map(|&i| {
                    let cm = &self.chain[i];
                    (cm.s, cm.id, cm.color.clone().unwrap_or_default())
                })
                .collect();

            if seg_marbles.is_empty() {
                continue;
            }

            // compute s_head (max s in segment)
            let s_head = seg_marbles
                .iter()
                .map(|(s, _, _)| *s)
                .fold(seg_marbles[0].0, |a, b| a.max(b));

            // arc length to head
            let L_head = arc_len_to(s_head, self);

            // desired L positions head->tail for segment
            let m = seg_marbles.len();
            let mut desired_Ls_head_to_tail: Vec<f32> = Vec::with_capacity(m);
            for i in 0..m {
                let desired = L_head - (i as f32) * spacing;
                desired_Ls_head_to_tail.push(desired);
            }

            // inverse arc length (binary search)
            let inverse_arc = |L_target: f32, gs: &GameState, L_head: f32| -> f32 {
                if L_target <= 0.0 {
                    return 0.0_f32;
                }
                let target = if L_target > L_head { L_head } else { L_target };

                let mut low = 0.0_f32;
                let mut high = 1.0_f32;
                for _ in 0..24 {
                    let mid = (low + high) * 0.5;
                    let lm = arc_len_to(mid, gs);
                    if lm < target {
                        low = mid;
                    } else {
                        high = mid;
                    }
                }
                (low + high) * 0.5
            };

            // compute s values head->tail, then reverse to tail->head
            let mut s_head_to_tail: Vec<f32> = Vec::with_capacity(m);
            for desired_L in desired_Ls_head_to_tail.iter() {
                let s_new = if *desired_L <= 0.0 {
                    0.0_f32
                } else {
                    inverse_arc(*desired_L, self, L_head)
                };
                s_head_to_tail.push(s_new);
            }
            s_head_to_tail.reverse(); // now tail->head

            // assign new s back to self.chain at the corresponding indices (tail->head)
            for (j, &chain_idx) in seg.iter().enumerate() {
                self.chain[chain_idx].s = s_head_to_tail[j];
            }
        }
    }

    /// Determine index of closest chain marble within collision distance for a given free marble.
    /// Gaps are ignored.
    fn find_collision_index(&self, marble: &Marble) -> Option<usize> {
        const COLLISION_DISTANCE: f32 = 0.7_f32; // tuning parameter (marble radius ~0.5)
        if self.chain.is_empty() {
            return None;
        }

        let mut best: Option<(usize, f32)> = None;
        for (idx, cm) in self.chain.iter().enumerate() {
            if cm.color.is_none() {
                continue; // skip gaps
            }
            let (cx, cz) = self.chain_world_pos(cm.s);
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

    /// Insert a free marble into the chain near collided index.
    /// We insert between coll_idx and coll_idx+1 and set s to midpoint (in s, then re-equalize within that segment).
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

        let len = self.chain.len();
        let after = coll_idx;
        // find next non-gap after `after` to compute a sensible next_s
        let mut next_s = None;
        for j in after + 1..len {
            if self.chain[j].color.is_some() {
                next_s = Some(self.chain[j].s);
                break;
            }
        }
        let cur_s = self.chain[after].s;
        let next_s = next_s.unwrap_or((cur_s + 0.02_f32).min(0.9999_f32));

        let insert_s = (cur_s + next_s) * 0.5_f32;

        let new_cm = ChainMarble {
            id: Some(new_id),
            s: insert_s,
            color: Some(color),
        };

        self.chain.push(new_cm);
        // keep sorted by s
        self.chain
            .sort_by(|a, b| a.s.partial_cmp(&b.s).unwrap_or(std::cmp::Ordering::Equal));

        // find index of the newly inserted marble (by id)
        let inserted_idx = self
            .chain
            .iter()
            .position(|c| c.id == Some(new_id))
            .unwrap_or(0);
        self.try_remove_matches(inserted_idx);
    }

    /// Attempt to remove contiguous match around index. Removes sequence if len >= 3.
    /// Instead of collapsing the chain, we mark removed positions as gaps (color=None, id=None)
    /// so visual gaps remain.
    fn try_remove_matches(&mut self, idx: usize) {
        if self.chain.is_empty() {
            return;
        }
        let len = self.chain.len();
        if idx >= len {
            return;
        }

        // if idx is a gap already, nothing to do
        if self.chain[idx].color.is_none() {
            return;
        }

        // clone color to avoid borrow issues
        let color = self.chain[idx].color.clone().unwrap();

        // Count left (stop at gaps)
        let mut left_count = 0usize;
        let mut cur = idx;
        while cur > 0 {
            let prev = cur - 1;
            if let Some(ref c) = self.chain[prev].color {
                if c == &color {
                    left_count += 1;
                    cur = prev;
                } else {
                    break;
                }
            } else {
                break; // gap stops contiguous match
            }
        }

        // Count right (stop at gaps)
        let mut right_count = 0usize;
        cur = idx;
        while cur + 1 < len {
            let next = cur + 1;
            if let Some(ref c) = self.chain[next].color {
                if c == &color {
                    right_count += 1;
                    cur = next;
                } else {
                    break;
                }
            } else {
                break; // gap stops contiguous match
            }
        }

        let total = 1 + left_count + right_count;
        if total >= 3 {
            // compute start and end indices inclusive
            let start_idx = if idx >= left_count {
                idx - left_count
            } else {
                0
            };
            let end_idx = (idx + right_count).min(len - 1);

            // Mark range as gaps (preserve s positions so gaps remain)
            for i in start_idx..=end_idx {
                self.chain[i].color = None;
                self.chain[i].id = None;
            }
            info!(
                "Marked {} matching marbles as gaps of color {}",
                total, color
            );
        }
    }

    /// Compute world-space x,z of a point along the Bezier path for parameter s in [0..1]
    fn chain_world_pos(&self, s: f32) -> (f32, f32) {
        // cubic bezier: B(s) = (1-u)^3 P0 + 3(1-u)^2 u P1 + 3(1-u) u^2 P2 + u^3 P3
        let u = s.clamp(0.0, 1.0);
        let iu = 1.0 - u;
        let w0 = iu * iu * iu;
        let w1 = 3.0 * iu * iu * u;
        let w2 = 3.0 * iu * u * u;
        let w3 = u * u * u;
        let x = w0 * self.p0.0 + w1 * self.p1.0 + w2 * self.p2.0 + w3 * self.p3.0;
        let z = w0 * self.p0.1 + w1 * self.p1.1 + w2 * self.p2.1 + w3 * self.p3.1;
        (x, z)
    }

    /// Produce a JSON snapshot string of the current state to broadcast.
    /// This flattens both free marbles and path marbles into a single "marbles" array.
    /// Gaps are excluded from the snapshot so the client sees holes.
    pub fn snapshot(&self) -> String {
        // players
        let players: Vec<Player> = self.players.values().cloned().collect();

        // free marbles (clone)
        let mut marbles: Vec<Marble> = self.marbles.clone();

        // append chain marbles converted to Marble objects with computed world positions
        for cm in self.chain.iter() {
            if cm.color.is_none() {
                continue; // gap - don't include a marble
            }
            let (x, z) = self.chain_world_pos(cm.s);
            let y = 0.5_f32; // slightly above ground
            marbles.push(Marble {
                id: cm.id.unwrap_or(0),
                x,
                y,
                z,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                life: 9999.0,
                color: cm.color.clone().unwrap_or_else(|| "unknown".to_string()),
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

/// Small helper: random color chooser using rng.random()
fn random_color_with_rng(rng: &mut impl Rng) -> String {
    let colors = ["red", "green", "blue", "yellow", "purple"];
    let idx = (rng.random::<f32>() * (colors.len() as f32)) as usize;
    colors[idx % colors.len()].to_string()
}

/// Generate a simple hex token using RNG
fn generate_token(rng: &mut impl Rng) -> String {
    let n = rng.random::<u128>();
    format!("{:032x}", n)
}

/// helper sin/cos where yaw is radians, with x = sin(yaw), z = cos(yaw)
fn yaw_sin(yaw: f32) -> f32 {
    yaw.sin()
}
fn yaw_cos(yaw: f32) -> f32 {
    yaw.cos()
}
