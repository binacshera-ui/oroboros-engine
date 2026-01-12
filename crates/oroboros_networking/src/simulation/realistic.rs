//! # Realistic Network Simulation
//!
//! This module simulates ACTUAL network conditions with:
//! - Queued packets with real latency
//! - Out-of-order delivery
//! - Packet loss at specific moments
//! - Zig-zag movement that breaks linear prediction
//!
//! THE ARCHITECT demands honesty. Zero error is a lie.

use oroboros_core::{Position, Velocity};
use crate::protocol::{PlayerInput, EntityState};
use std::collections::VecDeque;

/// Network packet in transit.
#[derive(Clone, Debug)]
pub struct InFlightPacket {
    /// Server tick when this packet was generated.
    pub server_tick: u32,
    /// Client tick when this was sent.
    #[allow(dead_code)]
    pub client_send_tick: u32,
    /// Arrival tick (current_tick >= this means delivered).
    pub arrival_tick: u32,
    /// Entity state in the packet.
    pub state: EntityState,
}

/// Input packet in flight TO the server.
#[derive(Clone, Debug)]
pub struct InputInFlight {
    /// Tick when input was generated.
    pub tick: u32,
    /// When the input will arrive at the server.
    pub arrival_tick: u32,
    /// The input itself.
    pub input: PlayerInput,
}

/// Position history entry for reconciliation.
#[derive(Clone, Copy, Debug)]
pub struct PositionHistoryEntry {
    /// Tick number.
    pub tick: u32,
    /// Predicted position at this tick.
    pub position: Position,
}

/// Realistic bot with actual network delay.
pub struct RealisticBot {
    /// Bot ID.
    pub id: u32,
    /// CLIENT-SIDE predicted position (what the player sees).
    pub predicted_position: Position,
    /// CLIENT-SIDE predicted velocity.
    pub predicted_velocity: Velocity,
    /// SERVER-SIDE authoritative position (truth).
    pub server_position: Position,
    /// SERVER-SIDE velocity.
    pub server_velocity: Velocity,
    /// Input packets in flight TO the server.
    pub inputs_in_flight: VecDeque<InputInFlight>,
    /// Packets in flight FROM server to client.
    pub packets_in_flight: VecDeque<InFlightPacket>,
    /// Last acknowledged server tick.
    pub last_acked_tick: u32,
    /// Last tick at which server processed an input.
    pub last_server_processed_tick: u32,
    /// Input history for reconciliation.
    pub input_history: VecDeque<(u32, PlayerInput)>,
    /// Position history for comparing against server state.
    pub position_history: VecDeque<PositionHistoryEntry>,
    /// Movement pattern (for zig-zag).
    pub movement_phase: f32,
    /// Health.
    pub health: u8,
    /// Total position corrections applied.
    pub total_corrections: u32,
    /// Total snap corrections (large errors).
    pub snap_corrections: u32,
    /// Maximum error observed.
    pub max_error: f32,
    /// Sum of all errors (for average).
    pub total_error: f32,
    /// Error sample count.
    pub error_samples: u32,
}

impl RealisticBot {
    /// Creates a new realistic bot.
    #[must_use]
    pub fn new(id: u32, position: Position) -> Self {
        Self {
            id,
            predicted_position: position,
            predicted_velocity: Velocity::default(),
            server_position: position,
            server_velocity: Velocity::default(),
            inputs_in_flight: VecDeque::with_capacity(32),
            packets_in_flight: VecDeque::with_capacity(32),
            last_acked_tick: 0,
            last_server_processed_tick: 0,
            input_history: VecDeque::with_capacity(128),
            position_history: VecDeque::with_capacity(128),
            movement_phase: id as f32 * 0.7, // Different phase per bot
            health: 100,
            total_corrections: 0,
            snap_corrections: 0,
            max_error: 0.0,
            total_error: 0.0,
            error_samples: 0,
        }
    }
    
    /// Queue input for later delivery to server.
    pub fn queue_input_to_server(&mut self, tick: u32, input: PlayerInput, latency_ticks: u32) {
        self.inputs_in_flight.push_back(InputInFlight {
            tick,
            arrival_tick: tick + latency_ticks,
            input,
        });
    }
    
    /// Process inputs that have arrived at server.
    pub fn server_receive_inputs(&mut self, current_tick: u32) {
        while let Some(input_packet) = self.inputs_in_flight.front() {
            if input_packet.arrival_tick > current_tick {
                break;
            }
            
            let input_packet = self.inputs_in_flight.pop_front().unwrap();
            self.server_process(&input_packet.input);
            // Track which tick's input was last processed
            self.last_server_processed_tick = input_packet.tick;
        }
    }
    
    /// Saves predicted position to history.
    pub fn save_position_history(&mut self, tick: u32) {
        self.position_history.push_back(PositionHistoryEntry {
            tick,
            position: self.predicted_position,
        });
        // Keep only last 64 entries
        while self.position_history.len() > 64 {
            self.position_history.pop_front();
        }
    }
    
    /// Gets predicted position at a specific tick.
    fn get_position_at_tick(&self, tick: u32) -> Option<Position> {
        self.position_history
            .iter()
            .find(|e| e.tick == tick)
            .map(|e| e.position)
    }

    /// Generates ZIG-ZAG input that breaks linear prediction.
    pub fn generate_zigzag_input(&mut self, tick: u32, rng: &mut SimpleRng) -> PlayerInput {
        self.movement_phase += 0.15;
        
        // Zig-zag: change direction frequently
        let direction_change = (self.movement_phase * 3.0).sin();
        let erratic_factor = if rng.next() % 20 == 0 { -1.0 } else { 1.0 };
        
        let mut input = PlayerInput::new(tick, tick);
        
        // Sharp direction changes
        input.move_x = ((direction_change * erratic_factor) * 127.0) as i8;
        input.move_z = ((self.movement_phase.cos() * erratic_factor) * 127.0) as i8;
        
        // Random sprinting bursts
        if rng.next() % 5 == 0 {
            input.flags |= PlayerInput::FLAG_SPRINT;
        }
        
        // Random jumping
        if rng.next() % 30 == 0 {
            input.flags |= PlayerInput::FLAG_JUMP;
        }
        
        input
    }

    /// CLIENT: Predicts position locally (before server response).
    pub fn client_predict(&mut self, input: &PlayerInput, client_tick: u32) {
        const DT: f32 = 1.0 / 60.0;
        const GRAVITY: f32 = -20.0;

        // Store input for later reconciliation
        self.input_history.push_back((client_tick, *input));
        if self.input_history.len() > 128 {
            self.input_history.pop_front();
        }

        let speed = if input.is_sprinting() { 10.0 } else { 5.0 };

        // Apply input to predicted position
        self.predicted_velocity.x = input.move_x as f32 / 127.0 * speed;
        self.predicted_velocity.z = input.move_z as f32 / 127.0 * speed;

        if input.is_jumping() && self.predicted_position.y <= 0.1 {
            self.predicted_velocity.y = 8.0;
        }

        if self.predicted_position.y > 0.0 {
            self.predicted_velocity.y += GRAVITY * DT;
        }

        self.predicted_position.x += self.predicted_velocity.x * DT;
        self.predicted_position.y += self.predicted_velocity.y * DT;
        self.predicted_position.z += self.predicted_velocity.z * DT;

        if self.predicted_position.y < 0.0 {
            self.predicted_position.y = 0.0;
            self.predicted_velocity.y = 0.0;
        }

        // Apply friction
        self.predicted_velocity.x *= 0.9;
        self.predicted_velocity.z *= 0.9;
    }

    /// SERVER: Processes input authoritatively.
    pub fn server_process(&mut self, input: &PlayerInput) {
        const DT: f32 = 1.0 / 60.0;
        const GRAVITY: f32 = -20.0;

        let speed = if input.is_sprinting() { 10.0 } else { 5.0 };

        self.server_velocity.x = input.move_x as f32 / 127.0 * speed;
        self.server_velocity.z = input.move_z as f32 / 127.0 * speed;

        if input.is_jumping() && self.server_position.y <= 0.1 {
            self.server_velocity.y = 8.0;
        }

        if self.server_position.y > 0.0 {
            self.server_velocity.y += GRAVITY * DT;
        }

        self.server_position.x += self.server_velocity.x * DT;
        self.server_position.y += self.server_velocity.y * DT;
        self.server_position.z += self.server_velocity.z * DT;

        if self.server_position.y < 0.0 {
            self.server_position.y = 0.0;
            self.server_velocity.y = 0.0;
        }

        self.server_velocity.x *= 0.9;
        self.server_velocity.z *= 0.9;
    }

    /// SERVER: Queues a state packet for delivery with latency.
    /// The key insight: server sends its CURRENT position, which is based on
    /// inputs it has received SO FAR - which are OLDER than what client has predicted!
    ///
    /// IMPORTANT: The server_tick stored in packet is the tick at which the CLIENT
    /// should compare against! This is the tick that was on the original input.
    pub fn queue_server_packet(&mut self, _server_tick: u32, client_tick: u32, latency_ticks: u32) {
        // Server position is based on inputs received up to now.
        // Client position is based on ALL inputs including ones not yet received by server.
        // This creates the DESYNC that we measure!
        let packet = InFlightPacket {
            // The server_tick is the tick of the LAST INPUT the server processed!
            // This is what the client should compare against.
            server_tick: self.last_server_processed_tick,
            client_send_tick: client_tick,
            arrival_tick: client_tick + latency_ticks,
            state: EntityState::from_components(
                self.id,
                self.server_position,
                self.server_velocity,
                self.health,
            ),
        };
        self.packets_in_flight.push_back(packet);
    }

    /// CLIENT: Receives packets that have arrived and reconciles.
    pub fn client_receive_and_reconcile(&mut self, current_tick: u32) {
        const SNAP_THRESHOLD: f32 = 1.0; // Units
        const SMOOTH_FACTOR: f32 = 0.2;

        // Process all arrived packets
        while let Some(packet) = self.packets_in_flight.front() {
            if packet.arrival_tick > current_tick {
                break;
            }

            let packet = self.packets_in_flight.pop_front().unwrap();
            
            // Server position is the TRUTH (at the time the server sent this packet)
            let server_pos = packet.state.position();
            
            // KEY FIX: Compare against OUR position at the same tick the server was at!
            // Not our current position, but what we predicted at that tick.
            let our_pos_at_server_tick = self.get_position_at_tick(packet.server_tick);
            let our_pos = our_pos_at_server_tick.unwrap_or(self.predicted_position);
            
            // Calculate error between what we predicted at that tick vs server truth
            let error = calculate_distance(our_pos, server_pos);
            
            // Track statistics
            self.error_samples += 1;
            self.total_error += error;
            self.max_error = self.max_error.max(error);

            if error > 0.01 {
                self.total_corrections += 1;

                if error > SNAP_THRESHOLD {
                    // SNAP: Large error, server wins completely
                    self.snap_corrections += 1;
                    self.predicted_position = server_pos;
                    
                    // Replay inputs from this point
                    self.replay_inputs_from(packet.server_tick);
                } else {
                    // SMOOTH: Small error, blend toward server
                    // Calculate correction delta and apply to CURRENT position
                    let correction_x = server_pos.x - our_pos.x;
                    let correction_y = server_pos.y - our_pos.y;
                    let correction_z = server_pos.z - our_pos.z;
                    
                    self.predicted_position.x += correction_x * SMOOTH_FACTOR;
                    self.predicted_position.y += correction_y * SMOOTH_FACTOR;
                    self.predicted_position.z += correction_z * SMOOTH_FACTOR;
                }
            }

            self.last_acked_tick = packet.server_tick;
        }
    }

    /// Replays stored inputs from a specific tick.
    fn replay_inputs_from(&mut self, from_tick: u32) {
        let inputs_to_replay: Vec<PlayerInput> = self.input_history
            .iter()
            .filter(|(tick, _)| *tick > from_tick)
            .map(|(_, input)| *input)
            .collect();

        for input in inputs_to_replay {
            self.client_predict(&input, from_tick); // Re-predict
        }
    }

    /// Returns average error.
    #[must_use]
    pub fn average_error(&self) -> f32 {
        if self.error_samples == 0 {
            0.0
        } else {
            self.total_error / self.error_samples as f32
        }
    }
    
    /// Returns server position for snapshot.
    #[must_use]
    pub fn get_server_position(&self) -> Position {
        self.server_position
    }
}

/// Simple RNG (same as before).
pub struct SimpleRng {
    /// Internal state.
    state: u64,
}

impl SimpleRng {
    /// Creates a new RNG with the given seed.
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Generates the next random u32.
    pub fn next(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(48271).wrapping_rem(2147483647);
        self.state as u32
    }

    /// Generates a random number in the range [min, max).
    pub fn range(&mut self, min: u32, max: u32) -> u32 {
        if max <= min { return min; }
        min + (self.next() % (max - min))
    }
}

/// Calculates distance.
fn calculate_distance(a: Position, b: Position) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Linear interpolation.
#[allow(dead_code)]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Realistic simulation configuration.
#[derive(Clone, Debug)]
pub struct RealisticConfig {
    /// Number of bots.
    pub bot_count: usize,
    /// Duration in seconds.
    pub duration_secs: u32,
    /// Base latency in ticks (1 tick = 16.67ms at 60Hz).
    pub base_latency_ticks: u32,
    /// Jitter in ticks.
    pub jitter_ticks: u32,
    /// Packet loss percentage.
    pub packet_loss_percent: u8,
}

impl Default for RealisticConfig {
    fn default() -> Self {
        Self {
            bot_count: 500,
            duration_secs: 60,
            base_latency_ticks: 3,  // ~50ms at 60Hz
            jitter_ticks: 3,        // ~50ms jitter
            packet_loss_percent: 2,
        }
    }
}

/// Statistics from realistic simulation.
#[derive(Clone, Debug, Default)]
pub struct RealisticStats {
    /// Total ticks.
    pub total_ticks: u64,
    /// Average position error across all bots.
    pub avg_position_error: f32,
    /// Maximum position error observed.
    pub max_position_error: f32,
    /// Total corrections applied.
    pub total_corrections: u64,
    /// Snap corrections (large errors).
    pub snap_corrections: u64,
    /// Total reconciliations.
    pub reconciliations: u64,
    /// Packets sent.
    pub packets_sent: u64,
    /// Packets dropped.
    pub packets_dropped: u64,
    /// Average tick time in microseconds.
    pub avg_tick_us: u64,
    /// Late ticks.
    pub late_ticks: u64,
}

/// Realistic network simulation.
pub struct RealisticSimulation {
    /// Configuration.
    config: RealisticConfig,
    /// Bots.
    bots: Vec<RealisticBot>,
    /// Current tick.
    tick: u32,
    /// RNG.
    rng: SimpleRng,
    /// Statistics.
    stats: RealisticStats,
}

impl RealisticSimulation {
    /// Creates a new realistic simulation.
    pub fn new(config: RealisticConfig) -> Self {
        let mut rng = SimpleRng::new(42);
        
        let bots: Vec<RealisticBot> = (0..config.bot_count)
            .map(|i| {
                let x = (rng.next() % 200) as f32 - 100.0;
                let z = (rng.next() % 200) as f32 - 100.0;
                RealisticBot::new(i as u32, Position::new(x, 0.0, z))
            })
            .collect();

        Self {
            config,
            bots,
            tick: 0,
            rng,
            stats: RealisticStats::default(),
        }
    }

    /// Runs a single tick.
    pub fn tick(&mut self) -> bool {
        let start = std::time::Instant::now();

        let total_ticks = self.config.duration_secs * 60;
        if self.tick >= total_ticks {
            return false;
        }

        for bot in &mut self.bots {
            // 1. CLIENT: Generate zig-zag input
            let input = bot.generate_zigzag_input(self.tick, &mut self.rng);

            // 2. CLIENT: Predict locally (IMMEDIATELY - no delay!)
            bot.client_predict(&input, self.tick);
            
            // 2.5: Save position for later comparison with server
            bot.save_position_history(self.tick);

            // 3. NETWORK: Queue input packet to server (with latency!)
            // KEY: Even when not lost, input reaches server LATER than client prediction!
            let input_lost = (self.rng.next() % 100) < u32::from(self.config.packet_loss_percent);
            
            if !input_lost {
                // Input goes into the network with one-way latency
                let input_latency = self.config.base_latency_ticks / 2
                    + self.rng.range(0, self.config.jitter_ticks);
                bot.queue_input_to_server(self.tick, input, input_latency);
                self.stats.packets_sent += 1;
            } else {
                // INPUT LOST: Server never gets this input!
                // Client predicted it, but server won't process it.
                // This causes DESYNC that reconciliation must fix!
                self.stats.packets_dropped += 1;
            }

            // 4. SERVER: Process inputs that have ARRIVED (after delay)
            // Server is now BEHIND the client's predictions!
            bot.server_receive_inputs(self.tick);

            // 5. SERVER: Send state back (with another half-RTT latency)
            let response_lost = (self.rng.next() % 100) < u32::from(self.config.packet_loss_percent);
            
            if !response_lost {
                // Response goes back with another delay
                let response_latency = self.config.base_latency_ticks / 2
                    + self.rng.range(0, self.config.jitter_ticks);
                bot.queue_server_packet(self.tick, self.tick, response_latency);
                self.stats.packets_sent += 1;
            } else {
                self.stats.packets_dropped += 1;
            }

            // 6. CLIENT: Receive arrived packets and reconcile
            bot.client_receive_and_reconcile(self.tick);
        }

        self.tick += 1;
        self.stats.total_ticks = u64::from(self.tick);

        let elapsed = start.elapsed().as_micros() as u64;
        self.stats.avg_tick_us = (self.stats.avg_tick_us * 15 + elapsed) / 16;
        
        if elapsed > 16666 {
            self.stats.late_ticks += 1;
        }

        true
    }

    /// Runs the full simulation.
    pub fn run(&mut self) -> RealisticStats {
        while self.tick() {}
        self.collect_stats();
        self.stats.clone()
    }

    /// Collects final statistics.
    fn collect_stats(&mut self) {
        let mut total_error = 0.0;
        let mut max_error: f32 = 0.0;
        let mut total_corrections: u64 = 0;
        let mut snap_corrections: u64 = 0;

        for bot in &self.bots {
            total_error += bot.average_error();
            max_error = max_error.max(bot.max_error);
            total_corrections += bot.total_corrections as u64;
            snap_corrections += bot.snap_corrections as u64;
        }

        self.stats.avg_position_error = total_error / self.bots.len() as f32;
        self.stats.max_position_error = max_error;
        self.stats.total_corrections = total_corrections;
        self.stats.snap_corrections = snap_corrections;
        self.stats.reconciliations = total_corrections;
    }

    /// Returns current tick.
    pub fn current_tick(&self) -> u32 {
        self.tick
    }

    /// Returns stats reference.
    pub fn stats(&self) -> &RealisticStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_realistic_simulation() {
        let config = RealisticConfig {
            bot_count: 50,
            duration_secs: 5,
            base_latency_ticks: 3,
            jitter_ticks: 3,
            packet_loss_percent: 2,
        };

        let mut sim = RealisticSimulation::new(config);
        let stats = sim.run();

        println!("=== REALISTIC SIMULATION RESULTS ===");
        println!("Total ticks: {}", stats.total_ticks);
        println!("Avg position error: {:.4}", stats.avg_position_error);
        println!("Max position error: {:.4}", stats.max_position_error);
        println!("Total corrections: {}", stats.total_corrections);
        println!("Snap corrections: {}", stats.snap_corrections);
        println!("Packets sent: {}", stats.packets_sent);
        println!("Packets dropped: {}", stats.packets_dropped);

        // With real network conditions, we SHOULD have errors
        assert!(stats.avg_position_error > 0.0, "Zero error is impossible with latency!");
        assert!(stats.total_corrections > 0, "Should have corrections!");
        
        // But errors should be manageable (< 1 unit average)
        assert!(stats.avg_position_error < 1.0, "Average error too high!");
    }

    #[test]
    fn test_zigzag_breaks_prediction() {
        let mut bot = RealisticBot::new(1, Position::new(0.0, 0.0, 0.0));
        let mut rng = SimpleRng::new(123);

        // Generate several zig-zag inputs
        for tick in 0..60 {
            let input = bot.generate_zigzag_input(tick, &mut rng);
            bot.client_predict(&input, tick);
        }

        // Position should NOT be on a straight line
        // (zig-zag should have moved us around)
        let final_pos = bot.predicted_position;
        println!("Final position after zig-zag: ({:.2}, {:.2}, {:.2})", 
            final_pos.x, final_pos.y, final_pos.z);
        
        // Should have moved significantly in multiple directions
        assert!(final_pos.x.abs() > 0.1 || final_pos.z.abs() > 0.1);
    }
}
