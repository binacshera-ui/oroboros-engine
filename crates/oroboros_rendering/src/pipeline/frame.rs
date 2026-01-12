//! Render frame data structures.
//!
//! Contains all data needed to render a single frame.

use crate::atmosphere::{VolumetricFogUniforms, NeonLightBuffer};
use super::RenderStats;

/// All data needed to render a frame.
///
/// This is produced by `RenderPipeline::prepare_frame` and consumed
/// by the GPU rendering code.
pub struct RenderFrame {
    /// Instance data for GPU upload.
    pub instance_data: Vec<u8>,
    
    /// Volumetric fog uniforms.
    pub fog_uniforms: VolumetricFogUniforms,
    
    /// Neon light buffer.
    pub light_buffer: NeonLightBuffer,
    
    /// Frame statistics.
    pub stats: RenderStats,
}

impl RenderFrame {
    /// Returns the instance data as a byte slice.
    #[must_use]
    pub fn instance_bytes(&self) -> &[u8] {
        &self.instance_data
    }
    
    /// Returns the fog uniforms as bytes.
    #[must_use]
    pub fn fog_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(&self.fog_uniforms)
    }
    
    /// Returns the light buffer as bytes.
    #[must_use]
    pub fn light_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(&self.light_buffer)
    }
    
    /// Returns true if there's anything to render.
    #[must_use]
    pub fn has_content(&self) -> bool {
        !self.instance_data.is_empty()
    }
    
    /// Returns the number of instances.
    #[must_use]
    pub fn instance_count(&self) -> u32 {
        self.stats.instances
    }
}
