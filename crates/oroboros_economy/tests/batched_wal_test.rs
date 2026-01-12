//! Integration test for Batched WAL.

use oroboros_economy::{BatchedWal, BatchedWalConfig};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn temp_wal_path() -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("test_batched_wal_{id}.wal"))
}

#[test]
fn test_batched_wal_basic() {
    let path = temp_wal_path();
    let config = BatchedWalConfig {
        max_batch_size: 10,
        max_batch_delay_ms: 5,
        ring_buffer_size: 100,
    };

    let wal = BatchedWal::open(&path, config).unwrap();

    // Write some entries
    let handle = wal.log_loot_drop(1, 1, 100, 5).unwrap();
    handle.wait();

    let stats = wal.stats();
    assert!(stats.total_ops >= 1);

    drop(wal);
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_batched_wal_throughput() {
    let path = temp_wal_path();
    let config = BatchedWalConfig {
        max_batch_size: 100,
        max_batch_delay_ms: 10,
        ring_buffer_size: 50_000,
    };

    let wal = Arc::new(BatchedWal::open(&path, config).unwrap());
    let total_ops = Arc::new(AtomicUsize::new(0));
    let ops_per_thread = 1000;
    let num_threads = 10;

    let start = Instant::now();

    // Spawn threads
    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let wal = Arc::clone(&wal);
            let total = Arc::clone(&total_ops);

            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    let handle = wal.log_loot_drop(t as u64, i as u32, 100, 1).unwrap();
                    handle.wait();
                    total.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    // Wait for all threads
    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total = total_ops.load(Ordering::Relaxed);
    let stats = wal.stats();

    println!("\n=== Batched WAL Throughput Test ===");
    println!("Total operations: {}", total);
    println!("Total time: {:?}", elapsed);
    println!(
        "Throughput: {:.0} ops/sec",
        total as f64 / elapsed.as_secs_f64()
    );
    println!("Average batch size: {:.1}", stats.avg_batch_size);
    println!("Average sync time: {:.2} µs", stats.avg_sync_time_us);
    println!(
        "Average time per op: {:.2} ns ({:.4} ms)",
        stats.avg_op_time_ns,
        stats.avg_op_time_ns / 1_000_000.0
    );

    // Target: < 0.05ms per operation
    let avg_op_ms = stats.avg_op_time_ns / 1_000_000.0;
    println!("\nTarget: < 0.05ms per op");
    println!("Actual: {:.4} ms per op", avg_op_ms);

    drop(wal);
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_50k_concurrent_crush() {
    let path = temp_wal_path();
    let config = BatchedWalConfig {
        max_batch_size: 500,
        max_batch_delay_ms: 5,
        ring_buffer_size: 100_000,
    };

    let wal = Arc::new(BatchedWal::open(&path, config).unwrap());
    let total_ops = 50_000usize;
    let num_threads = 50;
    let ops_per_thread = total_ops / num_threads;

    let completed = Arc::new(AtomicUsize::new(0));
    let latencies_sum = Arc::new(AtomicUsize::new(0));
    let max_latency = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();

    // Spawn threads
    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let wal = Arc::clone(&wal);
            let completed = Arc::clone(&completed);
            let latencies_sum = Arc::clone(&latencies_sum);
            let max_latency = Arc::clone(&max_latency);

            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    let op_start = Instant::now();

                    let handle = wal.log_loot_drop(t as u64, i as u32, 100, 1).unwrap();
                    handle.wait();

                    let latency_us = op_start.elapsed().as_micros() as usize;
                    latencies_sum.fetch_add(latency_us, Ordering::Relaxed);

                    // Update max latency
                    let mut current_max = max_latency.load(Ordering::Relaxed);
                    while latency_us > current_max {
                        match max_latency.compare_exchange_weak(
                            current_max,
                            latency_us,
                            Ordering::Relaxed,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(x) => current_max = x,
                        }
                    }

                    completed.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    // Wait for all
    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_completed = completed.load(Ordering::Relaxed);
    let total_latency_us = latencies_sum.load(Ordering::Relaxed);
    let peak_latency_us = max_latency.load(Ordering::Relaxed);
    let avg_latency_us = total_latency_us as f64 / total_completed as f64;

    let stats = wal.stats();

    println!("\n=== 50K CRUSH TEST RESULTS ===");
    println!("Total operations: {}", total_completed);
    println!("Total time: {:?}", elapsed);
    println!(
        "Throughput: {:.0} ops/sec",
        total_completed as f64 / elapsed.as_secs_f64()
    );
    println!("\n--- Latency ---");
    println!(
        "Average: {:.2} µs ({:.4} ms)",
        avg_latency_us,
        avg_latency_us / 1000.0
    );
    println!(
        "Peak: {} µs ({:.2} ms)",
        peak_latency_us,
        peak_latency_us as f64 / 1000.0
    );
    println!("\n--- WAL Stats ---");
    println!("Batches written: {}", stats.total_batches);
    println!("Average batch size: {:.1}", stats.avg_batch_size);
    println!("Average sync time: {:.2} µs", stats.avg_sync_time_us);
    println!("Bytes written: {} KB", stats.total_bytes / 1024);
    println!("\n--- Per-Operation Cost ---");
    println!(
        "Amortized I/O time: {:.4} ms/op",
        stats.avg_op_time_ns / 1_000_000.0
    );

    // Assertions
    assert_eq!(
        total_completed, total_ops,
        "All operations should complete"
    );

    drop(wal);
    std::fs::remove_file(&path).ok();
}
