//! # OROBOROS Rendering Engine
//!
//! GPU-bound voxel rendering engine designed for:
//! - 1,000,000+ voxels at 120 FPS @ 4K resolution
//! - Sub-1000 draw calls through aggressive instancing
//! - Zero CPU overhead - all computation on GPU
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    GPU PIPELINE                              │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Voxel Data → Greedy Mesh → Instance Buffer → GPU Instancing │
//! │       ↓                            ↓                         │
//! │  Occlusion Culling (Compute)  → Draw Indirect               │
//! │       ↓                            ↓                         │
//! │  Volumetric Fog → SSR → Post-Process → Final Frame          │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## ARCHITECT'S MANDATE
//!
//! - GPU utilization must be 100%
//! - CPU must stay under 5% for rendering
//! - No allocations in render loop
//! - No pop-in artifacts

#![deny(missing_docs)]
// Note: unsafe code is allowed in interop module for zero-copy ECS access
#![allow(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::perf)]

pub mod voxel;
pub mod instancing;
pub mod culling;
pub mod atmosphere;
pub mod pipeline;
pub mod interop;
pub mod effects;
pub mod integration;
pub mod assets;

pub use voxel::{Voxel, VoxelChunk, VoxelWorld, MaterialId, MaterialDef, MaterialRegistry, LocalPalette};
pub use instancing::{InstanceData, InstancedRenderer};
pub use culling::{OcclusionCuller, FrustumCuller};
pub use atmosphere::{VolumetricFog, NeonLighting, ScreenSpaceReflections};
pub use pipeline::{RenderPipeline, RenderFrame, RenderStats};
pub use interop::{PositionView, EntitySyncSystem, SharedWorldState, WorldStateSnapshot};
pub use effects::{EventVisualizer, VisualEvent, ParticleSystem, ParticleEmitter, ParticleShaders};

// === INTEGRATION (COLD FUSION) ===
pub use integration::{
    RenderBridge, RenderBridgeConfig, WorldReader,
    GameEvent, GameEventQueue, EventCategory,
    RenderLoop, RenderLoopConfig, FrameResult,
    CoreWorldReader,
};

// === ASSET PIPELINE (UNIT 6) ===
pub use assets::{
    VoxelModel, VoxelModelBuilder, ProceduralModels,
    ModelVoxel, ModelBounds,
    VoxLoader, VoxFile, VoxPalette, VoxError, ModelAssetLoader,
};