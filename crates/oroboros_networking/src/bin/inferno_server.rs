//! # Inferno Game Server
//!
//! The authoritative game server for OROBOROS Inferno world.
//!
//! ## Usage
//!
//! ```bash
//! inferno_server --port 7777 --tick-rate 60 --max-clients 500
//! ```

use oroboros_networking::server::{InfernoServer, ServerConfig, TickLoop};
use std::time::Instant;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║         OROBOROS INFERNO SERVER                                  ║");
    println!("║         THE AUTHORITATIVE WORLD                                  ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    // Parse command line arguments (simple parsing, no external deps)
    let args: Vec<String> = std::env::args().collect();
    let mut port = 7777u16;
    let mut tick_rate = 60u32;
    let mut max_clients = 500usize;
    let mut duration_secs: Option<u32> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" | "-p" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(7777);
                    i += 1;
                }
            }
            "--tick-rate" | "-t" => {
                if i + 1 < args.len() {
                    tick_rate = args[i + 1].parse().unwrap_or(60);
                    i += 1;
                }
            }
            "--max-clients" | "-m" => {
                if i + 1 < args.len() {
                    max_clients = args[i + 1].parse().unwrap_or(500);
                    i += 1;
                }
            }
            "--duration" | "-d" => {
                if i + 1 < args.len() {
                    duration_secs = args[i + 1].parse().ok();
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("Usage: inferno_server [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -p, --port <PORT>          UDP port to bind (default: 7777)");
                println!("  -t, --tick-rate <RATE>     Server tick rate in Hz (default: 60)");
                println!("  -m, --max-clients <NUM>    Maximum clients (default: 500)");
                println!("  -d, --duration <SECS>      Run for N seconds then exit");
                println!("  -h, --help                 Show this help");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    let bind_addr = format!("0.0.0.0:{}", port);

    println!("┌─ CONFIGURATION ─────────────────────────────────────────────────┐");
    println!("│ Bind Address:       {}                               ", bind_addr);
    println!("│ Tick Rate:          {} Hz                                       ", tick_rate);
    println!("│ Max Clients:        {}                                        ", max_clients);
    if let Some(d) = duration_secs {
        println!("│ Duration:           {} seconds                                 ", d);
    } else {
        println!("│ Duration:           infinite                                    ");
    }
    println!("└──────────────────────────────────────────────────────────────────┘");
    println!();

    let config = ServerConfig {
        tick_rate,
        max_clients,
        port,
        bind_address: bind_addr.parse().expect("Valid bind address"),
    };

    let mut server = InfernoServer::new(config);
    let mut tick_loop = TickLoop::new(tick_rate);

    println!("Starting server...");
    println!();

    let start = Instant::now();
    let mut last_stats_tick = 0u64;
    let stats_interval = tick_rate as u64 * 5; // Every 5 seconds

    loop {
        // Check duration limit
        if let Some(duration) = duration_secs {
            if start.elapsed().as_secs() >= duration as u64 {
                break;
            }
        }

        // Wait for next tick
        tick_loop.wait_for_next_tick();

        while tick_loop.should_tick() {
            let tick_start = tick_loop.begin_tick();

            // Run server tick
            server.tick();

            tick_loop.end_tick(tick_start);

            // Print stats periodically
            let current_tick = tick_loop.tick_count();
            if current_tick - last_stats_tick >= stats_interval {
                last_stats_tick = current_tick;
                let stats = tick_loop.stats();
                let state = server.state();

                println!("┌─ SERVER STATUS (Tick {}) ────────────────────────────────────", current_tick);
                println!("│ Uptime:             {:.1}s", start.elapsed().as_secs_f64());
                println!("│ Clients:            {}", state.active_clients());
                println!("│ Entities:           {}", state.active_entities());
                println!("│ Avg Tick Time:      {} μs", stats.avg_tick_us);
                println!("│ Late Ticks:         {} ({:.2}%)", 
                    stats.late_ticks,
                    stats.late_ticks as f64 / stats.total_ticks.max(1) as f64 * 100.0);
                
                let dragon = state.dragon();
                let state_name = match dragon.state {
                    0 => "SLEEP",
                    1 => "STALK",
                    2 => "INFERNO",
                    _ => "UNKNOWN",
                };
                println!("│ Dragon State:       {} (aggression: {})", state_name, dragon.aggression);
                println!("└──────────────────────────────────────────────────────────────────");
                println!();
            }
        }
    }

    let final_stats = tick_loop.stats();
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                    SERVER SHUTDOWN                               ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║ Total Ticks:        {:>10}                                 ║", final_stats.total_ticks);
    println!("║ Avg Tick Time:      {:>10} μs                             ║", final_stats.avg_tick_us);
    println!("║ Min Tick Time:      {:>10} μs                             ║", final_stats.min_tick_us);
    println!("║ Max Tick Time:      {:>10} μs                             ║", final_stats.max_tick_us);
    println!("║ Late Ticks:         {:>10}                                 ║", final_stats.late_ticks);
    println!("╚══════════════════════════════════════════════════════════════════╝");
}
