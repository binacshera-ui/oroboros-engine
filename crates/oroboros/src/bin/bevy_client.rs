//! # GLITCH WARS - Cyberpunk Voxel Client
//!
//! "Tron meets The Matrix in a collapsing Voxel Simulation"
//!
//! AESTHETIC: Neon vertex colors, HDR bloom, void atmosphere
//! PLATFORM: WASM/WebGL2 + Native
//! TARGET: 60 FPS on integrated graphics
//!
//! PHYSICS: bevy_xpbd_3d (Enterprise-grade, WASM compatible)

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::core_pipeline::bloom::BloomSettings;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::FogSettings;

// ENTERPRISE PHYSICS
use bevy_xpbd_3d::prelude::*;
// NOTE: PhysicsDebugPlugin removed - was showing debug wireframes

// NOTE: bevy_flycam REMOVED - was causing noclip/flying
// All movement is now physics-based via bevy_xpbd_3d

use oroboros_procedural::{WorldManager, WorldManagerConfig, WorldSeed, ChunkCoord, CHUNK_SIZE};

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

// =============================================================================
// MULTIPLAYER NETWORKING (WASM WebSocket)
// =============================================================================

// WASM-specific networking uses thread_local for WebSocket (not Send/Sync)
#[cfg(target_arch = "wasm32")]
mod networking {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{WebSocket, MessageEvent, CloseEvent, ErrorEvent};
    use std::sync::{Arc, Mutex};
    use std::collections::HashMap;
    use std::cell::RefCell;
    use bevy::prelude::*;
    
    /// Server URL - configurable
    pub const SERVER_URL: &str = "ws://162.55.2.222:3000";
    
    // Thread-local storage for WebSocket (not Send/Sync safe)
    thread_local! {
        static WEBSOCKET: RefCell<Option<WebSocket>> = RefCell::new(None);
    }
    
    /// Remote player data from server (with interpolation target)
    #[derive(Clone, Debug, Default)]
    #[allow(dead_code)]
    pub struct RemotePlayer {
        pub id: String,
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub yaw: f32,
        pub balance: u32,
        pub energy: f32,
        // Interpolation: previous position for smooth movement
        pub prev_x: f32,
        pub prev_y: f32,
        pub prev_z: f32,
        pub interp_t: f32, // 0.0 to 1.0
    }
    
    /// Network state resource (Send + Sync safe)
    #[derive(Resource, Default)]
    pub struct NetworkState {
        pub connected: bool,
        pub my_id: Option<String>,
        pub player_count: u32,
        pub ping_ms: u32,
        pub server_tick: u32,
        pub my_balance: u32,
        pub my_energy: f32,
        pub remote_players: Arc<Mutex<HashMap<String, RemotePlayer>>>,
    }
    
    // Static message queue for cross-callback communication
    static MESSAGE_QUEUE: Mutex<Vec<String>> = Mutex::new(Vec::new());
    
    impl NetworkState {
        /// Connect to the authoritative game server
        pub fn connect(&mut self) {
            web_sys::console::log_1(&format!("[NET] Connecting to {}...", SERVER_URL).into());
            
            let ws = match WebSocket::new(SERVER_URL) {
                Ok(ws) => ws,
                Err(e) => {
                    web_sys::console::error_1(&format!("[NET] WebSocket creation failed: {:?}", e).into());
                    return;
                }
            };
            
            ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
            
            // On message - push to static queue
            let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
                if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                    let msg_str: String = txt.into();
                    if let Ok(mut queue) = MESSAGE_QUEUE.lock() {
                        queue.push(msg_str);
                    }
                }
            });
            ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
            onmessage_callback.forget();
            
            // On open - send LOGIN immediately
            let onopen_callback = Closure::<dyn FnMut()>::new(move || {
                web_sys::console::log_1(&"[NET] ✓ WebSocket connected! Sending LOGIN...".into());
                let _ = js_sys::eval("if(window.updateServerStatus) updateServerStatus(true, 1, 0);");
                let _ = js_sys::eval("if(window.addNotification) addNotification('CONNECTED - AUTHENTICATING...', 'info');");
                
                // ================================================================
                // QA TEST: Randomize wallet prefix (50% whale, 50% noob)
                // ================================================================
                let random_val = js_sys::Math::random();
                let mock_wallet = if random_val < 0.5 {
                    // WHALE - will get $5000, WHALE tier
                    let suffix = format!("{:032x}", js_sys::Date::now() as u64);
                    format!("0xwhale{}", &suffix[..34])
                } else {
                    // NOOB - will get $50, FREE tier
                    let suffix = format!("{:032x}", js_sys::Date::now() as u64);
                    format!("0xnoob0{}", &suffix[..33])
                };
                
                let login_msg = format!(r#"{{"type":"LOGIN","wallet":"{}"}}"#, mock_wallet);
                
                // Huge log for QA
                web_sys::console::log_1(&"╔══════════════════════════════════════════════════════════════╗".into());
                web_sys::console::log_1(&"║               GLITCH WARS - LOGIN ATTEMPT                   ║".into());
                web_sys::console::log_1(&"╠══════════════════════════════════════════════════════════════╣".into());
                web_sys::console::log_1(&format!("║  Wallet: {}  ║", &mock_wallet).into());
                web_sys::console::log_1(&format!("║  Type: {} (random={:.2})                               ║", 
                    if random_val < 0.5 { "WHALE 🐋" } else { "NOOB  🆓" }, random_val).into());
                web_sys::console::log_1(&"╚══════════════════════════════════════════════════════════════╝".into());
                
                // Send LOGIN via thread-local websocket
                WEBSOCKET.with(|ws_cell| {
                    if let Some(ws) = ws_cell.borrow().as_ref() {
                        if ws.ready_state() == WebSocket::OPEN {
                            let _ = ws.send_with_str(&login_msg);
                        }
                    }
                });
            });
            ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
            onopen_callback.forget();
            
            // On close
            let onclose_callback = Closure::<dyn FnMut(_)>::new(move |e: CloseEvent| {
                web_sys::console::log_1(&format!("[NET] Disconnected: code={} reason={}", e.code(), e.reason()).into());
                let _ = js_sys::eval("if(window.updateServerStatus) updateServerStatus(false, 0, 0);");
                let _ = js_sys::eval("if(window.addNotification) addNotification('DISCONNECTED FROM SERVER', 'danger');");
            });
            ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
            onclose_callback.forget();
            
            // On error
            let onerror_callback = Closure::<dyn FnMut(_)>::new(move |_e: ErrorEvent| {
                web_sys::console::error_1(&"[NET] ✗ WebSocket error".into());
            });
            ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
            onerror_callback.forget();
            
            // Store in thread-local
            WEBSOCKET.with(|ws_cell| {
                *ws_cell.borrow_mut() = Some(ws);
            });
        }
        
        /// Send INPUT message (position update to authoritative server)
        pub fn send_input(&self, x: f32, y: f32, z: f32, yaw: f32) {
            let msg = format!(r#"{{"type":"INPUT","x":{},"y":{},"z":{},"yaw":{}}}"#, x, y, z, yaw);
            WEBSOCKET.with(|ws_cell| {
                if let Some(ws) = ws_cell.borrow().as_ref() {
                    if ws.ready_state() == WebSocket::OPEN {
                        let _ = ws.send_with_str(&msg);
                    }
                }
            });
        }
        
        /// Send MINE_BLOCK action (used when breaking blocks for economy)
        #[allow(dead_code)]
        pub fn send_mine_block(&self, block_x: i32, block_y: i32, block_z: i32) {
            let msg = format!(r#"{{"type":"MINE_BLOCK","blockX":{},"blockY":{},"blockZ":{}}}"#, block_x, block_y, block_z);
            WEBSOCKET.with(|ws_cell| {
                if let Some(ws) = ws_cell.borrow().as_ref() {
                    if ws.ready_state() == WebSocket::OPEN {
                        let _ = ws.send_with_str(&msg);
                    }
                }
            });
        }
        
        /// Send PING for latency measurement
        pub fn send_ping(&self) {
            let now = js_sys::Date::now();
            let msg = format!(r#"{{"type":"PING","timestamp":{}}}"#, now as u64);
            WEBSOCKET.with(|ws_cell| {
                if let Some(ws) = ws_cell.borrow().as_ref() {
                    if ws.ready_state() == WebSocket::OPEN {
                        let _ = ws.send_with_str(&msg);
                    }
                }
            });
        }
        
        /// Process received messages from authoritative server
        pub fn process_messages(&mut self) {
            let messages: Vec<String> = {
                let mut queue = MESSAGE_QUEUE.lock().unwrap();
                queue.drain(..).collect()
            };
            
            for msg_str in messages {
                // Parse message type - new multi-room protocol
                if msg_str.contains(r#""type":"CONNECTED""#) {
                    web_sys::console::log_1(&"[NET] Received CONNECTED, awaiting LOGIN response...".into());
                } else if msg_str.contains(r#""type":"ROOM_JOINED""#) {
                    self.handle_room_joined(&msg_str);
                } else if msg_str.contains(r#""type":"WELCOME""#) {
                    self.handle_welcome(&msg_str);
                } else if msg_str.contains(r#""type":"STATE""#) {
                    self.handle_state(&msg_str);
                } else if msg_str.contains(r#""type":"PLAYER_JOINED""#) {
                    self.handle_player_join(&msg_str);
                } else if msg_str.contains(r#""type":"PLAYER_LEFT""#) {
                    self.handle_player_leave(&msg_str);
                } else if msg_str.contains(r#""type":"MINE_SUCCESS""#) {
                    self.handle_mine_success(&msg_str);
                } else if msg_str.contains(r#""type":"BALANCE_UPDATE""#) {
                    self.handle_balance_update(&msg_str);
                } else if msg_str.contains(r#""type":"POSITION_CORRECTION""#) {
                    self.handle_position_correction(&msg_str);
                } else if msg_str.contains(r#""type":"PONG""#) {
                    self.handle_pong(&msg_str);
                } else if msg_str.contains(r#""type":"LOGIN_FAILED""#) {
                    self.handle_login_failed(&msg_str);
                }
            }
            
            // Update HUD with current state
            let _ = js_sys::eval(&format!(
                "if(window.updateServerStatus) updateServerStatus({}, {}, {});",
                self.connected, self.player_count, self.ping_ms
            ));
            let _ = js_sys::eval(&format!(
                "if(window.updateBalance) updateBalance({});",
                self.my_balance
            ));
            let _ = js_sys::eval(&format!(
                "if(window.updateEnergy) updateEnergy({});",
                self.my_energy as u32
            ));
        }
        
        /// Handle WELCOME message from server (legacy support)
        fn handle_welcome(&mut self, msg: &str) {
            // Parse: {"type":"WELCOME","id":"uuid","tick":0,"spawn":{...},"config":{...}}
            if let Some(id_start) = msg.find(r#""id":""#) {
                let id_start = id_start + 6;
                if let Some(id_end) = msg[id_start..].find('"') {
                    let my_id = msg[id_start..id_start+id_end].to_string();
                    web_sys::console::log_1(&format!("[NET] ✓ Registered as player {}", &my_id[..8]).into());
                    self.my_id = Some(my_id);
                    self.connected = true;
                    let _ = js_sys::eval("if(window.addNotification) addNotification('AUTHENTICATED WITH SERVER', 'success');");
                }
            }
        }
        
        /// Handle ROOM_JOINED message (multi-room protocol)
        fn handle_room_joined(&mut self, msg: &str) {
            // Parse: {"type":"ROOM_JOINED","roomId":"...","tier":"FREE/PREMIUM/WHALE",...}
            
            // Extract player ID
            if let Some(id_start) = msg.find(r#""playerId":""#) {
                let id_start = id_start + 12;
                if let Some(id_end) = msg[id_start..].find('"') {
                    self.my_id = Some(msg[id_start..id_start+id_end].to_string());
                }
            }
            
            // Extract tier and determine multiplier
            let (tier, multiplier, emoji) = if msg.contains(r#""tier":"WHALE""#) {
                ("WHALE", "5.0", "🐋")
            } else if msg.contains(r#""tier":"PREMIUM""#) {
                ("PREMIUM", "2.5", "💎")
            } else {
                ("FREE", "1.0", "🆓")
            };
            
            // Extract room name
            let room_name = if let Some(name_start) = msg.find(r#""name":""#) {
                let name_start = name_start + 8;
                if let Some(name_end) = msg[name_start..].find('"') {
                    msg[name_start..name_start+name_end].to_string()
                } else {
                    "Unknown".to_string()
                }
            } else {
                "Unknown".to_string()
            };
            
            self.connected = true;
            
            // ================================================================
            // QA: HUGE LOG FOR VISUAL CONFIRMATION
            // ================================================================
            web_sys::console::log_1(&"".into());
            web_sys::console::log_1(&"╔═══════════════════════════════════════════════════════════════════════════╗".into());
            web_sys::console::log_1(&"║                                                                           ║".into());
            web_sys::console::log_1(&format!("║   {}  ENTERED {} ROOM  {}                                          ║", emoji, tier, emoji).into());
            web_sys::console::log_1(&"║                                                                           ║".into());
            web_sys::console::log_1(&format!("║   LOOT MULTIPLIER: {}x                                                   ║", multiplier).into());
            web_sys::console::log_1(&format!("║   ROOM: {}                                                     ║", room_name).into());
            web_sys::console::log_1(&"║                                                                           ║".into());
            web_sys::console::log_1(&"╚═══════════════════════════════════════════════════════════════════════════╝".into());
            web_sys::console::log_1(&"".into());
            
            // HUGE on-screen notification
            let _ = js_sys::eval(&format!(
                "if(window.addNotification) addNotification('ENTERED {} ROOM - {}x LOOT {}', 'success');",
                tier, multiplier, emoji
            ));
        }
        
        /// Handle MINE_SUCCESS message
        fn handle_mine_success(&mut self, msg: &str) {
            // Parse: {"type":"MINE_SUCCESS","reward":50,"newBalance":150,"wasGold":true}
            
            // Extract reward
            let reward = if let Some(start) = msg.find(r#""reward":"#) {
                let start = start + 9;
                let end = msg[start..].find(',').or_else(|| msg[start..].find('}')).unwrap_or(0);
                msg[start..start+end].parse::<u32>().unwrap_or(0)
            } else {
                0
            };
            
            // Extract new balance
            if let Some(start) = msg.find(r#""newBalance":"#) {
                let start = start + 13;
                let end = msg[start..].find(',').or_else(|| msg[start..].find('}')).unwrap_or(0);
                if let Ok(balance) = msg[start..start+end].parse::<u32>() {
                    self.my_balance = balance;
                }
            }
            
            let was_gold = msg.contains(r#""wasGold":true"#);
            
            if was_gold {
                let _ = js_sys::eval(&format!(
                    "if(window.addNotification) addNotification('GOLD MINED! +${}', 'success');",
                    reward
                ));
            }
            
            // Update HUD
            let _ = js_sys::eval(&format!(
                "if(window.updateBalance) updateBalance({});",
                self.my_balance
            ));
        }
        
        /// Handle LOGIN_FAILED message
        fn handle_login_failed(&mut self, msg: &str) {
            let reason = if msg.contains(r#""reason":"INVALID_WALLET""#) {
                "Invalid wallet address"
            } else if msg.contains(r#""reason":"ROOM_FULL""#) {
                "Room is full"
            } else {
                "Unknown error"
            };
            
            web_sys::console::error_1(&format!("[NET] ✗ Login failed: {}", reason).into());
            let _ = js_sys::eval(&format!(
                "if(window.addNotification) addNotification('LOGIN FAILED: {}', 'danger');",
                reason
            ));
        }
        
        /// Handle STATE message (authoritative world snapshot)
        fn handle_state(&mut self, msg: &str) {
            // Parse: {"type":"STATE","tick":123,"time":1234567890,"players":[...]}
            
            // Extract tick
            if let Some(tick_start) = msg.find(r#""tick":"#) {
                let tick_start = tick_start + 7;
                if let Some(tick_end) = msg[tick_start..].find(',') {
                    if let Ok(tick) = msg[tick_start..tick_start+tick_end].parse::<u32>() {
                        self.server_tick = tick;
                    }
                }
            }
            
            // Parse players array
            if let Some(players_start) = msg.find(r#""players":["#) {
                let players_start = players_start + 11;
                if let Some(players_end) = msg[players_start..].find(']') {
                    let players_json = &msg[players_start..players_start+players_end];
                    self.parse_state_players(players_json);
                }
            }
        }
        
        /// Parse players from STATE message
        fn parse_state_players(&mut self, players_json: &str) {
            let mut players_map = self.remote_players.lock().unwrap();
            let mut seen_ids = std::collections::HashSet::new();
            
            // Parse each player object
            for player_str in players_json.split("},{") {
                let player_str = player_str.trim_start_matches('{').trim_end_matches('}');
                
                // Parse id
                let mut id = String::new();
                if let Some(id_start) = player_str.find(r#""id":""#) {
                    let id_start = id_start + 6;
                    if let Some(id_end) = player_str[id_start..].find('"') {
                        id = player_str[id_start..id_start+id_end].to_string();
                    }
                }
                
                if id.is_empty() { continue; }
                seen_ids.insert(id.clone());
                
                // Parse numeric fields
                let x = Self::parse_f32(player_str, r#""x":"#);
                let y = Self::parse_f32(player_str, r#""y":"#);
                let z = Self::parse_f32(player_str, r#""z":"#);
                let yaw = Self::parse_f32(player_str, r#""yaw":"#);
                let balance = Self::parse_u32(player_str, r#""balance":"#);
                let energy = Self::parse_f32(player_str, r#""energy":"#);
                
                // Update or create player with interpolation
                if let Some(existing) = players_map.get_mut(&id) {
                    // Store previous for interpolation
                    existing.prev_x = existing.x;
                    existing.prev_y = existing.y;
                    existing.prev_z = existing.z;
                    existing.x = x;
                    existing.y = y;
                    existing.z = z;
                    existing.yaw = yaw;
                    existing.balance = balance;
                    existing.energy = energy;
                    existing.interp_t = 0.0; // Reset interpolation
                } else {
                    // New player
                    players_map.insert(id.clone(), RemotePlayer {
                        id,
                        x, y, z, yaw,
                        balance, energy,
                        prev_x: x, prev_y: y, prev_z: z,
                        interp_t: 1.0,
                    });
                }
            }
            
            // Update player count
            self.player_count = seen_ids.len() as u32;
            
            // Remove players no longer in state
            players_map.retain(|id, _| seen_ids.contains(id));
        }
        
        /// Helper: parse f32 from JSON string
        fn parse_f32(s: &str, key: &str) -> f32 {
            if let Some(start) = s.find(key) {
                let start = start + key.len();
                let end = s[start..].find(|c: char| c == ',' || c == '}').unwrap_or(s.len() - start);
                s[start..start+end].parse().unwrap_or(0.0)
            } else {
                0.0
            }
        }
        
        /// Helper: parse u32 from JSON string
        fn parse_u32(s: &str, key: &str) -> u32 {
            if let Some(start) = s.find(key) {
                let start = start + key.len();
                let end = s[start..].find(|c: char| c == ',' || c == '}').unwrap_or(s.len() - start);
                s[start..start+end].parse().unwrap_or(0)
            } else {
                0
            }
        }
        
        fn handle_player_join(&self, msg: &str) {
            if let Some(count_start) = msg.find(r#""playerCount":"#) {
                let count_start = count_start + 14;
                if let Some(count_end) = msg[count_start..].find('}') {
                    if let Ok(_count) = msg[count_start..count_start+count_end].parse::<u32>() {
                        let _ = js_sys::eval("if(window.addNotification) addNotification('PLAYER JOINED THE ARENA', 'info');");
                    }
                }
            }
        }
        
        fn handle_player_leave(&self, msg: &str) {
            if let Some(id_start) = msg.find(r#""id":""#) {
                let id_start = id_start + 6;
                if let Some(id_end) = msg[id_start..].find('"') {
                    let left_id = &msg[id_start..id_start+id_end];
                    web_sys::console::log_1(&format!("[NET] Player {} left", &left_id[..8.min(left_id.len())]).into());
                    let _ = js_sys::eval("if(window.addNotification) addNotification('PLAYER LEFT THE ARENA', 'warning');");
                }
            }
        }
        
        fn handle_balance_update(&mut self, msg: &str) {
            // {"type":"BALANCE_UPDATE","balance":100,"energy":95,"reason":"MINE_BLOCK"}
            if let Some(bal_start) = msg.find(r#""balance":"#) {
                let bal_start = bal_start + 10;
                if let Some(bal_end) = msg[bal_start..].find(',') {
                    if let Ok(balance) = msg[bal_start..bal_start+bal_end].parse::<u32>() {
                        self.my_balance = balance;
                        let _ = js_sys::eval(&format!(
                            "if(window.updateBalance) updateBalance({});",
                            balance
                        ));
                    }
                }
            }
            if let Some(eng_start) = msg.find(r#""energy":"#) {
                let eng_start = eng_start + 9;
                if let Some(eng_end) = msg[eng_start..].find(',').or_else(|| msg[eng_start..].find('}')) {
                    if let Ok(energy) = msg[eng_start..eng_start+eng_end].parse::<f32>() {
                        self.my_energy = energy;
                    }
                }
            }
        }
        
        fn handle_position_correction(&self, msg: &str) {
            // Server rejected our movement - need to rubberband
            web_sys::console::warn_1(&format!("[NET] Position corrected by server: {}", msg).into());
            let _ = js_sys::eval("if(window.addNotification) addNotification('MOVEMENT REJECTED', 'danger');");
        }
        
        fn handle_pong(&mut self, msg: &str) {
            // {"type":"PONG","clientTime":123,"serverTime":456}
            if let Some(ct_start) = msg.find(r#""clientTime":"#) {
                let ct_start = ct_start + 13;
                if let Some(ct_end) = msg[ct_start..].find(',') {
                    if let Ok(client_time) = msg[ct_start..ct_start+ct_end].parse::<f64>() {
                        let now = js_sys::Date::now();
                        self.ping_ms = ((now - client_time) / 2.0) as u32;
                    }
                }
            }
        }
        
        /// Advance interpolation for smooth remote player movement
        pub fn update_interpolation(&mut self, dt: f32) {
            let mut players_map = self.remote_players.lock().unwrap();
            for player in players_map.values_mut() {
                // Smoothly interpolate from prev to current over ~33ms (one server tick)
                player.interp_t = (player.interp_t + dt * 30.0).min(1.0);
            }
        }
        
        /// Get interpolated position for a remote player (alternative API)
        #[allow(dead_code)]
        pub fn get_interpolated_pos(&self, player_id: &str) -> Option<(f32, f32, f32, f32)> {
            let players_map = self.remote_players.lock().ok()?;
            let player = players_map.get(player_id)?;
            
            let t = player.interp_t;
            let x = player.prev_x + (player.x - player.prev_x) * t;
            let y = player.prev_y + (player.y - player.prev_y) * t;
            let z = player.prev_z + (player.z - player.prev_z) * t;
            
            Some((x, y, z, player.yaw))
        }
    }
    
    /// Component marker for remote player ghosts
    #[derive(Component)]
    pub struct RemotePlayerGhost {
        pub player_id: String,
    }
}

/// System to update remote player ghosts with smooth interpolation
#[cfg(target_arch = "wasm32")]
fn update_remote_players_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    network: Res<networking::NetworkState>,
    mut ghosts: Query<(Entity, &networking::RemotePlayerGhost, &mut Transform)>,
) {
    use std::collections::HashSet;
    
    let remote_players = network.remote_players.lock().unwrap();
    let my_id_str = network.my_id.clone();
    
    // Track which ghosts exist
    let mut existing_ids: HashSet<String> = HashSet::new();
    
    // Update existing ghosts with server-side interpolation
    for (entity, ghost, mut transform) in ghosts.iter_mut() {
        // Skip our own ID
        if Some(&ghost.player_id) == my_id_str.as_ref() {
            commands.entity(entity).despawn();
            continue;
        }
        
        if let Some(player) = remote_players.get(&ghost.player_id) {
            // Use interpolated position for smooth movement
            let t = player.interp_t;
            let x = player.prev_x + (player.x - player.prev_x) * t;
            let y = player.prev_y + (player.y - player.prev_y) * t;
            let z = player.prev_z + (player.z - player.prev_z) * t;
            
            let target = Vec3::new(
                x * BLOCK_SCALE,
                y * BLOCK_SCALE,
                z * BLOCK_SCALE,
            );
            
            // Additional client-side smoothing
            transform.translation = transform.translation.lerp(target, 0.4);
            transform.rotation = Quat::from_rotation_y(player.yaw);
            existing_ids.insert(ghost.player_id.clone());
        } else {
            // Player disconnected - remove ghost
            commands.entity(entity).despawn();
        }
    }
    
    // Spawn new ghosts for new players
    for (id, player) in remote_players.iter() {
        // Skip our own ID
        if Some(id) == my_id_str.as_ref() {
            continue;
        }
        
        if !existing_ids.contains(id) {
            // Spawn new ghost (RED cube)
            commands.spawn((
                networking::RemotePlayerGhost { player_id: id.clone() },
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(
                        BLOCK_SCALE * 2.0,
                        BLOCK_SCALE * 3.0,
                        BLOCK_SCALE * 2.0,
                    )),
                    material: materials.add(StandardMaterial {
                        base_color: Color::rgba(1.0, 0.2, 0.2, 0.8),
                        emissive: Color::rgb(2.0, 0.3, 0.3),
                        alpha_mode: bevy::pbr::AlphaMode::Blend,
                        ..default()
                    }),
                    transform: Transform::from_xyz(
                        player.x * BLOCK_SCALE,
                        player.y * BLOCK_SCALE,
                        player.z * BLOCK_SCALE,
                    ),
                    ..default()
                },
                Name::new(format!("Ghost_{}", &id[..8.min(id.len())])),
            ));
            
            #[cfg(target_arch = "wasm32")]
            web_sys::console::log_1(&format!("[NET] Spawned ghost for player {}", &id[..8.min(id.len())]).into());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod networking {
    use bevy::prelude::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    
    // Stub for non-WASM builds
    #[derive(Resource, Default)]
    pub struct NetworkState {
        pub connected: bool,
        pub my_id: Option<String>,
        pub player_count: u32,
        pub ping_ms: u32,
        pub remote_players: Arc<Mutex<HashMap<String, RemotePlayer>>>,
    }
    
    #[derive(Clone)]
    pub struct RemotePlayer {
        pub id: String,
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub yaw: f32,
        pub loot: u32,
    }
    
    impl NetworkState {
        pub fn connect(&mut self) {}
        pub fn send_position(&self, _x: f32, _y: f32, _z: f32, _yaw: f32) {}
        pub fn send_ping(&mut self) {}
        pub fn process_messages(&mut self) {}
    }
    
    #[derive(Component)]
    pub struct RemotePlayerGhost {
        pub player_id: String,
    }
    
    pub fn update_remote_players() {}
}

// =============================================================================
// CONFIGURATION
// =============================================================================

/// UNDERCITY: 9x9 chunks = 288x288 blocks with 3D caves
/// Large enough for exploration, small enough for WASM performance
const LOAD_RADIUS: i32 = 4; // -4..4 = 9x9 chunks centered at origin

/// World seed for procedural generation
const WORLD_SEED: u64 = 42;

/// Block scale - each voxel is 0.25 units (quarter size)
/// This makes the world feel larger and more detailed
const BLOCK_SCALE: f32 = 0.25;

// =============================================================================
// ECONOMY ENGINE - The Weight System & Staking Logic
// =============================================================================

/// Player's economic state
#[derive(Component, Default)]
#[allow(dead_code)]
struct PlayerEconomy {
    /// Number of crystals collected
    loot_count: u32,
    /// Total value in virtual currency
    net_worth: f32,
    /// Current load affecting movement
    current_load: f32,
    /// Stamina/Energy (depletes over time)
    stamina: f32,
}

/// Map tiers based on staking amount
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
enum MapTier {
    /// Low stake: Small map, low loot density
    Scavenger,
    /// High stake: Large map with caves, high loot
    HighRoller,
    /// VIP: Maximum map size, rare spawns
    Whale,
}

impl MapTier {
    /// Determines map tier based on staked amount (mockup)
    #[allow(dead_code)]
    fn from_stake(stake_amount: f32) -> Self {
        if stake_amount >= 10000.0 {
            MapTier::Whale
        } else if stake_amount >= 1000.0 {
            MapTier::HighRoller
        } else {
            MapTier::Scavenger
        }
    }
    
    /// Returns load radius for this tier
    #[allow(dead_code)]
    fn load_radius(&self) -> i32 {
        match self {
            MapTier::Scavenger => 2,  // 5x5 chunks
            MapTier::HighRoller => 4, // 9x9 chunks
            MapTier::Whale => 6,      // 13x13 chunks
        }
    }
}

/// Calculate movement damping based on carried loot
/// Rich players move like trucks!
#[allow(dead_code)]
fn calculate_load_damping(loot_count: u32) -> f32 {
    5.0 + (loot_count as f32 * 0.5)
}

/// Death tax calculation
/// On death: 20% burned, 30% dropped as loot
#[allow(dead_code)]
struct DeathTax {
    burn_amount: f32,   // Permanently destroyed
    drop_amount: f32,   // Becomes lootable
    kept_amount: f32,   // Player retains this
}

impl DeathTax {
    #[allow(dead_code)]
    fn calculate(wallet_value: f32) -> Self {
        Self {
            burn_amount: wallet_value * 0.2,
            drop_amount: wallet_value * 0.3,
            kept_amount: wallet_value * 0.5,
        }
    }
}

// =============================================================================
// BACKEND BRIDGE - Wraps our existing backend for Bevy
// =============================================================================

/// Inner data that requires mutex protection
struct BackendInner {
    /// The world manager from oroboros_procedural
    world_manager: WorldManager,
    
    /// Currently loaded chunks
    loaded_chunks: HashSet<ChunkCoord>,
    
    /// Chunks that need mesh regeneration
    dirty_chunks: HashSet<ChunkCoord>,
    
    /// Last player chunk position
    last_player_chunk: Option<ChunkCoord>,
}

/// The bridge between our custom backend and Bevy's ECS
/// 
/// Wrapped in Mutex because WorldManager contains non-Sync types
#[derive(Resource)]
pub struct BackendBridge {
    inner: Mutex<BackendInner>,
}

impl BackendBridge {
    /// Creates a new bridge with initialized world
    pub fn new(seed: u64) -> Self {
        let config = WorldManagerConfig {
            load_radius: LOAD_RADIUS,
            unload_radius: LOAD_RADIUS + 2,
            max_chunks_per_frame: 4,
            world_save_path: std::path::PathBuf::from("world/chunks"),
        };
        
        let world_manager = WorldManager::new(WorldSeed::new(seed), config);
        
        info!("BackendBridge initialized with seed {}", seed);
        
        Self {
            inner: Mutex::new(BackendInner {
                world_manager,
                loaded_chunks: HashSet::new(),
                dirty_chunks: HashSet::new(),
                last_player_chunk: None,
            }),
        }
    }
}

// =============================================================================
// CHUNK ENTITY TRACKING
// =============================================================================

/// Component to mark entities as chunk meshes
#[derive(Component)]
pub struct ChunkMesh {
    /// The chunk coordinate this mesh represents.
    pub coord: ChunkCoord,
}

/// Tracks which chunks have been rendered
#[derive(Resource, Default)]
pub struct RenderedChunks {
    /// Map of chunk coordinates to their entity IDs.
    pub chunks: HashMap<ChunkCoord, Entity>,
}

// =============================================================================
// BLOCK TYPES (from backend)
// =============================================================================

/// Block types for BRUTALIST MEGA-STRUCTURE
/// Inspired by: Alice in Borderland + Squid Game
/// Palette: Concrete Grey, Neon Red, Ice White
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
enum BlockType {
    Air = 0,
    /// Floor - Matte Grey Concrete
    ConcreteFloor = 1,
    /// Walls - Darker Concrete  
    ConcreteWall = 2,
    /// Hazard - Glowing Red Neon (DEATH)
    HazardNeon = 3,
    /// Goal - Ice White Laser Floor
    GoalZone = 4,
    /// Bedrock - Indestructible Black
    Bedrock = 5,
    /// Gold Loot - Contrasting Gold
    GoldLoot = 6,
    /// Bridge/Beam - Weathered Metal
    MetalBridge = 7,
}

impl BlockType {
    fn from_id(id: u16) -> Self {
        match id {
            0 => BlockType::Air,
            1 => BlockType::ConcreteFloor,
            2 => BlockType::ConcreteWall,
            3 => BlockType::HazardNeon,
            4 => BlockType::GoalZone,
            5 => BlockType::Bedrock,
            6 => BlockType::GoldLoot,
            7 => BlockType::MetalBridge,
            _ => BlockType::ConcreteWall,
        }
    }
    
    /// Returns RGBA vertex color for BRUTALIST PALETTE
    /// Grey concrete, red neon hazards, white goals
    fn vertex_color(&self) -> [f32; 4] {
        match self {
            // ID 0: Air -> Invisible
            BlockType::Air => [0.0, 0.0, 0.0, 0.0],
            
            // ID 1: Concrete Floor -> #505050 Matte Grey
            // The arena floor - visible but muted
            BlockType::ConcreteFloor => [0.314, 0.314, 0.314, 1.0],
            
            // ID 2: Concrete Wall -> #303030 Darker Grey
            // Massive brutalist walls
            BlockType::ConcreteWall => [0.188, 0.188, 0.188, 1.0],
            
            // ID 3: Hazard Neon -> #FF0000 GLOWING RED
            // HDR emission for bloom - DEATH ZONE
            BlockType::HazardNeon => [5.0, 0.0, 0.0, 1.0],
            
            // ID 4: Goal Zone -> #E0FFFF Ice White Glow
            // The extraction point - safety
            BlockType::GoalZone => [4.0, 5.0, 5.0, 1.0],
            
            // ID 5: Bedrock -> #101010 Near Black
            // Indestructible foundation
            BlockType::Bedrock => [0.063, 0.063, 0.063, 1.0],
            
            // ID 6: Gold Loot -> Bright Gold (contrasts grey)
            // Valuable collectible
            BlockType::GoldLoot => [4.0, 3.0, 0.3, 1.0],
            
            // ID 7: Metal Bridge -> #404045 Weathered Steel
            // Thin walkways connecting platforms
            BlockType::MetalBridge => [0.25, 0.25, 0.27, 1.0],
        }
    }
    
    /// Returns PBR Metallic value
    #[allow(dead_code)]
    fn metallic(&self) -> f32 {
        match self {
            BlockType::Air => 0.0,
            BlockType::ConcreteFloor => 0.0,  // Concrete is not metal
            BlockType::ConcreteWall => 0.0,   // Concrete is not metal
            BlockType::HazardNeon => 0.1,     // Slight metallic sheen
            BlockType::GoalZone => 0.2,       // Slight metallic
            BlockType::Bedrock => 0.0,        // Matte black
            BlockType::GoldLoot => 1.0,       // Pure metal
            BlockType::MetalBridge => 0.9,    // Steel
        }
    }
    
    /// Returns PBR Roughness value (0.0 = mirror, 1.0 = matte)
    #[allow(dead_code)]
    fn roughness(&self) -> f32 {
        match self {
            BlockType::Air => 1.0,
            BlockType::ConcreteFloor => 0.9,  // Very matte concrete
            BlockType::ConcreteWall => 1.0,   // Completely matte
            BlockType::HazardNeon => 0.1,     // Shiny neon
            BlockType::GoalZone => 0.2,       // Glossy
            BlockType::Bedrock => 0.95,       // Almost matte
            BlockType::GoldLoot => 0.3,       // Shiny gold
            BlockType::MetalBridge => 0.6,    // Worn metal
        }
    }
    
    fn is_solid(&self) -> bool {
        !matches!(self, BlockType::Air)
    }
}

// =============================================================================
// MESH GENERATION - The Critical Fix
// =============================================================================

/// Face directions for cube generation
const FACE_NORMALS: [[f32; 3]; 6] = [
    [1.0, 0.0, 0.0],   // +X (Right)
    [-1.0, 0.0, 0.0],  // -X (Left)
    [0.0, 1.0, 0.0],   // +Y (Top)
    [0.0, -1.0, 0.0],  // -Y (Bottom)
    [0.0, 0.0, 1.0],   // +Z (Front)
    [0.0, 0.0, -1.0],  // -Z (Back)
];

/// Result of mesh generation (VERTEX COLORING: color is baked into vertices)
struct MeshResult {
    mesh: Mesh,
}

/// Generates a Bevy Mesh from chunk data using simple culled meshing
fn generate_chunk_mesh(
    inner: &mut BackendInner,
    coord: ChunkCoord,
) -> Option<MeshResult> {
    // Check if chunk exists
    if inner.world_manager.get_chunk(coord).is_none() {
        warn!("Chunk [{},{}] not found in world manager!", coord.x, coord.z);
        return None;
    }
    
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    // VERTEX COLORING: Colors per vertex (RGBA)
    let mut colors: Vec<[f32; 4]> = Vec::new();
    
    // Color tracking
    let mut color_counts: HashMap<u8, u32> = HashMap::new();
    let mut solid_blocks_found = 0u32;
    let mut faces_added = 0u32;
    
    let chunk_world_x = coord.x * CHUNK_SIZE as i32;
    let chunk_world_z = coord.z * CHUNK_SIZE as i32;
    
    // Iterate through all voxels in chunk
    for local_y in 0..256 {
        for local_z in 0..CHUNK_SIZE as i32 {
            for local_x in 0..CHUNK_SIZE as i32 {
                let world_x = chunk_world_x + local_x;
                let world_z = chunk_world_z + local_z;
                
                let block = match inner.world_manager.get_block(world_x, local_y, world_z) {
                    Some(b) => BlockType::from_id(b.id),
                    None => BlockType::Air,
                };
                
                if !block.is_solid() {
                    continue;
                }
                
                solid_blocks_found += 1;
                *color_counts.entry(block as u8).or_insert(0) += 1;
                
                let pos = [world_x as f32, local_y as f32, world_z as f32];
                // VERTEX COLORING: Get color for this block
                let block_color = block.vertex_color();
                
                // Check each face
                for face in 0..6 {
                    let (nx, ny, nz) = get_neighbor_offset(face);
                    let neighbor_x = world_x + nx;
                    let neighbor_y = local_y + ny;
                    let neighbor_z = world_z + nz;
                    
                    let neighbor_solid = is_solid_at(&inner.world_manager, neighbor_x, neighbor_y, neighbor_z);
                    
                    if !neighbor_solid {
                        add_face(&mut positions, &mut normals, &mut uvs, &mut indices, &mut colors, pos, face, block_color);
                        faces_added += 1;
                    }
                }
            }
        }
    }
    
    // DEBUG: Log what we found
    if solid_blocks_found == 0 {
        info!("Chunk [{},{}]: No solid blocks found (empty chunk)", coord.x, coord.z);
        return None;
    }
    
    if positions.is_empty() {
        warn!("Chunk [{},{}]: {} solid blocks but 0 visible faces!", 
              coord.x, coord.z, solid_blocks_found);
        return None;
    }
    
    info!("Chunk [{},{}]: {} solid blocks, {} faces, {} vertices", 
          coord.x, coord.z, solid_blocks_found, faces_added, positions.len());
    
    // Build Bevy Mesh with VERTEX COLORS
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    // VERTEX COLORING: Inject colors directly into mesh vertices
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    
    Some(MeshResult { mesh })
}

/// Get neighbor offset for a face
fn get_neighbor_offset(face: usize) -> (i32, i32, i32) {
    match face {
        0 => (1, 0, 0),   // +X
        1 => (-1, 0, 0),  // -X
        2 => (0, 1, 0),   // +Y
        3 => (0, -1, 0),  // -Y
        4 => (0, 0, 1),   // +Z
        5 => (0, 0, -1),  // -Z
        _ => (0, 0, 0),
    }
}

/// Check if position is solid
fn is_solid_at(world: &WorldManager, x: i32, y: i32, z: i32) -> bool {
    if y < 0 || y >= 256 {
        return false;
    }
    match world.get_block(x, y, z) {
        Some(b) => BlockType::from_id(b.id).is_solid(),
        None => false, // Unloaded = draw face
    }
}

/// Add a face to the mesh with VERTEX COLORING
fn add_face(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    colors: &mut Vec<[f32; 4]>,
    pos: [f32; 3],
    face: usize,
    block_color: [f32; 4],
) {
    let base_index = positions.len() as u32;
    let normal = FACE_NORMALS[face];
    let verts = get_face_vertices(pos, face);
    
    for vert in &verts {
        positions.push(*vert);
        normals.push(normal);
    }
    
    uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    
    // VERTEX COLORING: Push color for each of the 4 vertices
    for _ in 0..4 {
        colors.push(block_color);
    }
    
    // Counter-clockwise winding (Bevy default)
    indices.extend_from_slice(&[
        base_index, base_index + 1, base_index + 2,
        base_index, base_index + 2, base_index + 3,
    ]);
}

/// Get the 4 vertices for a face (CCW winding when viewed from OUTSIDE the cube)
/// Bevy uses CCW = front-face, RIGHT_HANDED_Y_UP
/// Uses BLOCK_SCALE for smaller voxels
fn get_face_vertices(pos: [f32; 3], face: usize) -> [[f32; 3]; 4] {
    let [x, y, z] = pos;
    let s = BLOCK_SCALE; // Block size
    
    // Scale the position
    let sx = x * s;
    let sy = y * s;
    let sz = z * s;
    
    // Each face: 4 vertices in CCW order when viewed from the direction of the normal
    match face {
        // +X (Right): Normal = (1,0,0), view from +X looking at -X
        0 => [
            [sx + s, sy, sz + s],       // bottom-front
            [sx + s, sy, sz],           // bottom-back
            [sx + s, sy + s, sz],       // top-back
            [sx + s, sy + s, sz + s],   // top-front
        ],
        // -X (Left): Normal = (-1,0,0), view from -X looking at +X
        1 => [
            [sx, sy, sz],               // bottom-back
            [sx, sy, sz + s],           // bottom-front
            [sx, sy + s, sz + s],       // top-front
            [sx, sy + s, sz],           // top-back
        ],
        // +Y (Top): Normal = (0,1,0), view from +Y looking at -Y
        2 => [
            [sx, sy + s, sz + s],       // front-left
            [sx + s, sy + s, sz + s],   // front-right
            [sx + s, sy + s, sz],       // back-right
            [sx, sy + s, sz],           // back-left
        ],
        // -Y (Bottom): Normal = (0,-1,0), view from -Y looking at +Y
        3 => [
            [sx, sy, sz],               // back-left
            [sx + s, sy, sz],           // back-right
            [sx + s, sy, sz + s],       // front-right
            [sx, sy, sz + s],           // front-left
        ],
        // +Z (Front): Normal = (0,0,1), view from +Z looking at -Z
        4 => [
            [sx, sy, sz + s],           // bottom-left
            [sx + s, sy, sz + s],       // bottom-right
            [sx + s, sy + s, sz + s],   // top-right
            [sx, sy + s, sz + s],       // top-left
        ],
        // -Z (Back): Normal = (0,0,-1), view from -Z looking at +Z
        5 => [
            [sx + s, sy, sz],           // bottom-right
            [sx, sy, sz],               // bottom-left
            [sx, sy + s, sz],           // top-left
            [sx + s, sy + s, sz],       // top-right
        ],
        _ => [[0.0; 3]; 4],
    }
}

// =============================================================================
// BEVY SYSTEMS
// =============================================================================

/// FIXED ARENA: No infinite chunk loading
/// Arena is pre-loaded in setup(), this system is now disabled
#[allow(dead_code)]
fn update_chunk_streaming(
    _bridge: Res<BackendBridge>,
    _player_query: Query<&Transform, With<Player>>,
) {
    // DISABLED: Arena is fixed size, no streaming needed
    // All chunks are loaded once in setup()
}

/// System to sync dirty chunks to Bevy meshes
fn sync_chunks_to_bevy(
    mut commands: Commands,
    bridge: Res<BackendBridge>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rendered_chunks: ResMut<RenderedChunks>,
) {
    let mut inner = bridge.inner.lock().unwrap();
    
    // Get dirty chunks
    let dirty_coords: Vec<_> = inner.dirty_chunks.drain().collect();
    
    if dirty_coords.is_empty() {
        return;
    }
    
    info!("Processing {} dirty chunks", dirty_coords.len());
    
    let mut total_vertices = 0;
    let mut empty_chunks = 0;
    let mut valid_chunks = 0;
    
    for coord in dirty_coords {
        if let Some(result) = generate_chunk_mesh(&mut inner, coord) {
            // SANITY CHECK: Is the mesh actually populated?
            let vertex_count = result.mesh.count_vertices();
            
            if vertex_count == 0 {
                warn!("⚠️ Chunk [{},{}] generated EMPTY mesh! World data not loaded?", 
                      coord.x, coord.z);
                empty_chunks += 1;
                continue;
            }
            
            total_vertices += vertex_count;
            valid_chunks += 1;
            
            // Remove old entity if exists
            if let Some(entity) = rendered_chunks.chunks.remove(&coord) {
                commands.entity(entity).despawn();
            }
            
            // CRITICAL: Create physics collider from mesh BEFORE adding to assets
            // This allows the player to walk on the terrain
            let collider = Collider::trimesh_from_mesh(&result.mesh);
            
            // Create new entity with BRUTALIST PBR material (matte concrete)
            let mut entity_commands = commands.spawn((
                PbrBundle {
                    mesh: meshes.add(result.mesh),
                    material: materials.add(StandardMaterial {
                        // VERTEX COLORING: Must be WHITE to multiply with vertex colors
                        base_color: Color::WHITE,
                        // BRUTALIST PBR: Matte Concrete
                        metallic: 0.0,              // Concrete is not metallic
                        perceptual_roughness: 0.85, // Very matte - brutalist look
                        reflectance: 0.1,           // Minimal reflections
                        // Emissive driven by vertex colors >1.0 (neon hazards)
                        emissive: Color::BLACK,
                        // Back-face culling ON for performance
                        cull_mode: Some(bevy::render::render_resource::Face::Back),
                        double_sided: false,
                        ..default()
                    }),
                    transform: Transform::IDENTITY,
                    ..default()
                },
                ChunkMesh { coord },
            ));
            
            // Add physics collider if mesh conversion succeeded
            if let Some(col) = collider {
                entity_commands.insert((
                    RigidBody::Static,  // Terrain doesn't move
                    col,                // Terrain is solid
                ));
            } else {
                warn!("⚠️ Failed to create collider for chunk [{},{}]", coord.x, coord.z);
            }
            
            let entity = entity_commands.id();
            rendered_chunks.chunks.insert(coord, entity);
        } else {
            empty_chunks += 1;
        }
    }
    
    if valid_chunks > 0 || empty_chunks > 0 {
        info!("✅ Mesh stats: {} chunks with {} total vertices, {} empty chunks", 
              valid_chunks, total_vertices, empty_chunks);
    }
}

/// System to unload distant chunks
fn unload_distant_chunks(
    mut commands: Commands,
    bridge: Res<BackendBridge>,
    mut rendered_chunks: ResMut<RenderedChunks>,
) {
    let inner = bridge.inner.lock().unwrap();
    let loaded = &inner.loaded_chunks;
    
    let to_unload: Vec<ChunkCoord> = rendered_chunks.chunks
        .keys()
        .filter(|coord| !loaded.contains(coord))
        .copied()
        .collect();
    
    drop(inner); // Release lock before despawning
    
    for coord in to_unload {
        if let Some(entity) = rendered_chunks.chunks.remove(&coord) {
            commands.entity(entity).despawn();
        }
    }
}

/// Marker component for the player entity
#[derive(Component)]
struct Player;

/// Marker component for the player camera
#[derive(Component)]
struct PlayerCamera;

/// Camera mode - first or third person
#[derive(Component)]
struct CameraMode {
    /// True = third person, False = first person
    third_person: bool,
    /// Distance from player in third person
    distance: f32,
}

/// Setup system - runs once at startup
/// GLITCH WARS aesthetic: Void background, neon glow, physics-based movement
fn setup(
    mut commands: Commands,
    bridge: Res<BackendBridge>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    info!("===========================================");
    info!("THE MEGA-STRUCTURE - Survive the Arena");
    info!("===========================================");
    
    // Pre-load spawn area and mark all initial chunks as dirty
    let spawn_y = {
        let mut inner = bridge.inner.lock().unwrap();
        
        info!("Pre-loading spawn area...");
        inner.world_manager.ensure_loaded_around(0.0, 0.0, 3);
        inner.world_manager.flush_generation_queue();
        
        let player_chunk = WorldManager::world_to_chunk(0.0, 0.0);
        inner.last_player_chunk = Some(player_chunk);
        
        // CRITICAL: Mark ALL loaded chunks as dirty so they get meshed!
        for dz in -LOAD_RADIUS..=LOAD_RADIUS {
            for dx in -LOAD_RADIUS..=LOAD_RADIUS {
                let coord = ChunkCoord::new(player_chunk.x + dx, player_chunk.z + dz);
                if inner.world_manager.get_chunk(coord).is_some() {
                    inner.loaded_chunks.insert(coord);
                    inner.dirty_chunks.insert(coord);
                }
            }
        }
        
        let stats = inner.world_manager.stats();
        info!("Spawn loaded: {} chunks, {} marked dirty", 
              stats.loaded_chunks, inner.dirty_chunks.len());
        
        // Spawn above the floor (BASE_FLOOR_Y = 4 blocks)
        // Floor is at Y=4, player spawns just above
        (4.0 + 2.0) * BLOCK_SCALE // Floor + clearance
    };
    
    info!("Spawn position: (0.0, {}, 0.0)", spawn_y);
    
    // =========================================================================
    // PLAYER - Compact, Fast Character Controller
    // =========================================================================
    // Smaller player = feels faster, fits through tight spaces
    // At BLOCK_SCALE=0.25: 3 blocks = 0.75 world units
    let player_height = 3.0 * BLOCK_SCALE;  // 0.75 units (3 blocks tall)
    let player_radius = 0.8 * BLOCK_SCALE;  // 0.2 units (compact)
    
    let player_id = commands.spawn((
        Player,
        Name::new("Player"),
        // ECONOMY: Player's financial state
        PlayerEconomy {
            loot_count: 0,
            net_worth: 0.0,
            current_load: 0.0,
            stamina: 100.0,
        },
        // Physics components (Enterprise-grade) - MUST COLLIDE WITH TERRAIN
        RigidBody::Dynamic,
        Collider::capsule(player_height * 0.4, player_radius), // Tall capsule
        LockedAxes::ROTATION_LOCKED,         // Don't tip over!
        Friction::new(0.3),                  // Lower friction for speed
        Restitution::new(0.0),               // No bouncing
        LinearDamping(5.0),                  // Faster movement, less drag
        GravityScale(3.0),                   // Strong gravity for snappy jumps
        // ShapeCaster for ground detection
        ShapeCaster::new(
            Collider::sphere(player_radius * 0.8),
            Vec3::new(0.0, -player_height * 0.4, 0.0),
            Quat::IDENTITY,
            Direction3d::NEG_Y,
        ).with_max_time_of_impact(0.2), // Check just below feet
        // Visual representation - bright so player is visible
        PbrBundle {
            mesh: meshes.add(Capsule3d::new(player_radius, player_height)),
            material: materials.add(StandardMaterial {
                base_color: Color::rgb(1.0, 0.9, 0.2), // Bright Yellow/Gold
                emissive: Color::rgb(0.5, 0.4, 0.0),   // Warm glow
                metallic: 0.8,
                perceptual_roughness: 0.2,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, spawn_y, 0.0),
            ..default()
        },
    )).id();
    
    // CAMERA - First/Third Person, attached to Player as child
    // Press V to toggle between first and third person views
    let cam_distance = 20.0 * BLOCK_SCALE; // Further back to see more of the maze
    let cam_height = 12.0 * BLOCK_SCALE;   // Higher for better overview
    commands.spawn((
        PlayerCamera,
        CameraMode {
            third_person: true,  // Start in third person to see the player
            distance: cam_distance,
        },
        Camera3dBundle {
            camera: Camera {
                hdr: true, // Required for bloom
                ..default()
            },
            tonemapping: Tonemapping::TonyMcMapface,
            transform: Transform::from_xyz(0.0, cam_height, cam_distance)
                .looking_at(Vec3::new(0.0, player_height * 0.3, 0.0), Vec3::Y),
            ..default()
        },
        BloomSettings {
            intensity: 0.4,
            low_frequency_boost: 0.6,
            high_pass_frequency: 1.0,
            ..default()
        },
        // BRUTALIST FOG - Hides chunk loading, adds menace
        // Extends further for faster movement
        FogSettings {
            color: Color::rgb(0.05, 0.05, 0.07), // #0D0D12 - Darker void
            falloff: bevy::pbr::FogFalloff::Linear {
                start: 8.0,    // Clear near player
                end: 100.0,    // Extended for fast gameplay
            },
            ..default()
        },
    )).set_parent(player_id);
    
    info!("Player spawned with Physics Character Controller");
    
    // =========================================================================
    // THE EXTRACTION BEAM - Pure White/Blue Laser
    // The only safe zone in the arena - reach it to escape
    // =========================================================================
    let beam_radius = 3.0 * BLOCK_SCALE;
    let beam_height = 80.0 * BLOCK_SCALE; // Visible but not overwhelming
    commands.spawn((
        Name::new("ExtractionBeam"),
        PbrBundle {
            mesh: meshes.add(Cylinder::new(beam_radius, beam_height)),
            material: materials.add(StandardMaterial {
                base_color: Color::rgba(0.88, 1.0, 1.0, 0.12), // Ice white, subtle
                emissive: Color::rgb(3.0, 5.0, 6.0),           // Bright glow
                alpha_mode: bevy::pbr::AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, beam_height * 0.5 + 4.0 * BLOCK_SCALE, 0.0),
            ..default()
        },
    ));
    info!("Extraction Beam spawned - REACH IT TO ESCAPE!");
    
    // =========================================================================
    // BRUTALIST LIGHTING - Stadium Sun / Moonlight Effect
    // =========================================================================
    // One strong directional light casting long, sharp shadows
    // Like a giant artificial sun in a dead arena
    
    // THE STADIUM SUN - High angle, harsh shadows
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 50000.0,  // Intense artificial light
            shadows_enabled: true,
            shadow_depth_bias: 0.02,
            shadow_normal_bias: 0.6,
            color: Color::rgb(0.95, 0.95, 1.0), // Cold white
            ..default()
        },
        transform: Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -1.2,  // Steep angle for long shadows
            0.4,
            0.0,
        )),
        ..default()
    });
    
    // Player carried light - dim flashlight for nearby visibility
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 15000.0,  // Dim - just for immediate area
            color: Color::rgb(0.9, 0.85, 0.8), // Slightly warm
            range: 8.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 0.5, 0.0),
        ..default()
    }).set_parent(player_id);
    
    // DARK ambient - shadows should be DARK
    commands.insert_resource(AmbientLight {
        color: Color::rgb(0.15, 0.15, 0.2), // Cold blue-grey
        brightness: 50.0,                    // Very low - makes shadows scary
    });
    
    info!("THE MEGA-STRUCTURE initialized. WASD to move, SPACE to jump.");
    info!("Find the WHITE BEAM at origin to ESCAPE!");
    info!("===========================================");
}

/// Find ground height at position (for UNDERCITY terrain)
#[allow(dead_code)]
fn find_ground_height(world: &WorldManager, x: i32, z: i32) -> i32 {
    // Search from top down to find first solid block
    for y in (0..128).rev() {
        if let Some(block) = world.get_block(x, y, z) {
            if BlockType::from_id(block.id).is_solid() {
                return y + 1;
            }
        }
    }
    // Default to surface level if nothing found (should be ~48)
    50
}

// =============================================================================
// MINING SYSTEM - Block Breaking & Placing
// =============================================================================

/// Maximum reach distance for mining (in world units, scaled)
const MINING_REACH: f32 = 5.0 * BLOCK_SCALE;

/// System to handle block mining (breaking/placing)
fn handle_mining(
    mouse_button: Res<ButtonInput<MouseButton>>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    player_query: Query<&Transform, With<Player>>,
    bridge: Res<BackendBridge>,
    mut gizmos: Gizmos,
) {
    let Ok(camera_global) = camera_query.get_single() else {
        return;
    };
    let Ok(_player_transform) = player_query.get_single() else {
        return;
    };
    
    // Get camera world position and forward direction
    let ray_origin = camera_global.translation();
    let ray_direction = camera_global.forward(); // GlobalTransform::forward() returns Vec3
    
    // Simple voxel raycast (DDA algorithm) - scale origin to block coordinates
    let scaled_origin = ray_origin / BLOCK_SCALE;
    let scaled_reach = MINING_REACH / BLOCK_SCALE;
    
    if let Some((hit_pos, hit_normal)) = voxel_raycast(&bridge, scaled_origin, ray_direction, scaled_reach) {
        // Draw crosshair at hit point (scaled back to world coordinates)
        let hit_world = Vec3::new(
            (hit_pos.0 as f32 + 0.5) * BLOCK_SCALE, 
            (hit_pos.1 as f32 + 0.5) * BLOCK_SCALE, 
            (hit_pos.2 as f32 + 0.5) * BLOCK_SCALE
        );
        gizmos.cuboid(
            Transform::from_translation(hit_world).with_scale(Vec3::splat(BLOCK_SCALE * 1.02)),
            Color::rgba(1.0, 1.0, 0.0, 0.5),
        );
        
        // Left Click: Break block (set to Air - ID 0)
        if mouse_button.just_pressed(MouseButton::Left) {
            let mut inner = bridge.inner.lock().unwrap();
            if inner.world_manager.set_block(hit_pos.0, hit_pos.1 as i32, hit_pos.2, 0) {
                info!("Block broken at ({}, {}, {})", hit_pos.0, hit_pos.1, hit_pos.2);
                // Mark chunk as dirty for re-meshing
                let chunk_coord = ChunkCoord::new(
                    hit_pos.0.div_euclid(CHUNK_SIZE as i32),
                    hit_pos.2.div_euclid(CHUNK_SIZE as i32),
                );
                inner.dirty_chunks.insert(chunk_coord);
            }
        }
        
        // Right Click: Place block (Gold - ID 3)
        if mouse_button.just_pressed(MouseButton::Right) {
            // Place at adjacent position (using normal)
            let place_pos = (
                hit_pos.0 + hit_normal.0,
                (hit_pos.1 as i32 + hit_normal.1) as usize,
                hit_pos.2 + hit_normal.2,
            );
            
            let mut inner = bridge.inner.lock().unwrap();
            if inner.world_manager.set_block(place_pos.0, place_pos.1 as i32, place_pos.2, 3) {
                info!("Block placed at ({}, {}, {})", place_pos.0, place_pos.1, place_pos.2);
                // Mark chunk as dirty for re-meshing
                let chunk_coord = ChunkCoord::new(
                    place_pos.0.div_euclid(CHUNK_SIZE as i32),
                    place_pos.2.div_euclid(CHUNK_SIZE as i32),
                );
                inner.dirty_chunks.insert(chunk_coord);
            }
        }
    }
}

/// Simple voxel raycast using DDA algorithm
/// Returns (hit_position, hit_normal) or None
fn voxel_raycast(
    bridge: &BackendBridge,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<((i32, usize, i32), (i32, i32, i32))> {
    let inner = bridge.inner.lock().unwrap();
    
    // Current voxel position
    let mut x = origin.x.floor() as i32;
    let mut y = origin.y.floor() as i32;
    let mut z = origin.z.floor() as i32;
    
    // Direction signs
    let step_x = if direction.x > 0.0 { 1 } else { -1 };
    let step_y = if direction.y > 0.0 { 1 } else { -1 };
    let step_z = if direction.z > 0.0 { 1 } else { -1 };
    
    // Delta distances
    let delta_x = if direction.x.abs() < 0.0001 { f32::MAX } else { (1.0 / direction.x).abs() };
    let delta_y = if direction.y.abs() < 0.0001 { f32::MAX } else { (1.0 / direction.y).abs() };
    let delta_z = if direction.z.abs() < 0.0001 { f32::MAX } else { (1.0 / direction.z).abs() };
    
    // Initial t values
    let mut t_max_x = if direction.x > 0.0 {
        ((x + 1) as f32 - origin.x) * delta_x
    } else {
        (origin.x - x as f32) * delta_x
    };
    let mut t_max_y = if direction.y > 0.0 {
        ((y + 1) as f32 - origin.y) * delta_y
    } else {
        (origin.y - y as f32) * delta_y
    };
    let mut t_max_z = if direction.z > 0.0 {
        ((z + 1) as f32 - origin.z) * delta_z
    } else {
        (origin.z - z as f32) * delta_z
    };
    
    let mut distance = 0.0;
    let mut last_normal = (0, 0, 0);
    
    while distance < max_distance {
        // Check current voxel
        if y >= 0 && y < 256 {
            if let Some(block) = inner.world_manager.get_block(x, y, z) {
                if BlockType::from_id(block.id).is_solid() {
                    return Some(((x, y as usize, z), last_normal));
                }
            }
        }
        
        // Step to next voxel
        if t_max_x < t_max_y {
            if t_max_x < t_max_z {
                x += step_x;
                distance = t_max_x;
                t_max_x += delta_x;
                last_normal = (-step_x, 0, 0);
            } else {
                z += step_z;
                distance = t_max_z;
                t_max_z += delta_z;
                last_normal = (0, 0, -step_z);
            }
        } else {
            if t_max_y < t_max_z {
                y += step_y;
                distance = t_max_y;
                t_max_y += delta_y;
                last_normal = (0, -step_y, 0);
            } else {
                z += step_z;
                distance = t_max_z;
                t_max_z += delta_z;
                last_normal = (0, 0, -step_z);
            }
        }
    }
    
    None
}

// =============================================================================
// PHYSICS MOVEMENT CONTROLLER - WASD + Jump
// =============================================================================

/// Movement speed - FAST arcade movement (12 blocks/sec)
const PLAYER_SPEED: f32 = 12.0 * BLOCK_SCALE; // 3.0 units/sec - 3x faster
/// Jump impulse - high jumps for parkour (can clear ~4 blocks)
const JUMP_IMPULSE: f32 = 6.0; // 2x higher jumps

/// Physics-based movement controller
/// Uses WASD for horizontal movement, SPACE for jump
fn movement_controller(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&Transform, &mut LinearVelocity), With<Player>>,
    camera_query: Query<&Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    let Ok((player_transform, mut velocity)) = player_query.get_single_mut() else {
        return;
    };
    
    // Get camera direction for movement relative to view
    let camera_forward = if let Ok(cam_transform) = camera_query.get_single() {
        // Use parent (player) transform combined with camera local transform
        let world_cam = player_transform.rotation * cam_transform.rotation;
        let forward = world_cam * Vec3::NEG_Z;
        Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero()
    } else {
        let fwd = player_transform.forward();
        Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero()
    };
    
    let camera_right = Vec3::new(camera_forward.z, 0.0, -camera_forward.x);
    
    // Calculate movement direction from input
    let mut move_dir = Vec3::ZERO;
    
    if keyboard.pressed(KeyCode::KeyW) {
        move_dir += camera_forward;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        move_dir -= camera_forward;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        move_dir -= camera_right;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        move_dir += camera_right;
    }
    
    // Normalize and apply horizontal movement (preserve vertical velocity)
    if move_dir.length_squared() > 0.0 {
        move_dir = move_dir.normalize();
        velocity.x = move_dir.x * PLAYER_SPEED;
        velocity.z = move_dir.z * PLAYER_SPEED;
    }
    
    // Ground check based on velocity (if not moving down fast, probably grounded)
    // This is a simple approximation; ShapeCaster provides better detection
    let is_grounded = velocity.y.abs() < 0.5;
    
    // Jump (only when grounded)
    if keyboard.just_pressed(KeyCode::Space) && is_grounded {
        velocity.y = JUMP_IMPULSE;
    }
}

// =============================================================================
// MOUSE LOOK - Rotate player/camera based on mouse movement
// =============================================================================

/// Mouse sensitivity for looking around
const MOUSE_SENSITIVITY: f32 = 0.003;

/// System to toggle camera mode (V key)
fn toggle_camera_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut camera_query: Query<(&mut CameraMode, &mut Transform), With<PlayerCamera>>,
) {
    if keyboard.just_pressed(KeyCode::KeyV) {
        if let Ok((mut mode, mut transform)) = camera_query.get_single_mut() {
            mode.third_person = !mode.third_person;
            
            let player_height = 7.0 * BLOCK_SCALE;
            
            if mode.third_person {
                // Third person: behind and above
                let cam_height = 8.0 * BLOCK_SCALE;
                transform.translation = Vec3::new(0.0, cam_height, mode.distance);
                transform.look_at(Vec3::new(0.0, player_height * 0.3, 0.0), Vec3::Y);
                info!("Camera: Third Person (press V to change)");
            } else {
                // First person: at eye level (top of player)
                transform.translation = Vec3::new(0.0, player_height * 0.4, 0.0);
                transform.rotation = Quat::IDENTITY;
                info!("Camera: First Person (press V to change)");
            }
        }
    }
}

/// System to handle mouse look (rotate player body for yaw, camera for pitch)
fn mouse_look(
    mut mouse_motion: EventReader<bevy::input::mouse::MouseMotion>,
    mut player_query: Query<&mut Transform, With<Player>>,
    mut camera_query: Query<(&mut Transform, &CameraMode), (With<PlayerCamera>, Without<Player>)>,
) {
    let mut delta = Vec2::ZERO;
    for event in mouse_motion.read() {
        delta += event.delta;
    }
    
    if delta == Vec2::ZERO {
        return;
    }
    
    // Rotate player body (yaw - left/right)
    if let Ok(mut player_transform) = player_query.get_single_mut() {
        player_transform.rotate_y(-delta.x * MOUSE_SENSITIVITY);
    }
    
    let player_height = 7.0 * BLOCK_SCALE;
    
    // Handle camera based on mode
    if let Ok((mut camera_transform, mode)) = camera_query.get_single_mut() {
        if mode.third_person {
            // Third person: orbit camera around player
            let current_pitch = camera_transform.rotation.to_euler(EulerRot::YXZ).1;
            let new_pitch = (current_pitch - delta.y * MOUSE_SENSITIVITY).clamp(-0.8, 1.2);
            
            // Update camera position based on pitch
            let distance = mode.distance;
            let base_height = 6.0 * BLOCK_SCALE;
            let height = base_height + distance * new_pitch.sin().abs() * 0.5;
            let back = distance * new_pitch.cos().max(0.3);
            
            camera_transform.translation = Vec3::new(0.0, height, back);
            camera_transform.look_at(Vec3::new(0.0, player_height * 0.3, 0.0), Vec3::Y);
        } else {
            // First person: rotate camera pitch (up/down)
            let pitch = (camera_transform.rotation.to_euler(EulerRot::YXZ).1 - delta.y * MOUSE_SENSITIVITY)
                .clamp(-1.5, 1.5); // ~85 degrees up/down
            camera_transform.rotation = Quat::from_rotation_x(pitch);
        }
    }
}

// =============================================================================
// VOID FALL RESPAWN - Teleport player if they fall off the map
// =============================================================================

/// If player falls into the RED HAZARD ZONE, respawn them
fn check_void_fall(
    mut player_query: Query<(&mut Transform, &mut LinearVelocity), With<Player>>,
) {
    let Ok((mut transform, mut velocity)) = player_query.get_single_mut() else {
        return;
    };
    
    // Hazard zone is Y=2, floor is Y=4
    // Respawn if below Y=3 (falling into pit)
    if transform.translation.y < 3.0 * BLOCK_SCALE {
        // Respawn above floor at origin (BASE_FLOOR_Y=4 + clearance)
        transform.translation = Vec3::new(0.0, 6.0 * BLOCK_SCALE, 0.0);
        // Reset velocity
        velocity.0 = Vec3::ZERO;
        info!("⚠️ DEATH - Fell into the Red Zone! Respawning...");
    }
}

// =============================================================================
// WASM POINTER LOCK - Click to grab mouse
// =============================================================================

/// System to grab mouse on click (WASM requires user gesture for pointer lock)
#[cfg(target_arch = "wasm32")]
fn grab_mouse_on_click(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window>,
) {
    use bevy::window::CursorGrabMode;
    
    if mouse_button.just_pressed(MouseButton::Left) {
        if let Ok(mut window) = windows.get_single_mut() {
            // Only grab if not already grabbed
            if window.cursor.grab_mode == CursorGrabMode::None {
                window.cursor.grab_mode = CursorGrabMode::Locked;
                window.cursor.visible = false;
                info!("Mouse grabbed - pointer lock active");
            }
        }
    }
}

/// System to release mouse on Escape (WASM)
#[cfg(target_arch = "wasm32")]
fn release_mouse_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window>,
) {
    use bevy::window::CursorGrabMode;
    
    if keyboard.just_pressed(KeyCode::Escape) {
        if let Ok(mut window) = windows.get_single_mut() {
            window.cursor.grab_mode = CursorGrabMode::None;
            window.cursor.visible = true;
            info!("Mouse released - pointer lock disabled");
        }
    }
}

// =============================================================================
// MAIN - The Entry Point
// =============================================================================

fn main() {
    // WASM: Install panic hook for better error messages in browser console
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
    
    let mut app = App::new();
    
    // Configure plugins differently for WASM vs Native
    #[cfg(target_arch = "wasm32")]
    {
        // WASM: Single-threaded + Canvas binding + Cursor unlocked initially
        use bevy::core::TaskPoolPlugin;
        use bevy::window::CursorGrabMode;
        use bevy::window::WindowMode;
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "GLITCH WARS - The Simulation".into(),
                        // WINDOWED: Use CSS to stretch canvas to viewport
                        // NOTE: BorderlessFullscreen requires user gesture in browsers
                        mode: WindowMode::Windowed,
                        // CRITICAL: Bind to canvas element in index.html
                        canvas: Some("#bevy".into()),
                        prevent_default_event_handling: true,
                        // Large resolution - CSS will scale down
                        resolution: (1920., 1080.).into(),
                        // WASM: Start with cursor UNLOCKED (user must click first)
                        cursor: bevy::window::Cursor {
                            visible: true,
                            grab_mode: CursorGrabMode::None,
                            ..default()
                        },
                        ..default()
                    }),
                    ..default()
                })
                .set(TaskPoolPlugin {
                    task_pool_options: bevy::core::TaskPoolOptions::with_num_threads(1),
                })
        );
    }
    
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Native: Full multi-threading
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "GLITCH WARS - The Simulation".into(),
                resolution: (1280., 720.).into(),
                ..default()
            }),
            ..default()
        }));
    }
    
    // ENTERPRISE PHYSICS - bevy_xpbd_3d
    // Note: parallel disabled for WASM compatibility (handled by TaskPoolOptions above)
    app.add_plugins(PhysicsPlugins::default());
    
    // NOTE: PhysicsDebugPlugin removed - debug wireframes disabled
    
    // Configure physics timestep for smooth gameplay
    app.insert_resource(bevy_xpbd_3d::prelude::SubstepCount(4));
    
    app
        // THE VOID - Pure black abyss (matches fog end color)
        .insert_resource(ClearColor(Color::rgb(0.04, 0.04, 0.05)))
        
        // Configure gravity (slightly stronger for snappy movement)
        .insert_resource(Gravity(Vec3::new(0.0, -20.0, 0.0)))
        
        // NOTE: AtmospherePlugin disabled for WASM compatibility
        // NOTE: bevy_flycam REMOVED - was causing noclip/flying
        // All camera movement is now physics-based
        
        // Our resources
        .insert_resource(BackendBridge::new(WORLD_SEED))
        .insert_resource(RenderedChunks::default())
        
        // Our systems
        .add_systems(Startup, setup)
        .add_systems(Update, update_chunk_streaming)
        .add_systems(Update, sync_chunks_to_bevy.after(update_chunk_streaming))
        .add_systems(Update, unload_distant_chunks.after(sync_chunks_to_bevy))
        // Physics-based movement (WASD + Jump)
        .add_systems(Update, movement_controller)
        // Camera mode toggle (V key)
        .add_systems(Update, toggle_camera_mode)
        // Mouse look (rotate player/camera)
        .add_systems(Update, mouse_look)
        // Void fall respawn
        .add_systems(Update, check_void_fall)
        // Mining system (block breaking/placing)
        .add_systems(Update, handle_mining);
    
    // WASM: Add pointer lock systems (click to grab, escape to release)
    #[cfg(target_arch = "wasm32")]
    {
        app.add_systems(Update, grab_mouse_on_click);
        app.add_systems(Update, release_mouse_on_escape);
    }
    
    // MULTIPLAYER: Add networking resource and systems
    app.insert_resource(networking::NetworkState::default());
    // Initialize networking on startup (for WASM, this connects to the server)
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Startup, init_networking);
    app.add_systems(Update, network_tick);
    app.add_systems(Update, process_network_messages);
    
    #[cfg(target_arch = "wasm32")]
    {
        app.add_systems(Update, update_remote_players_system);
    }
        
    app.run();
}

// =============================================================================
// NETWORKING SYSTEMS
// =============================================================================

/// Initialize networking on startup (currently using lazy init instead)
#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
fn init_networking(mut network: ResMut<networking::NetworkState>) {
    web_sys::console::log_1(&"[GAME] Initializing multiplayer networking...".into());
    info!("Initializing multiplayer networking...");
    network.connect();
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn init_networking(_network: ResMut<networking::NetworkState>) {
    info!("Networking disabled on native build");
}

/// Network tick - send position to authoritative server
#[cfg(target_arch = "wasm32")]
fn network_tick(
    mut network: ResMut<networking::NetworkState>,
    player_query: Query<&Transform, With<Player>>,
    time: Res<Time>,
    mut last_send: Local<f32>,
    mut last_ping: Local<f32>,
    mut initialized: Local<bool>,
    mut frame_count: Local<u32>,
) {
    // Debug: Log every 60 frames
    *frame_count += 1;
    if *frame_count == 1 || *frame_count % 300 == 0 {
        web_sys::console::log_1(&format!("[NET-TICK] Frame {}, initialized={}", *frame_count, *initialized).into());
    }
    
    // Lazy initialization - connect on first tick
    if !*initialized {
        *initialized = true;
        web_sys::console::log_1(&"[NET] *** INITIALIZING MULTIPLAYER CONNECTION ***".into());
        network.connect();
    }
    
    let dt = time.delta_seconds();
    
    // Update interpolation for smooth remote player movement
    network.update_interpolation(dt);
    
    // Send INPUT at 20 updates per second
    *last_send += dt;
    if *last_send >= 0.05 { // 20 Hz
        *last_send = 0.0;
        
        if let Ok(transform) = player_query.get_single() {
            // Convert from world coordinates back to block coordinates
            let pos = transform.translation / BLOCK_SCALE;
            let (_, yaw, _) = transform.rotation.to_euler(EulerRot::YXZ);
            network.send_input(pos.x, pos.y, pos.z, yaw);
        }
    }
    
    // Send PING every 2 seconds for latency measurement
    *last_ping += dt;
    if *last_ping >= 2.0 {
        *last_ping = 0.0;
        network.send_ping();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn network_tick() {}

/// Process network messages (needs mutable access)
#[cfg(target_arch = "wasm32")]
fn process_network_messages(mut network: ResMut<networking::NetworkState>) {
    network.process_messages();
}

#[cfg(not(target_arch = "wasm32"))]
fn process_network_messages(_network: ResMut<networking::NetworkState>) {}
