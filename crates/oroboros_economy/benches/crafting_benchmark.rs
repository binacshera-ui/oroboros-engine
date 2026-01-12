//! Benchmark for crafting system performance.
//!
//! Run with: cargo bench --package oroboros_economy --bench crafting_benchmark

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oroboros_economy::crafting::{CraftingGraph, Recipe, RecipeItem};
use oroboros_economy::inventory::Inventory;

fn create_test_graph() -> CraftingGraph {
    let mut graph = CraftingGraph::new();

    // Add 100 recipes with varying complexity
    for i in 0..100u32 {
        let inputs = vec![
            RecipeItem::new(i * 10, (i % 5) + 1),
            RecipeItem::new(i * 10 + 1, (i % 3) + 1),
        ];
        let outputs = vec![RecipeItem::new(i * 10 + 5, 1)];

        let recipe = Recipe::new(i, format!("Recipe_{i}"), inputs, outputs)
            .unwrap()
            .with_level((i % 50) as u8);

        graph.add_recipe(recipe).unwrap();
    }

    graph
}

fn benchmark_cycle_detection(c: &mut Criterion) {
    let mut graph = create_test_graph();

    c.bench_function("cycle_detection_100_recipes", |b| {
        b.iter(|| {
            // Force re-validation
            graph.validated = false;
            black_box(graph.validate_no_cycles())
        });
    });
}

fn benchmark_can_craft(c: &mut Criterion) {
    let graph = create_test_graph();
    let mut inventory = Inventory::new();

    // Fill inventory with materials
    for i in 0..100u32 {
        inventory.add(i * 10, 100, 64).unwrap();
        inventory.add(i * 10 + 1, 100, 64).unwrap();
    }

    c.bench_function("can_craft_check", |b| {
        let mut i = 0u32;
        b.iter(|| {
            i = (i + 1) % 100;
            black_box(graph.can_craft(&inventory, i, 100))
        });
    });
}

fn benchmark_craft_transaction(c: &mut Criterion) {
    let graph = create_test_graph();

    c.bench_function("craft_with_rollback_potential", |b| {
        b.iter(|| {
            let mut inventory = Inventory::new();
            // Add materials for recipe 0
            inventory.add(0, 10, 64).unwrap();
            inventory.add(1, 10, 64).unwrap();

            // Craft
            black_box(graph.craft(&mut inventory, 0, 100))
        });
    });
}

fn benchmark_snapshot_restore(c: &mut Criterion) {
    let mut inventory = Inventory::new();
    for i in 0..50 {
        inventory.add(i, 64, 64).unwrap();
    }

    c.bench_function("inventory_snapshot_restore", |b| {
        b.iter(|| {
            let snapshot = inventory.snapshot();
            inventory.remove(0, 10).unwrap();
            inventory.restore(black_box(&snapshot));
        });
    });
}

criterion_group!(
    benches,
    benchmark_cycle_detection,
    benchmark_can_craft,
    benchmark_craft_transaction,
    benchmark_snapshot_restore
);
criterion_main!(benches);
