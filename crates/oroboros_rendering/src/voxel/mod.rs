//! Voxel data structures and world management.
//!
//! This module provides zero-allocation voxel handling optimized for GPU rendering.

mod chunk;
mod world;
mod greedy_mesh;
mod palette;
mod material_system;

pub use chunk::{Voxel, VoxelChunk, ChunkCoord, CHUNK_SIZE, CHUNK_VOLUME};
pub use world::VoxelWorld;
pub use greedy_mesh::{GreedyMesher, MeshQuad};
pub use palette::{CompressedVoxel, CompressedChunk, PaletteMaterial, MaterialPalette};
pub use material_system::{MaterialId, MaterialDef, MaterialRegistry, LocalPalette, LocalPaletteBuilder};