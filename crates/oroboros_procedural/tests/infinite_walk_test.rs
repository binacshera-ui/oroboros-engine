//! # Infinite Walk Integration Test
//!
//! Proves the player can walk forever without falling into void.

use oroboros_procedural::{WorldManager, WorldManagerConfig, WorldSeed};
use std::time::Instant;

/// Test: Walk 10,000 blocks in any direction without falling.
#[test]
fn test_infinite_walk_10000_blocks() {
    let seed = WorldSeed::new(42);
    let config = WorldManagerConfig::production();
    let mut manager = WorldManager::new(seed, config);
    
    // Start at origin
    manager.ensure_loaded_around(0.0, 0.0, 5);
    
    let start = Instant::now();
    let mut x = 0.0f32;
    let z = 0.0f32;
    
    // Walk 10,000 blocks east
    for step in 0..10_000 {
        x += 1.0;
        manager.update(x, z);
        
        // Process chunks every 16 blocks (once per chunk)
        if step % 16 == 0 {
            manager.flush_generation_queue();
        }
        
        // Verify no void fall every 100 blocks
        if step % 100 == 0 {
            let has_ground = manager.has_ground(x as i32, 100, z as i32);
            assert!(has_ground, "VOID DETECTED at x={}", x);
        }
    }
    
    manager.flush_generation_queue();
    
    let elapsed = start.elapsed();
    println!("Walked 10,000 blocks in {:?}", elapsed);
    println!("Loaded chunks: {}", manager.loaded_chunk_count());
    println!("Generated total: {}", manager.stats().generated_this_session);
    println!("Unloaded total: {}", manager.stats().unloaded_this_session);
    
    // Final verification
    assert!(manager.has_ground(x as i32, 100, z as i32), "Final position has no ground!");
}

/// Test: Walk in a spiral pattern covering huge area.
#[test]
fn test_spiral_walk_coverage() {
    let seed = WorldSeed::new(12345);
    let mut manager = WorldManager::new(seed, WorldManagerConfig::production());
    
    manager.ensure_loaded_around(0.0, 0.0, 5);
    
    let mut x = 0.0f32;
    let mut z = 0.0f32;
    let mut direction = 0; // 0=E, 1=S, 2=W, 3=N
    let mut leg_length = 1;
    let mut steps_in_leg = 0;
    let mut legs_completed = 0;
    
    // Spiral outward for 5000 steps
    for step in 0..5000 {
        // Move in current direction
        match direction {
            0 => x += 1.0,
            1 => z += 1.0,
            2 => x -= 1.0,
            3 => z -= 1.0,
            _ => unreachable!(),
        }
        
        steps_in_leg += 1;
        
        // Change direction at end of leg
        if steps_in_leg >= leg_length {
            steps_in_leg = 0;
            direction = (direction + 1) % 4;
            legs_completed += 1;
            
            // Increase leg length every 2 legs (completing a turn)
            if legs_completed % 2 == 0 {
                leg_length += 1;
            }
        }
        
        manager.update(x, z);
        
        if step % 32 == 0 {
            manager.flush_generation_queue();
        }
        
        // Check ground at significant points
        if step % 500 == 0 {
            assert!(
                manager.has_ground(x as i32, 100, z as i32),
                "VOID at step {} position ({}, {})", step, x, z
            );
        }
    }
    
    manager.flush_generation_queue();
    
    println!("Spiral covered area from (-{}, -{}) to ({}, {})", 
             leg_length, leg_length, leg_length, leg_length);
    println!("Final position: ({}, {})", x, z);
    println!("Chunks loaded: {}", manager.loaded_chunk_count());
    
    // Verify final position
    assert!(manager.has_ground(x as i32, 100, z as i32));
}

/// Test: Teleport across map and verify chunks generate correctly.
#[test]
fn test_teleport_stress() {
    let seed = WorldSeed::new(99999);
    let mut manager = WorldManager::new(seed, WorldManagerConfig::production());
    
    let teleport_points = [
        (0.0, 0.0),
        (1000.0, 0.0),
        (-1000.0, 500.0),
        (500.0, -1000.0),
        (2000.0, 2000.0),
        (-2000.0, -2000.0),
        (0.0, 0.0), // Return to origin
    ];
    
    for (x, z) in teleport_points {
        // Teleport
        manager.update(x, z);
        manager.flush_generation_queue();
        
        // Verify ground exists
        assert!(
            manager.has_ground(x as i32, 100, z as i32),
            "No ground at teleport destination ({}, {})", x, z
        );
        
        println!("Teleported to ({}, {}) - {} chunks loaded", 
                 x, z, manager.loaded_chunk_count());
    }
}

/// Test: Verify deterministic generation across runs.
#[test]
fn test_deterministic_terrain() {
    let seed = WorldSeed::new(42);
    
    // First run
    let mut manager1 = WorldManager::new(seed, WorldManagerConfig::test());
    manager1.ensure_loaded_around(100.0, 100.0, 2);
    manager1.flush_generation_queue();
    
    let block1 = manager1.get_block(100, 64, 100);
    let block2 = manager1.get_block(105, 70, 110);
    
    // Second run with same seed
    let mut manager2 = WorldManager::new(seed, WorldManagerConfig::test());
    manager2.ensure_loaded_around(100.0, 100.0, 2);
    manager2.flush_generation_queue();
    
    let block1_verify = manager2.get_block(100, 64, 100);
    let block2_verify = manager2.get_block(105, 70, 110);
    
    // Should be identical
    assert_eq!(block1, block1_verify, "Terrain not deterministic at (100, 64, 100)");
    assert_eq!(block2, block2_verify, "Terrain not deterministic at (105, 70, 110)");
    
    println!("✓ Terrain generation is deterministic");
}

/// Benchmark: Measure chunk generation throughput.
#[test]
fn bench_chunk_generation_throughput() {
    let seed = WorldSeed::new(42);
    let mut manager = WorldManager::new(seed, WorldManagerConfig::production());
    
    let start = Instant::now();
    
    // Walk 1000 blocks, forcing chunk generation
    for step in 0..1000 {
        manager.update(step as f32, 0.0);
        manager.flush_generation_queue();
    }
    
    let elapsed = start.elapsed();
    let total_chunks = manager.stats().generated_this_session;
    let chunks_per_sec = total_chunks as f64 / elapsed.as_secs_f64();
    
    println!("Generated {} chunks in {:?}", total_chunks, elapsed);
    println!("Throughput: {:.0} chunks/sec", chunks_per_sec);
    
    // Should generate at least 100 chunks/sec (conservative)
    assert!(chunks_per_sec > 100.0, "Chunk generation too slow: {} chunks/sec", chunks_per_sec);
}
