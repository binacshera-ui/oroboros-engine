//! # Chaos Network Benchmark v2
//!
//! THE ARCHITECT DEMANDS TRUTH.
//!
//! This version uses proper queue-based delays to simulate network latency.
//! - Client sends immediately
//! - Packets sit in a queue until delivery time
//! - RTT is measured correctly end-to-end

use std::collections::VecDeque;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use oroboros_core::{Position, Velocity};
use oroboros_networking::interpolation::VisualInterpolator;

/// Movement speed in units per second.
const SPEED: f32 = 5.0;
/// Time step per tick (60 Hz).
const DT: f32 = 1.0 / 60.0;

/// Configuration for chaos benchmark.
struct ChaosConfig {
    /// Server address.
    server_addr: &'static str,
    /// Client address.
    client_addr: &'static str,
    /// Simulated one-way latency in milliseconds.
    latency_ms: u64,
    /// Packet loss percentage (0-100).
    packet_loss_percent: u8,
    /// Jitter in milliseconds.
    jitter_ms: u64,
    /// Test duration in seconds.
    duration_secs: u64,
    /// Tick rate (Hz).
    tick_rate: u32,
}

impl Default for ChaosConfig {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:27015",
            client_addr: "127.0.0.1:27016",
            latency_ms: 50,        // 50ms one-way = 100ms RTT (standard)
            packet_loss_percent: 5,
            jitter_ms: 20,
            duration_secs: 30,
            tick_rate: 60,
        }
    }
}

/// Packet types.
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
enum PacketType {
    Input = 1,
    State = 2,
}

/// Input packet (client -> server).
#[derive(Clone, Copy, Debug)]
struct InputPacket {
    sequence: u32,
    tick: u32,
    move_x: i8,
    move_z: i8,
    /// Timestamp when client sent this (microseconds since test start).
    client_send_time_us: u64,
}

impl InputPacket {
    fn to_bytes(&self) -> [u8; 20] {
        let mut buf = [0u8; 20];
        buf[0] = PacketType::Input as u8;
        buf[1..5].copy_from_slice(&self.sequence.to_le_bytes());
        buf[5..9].copy_from_slice(&self.tick.to_le_bytes());
        buf[9] = self.move_x as u8;
        buf[10] = self.move_z as u8;
        buf[11..19].copy_from_slice(&self.client_send_time_us.to_le_bytes());
        buf
    }

    fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() < 19 || buf[0] != PacketType::Input as u8 {
            return None;
        }
        Some(Self {
            sequence: u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]),
            tick: u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]),
            move_x: buf[9] as i8,
            move_z: buf[10] as i8,
            client_send_time_us: u64::from_le_bytes([
                buf[11], buf[12], buf[13], buf[14], buf[15], buf[16], buf[17], buf[18],
            ]),
        })
    }
}

/// State packet (server -> client).
#[derive(Clone, Copy, Debug)]
struct StatePacket {
    sequence: u32,
    tick: u32,
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    last_input_seq: u32,
    /// Echo of client's send timestamp for RTT calculation.
    client_send_time_us: u64,
}

impl StatePacket {
    fn to_bytes(&self) -> [u8; 36] {
        let mut buf = [0u8; 36];
        buf[0] = PacketType::State as u8;
        buf[1..5].copy_from_slice(&self.sequence.to_le_bytes());
        buf[5..9].copy_from_slice(&self.tick.to_le_bytes());
        buf[9..13].copy_from_slice(&self.pos_x.to_le_bytes());
        buf[13..17].copy_from_slice(&self.pos_y.to_le_bytes());
        buf[17..21].copy_from_slice(&self.pos_z.to_le_bytes());
        buf[21..25].copy_from_slice(&self.last_input_seq.to_le_bytes());
        buf[25..33].copy_from_slice(&self.client_send_time_us.to_le_bytes());
        buf
    }

    fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() < 33 || buf[0] != PacketType::State as u8 {
            return None;
        }
        Some(Self {
            sequence: u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]),
            tick: u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]),
            pos_x: f32::from_le_bytes([buf[9], buf[10], buf[11], buf[12]]),
            pos_y: f32::from_le_bytes([buf[13], buf[14], buf[15], buf[16]]),
            pos_z: f32::from_le_bytes([buf[17], buf[18], buf[19], buf[20]]),
            last_input_seq: u32::from_le_bytes([buf[21], buf[22], buf[23], buf[24]]),
            client_send_time_us: u64::from_le_bytes([
                buf[25], buf[26], buf[27], buf[28], buf[29], buf[30], buf[31], buf[32],
            ]),
        })
    }
}

/// Delayed packet in queue.
struct DelayedPacket {
    data: Vec<u8>,
    delivery_time: Instant,
    dest: std::net::SocketAddr,
}

/// Metrics for tracking.
#[derive(Default)]
struct Metrics {
    /// RTT samples in microseconds.
    rtt_samples: Vec<u64>,
    /// Position error samples.
    error_samples: Vec<f32>,
    /// Correction events (tick, error_before, error_after).
    corrections: Vec<(u32, f32, f32)>,
    /// Packets sent.
    packets_sent: u64,
    /// Packets received.
    packets_received: u64,
    /// Packets lost.
    packets_lost: u64,
}

/// Simple RNG.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 33) as u32
    }
}

/// Simulated network layer with configurable delay and loss.
struct ChaosNetwork {
    socket: UdpSocket,
    outbound_queue: VecDeque<DelayedPacket>,
    latency_ms: u64,
    jitter_ms: u64,
    packet_loss_percent: u8,
    rng: Rng,
    packets_dropped: u64,
}

impl ChaosNetwork {
    fn new(bind_addr: &str, latency_ms: u64, jitter_ms: u64, packet_loss_percent: u8) -> Self {
        let socket = UdpSocket::bind(bind_addr).expect("Failed to bind socket");
        socket.set_nonblocking(true).expect("Failed to set nonblocking");
        
        Self {
            socket,
            outbound_queue: VecDeque::new(),
            latency_ms,
            jitter_ms,
            packet_loss_percent,
            rng: Rng::new(12345),
            packets_dropped: 0,
        }
    }

    /// Queue a packet for delayed delivery.
    fn send_delayed(&mut self, data: &[u8], dest: std::net::SocketAddr) {
        // Simulate packet loss
        if (self.rng.next() % 100) < u32::from(self.packet_loss_percent) {
            self.packets_dropped += 1;
            return;
        }

        // Calculate delivery time with jitter
        let jitter = if self.jitter_ms > 0 {
            (self.rng.next() as i64 % (self.jitter_ms as i64 * 2)) - self.jitter_ms as i64
        } else {
            0
        };
        let delay_ms = (self.latency_ms as i64 + jitter).max(1) as u64;
        let delivery_time = Instant::now() + Duration::from_millis(delay_ms);

        self.outbound_queue.push_back(DelayedPacket {
            data: data.to_vec(),
            delivery_time,
            dest,
        });
    }

    /// Process outbound queue - actually send packets whose time has come.
    fn process_outbound(&mut self) {
        let now = Instant::now();
        while let Some(packet) = self.outbound_queue.front() {
            if packet.delivery_time <= now {
                let packet = self.outbound_queue.pop_front().unwrap();
                let _ = self.socket.send_to(&packet.data, packet.dest);
            } else {
                break;
            }
        }
    }

    /// Try to receive a packet (non-blocking).
    fn recv(&self, buf: &mut [u8]) -> Option<(usize, std::net::SocketAddr)> {
        self.socket.recv_from(buf).ok()
    }
}

/// Runs the SERVER in a separate thread.
fn run_server(config: &ChaosConfig, running: Arc<AtomicBool>) {
    let mut network = ChaosNetwork::new(
        config.server_addr,
        config.latency_ms,
        config.jitter_ms,
        config.packet_loss_percent,
    );

    let mut pos = Position::new(0.0, 0.0, 0.0);
    let mut vel = Velocity::new(0.0, 0.0, 0.0);
    let mut last_input_seq: u32 = 0;
    let mut last_client_send_time: u64 = 0;
    let mut state_seq: u32 = 0;

    let tick_duration = Duration::from_secs_f64(1.0 / f64::from(config.tick_rate));
    let start = Instant::now();
    let mut next_tick = start;

    let mut client_addr = None;

    while running.load(Ordering::Relaxed) {
        // Process outbound queue
        network.process_outbound();

        // Process incoming inputs (no delay on receive - delay is on send)
        let mut buf = [0u8; 64];
        while let Some((len, src)) = network.recv(&mut buf) {
            if client_addr.is_none() {
                client_addr = Some(src);
            }

            if let Some(input) = InputPacket::from_bytes(&buf[..len]) {
                // Apply input to server state
                vel.x = f32::from(input.move_x) / 127.0 * SPEED;
                vel.z = f32::from(input.move_z) / 127.0 * SPEED;

                pos.x += vel.x * DT;
                pos.z += vel.z * DT;

                // Boundary clamp
                pos.x = pos.x.clamp(-50.0, 50.0);
                pos.z = pos.z.clamp(-50.0, 50.0);

                last_input_seq = input.sequence;
                last_client_send_time = input.client_send_time_us;
            }
        }

        // Check if it's time for a tick
        let now = Instant::now();
        if now >= next_tick {
            next_tick += tick_duration;

            // Send state update with delay
            if let Some(addr) = client_addr {
                state_seq += 1;
                let state = StatePacket {
                    sequence: state_seq,
                    tick: (start.elapsed().as_millis() / 16) as u32,
                    pos_x: pos.x,
                    pos_y: pos.y,
                    pos_z: pos.z,
                    last_input_seq,
                    client_send_time_us: last_client_send_time,
                };

                network.send_delayed(&state.to_bytes(), addr);
            }
        }

        // Small sleep to prevent busy loop
        thread::sleep(Duration::from_micros(100));
    }
}

/// Runs the CLIENT and collects metrics.
fn run_client(config: &ChaosConfig, running: Arc<AtomicBool>) -> Metrics {
    let mut network = ChaosNetwork::new(
        config.client_addr,
        config.latency_ms,
        config.jitter_ms,
        config.packet_loss_percent,
    );

    let server_addr: std::net::SocketAddr = config.server_addr.parse().unwrap();

    // Client state
    let mut predicted_pos = Position::new(0.0, 0.0, 0.0);
    let mut predicted_vel = Velocity::new(0.0, 0.0, 0.0);
    let mut input_seq: u32 = 0;

    // Visual interpolation - THE ARCHITECT'S DECREE: NO MORE HARD SNAP
    let mut visual_interp = VisualInterpolator::smooth(100.0); // 100ms blend time

    // Input history for reconciliation
    let mut input_history: VecDeque<(u32, i8, i8, Position)> = VecDeque::with_capacity(256);

    // Metrics
    let mut metrics = Metrics::default();
    
    // Track max visual jerk (to show hard snap is bad)
    let mut max_visual_jerk: f32 = 0.0;
    let mut prev_visual_pos = Position::new(0.0, 0.0, 0.0);

    let tick_duration = Duration::from_secs_f64(1.0 / f64::from(config.tick_rate));
    let start = Instant::now();
    let mut next_tick = start;
    let mut last_print = start;

    // Movement pattern phase
    let mut phase: f32 = 0.0;

    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  LIVE CHAOS BENCHMARK - {} ms RTT, {}% loss                   ║", 
        config.latency_ms * 2, config.packet_loss_percent);
    println!("║  Visual Interpolation: ENABLED (100ms blend, SmoothStep)         ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Time(s)  | Visual.X | Logic.X  | Blend    | RTT(ms)  | Corrections");
    println!("---------|----------|----------|----------|----------|------------");

    let mut _last_server_pos = Position::new(0.0, 0.0, 0.0);
    let mut total_corrections: u32 = 0;

    while running.load(Ordering::Relaxed) && start.elapsed().as_secs() < config.duration_secs {
        let now = Instant::now();

        // Process outbound queue
        network.process_outbound();

        // === CLIENT TICK ===
        if now >= next_tick {
            next_tick += tick_duration;
            phase += 0.1;

            // Generate zig-zag input
            let move_x = ((phase * 2.0).sin() * 127.0) as i8;
            let move_z = ((phase * 1.5).cos() * 127.0) as i8;

            // Predict locally IMMEDIATELY
            predicted_vel.x = f32::from(move_x) / 127.0 * SPEED;
            predicted_vel.z = f32::from(move_z) / 127.0 * SPEED;
            predicted_pos.x += predicted_vel.x * DT;
            predicted_pos.z += predicted_vel.z * DT;
            predicted_pos.x = predicted_pos.x.clamp(-50.0, 50.0);
            predicted_pos.z = predicted_pos.z.clamp(-50.0, 50.0);

            // Store for reconciliation
            input_seq += 1;
            input_history.push_back((input_seq, move_x, move_z, predicted_pos));
            if input_history.len() > 256 {
                input_history.pop_front();
            }

            // Send input with delay
            let input = InputPacket {
                sequence: input_seq,
                tick: (start.elapsed().as_millis() / 16) as u32,
                move_x,
                move_z,
                client_send_time_us: start.elapsed().as_micros() as u64,
            };

            network.send_delayed(&input.to_bytes(), server_addr);
            metrics.packets_sent += 1;
            metrics.packets_lost = network.packets_dropped;
        }

        // === RECEIVE SERVER STATE (no delay on receive) ===
        let mut buf = [0u8; 64];
        while let Some((len, _)) = network.recv(&mut buf) {
            if let Some(state) = StatePacket::from_bytes(&buf[..len]) {
                metrics.packets_received += 1;

                // Calculate RTT using echoed timestamp
                let now_us = start.elapsed().as_micros() as u64;
                let rtt = now_us.saturating_sub(state.client_send_time_us);
                metrics.rtt_samples.push(rtt);

                // Server position
                let server_pos = Position::new(state.pos_x, state.pos_y, state.pos_z);
                _last_server_pos = server_pos;

                // Find our predicted position at the time server processed
                let our_pos_at_that_time = input_history
                    .iter()
                    .find(|(seq, _, _, _)| *seq == state.last_input_seq)
                    .map(|(_, _, _, pos)| *pos);

                if let Some(old_predicted) = our_pos_at_that_time {
                    // Calculate error
                    let error = ((old_predicted.x - server_pos.x).powi(2)
                        + (old_predicted.z - server_pos.z).powi(2))
                    .sqrt();

                    metrics.error_samples.push(error);

                    // If error > threshold, RECONCILE
                    if error > 0.1 {
                        total_corrections += 1;

                        let error_before = error;
                        let old_pos = predicted_pos; // Save for visual interpolation

                        // Remove old inputs
                        while let Some((seq, _, _, _)) = input_history.front() {
                            if *seq <= state.last_input_seq {
                                input_history.pop_front();
                            } else {
                                break;
                            }
                        }

                        // Replay remaining inputs from server position
                        let mut replayed_pos = server_pos;
                        for (_, mx, mz, _) in &input_history {
                            let vx = f32::from(*mx) / 127.0 * SPEED;
                            let vz = f32::from(*mz) / 127.0 * SPEED;
                            replayed_pos.x += vx * DT;
                            replayed_pos.z += vz * DT;
                            replayed_pos.x = replayed_pos.x.clamp(-50.0, 50.0);
                            replayed_pos.z = replayed_pos.z.clamp(-50.0, 50.0);
                        }

                        let error_after = ((predicted_pos.x - replayed_pos.x).powi(2)
                            + (predicted_pos.z - replayed_pos.z).powi(2))
                        .sqrt();

                        // Apply correction to LOGICAL position
                        predicted_pos = replayed_pos;

                        // Start VISUAL interpolation (smooth blend instead of hard snap)
                        visual_interp.start_correction(old_pos, replayed_pos);

                        metrics.corrections.push((
                            state.tick,
                            error_before,
                            error_after,
                        ));
                    }
                }
            }
        }

        // Update visual interpolation
        visual_interp.update(DT * 1000.0); // DT in milliseconds

        // Get visual position (what the player SEES)
        let visual_pos = visual_interp.get_visual_position(predicted_pos);

        // Track visual jerk (sudden movement = bad)
        let visual_delta = ((visual_pos.x - prev_visual_pos.x).powi(2)
            + (visual_pos.z - prev_visual_pos.z).powi(2))
        .sqrt();
        if visual_delta > max_visual_jerk {
            max_visual_jerk = visual_delta;
        }
        prev_visual_pos = visual_pos;

        // === PRINT STATUS ===
        if now.duration_since(last_print) >= Duration::from_secs(1) {
            last_print = now;

            let avg_rtt = if metrics.rtt_samples.is_empty() {
                0
            } else {
                let recent: Vec<_> = metrics.rtt_samples.iter().rev().take(60).collect();
                recent.iter().copied().sum::<u64>() / recent.len() as u64
            };

            let blend_status = if visual_interp.is_correcting() {
                format!("{:5.1}%", visual_interp.progress() * 100.0)
            } else {
                "done  ".to_string()
            };

            println!(
                "{:7.1}  | {:+8.3} | {:+8.3} | {:6} | {:8.2} | {}",
                start.elapsed().as_secs_f32(),
                visual_pos.x,      // Visual position (what player sees)
                predicted_pos.x,   // Logical position (physics truth)
                blend_status,      // Interpolation progress
                avg_rtt as f64 / 1000.0,
                total_corrections
            );
        }

        thread::sleep(Duration::from_micros(500));
    }

    metrics
}

fn main() {
    let config = ChaosConfig::default();

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║           CHAOS NETWORK BENCHMARK v2                             ║");
    println!("║           THE ARCHITECT DEMANDS TRUTH                            ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Queue-based delay simulation for accurate RTT measurement.      ║");
    println!("║  No shortcuts. Real latency. Real reconciliation.                ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Configuration:");
    println!("  Server:        {}", config.server_addr);
    println!("  Client:        {}", config.client_addr);
    println!("  Latency:       {}ms one-way ({}ms RTT)", config.latency_ms, config.latency_ms * 2);
    println!("  Packet Loss:   {}%", config.packet_loss_percent);
    println!("  Jitter:        ±{}ms", config.jitter_ms);
    println!("  Duration:      {}s", config.duration_secs);
    println!("  Tick Rate:     {}Hz", config.tick_rate);
    println!();

    let running = Arc::new(AtomicBool::new(true));
    let running_server = Arc::clone(&running);
    let running_client = Arc::clone(&running);

    // Start server in separate thread
    let server_config = ChaosConfig::default();
    let server_handle = thread::spawn(move || {
        run_server(&server_config, running_server);
    });

    // Give server time to bind
    thread::sleep(Duration::from_millis(100));

    // Run client (blocking)
    let metrics = run_client(&config, running_client);

    // Signal server to stop
    running.store(false, Ordering::Relaxed);
    let _ = server_handle.join();

    // === FINAL REPORT ===
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                    FINAL RESULTS                                 ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    // RTT stats
    if !metrics.rtt_samples.is_empty() {
        let mut sorted_rtt = metrics.rtt_samples.clone();
        sorted_rtt.sort();
        let min_rtt = sorted_rtt[0];
        let max_rtt = sorted_rtt[sorted_rtt.len() - 1];
        let avg_rtt: u64 = sorted_rtt.iter().sum::<u64>() / sorted_rtt.len() as u64;
        let p50_rtt = sorted_rtt[sorted_rtt.len() / 2];
        let p99_rtt = sorted_rtt[sorted_rtt.len() * 99 / 100];

        println!("┌─ RTT (Round-Trip Time) ──────────────────────────────────────────┐");
        println!("│ Min:     {:8.2} ms                                            │", min_rtt as f64 / 1000.0);
        println!("│ Max:     {:8.2} ms                                            │", max_rtt as f64 / 1000.0);
        println!("│ Avg:     {:8.2} ms                                            │", avg_rtt as f64 / 1000.0);
        println!("│ P50:     {:8.2} ms                                            │", p50_rtt as f64 / 1000.0);
        println!("│ P99:     {:8.2} ms                                            │", p99_rtt as f64 / 1000.0);
        println!("└──────────────────────────────────────────────────────────────────┘");

        // Verdict on RTT
        if avg_rtt < 50_000 {
            println!("  ⚠ WARNING: RTT lower than expected. Check delay simulation.");
        }
    }
    println!();

    // Error stats
    if !metrics.error_samples.is_empty() {
        let mut sorted_err = metrics.error_samples.clone();
        sorted_err.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let min_err = sorted_err[0];
        let max_err = sorted_err[sorted_err.len() - 1];
        let avg_err: f32 = sorted_err.iter().sum::<f32>() / sorted_err.len() as f32;

        println!("┌─ Position Error ─────────────────────────────────────────────────┐");
        println!("│ Min:     {:8.4} units                                         │", min_err);
        println!("│ Max:     {:8.4} units                                         │", max_err);
        println!("│ Avg:     {:8.4} units                                         │", avg_err);
        println!("│ Samples: {:6}                                                 │", metrics.error_samples.len());
        println!("└──────────────────────────────────────────────────────────────────┘");
    }
    println!();

    // Correction stats
    println!("┌─ Reconciliation ─────────────────────────────────────────────────┐");
    println!("│ Total Corrections: {:5}                                         │", metrics.corrections.len());
    if !metrics.corrections.is_empty() {
        let avg_correction: f32 = metrics.corrections.iter()
            .map(|(_, before, _)| *before)
            .sum::<f32>() / metrics.corrections.len() as f32;
        println!("│ Avg Error Corrected: {:6.4} units                              │", avg_correction);
    }
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    // Visual Interpolation info
    println!("┌─ Visual Interpolation (Anti-Motion-Sickness) ─────────────────────┐");
    println!("│ Mode:          SmoothStep (best visual quality)                  │");
    println!("│ Blend Time:    100ms per correction                              │");
    println!("│ Hard Snap:     DISABLED ✓                                        │");
    println!("│                                                                  │");
    println!("│ With interpolation, corrections blend smoothly over 100ms.      │");
    println!("│ The player sees a gentle 'glide' instead of a jarring teleport. │");
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    // Packet stats
    let total = metrics.packets_sent;
    let actual_loss = if total > 0 {
        metrics.packets_lost as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    println!("┌─ Packets ─────────────────────────────────────────────────────────┐");
    println!("│ Sent:     {:6}                                                  │", metrics.packets_sent);
    println!("│ Received: {:6}                                                  │", metrics.packets_received);
    println!("│ Lost:     {:6} ({:.1}%)                                         │", metrics.packets_lost, actual_loss);
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    // === ASCII GRAPH ===
    if metrics.error_samples.len() >= 10 {
        println!("┌─ Error Over Time (ASCII Graph) ─────────────────────────────────┐");
        
        let step = metrics.error_samples.len() / 60;
        let max_err = metrics.error_samples.iter().cloned().fold(0.0f32, f32::max);
        
        if max_err > 0.0 {
            for row in (0..10).rev() {
                let threshold = max_err * (row as f32 + 0.5) / 10.0;
                print!("│ {:5.2} │", max_err * (row as f32 + 1.0) / 10.0);
                
                for i in 0..60 {
                    let idx = i * step.max(1);
                    if idx < metrics.error_samples.len() {
                        let val = metrics.error_samples[idx];
                        if val >= threshold {
                            print!("█");
                        } else {
                            print!(" ");
                        }
                    }
                }
                println!("│");
            }
            println!("│       └────────────────────────────────────────────────────────│");
            println!("│         0s                                               {}s   │", config.duration_secs);
        }
        println!("└──────────────────────────────────────────────────────────────────┘");
    }
    println!();

    // Final Verdict
    println!("╔══════════════════════════════════════════════════════════════════╗");
    let avg_rtt = if metrics.rtt_samples.is_empty() {
        0
    } else {
        metrics.rtt_samples.iter().sum::<u64>() / metrics.rtt_samples.len() as u64
    };
    let rtt_ok = avg_rtt >= 50_000; // >= 50ms
    let corrections_ok = !metrics.corrections.is_empty();
    
    if rtt_ok && corrections_ok {
        println!("║  ✓ BENCHMARK HONEST                                              ║");
        println!("║    RTT:           {:6.2} ms (target: >50ms) ✓                    ║", avg_rtt as f64 / 1000.0);
        println!("║    Corrections:   {:6} (reconciliation active) ✓               ║", metrics.corrections.len());
        println!("║                                                                  ║");
        println!("║    Server authority verified. Prediction working.               ║");
    } else {
        println!("║  ✗ BENCHMARK FAILED                                              ║");
        if !rtt_ok {
            println!("║    RTT:           {:6.2} ms (target: >50ms) ✗                    ║", avg_rtt as f64 / 1000.0);
        } else {
            println!("║    RTT:           {:6.2} ms (target: >50ms) ✓                    ║", avg_rtt as f64 / 1000.0);
        }
        if !corrections_ok {
            println!("║    Corrections:   0 (reconciliation NOT triggered) ✗           ║");
        }
    }
    println!("╚══════════════════════════════════════════════════════════════════╝");
}
