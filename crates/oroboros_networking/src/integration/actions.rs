//! # Player Actions
//!
//! Implementation of player actions with timing measurements.

use std::time::Instant;

use oroboros_core::Position;

use crate::integration::events::*;
use crate::integration::traits::*;
use crate::integration::game_loop::*;

/// Result of a block break action with timing.
#[derive(Clone, Debug)]
pub struct BlockBreakResult {
    /// Whether the break succeeded.
    pub success: bool,
    /// Loot dropped.
    pub loot: Vec<LootDrop>,
    /// Time from action to completion (microseconds).
    pub latency_us: u64,
    /// Breakdown of time spent in each step.
    pub timing: BlockBreakTiming,
}

/// Timing breakdown for block break action.
#[derive(Clone, Debug, Default)]
pub struct BlockBreakTiming {
    /// Time to validate block (microseconds).
    pub validate_us: u64,
    /// Time to call economy (microseconds).
    pub economy_us: u64,
    /// Time to update world (microseconds).
    pub world_update_us: u64,
    /// Time to update inventory (microseconds).
    pub inventory_us: u64,
    /// Time to generate events (microseconds).
    pub events_us: u64,
    /// Total time (microseconds).
    pub total_us: u64,
}

/// Golden Path test runner.
///
/// Tests the complete flow: Client → Server → Unit 3 → Unit 1 → Client
pub struct GoldenPathTest {
    /// Test results.
    results: Vec<GoldenPathResult>,
}

/// Result of one Golden Path test.
#[derive(Clone, Debug)]
pub struct GoldenPathResult {
    /// Test name.
    pub name: String,
    /// Whether it passed.
    pub passed: bool,
    /// Latency in microseconds.
    pub latency_us: u64,
    /// Target latency in microseconds.
    pub target_us: u64,
    /// Details.
    pub details: String,
}

impl GoldenPathTest {
    /// Creates a new test runner.
    pub fn new() -> Self {
        Self { results: Vec::new() }
    }
    
    /// Runs all Golden Path tests.
    pub fn run_all(&mut self) {
        self.test_block_break();
        self.test_movement();
        self.test_attack();
    }
    
    /// Tests the block break Golden Path.
    fn test_block_break(&mut self) {
        let start = Instant::now();
        
        // Setup
        let memory = MockMemoryOwner::new();
        let economy = MockEconomyAuditor::new();
        let visuals = MockVisualFeedback::new();
        
        // Create server
        let mut server = GameServer::new(
            ServerConfig::default(),
            memory,
            economy,
            visuals,
        );
        
        // Connect player
        let player_id = 1;
        let spawn_pos = Position::new(0.0, 0.0, 0.0);
        server.connect_player(player_id, spawn_pos);
        
        // Place a diamond block in the world (using test-only method)
        server.test_set_block((5, 5, 5), 1); // Block type 1 = diamond
        
        let setup_time = start.elapsed();
        
        // === THE GOLDEN PATH ===
        let action_start = Instant::now();
        
        // Step 1: Client sends BreakBlock action
        server.queue_action(player_id, PlayerAction::BreakBlock {
            sequence: 1,
            block_pos: (5, 5, 5),
        });
        
        // Step 2-5: Server processes (tick)
        let events = server.tick();
        
        let action_time = action_start.elapsed();
        
        // Verify results
        let mut passed = true;
        let mut details = String::new();
        
        // Check block was removed (using controlled getter)
        let block = server.get_block((5, 5, 5));
        if block != 0 {
            passed = false;
            details.push_str("Block not removed. ");
        }
        
        // Check event was generated
        let block_broken_event = events.iter().find(|(_, e)| matches!(e, GameEvent::BlockBroken { .. }));
        if block_broken_event.is_none() {
            passed = false;
            details.push_str("No BlockBroken event. ");
        }
        
        // Check latency
        let latency_us = action_time.as_micros() as u64;
        let target_us = 50_000; // 50ms target
        
        if latency_us > target_us {
            passed = false;
            details.push_str(&format!("Latency {} > {} target. ", latency_us, target_us));
        }
        
        if details.is_empty() {
            details = format!(
                "Block broken in {}μs (setup: {}μs)",
                latency_us,
                setup_time.as_micros()
            );
        }
        
        self.results.push(GoldenPathResult {
            name: "Block Break (Golden Path)".to_string(),
            passed,
            latency_us,
            target_us,
            details,
        });
    }
    
    /// Tests movement.
    fn test_movement(&mut self) {
        let _start = Instant::now();
        
        // Setup
        let memory = MockMemoryOwner::new();
        let economy = MockEconomyAuditor::new();
        let visuals = MockVisualFeedback::new();
        
        let mut server = GameServer::new(
            ServerConfig::default(),
            memory,
            economy,
            visuals,
        );
        
        let player_id = 1;
        let spawn_pos = Position::new(0.0, 0.0, 0.0);
        let entity_id = server.connect_player(player_id, spawn_pos);
        
        // Move
        let action_start = Instant::now();
        
        server.queue_action(player_id, PlayerAction::Move {
            sequence: 1,
            direction: (1.0, 0.0, 0.0),
            sprint: false,
        });
        
        server.tick();
        
        let action_time = action_start.elapsed();
        
        // Verify (using controlled getter)
        let new_pos = server.get_entity_position(entity_id).unwrap();
        let expected_x = 5.0 / 60.0; // speed * dt
        let passed = (new_pos.x - expected_x).abs() < 0.001;
        
        let latency_us = action_time.as_micros() as u64;
        
        self.results.push(GoldenPathResult {
            name: "Movement".to_string(),
            passed,
            latency_us,
            target_us: 50_000,
            details: format!("Moved to x={:.4} in {}μs", new_pos.x, latency_us),
        });
    }
    
    /// Tests attack.
    fn test_attack(&mut self) {
        let memory = MockMemoryOwner::new();
        let economy = MockEconomyAuditor::new();
        let visuals = MockVisualFeedback::new();
        
        let mut server = GameServer::new(
            ServerConfig::default(),
            memory,
            economy,
            visuals,
        );
        
        // Two players
        let attacker_id = 1;
        let defender_id = 2;
        
        let _attacker_entity = server.connect_player(attacker_id, Position::new(0.0, 0.0, 0.0));
        let defender_entity = server.connect_player(defender_id, Position::new(2.0, 0.0, 0.0));
        
        let action_start = Instant::now();
        
        // Attack
        server.queue_action(attacker_id, PlayerAction::Attack {
            sequence: 1,
            direction: (1.0, 0.0, 0.0),
            target: Some(defender_entity),
        });
        
        let events = server.tick();
        
        let action_time = action_start.elapsed();
        
        // Verify damage was dealt (using controlled getter)
        let defender_health = server.get_entity_health(defender_entity).unwrap_or(100);
        let passed = defender_health < 100;
        
        let latency_us = action_time.as_micros() as u64;
        
        self.results.push(GoldenPathResult {
            name: "Attack".to_string(),
            passed,
            latency_us,
            target_us: 50_000,
            details: format!(
                "Defender health: {} (events: {}) in {}μs",
                defender_health,
                events.len(),
                latency_us
            ),
        });
    }
    
    /// Prints test results.
    pub fn print_results(&self) {
        println!();
        println!("╔══════════════════════════════════════════════════════════════════╗");
        println!("║              GOLDEN PATH INTEGRATION TESTS                       ║");
        println!("╚══════════════════════════════════════════════════════════════════╝");
        println!();
        
        let mut all_passed = true;
        
        for result in &self.results {
            let status = if result.passed { "✓ PASS" } else { "✗ FAIL" };
            let latency_status = if result.latency_us <= result.target_us {
                format!("{}μs ≤ {}μs ✓", result.latency_us, result.target_us)
            } else {
                format!("{}μs > {}μs ✗", result.latency_us, result.target_us)
            };
            
            println!("┌─ {} ─────────────────────────────────────────┐", result.name);
            println!("│ Status:  {}                                           │", status);
            println!("│ Latency: {}                              │", latency_status);
            println!("│ Details: {}│", format!("{:<47}", result.details));
            println!("└──────────────────────────────────────────────────────────────────┘");
            println!();
            
            if !result.passed {
                all_passed = false;
            }
        }
        
        println!("╔══════════════════════════════════════════════════════════════════╗");
        if all_passed {
            println!("║  ✓ ALL TESTS PASSED - GOLDEN PATH VERIFIED                      ║");
        } else {
            println!("║  ✗ SOME TESTS FAILED - GOLDEN PATH BROKEN                       ║");
        }
        println!("╚══════════════════════════════════════════════════════════════════╝");
    }
    
    /// Returns true if all tests passed.
    pub fn all_passed(&self) -> bool {
        self.results.iter().all(|r| r.passed)
    }
}

impl Default for GoldenPathTest {
    fn default() -> Self {
        Self::new()
    }
}
