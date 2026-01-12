//! # Realistic Network Simulation Binary
//!
//! THE ARCHITECT DEMANDS TRUTH.
//! Zero error is a LIE.

fn main() {
    use oroboros_networking::simulation::realistic::{RealisticSimulation, RealisticConfig};
    use std::time::Instant;

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║    OROBOROS INFERNO - REALISTIC NETWORK SIMULATION               ║");
    println!("║    THE ARCHITECT DEMANDS TRUTH                                   ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║    Zero error is a LIE. Real networks have LATENCY.              ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    let config = RealisticConfig {
        bot_count: 500,
        duration_secs: 60,
        base_latency_ticks: 3,   // ~50ms base RTT at 60Hz
        jitter_ticks: 3,         // ~50ms jitter
        packet_loss_percent: 2,  // 2% loss
    };

    println!("┌─ CONFIGURATION ─────────────────────────────────────────────────┐");
    println!("│ Bot Count:          {} entities                              │", config.bot_count);
    println!("│ Duration:           {} seconds                                   │", config.duration_secs);
    println!("│ Base Latency:       {} ticks (~{:.0}ms)                         │", 
        config.base_latency_ticks, 
        config.base_latency_ticks as f32 * 16.67);
    println!("│ Jitter:             {} ticks (~{:.0}ms)                         │", 
        config.jitter_ticks,
        config.jitter_ticks as f32 * 16.67);
    println!("│ Packet Loss:        {}%                                          │", config.packet_loss_percent);
    println!("│ Movement:           ZIG-ZAG (non-linear prediction stress)      │");
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    println!("Starting REALISTIC simulation (with actual network delay)...");
    let start = Instant::now();

    let mut simulation = RealisticSimulation::new(config.clone());
    
    let total_ticks = u64::from(config.duration_secs) * 60;
    let mut last_progress = 0;

    while simulation.tick() {
        let progress = (u64::from(simulation.current_tick()) * 100 / total_ticks) as usize;
        if progress > last_progress && progress % 10 == 0 {
            print!("\r[");
            for i in 0..10 {
                if i < progress / 10 {
                    print!("█");
                } else {
                    print!("░");
                }
            }
            print!("] {}% - Tick {}/{}", progress, simulation.current_tick(), total_ticks);
            last_progress = progress;
        }
    }
    println!();
    println!();

    let elapsed = start.elapsed();
    let stats = simulation.stats();

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                    REALISTIC SIMULATION RESULTS                  ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    println!("┌─ TIMING ────────────────────────────────────────────────────────┐");
    println!("│ Real Time:          {:.2} seconds                               ", elapsed.as_secs_f64());
    println!("│ Simulated Time:     {} seconds                                   ", config.duration_secs);
    println!("│ Realtime Factor:    {:.2}x                                       ", 
        f64::from(config.duration_secs) / elapsed.as_secs_f64());
    println!("│ Total Ticks:        {}                                        ", stats.total_ticks);
    println!("│ Avg Tick Time:      {} μs                                       ", stats.avg_tick_us);
    println!("│ Late Ticks:         {}                                          ", stats.late_ticks);
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    println!("┌─ NETWORK ────────────────────────────────────────────────────────┐");
    println!("│ Packets Sent:       {}                                      ", stats.packets_sent);
    println!("│ Packets Dropped:    {}                                        ", stats.packets_dropped);
    let actual_loss = stats.packets_dropped as f64 / 
        (stats.packets_sent + stats.packets_dropped) as f64 * 100.0;
    println!("│ Actual Loss Rate:   {:.2}%                                       ", actual_loss);
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    println!("┌─ RECONCILIATION (THE TRUTH) ─────────────────────────────────────┐");
    println!("│                                                                  │");
    println!("│ Avg Position Error: {:.4} units                                  ", stats.avg_position_error);
    println!("│ Max Position Error: {:.4} units                                  ", stats.max_position_error);
    println!("│                                                                  │");
    println!("│ Total Corrections:  {} (prediction was wrong)              ", stats.total_corrections);
    println!("│ Snap Corrections:   {} (error > 1.0 unit, forced snap)     ", stats.snap_corrections);
    println!("│                                                                  │");
    
    let snap_rate = if stats.reconciliations > 0 {
        stats.snap_corrections as f64 / stats.reconciliations as f64 * 100.0
    } else {
        0.0
    };
    println!("│ Snap Rate:          {:.2}%                                        ", snap_rate);
    println!("│                                                                  │");

    // Evaluate quality
    let error_ok = stats.avg_position_error > 0.0 && stats.avg_position_error < 0.5;
    let snaps_ok = snap_rate < 5.0;
    
    if stats.avg_position_error == 0.0 {
        println!("│ ⚠ WARNING: Zero error detected! This is IMPOSSIBLE with         │");
        println!("│            real network delay. Check simulation integrity.      │");
    } else if error_ok {
        println!("│ ✓ Error within acceptable range (<0.5 units average)            │");
    } else {
        println!("│ ✗ Error too high - prediction quality needs improvement         │");
    }

    if snaps_ok {
        println!("│ ✓ Snap rate acceptable (<5%)                                    │");
    } else {
        println!("│ ✗ Too many snaps - movement will feel jerky                     │");
    }
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    // Final verdict
    println!("╔══════════════════════════════════════════════════════════════════╗");
    if error_ok && snaps_ok && stats.avg_position_error > 0.0 {
        println!("║  ✓ SIMULATION HONEST                                            ║");
        println!("║    Real network delay simulated.                                ║");
        println!("║    Reconciliation working correctly.                            ║");
        println!("║    Movement is smooth despite network jitter.                   ║");
    } else if stats.avg_position_error == 0.0 {
        println!("║  ✗ SIMULATION DISHONEST                                         ║");
        println!("║    Zero error is physically impossible.                         ║");
        println!("║    Network delay not being simulated correctly.                 ║");
    } else {
        println!("║  ⚠ SIMULATION NEEDS TUNING                                      ║");
        if !error_ok {
            println!("║    Position error too high                                      ║");
        }
        if !snaps_ok {
            println!("║    Too many snap corrections                                    ║");
        }
    }
    println!("╚══════════════════════════════════════════════════════════════════╝");
}
