//! # 500 Bot Simulation
//!
//! MISSION: Prove smooth movement under ARCHITECT's conditions:
//! - 500 bots
//! - 2% packet loss
//! - 50ms jitter
//! - 60Hz tick rate
//!
//! This binary runs a complete simulation and outputs statistics.

use oroboros_networking::simulation::{BotSimulation, SimulationConfig, NetworkConditions};
use std::time::Instant;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║         OROBOROS INFERNO - 500 BOT SIMULATION                    ║");
    println!("║         THE ARCHITECT'S STRESS TEST                              ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    // Configuration as specified by THE ARCHITECT
    let config = SimulationConfig {
        bot_count: 500,
        tick_rate: 60,
        duration_secs: 60, // 1 minute simulation
        network: NetworkConditions {
            base_latency_ms: 30,
            jitter_ms: 50,      // 50ms jitter as specified
            packet_loss_percent: 2,  // 2% packet loss as specified
            duplicate_percent: 0,
            out_of_order_percent: 0,
        },
        arena_size: 200.0,
        enable_shooting: true,
    };

    println!("┌─ CONFIGURATION ─────────────────────────────────────────────────┐");
    println!("│ Bot Count:          {} entities                              │", config.bot_count);
    println!("│ Tick Rate:          {} Hz                                       │", config.tick_rate);
    println!("│ Duration:           {} seconds                                   │", config.duration_secs);
    println!("│ Base Latency:       {} ms                                       │", config.network.base_latency_ms);
    println!("│ Jitter:             {} ms                                       │", config.network.jitter_ms);
    println!("│ Packet Loss:        {}%                                          │", config.network.packet_loss_percent);
    println!("│ Arena Size:         {}x{}                                   │", config.arena_size, config.arena_size);
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    println!("Starting simulation...");
    let start = Instant::now();

    let mut simulation = BotSimulation::new(config.clone());

    // Progress tracking
    let total_ticks = config.duration_secs as u64 * config.tick_rate as u64;
    let mut last_progress = 0;

    while simulation.tick() {
        let progress = (simulation.current_tick() * 100 / total_ticks) as usize;
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
    println!("║                    SIMULATION RESULTS                            ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    println!("┌─ TIMING ────────────────────────────────────────────────────────┐");
    println!("│ Real Time:          {:.2} seconds                               ", elapsed.as_secs_f64());
    println!("│ Simulated Time:     {} seconds                                   ", config.duration_secs);
    println!("│ Realtime Factor:    {:.2}x                                       ", 
        config.duration_secs as f64 / elapsed.as_secs_f64());
    println!("│ Total Ticks:        {}                                        ", stats.total_ticks);
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    println!("┌─ TICK PERFORMANCE ─────────────────────────────────────────────┐");
    let budget_us = 1_000_000u64 / config.tick_rate as u64;
    println!("│ Budget per Tick:    {} μs                                    ", budget_us);
    println!("│ Min Tick Time:      {} μs                                       ", stats.min_tick_us);
    println!("│ Max Tick Time:      {} μs                                       ", stats.max_tick_us);
    println!("│ Avg Tick Time:      {} μs                                       ", stats.avg_tick_us);
    println!("│ Late Ticks:         {} ({:.2}%)                                 ", 
        stats.late_ticks, 
        stats.late_ticks as f64 / stats.total_ticks as f64 * 100.0);
    
    let tick_ok = stats.avg_tick_us < budget_us;
    if tick_ok {
        println!("│ Status:             ✓ WITHIN BUDGET                           │");
    } else {
        println!("│ Status:             ✗ OVER BUDGET                             │");
    }
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

    println!("┌─ SMOOTHNESS (CRITICAL) ───────────────────────────────────────────┐");
    println!("│ Avg Position Error: {:.4} units                                  ", stats.avg_position_error);
    println!("│ Max Position Error: {:.4} units                                  ", stats.max_position_error);
    println!("│ Reconciliations:    {}                                      ", stats.reconciliation_count);
    println!("│ Snap Corrections:   {} ({:.2}%)                                   ", 
        stats.snap_count,
        stats.snap_count as f64 / stats.reconciliation_count.max(1) as f64 * 100.0);
    
    // ARCHITECT's criteria: movement must be smooth
    let smoothness_ok = stats.avg_position_error < 0.5 && 
        (stats.snap_count as f64 / stats.reconciliation_count.max(1) as f64) < 0.05;
    
    if smoothness_ok {
        println!("│ Status:             ✓ SMOOTH MOVEMENT                         │");
    } else {
        println!("│ Status:             ✗ TOO JERKY                               │");
    }
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    println!("┌─ DRAGON STATE MACHINE ──────────────────────────────────────────┐");
    let dragon = simulation.dragon_state();
    let state_name = match dragon.state {
        0 => "SLEEP (market calm)",
        1 => "STALK (volatility rising)",
        2 => "INFERNO (crash/spike)",
        _ => "UNKNOWN",
    };
    println!("│ Current State:      {}                        ", state_name);
    println!("│ Aggression Level:   {}/255                                     ", dragon.aggression);
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    // Final verdict
    println!("╔══════════════════════════════════════════════════════════════════╗");
    if tick_ok && smoothness_ok {
        println!("║  ✓ MISSION ACCOMPLISHED                                         ║");
        println!("║    500 bots, 2% packet loss, 50ms jitter                        ║");
        println!("║    Movement is SMOOTH. THE ARCHITECT is pleased.                ║");
    } else {
        println!("║  ✗ MISSION FAILED                                               ║");
        if !tick_ok {
            println!("║    Tick performance is over budget                              ║");
        }
        if !smoothness_ok {
            println!("║    Movement is not smooth enough                                ║");
        }
    }
    println!("╚══════════════════════════════════════════════════════════════════╝");
}
