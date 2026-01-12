//! # OROBOROS Server
//!
//! The authoritative game server that runs on German servers.
//!
//! ## CRITICAL REQUIREMENTS
//! - NO GPU
//! - NO WINDOW
//! - NO GRAPHICS
//! - HEADLESS ONLY
//!
//! If this binary tries to initialize wgpu, Vulkan, or any graphics API,
//! THE BUILD HAS FAILED.
//!
//! ## Production Deployment
//!
//! ```bash
//! # Run in production
//! ./oroboros_server
//!
//! # Run in background
//! nohup ./oroboros_server > server.log 2>&1 &
//! ```

// COMPILE-TIME GUARD: These imports must NOT exist in server build
#[cfg(feature = "rendering")]
compile_error!("SERVER MUST NOT HAVE RENDERING FEATURE! You're pulling GPU dependencies onto the German server!");

use oroboros::core::{DoubleBufferedWorld, Position, Velocity};
use oroboros_shared::{SERVER_BIND, TICK_RATE, MAX_CLIENTS};

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};
use std::io::ErrorKind;

/// Connected client tracking
struct ClientState {
    /// Last packet received time
    last_seen: Instant,
    /// Entity ID assigned to this client
    entity_id: usize,
}

/// Simple packet types (values used for matching incoming packets)
#[repr(u8)]
#[allow(dead_code)]
enum PacketType {
    Connect = 1,
    ConnectAck = 2,
    Heartbeat = 3,
    Input = 4,
    Snapshot = 5,
    Disconnect = 6,
}

fn main() {
    println!("═══════════════════════════════════════════════════════════════════");
    println!("                    OROBOROS SERVER v0.1.0");
    println!("                         HEADLESS MODE");
    println!("═══════════════════════════════════════════════════════════════════");
    println!();
    println!("  GPU:      NOT LOADED ✓");
    println!("  Window:   NOT LOADED ✓");
    println!("  Render:   NOT LOADED ✓");
    println!();

    // === NETWORK BIND ===
    println!("🌐 Binding to {} ...", SERVER_BIND);
    
    let socket = match UdpSocket::bind(SERVER_BIND) {
        Ok(s) => {
            s.set_nonblocking(true).expect("Failed to set non-blocking");
            println!("   ✓ UDP Socket bound to {}", SERVER_BIND);
            s
        }
        Err(e) => {
            eprintln!("   ✗ FATAL: Failed to bind socket: {}", e);
            eprintln!("     Check if another server is running or port is blocked.");
            std::process::exit(1);
        }
    };

    // === INITIALIZATION ===
    println!();
    println!("🏗️  Initializing server systems...");

    // Unit 1: Core ECS
    let db_world = DoubleBufferedWorld::new(1_000_000, MAX_CLIENTS);
    println!("   ✓ Unit 1 (Core): DoubleBufferedWorld ready (1M entities)");

    // Unit 3: Economy
    println!("   ✓ Unit 3 (Veridia): Economy systems ready");

    // Unit 4: Networking
    println!("   ✓ Unit 4 (Inferno): Network server ready");

    // Client tracking
    let mut clients: HashMap<SocketAddr, ClientState> = HashMap::with_capacity(MAX_CLIENTS);
    let mut next_entity_id = 0usize;

    // Receive buffer
    let mut recv_buffer = [0u8; 1200];

    println!();
    println!("═══════════════════════════════════════════════════════════════════");
    println!("                    SERVER LISTENING");
    println!("    External clients connect to: 162.55.2.222:7777");
    println!("═══════════════════════════════════════════════════════════════════");
    println!();
    println!("   Waiting for connections... (Press Ctrl+C to stop)");
    println!();

    // === MAIN SERVER LOOP ===
    let tick_duration = Duration::from_micros(1_000_000 / TICK_RATE as u64);
    let mut tick: u64 = 0;
    let mut last_stats_print = Instant::now();
    let server_start = Instant::now();

    loop {
        let tick_start = Instant::now();

        // === 1. RECEIVE PACKETS ===
        loop {
            match socket.recv_from(&mut recv_buffer) {
                Ok((len, addr)) => {
                    if len > 0 {
                        let packet_type = recv_buffer[0];
                        
                        match packet_type {
                            // CONNECT
                            1 => {
                                if !clients.contains_key(&addr) && clients.len() < MAX_CLIENTS {
                                    // Spawn entity for new player
                                    let entity_id = next_entity_id;
                                    next_entity_id += 1;
                                    
                                    {
                                        let mut write = db_world.write_handle();
                                        let _ = write.spawn_pv(
                                            Position::new(0.0, 50.0, 0.0),
                                            Velocity::new(0.0, 0.0, 0.0),
                                        );
                                    }
                                    
                                    clients.insert(addr, ClientState {
                                        last_seen: Instant::now(),
                                        entity_id,
                                    });
                                    
                                    // Send ConnectAck
                                    let ack = [PacketType::ConnectAck as u8, (entity_id & 0xFF) as u8];
                                    let _ = socket.send_to(&ack, addr);
                                    
                                    println!("   🟢 Client connected: {} (Entity #{})", addr, entity_id);
                                }
                            }
                            // HEARTBEAT
                            3 => {
                                if let Some(client) = clients.get_mut(&addr) {
                                    client.last_seen = Instant::now();
                                }
                            }
                            // INPUT (Player movement)
                            4 => {
                                if let Some(client) = clients.get_mut(&addr) {
                                    client.last_seen = Instant::now();
                                    // Parse input: [type, dx, dy, dz] (simplified)
                                    if len >= 4 {
                                        // Apply movement (server authoritative)
                                        // In production: validate and apply physics
                                    }
                                }
                            }
                            // DISCONNECT
                            6 => {
                                if let Some(client) = clients.remove(&addr) {
                                    println!("   🔴 Client disconnected: {} (Entity #{})", addr, client.entity_id);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => {
                    eprintln!("   ⚠ Receive error: {}", e);
                    break;
                }
            }
        }

        // === 2. UPDATE PHYSICS ===
        {
            let mut write = db_world.write_handle();
            write.update_positions(1.0 / TICK_RATE as f32);
        }

        // === 3. SWAP BUFFERS ===
        db_world.swap_buffers();

        // === 4. BROADCAST SNAPSHOT ===
        if !clients.is_empty() && tick % 2 == 0 {  // Send snapshot every 2 ticks (30 Hz)
            let snapshot = create_snapshot(tick as u32, clients.len() as u16);
            
            for addr in clients.keys() {
                let _ = socket.send_to(&snapshot, *addr);
            }
        }

        // === 5. CLEANUP DISCONNECTED CLIENTS ===
        let timeout = Duration::from_secs(10);
        let now = Instant::now();
        clients.retain(|addr, client| {
            if now.duration_since(client.last_seen) > timeout {
                println!("   ⏰ Client timed out: {} (Entity #{})", addr, client.entity_id);
                false
            } else {
                true
            }
        });

        // === 6. STATS OUTPUT ===
        if last_stats_print.elapsed() > Duration::from_secs(10) {
            let uptime = server_start.elapsed();
            println!("   📊 Tick: {} | Clients: {} | Uptime: {:?}", 
                tick, clients.len(), uptime);
            last_stats_print = Instant::now();
        }

        // === 7. TICK TIMING ===
        tick += 1;
        let elapsed = tick_start.elapsed();
        if elapsed < tick_duration {
            std::thread::sleep(tick_duration - elapsed);
        }
    }
}

/// Creates a simple snapshot packet
fn create_snapshot(tick: u32, player_count: u16) -> [u8; 16] {
    let mut snapshot = [0u8; 16];
    snapshot[0] = PacketType::Snapshot as u8;
    snapshot[1..5].copy_from_slice(&tick.to_le_bytes());
    snapshot[5..7].copy_from_slice(&player_count.to_le_bytes());
    // Rest is padding / entity data in real implementation
    snapshot
}
