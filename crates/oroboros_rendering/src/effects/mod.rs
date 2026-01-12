//! # Visual Effects System
//!
//! ARCHITECT'S MANDATE: When a Legendary drops, the screen EXPLODES.
//!
//! This module provides:
//! - `EventVisualizer` - Listens for economy events and triggers effects
//! - `ParticleSystem` - GPU compute-based particle simulation
//! - `VoxelExplosion` - Voxel-based firework effects
//!
//! Performance target: Event → Visual in SAME FRAME (<16ms)

mod event_visualizer;
mod particle_system;
mod particle_shaders;

pub use event_visualizer::{EventVisualizer, VisualEvent, EventConfig, Rarity, VisualizerStats};
pub use particle_system::{
    ParticleSystem, Particle, ParticleEmitter, ParticleConfig,
    ParticleStats, EmitterType,
};
pub use particle_shaders::{
    ParticleShaders, ParticleBlendMode, ParticleDepthMode, ParticleRenderPass,
    BlendStateConfig, BlendFactor, BlendOp, ParticleRenderConfig,
};
