//! Render Loop - Main integration point for Unit 2
//!
//! THE GOLDEN PATH (from OPERATION COLD FUSION):
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         FRAME TIMELINE                          │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │  0ms    ├── Begin Frame                                        │
//! │         │   ├── Get WorldReadHandle from Unit 1                │
//! │         │   ├── Drain GameEventQueue                           │
//! │         │   └── Extract Frame Data                              │
//! │                                                                 │
//! │  2ms    ├── Process Events → EventVisualizer                   │
//! │         │   └── Generate ParticleEmitters                       │
//! │                                                                 │
//! │  4ms    ├── Update Particle System (GPU)                       │
//! │                                                                 │
//! │  6ms    ├── Render World                                       │
//! │         │   ├── Pass 1: Opaque geometry                        │
//! │         │   ├── Pass 2: Emissive particles (fire, neon)        │
//! │         │   ├── Pass 3: Volumetric particles (smoke, ash)      │
//! │         │   └── Pass 4: Post-process + UI                       │
//! │                                                                 │
//! │  14ms   ├── Present                                            │
//! │                                                                 │
//! │  16ms   └── Frame Complete                                     │
//! │                                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

use super::game_events::{GameEvent, GameEventQueue};
use super::render_bridge::{RenderBridge, RenderBridgeConfig, WorldReader};
use crate::effects::{EventVisualizer, EventConfig, ParticleSystem, VisualEvent, Rarity};

/// Configuration for the render loop
#[derive(Debug, Clone)]
pub struct RenderLoopConfig {
    /// Target frame rate
    pub target_fps: u32,
    /// Enable V-Sync
    pub vsync: bool,
    /// Maximum frame time before warning (microseconds)
    pub frame_budget_us: u32,
    /// Render bridge config
    pub bridge_config: RenderBridgeConfig,
    /// Event visualizer config
    pub event_config: EventConfig,
}

impl Default for RenderLoopConfig {
    fn default() -> Self {
        Self {
            target_fps: 120,
            vsync: true,
            frame_budget_us: 16_666, // ~16ms for 60fps
            bridge_config: RenderBridgeConfig::default(),
            event_config: EventConfig::default(),
        }
    }
}

/// Result of a single frame
#[derive(Debug, Clone)]
pub struct FrameResult {
    /// Frame number
    pub frame_number: u64,
    /// Total frame time (microseconds)
    pub frame_time_us: u32,
    /// Time spent reading from ECS
    pub ecs_read_us: u32,
    /// Time spent processing events
    pub event_process_us: u32,
    /// Time spent on particle update
    pub particle_update_us: u32,
    /// Time spent rendering
    pub render_us: u32,
    /// Events processed
    pub events_processed: u32,
    /// Entities rendered
    pub entities_rendered: u32,
    /// Particles alive
    pub particles_alive: u32,
    /// Over budget warning
    pub over_budget: bool,
}

/// Statistics for the render loop
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderLoopStats {
    /// Total frames rendered
    pub total_frames: u64,
    /// Average frame time (microseconds)
    pub avg_frame_time_us: u32,
    /// Worst frame time (microseconds)
    pub worst_frame_time_us: u32,
    /// Frames over budget
    pub frames_over_budget: u32,
}

/// The main render loop for Unit 2
///
/// Orchestrates:
/// - Reading from Unit 1's double buffer
/// - Processing events from Unit 3/4
/// - Running particle simulations
/// - Rendering to screen
pub struct RenderLoop {
    /// Configuration
    config: RenderLoopConfig,
    /// Bridge to Unit 1's ECS
    bridge: RenderBridge,
    /// Event queue (receives from Unit 3/4)
    event_queue: GameEventQueue,
    /// Event visualizer (converts events to particles)
    visualizer: EventVisualizer,
    /// GPU particle system
    particles: ParticleSystem,
    /// Frame counter
    frame_count: u64,
    /// Statistics
    stats: RenderLoopStats,
}

impl RenderLoop {
    /// Creates a new render loop
    #[must_use]
    pub fn new(config: RenderLoopConfig) -> Self {
        Self {
            bridge: RenderBridge::new(config.bridge_config.clone()),
            event_queue: GameEventQueue::new(),
            visualizer: EventVisualizer::new(config.event_config.clone()),
            particles: ParticleSystem::default(),
            frame_count: 0,
            stats: RenderLoopStats::default(),
            config,
        }
    }

    /// Returns mutable access to the event queue
    ///
    /// Unit 4 and Unit 3 push events here.
    pub fn event_queue_mut(&mut self) -> &mut GameEventQueue {
        &mut self.event_queue
    }

    /// Returns the event queue for reading
    #[must_use]
    pub fn event_queue(&self) -> &GameEventQueue {
        &self.event_queue
    }

    /// Executes one frame of rendering
    ///
    /// # Type Parameters
    ///
    /// * `W` - World reader type (from Unit 1)
    ///
    /// # Arguments
    ///
    /// * `world` - Read handle to Unit 1's double buffer
    /// * `camera_pos` - Current camera position
    /// * `view_proj` - View-projection matrix
    /// * `delta_time` - Time since last frame
    pub fn frame<W: WorldReader>(
        &mut self,
        world: &W,
        camera_pos: [f32; 3],
        view_proj: [[f32; 4]; 4],
        delta_time: f32,
    ) -> FrameResult {
        let frame_start = std::time::Instant::now();
        self.frame_count += 1;

        // === PHASE 1: Read from ECS ===
        let ecs_start = std::time::Instant::now();
        let frame_data = self.bridge.extract_frame_data(
            world,
            camera_pos,
            view_proj,
            delta_time,
            self.frame_count,
        );
        let ecs_time = ecs_start.elapsed();

        // === PHASE 2: Process Events ===
        let event_start = std::time::Instant::now();
        let events_processed = self.process_events();
        let event_time = event_start.elapsed();

        // === PHASE 3: Update Particles ===
        let particle_start = std::time::Instant::now();
        let _spawn_commands = self.particles.update(delta_time);
        let particle_time = particle_start.elapsed();

        // === PHASE 4: Render (would call GPU here) ===
        let render_start = std::time::Instant::now();
        // In real implementation, this would:
        // 1. Upload instance data to GPU
        // 2. Execute render passes
        // 3. Present frame
        let render_time = render_start.elapsed();

        // === PHASE 5: Finalize ===
        let total_time = frame_start.elapsed();
        let total_us = total_time.as_micros() as u32;

        // Update statistics
        self.stats.total_frames += 1;
        if total_us > self.stats.worst_frame_time_us {
            self.stats.worst_frame_time_us = total_us;
        }
        let over_budget = total_us > self.config.frame_budget_us;
        if over_budget {
            self.stats.frames_over_budget += 1;
        }

        // Reset event queue stats for next frame
        self.event_queue.reset_stats();

        FrameResult {
            frame_number: self.frame_count,
            frame_time_us: total_us,
            ecs_read_us: ecs_time.as_micros() as u32,
            event_process_us: event_time.as_micros() as u32,
            particle_update_us: particle_time.as_micros() as u32,
            render_us: render_time.as_micros() as u32,
            events_processed,
            entities_rendered: frame_data.entities.len() as u32,
            particles_alive: self.particles.stats().alive_count,
            over_budget,
        }
    }

    /// Processes all pending events
    fn process_events(&mut self) -> u32 {
        // Collect events first to avoid double borrow
        let events: Vec<_> = self.event_queue.drain().collect();
        let count = events.len() as u32;

        for event in events {
            self.handle_event(event);
        }

        // Get emitters from visualizer and add to particle system
        let emitters = self.visualizer.process_events();
        self.particles.add_emitters(emitters);

        count
    }

    /// Handles a single game event
    fn handle_event(&mut self, event: GameEvent) {
        match event {
            GameEvent::BlockBreak(e) => {
                // Push block break particles
                self.visualizer.push_event(VisualEvent::ItemDrop {
                    position: e.position,
                    rarity: Rarity::Common,
                    item_id: e.block_type as u32,
                });
            }
            GameEvent::ItemDrop(e) => {
                // THE GOLDEN PATH: Item drop triggers particles
                self.visualizer.push_item_drop(e.position, e.rarity, e.item_id);
            }
            GameEvent::Damage(e) => {
                self.visualizer.push_event(VisualEvent::CombatHit {
                    position: e.position,
                    damage: e.damage,
                    is_critical: e.is_critical,
                });
            }
            GameEvent::Death(e) => {
                // Big death effect
                self.visualizer.push_event(VisualEvent::ItemDrop {
                    position: e.position,
                    rarity: Rarity::Epic,
                    item_id: 0,
                });
            }
            GameEvent::Transaction(e) => {
                self.visualizer.push_event(VisualEvent::Transaction {
                    screen_pos: e.screen_pos,
                    profit: e.amount,
                });
            }
        }
    }

    /// Returns the particle system for direct access
    #[must_use]
    pub fn particles(&self) -> &ParticleSystem {
        &self.particles
    }

    /// Returns mutable access to particles
    pub fn particles_mut(&mut self) -> &mut ParticleSystem {
        &mut self.particles
    }

    /// Returns the visualizer
    #[must_use]
    pub fn visualizer(&self) -> &EventVisualizer {
        &self.visualizer
    }

    /// Returns statistics
    #[must_use]
    pub fn stats(&self) -> RenderLoopStats {
        self.stats
    }

    /// Returns the current frame count
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

impl Default for RenderLoop {
    fn default() -> Self {
        Self::new(RenderLoopConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::render_bridge::MockWorldReader;

    #[test]
    fn test_render_loop_frame() {
        let mut render_loop = RenderLoop::new(RenderLoopConfig::default());

        // Simulate events from Unit 3/4
        render_loop.event_queue_mut().push_item_drop(
            [10.0, 5.0, 10.0],
            100,
            4, // Legendary!
            1,
            1,
        );

        let mock_world = MockWorldReader {
            entities: vec![
                (1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
            ],
        };

        let result = render_loop.frame(
            &mock_world,
            [0.0, 0.0, 0.0],
            [[1.0, 0.0, 0.0, 0.0]; 4],
            0.016,
        );

        assert_eq!(result.frame_number, 1);
        assert_eq!(result.events_processed, 1);
        assert_eq!(result.entities_rendered, 1);
    }

    #[test]
    fn test_golden_path_item_drop() {
        let mut render_loop = RenderLoop::new(RenderLoopConfig::default());

        // Simulate: Player breaks rock, gets diamond (Legendary drop)
        render_loop.event_queue_mut().push_block_break(
            [100.0, 50.0, 100.0],
            1, // Stone
            1, // Player 1
            60, // Tick 60
        );

        render_loop.event_queue_mut().push_item_drop(
            [100.0, 50.0, 100.0],
            999, // Diamond
            4,   // Legendary
            1,
            1,
        );

        let mock_world = MockWorldReader { entities: vec![] };

        let result = render_loop.frame(
            &mock_world,
            [100.0, 50.0, 100.0],
            [[1.0, 0.0, 0.0, 0.0]; 4],
            0.016,
        );

        // Should have processed both events
        assert_eq!(result.events_processed, 2);

        // Particle system should have spawned emitters
        // (10,000 particles for Legendary!)
        assert!(render_loop.particles().stats().spawned_this_frame > 0 || 
                render_loop.visualizer().stats().particles_spawned > 0);
    }
}
