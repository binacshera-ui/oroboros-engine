//! Benchmark for loot table performance.
//!
//! TARGET: 1,000,000 drops per second
//!
//! Run with: cargo bench --package oroboros_economy --bench loot_benchmark

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use oroboros_economy::loot::{LootCalculator, LootEntry, LootTable, Rarity};

fn create_test_table() -> LootTable {
    LootTable {
        block_id: 1,
        block_rarity: Rarity::Common,
        entries: vec![
            LootEntry {
                item_id: 100,
                weight: 70,
                min_quantity: 1,
                max_quantity: 3,
                rarity: Rarity::Common,
                min_level: 0,
                min_pickaxe_tier: 0,
            },
            LootEntry {
                item_id: 101,
                weight: 20,
                min_quantity: 1,
                max_quantity: 1,
                rarity: Rarity::Uncommon,
                min_level: 5,
                min_pickaxe_tier: 1,
            },
            LootEntry {
                item_id: 102,
                weight: 8,
                min_quantity: 1,
                max_quantity: 1,
                rarity: Rarity::Rare,
                min_level: 10,
                min_pickaxe_tier: 2,
            },
            LootEntry {
                item_id: 103,
                weight: 2,
                min_quantity: 1,
                max_quantity: 1,
                rarity: Rarity::Epic,
                min_level: 20,
                min_pickaxe_tier: 3,
            },
        ],
        total_weight: 0,
    }
}

fn benchmark_single_drop(c: &mut Criterion) {
    let mut calc = LootCalculator::new();
    calc.register_table(create_test_table());

    c.bench_function("single_drop_calculation", |b| {
        let mut i = 0u32;
        b.iter(|| {
            i = i.wrapping_add(1);
            black_box(calc.calculate_drop(
                black_box(1),
                black_box(50),
                black_box(3),
                black_box(i),
                black_box(i.wrapping_mul(2)),
            ))
        });
    });
}

fn benchmark_million_drops(c: &mut Criterion) {
    let mut calc = LootCalculator::new();
    calc.register_table(create_test_table());

    let mut group = c.benchmark_group("million_drops");
    group.throughput(Throughput::Elements(1_000_000));
    group.sample_size(10);

    group.bench_function("1M_drops", |b| {
        b.iter(|| {
            for i in 0..1_000_000u32 {
                black_box(calc.calculate_drop(
                    1,
                    50,
                    3,
                    i,
                    i.wrapping_mul(0x9E3779B9),
                ));
            }
        });
    });

    group.finish();
}

fn benchmark_statistics(c: &mut Criterion) {
    let mut calc = LootCalculator::new();
    calc.register_table(create_test_table());

    c.bench_function("statistics_100k", |b| {
        b.iter(|| {
            black_box(calc.run_statistics(
                black_box(1),
                black_box(50),
                black_box(3),
                black_box(100_000),
            ))
        });
    });
}

criterion_group!(
    benches,
    benchmark_single_drop,
    benchmark_million_drops,
    benchmark_statistics
);
criterion_main!(benches);
