//! # Server Tick Loop
//!
//! Fixed-timestep game loop running at 60Hz.
//!
//! ## Design
//!
//! The tick loop must:
//! - Run exactly 60 times per second
//! - Never allocate memory
//! - Process all inputs before updating state
//! - Broadcast state after update

use std::time::{Duration, Instant};
use crate::TICK_DURATION_MICROS;

/// Fixed-timestep tick loop controller.
///
/// Ensures consistent tick rate regardless of processing time.
pub struct TickLoop {
    /// Target tick duration.
    tick_duration: Duration,
    /// Time of last tick.
    last_tick: Instant,
    /// Accumulated time since last tick.
    accumulator: Duration,
    /// Total ticks executed.
    tick_count: u64,
    /// Frame time statistics.
    stats: TickStats,
}

/// Tick timing statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct TickStats {
    /// Minimum tick duration observed.
    pub min_tick_us: u64,
    /// Maximum tick duration observed.
    pub max_tick_us: u64,
    /// Average tick duration (rolling).
    pub avg_tick_us: u64,
    /// Number of late ticks (took longer than budget).
    pub late_ticks: u64,
    /// Total ticks measured.
    pub total_ticks: u64,
}

impl TickLoop {
    /// Creates a new tick loop with the specified rate.
    #[must_use]
    pub fn new(tick_rate: u32) -> Self {
        let tick_duration = Duration::from_micros(1_000_000 / u64::from(tick_rate));
        
        Self {
            tick_duration,
            last_tick: Instant::now(),
            accumulator: Duration::ZERO,
            tick_count: 0,
            stats: TickStats {
                min_tick_us: u64::MAX,
                max_tick_us: 0,
                avg_tick_us: tick_duration.as_micros() as u64,
                late_ticks: 0,
                total_ticks: 0,
            },
        }
    }

    /// Creates a tick loop for Inferno (60Hz).
    #[must_use]
    pub fn inferno() -> Self {
        Self::new(60)
    }

    /// Returns true if it's time to execute a tick.
    ///
    /// Call this in a loop until it returns false.
    #[must_use]
    pub fn should_tick(&mut self) -> bool {
        let now = Instant::now();
        self.accumulator += now.duration_since(self.last_tick);
        self.last_tick = now;
        
        self.accumulator >= self.tick_duration
    }

    /// Marks the start of a tick.
    ///
    /// Returns the tick start time for duration measurement.
    #[must_use]
    pub fn begin_tick(&mut self) -> Instant {
        self.accumulator = self.accumulator.saturating_sub(self.tick_duration);
        self.tick_count += 1;
        Instant::now()
    }

    /// Marks the end of a tick.
    ///
    /// Records statistics about tick duration.
    pub fn end_tick(&mut self, start: Instant) {
        let duration = start.elapsed();
        let duration_us = duration.as_micros() as u64;
        
        // Update stats
        self.stats.total_ticks += 1;
        self.stats.min_tick_us = self.stats.min_tick_us.min(duration_us);
        self.stats.max_tick_us = self.stats.max_tick_us.max(duration_us);
        
        // Rolling average
        self.stats.avg_tick_us = (self.stats.avg_tick_us * 15 + duration_us) / 16;
        
        if duration > self.tick_duration {
            self.stats.late_ticks += 1;
        }
    }

    /// Waits until the next tick is due.
    ///
    /// Uses spin-wait for the final microseconds to ensure accuracy.
    pub fn wait_for_next_tick(&self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick);
        
        if elapsed < self.tick_duration {
            let remaining = self.tick_duration - elapsed;
            
            // Sleep for most of the time
            if remaining > Duration::from_micros(1000) {
                std::thread::sleep(remaining - Duration::from_micros(500));
            }
            
            // Spin-wait for precision
            while Instant::now().duration_since(self.last_tick) < self.tick_duration {
                std::hint::spin_loop();
            }
        }
    }

    /// Returns the current tick count.
    #[must_use]
    pub const fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Returns tick statistics.
    #[must_use]
    pub const fn stats(&self) -> &TickStats {
        &self.stats
    }

    /// Returns the target tick duration.
    #[must_use]
    pub const fn tick_duration(&self) -> Duration {
        self.tick_duration
    }

    /// Resets statistics.
    pub fn reset_stats(&mut self) {
        self.stats = TickStats {
            min_tick_us: u64::MAX,
            max_tick_us: 0,
            avg_tick_us: TICK_DURATION_MICROS,
            late_ticks: 0,
            total_ticks: 0,
        };
    }
}

impl Default for TickLoop {
    fn default() -> Self {
        Self::inferno()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_loop_creation() {
        let tick_loop = TickLoop::new(60);
        assert_eq!(tick_loop.tick_count(), 0);
        assert_eq!(tick_loop.tick_duration(), Duration::from_micros(16666));
    }

    #[test]
    fn test_tick_execution() {
        let mut tick_loop = TickLoop::new(1000); // 1000Hz for faster test
        
        // Wait a bit
        std::thread::sleep(Duration::from_millis(5));
        
        // Should have ticks pending
        assert!(tick_loop.should_tick());
        
        let start = tick_loop.begin_tick();
        // Do nothing
        tick_loop.end_tick(start);
        
        assert_eq!(tick_loop.tick_count(), 1);
    }

    #[test]
    fn test_stats_tracking() {
        let mut tick_loop = TickLoop::new(1000);
        
        // Execute a few ticks
        for _ in 0..10 {
            std::thread::sleep(Duration::from_micros(100));
            while tick_loop.should_tick() {
                let start = tick_loop.begin_tick();
                std::thread::sleep(Duration::from_micros(50));
                tick_loop.end_tick(start);
            }
        }
        
        let stats = tick_loop.stats();
        assert!(stats.total_ticks > 0);
        assert!(stats.min_tick_us > 0);
        assert!(stats.min_tick_us <= stats.max_tick_us);
    }
}
