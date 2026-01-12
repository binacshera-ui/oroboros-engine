//! # Dirty Copy Benchmark
//!
//! ARCHITECT'S CHALLENGE: Prove the Cold Buffer Problem is solved.
//!
//! This benchmark measures:
//! 1. Full 32MB copy (naive solution)
//! 2. Sparse dirty copy (our solution)
//! 3. Memory bandwidth usage
//!
//! Target: <5% overhead when <5% of entities are dirty.

#![allow(dead_code)]
#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use oroboros_core::{ArchetypeWorld, Position, Velocity};

/// Benchmark full buffer copy (32MB for 1M entities)
fn bench_full_copy(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_sync_full_copy");

    for entity_count in [100_000, 500_000, 1_000_000] {
        // Create source world with entities
        let mut source = ArchetypeWorld::new(entity_count, 0);
        for i in 0..entity_count {
            let _ = source.spawn_pv(
                Position::new(i as f32, 0.0, 0.0),
                Velocity::new(1.0, 0.0, 0.0),
            );
        }
        // Mark all as dirty (simulates physics update)
        source.pv_table.dirty_tracker_mut().mark_all_dirty(entity_count);

        // Create destination world
        let mut dest = ArchetypeWorld::new(entity_count, 0);

        group.throughput(criterion::Throughput::Bytes(
            (entity_count * 32) as u64 // ~32 bytes per entity
        ));

        group.bench_with_input(
            BenchmarkId::new("full_copy", entity_count),
            &entity_count,
            |b, _| {
                b.iter(|| {
                    dest.sync_dirty_from(black_box(&source));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark sparse dirty copy (only changed entities)
fn bench_sparse_copy(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_sync_sparse_copy");

    let entity_count = 1_000_000;

    for dirty_pct in [1, 5, 10, 25, 50] {
        let dirty_count = entity_count * dirty_pct / 100;

        // Create source world with entities
        let mut source = ArchetypeWorld::new(entity_count, 0);
        for i in 0..entity_count {
            let _ = source.spawn_pv(
                Position::new(i as f32, 0.0, 0.0),
                Velocity::new(1.0, 0.0, 0.0),
            );
        }
        // Clear dirty flags from spawning
        source.clear_dirty();

        // Mark only a percentage as dirty (simulates partial update)
        for i in 0..dirty_count {
            source.pv_table.dirty_tracker_mut().mark_dirty(i);
        }

        // Create destination world with matching data
        let mut dest = ArchetypeWorld::new(entity_count, 0);
        for i in 0..entity_count {
            let _ = dest.spawn_pv(
                Position::new(i as f32, 0.0, 0.0),
                Velocity::new(1.0, 0.0, 0.0),
            );
        }

        group.throughput(criterion::Throughput::Bytes(
            (dirty_count * 32) as u64 // Only dirty bytes
        ));

        group.bench_with_input(
            BenchmarkId::new(format!("{}pct_dirty", dirty_pct), entity_count),
            &entity_count,
            |b, _| {
                b.iter(|| {
                    dest.sync_dirty_from(black_box(&source));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark the complete swap_buffers cycle
fn bench_swap_cycle(c: &mut Criterion) {
    use oroboros_core::DoubleBufferedWorld;

    let mut group = c.benchmark_group("swap_buffers_cycle");

    let entity_count = 1_000_000;

    // Test with different dirty ratios
    for dirty_pct in [5, 25, 50, 100] {
        let db = DoubleBufferedWorld::new(entity_count, 0);

        // Populate write buffer
        {
            let mut write = db.write_handle();
            for i in 0..entity_count {
                let _ = write.spawn_pv(
                    Position::new(i as f32, 0.0, 0.0),
                    Velocity::new(1.0, 0.0, 0.0),
                );
            }
            // Clear dirty flags first
            write.clear_dirty();

            // Mark desired percentage as dirty
            let dirty_count = entity_count * dirty_pct / 100;
            write.pv_table.dirty_tracker_mut().mark_all_dirty(dirty_count.min(entity_count));
        }

        let label = format!("{}pct_dirty_1M", dirty_pct);

        group.bench_function(&label, |b| {
            b.iter(|| {
                // This includes the dirty copy
                db.swap_buffers();
            });
        });
    }

    group.finish();
}

/// Report bandwidth savings
fn bench_bandwidth_savings(c: &mut Criterion) {
    let mut group = c.benchmark_group("bandwidth_comparison");

    let entity_count = 1_000_000;
    let bytes_per_entity = 32usize; // Position (12) + Velocity (12) + padding
    let full_copy_bytes = entity_count * bytes_per_entity;

    // Simulated scenarios
    let scenarios = [
        ("idle_world_1pct", 1),        // Most entities static
        ("active_world_5pct", 5),      // Typical gameplay
        ("combat_zone_25pct", 25),     // Heavy action area
        ("physics_tick_100pct", 100),  // Full physics update
    ];

    for (name, dirty_pct) in scenarios {
        let mut source = ArchetypeWorld::new(entity_count, 0);
        for i in 0..entity_count {
            let _ = source.spawn_pv(
                Position::new(i as f32, 0.0, 0.0),
                Velocity::new(1.0, 0.0, 0.0),
            );
        }
        source.clear_dirty();

        let dirty_count = entity_count * dirty_pct / 100;
        if dirty_pct == 100 {
            source.pv_table.dirty_tracker_mut().mark_all_dirty(entity_count);
        } else {
            for i in 0..dirty_count {
                source.pv_table.dirty_tracker_mut().mark_dirty(i);
            }
        }

        let mut dest = ArchetypeWorld::new(entity_count, 0);
        for i in 0..entity_count {
            let _ = dest.spawn_pv(
                Position::new(i as f32, 0.0, 0.0),
                Velocity::new(1.0, 0.0, 0.0),
            );
        }

        let stats = source.sync_stats();
        let sparse_bytes = stats.pv_stats.sparse_copy_bytes;
        let savings = 100.0 * (1.0 - sparse_bytes as f64 / full_copy_bytes as f64);

        println!(
            "[{}] Full: {} MB, Sparse: {} MB, Savings: {:.1}%",
            name,
            full_copy_bytes / 1_000_000,
            sparse_bytes / 1_000_000,
            savings
        );

        group.bench_function(name, |b| {
            b.iter(|| {
                dest.sync_dirty_from(black_box(&source));
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_sparse_copy,
    bench_swap_cycle,
    bench_bandwidth_savings,
);
criterion_main!(benches);
