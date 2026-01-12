//! Event Visualizer - Connects Economy Events to Visual Effects
//!
//! Listens for events from Unit 3 (Economy) and triggers GPU particle effects.
//! Response time: SAME FRAME (< 16ms from event to visual)

use super::particle_system::{ParticleEmitter, ParticleConfig, EmitterType};
use std::collections::VecDeque;

/// Maximum pending events to prevent memory explosion
const MAX_PENDING_EVENTS: usize = 256;

/// Rarity levels matching oroboros_economy::Rarity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Rarity {
    /// Common items - no special effect
    Common = 0,
    /// Uncommon items - subtle glow
    Uncommon = 1,
    /// Rare items - blue sparkles
    Rare = 2,
    /// Epic items - purple burst
    Epic = 3,
    /// Legendary items - GOLDEN EXPLOSION
    Legendary = 4,
    /// Mythic items - REALITY DISTORTION
    Mythic = 5,
}

impl Rarity {
    /// Converts from economy crate's u8 representation
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Common,
            1 => Self::Uncommon,
            2 => Self::Rare,
            3 => Self::Epic,
            4 => Self::Legendary,
            _ => Self::Mythic,
        }
    }
    
    /// Particle count for this rarity
    #[must_use]
    pub const fn particle_count(self) -> u32 {
        match self {
            Self::Common => 0,
            Self::Uncommon => 50,
            Self::Rare => 500,
            Self::Epic => 2000,
            Self::Legendary => 10_000,
            Self::Mythic => 25_000,
        }
    }
    
    /// Base color for this rarity (RGB)
    #[must_use]
    pub const fn base_color(self) -> [f32; 3] {
        match self {
            Self::Common => [0.5, 0.5, 0.5],     // Gray
            Self::Uncommon => [0.2, 0.8, 0.2],    // Green
            Self::Rare => [0.2, 0.4, 1.0],        // Blue
            Self::Epic => [0.6, 0.2, 0.9],        // Purple
            Self::Legendary => [1.0, 0.8, 0.2],   // Gold
            Self::Mythic => [1.0, 0.2, 0.2],      // Red
        }
    }
    
    /// Emission intensity multiplier
    #[must_use]
    pub const fn emission_intensity(self) -> f32 {
        match self {
            Self::Common => 0.0,
            Self::Uncommon => 1.0,
            Self::Rare => 2.0,
            Self::Epic => 4.0,
            Self::Legendary => 8.0,
            Self::Mythic => 16.0,
        }
    }
}

/// Visual events that can be triggered
#[derive(Debug, Clone)]
pub enum VisualEvent {
    /// Item dropped from loot
    ItemDrop {
        /// Position in world space
        position: [f32; 3],
        /// Item rarity
        rarity: Rarity,
        /// Item ID (for special effects)
        item_id: u32,
    },
    /// Combat hit effect
    CombatHit {
        /// Impact position
        position: [f32; 3],
        /// Damage amount (affects intensity)
        damage: u32,
        /// Is critical hit?
        is_critical: bool,
    },
    /// Crafting success
    CraftingComplete {
        /// Crafting station position
        position: [f32; 3],
        /// Result item rarity
        rarity: Rarity,
    },
    /// Transaction completed (profit/loss indication)
    Transaction {
        /// UI position (screen space)
        screen_pos: [f32; 2],
        /// Profit amount (negative = loss)
        profit: i64,
    },
    /// Dragon breath fire (networked from Unit 4)
    DragonFire {
        /// Start position
        start: [f32; 3],
        /// End position
        end: [f32; 3],
        /// Intensity
        intensity: f32,
    },
}

/// Configuration for event visualization
#[derive(Debug, Clone)]
pub struct EventConfig {
    /// Enable legendary effects
    pub legendary_effects: bool,
    /// Particle quality (0.0 = min, 1.0 = max)
    pub particle_quality: f32,
    /// Maximum concurrent emitters
    pub max_emitters: usize,
    /// Screen shake intensity (0.0 = off)
    pub screen_shake: f32,
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            legendary_effects: true,
            particle_quality: 1.0,
            max_emitters: 64,
            screen_shake: 1.0,
        }
    }
}

/// Connects economy/game events to visual effects
pub struct EventVisualizer {
    /// Configuration
    config: EventConfig,
    /// Pending events to process this frame
    pending_events: VecDeque<VisualEvent>,
    /// Active emitters spawned from events
    active_emitters: Vec<ParticleEmitter>,
    /// Statistics
    stats: VisualizerStats,
}

/// Statistics for the visualizer
#[derive(Debug, Clone, Copy, Default)]
pub struct VisualizerStats {
    /// Events received this frame
    pub events_received: u32,
    /// Events processed this frame
    pub events_processed: u32,
    /// Active emitters
    pub active_emitters: u32,
    /// Particles spawned this frame
    pub particles_spawned: u32,
}

impl EventVisualizer {
    /// Creates a new event visualizer
    #[must_use]
    pub fn new(config: EventConfig) -> Self {
        Self {
            config,
            pending_events: VecDeque::with_capacity(64),
            active_emitters: Vec::with_capacity(64),
            stats: VisualizerStats::default(),
        }
    }
    
    /// Pushes a visual event to be processed this frame
    ///
    /// Call this when you receive an event from Unit 3 (Economy)
    /// or Unit 4 (Networking). The effect will appear SAME FRAME.
    pub fn push_event(&mut self, event: VisualEvent) {
        if self.pending_events.len() < MAX_PENDING_EVENTS {
            self.pending_events.push_back(event);
            self.stats.events_received += 1;
        }
    }
    
    /// Pushes an item drop event (convenience method)
    pub fn push_item_drop(&mut self, position: [f32; 3], rarity: u8, item_id: u32) {
        self.push_event(VisualEvent::ItemDrop {
            position,
            rarity: Rarity::from_u8(rarity),
            item_id,
        });
    }
    
    /// Processes all pending events and returns emitters to spawn
    ///
    /// Call this at the START of each frame, before particle update.
    /// Returns new emitters that should be added to the particle system.
    pub fn process_events(&mut self) -> Vec<ParticleEmitter> {
        self.stats = VisualizerStats::default();
        let mut new_emitters = Vec::new();
        
        while let Some(event) = self.pending_events.pop_front() {
            if let Some(emitter) = self.event_to_emitter(&event) {
                self.stats.particles_spawned += emitter.config.spawn_count;
                new_emitters.push(emitter);
                self.stats.events_processed += 1;
            }
        }
        
        // Update active emitter count
        self.stats.active_emitters = new_emitters.len() as u32;
        
        new_emitters
    }
    
    /// Converts an event to a particle emitter
    fn event_to_emitter(&self, event: &VisualEvent) -> Option<ParticleEmitter> {
        match event {
            VisualEvent::ItemDrop { position, rarity, item_id: _ } => {
                self.create_loot_emitter(*position, *rarity)
            }
            VisualEvent::CombatHit { position, damage, is_critical } => {
                Some(self.create_combat_emitter(*position, *damage, *is_critical))
            }
            VisualEvent::CraftingComplete { position, rarity } => {
                self.create_crafting_emitter(*position, *rarity)
            }
            VisualEvent::Transaction { screen_pos, profit } => {
                Some(self.create_transaction_emitter(*screen_pos, *profit))
            }
            VisualEvent::DragonFire { start, end, intensity } => {
                Some(self.create_dragon_fire_emitter(*start, *end, *intensity))
            }
        }
    }
    
    /// Creates a loot drop particle emitter
    fn create_loot_emitter(&self, position: [f32; 3], rarity: Rarity) -> Option<ParticleEmitter> {
        // Skip common items
        if rarity == Rarity::Common {
            return None;
        }
        
        let particle_count = (rarity.particle_count() as f32 * self.config.particle_quality) as u32;
        let base_color = rarity.base_color();
        let emission = rarity.emission_intensity();
        
        let config = match rarity {
            Rarity::Common => return None,
            
            Rarity::Uncommon => ParticleConfig {
                spawn_count: particle_count,
                lifetime_min: 0.5,
                lifetime_max: 1.0,
                velocity_min: [0.0, 1.0, 0.0],
                velocity_max: [0.5, 2.0, 0.5],
                color_start: [base_color[0], base_color[1], base_color[2], 1.0],
                color_end: [base_color[0], base_color[1], base_color[2], 0.0],
                size_start: 0.1,
                size_end: 0.05,
                emission_intensity: emission,
                gravity: -2.0,
                drag: 0.5,
                ..ParticleConfig::default()
            },
            
            Rarity::Rare => ParticleConfig {
                spawn_count: particle_count,
                lifetime_min: 0.8,
                lifetime_max: 1.5,
                velocity_min: [-2.0, 2.0, -2.0],
                velocity_max: [2.0, 5.0, 2.0],
                color_start: [base_color[0], base_color[1], base_color[2], 1.0],
                color_end: [0.0, 0.2, 1.0, 0.0],
                size_start: 0.15,
                size_end: 0.02,
                emission_intensity: emission,
                gravity: -1.0,
                drag: 0.3,
                ..ParticleConfig::default()
            },
            
            Rarity::Epic => ParticleConfig {
                spawn_count: particle_count,
                lifetime_min: 1.0,
                lifetime_max: 2.0,
                velocity_min: [-4.0, 3.0, -4.0],
                velocity_max: [4.0, 8.0, 4.0],
                color_start: [base_color[0], base_color[1], base_color[2], 1.0],
                color_end: [0.8, 0.4, 1.0, 0.0],
                size_start: 0.2,
                size_end: 0.05,
                emission_intensity: emission,
                gravity: -0.5,
                drag: 0.2,
                turbulence: 1.0,
                ..ParticleConfig::default()
            },
            
            // LEGENDARY - THE BIG ONE
            Rarity::Legendary => ParticleConfig {
                spawn_count: particle_count,
                lifetime_min: 1.5,
                lifetime_max: 3.0,
                velocity_min: [-8.0, 5.0, -8.0],
                velocity_max: [8.0, 15.0, 8.0],
                color_start: [1.0, 0.9, 0.3, 1.0],  // Bright gold
                color_end: [1.0, 0.5, 0.0, 0.0],    // Fade to orange
                size_start: 0.3,
                size_end: 0.1,
                emission_intensity: emission,
                gravity: 0.5,  // Floats up!
                drag: 0.1,
                turbulence: 2.0,
                spawn_rate: 0.0,  // All at once (burst)
                ..ParticleConfig::default()
            },
            
            // MYTHIC - REALITY BREAKS
            Rarity::Mythic => ParticleConfig {
                spawn_count: particle_count,
                lifetime_min: 2.0,
                lifetime_max: 4.0,
                velocity_min: [-12.0, 8.0, -12.0],
                velocity_max: [12.0, 20.0, 12.0],
                color_start: [1.0, 0.3, 0.3, 1.0],  // Red
                color_end: [0.0, 0.0, 0.0, 0.0],    // Void
                size_start: 0.4,
                size_end: 0.15,
                emission_intensity: emission,
                gravity: -0.2,
                drag: 0.05,
                turbulence: 4.0,
                spawn_rate: 0.0,
                distortion: 1.0,  // Screen distortion!
                ..ParticleConfig::default()
            },
        };
        
        Some(ParticleEmitter::new(
            position,
            config,
            EmitterType::Burst,
        ))
    }
    
    /// Creates a combat hit emitter
    fn create_combat_emitter(&self, position: [f32; 3], damage: u32, is_critical: bool) -> ParticleEmitter {
        let intensity = (damage as f32 / 100.0).min(3.0);
        let particle_count = if is_critical { 500 } else { 100 };
        
        let color = if is_critical {
            [1.0, 1.0, 0.0, 1.0]  // Yellow for crit
        } else {
            [1.0, 0.3, 0.0, 1.0]  // Orange for normal
        };
        
        ParticleEmitter::new(
            position,
            ParticleConfig {
                spawn_count: (particle_count as f32 * self.config.particle_quality) as u32,
                lifetime_min: 0.2,
                lifetime_max: 0.5,
                velocity_min: [-3.0 * intensity, 0.0, -3.0 * intensity],
                velocity_max: [3.0 * intensity, 5.0 * intensity, 3.0 * intensity],
                color_start: color,
                color_end: [color[0], color[1], color[2], 0.0],
                size_start: 0.1 * intensity,
                size_end: 0.02,
                emission_intensity: if is_critical { 4.0 } else { 2.0 },
                gravity: -5.0,
                drag: 0.8,
                ..ParticleConfig::default()
            },
            EmitterType::Burst,
        )
    }
    
    /// Creates a crafting completion emitter
    fn create_crafting_emitter(&self, position: [f32; 3], rarity: Rarity) -> Option<ParticleEmitter> {
        if rarity == Rarity::Common {
            return None;
        }
        
        let base_color = rarity.base_color();
        
        Some(ParticleEmitter::new(
            position,
            ParticleConfig {
                spawn_count: (rarity.particle_count() / 2) as u32,
                lifetime_min: 0.5,
                lifetime_max: 1.5,
                velocity_min: [-1.0, 0.5, -1.0],
                velocity_max: [1.0, 3.0, 1.0],
                color_start: [base_color[0], base_color[1], base_color[2], 0.8],
                color_end: [base_color[0], base_color[1], base_color[2], 0.0],
                size_start: 0.15,
                size_end: 0.05,
                emission_intensity: rarity.emission_intensity() * 0.5,
                gravity: -1.0,
                drag: 0.4,
                ..ParticleConfig::default()
            },
            EmitterType::Burst,
        ))
    }
    
    /// Creates a transaction indicator emitter (screen space)
    fn create_transaction_emitter(&self, screen_pos: [f32; 2], profit: i64) -> ParticleEmitter {
        let (color, direction) = if profit >= 0 {
            // PROFIT: Green, floats UP
            ([0.0, 1.0, 0.3, 1.0], [0.0, 2.0, 0.0])
        } else {
            // LOSS: Red, falls DOWN
            ([1.0, 0.2, 0.0, 1.0], [0.0, -2.0, 0.0])
        };
        
        let intensity = ((profit.abs() as f32) / 1000.0).min(5.0).max(0.5);
        
        ParticleEmitter::new(
            [screen_pos[0], screen_pos[1], 0.0],
            ParticleConfig {
                spawn_count: (50.0 * intensity) as u32,
                lifetime_min: 0.5,
                lifetime_max: 1.0,
                velocity_min: [direction[0] - 0.5, direction[1], direction[2] - 0.5],
                velocity_max: [direction[0] + 0.5, direction[1] * 2.0, direction[2] + 0.5],
                color_start: color,
                color_end: [color[0], color[1], color[2], 0.0],
                size_start: 0.05 * intensity,
                size_end: 0.01,
                emission_intensity: 2.0,
                gravity: 0.0,  // Screen space, no gravity
                drag: 0.3,
                is_screen_space: true,
                ..ParticleConfig::default()
            },
            EmitterType::Burst,
        )
    }
    
    /// Creates a dragon fire trail emitter
    fn create_dragon_fire_emitter(&self, start: [f32; 3], end: [f32; 3], intensity: f32) -> ParticleEmitter {
        let direction = [
            end[0] - start[0],
            end[1] - start[1],
            end[2] - start[2],
        ];
        
        ParticleEmitter::new(
            start,
            ParticleConfig {
                spawn_count: (5000.0 * intensity * self.config.particle_quality) as u32,
                lifetime_min: 0.3,
                lifetime_max: 1.0,
                velocity_min: [direction[0] * 0.8, direction[1] * 0.8, direction[2] * 0.8],
                velocity_max: [direction[0] * 1.2, direction[1] * 1.2, direction[2] * 1.2],
                color_start: [1.0, 0.6, 0.1, 1.0],  // Orange fire
                color_end: [0.3, 0.0, 0.0, 0.0],    // Dark red fade
                size_start: 0.5 * intensity,
                size_end: 0.1,
                emission_intensity: 10.0 * intensity,
                gravity: 2.0,  // Fire rises
                drag: 0.2,
                turbulence: 3.0,
                ..ParticleConfig::default()
            },
            EmitterType::Stream { duration: 0.5 },
        )
    }
    
    /// Returns current statistics
    #[must_use]
    pub fn stats(&self) -> VisualizerStats {
        self.stats
    }
    
    /// Clears all pending events (use on scene change)
    pub fn clear(&mut self) {
        self.pending_events.clear();
        self.active_emitters.clear();
    }
}

impl Default for EventVisualizer {
    fn default() -> Self {
        Self::new(EventConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_legendary_creates_emitter() {
        let mut viz = EventVisualizer::new(EventConfig::default());
        
        viz.push_event(VisualEvent::ItemDrop {
            position: [0.0, 0.0, 0.0],
            rarity: Rarity::Legendary,
            item_id: 1000,
        });
        
        let emitters = viz.process_events();
        assert_eq!(emitters.len(), 1);
        assert_eq!(emitters[0].config.spawn_count, 10_000);
    }
    
    #[test]
    fn test_common_skipped() {
        let mut viz = EventVisualizer::new(EventConfig::default());
        
        viz.push_event(VisualEvent::ItemDrop {
            position: [0.0, 0.0, 0.0],
            rarity: Rarity::Common,
            item_id: 1,
        });
        
        let emitters = viz.process_events();
        assert!(emitters.is_empty());
    }
    
    #[test]
    fn test_particle_quality_scaling() {
        let mut viz = EventVisualizer::new(EventConfig {
            particle_quality: 0.5,
            ..Default::default()
        });
        
        viz.push_event(VisualEvent::ItemDrop {
            position: [0.0, 0.0, 0.0],
            rarity: Rarity::Legendary,
            item_id: 1000,
        });
        
        let emitters = viz.process_events();
        assert_eq!(emitters[0].config.spawn_count, 5000); // 10000 * 0.5
    }
}
