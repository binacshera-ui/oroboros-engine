//! # Golden Path Integration Test
//!
//! THE ARCHITECT'S CHALLENGE:
//!
//! Click → Break Block → Server Validates → Economy Calculates →
//! Inventory Updates → Particles Spawn → UI Shows "+1 Diamond"
//!
//! ALL IN < 50ms.
//!
//! This test simulates the complete data flow without actual networking
//! or rendering, measuring the time from input to output.

use std::time::Instant;

use oroboros::{
    core::{Position, Velocity, EntityId, DoubleBufferedWorld},
    economy::{EconomySystem, LootTable, Rarity, BlockchainSalt},
    events::{EventBus, GameEvent},
};
use oroboros_economy::loot::LootEntry;

/// Results from the Golden Path test.
#[derive(Debug)]
#[allow(dead_code)]
struct GoldenPathResult {
    /// Total time in microseconds.
    total_us: u64,
    /// Time for server validation.
    validation_us: u64,
    /// Time for economy calculation.
    economy_us: u64,
    /// Time for inventory update.
    inventory_us: u64,
    /// Time for event dispatch.
    event_us: u64,
    /// Whether a diamond was dropped.
    got_diamond: bool,
    /// Item that dropped (if any).
    dropped_item: Option<u32>,
    /// Quantity dropped.
    quantity: u32,
}

/// Simulates the complete "break diamond ore" scenario.
fn run_golden_path_iteration(
    _world: &DoubleBufferedWorld,
    economy: &mut EconomySystem,
    event_bus: &EventBus,
    player_id: u64,
    nonce: u32,
) -> GoldenPathResult {
    let total_start = Instant::now();

    // =========================================================================
    // STEP 1: Server Validation (Unit 4)
    // =========================================================================
    // In the real game, this would check:
    // - Is the player close enough to the block?
    // - Does the player have the right tool?
    // - Is the block breakable?
    // For this test, we simulate with a simple check.
    let validation_start = Instant::now();

    let player_pos = Position::new(10.0, 64.0, 10.0);
    let block_pos = [10, 64, 11]; // 1 block away

    // Validate distance (must be within 5 blocks)
    let dx = (player_pos.x - block_pos[0] as f32).abs();
    let dy = (player_pos.y - block_pos[1] as f32).abs();
    let dz = (player_pos.z - block_pos[2] as f32).abs();
    let _valid = dx < 5.0 && dy < 5.0 && dz < 5.0;

    let validation_us = validation_start.elapsed().as_micros() as u64;

    // =========================================================================
    // STEP 2: Economy Calculation (Unit 3)
    // =========================================================================
    let economy_start = Instant::now();

    // Diamond ore block = ID 56 (Minecraft convention)
    let block_id = 56u32;
    let player_level = 50u8;
    let pickaxe_tier = 4u8; // Diamond pickaxe
    let weather_seed = 12345u32;

    let result = economy.process_mining_hit(
        player_id,
        block_id,
        player_level,
        pickaxe_tier,
        weather_seed,
        nonce,
    );

    let economy_us = economy_start.elapsed().as_micros() as u64;

    // =========================================================================
    // STEP 3: Inventory Update (Unit 1 via Unit 3)
    // =========================================================================
    let inventory_start = Instant::now();

    // The economy system already updated the inventory
    // Here we'd write to the ECS, but for this test we just measure the time
    let inventory = economy.get_inventory(player_id);
    let _diamond_count = inventory.map(|inv| inv.count_item(264)).unwrap_or(0);

    let inventory_us = inventory_start.elapsed().as_micros() as u64;

    // =========================================================================
    // STEP 4: Event Dispatch (Unit 4 → Unit 2)
    // =========================================================================
    let event_start = Instant::now();

    let sender = event_bus.sender();
    let got_diamond;
    let mut dropped_item = None;
    let mut quantity = 0;

    if let Ok(tx_result) = result {
        if let Some(loot) = &tx_result.loot_drop {
            got_diamond = loot.item_id == 264; // Diamond item ID
            dropped_item = Some(loot.item_id);
            quantity = loot.quantity;

            // Send visual event to rendering
            let _ = sender.send(GameEvent::BlockBroken {
                entity_id: EntityId::new(player_id as u32, 0),
                block_pos,
                block_type: block_id,
                tool_tier: pickaxe_tier,
            });

            // Send loot event
            let _ = sender.send(GameEvent::LootDropped {
                entity_id: EntityId::new(player_id as u32, 0),
                position: [block_pos[0] as f32, block_pos[1] as f32, block_pos[2] as f32],
                item_id: loot.item_id,
                quantity: loot.quantity,
                rarity: loot.rarity as u8,
            });
        } else {
            got_diamond = false;
        }
    } else {
        got_diamond = false;
    }

    let event_us = event_start.elapsed().as_micros() as u64;

    // =========================================================================
    // TOTAL TIME
    // =========================================================================
    let total_us = total_start.elapsed().as_micros() as u64;

    GoldenPathResult {
        total_us,
        validation_us,
        economy_us,
        inventory_us,
        event_us,
        got_diamond,
        dropped_item,
        quantity,
    }
}

/// Creates a test economy system with diamond ore loot table.
fn create_test_economy() -> EconomySystem {
    let path = std::env::temp_dir().join(format!(
        "golden_path_test_{}.wal",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let mut economy = EconomySystem::new(&path).expect("Failed to create economy system");

    // Diamond ore loot table (block ID 56)
    let diamond_table = LootTable {
        block_id: 56,
        block_rarity: Rarity::Rare,
        entries: vec![
            LootEntry {
                item_id: 264, // Diamond
                weight: 100,
                min_quantity: 1,
                max_quantity: 3,
                rarity: Rarity::Rare,
                min_level: 0,
                min_pickaxe_tier: 4, // Need diamond pickaxe
            },
        ],
        total_weight: 0,
    };

    economy.register_loot_table(diamond_table);
    economy.update_blockchain_salt(BlockchainSalt::test_salt());
    economy.set_max_stack(264, 64); // Diamonds stack to 64

    economy
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║           GOLDEN PATH INTEGRATION TEST                           ║");
    println!("║           Click → Diamond → Particles                            ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  TARGET: < 50ms from click to visual feedback                    ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    // Create systems
    let world = DoubleBufferedWorld::new(1000, 1000);
    let mut economy = create_test_economy();
    let event_bus = EventBus::new(1024);
    let receiver = event_bus.receiver();

    // Spawn a player entity
    {
        let mut write = world.write_handle();
        let _ = write.spawn_pv(
            Position::new(10.0, 64.0, 10.0),
            Velocity::new(0.0, 0.0, 0.0),
        );
    }

    // Warm up
    println!("Warming up...");
    for i in 0..100 {
        let _ = run_golden_path_iteration(&world, &mut economy, &event_bus, 1, i);
        let _ = receiver.drain(); // Clear events
    }

    // Run test
    let iterations = 1000;
    let mut results = Vec::with_capacity(iterations);
    let mut diamonds_found = 0u32;

    println!("Running {} iterations...", iterations);
    let test_start = Instant::now();

    for i in 0..iterations {
        let result = run_golden_path_iteration(&world, &mut economy, &event_bus, 1, (i + 100) as u32);

        if result.got_diamond {
            diamonds_found += 1;
        }
        results.push(result);

        // Drain events (simulates render consuming them)
        let _ = receiver.drain();
    }

    let test_duration = test_start.elapsed();

    // Calculate statistics
    let total_us: Vec<u64> = results.iter().map(|r| r.total_us).collect();
    let avg_total = total_us.iter().sum::<u64>() / iterations as u64;
    let min_total = *total_us.iter().min().unwrap();
    let max_total = *total_us.iter().max().unwrap();

    let avg_validation = results.iter().map(|r| r.validation_us).sum::<u64>() / iterations as u64;
    let avg_economy = results.iter().map(|r| r.economy_us).sum::<u64>() / iterations as u64;
    let avg_inventory = results.iter().map(|r| r.inventory_us).sum::<u64>() / iterations as u64;
    let avg_event = results.iter().map(|r| r.event_us).sum::<u64>() / iterations as u64;

    // Check if we met the target
    let target_us = 50_000u64; // 50ms
    let requirement_met = max_total < target_us;

    // Print results
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                    GOLDEN PATH RESULTS                           ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("┌─ THROUGHPUT ───────────────────────────────────────────────────┐");
    println!("│ Test Duration:      {:.2}s                                      ", test_duration.as_secs_f64());
    println!("│ Iterations:         {}                                        ", iterations);
    println!("│ Operations/sec:     {:.0}                                      ", iterations as f64 / test_duration.as_secs_f64());
    println!("│ Diamonds Found:     {} ({:.1}% drop rate)                      ", 
        diamonds_found, 
        diamonds_found as f64 / iterations as f64 * 100.0);
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─ LATENCY (THE CRITICAL METRIC) ─────────────────────────────────┐");
    println!("│                                                                  │");
    println!("│ Average Total:      {:.3} ms                                    ", avg_total as f64 / 1000.0);
    println!("│ Minimum Total:      {:.3} ms                                    ", min_total as f64 / 1000.0);
    println!("│ Maximum Total:      {:.3} ms                                    ", max_total as f64 / 1000.0);
    println!("│                                                                  │");

    if requirement_met {
        println!("│ ✓ REQUIREMENT MET: Max {:.3}ms < 50ms target                  ", max_total as f64 / 1000.0);
    } else {
        println!("│ ✗ REQUIREMENT FAILED: Max {:.3}ms > 50ms target               ", max_total as f64 / 1000.0);
    }
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─ BREAKDOWN ─────────────────────────────────────────────────────┐");
    println!("│                                                                  │");
    println!("│ Server Validation:  {:.3} ms (Unit 4)                           ", avg_validation as f64 / 1000.0);
    println!("│ Economy (Loot+WAL): {:.3} ms (Unit 3)                           ", avg_economy as f64 / 1000.0);
    println!("│ Inventory Update:   {:.3} ms (Unit 1)                           ", avg_inventory as f64 / 1000.0);
    println!("│ Event Dispatch:     {:.3} ms (→ Unit 2)                         ", avg_event as f64 / 1000.0);
    println!("│                                                                  │");
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    // Final inventory check
    if let Some(inv) = economy.get_inventory(1) {
        let total_diamonds = inv.count_item(264);
        println!("┌─ FINAL STATE ────────────────────────────────────────────────────┐");
        println!("│ Total Diamonds in Inventory: {}                               ", total_diamonds);
        println!("└──────────────────────────────────────────────────────────────────┘");
    }

    // Exit with appropriate code
    if requirement_met {
        println!();
        println!("✅ GOLDEN PATH TEST PASSED");
        std::process::exit(0);
    } else {
        println!();
        println!("❌ GOLDEN PATH TEST FAILED");
        std::process::exit(1);
    }
}
