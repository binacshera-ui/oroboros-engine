//! # OROBOROS Game Loop
//!
//! THE ARCHITECT'S ORCHESTRATION:
//! ```text
//! Frame N:
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │ 1. BEGIN FRAME                                                      │
//! │    └─ Acquire write handle to Buffer A                              │
//! │                                                                     │
//! │ 2. LOGIC TICK (Unit 4 writes to Buffer A)                          │
//! │    ├─ Process network input                                         │
//! │    ├─ Update physics                                                │
//! │    ├─ Call economy (Unit 3) for mining/combat                      │
//! │    └─ Write positions to ECS                                        │
//! │                                                                     │
//! │ 3. ECONOMY TICK (Unit 3)                                           │
//! │    ├─ Process pending transactions                                  │
//! │    └─ Update inventories in ECS                                     │
//! │                                                                     │
//! │ 4. SWAP BUFFERS (Unit 1)                                           │
//! │    └─ Atomic pointer swap + dirty copy                              │
//! │                                                                     │
//! │ 5. RENDER TICK (Unit 2 reads from Buffer B)                        │
//! │    ├─ Read entity positions                                         │
//! │    ├─ Process visual events (particles, UI)                         │
//! │    └─ Submit GPU commands                                           │
//! │                                                                     │
//! │ 6. END FRAME                                                        │
//! │    └─ Wait for vsync / frame budget                                 │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use oroboros_core::{DoubleBufferedWorld, FrameSync, WorldWriteHandle, WorldReadHandle};

use crate::events::{EventSystem, EventReceiver, EventSender};

/// Target frame time for 60 FPS.
pub const TARGET_FRAME_TIME: Duration = Duration::from_micros(16_666);

/// Maximum allowed frame time before warning.
pub const MAX_FRAME_TIME: Duration = Duration::from_millis(33);

/// Configuration for the game loop.
#[derive(Clone, Debug)]
pub struct GameLoopConfig {
    /// Number of Position+Velocity entities to pre-allocate.
    pub pv_capacity: usize,
    /// Number of Position-only entities to pre-allocate.
    pub p_capacity: usize,
    /// Event channel capacity.
    pub event_capacity: usize,
    /// Enable frame timing logs.
    pub enable_timing_logs: bool,
    /// Target frames per second.
    pub target_fps: u32,
}

impl Default for GameLoopConfig {
    fn default() -> Self {
        Self {
            pv_capacity: 1_000_000,   // 1M moving entities
            p_capacity: 10_000_000,   // 10M static voxels
            event_capacity: 2048,
            enable_timing_logs: false,
            target_fps: 60,
        }
    }
}

/// Frame timing statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameStats {
    /// Total frame time in microseconds.
    pub total_us: u64,
    /// Logic tick time in microseconds.
    pub logic_us: u64,
    /// Economy tick time in microseconds.
    pub economy_us: u64,
    /// Buffer swap time in microseconds.
    pub swap_us: u64,
    /// Render tick time in microseconds.
    pub render_us: u64,
    /// Frame number.
    pub frame: u64,
    /// Events processed this frame.
    pub events_processed: u32,
}

/// Handles for a single frame's work.
///
/// This struct is created at the start of each frame and dropped at the end.
pub struct FrameContext<'a> {
    /// Write handle for logic to update entities.
    pub write: WorldWriteHandle,
    /// Event sender for logic to emit events.
    pub logic_events: &'a EventSender,
    /// Event receiver for logic to receive network input.
    pub input_events: &'a EventReceiver,
    /// Economy event sender.
    pub economy_events: &'a EventSender,
    /// Current frame number.
    pub frame: u64,
    /// Delta time since last frame.
    pub delta_time: f32,
}

/// Read-only context for rendering.
pub struct RenderContext<'a> {
    /// Read handle for rendering to access entities.
    pub read: WorldReadHandle,
    /// Event receiver for visual events.
    pub visual_events: &'a EventReceiver,
    /// Economy events (for UI updates).
    pub economy_events: &'a EventReceiver,
    /// Current frame number.
    pub frame: u64,
}

/// The main game loop orchestrator.
///
/// Owns the ECS world and event system, manages frame lifecycle.
pub struct GameLoop {
    /// The double-buffered ECS world (Unit 1's domain).
    world: Arc<DoubleBufferedWorld>,
    /// Frame synchronization helper.
    frame_sync: FrameSync,
    /// Event system for inter-unit communication.
    events: EventSystem,
    /// Configuration.
    config: GameLoopConfig,
    /// Frame counter.
    frame_count: u64,
    /// Last frame start time.
    last_frame_time: Instant,
    /// Accumulated frame statistics.
    stats_accumulator: FrameStatsAccumulator,
}

impl GameLoop {
    /// Creates a new game loop.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the game loop
    #[must_use]
    pub fn new(config: GameLoopConfig) -> Self {
        let world = DoubleBufferedWorld::new(config.pv_capacity, config.p_capacity);
        let frame_sync = world.frame_sync();
        let events = EventSystem::new();

        Self {
            world,
            frame_sync,
            events,
            config,
            frame_count: 0,
            last_frame_time: Instant::now(),
            stats_accumulator: FrameStatsAccumulator::new(),
        }
    }

    /// Begins a new frame.
    ///
    /// Returns a context for the frame's work, or None if shutdown requested.
    ///
    /// # Panics
    ///
    /// Panics if called while a previous frame is still active.
    #[must_use]
    pub fn begin_frame(&mut self) -> FrameContext<'_> {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;

        // Clamp delta time to prevent physics explosion after pause
        let delta_time = delta.as_secs_f32().min(0.1);

        let write = self.world.write_handle();

        FrameContext {
            write,
            logic_events: &self.events.logic_sender,
            input_events: &self.events.logic_receiver,
            economy_events: &self.events.economy_sender,
            frame: self.frame_count,
            delta_time,
        }
    }

    /// Swaps buffers after logic tick.
    ///
    /// Call this after logic has finished writing but before rendering.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The frame context (consumed to ensure write handle is dropped)
    pub fn swap_buffers(&self, ctx: FrameContext<'_>) {
        // Drop the write handle explicitly
        drop(ctx.write);

        // Perform the swap (includes dirty copy)
        self.frame_sync.end_frame();
    }

    /// Gets a render context for the current frame.
    ///
    /// Call this after `swap_buffers()`.
    #[must_use]
    pub fn render_context(&self) -> RenderContext<'_> {
        RenderContext {
            read: self.world.read_handle(),
            visual_events: &self.events.render_receiver,
            economy_events: &self.events.economy_receiver,
            frame: self.frame_count,
        }
    }

    /// Ends the current frame.
    ///
    /// Records timing and prepares for next frame.
    pub fn end_frame(&mut self, stats: FrameStats) {
        self.frame_count += 1;
        self.stats_accumulator.record(stats);

        // Log slow frames
        if self.config.enable_timing_logs && stats.total_us > MAX_FRAME_TIME.as_micros() as u64 {
            eprintln!(
                "⚠️ Frame {} exceeded budget: {:.2}ms (target: {:.2}ms)",
                self.frame_count,
                stats.total_us as f64 / 1000.0,
                TARGET_FRAME_TIME.as_micros() as f64 / 1000.0
            );
        }
    }

    /// Returns the current frame count.
    #[inline]
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Returns a reference to the event system.
    #[must_use]
    pub fn events(&self) -> &EventSystem {
        &self.events
    }

    /// Returns the accumulated statistics.
    #[must_use]
    pub fn stats(&self) -> &FrameStatsAccumulator {
        &self.stats_accumulator
    }

    /// Returns the world sync statistics (for profiling dirty copy).
    #[must_use]
    pub fn sync_stats(&self) -> oroboros_core::WorldSyncStats {
        self.world.sync_stats()
    }

    /// Gets a clone of the world Arc (for sharing with other threads).
    #[must_use]
    pub fn world_handle(&self) -> Arc<DoubleBufferedWorld> {
        Arc::clone(&self.world)
    }
}

/// Accumulator for frame statistics.
#[derive(Clone, Debug)]
pub struct FrameStatsAccumulator {
    /// Total frames recorded.
    pub frames_recorded: u64,
    /// Sum of total frame times.
    pub total_us_sum: u64,
    /// Sum of logic tick times.
    pub logic_us_sum: u64,
    /// Sum of swap times.
    pub swap_us_sum: u64,
    /// Sum of render times.
    pub render_us_sum: u64,
    /// Min frame time.
    pub min_frame_us: u64,
    /// Max frame time.
    pub max_frame_us: u64,
    /// Frames that exceeded budget.
    pub frames_over_budget: u64,
}

impl FrameStatsAccumulator {
    /// Creates a new accumulator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            frames_recorded: 0,
            total_us_sum: 0,
            logic_us_sum: 0,
            swap_us_sum: 0,
            render_us_sum: 0,
            min_frame_us: u64::MAX,
            max_frame_us: 0,
            frames_over_budget: 0,
        }
    }

    /// Records a frame's statistics.
    pub fn record(&mut self, stats: FrameStats) {
        self.frames_recorded += 1;
        self.total_us_sum += stats.total_us;
        self.logic_us_sum += stats.logic_us;
        self.swap_us_sum += stats.swap_us;
        self.render_us_sum += stats.render_us;
        self.min_frame_us = self.min_frame_us.min(stats.total_us);
        self.max_frame_us = self.max_frame_us.max(stats.total_us);

        if stats.total_us > TARGET_FRAME_TIME.as_micros() as u64 {
            self.frames_over_budget += 1;
        }
    }

    /// Returns average frame time in milliseconds.
    #[must_use]
    pub fn avg_frame_ms(&self) -> f64 {
        if self.frames_recorded == 0 {
            return 0.0;
        }
        (self.total_us_sum as f64 / self.frames_recorded as f64) / 1000.0
    }

    /// Returns average FPS.
    #[must_use]
    pub fn avg_fps(&self) -> f64 {
        let avg_ms = self.avg_frame_ms();
        if avg_ms <= 0.0 {
            return 0.0;
        }
        1000.0 / avg_ms
    }

    /// Returns the percentage of frames over budget.
    #[must_use]
    pub fn over_budget_ratio(&self) -> f64 {
        if self.frames_recorded == 0 {
            return 0.0;
        }
        self.frames_over_budget as f64 / self.frames_recorded as f64
    }

    /// Prints a summary of the statistics.
    pub fn print_summary(&self) {
        println!("╔══════════════════════════════════════════════════════════════════╗");
        println!("║                    FRAME STATISTICS SUMMARY                      ║");
        println!("╚══════════════════════════════════════════════════════════════════╝");
        println!();
        println!("┌─ TIMING ───────────────────────────────────────────────────────┐");
        println!("│ Frames Recorded:    {}                                        ", self.frames_recorded);
        println!("│ Average Frame:      {:.3} ms ({:.1} FPS)                      ", self.avg_frame_ms(), self.avg_fps());
        println!("│ Min Frame:          {:.3} ms                                  ", self.min_frame_us as f64 / 1000.0);
        println!("│ Max Frame:          {:.3} ms                                  ", self.max_frame_us as f64 / 1000.0);
        println!("└──────────────────────────────────────────────────────────────────┘");
        println!();
        println!("┌─ BUDGET ───────────────────────────────────────────────────────┐");
        println!("│ Target:             {:.3} ms (60 FPS)                          ", TARGET_FRAME_TIME.as_micros() as f64 / 1000.0);
        println!("│ Over Budget:        {} frames ({:.1}%)                        ", 
            self.frames_over_budget, 
            self.over_budget_ratio() * 100.0);
        println!("└──────────────────────────────────────────────────────────────────┘");

        if self.frames_recorded > 0 {
            println!();
            println!("┌─ BREAKDOWN ─────────────────────────────────────────────────────┐");
            let avg_logic = (self.logic_us_sum as f64 / self.frames_recorded as f64) / 1000.0;
            let avg_swap = (self.swap_us_sum as f64 / self.frames_recorded as f64) / 1000.0;
            let avg_render = (self.render_us_sum as f64 / self.frames_recorded as f64) / 1000.0;
            println!("│ Logic (Unit 4):     {:.3} ms                                  ", avg_logic);
            println!("│ Swap (Unit 1):      {:.3} ms                                  ", avg_swap);
            println!("│ Render (Unit 2):    {:.3} ms                                  ", avg_render);
            println!("└──────────────────────────────────────────────────────────────────┘");
        }
    }
}

impl Default for FrameStatsAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_loop_creation() {
        let config = GameLoopConfig::default();
        let game_loop = GameLoop::new(config);
        assert_eq!(game_loop.frame_count(), 0);
    }

    #[test]
    fn test_frame_cycle() {
        let config = GameLoopConfig {
            pv_capacity: 1000,
            p_capacity: 1000,
            ..Default::default()
        };
        let mut game_loop = GameLoop::new(config);

        // Begin frame
        let ctx = game_loop.begin_frame();
        assert_eq!(ctx.frame, 0);

        // Swap
        game_loop.swap_buffers(ctx);

        // Render
        let render_ctx = game_loop.render_context();
        assert_eq!(render_ctx.frame, 0);
        drop(render_ctx);

        // End frame
        game_loop.end_frame(FrameStats {
            total_us: 1000,
            logic_us: 500,
            economy_us: 100,
            swap_us: 200,
            render_us: 200,
            frame: 0,
            events_processed: 0,
        });

        assert_eq!(game_loop.frame_count(), 1);
    }

    #[test]
    fn test_stats_accumulator() {
        let mut acc = FrameStatsAccumulator::new();

        for i in 0..100 {
            acc.record(FrameStats {
                total_us: 10_000 + (i * 100),
                logic_us: 5000,
                economy_us: 1000,
                swap_us: 2000,
                render_us: 2000,
                frame: i,
                events_processed: 10,
            });
        }

        assert_eq!(acc.frames_recorded, 100);
        assert!(acc.avg_fps() > 50.0);
        assert!(acc.avg_fps() < 100.0);
    }
}
