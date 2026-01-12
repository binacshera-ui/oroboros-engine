//! # OROBOROS Procedural Generation
//!
//! Deterministic world generation for infinite, reproducible worlds.
//!
//! ## Design Principles
//!
//! 1. **Deterministic**: Same seed always produces the same world
//! 2. **Chunked**: World is generated in fixed-size chunks
//! 3. **Streamable**: Chunks can be generated/discarded independently
//! 4. **Fast**: 10,000x10,000 world in under 3 seconds
//!
//! ## Core Components
//!
//! - `SimplexNoise`: 2D/3D noise generation
//! - `ChunkGenerator`: Produces world chunks from noise
//! - `BiomeClassifier`: Determines terrain types from noise values
//! - `WorldManager`: Dynamic chunk loading/unloading
//! - `ChunkPersistence`: WAL integration for block modifications
//!
//! ## Example
//!
//! ```rust,ignore
//! use oroboros_procedural::{WorldManager, WorldSeed};
//!
//! let seed = WorldSeed::new(12345);
//! let mut manager = WorldManager::with_seed(seed);
//!
//! // Player at position (100, 200)
//! manager.update(100.0, 200.0);
//! manager.flush_generation_queue();
//!
//! // Check ground exists
//! assert!(manager.has_ground(100, 64, 200));
//! ```

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::perf)]

pub mod biome;
pub mod chunk;
pub mod chunk_persistence;
pub mod noise;
pub mod world_manager;

pub use biome::{Biome, BiomeClassifier};
pub use chunk::{Block, Chunk, ChunkCoord, ChunkGenerator, CHUNK_SIZE};
pub use chunk_persistence::{BlockModifyPayload, ChunkOpType, ChunkPersistence, WorldChunkSystem};
pub use noise::{SimplexNoise, WorldSeed};
pub use world_manager::{
    ChunkModification, ChunkState, ModificationEntry, WorldManager, WorldManagerConfig, WorldStats,
};
