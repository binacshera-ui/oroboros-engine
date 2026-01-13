//! Voxel data structures and world management.
//!
//! This module provides zero-allocation voxel handling optimized for GPU rendering.
//!
//! OPERATION INDUSTRIAL STANDARD:
//! Uses block-mesh-rs for industry-standard greedy meshing.

mod chunk;
mod world;
mod palette;
mod material_system;
mod standard_mesher;

pub use chunk::{Voxel, VoxelChunk, ChunkCoord, CHUNK_SIZE, CHUNK_VOLUME};
pub use world::VoxelWorld;
pub use palette::{CompressedVoxel, CompressedChunk, PaletteMaterial, MaterialPalette};
pub use material_system::{MaterialId, MaterialDef, MaterialRegistry, LocalPalette, LocalPaletteBuilder};

// INDUSTRIAL STANDARD MESHING (block-mesh-rs)
// COURSE CORRECTION: Now outputs Vertex + Index buffers
pub use standard_mesher::{
    StandardMesher,
    MeshQuad,          // Legacy compatibility
    MeshVoxel,
    PaddedChunkBuffer,
    TerrainVertex,     // NEW: Standard vertex format
    ChunkMesh,         // NEW: Complete mesh (vertices + indices)
    PADDED_CHUNK_SIZE,
    PADDED_CHUNK_VOLUME,
};