//! # Event Listener
//!
//! High-performance blockchain event listener.
//! Designed for sub-5ms E2E latency.

use crossbeam_channel::{Receiver, Sender, bounded};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use super::events::{BlockchainEvent, EventParser, NFTStateChange, NFTTransfer};

/// Configuration for the event listener.
#[derive(Clone, Debug)]
pub struct ListenerConfig {
    /// RPC endpoint URL.
    pub rpc_url: String,
    /// Contract address to watch.
    pub contract_address: [u8; 20],
    /// Channel buffer size for events.
    pub channel_buffer: usize,
    /// Poll interval in milliseconds (for non-websocket).
    pub poll_interval_ms: u64,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://127.0.0.1:8545".to_string(),
            contract_address: [0u8; 20],
            channel_buffer: 1024,
            poll_interval_ms: 10,
        }
    }
}

/// Statistics for the event listener.
#[derive(Debug, Default)]
pub struct ListenerStats {
    /// Total events received.
    pub events_received: AtomicU64,
    /// Total events processed.
    pub events_processed: AtomicU64,
    /// Average latency in microseconds.
    pub avg_latency_us: AtomicU64,
    /// Maximum latency in microseconds.
    pub max_latency_us: AtomicU64,
}

/// High-performance blockchain event listener.
///
/// This listener is designed for minimal latency:
/// - Uses channels for lock-free event passing
/// - Pre-parses events to avoid allocation in hot path
/// - Tracks latency statistics
///
/// # Architecture
///
/// ```text
/// ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
/// │   RPC/WS     │ ──▶ │   Listener   │ ──▶ │   Channel    │ ──▶ Game
/// │   Node       │     │   (Parser)   │     │   (Bounded)  │
/// └──────────────┘     └──────────────┘     └──────────────┘
/// ```
pub struct EventListener {
    /// Sender side of event channel.
    sender: Sender<(BlockchainEvent, Instant)>,
    /// Receiver side of event channel.
    receiver: Receiver<(BlockchainEvent, Instant)>,
    /// Whether the listener is running.
    running: Arc<AtomicBool>,
    /// Performance statistics.
    stats: Arc<ListenerStats>,
    /// Configuration.
    #[allow(dead_code)]
    config: ListenerConfig,
}

impl EventListener {
    /// Creates a new event listener.
    ///
    /// # Arguments
    ///
    /// * `config` - Listener configuration
    #[must_use]
    pub fn new(config: ListenerConfig) -> Self {
        let (sender, receiver) = bounded(config.channel_buffer);

        Self {
            sender,
            receiver,
            running: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(ListenerStats::default()),
            config,
        }
    }

    /// Returns a clone of the event receiver.
    ///
    /// The game loop should poll this receiver for events.
    #[must_use]
    pub fn receiver(&self) -> Receiver<(BlockchainEvent, Instant)> {
        self.receiver.clone()
    }

    /// Returns a reference to the statistics.
    #[must_use]
    pub fn stats(&self) -> Arc<ListenerStats> {
        Arc::clone(&self.stats)
    }

    /// Checks if the listener is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Stops the listener.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Simulates receiving an event (for testing/benchmarking).
    ///
    /// This bypasses the actual RPC and directly injects an event
    /// into the channel with timestamp for latency measurement.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to inject
    ///
    /// # Returns
    ///
    /// `true` if the event was sent, `false` if channel is full.
    pub fn inject_event(&self, event: BlockchainEvent) -> bool {
        let timestamp = Instant::now();
        self.sender.try_send((event, timestamp)).is_ok()
    }

    /// Processes a raw log and sends parsed event to channel.
    ///
    /// # Arguments
    ///
    /// * `topics` - Event topics
    /// * `data` - Event data
    /// * `block_number` - Block number
    /// * `tx_index` - Transaction index
    /// * `log_index` - Log index
    ///
    /// # Returns
    ///
    /// `true` if event was parsed and sent successfully.
    pub fn process_raw_log(
        &self,
        topics: &[[u8; 32]],
        data: &[u8],
        block_number: u64,
        tx_index: u32,
        log_index: u32,
    ) -> bool {
        let timestamp = Instant::now();

        // Try to parse as StateChanged first
        if let Some(state_change) = EventParser::parse_state_changed(
            topics,
            data,
            block_number,
            tx_index,
            log_index,
        ) {
            let event = BlockchainEvent::NFTStateChanged(state_change);
            return self.sender.try_send((event, timestamp)).is_ok();
        }

        // Try to parse as Transfer
        if let Some(transfer) = EventParser::parse_transfer(topics, block_number) {
            let event = BlockchainEvent::NFTTransfer(transfer);
            return self.sender.try_send((event, timestamp)).is_ok();
        }

        false
    }

    /// Records latency statistics.
    ///
    /// Call this after processing an event with its original timestamp.
    ///
    /// # Arguments
    ///
    /// * `event_timestamp` - When the event was received
    pub fn record_latency(&self, event_timestamp: Instant) {
        let latency_us = event_timestamp.elapsed().as_micros() as u64;

        self.stats.events_processed.fetch_add(1, Ordering::Relaxed);

        // Update max latency
        let _ = self.stats.max_latency_us.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |current| {
                if latency_us > current {
                    Some(latency_us)
                } else {
                    None
                }
            },
        );

        // Simple moving average (not perfectly accurate but fast)
        let count = self.stats.events_processed.load(Ordering::Relaxed);
        let current_avg = self.stats.avg_latency_us.load(Ordering::Relaxed);
        let new_avg = if count == 1 {
            latency_us
        } else {
            // Weighted average favoring recent values
            (current_avg * 7 + latency_us) / 8
        };
        self.stats.avg_latency_us.store(new_avg, Ordering::Relaxed);
    }
}

/// Simulated event generator for testing.
///
/// Creates realistic events for benchmarking without network I/O.
pub struct EventSimulator {
    /// Next token ID to use.
    next_token_id: u64,
}

impl EventSimulator {
    /// Creates a new event simulator.
    #[must_use]
    pub const fn new() -> Self {
        Self { next_token_id: 1 }
    }

    /// Generates a state change event.
    #[must_use]
    pub fn generate_state_change(&mut self, block_number: u64) -> NFTStateChange {
        let token_id = self.next_token_id;
        self.next_token_id += 1;

        NFTStateChange {
            token_id: alloy_primitives::U256::from(token_id),
            owner: alloy_primitives::Address::ZERO,
            evolution_stage: (token_id % 5) as u8,
            experience: (token_id * 100) as u32,
            strength: (token_id % 1000) as u16,
            agility: ((token_id + 1) % 1000) as u16,
            intelligence: ((token_id + 2) % 1000) as u16,
            visual_dna: [token_id as u8; 32],
            block_number,
            tx_index: 0,
            log_index: 0,
        }
    }

    /// Generates a transfer event.
    #[must_use]
    pub fn generate_transfer(&mut self, block_number: u64) -> NFTTransfer {
        let token_id = self.next_token_id;
        self.next_token_id += 1;

        NFTTransfer {
            token_id: alloy_primitives::U256::from(token_id),
            from: alloy_primitives::Address::ZERO,
            to: alloy_primitives::Address::repeat_byte(1),
            block_number,
        }
    }
}

impl Default for EventSimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_listener_creation() {
        let config = ListenerConfig::default();
        let listener = EventListener::new(config);

        assert!(!listener.is_running());
    }

    #[test]
    fn test_event_injection() {
        let config = ListenerConfig::default();
        let listener = EventListener::new(config);
        let receiver = listener.receiver();

        let mut simulator = EventSimulator::new();
        let state_change = simulator.generate_state_change(1);
        let event = BlockchainEvent::NFTStateChanged(state_change);

        assert!(listener.inject_event(event));

        let (received, _timestamp) = receiver.try_recv().unwrap();
        match received {
            BlockchainEvent::NFTStateChanged(sc) => {
                assert_eq!(sc.block_number, 1);
            }
            _ => panic!("Wrong event type"),
        }
    }
}
