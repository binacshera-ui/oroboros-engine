//! # Dragon State Machine
//!
//! The Dragon is an algorithmic boss that responds to market data.
//! It represents the LIQUIDITY POOL in the game world.
//!
//! ## States
//!
//! - **SLEEP**: Market is calm, volatility < 20%. Dragon sleeps, players can mine.
//! - **STALK**: Volatility rising (20-60%). Dragon watches, occasional attacks.
//! - **INFERNO**: Market crash/spike (>60%). Dragon rampages, liquidation event.
//!
//! ## Determinism
//!
//! All state transitions are deterministic based on market data.
//! All clients MUST see the same transition at the EXACT same tick.

use oroboros_networking::protocol::DragonState;
use std::time::Instant;

/// Market data sample for dragon AI.
#[derive(Clone, Copy, Debug, Default)]
pub struct MarketData {
    /// Current ETH price in USD (cents).
    pub eth_price_cents: u64,
    /// 24h price change percentage (basis points, e.g., 500 = 5%).
    pub change_24h_bps: i32,
    /// Current volatility index (0-10000, where 10000 = 100%).
    pub volatility_index: u32,
    /// Timestamp of this data (Unix epoch ms).
    pub timestamp_ms: u64,
}

impl MarketData {
    /// Creates market data from price and volatility.
    #[must_use]
    pub fn new(eth_price: f64, volatility_percent: f32) -> Self {
        Self {
            eth_price_cents: (eth_price * 100.0) as u64,
            change_24h_bps: 0,
            volatility_index: (volatility_percent * 100.0) as u32,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        }
    }

    /// Returns volatility as a percentage (0-100).
    #[must_use]
    pub fn volatility_percent(&self) -> f32 {
        self.volatility_index as f32 / 100.0
    }
}

/// Configuration for dragon behavior.
#[derive(Clone, Debug)]
pub struct DragonConfig {
    /// Volatility threshold for STALK state (%).
    pub stalk_threshold: f32,
    /// Volatility threshold for INFERNO state (%).
    pub inferno_threshold: f32,
    /// Minimum ticks in each state before transition.
    pub min_state_duration: u32,
    /// Aggression increase rate per tick in INFERNO.
    pub aggression_rate: u8,
    /// Maximum broadcast delay in microseconds.
    pub max_broadcast_delay_us: u64,
}

impl Default for DragonConfig {
    fn default() -> Self {
        Self {
            stalk_threshold: 20.0,
            inferno_threshold: 60.0,
            min_state_duration: 60, // 1 second at 60Hz
            aggression_rate: 2,
            max_broadcast_delay_us: 5000, // 5ms as specified by ARCHITECT
        }
    }
}

/// The Dragon State Machine.
///
/// This is the brain of the algorithmic dragon.
/// It receives market data and outputs deterministic state changes.
pub struct DragonStateMachine {
    /// Current state.
    state: DragonState,
    /// Configuration.
    config: DragonConfig,
    /// Current tick.
    tick: u32,
    /// Tick when current state was entered.
    state_entered_tick: u32,
    /// Current market data.
    market_data: MarketData,
    /// Position in the arena.
    position: (f32, f32),
    /// Target player ID (for stalking/attacking).
    target_player: Option<u32>,
    /// Last state change time (for latency measurement).
    last_change_time: Option<Instant>,
    /// Broadcast latency of last state change (microseconds).
    last_broadcast_latency_us: u64,
}

impl DragonStateMachine {
    /// Creates a new dragon state machine.
    #[must_use]
    pub fn new(config: DragonConfig) -> Self {
        Self {
            state: DragonState::new(0, DragonState::STATE_SLEEP),
            config,
            tick: 0,
            state_entered_tick: 0,
            market_data: MarketData::default(),
            position: (0.0, 0.0),
            target_player: None,
            last_change_time: None,
            last_broadcast_latency_us: 0,
        }
    }

    /// Returns the current state.
    #[must_use]
    pub fn state(&self) -> DragonState {
        DragonState {
            tick: self.tick,
            state: self.state.state,
            aggression: self.state.aggression,
            target_id: self.target_player.unwrap_or(0) as u16,
            pos_x: self.position.0,
            pos_z: self.position.1,
        }
    }

    /// Updates the dragon with new market data.
    ///
    /// Returns true if state changed (requires broadcast).
    pub fn update(&mut self, tick: u32, market_data: MarketData) -> bool {
        self.tick = tick;
        self.market_data = market_data;
        self.state.tick = tick;

        let volatility = market_data.volatility_percent();
        let ticks_in_state = tick.saturating_sub(self.state_entered_tick);

        // Determine target state based on market
        let target_state = if volatility >= self.config.inferno_threshold {
            DragonState::STATE_INFERNO
        } else if volatility >= self.config.stalk_threshold {
            DragonState::STATE_STALK
        } else {
            DragonState::STATE_SLEEP
        };

        // Check if we should transition
        let should_transition = target_state != self.state.state
            && ticks_in_state >= self.config.min_state_duration;

        if should_transition {
            self.transition_to(target_state);
            return true;
        }

        // Update aggression in current state
        self.update_aggression(volatility);

        // Update position based on state
        self.update_position();

        false
    }

    /// Transitions to a new state.
    fn transition_to(&mut self, new_state: u8) {
        let old_state = self.state.state;
        self.state.state = new_state;
        self.state_entered_tick = self.tick;
        self.last_change_time = Some(Instant::now());

        // Log the transition
        tracing::info!(
            "Dragon state transition: {} -> {} at tick {} (volatility: {:.1}%)",
            state_name(old_state),
            state_name(new_state),
            self.tick,
            self.market_data.volatility_percent()
        );

        // Reset aggression on sleep
        if new_state == DragonState::STATE_SLEEP {
            self.state.aggression = 0;
            self.target_player = None;
        }
    }

    /// Updates aggression level.
    fn update_aggression(&mut self, volatility: f32) {
        match self.state.state {
            DragonState::STATE_SLEEP => {
                self.state.aggression = 0;
            }
            DragonState::STATE_STALK => {
                // Aggression based on volatility
                self.state.aggression = ((volatility / 100.0) * 128.0) as u8;
            }
            DragonState::STATE_INFERNO => {
                // Aggression increases over time
                self.state.aggression = self.state.aggression
                    .saturating_add(self.config.aggression_rate)
                    .min(255);
            }
            _ => {}
        }
    }

    /// Updates dragon position.
    fn update_position(&mut self) {
        const ARENA_SIZE: f32 = 100.0;
        const DT: f32 = 1.0 / 60.0;

        match self.state.state {
            DragonState::STATE_SLEEP => {
                // Stay at center
                self.position = (0.0, 0.0);
            }
            DragonState::STATE_STALK => {
                // Circle around arena
                let angle = self.tick as f32 * 0.02;
                let radius = ARENA_SIZE * 0.6;
                self.position = (angle.cos() * radius, angle.sin() * radius);
            }
            DragonState::STATE_INFERNO => {
                // Rapid movement toward target
                if let Some(_target) = self.target_player {
                    // Would chase target here
                    // For now, aggressive circling
                    let angle = self.tick as f32 * 0.1;
                    let radius = ARENA_SIZE * 0.4;
                    self.position = (angle.cos() * radius, angle.sin() * radius);
                }
            }
            _ => {}
        }
    }

    /// Sets the target player.
    pub fn set_target(&mut self, player_id: Option<u32>) {
        self.target_player = player_id;
    }

    /// Records broadcast completion for latency tracking.
    pub fn record_broadcast_complete(&mut self) {
        if let Some(change_time) = self.last_change_time {
            self.last_broadcast_latency_us = change_time.elapsed().as_micros() as u64;
            self.last_change_time = None;

            // Check against ARCHITECT's requirement
            if self.last_broadcast_latency_us > self.config.max_broadcast_delay_us {
                tracing::warn!(
                    "Dragon broadcast latency {} μs exceeds limit {} μs",
                    self.last_broadcast_latency_us,
                    self.config.max_broadcast_delay_us
                );
            }
        }
    }

    /// Returns the last broadcast latency.
    #[must_use]
    pub const fn last_broadcast_latency_us(&self) -> u64 {
        self.last_broadcast_latency_us
    }

    /// Returns true if broadcast latency is within spec.
    #[must_use]
    pub fn is_latency_ok(&self) -> bool {
        self.last_broadcast_latency_us <= self.config.max_broadcast_delay_us
    }

    /// Returns the current market data.
    #[must_use]
    pub const fn market_data(&self) -> &MarketData {
        &self.market_data
    }

    /// Returns ticks spent in current state.
    #[must_use]
    pub fn ticks_in_state(&self) -> u32 {
        self.tick.saturating_sub(self.state_entered_tick)
    }
}

impl Default for DragonStateMachine {
    fn default() -> Self {
        Self::new(DragonConfig::default())
    }
}

/// Returns a human-readable state name.
fn state_name(state: u8) -> &'static str {
    match state {
        DragonState::STATE_SLEEP => "SLEEP",
        DragonState::STATE_STALK => "STALK",
        DragonState::STATE_INFERNO => "INFERNO",
        _ => "UNKNOWN",
    }
}

/// Mock market data source for testing.
pub struct MockMarketDataSource {
    /// Base ETH price.
    base_price: f64,
    /// Current tick.
    tick: u32,
    /// Volatility pattern type.
    pattern: VolatilityPattern,
}

/// Pattern for volatility simulation.
#[derive(Clone, Copy, Debug)]
pub enum VolatilityPattern {
    /// Calm market, low volatility.
    Calm,
    /// Rising volatility.
    Rising,
    /// High volatility crash.
    Crash,
    /// Sine wave pattern (for testing).
    Sine,
    /// Load from CSV file pattern index.
    CsvPattern(usize),
}

impl MockMarketDataSource {
    /// Creates a new mock data source.
    #[must_use]
    pub fn new(base_price: f64, pattern: VolatilityPattern) -> Self {
        Self {
            base_price,
            tick: 0,
            pattern,
        }
    }

    /// Gets market data for the current tick.
    pub fn get_data(&mut self) -> MarketData {
        self.tick += 1;
        
        let volatility = match self.pattern {
            VolatilityPattern::Calm => 10.0,
            VolatilityPattern::Rising => {
                // Gradually increase over 10 seconds
                let t = (self.tick as f32 / 600.0).min(1.0);
                10.0 + t * 60.0
            }
            VolatilityPattern::Crash => {
                // Sudden spike after 5 seconds
                if self.tick > 300 { 80.0 } else { 15.0 }
            }
            VolatilityPattern::Sine => {
                // Sine wave between 10% and 80%
                let t = self.tick as f32 / 120.0;
                45.0 + t.sin() * 35.0
            }
            VolatilityPattern::CsvPattern(_) => {
                // Would load from CSV
                30.0
            }
        };

        let price_factor = 1.0 + (volatility - 50.0) / 100.0 * 0.1;
        let price = self.base_price * price_factor;

        MarketData::new(price, volatility)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let config = DragonConfig {
            stalk_threshold: 20.0,
            inferno_threshold: 60.0,
            min_state_duration: 0, // Instant transitions for testing
            ..Default::default()
        };

        let mut dragon = DragonStateMachine::new(config);

        // Start sleeping
        assert_eq!(dragon.state().state, DragonState::STATE_SLEEP);

        // Low volatility - stay sleeping
        let data = MarketData::new(2000.0, 10.0);
        dragon.update(1, data);
        assert_eq!(dragon.state().state, DragonState::STATE_SLEEP);

        // Medium volatility - stalk
        let data = MarketData::new(2000.0, 40.0);
        let changed = dragon.update(2, data);
        assert!(changed);
        assert_eq!(dragon.state().state, DragonState::STATE_STALK);

        // High volatility - inferno
        let data = MarketData::new(2000.0, 80.0);
        let changed = dragon.update(3, data);
        assert!(changed);
        assert_eq!(dragon.state().state, DragonState::STATE_INFERNO);

        // Back to calm
        let data = MarketData::new(2000.0, 5.0);
        let changed = dragon.update(4, data);
        assert!(changed);
        assert_eq!(dragon.state().state, DragonState::STATE_SLEEP);
    }

    #[test]
    fn test_min_state_duration() {
        let config = DragonConfig {
            min_state_duration: 60,
            ..Default::default()
        };

        let mut dragon = DragonStateMachine::new(config);

        // Start sleeping at tick 0
        dragon.update(0, MarketData::new(2000.0, 10.0));

        // Try to transition at tick 30 - too early
        let data = MarketData::new(2000.0, 80.0);
        let changed = dragon.update(30, data);
        assert!(!changed);
        assert_eq!(dragon.state().state, DragonState::STATE_SLEEP);

        // Transition at tick 60 - enough time passed
        let data = MarketData::new(2000.0, 80.0);
        let changed = dragon.update(60, data);
        assert!(changed);
        assert_eq!(dragon.state().state, DragonState::STATE_INFERNO);
    }

    #[test]
    fn test_aggression_increase() {
        let mut dragon = DragonStateMachine::new(DragonConfig {
            min_state_duration: 0,
            aggression_rate: 5,
            ..Default::default()
        });

        // Enter inferno
        dragon.update(0, MarketData::new(2000.0, 80.0));
        let initial_aggression = dragon.state().aggression;

        // Update several times
        for i in 1..10 {
            dragon.update(i, MarketData::new(2000.0, 80.0));
        }

        // Aggression should have increased
        assert!(dragon.state().aggression > initial_aggression);
    }

    #[test]
    fn test_determinism() {
        // Two dragons with same inputs should produce identical outputs
        let config = DragonConfig::default();
        let mut dragon1 = DragonStateMachine::new(config.clone());
        let mut dragon2 = DragonStateMachine::new(config);

        let data_sequence = [
            MarketData::new(2000.0, 10.0),
            MarketData::new(2000.0, 30.0),
            MarketData::new(2000.0, 70.0),
            MarketData::new(2000.0, 50.0),
        ];

        for (i, data) in data_sequence.iter().enumerate() {
            dragon1.update(i as u32 * 100, *data);
            dragon2.update(i as u32 * 100, *data);

            assert_eq!(dragon1.state().state, dragon2.state().state);
            assert_eq!(dragon1.state().aggression, dragon2.state().aggression);
        }
    }

    #[test]
    fn test_mock_data_source() {
        let mut source = MockMarketDataSource::new(2000.0, VolatilityPattern::Sine);

        let mut min_vol = f32::MAX;
        let mut max_vol = f32::MIN;

        for _ in 0..600 {
            let data = source.get_data();
            let vol = data.volatility_percent();
            min_vol = min_vol.min(vol);
            max_vol = max_vol.max(vol);
        }

        // Sine should oscillate
        assert!(max_vol - min_vol > 50.0);
    }
}
