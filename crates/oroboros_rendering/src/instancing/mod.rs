//! GPU Instancing system for efficient voxel rendering.
//!
//! This module provides hardware-accelerated instanced rendering that can
//! display millions of voxel faces with minimal draw calls.
//!
//! ## Key Concepts
//!
//! - **Instance Buffer**: Pre-allocated GPU buffer holding instance data
//! - **Indirect Drawing**: GPU-driven draw calls via `DrawIndexedIndirect`
//! - **Zero CPU Overhead**: All culling and batching happens on GPU

mod instance_data;
mod renderer;
mod buffer;
mod gpu_driven;

pub use instance_data::InstanceData;
pub use renderer::{InstancedRenderer, RenderStats};
pub use buffer::InstanceBuffer;
pub use gpu_driven::{
    GPUDrivenRenderer, ChunkGPUData, CullingUniforms, GPUDrivenStats,
    MultiDrawBuffer, DrawIndexedIndirectArgs, MAX_DRAW_COMMANDS,
};