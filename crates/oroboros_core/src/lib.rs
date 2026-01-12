//! # OROBOROS Core Engine
//!
//! Zero-allocation Entity Component System (ECS) designed for:
//! - 1,000,000+ entities at 120 FPS
//! - Sub-millisecond tick times
//! - Zero garbage collection pressure
//!
//! ## Architecture Rules
//!
//! 1. **No heap allocations in hot path** - All memory is pre-allocated
//! 2. **Data-oriented design** - Components are stored in contiguous arrays
//! 3. **Cache-friendly iteration** - Hot data is packed together
//!
//! ## Example
//!
//! ```rust,ignore
//! use oroboros_core::{World, Position, Velocity};
//!
//! let mut world = World::new(1_000_000);
//! // All memory pre-allocated, zero allocations during gameplay
//! ```

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::perf)]

pub mod ecs;
pub mod memory;
pub mod sync;

pub use ecs::{
    ArchetypeTable, ArchetypeWorld, ArchetypeSignature,
    Component, ComponentStorage, Entity, EntityId, Position, Velocity, Voxel, World,
    DirtyTracker, SyncStats, WorldSyncStats,
};
pub use memory::{Arena, PoolAllocator, PoolHandle};
pub use sync::{DoubleBufferedWorld, WorldWriteHandle, WorldReadHandle, FrameSync};