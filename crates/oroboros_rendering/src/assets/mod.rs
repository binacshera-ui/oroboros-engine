//! # Asset Pipeline - Unit 6
//!
//! Bridge between Art & Code. Handles loading and procedural generation of voxel models.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    ASSET PIPELINE                           │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Procedural Models  →  VoxelModel  →  Instance Buffer       │
//! │         ↓                   ↓                               │
//! │  VOX Loader (Future)   Greedy Mesh  →  GPU Upload           │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## ARCHITECT'S MANDATE
//!
//! - Models must be generated in code first (no artist dependency)
//! - VOX loader for future artist integration
//! - All models must convert to engine's Instance format
//! - Zero runtime allocation for model access

mod procedural_models;
mod vox_loader;

pub use procedural_models::{
    VoxelModel, VoxelModelBuilder, ProceduralModels,
    ModelVoxel, ModelBounds, colors,
};
pub use vox_loader::{
    VoxLoader, VoxFile, VoxPalette, VoxError, ModelAssetLoader,
};
