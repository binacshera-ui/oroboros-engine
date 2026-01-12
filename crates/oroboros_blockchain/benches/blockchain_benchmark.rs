//! # Blockchain Performance Benchmark
//!
//! ARCHITECT'S REQUIREMENTS:
//! - Event parsing: < 1ms
//! - State update: < 1ms
//! - Total E2E: < 5ms
//!
//! Run with: `cargo bench --package oroboros_blockchain`

// Benchmarks don't need strict docs
#![allow(missing_docs)]
#![allow(dead_code)]

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Instant;

use oroboros_blockchain::{
    BlockchainEvent, NFTStateChange, ChainSyncedState, EventListener,
    ListenerConfig, EventSimulator,
};
use oroboros_blockchain::events::EventParser;

/// Benchmark: Event parsing speed.
fn bench_event_parsing(c: &mut Criterion) {
    // Create realistic raw log data
    let topics = [
        [0u8; 32], // Event signature
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1], // tokenId
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1], // owner
    ];

    // 192 bytes of event data
    let mut data = vec![0u8; 192];
    data[31] = 2; // evolutionStage
    data[63] = 100; // experience (lower byte)
    data[95] = 50; // strength (lower byte)

    c.bench_function("parse_state_changed_event", |b| {
        b.iter(|| {
            black_box(EventParser::parse_state_changed(
                &topics,
                &data,
                12345,
                0,
                0,
            ))
        });
    });
}

/// Benchmark: State update speed.
fn bench_state_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_update");

    // Test with different state sizes
    for size in [100, 1000, 10000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                let mut state = ChainSyncedState::new(size * 2, size);
                let mut simulator = EventSimulator::new();

                // Pre-populate state
                for i in 0..size {
                    let change = simulator.generate_state_change(i as u64);
                    state.process_event(&BlockchainEvent::NFTStateChanged(change));
                }

                // Benchmark updating existing NFTs
                let mut i = 0u64;
                b.iter(|| {
                    let change = NFTStateChange {
                        token_id: alloy_primitives::U256::from(i % size as u64 + 1),
                        owner: alloy_primitives::Address::repeat_byte((i % 255) as u8),
                        evolution_stage: (i % 5) as u8,
                        experience: i as u32,
                        strength: (i % 1000) as u16,
                        agility: (i % 1000) as u16,
                        intelligence: (i % 1000) as u16,
                        visual_dna: [0u8; 32],
                        block_number: i,
                        tx_index: 0,
                        log_index: 0,
                    };
                    state.process_event(&BlockchainEvent::NFTStateChanged(change));
                    i += 1;
                    black_box(state.nft_count())
                });
            },
        );
    }

    group.finish();
}

/// THE CRITICAL BENCHMARK: Full E2E latency measurement.
fn bench_e2e_latency(c: &mut Criterion) {
    c.bench_function("CRITICAL_e2e_latency", |b| {
        let config = ListenerConfig {
            channel_buffer: 1024,
            ..Default::default()
        };
        let listener = EventListener::new(config);
        let receiver = listener.receiver();
        let mut state = ChainSyncedState::default();
        let mut simulator = EventSimulator::new();

        b.iter(|| {
            // 1. Generate event (simulates chain event)
            let start = Instant::now();
            let change = simulator.generate_state_change(1);
            let event = BlockchainEvent::NFTStateChanged(change);

            // 2. Send through channel (simulates network delivery)
            listener.inject_event(event);

            // 3. Receive from channel
            let (received_event, _timestamp) = receiver.recv().unwrap();

            // 4. Process into game state
            state.process_event(&received_event);

            // 5. Measure total time
            let elapsed = start.elapsed();
            black_box(elapsed.as_nanos())
        });
    });
}

/// Benchmark: Batch event processing.
fn bench_batch_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_processing");

    for batch_size in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &batch_size| {
                let mut state = ChainSyncedState::new(batch_size * 2, batch_size);
                let mut simulator = EventSimulator::new();

                b.iter(|| {
                    let events: Vec<BlockchainEvent> = (0..batch_size)
                        .map(|i| {
                            BlockchainEvent::NFTStateChanged(
                                simulator.generate_state_change(i as u64),
                            )
                        })
                        .collect();

                    state.process_batch(events.iter());
                    black_box(state.nft_count())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Channel throughput.
fn bench_channel_throughput(c: &mut Criterion) {
    c.bench_function("channel_throughput_1000", |b| {
        let config = ListenerConfig {
            channel_buffer: 2048,
            ..Default::default()
        };
        let listener = EventListener::new(config);
        let receiver = listener.receiver();
        let mut simulator = EventSimulator::new();

        b.iter(|| {
            // Send 1000 events
            for i in 0..1000 {
                let change = simulator.generate_state_change(i);
                let event = BlockchainEvent::NFTStateChanged(change);
                listener.inject_event(event);
            }

            // Receive all 1000 events
            for _ in 0..1000 {
                let _ = receiver.recv().unwrap();
            }

            black_box(())
        });
    });
}

criterion_group!(
    benches,
    bench_event_parsing,
    bench_state_update,
    bench_e2e_latency,
    bench_batch_processing,
    bench_channel_throughput,
);

criterion_main!(benches);
