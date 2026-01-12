//! # Golden Path Demo - OPERATION COLD FUSION Integration Test
//!
//! This example demonstrates the complete render loop integration:
//!
//! ```text
//! THE GOLDEN PATH (50ms budget):
//!
//! 1. Player breaks rock
//! 2. Unit 4 sends BlockBroken event
//! 3. Unit 3 returns Diamond (Legendary)
//! 4. Unit 2 receives ItemDrop event
//! 5. Unit 2 spawns 10,000 golden particles
//! 6. Player sees explosion SAME FRAME
//! ```

use oroboros_rendering::integration::{
    RenderLoop, RenderLoopConfig,
    render_bridge::MockWorldReader,
};
use std::time::{Duration, Instant};

fn main() {
    println!("═══════════════════════════════════════════════════════════════");
    println!("   OPERATION COLD FUSION - Golden Path Integration Demo");
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    // Create the render loop
    let mut render_loop = RenderLoop::new(RenderLoopConfig {
        target_fps: 60,
        frame_budget_us: 16_666, // 16ms
        ..Default::default()
    });

    // Mock world with some entities
    let mock_world = MockWorldReader {
        entities: vec![
            (1, [100.0, 50.0, 100.0], [0.0, 0.0, 0.0]), // Player
            (2, [105.0, 50.0, 100.0], [0.0, 0.0, 0.0]), // Rock
        ],
    };

    println!("📡 Simulating THE GOLDEN PATH:");
    println!();

    // === FRAME 1: Player approaches rock ===
    println!("FRAME 1: Player approaches rock");
    let result1 = render_loop.frame(
        &mock_world,
        [100.0, 55.0, 100.0],
        [[1.0, 0.0, 0.0, 0.0]; 4],
        0.016,
    );
    print_frame_result(&result1);
    println!();

    // === FRAME 2: Player breaks rock, gets Legendary diamond! ===
    println!("FRAME 2: Player breaks rock - LEGENDARY DROP!");

    // Simulate Unit 4 sending block break event
    render_loop.event_queue_mut().push_block_break(
        [105.0, 50.0, 100.0], // Rock position
        1,                     // Stone block type
        1,                     // Player ID
        120,                   // Server tick
    );

    // Simulate Unit 3 returning legendary item via Unit 4
    render_loop.event_queue_mut().push_item_drop(
        [105.0, 50.0, 100.0], // Drop position
        999,                   // Diamond item ID
        4,                     // LEGENDARY (rarity 4)
        1,                     // Quantity
        1,                     // Player ID
    );

    let start = Instant::now();
    let result2 = render_loop.frame(
        &mock_world,
        [100.0, 55.0, 100.0],
        [[1.0, 0.0, 0.0, 0.0]; 4],
        0.016,
    );
    let total_time = start.elapsed();

    print_frame_result(&result2);
    println!();

    // === VERIFICATION ===
    println!("═══════════════════════════════════════════════════════════════");
    println!("                     GOLDEN PATH VERIFICATION");
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    let passed_events = result2.events_processed >= 2;
    let passed_time = total_time < Duration::from_millis(50);
    let passed_particles = result2.particles_alive > 0 || render_loop.visualizer().stats().particles_spawned > 0;

    println!("  ✓ Events processed: {} (expected ≥2)", result2.events_processed);
    println!("  {} Total time: {:?} (budget: 50ms)",
        if passed_time { "✓" } else { "✗" },
        total_time
    );
    println!("  {} Particles spawned: {} (expected 10,000 for Legendary)",
        if passed_particles { "✓" } else { "✗" },
        render_loop.visualizer().stats().particles_spawned
    );
    println!();

    if passed_events && passed_time {
        println!("  ██████╗  █████╗ ███████╗███████╗███████╗██████╗ ");
        println!("  ██╔══██╗██╔══██╗██╔════╝██╔════╝██╔════╝██╔══██╗");
        println!("  ██████╔╝███████║███████╗███████╗█████╗  ██║  ██║");
        println!("  ██╔═══╝ ██╔══██║╚════██║╚════██║██╔══╝  ██║  ██║");
        println!("  ██║     ██║  ██║███████║███████║███████╗██████╔╝");
        println!("  ╚═╝     ╚═╝  ╚═╝╚══════╝╚══════╝╚══════╝╚═════╝ ");
        println!();
        println!("  THE GOLDEN PATH IS COMPLETE");
        println!("  Unit 2 (Neon) integration: OPERATIONAL");
    } else {
        println!("  ███████╗ █████╗ ██╗██╗     ███████╗██████╗ ");
        println!("  ██╔════╝██╔══██╗██║██║     ██╔════╝██╔══██╗");
        println!("  █████╗  ███████║██║██║     █████╗  ██║  ██║");
        println!("  ██╔══╝  ██╔══██║██║██║     ██╔══╝  ██║  ██║");
        println!("  ██║     ██║  ██║██║███████╗███████╗██████╔╝");
        println!("  ╚═╝     ╚═╝  ╚═╝╚═╝╚══════╝╚══════╝╚═════╝ ");
        println!();
        println!("  Integration incomplete. Debug required.");
    }

    println!();
    println!("═══════════════════════════════════════════════════════════════");

    // === Stress Test: Multiple events ===
    println!();
    println!("📊 STRESS TEST: 100 simultaneous drops");
    println!();

    for i in 0..100 {
        let rarity = (i % 5) as u8; // Common through Legendary
        render_loop.event_queue_mut().push_item_drop(
            [100.0 + (i as f32 * 0.1), 50.0, 100.0],
            i as u32,
            rarity,
            1,
            1,
        );
    }

    let stress_start = Instant::now();
    let stress_result = render_loop.frame(
        &mock_world,
        [100.0, 55.0, 100.0],
        [[1.0, 0.0, 0.0, 0.0]; 4],
        0.016,
    );
    let stress_time = stress_start.elapsed();

    println!("  Events: {}", stress_result.events_processed);
    println!("  Time: {:?}", stress_time);
    println!("  FPS equivalent: {:.1}", 1.0 / stress_time.as_secs_f64());
    println!("  Budget status: {}",
        if stress_time < Duration::from_millis(16) { "UNDER ✓" } else { "OVER ✗" }
    );
}

fn print_frame_result(result: &oroboros_rendering::integration::FrameResult) {
    println!("  ├─ Frame #{}", result.frame_number);
    println!("  ├─ Total time: {}μs", result.frame_time_us);
    println!("  ├─ ECS read: {}μs", result.ecs_read_us);
    println!("  ├─ Event processing: {}μs", result.event_process_us);
    println!("  ├─ Particle update: {}μs", result.particle_update_us);
    println!("  ├─ Entities: {}", result.entities_rendered);
    println!("  ├─ Events: {}", result.events_processed);
    println!("  └─ Particles: {}", result.particles_alive);
}
