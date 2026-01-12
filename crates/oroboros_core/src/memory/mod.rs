//! # Memory Management
//!
//! Pre-allocated memory pools and arenas for zero-allocation gameplay.
//!
//! ## Design Philosophy
//!
//! All memory is allocated once at startup. During gameplay:
//! - No heap allocations
//! - No garbage collection
//! - Predictable, flat latency

mod arena;
mod pool;

pub use arena::Arena;
pub use pool::{PoolAllocator, PoolHandle};
