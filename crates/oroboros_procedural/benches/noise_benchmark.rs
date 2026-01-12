//! Benchmark for noise generation performance.
//!
//! TARGET: 1,000,000 samples per second
//!
//! Run with: cargo bench --package oroboros_procedural --bench noise_benchmark

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use oroboros_procedural::noise::{SimplexNoise, WorldSeed};

fn benchmark_single_sample(c: &mut Criterion) {
    let noise = SimplexNoise::new(WorldSeed::new(42));

    c.bench_function("single_noise_sample", |b| {
        let mut x = 0.0f64;
        b.iter(|| {
            x += 0.1;
            black_box(noise.sample(black_box(x), black_box(x * 0.7)))
        });
    });
}

fn benchmark_million_samples(c: &mut Criterion) {
    let noise = SimplexNoise::new(WorldSeed::new(42));

    let mut group = c.benchmark_group("million_samples");
    group.throughput(Throughput::Elements(1_000_000));
    group.sample_size(10);

    group.bench_function("1M_noise_samples", |b| {
        b.iter(|| {
            for i in 0..1_000_000 {
                let x = (i % 1000) as f64 * 0.1;
                let y = (i / 1000) as f64 * 0.1;
                black_box(noise.sample(x, y));
            }
        });
    });

    group.finish();
}

fn benchmark_octaved_noise(c: &mut Criterion) {
    let noise = SimplexNoise::new(WorldSeed::new(42));

    c.bench_function("octaved_noise_6_octaves", |b| {
        let mut x = 0.0f64;
        b.iter(|| {
            x += 0.1;
            black_box(noise.octaved(black_box(x), black_box(x * 0.7), 6, 0.5, 2.0))
        });
    });
}

fn benchmark_ridged_noise(c: &mut Criterion) {
    let noise = SimplexNoise::new(WorldSeed::new(42));

    c.bench_function("ridged_noise_4_octaves", |b| {
        let mut x = 0.0f64;
        b.iter(|| {
            x += 0.1;
            black_box(noise.ridged(black_box(x), black_box(x * 0.7), 4, 0.5, 2.0))
        });
    });
}

fn benchmark_discrete_sampling(c: &mut Criterion) {
    let noise = SimplexNoise::new(WorldSeed::new(42));

    c.bench_function("discrete_sample_10_values", |b| {
        let mut x = 0.0f64;
        b.iter(|| {
            x += 0.1;
            black_box(noise.sample_discrete(black_box(x), black_box(x * 0.7), 10))
        });
    });
}

criterion_group!(
    benches,
    benchmark_single_sample,
    benchmark_million_samples,
    benchmark_octaved_noise,
    benchmark_ridged_noise,
    benchmark_discrete_sampling
);
criterion_main!(benches);
