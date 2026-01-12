//! # COLD FUSION Integration - Full System Demo
//!
//! This demonstrates the REAL integration between Unit 2 and Unit 1
//! using the actual DoubleBufferedWorld from oroboros_core.
//!
//! THE GOLDEN PATH:
//! 1. Unit 1 creates DoubleBufferedWorld
//! 2. Unit 4 writes entity positions
//! 3. Unit 2 reads via CoreWorldReader
//! 4. Events trigger particles
//! 5. Screen explodes with gold

use oroboros_core::{DoubleBufferedWorld, Position, Velocity};
use oroboros_rendering::integration::{
    RenderLoop, RenderLoopConfig, CoreWorldReader,
};
use std::time::{Duration, Instant};

fn main() {
    println!("═══════════════════════════════════════════════════════════════════");
    println!("   OPERATION COLD FUSION - Full Integration Demo");
    println!("   Unit 1 (Core) ←→ Unit 2 (Neon) Integration");
    println!("═══════════════════════════════════════════════════════════════════");
    println!();

    // === UNIT 1: Create the Double Buffered World ===
    println!("🏗️  Unit 1 (Core): Creating DoubleBufferedWorld...");
    let db_world = DoubleBufferedWorld::new(10_000, 10_000);
    println!("   ✓ PV capacity: 10,000 entities");
    println!("   ✓ P capacity: 10,000 static objects");
    println!();

    // === UNIT 4 SIMULATION: Write entities to the world ===
    println!("🔥 Unit 4 (Inferno): Spawning entities...");
    {
        let mut write = db_world.write_handle();
        
        // Spawn player
        let player_id = write.spawn_pv(
            Position::new(100.0, 50.0, 100.0),
            Velocity::new(0.0, 0.0, 0.0),
        );
        println!("   ✓ Player spawned: {:?}", player_id);
        
        // Spawn some moving NPCs
        for i in 0..100 {
            let angle = (i as f32) * 0.0628; // Spread around
            let _ = write.spawn_pv(
                Position::new(
                    100.0 + angle.cos() * 50.0,
                    50.0,
                    100.0 + angle.sin() * 50.0,
                ),
                Velocity::new(angle.sin(), 0.0, angle.cos()),
            );
        }
        println!("   ✓ 100 NPCs spawned");
        
        // Update positions (simulate one tick)
        write.update_positions(0.016);
        println!("   ✓ Positions updated (delta=16ms)");
    }
    
    // Swap buffers to make data available to rendering
    db_world.swap_buffers();
    println!("   ✓ Buffers swapped (data ready for render)");
    println!();

    // === UNIT 2: Create Render Loop ===
    println!("🎨 Unit 2 (Neon): Initializing render loop...");
    let mut render_loop = RenderLoop::new(RenderLoopConfig::default());
    println!("   ✓ Render loop ready");
    println!();

    // === THE GOLDEN PATH ===
    println!("═══════════════════════════════════════════════════════════════════");
    println!("                      THE GOLDEN PATH");
    println!("═══════════════════════════════════════════════════════════════════");
    println!();

    // Simulate game events
    println!("📡 Simulating game events...");
    
    // Block break event
    render_loop.event_queue_mut().push_block_break(
        [105.0, 50.0, 100.0],
        1,   // Stone
        1,   // Player 1
        120, // Tick
    );
    println!("   ✓ BlockBreak event queued");
    
    // LEGENDARY item drop!
    render_loop.event_queue_mut().push_item_drop(
        [105.0, 50.0, 100.0],
        999, // Diamond
        4,   // LEGENDARY
        1,
        1,
    );
    println!("   ✓ ItemDrop (LEGENDARY) event queued");
    println!();

    // === RENDER FRAME ===
    println!("🖼️  Rendering frame with real Core data...");
    
    let frame_start = Instant::now();
    
    // Get read handle from Unit 1's double buffer
    let read_handle = db_world.read_handle();
    
    // Create adapter
    let reader = CoreWorldReader::new(&read_handle);
    
    // Execute frame
    let result = render_loop.frame(
        &reader,
        [100.0, 55.0, 100.0], // Camera
        [[1.0, 0.0, 0.0, 0.0]; 4], // View-proj (identity for demo)
        0.016, // Delta time
    );
    
    let frame_time = frame_start.elapsed();
    
    println!();
    println!("   Frame #{}: ", result.frame_number);
    println!("   ├─ Total time: {:?}", frame_time);
    println!("   ├─ ECS read: {}μs", result.ecs_read_us);
    println!("   ├─ Event processing: {}μs", result.event_process_us);
    println!("   ├─ Particle update: {}μs", result.particle_update_us);
    println!("   ├─ Entities read: {}", result.entities_rendered);
    println!("   ├─ Events processed: {}", result.events_processed);
    println!("   └─ Particles spawned: {}", render_loop.visualizer().stats().particles_spawned);
    println!();

    // === VERIFICATION ===
    println!("═══════════════════════════════════════════════════════════════════");
    println!("                      INTEGRATION VERIFICATION");
    println!("═══════════════════════════════════════════════════════════════════");
    println!();

    let passed_time = frame_time < Duration::from_millis(50);
    let passed_events = result.events_processed >= 2;
    let passed_entities = result.entities_rendered > 0;
    let passed_particles = render_loop.visualizer().stats().particles_spawned == 10_000;

    println!("  {} Frame time: {:?} (budget: 50ms)", 
        if passed_time { "✓" } else { "✗" }, frame_time);
    println!("  {} Events processed: {} (expected ≥2)", 
        if passed_events { "✓" } else { "✗" }, result.events_processed);
    println!("  {} Entities rendered: {} (expected >0)", 
        if passed_entities { "✓" } else { "✗" }, result.entities_rendered);
    println!("  {} Particles: {} (expected 10,000 for Legendary)", 
        if passed_particles { "✓" } else { "✗" }, render_loop.visualizer().stats().particles_spawned);
    println!();

    let all_passed = passed_time && passed_events && passed_entities && passed_particles;

    if all_passed {
        println!("  ╔═══════════════════════════════════════════════════════════════╗");
        println!("  ║                                                               ║");
        println!("  ║   ██████╗ ██████╗ ██╗     ██████╗     ███████╗██╗   ██╗       ║");
        println!("  ║  ██╔════╝██╔═══██╗██║     ██╔══██╗    ██╔════╝██║   ██║       ║");
        println!("  ║  ██║     ██║   ██║██║     ██║  ██║    █████╗  ██║   ██║       ║");
        println!("  ║  ██║     ██║   ██║██║     ██║  ██║    ██╔══╝  ██║   ██║       ║");
        println!("  ║  ╚██████╗╚██████╔╝███████╗██████╔╝    ██║     ╚██████╔╝       ║");
        println!("  ║   ╚═════╝ ╚═════╝ ╚══════╝╚═════╝     ╚═╝      ╚═════╝        ║");
        println!("  ║                                                               ║");
        println!("  ║   ███████╗██╗   ██╗███████╗██╗ ██████╗ ███╗   ██╗             ║");
        println!("  ║   ██╔════╝██║   ██║██╔════╝██║██╔═══██╗████╗  ██║             ║");
        println!("  ║   █████╗  ██║   ██║███████╗██║██║   ██║██╔██╗ ██║             ║");
        println!("  ║   ██╔══╝  ██║   ██║╚════██║██║██║   ██║██║╚██╗██║             ║");
        println!("  ║   ██║     ╚██████╔╝███████║██║╚██████╔╝██║ ╚████║             ║");
        println!("  ║   ╚═╝      ╚═════╝ ╚══════╝╚═╝ ╚═════╝ ╚═╝  ╚═══╝             ║");
        println!("  ║                                                               ║");
        println!("  ║               INTEGRATION SUCCESSFUL                          ║");
        println!("  ║                                                               ║");
        println!("  ╚═══════════════════════════════════════════════════════════════╝");
    } else {
        println!("  ╔═══════════════════════════════════════════════════════════════╗");
        println!("  ║   INTEGRATION INCOMPLETE - REVIEW REQUIRED                    ║");
        println!("  ╚═══════════════════════════════════════════════════════════════╝");
    }

    println!();
    println!("═══════════════════════════════════════════════════════════════════");

    // === PERFORMANCE TEST ===
    println!();
    println!("📊 PERFORMANCE TEST: 60 consecutive frames");
    println!();

    let mut total_time = Duration::ZERO;
    let mut worst_frame = Duration::ZERO;

    for frame in 0..60 {
        // Simulate Unit 4 updating positions
        {
            let mut write = db_world.write_handle();
            write.update_positions(0.016);
        }
        db_world.swap_buffers();

        // Add some random events
        if frame % 10 == 0 {
            render_loop.event_queue_mut().push_item_drop(
                [100.0, 50.0, 100.0],
                frame as u32,
                (frame % 5) as u8,
                1,
                1,
            );
        }

        // Render
        let read = db_world.read_handle();
        let reader = CoreWorldReader::new(&read);
        let start = Instant::now();
        let _ = render_loop.frame(&reader, [100.0, 55.0, 100.0], [[1.0, 0.0, 0.0, 0.0]; 4], 0.016);
        let elapsed = start.elapsed();

        total_time += elapsed;
        if elapsed > worst_frame {
            worst_frame = elapsed;
        }
    }

    let avg_time = total_time / 60;
    let fps = 1.0 / avg_time.as_secs_f64();

    println!("   Average frame time: {:?}", avg_time);
    println!("   Worst frame time: {:?}", worst_frame);
    println!("   Theoretical FPS: {:.1}", fps);
    println!("   Budget utilization: {:.1}%", (avg_time.as_micros() as f64 / 16666.0) * 100.0);
    println!();

    if fps > 1000.0 {
        println!("   ✓ EXCEEDS TARGET (>1000 FPS theoretical)");
    } else {
        println!("   ✗ BELOW TARGET");
    }

    println!();
    println!("═══════════════════════════════════════════════════════════════════");
}
