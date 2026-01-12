//! # Vertical Slice Integration Test Binary
//!
//! Runs the full combat loop test:
//! Attack → UDP → Physics → Economy → Response
//!
//! THE ARCHITECT DEMANDS: < 50ms RTT on local network.

fn main() {
    // Use different ports for each run to avoid bind conflicts
    let port_base: u16 = 28800 + (std::process::id() as u16 % 1000);
    
    let config = oroboros::integration::VerticalSliceConfig {
        server_addr: format!("127.0.0.1:{}", port_base).parse().unwrap(),
        client_addr: format!("127.0.0.1:{}", port_base + 1).parse().unwrap(),
        entity_count: 500,
        max_rtt_ms: 50,
    };

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║             THE ARCHITECT'S VERTICAL SLICE TEST                  ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  \"The whole pipeline: Attack → UDP → Physics → Economy →        ║");
    println!("║   Response, in under 50ms. No excuses.\"                         ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    let metrics = oroboros::integration::run_vertical_slice_test(config, 1000);

    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    if metrics.rtt_requirement_met {
        println!("║  ✓ MISSION ACCOMPLISHED                                         ║");
        println!("║    Full combat loop under {}ms RTT                              ║", 50);
    } else {
        println!("║  ✗ MISSION FAILED                                               ║");
        println!("║    RTT exceeded {}ms target                                     ║", 50);
    }
    println!("╚══════════════════════════════════════════════════════════════════╝");
}
