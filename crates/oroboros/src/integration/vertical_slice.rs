//! # Vertical Slice Test
//!
//! Full integration test: Attack → UDP → Physics → Economy → Response
//!
//! THE ARCHITECT DEMANDS: < 50ms RTT on local network.

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use oroboros_core::Position;

use super::combat::{AttackCommand, AttackResult, CombatProcessor, ServerEntity};

/// Packet types for vertical slice.
#[repr(u8)]
enum PacketType {
    /// Attack command.
    Attack = 1,
    /// Attack result.
    AttackResult = 2,
}

/// Vertical slice configuration.
#[derive(Clone, Debug)]
pub struct VerticalSliceConfig {
    /// Server address.
    pub server_addr: SocketAddr,
    /// Client address.
    pub client_addr: SocketAddr,
    /// Number of entities on server.
    pub entity_count: usize,
    /// Maximum acceptable RTT in milliseconds.
    pub max_rtt_ms: u64,
}

impl Default for VerticalSliceConfig {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:8888".parse().unwrap(),
            client_addr: "127.0.0.1:8889".parse().unwrap(),
            entity_count: 500,
            max_rtt_ms: 50,
        }
    }
}

/// Metrics from vertical slice test.
#[derive(Clone, Debug, Default)]
pub struct SliceMetrics {
    /// Total attacks sent.
    pub attacks_sent: u64,
    /// Total responses received.
    pub responses_received: u64,
    /// Total hits.
    pub hits: u64,
    /// Total misses.
    pub misses: u64,
    /// Average RTT in microseconds.
    pub avg_rtt_us: u64,
    /// Minimum RTT in microseconds.
    pub min_rtt_us: u64,
    /// Maximum RTT in microseconds.
    pub max_rtt_us: u64,
    /// Average server processing time in microseconds.
    pub avg_server_processing_us: u64,
    /// RTT requirement met?
    pub rtt_requirement_met: bool,
}

/// Server for vertical slice.
pub struct VerticalSliceServer {
    socket: UdpSocket,
    processor: CombatProcessor,
    running: Arc<AtomicBool>,
    packets_processed: Arc<AtomicU64>,
}

impl VerticalSliceServer {
    /// Creates a new server.
    ///
    /// # Errors
    ///
    /// Returns an error if the UDP socket cannot be bound.
    pub fn new(config: &VerticalSliceConfig) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(config.server_addr)?;
        socket.set_nonblocking(true)?;

        let mut processor = CombatProcessor::new();

        // Add entities
        for i in 0..config.entity_count as u32 {
            let x = (i % 100) as f32 - 50.0;
            let z = (i / 100) as f32 - 50.0;
            processor.add_entity(ServerEntity::new(
                i,
                Position::new(x, 0.0, z),
                100,
            ));
        }

        Ok(Self {
            socket,
            processor,
            running: Arc::new(AtomicBool::new(false)),
            packets_processed: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Runs the server (blocking).
    pub fn run(&mut self) {
        self.running.store(true, Ordering::Release);
        let mut buf = [0u8; 1024];

        while self.running.load(Ordering::Acquire) {
            match self.socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    if len < 1 {
                        continue;
                    }

                    let packet_type = buf[0];
                    
                    if packet_type == PacketType::Attack as u8 {
                        if let Some(command) = AttackCommand::from_bytes(&buf[1..len]) {
                            // Process attack
                            let result = self.processor.process_attack(&command);
                            
                            // Send response
                            let mut response = vec![PacketType::AttackResult as u8];
                            response.extend_from_slice(&result.to_bytes());
                            
                            let _ = self.socket.send_to(&response, src);
                            self.packets_processed.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data, spin briefly
                    std::hint::spin_loop();
                }
                Err(_e) => {
                    // Ignore errors
                }
            }
        }
    }

    /// Stops the server.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }

    /// Gets packets processed count.
    #[must_use]
    pub fn packets_processed(&self) -> u64 {
        self.packets_processed.load(Ordering::Relaxed)
    }

    /// Gets running flag for thread.
    #[must_use]
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }
}

/// Client for vertical slice.
pub struct VerticalSliceClient {
    socket: UdpSocket,
    server_addr: SocketAddr,
    sequence: u32,
}

impl VerticalSliceClient {
    /// Creates a new client.
    ///
    /// # Errors
    ///
    /// Returns an error if the UDP socket cannot be bound.
    pub fn new(config: &VerticalSliceConfig) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(config.client_addr)?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;

        Ok(Self {
            socket,
            server_addr: config.server_addr,
            sequence: 0,
        })
    }

    /// Sends an attack and waits for response.
    /// Returns (RTT in microseconds, `AttackResult`).
    pub fn attack(&mut self, origin: Position, direction: (f32, f32, f32)) -> Option<(u64, AttackResult)> {
        self.sequence += 1;
        
        let command = AttackCommand::new(self.sequence, 0, origin, direction);
        
        // Build packet
        let mut packet = vec![PacketType::Attack as u8];
        packet.extend_from_slice(&command.to_bytes());

        // Send and measure RTT
        let start = Instant::now();
        
        if self.socket.send_to(&packet, self.server_addr).is_err() {
            return None;
        }

        // Wait for response
        let mut buf = [0u8; 1024];
        match self.socket.recv_from(&mut buf) {
            Ok((len, _src)) => {
                let rtt = start.elapsed().as_micros() as u64;
                
                if len > 1 && buf[0] == PacketType::AttackResult as u8 {
                    if let Some(result) = AttackResult::from_bytes(&buf[1..len]) {
                        return Some((rtt, result));
                    }
                }
                None
            }
            Err(_) => None,
        }
    }
}

/// Runs the vertical slice test.
#[must_use]
pub fn run_vertical_slice_test(config: VerticalSliceConfig, num_attacks: u32) -> SliceMetrics {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║         VERTICAL SLICE INTEGRATION TEST                          ║");
    println!("║         Attack → UDP → Physics → Economy → Response              ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Target: < {}ms RTT                                              ║", config.max_rtt_ms);
    println!("║  Entities: {}                                                  ║", config.entity_count);
    println!("║  Attacks: {}                                                  ║", num_attacks);
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    // Start server in thread
    let mut server = VerticalSliceServer::new(&config).expect("Failed to create server");
    let running = server.running_flag();
    
    let server_handle = thread::spawn(move || {
        server.run();
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(50));

    // Create client
    let mut client = VerticalSliceClient::new(&config).expect("Failed to create client");

    let mut metrics = SliceMetrics::default();
    metrics.min_rtt_us = u64::MAX;

    let mut rng_state: u64 = 42;
    let mut rand = || {
        rng_state = rng_state.wrapping_mul(48271).wrapping_rem(2_147_483_647);
        rng_state as i32
    };

    println!("Running {} attacks...", num_attacks);
    let start = Instant::now();

    for i in 0..num_attacks {
        // Random attack position and direction
        let origin = Position::new(
            f32::from((rand() % 100 - 50) as i16),
            0.0,
            f32::from((rand() % 100 - 50) as i16),
        );
        let dir_x = f32::from((rand() % 200 - 100) as i16) / 100.0;
        let dir_z = f32::from((rand() % 200 - 100) as i16) / 100.0;
        let len = (dir_x * dir_x + dir_z * dir_z).sqrt().max(0.001);
        let direction = (dir_x / len, 0.0, dir_z / len);

        metrics.attacks_sent += 1;

        if let Some((rtt_us, result)) = client.attack(origin, direction) {
            metrics.responses_received += 1;
            metrics.avg_rtt_us = (metrics.avg_rtt_us * metrics.responses_received.saturating_sub(1) + rtt_us) 
                / metrics.responses_received.max(1);
            metrics.min_rtt_us = metrics.min_rtt_us.min(rtt_us);
            metrics.max_rtt_us = metrics.max_rtt_us.max(rtt_us);
            metrics.avg_server_processing_us = (metrics.avg_server_processing_us * metrics.responses_received.saturating_sub(1) 
                + result.processing_time_us) / metrics.responses_received.max(1);

            if result.hit {
                metrics.hits += 1;
            } else {
                metrics.misses += 1;
            }
        }

        if (i + 1) % 100 == 0 {
            print!("\rProgress: {}/{}", i + 1, num_attacks);
        }
    }
    println!();

    let elapsed = start.elapsed();
    
    // Stop server
    running.store(false, Ordering::Release);
    // Send dummy packet to wake up server
    let _ = UdpSocket::bind("127.0.0.1:0")
        .and_then(|s| s.send_to(&[0], config.server_addr));
    
    let _ = server_handle.join();

    // Check RTT requirement
    metrics.rtt_requirement_met = (metrics.max_rtt_us / 1000) < config.max_rtt_ms;

    // Print results
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                    VERTICAL SLICE RESULTS                        ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("┌─ THROUGHPUT ───────────────────────────────────────────────────┐");
    println!("│ Total Time:         {:.2}s                                      ", elapsed.as_secs_f64());
    println!("│ Attacks/sec:        {:.0}                                       ", f64::from(num_attacks) / elapsed.as_secs_f64());
    println!("│ Response Rate:      {:.1}%                                      ", 
        metrics.responses_received as f64 / metrics.attacks_sent as f64 * 100.0);
    println!("│ Hit Rate:           {:.1}%                                      ", 
        metrics.hits as f64 / metrics.responses_received.max(1) as f64 * 100.0);
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─ LATENCY (THE CRITICAL METRIC) ─────────────────────────────────┐");
    println!("│                                                                  │");
    println!("│ Average RTT:        {:.3} ms                                    ", metrics.avg_rtt_us as f64 / 1000.0);
    println!("│ Minimum RTT:        {:.3} ms                                    ", metrics.min_rtt_us as f64 / 1000.0);
    println!("│ Maximum RTT:        {:.3} ms                                    ", metrics.max_rtt_us as f64 / 1000.0);
    println!("│                                                                  │");
    println!("│ Server Processing:  {:.1} μs (average)                          ", metrics.avg_server_processing_us as f64);
    println!("│                                                                  │");
    
    if metrics.rtt_requirement_met {
        println!("│ ✓ RTT REQUIREMENT MET: Max {:.3}ms < {}ms target             ", 
            metrics.max_rtt_us as f64 / 1000.0, config.max_rtt_ms);
    } else {
        println!("│ ✗ RTT REQUIREMENT FAILED: Max {:.3}ms > {}ms target          ", 
            metrics.max_rtt_us as f64 / 1000.0, config.max_rtt_ms);
    }
    println!("└──────────────────────────────────────────────────────────────────┘");

    metrics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertical_slice_local() {
        let config = VerticalSliceConfig {
            server_addr: "127.0.0.1:18888".parse().unwrap(),
            client_addr: "127.0.0.1:18889".parse().unwrap(),
            entity_count: 100,
            max_rtt_ms: 50,
        };

        let metrics = run_vertical_slice_test(config, 100);

        assert!(metrics.responses_received > 90, "Should receive most responses");
        assert!(metrics.rtt_requirement_met, "RTT should be under 50ms on localhost");
        
        println!("\nAverage RTT: {:.3} ms", metrics.avg_rtt_us as f64 / 1000.0);
        println!("Max RTT: {:.3} ms", metrics.max_rtt_us as f64 / 1000.0);
    }
}
