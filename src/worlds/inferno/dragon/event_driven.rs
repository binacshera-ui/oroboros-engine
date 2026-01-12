//! # Event-Driven Dragon
//!
//! THE ARCHITECT DEMANDS: Zero arbitrage window.
//!
//! The dragon cannot be bound to the 60Hz game loop.
//! When market crashes, it must react in MICROSECONDS, not milliseconds.
//!
//! ## Architecture
//!
//! ```text
//! Market Feed ──┬──> Dragon (Event-Driven) ──> Immediate Broadcast
//!               │                              (bypasses tick loop)
//!               └──> Game Loop (60Hz) ──> Normal updates
//! ```
//!
//! The dragon runs on its OWN thread, reacting to market events immediately.

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};

/// Market event that triggers dragon state change.
#[derive(Clone, Copy, Debug)]
pub struct MarketEvent {
    /// Event timestamp (nanoseconds since epoch).
    pub timestamp_ns: u64,
    /// ETH price in cents.
    pub eth_price_cents: u64,
    /// Volatility index (0-10000 = 0-100%).
    pub volatility_bps: u32,
    /// Type of event.
    pub event_type: MarketEventType,
}

/// Type of market event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MarketEventType {
    /// Normal tick (periodic update).
    Tick = 0,
    /// Price spike (sudden increase).
    Spike = 1,
    /// Price crash (sudden decrease).
    Crash = 2,
    /// Volatility surge.
    VolatilitySurge = 3,
    /// Liquidation cascade.
    LiquidationCascade = 4,
}

/// Dragon state change broadcast.
#[derive(Clone, Copy, Debug)]
pub struct DragonBroadcast {
    /// Timestamp of state change (nanoseconds).
    pub timestamp_ns: u64,
    /// New state.
    pub state: DragonStateValue,
    /// Aggression level (0-255).
    pub aggression: u8,
    /// Latency from market event to broadcast (nanoseconds).
    pub latency_ns: u64,
}

/// Dragon state values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DragonStateValue {
    /// Sleeping - market calm.
    Sleep = 0,
    /// Stalking - volatility rising.
    Stalk = 1,
    /// Inferno - market crash/spike, liquidation mode.
    Inferno = 2,
}

impl From<u8> for DragonStateValue {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::Sleep,
            1 => Self::Stalk,
            2 => Self::Inferno,
            _ => Self::Sleep,
        }
    }
}

/// Shared dragon state (lock-free).
pub struct SharedDragonState {
    /// Current state (atomic for lock-free access).
    state: AtomicU8,
    /// Aggression level.
    aggression: AtomicU8,
    /// Last update timestamp (nanoseconds).
    last_update_ns: AtomicU64,
    /// Total state changes.
    state_changes: AtomicU64,
    /// Worst case latency observed (nanoseconds).
    worst_latency_ns: AtomicU64,
}

impl SharedDragonState {
    /// Creates new shared state.
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(DragonStateValue::Sleep as u8),
            aggression: AtomicU8::new(0),
            last_update_ns: AtomicU64::new(0),
            state_changes: AtomicU64::new(0),
            worst_latency_ns: AtomicU64::new(0),
        }
    }

    /// Gets current state (lock-free).
    #[inline]
    pub fn state(&self) -> DragonStateValue {
        DragonStateValue::from(self.state.load(Ordering::Relaxed))
    }

    /// Gets aggression level.
    #[inline]
    pub fn aggression(&self) -> u8 {
        self.aggression.load(Ordering::Relaxed)
    }

    /// Gets total state changes.
    pub fn state_changes(&self) -> u64 {
        self.state_changes.load(Ordering::Relaxed)
    }

    /// Gets worst latency in nanoseconds.
    pub fn worst_latency_ns(&self) -> u64 {
        self.worst_latency_ns.load(Ordering::Relaxed)
    }

    /// Updates state (called by dragon thread only).
    fn update(&self, new_state: DragonStateValue, aggression: u8, timestamp_ns: u64, latency_ns: u64) {
        let old_state = self.state.swap(new_state as u8, Ordering::Release);
        self.aggression.store(aggression, Ordering::Release);
        self.last_update_ns.store(timestamp_ns, Ordering::Release);
        
        if old_state != new_state as u8 {
            self.state_changes.fetch_add(1, Ordering::Relaxed);
        }
        
        // Track worst latency
        let mut current_worst = self.worst_latency_ns.load(Ordering::Relaxed);
        while latency_ns > current_worst {
            match self.worst_latency_ns.compare_exchange_weak(
                current_worst,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_worst = actual,
            }
        }
    }
}

impl Default for SharedDragonState {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for event-driven dragon.
#[derive(Clone, Debug)]
pub struct EventDragonConfig {
    /// Volatility threshold for STALK (basis points).
    pub stalk_threshold_bps: u32,
    /// Volatility threshold for INFERNO (basis points).
    pub inferno_threshold_bps: u32,
    /// Maximum allowed latency from event to broadcast (nanoseconds).
    pub max_latency_ns: u64,
    /// Cooldown between state changes (nanoseconds).
    pub state_change_cooldown_ns: u64,
}

impl Default for EventDragonConfig {
    fn default() -> Self {
        Self {
            stalk_threshold_bps: 2000,  // 20%
            inferno_threshold_bps: 6000, // 60%
            max_latency_ns: 1_000_000,   // 1ms max (ARCHITECT demanded <5ms)
            state_change_cooldown_ns: 100_000_000, // 100ms cooldown
        }
    }
}

/// Event-driven dragon controller.
///
/// This runs on its own thread, reacting to market events IMMEDIATELY.
/// It does NOT wait for the game tick loop.
pub struct EventDrivenDragon {
    /// Configuration.
    config: EventDragonConfig,
    /// Shared state (read by game loop, written by dragon thread).
    shared_state: Arc<SharedDragonState>,
    /// Channel to receive market events.
    event_rx: Receiver<MarketEvent>,
    /// Channel to send broadcasts.
    broadcast_tx: Sender<DragonBroadcast>,
    /// Last state change timestamp.
    last_state_change_ns: u64,
    /// Running flag.
    running: bool,
}

impl EventDrivenDragon {
    /// Creates a new event-driven dragon.
    pub fn new(
        config: EventDragonConfig,
        shared_state: Arc<SharedDragonState>,
        event_rx: Receiver<MarketEvent>,
        broadcast_tx: Sender<DragonBroadcast>,
    ) -> Self {
        Self {
            config,
            shared_state,
            event_rx,
            broadcast_tx,
            last_state_change_ns: 0,
            running: true,
        }
    }

    /// Runs the dragon event loop (call this in a dedicated thread).
    pub fn run(&mut self) {
        while self.running {
            match self.event_rx.try_recv() {
                Ok(event) => {
                    self.handle_event(event);
                }
                Err(TryRecvError::Empty) => {
                    // No events, spin-wait briefly
                    std::hint::spin_loop();
                }
                Err(TryRecvError::Disconnected) => {
                    self.running = false;
                }
            }
        }
    }

    /// Handles a market event IMMEDIATELY.
    fn handle_event(&mut self, event: MarketEvent) {
        let start = Instant::now();
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        // Determine target state based on volatility
        let target_state = if event.volatility_bps >= self.config.inferno_threshold_bps {
            DragonStateValue::Inferno
        } else if event.volatility_bps >= self.config.stalk_threshold_bps {
            DragonStateValue::Stalk
        } else {
            DragonStateValue::Sleep
        };

        // Check cooldown
        let since_last_change = now_ns.saturating_sub(self.last_state_change_ns);
        let current_state = self.shared_state.state();

        // Calculate aggression
        let aggression = if event.volatility_bps >= self.config.inferno_threshold_bps {
            ((event.volatility_bps - self.config.inferno_threshold_bps) / 40).min(255) as u8
        } else {
            (event.volatility_bps / 100).min(255) as u8
        };

        // State change logic
        let state_changed = target_state != current_state 
            && since_last_change >= self.config.state_change_cooldown_ns;

        if state_changed {
            self.last_state_change_ns = now_ns;
        }

        let final_state = if state_changed { target_state } else { current_state };

        // Update shared state (lock-free)
        let latency_ns = start.elapsed().as_nanos() as u64;
        self.shared_state.update(final_state, aggression, now_ns, latency_ns);

        // Broadcast if state changed
        if state_changed {
            let broadcast = DragonBroadcast {
                timestamp_ns: now_ns,
                state: target_state,
                aggression,
                latency_ns,
            };

            // Non-blocking send
            let _ = self.broadcast_tx.try_send(broadcast);

            // Check latency requirement
            if latency_ns > self.config.max_latency_ns {
                // Log warning - this should NEVER happen
                eprintln!(
                    "CRITICAL: Dragon latency {} ns exceeds max {} ns!",
                    latency_ns, self.config.max_latency_ns
                );
            }
        }
    }

    /// Stops the dragon.
    pub fn stop(&mut self) {
        self.running = false;
    }
}

/// Creates the event-driven dragon system.
///
/// Returns:
/// - Shared state (for game loop to read)
/// - Event sender (for market feed to send events)
/// - Broadcast receiver (for network to broadcast state changes)
pub fn create_event_dragon_system(
    config: EventDragonConfig,
) -> (
    Arc<SharedDragonState>,
    Sender<MarketEvent>,
    Receiver<DragonBroadcast>,
    std::thread::JoinHandle<()>,
) {
    let shared_state = Arc::new(SharedDragonState::new());
    let (event_tx, event_rx) = bounded(1000);
    let (broadcast_tx, broadcast_rx) = bounded(100);

    let state_clone = Arc::clone(&shared_state);
    
    let handle = std::thread::Builder::new()
        .name("dragon-event-loop".into())
        .spawn(move || {
            let mut dragon = EventDrivenDragon::new(
                config,
                state_clone,
                event_rx,
                broadcast_tx,
            );
            dragon.run();
        })
        .expect("Failed to spawn dragon thread");

    (shared_state, event_tx, broadcast_rx, handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_immediate_response() {
        let config = EventDragonConfig {
            stalk_threshold_bps: 2000,
            inferno_threshold_bps: 6000,
            max_latency_ns: 1_000_000, // 1ms
            state_change_cooldown_ns: 0, // No cooldown for test
        };

        let (shared_state, event_tx, broadcast_rx, _handle) = create_event_dragon_system(config);

        // Send crash event
        let event = MarketEvent {
            timestamp_ns: 0,
            eth_price_cents: 200000,
            volatility_bps: 8000, // 80% volatility = INFERNO
            event_type: MarketEventType::Crash,
        };

        event_tx.send(event).unwrap();

        // Wait briefly for processing
        std::thread::sleep(Duration::from_micros(100));

        // Check state changed to INFERNO
        assert_eq!(shared_state.state(), DragonStateValue::Inferno);

        // Check broadcast was sent
        let broadcast = broadcast_rx.try_recv().unwrap();
        assert_eq!(broadcast.state, DragonStateValue::Inferno);
        
        // Check latency is under 1ms
        assert!(broadcast.latency_ns < 1_000_000, 
            "Latency {} ns exceeds 1ms!", broadcast.latency_ns);

        println!("Dragon response latency: {} ns ({:.3} μs)", 
            broadcast.latency_ns, broadcast.latency_ns as f64 / 1000.0);
    }

    #[test]
    fn test_no_arbitrage_window() {
        let config = EventDragonConfig {
            max_latency_ns: 1_000_000,
            state_change_cooldown_ns: 0,
            ..Default::default()
        };

        let (shared_state, event_tx, broadcast_rx, _handle) = create_event_dragon_system(config);

        // Measure time from event send to state change
        let start = Instant::now();

        let event = MarketEvent {
            timestamp_ns: 0,
            eth_price_cents: 200000,
            volatility_bps: 9000,
            event_type: MarketEventType::LiquidationCascade,
        };

        event_tx.send(event).unwrap();

        // Poll until state changes
        loop {
            if shared_state.state() == DragonStateValue::Inferno {
                break;
            }
            if start.elapsed() > Duration::from_millis(10) {
                panic!("Dragon took too long to respond!");
            }
            std::hint::spin_loop();
        }

        let response_time = start.elapsed();
        println!("Total response time: {:?}", response_time);

        // Must be under 1ms (the game tick is 16ms, we need to be WAY faster)
        assert!(response_time < Duration::from_millis(1),
            "Response time {:?} creates arbitrage window!", response_time);
    }
}
