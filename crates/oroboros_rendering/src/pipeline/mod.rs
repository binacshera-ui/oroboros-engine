//! Render pipeline orchestration.
//!
//! Manages the complete rendering pipeline from voxels to final frame.

mod frame;
mod stats;

pub use frame::RenderFrame;
pub use stats::RenderStats;

use crate::voxel::VoxelWorld;
use crate::instancing::InstancedRenderer;
use crate::culling::FrustumCuller;
use crate::atmosphere::{VolumetricFog, NeonLighting, ScreenSpaceReflections};

/// Complete render pipeline for OROBOROS.
///
/// This orchestrates all rendering subsystems into a cohesive whole.
pub struct RenderPipeline {
    /// Instanced voxel renderer.
    instanced_renderer: InstancedRenderer,
    
    /// Frustum culler.
    frustum_culler: FrustumCuller,
    
    /// Volumetric fog effect.
    volumetric_fog: VolumetricFog,
    
    /// Neon lighting system.
    neon_lighting: NeonLighting,
    
    /// Screen-space reflections.
    ssr: ScreenSpaceReflections,
    
    /// Frame statistics.
    stats: RenderStats,
}

impl RenderPipeline {
    /// Creates a new render pipeline.
    #[must_use]
    pub fn new() -> Self {
        Self {
            instanced_renderer: InstancedRenderer::new(),
            frustum_culler: FrustumCuller::new(),
            volumetric_fog: VolumetricFog::new(),
            neon_lighting: NeonLighting::new(),
            ssr: ScreenSpaceReflections::new(),
            stats: RenderStats::default(),
        }
    }
    
    /// Updates the pipeline for a new frame.
    pub fn begin_frame(&mut self, delta_time: f32) {
        self.neon_lighting.begin_frame(delta_time);
        self.stats = RenderStats::default();
    }
    
    /// Updates chunk meshes from world changes.
    pub fn update_meshes(&mut self, world: &VoxelWorld) {
        self.instanced_renderer.update_meshes(world);
    }
    
    /// Updates view frustum from camera.
    pub fn update_camera(&mut self, view_projection: &[[f32; 4]; 4]) {
        self.frustum_culler.update(view_projection);
    }
    
    /// Sets weather conditions.
    pub fn set_weather(&mut self, fog_intensity: f32, wetness: f32) {
        self.volumetric_fog.set_weather(fog_intensity);
        self.ssr.set_wetness(wetness);
    }
    
    /// Adds a neon light to the scene.
    pub fn add_neon_light(&mut self, light: crate::atmosphere::NeonLight) -> bool {
        self.neon_lighting.add_light(light)
    }
    
    /// Prepares all render data for GPU upload.
    ///
    /// Returns a `RenderFrame` containing all data needed for rendering.
    pub fn prepare_frame(
        &mut self,
        camera_pos: [f32; 3],
        inv_view_proj: [[f32; 4]; 4],
    ) -> RenderFrame {
        let frustum_planes = self.frustum_culler.planes();
        
        // Prepare instance data
        let instance_data = self.instanced_renderer.prepare_frame(camera_pos, &frustum_planes).to_vec();
        let instancer_stats = self.instanced_renderer.stats();
        
        // Prepare atmosphere data
        let fog_uniforms = self.volumetric_fog.create_uniforms(camera_pos, inv_view_proj);
        
        // Update stats
        self.stats.draw_calls = instancer_stats.draw_calls;
        self.stats.instances = instancer_stats.instance_count;
        self.stats.chunks_rendered = instancer_stats.chunks_rendered;
        self.stats.chunks_culled = instancer_stats.chunks_culled;
        self.stats.neon_lights = self.neon_lighting.light_count();
        
        RenderFrame {
            instance_data,
            fog_uniforms,
            light_buffer: *self.neon_lighting.buffer(),
            stats: self.stats,
        }
    }
    
    /// Returns rendering statistics.
    #[must_use]
    pub const fn stats(&self) -> RenderStats {
        self.stats
    }
    
    /// Returns a reference to the volumetric fog system.
    #[must_use]
    pub fn fog(&self) -> &VolumetricFog {
        &self.volumetric_fog
    }
    
    /// Returns a mutable reference to the volumetric fog system.
    pub fn fog_mut(&mut self) -> &mut VolumetricFog {
        &mut self.volumetric_fog
    }
    
    /// Returns a reference to the SSR system.
    #[must_use]
    pub fn ssr(&self) -> &ScreenSpaceReflections {
        &self.ssr
    }
    
    /// Returns a mutable reference to the SSR system.
    pub fn ssr_mut(&mut self) -> &mut ScreenSpaceReflections {
        &mut self.ssr
    }
}

impl Default for RenderPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pipeline_creation() {
        let pipeline = RenderPipeline::new();
        assert_eq!(pipeline.stats().draw_calls, 0);
    }
}
