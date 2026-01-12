//! # ECS-Rendering Interop
//!
//! ARCHITECT'S MANDATE: Logic moves entity → Graphics shows it SAME TICK.
//! No 16ms lag. No frame delay. SAME TICK.
//!
//! Solution: Zero-copy view into ECS ComponentStorage
//! - Rendering reads directly from ECS memory
//! - No data copying between systems
//! - Atomic generation counter for synchronization
//!
//! The interop layer provides:
//! 1. `PositionView` - Direct read access to ECS positions
//! 2. `EntitySyncSystem` - Syncs ECS entities to GPU instances
//! 3. `SharedWorldState` - Lock-free world state sharing

mod position_view;
mod entity_sync;
mod shared_state;

pub use position_view::{PositionView, OwnedPositionView};
pub use entity_sync::{EntitySyncSystem, SyncConfig, SyncGeneration, SyncStats};
pub use shared_state::{SharedWorldState, WorldStateSnapshot, WorldStateHandle, WriteGuard};
