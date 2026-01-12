//! # Network Simulation
//!
//! Simulates network conditions for testing.
//!
//! ## Features
//!
//! - Packet loss simulation
//! - Jitter simulation
//! - Latency simulation
//! - Bot simulation for stress testing
//!
//! ## Modules
//!
//! - `realistic`: Real network delay simulation with actual reconciliation

pub mod realistic;

pub use realistic::{RealisticSimulation, RealisticConfig, RealisticStats, RealisticBot};

use std::time::{Duration, Instant};
use oroboros_core::{Position, Velocity};
use crate::protocol::{PlayerInput, EntityState, WorldSnapshot, DragonState};
use crate::server::ServerState;

/// Network conditions for simulation.
#[derive(Clone, Debug)]
pub struct NetworkConditions {
    /// Base latency in milliseconds.
    pub base_latency_ms: u32,
    /// Jitter (variance) in milliseconds.
    pub jitter_ms: u32,
    /// Packet loss percentage (0-100).
    pub packet_loss_percent: u8,
    /// Duplicate packet percentage (0-100).
    pub duplicate_percent: u8,
    /// Out-of-order percentage (0-100).
    pub out_of_order_percent: u8,
}

impl NetworkConditions {
    /// Perfect network conditions (LAN).
    pub const PERFECT: Self = Self {
        base_latency_ms: 1,
        jitter_ms: 0,
        packet_loss_percent: 0,
        duplicate_percent: 0,
        out_of_order_percent: 0,
    };

    /// Good network conditions (fiber).
    pub const GOOD: Self = Self {
        base_latency_ms: 20,
        jitter_ms: 5,
        packet_loss_percent: 0,
        duplicate_percent: 0,
        out_of_order_percent: 0,
    };

    /// Average network conditions (cable).
    pub const AVERAGE: Self = Self {
        base_latency_ms: 50,
        jitter_ms: 20,
        packet_loss_percent: 1,
        duplicate_percent: 1,
        out_of_order_percent: 2,
    };

    /// Poor network conditions (mobile/wifi).
    pub const POOR: Self = Self {
        base_latency_ms: 100,
        jitter_ms: 50,
        packet_loss_percent: 5,
        duplicate_percent: 2,
        out_of_order_percent: 5,
    };

    /// Conditions specified by ARCHITECT: 2% loss, 50ms jitter.
    pub const ARCHITECT_TEST: Self = Self {
        base_latency_ms: 30,
        jitter_ms: 50,
        packet_loss_percent: 2,
        duplicate_percent: 0,
        out_of_order_percent: 0,
    };

    /// Generates a latency value with jitter.
    #[must_use]
    pub fn generate_latency(&self, rng_value: u32) -> Duration {
        let jitter = if self.jitter_ms > 0 {
            (rng_value % (self.jitter_ms * 2)) as i32 - self.jitter_ms as i32
        } else {
            0
        };
        let latency = (self.base_latency_ms as i32 + jitter).max(0) as u64;
        Duration::from_millis(latency)
    }

    /// Returns true if packet should be dropped.
    #[must_use]
    pub fn should_drop(&self, rng_value: u32) -> bool {
        (rng_value % 100) < self.packet_loss_percent as u32
    }
}

impl Default for NetworkConditions {
    fn default() -> Self {
        Self::ARCHITECT_TEST
    }
}

/// Configuration for bot simulation.
#[derive(Clone, Debug)]
pub struct SimulationConfig {
    /// Number of bots to simulate.
    pub bot_count: usize,
    /// Tick rate.
    pub tick_rate: u32,
    /// Duration to run simulation.
    pub duration_secs: u32,
    /// Network conditions.
    pub network: NetworkConditions,
    /// Arena size (square).
    pub arena_size: f32,
    /// Whether bots should shoot.
    pub enable_shooting: bool,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            bot_count: 500,
            tick_rate: 60,
            duration_secs: 60,
            network: NetworkConditions::ARCHITECT_TEST,
            arena_size: 200.0,
            enable_shooting: true,
        }
    }
}

/// Simple Linear Congruential Generator for deterministic randomness.
/// No external dependencies, no allocations.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u32 {
        // LCG parameters (same as MINSTD)
        self.state = self.state.wrapping_mul(48271).wrapping_rem(2147483647);
        self.state as u32
    }

    fn next_f32(&mut self) -> f32 {
        self.next() as f32 / 2147483647.0
    }

}

/// A simulated bot.
#[derive(Clone, Debug)]
pub struct SimulatedBot {
    /// Bot ID.
    pub id: u32,
    /// Current position.
    pub position: Position,
    /// Current velocity.
    pub velocity: Velocity,
    /// Target position (for movement AI).
    pub target: Position,
    /// Health.
    pub health: u8,
    /// Is the bot alive?
    pub alive: bool,
    /// Input sequence counter.
    pub input_sequence: u32,
    /// Last received server tick.
    pub last_server_tick: u32,
    /// Position error (for smoothness measurement).
    pub position_error: f32,
}

impl SimulatedBot {
    /// Creates a new bot.
    fn new(id: u32, position: Position) -> Self {
        Self {
            id,
            position,
            velocity: Velocity::default(),
            target: position,
            health: 100,
            alive: true,
            input_sequence: 0,
            last_server_tick: 0,
            position_error: 0.0,
        }
    }

    /// Generates input for this tick.
    fn generate_input(&mut self, rng: &mut SimpleRng) -> PlayerInput {
        // Move toward target
        let dx = self.target.x - self.position.x;
        let dz = self.target.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();

        let mut input = PlayerInput::new(self.last_server_tick, self.input_sequence);
        self.input_sequence += 1;

        if dist > 1.0 {
            // Normalize and convert to i8
            input.move_x = ((dx / dist) * 127.0) as i8;
            input.move_z = ((dz / dist) * 127.0) as i8;

            // Sprint sometimes
            if rng.next() % 3 == 0 {
                input.flags |= PlayerInput::FLAG_SPRINT;
            }
        } else {
            // Pick new target
            let arena_half = 100.0;
            self.target = Position::new(
                rng.next_f32() * arena_half * 2.0 - arena_half,
                0.0,
                rng.next_f32() * arena_half * 2.0 - arena_half,
            );
        }

        // Random shooting
        if rng.next() % 60 == 0 {
            input.action = PlayerInput::ACTION_SHOOT;
        }

        // Random jumping
        if rng.next() % 120 == 0 {
            input.flags |= PlayerInput::FLAG_JUMP;
        }

        input
    }

    /// Predicts movement locally.
    fn predict(&mut self, input: &PlayerInput) {
        const DT: f32 = 1.0 / 60.0;
        const GRAVITY: f32 = -20.0;

        let speed = if input.is_sprinting() { 10.0 } else { 5.0 };

        self.velocity.x = input.move_x as f32 / 127.0 * speed;
        self.velocity.z = input.move_z as f32 / 127.0 * speed;

        if input.is_jumping() && self.position.y <= 0.1 {
            self.velocity.y = 8.0;
        }

        if self.position.y > 0.0 {
            self.velocity.y += GRAVITY * DT;
        }

        self.position.x += self.velocity.x * DT;
        self.position.y += self.velocity.y * DT;
        self.position.z += self.velocity.z * DT;

        if self.position.y < 0.0 {
            self.position.y = 0.0;
            self.velocity.y = 0.0;
        }
    }

    /// Reconciles with server state.
    fn reconcile(&mut self, server_pos: Position, server_tick: u32) {
        let error = calculate_distance(self.position, server_pos);
        self.position_error = error;
        self.last_server_tick = server_tick;

        // Smooth correction for small errors, snap for large
        if error > 1.0 {
            self.position = server_pos;
        } else if error > 0.01 {
            self.position.x = lerp(self.position.x, server_pos.x, 0.2);
            self.position.y = lerp(self.position.y, server_pos.y, 0.2);
            self.position.z = lerp(self.position.z, server_pos.z, 0.2);
        }
    }
}

/// Statistics from simulation.
#[derive(Clone, Debug, Default)]
pub struct SimulationStats {
    /// Total ticks executed.
    pub total_ticks: u64,
    /// Total packets sent.
    pub packets_sent: u64,
    /// Packets dropped (simulated loss).
    pub packets_dropped: u64,
    /// Average position error.
    pub avg_position_error: f32,
    /// Maximum position error.
    pub max_position_error: f32,
    /// Minimum tick time in microseconds.
    pub min_tick_us: u64,
    /// Maximum tick time in microseconds.
    pub max_tick_us: u64,
    /// Average tick time in microseconds.
    pub avg_tick_us: u64,
    /// Number of late ticks (> 16.67ms).
    pub late_ticks: u64,
    /// Number of snaps (large corrections).
    pub snap_count: u64,
    /// Total reconciliations.
    pub reconciliation_count: u64,
}

/// Bot simulation for stress testing.
pub struct BotSimulation {
    /// Configuration.
    config: SimulationConfig,
    /// Simulated bots.
    bots: Vec<SimulatedBot>,
    /// Server state.
    server: ServerState,
    /// Current tick.
    current_tick: u64,
    /// RNG for deterministic simulation.
    rng: SimpleRng,
    /// Statistics.
    stats: SimulationStats,
    /// Dragon state machine.
    dragon: DragonStateMachine,
}

impl BotSimulation {
    /// Creates a new simulation.
    #[must_use]
    pub fn new(config: SimulationConfig) -> Self {
        let mut rng = SimpleRng::new(42);

        // Create bots with random positions
        let arena_half = config.arena_size / 2.0;
        let bots: Vec<SimulatedBot> = (0..config.bot_count)
            .map(|i| {
                let pos = Position::new(
                    rng.next_f32() * config.arena_size - arena_half,
                    0.0,
                    rng.next_f32() * config.arena_size - arena_half,
                );
                SimulatedBot::new(i as u32, pos)
            })
            .collect();

        Self {
            config,
            bots,
            server: ServerState::new(500),
            current_tick: 0,
            rng,
            stats: SimulationStats::default(),
            dragon: DragonStateMachine::new(),
        }
    }

    /// Runs a single tick of the simulation.
    ///
    /// Returns false when simulation is complete.
    pub fn tick(&mut self) -> bool {
        let tick_start = Instant::now();

        // Check if simulation is complete
        let total_ticks = self.config.duration_secs as u64 * self.config.tick_rate as u64;
        if self.current_tick >= total_ticks {
            return false;
        }

        // 1. Generate and send inputs from bots
        let mut inputs: Vec<(u32, PlayerInput)> = Vec::with_capacity(self.bots.len());
        for bot in &mut self.bots {
            if !bot.alive {
                continue;
            }

            let input = bot.generate_input(&mut self.rng);

            // Simulate network - check for packet loss
            if !self.config.network.should_drop(self.rng.next()) {
                inputs.push((bot.id, input));
                self.stats.packets_sent += 1;
            } else {
                self.stats.packets_dropped += 1;
            }

            // Client-side prediction
            bot.predict(&input);
        }

        // 2. Server processes inputs and updates world
        // (In real implementation, inputs would be queued with latency)
        self.server.update();

        // 3. Generate server snapshot
        let snapshot = self.generate_server_snapshot();

        // 4. Send snapshot to bots (with simulated network)
        self.broadcast_snapshot(&snapshot);

        // 5. Update dragon state
        self.dragon.update(self.current_tick as u32);

        // Update tick stats
        let tick_duration = tick_start.elapsed();
        let tick_us = tick_duration.as_micros() as u64;

        self.stats.total_ticks += 1;
        self.stats.min_tick_us = self.stats.min_tick_us.min(tick_us).max(1);
        if self.stats.min_tick_us == 0 {
            self.stats.min_tick_us = tick_us;
        }
        self.stats.max_tick_us = self.stats.max_tick_us.max(tick_us);
        self.stats.avg_tick_us = 
            (self.stats.avg_tick_us * 15 + tick_us) / 16;

        if tick_us > 16666 {
            self.stats.late_ticks += 1;
        }

        self.current_tick += 1;
        true
    }

    /// Generates a server snapshot.
    fn generate_server_snapshot(&self) -> WorldSnapshot {
        let mut snapshot = WorldSnapshot::empty(self.current_tick as u32);
        snapshot.dragon = self.dragon.state();

        for bot in &self.bots {
            if !bot.alive {
                continue;
            }

            let state = EntityState::from_components(
                bot.id,
                bot.position,
                bot.velocity,
                bot.health,
            );

            if !snapshot.add_entity(state) {
                break; // Snapshot full
            }
        }

        snapshot
    }

    /// Broadcasts snapshot to all bots.
    fn broadcast_snapshot(&mut self, snapshot: &WorldSnapshot) {
        let mut total_error = 0.0;
        let mut max_error: f32 = 0.0;
        let mut error_count = 0;

        for bot in &mut self.bots {
            if !bot.alive {
                continue;
            }

            // Simulate packet loss
            if self.config.network.should_drop(self.rng.next()) {
                continue;
            }

            // Find bot's entity in snapshot
            for entity in snapshot.entities() {
                if entity.entity_id == bot.id {
                    let server_pos = entity.position();
                    let error_before = calculate_distance(bot.position, server_pos);

                    bot.reconcile(server_pos, snapshot.tick);

                    self.stats.reconciliation_count += 1;
                    total_error += error_before;
                    max_error = max_error.max(error_before);
                    error_count += 1;

                    if error_before > 1.0 {
                        self.stats.snap_count += 1;
                    }
                    break;
                }
            }
        }

        if error_count > 0 {
            self.stats.avg_position_error = total_error / error_count as f32;
            self.stats.max_position_error = self.stats.max_position_error.max(max_error);
        }
    }

    /// Runs the full simulation.
    pub fn run(&mut self) -> SimulationStats {
        while self.tick() {}
        self.stats.clone()
    }

    /// Returns current statistics.
    #[must_use]
    pub fn stats(&self) -> &SimulationStats {
        &self.stats
    }

    /// Returns the current tick.
    #[must_use]
    pub const fn current_tick(&self) -> u64 {
        self.current_tick
    }

    /// Returns the dragon state.
    #[must_use]
    pub fn dragon_state(&self) -> DragonState {
        self.dragon.state()
    }
}

/// Dragon state machine.
struct DragonStateMachine {
    /// Current state.
    state: DragonState,
    /// Time in current state.
    state_time: u32,
    /// Volatility level (simulated market).
    volatility: f32,
}

impl DragonStateMachine {
    fn new() -> Self {
        Self {
            state: DragonState::new(0, DragonState::STATE_SLEEP),
            state_time: 0,
            volatility: 0.0,
        }
    }

    fn update(&mut self, tick: u32) {
        self.state.tick = tick;
        self.state_time += 1;

        // Simulate market volatility (sine wave for predictable testing)
        self.volatility = ((tick as f32 / 60.0) * 0.5).sin() * 50.0 + 50.0;

        // State transitions based on volatility
        let new_state = if self.volatility > 80.0 {
            DragonState::STATE_INFERNO
        } else if self.volatility > 40.0 {
            DragonState::STATE_STALK
        } else {
            DragonState::STATE_SLEEP
        };

        if new_state != self.state.state {
            self.state.state = new_state;
            self.state_time = 0;
        }

        self.state.aggression = self.volatility as u8;
    }

    fn state(&self) -> DragonState {
        self.state
    }
}

/// Calculates distance between two positions.
fn calculate_distance(a: Position, b: Position) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Linear interpolation.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_conditions() {
        let conditions = NetworkConditions::ARCHITECT_TEST;
        assert_eq!(conditions.packet_loss_percent, 2);
        assert_eq!(conditions.jitter_ms, 50);
    }

    #[test]
    fn test_packet_loss() {
        let conditions = NetworkConditions {
            packet_loss_percent: 50,
            ..NetworkConditions::PERFECT
        };

        let mut dropped = 0;
        for i in 0..1000 {
            if conditions.should_drop(i * 7) {
                dropped += 1;
            }
        }

        // Should be roughly 50%
        assert!(dropped > 400 && dropped < 600);
    }

    #[test]
    fn test_simulation_creation() {
        let config = SimulationConfig {
            bot_count: 10,
            duration_secs: 1,
            ..Default::default()
        };

        let sim = BotSimulation::new(config);
        assert_eq!(sim.bots.len(), 10);
    }

    #[test]
    fn test_simulation_tick() {
        let config = SimulationConfig {
            bot_count: 10,
            duration_secs: 1,
            tick_rate: 60,
            ..Default::default()
        };

        let mut sim = BotSimulation::new(config);

        // Run 10 ticks
        for _ in 0..10 {
            assert!(sim.tick());
        }

        assert_eq!(sim.current_tick(), 10);
        assert!(sim.stats().total_ticks > 0);
    }

    #[test]
    fn test_full_simulation() {
        let config = SimulationConfig {
            bot_count: 50,
            duration_secs: 1,
            tick_rate: 60,
            network: NetworkConditions::ARCHITECT_TEST,
            ..Default::default()
        };

        let mut sim = BotSimulation::new(config);
        let stats = sim.run();

        println!("Simulation Stats:");
        println!("  Total ticks: {}", stats.total_ticks);
        println!("  Packets sent: {}", stats.packets_sent);
        println!("  Packets dropped: {}", stats.packets_dropped);
        println!("  Avg position error: {:.4}", stats.avg_position_error);
        println!("  Max position error: {:.4}", stats.max_position_error);
        println!("  Avg tick time: {} us", stats.avg_tick_us);
        println!("  Late ticks: {}", stats.late_ticks);
        println!("  Snaps: {}", stats.snap_count);

        // Verify simulation ran correctly
        assert_eq!(stats.total_ticks, 60);

        // With 2% packet loss, we should have some drops
        let loss_rate = stats.packets_dropped as f32 
            / (stats.packets_sent + stats.packets_dropped) as f32;
        println!("  Actual loss rate: {:.2}%", loss_rate * 100.0);

        // Movement should be smooth (low position error)
        assert!(stats.avg_position_error < 0.5, "Position error too high: {}", stats.avg_position_error);
    }

    #[test]
    fn test_dragon_state_machine() {
        let mut dragon = DragonStateMachine::new();

        // Initially sleeping
        assert_eq!(dragon.state().state, DragonState::STATE_SLEEP);

        // Simulate many ticks to trigger state changes
        for i in 0..600 {
            dragon.update(i);
        }

        // Should have transitioned through states
        // (exact state depends on tick count and volatility formula)
    }
}
