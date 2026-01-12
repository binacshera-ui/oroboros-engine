//! # ECS Performance Benchmark
//!
//! ARCHITECT'S REQUIREMENTS:
//! - 1,000,000 entities
//! - < 1ms per tick
//! - 0 allocations during tick
//!
//! Run with: `cargo bench --package oroboros_core`

// Benchmarks don't need docs and may have intentionally unused code
#![allow(missing_docs)]
#![allow(dead_code)]

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use oroboros_core::{World, Position, Velocity, Component};

/// The required entity count for the benchmark.
const ENTITY_COUNT: usize = 1_000_000;

/// Maximum allowed time per tick in nanoseconds (1ms = 1,000,000ns).
const MAX_TICK_NS: u64 = 1_000_000;

/// Benchmark: Create world with 1M entities.
fn bench_world_creation(c: &mut Criterion) {
    c.bench_function("world_creation_1M", |b| {
        b.iter(|| {
            black_box(World::new(ENTITY_COUNT))
        });
    });
}

/// Benchmark: Spawn 1M entities.
fn bench_spawn_entities(c: &mut Criterion) {
    let mut group = c.benchmark_group("spawn_entities");

    for count in [10_000, 100_000, ENTITY_COUNT] {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let mut world = World::new(count);
                    for _ in 0..count {
                        black_box(world.spawn());
                    }
                    world.alive_count()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Batch spawn with position initialization.
fn bench_batch_spawn(c: &mut Criterion) {
    c.bench_function("batch_spawn_1M_with_positions", |b| {
        b.iter(|| {
            let mut world = World::new(ENTITY_COUNT);
            world.spawn_batch_with_positions(ENTITY_COUNT, |i| {
                let f = i as f32;
                Position::new(f * 0.1, f * 0.2, f * 0.3)
            });
            world.alive_count()
        });
    });
}

/// THE CRITICAL BENCHMARK: Update 1M entity positions in < 1ms.
fn bench_position_update(c: &mut Criterion) {
    // Pre-create world with all entities spawned
    let mut world = World::new(ENTITY_COUNT);
    world.spawn_batch_with_positions(ENTITY_COUNT, |i| {
        let f = i as f32;
        Position::new(f, f, f)
    });

    // Set velocities for all entities
    for i in 0..ENTITY_COUNT {
        world.velocities.set(i, Velocity::new(0.1, 0.2, 0.3));
        // Mark entity as having velocity component
        world.entities[i].add_component(Velocity::ID);
    }

    c.bench_function("CRITICAL_tick_1M_positions", |b| {
        b.iter(|| {
            world.tick_positions();
            black_box(world.alive_count())
        });
    });
}

/// Benchmark: Unchecked position update (maximum speed).
fn bench_position_update_unchecked(c: &mut Criterion) {
    let mut world = World::new(ENTITY_COUNT);
    world.spawn_batch_with_positions(ENTITY_COUNT, |i| {
        let f = i as f32;
        Position::new(f, f, f)
    });

    for i in 0..ENTITY_COUNT {
        world.velocities.set(i, Velocity::new(0.1, 0.2, 0.3));
    }

    c.bench_function("tick_1M_positions_unchecked", |b| {
        b.iter(|| {
            world.update_all_positions_unchecked(0.016);
            black_box(world.alive_count())
        });
    });
}

/// Benchmark: Raw slice iteration (theoretical minimum).
fn bench_raw_slice_update(c: &mut Criterion) {
    // Create raw arrays to establish theoretical minimum
    let mut positions: Vec<[f32; 4]> = vec![[0.0; 4]; ENTITY_COUNT];
    let velocities: Vec<[f32; 4]> = vec![[0.1, 0.2, 0.3, 0.0]; ENTITY_COUNT];

    c.bench_function("raw_slice_1M_update", |b| {
        b.iter(|| {
            for (pos, vel) in positions.iter_mut().zip(velocities.iter()) {
                pos[0] += vel[0] * 0.016;
                pos[1] += vel[1] * 0.016;
                pos[2] += vel[2] * 0.016;
            }
            black_box(positions.len())
        });
    });
}

/// Benchmark: Component storage access patterns.
fn bench_component_access(c: &mut Criterion) {
    let mut world = World::new(ENTITY_COUNT);
    world.spawn_batch_with_positions(ENTITY_COUNT, |i| {
        Position::new(i as f32, 0.0, 0.0)
    });

    let mut group = c.benchmark_group("component_access");

    // Sequential read
    group.bench_function("sequential_read_1M", |b| {
        b.iter(|| {
            let mut sum = 0.0_f32;
            for pos in world.positions.as_slice().iter() {
                sum += pos.x;
            }
            black_box(sum)
        });
    });

    // Sequential write
    group.bench_function("sequential_write_1M", |b| {
        b.iter(|| {
            for pos in world.positions.as_mut_slice().iter_mut() {
                pos.x += 0.001;
            }
            black_box(world.alive_count())
        });
    });

    // Random access (worst case for cache)
    let indices: Vec<usize> = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        (0..10000).map(|i| {
            let mut hasher = DefaultHasher::new();
            i.hash(&mut hasher);
            (hasher.finish() as usize) % ENTITY_COUNT
        }).collect()
    };

    group.bench_function("random_access_10K", |b| {
        b.iter(|| {
            let mut sum = 0.0_f32;
            for &idx in &indices {
                if let Some(pos) = world.positions.get(idx) {
                    sum += pos.x;
                }
            }
            black_box(sum)
        });
    });

    group.finish();
}

/// Benchmark: Entity spawn/despawn cycle.
fn bench_spawn_despawn_cycle(c: &mut Criterion) {
    let mut world = World::new(ENTITY_COUNT);

    // Pre-spawn half
    let mut ids = Vec::with_capacity(ENTITY_COUNT / 2);
    for _ in 0..(ENTITY_COUNT / 2) {
        ids.push(world.spawn());
    }

    c.bench_function("spawn_despawn_cycle_100K", |b| {
        b.iter(|| {
            // Despawn 100K
            for id in ids.iter().take(100_000) {
                world.despawn(*id);
            }
            // Respawn 100K
            for id in ids.iter_mut().take(100_000) {
                *id = world.spawn();
            }
            black_box(world.alive_count())
        });
    });
}

criterion_group!(
    benches,
    bench_world_creation,
    bench_spawn_entities,
    bench_batch_spawn,
    bench_position_update,
    bench_position_update_unchecked,
    bench_raw_slice_update,
    bench_component_access,
    bench_spawn_despawn_cycle,
);

criterion_main!(benches);
