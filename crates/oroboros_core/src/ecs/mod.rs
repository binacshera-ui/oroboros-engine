//! # Entity Component System
//!
//! A zero-allocation ECS designed for maximum performance.
//!
//! ## Design Philosophy
//!
//! - All storage is pre-allocated at world creation
//! - Components are stored in dense arrays for cache efficiency
//! - Entity IDs are simple indices with generation counters
//! - No dynamic dispatch in hot paths

pub mod archetype;
mod component;
mod entity;
mod storage;
mod world;

pub use archetype::{
    ArchetypeTable, ArchetypeWorld, ArchetypeSignature,
    DirtyTracker, SyncStats, WorldSyncStats,
};
pub use component::{Component, Position, Velocity, Voxel};
pub use entity::{Entity, EntityId};
pub use storage::ComponentStorage;
pub use world::World;
