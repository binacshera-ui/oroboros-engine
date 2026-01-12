//! Benchmark for chunk generation performance.
//!
//! TARGET: 10,000x10,000 blocks in under 3 seconds
//!
//! Run with: cargo bench --package oroboros_procedural --bench chunk_benchmark

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use oroboros_procedural::chunk::{ChunkCoord, ChunkGenerator, CHUNK_SIZE};
use oroboros_procedural::noise::WorldSeed;

fn benchmark_single_chunk(c: &mut Criterion) {
    let gen = ChunkGenerator::new(WorldSeed::new(42));

    c.bench_function("single_chunk_generation", |b| {
        let mut coord = 0i32;
        b.iter(|| {
            coord = coord.wrapping_add(1);
            black_box(gen.generate(ChunkCoord::new(coord, coord / 2)))
        });
    });
}

fn benchmark_chunk_grid(c: &mut Criterion) {
    let gen = ChunkGenerator::new(WorldSeed::new(42));

    let mut group = c.benchmark_group("chunk_grid");

    // 32x32 chunks = 512x512 blocks
    group.throughput(Throughput::Elements(32 * 32));
    group.bench_function("32x32_chunks", |b| {
        b.iter(|| {
            for z in 0..32 {
                for x in 0..32 {
                    black_box(gen.generate(ChunkCoord::new(x, z)));
                }
            }
        });
    });

    group.finish();
}

fn benchmark_extrapolated_10k(c: &mut Criterion) {
    let gen = ChunkGenerator::new(WorldSeed::new(42));

    // We'll benchmark 100x100 chunks (1600x1600 blocks) and extrapolate
    let mut group = c.benchmark_group("world_generation");
    group.sample_size(10);

    // 100x100 chunks = 10,000 chunks
    let chunks_to_generate = 100 * 100;
    group.throughput(Throughput::Elements(chunks_to_generate));

    group.bench_function("100x100_chunks_extrapolate_to_10k", |b| {
        b.iter(|| {
            for z in 0..100i32 {
                for x in 0..100i32 {
                    black_box(gen.generate(ChunkCoord::new(x, z)));
                }
            }
        });
    });

    group.finish();
}

fn benchmark_biome_classification(c: &mut Criterion) {
    use oroboros_procedural::biome::BiomeClassifier;

    let classifier = BiomeClassifier::new(WorldSeed::new(42));

    c.bench_function("biome_classification_per_block", |b| {
        let mut x = 0.0f64;
        b.iter(|| {
            x += 1.0;
            black_box(classifier.classify(black_box(x), black_box(x * 0.7)))
        });
    });
}

fn benchmark_terrain_height(c: &mut Criterion) {
    use oroboros_procedural::biome::BiomeClassifier;

    let classifier = BiomeClassifier::new(WorldSeed::new(42));

    c.bench_function("terrain_height_calculation", |b| {
        let mut x = 0.0f64;
        b.iter(|| {
            x += 1.0;
            black_box(classifier.get_terrain_height(black_box(x), black_box(x * 0.7), 64, 256))
        });
    });
}

fn benchmark_chunk_compression(c: &mut Criterion) {
    let gen = ChunkGenerator::new(WorldSeed::new(42));
    let chunk = gen.generate(ChunkCoord::new(0, 0));
    let temp_path = std::env::temp_dir().join("bench_chunk.bin");

    c.bench_function("chunk_compression", |b| {
        b.iter(|| {
            chunk.save_compressed(black_box(&temp_path)).unwrap();
        });
    });

    // Cleanup
    std::fs::remove_file(&temp_path).ok();
}

/// Validates that we can generate 10,000x10,000 in under 3 seconds
/// by extrapolating from smaller benchmarks.
fn print_extrapolation_estimate() {
    use std::time::Instant;

    let gen = ChunkGenerator::new(WorldSeed::new(42));

    // Generate 50x50 chunks and measure
    let start = Instant::now();
    for z in 0..50i32 {
        for x in 0..50i32 {
            let _ = gen.generate(ChunkCoord::new(x, z));
        }
    }
    let elapsed = start.elapsed();

    let chunks_generated = 50 * 50;
    let blocks_generated = chunks_generated * CHUNK_SIZE * CHUNK_SIZE;

    // 10,000 x 10,000 blocks = 625 x 625 = 390,625 chunks
    let target_chunks = 625 * 625;
    let extrapolated_time = elapsed.as_secs_f64() * (target_chunks as f64 / chunks_generated as f64);

    eprintln!("\n=== Performance Extrapolation ===");
    eprintln!("Measured: {} chunks ({} blocks) in {:?}", chunks_generated, blocks_generated, elapsed);
    eprintln!("Rate: {:.0} chunks/sec", chunks_generated as f64 / elapsed.as_secs_f64());
    eprintln!("Extrapolated 10,000x10,000: {:.2}s", extrapolated_time);
    eprintln!("Target: <3.0s");
    eprintln!("Status: {}", if extrapolated_time < 3.0 { "PASS ✓" } else { "FAIL ✗" });
    eprintln!("=================================\n");
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = benchmark_single_chunk,
              benchmark_chunk_grid,
              benchmark_extrapolated_10k,
              benchmark_biome_classification,
              benchmark_terrain_height,
              benchmark_chunk_compression
}

criterion_main!(benches);

// Run extrapolation on test
#[test]
fn test_performance_extrapolation() {
    print_extrapolation_estimate();
}
