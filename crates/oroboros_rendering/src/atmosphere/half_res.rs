//! Half-Resolution Effect Rendering.
//!
//! ARCHITECT'S FEEDBACK: Ray-marched fog at full res = GPU meltdown.
//!
//! Solution:
//! - Render expensive effects at HALF resolution
//! - Use bilateral upscale to preserve edges
//! - Apply temporal accumulation for stability
//!
//! Performance budget:
//! - Full res fog: ~4ms @ 4K
//! - Half res fog: ~1ms @ 4K (4x faster!)
//! - Upscale cost: ~0.3ms
//! - Net savings: 2.7ms per frame

use bytemuck::{Pod, Zeroable};

/// Configuration for half-resolution rendering.
#[derive(Debug, Clone, Copy)]
pub struct HalfResConfig {
    /// Scale factor (0.5 = half resolution).
    pub scale: f32,
    /// Enable temporal accumulation.
    pub temporal: bool,
    /// Temporal blend factor (higher = more stable, more ghosting).
    pub temporal_blend: f32,
    /// Depth threshold for bilateral upscale.
    pub depth_threshold: f32,
    /// Normal threshold for bilateral upscale.
    pub normal_threshold: f32,
}

impl Default for HalfResConfig {
    fn default() -> Self {
        Self {
            scale: 0.5,
            temporal: true,
            temporal_blend: 0.9,
            depth_threshold: 0.1,
            normal_threshold: 0.5,
        }
    }
}

/// Half-resolution render target dimensions.
#[derive(Debug, Clone, Copy, Default)]
pub struct HalfResDimensions {
    /// Full resolution width.
    pub full_width: u32,
    /// Full resolution height.
    pub full_height: u32,
    /// Half resolution width.
    pub half_width: u32,
    /// Half resolution height.
    pub half_height: u32,
}

impl HalfResDimensions {
    /// Calculates dimensions from full resolution and scale.
    #[must_use]
    pub fn new(full_width: u32, full_height: u32, scale: f32) -> Self {
        Self {
            full_width,
            full_height,
            half_width: ((full_width as f32 * scale) as u32).max(1),
            half_height: ((full_height as f32 * scale) as u32).max(1),
        }
    }
    
    /// Memory savings ratio.
    #[must_use]
    pub fn savings_ratio(&self) -> f32 {
        let full_pixels = self.full_width * self.full_height;
        let half_pixels = self.half_width * self.half_height;
        1.0 - (half_pixels as f32 / full_pixels as f32)
    }
}

/// Uniforms for bilateral upscale shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct UpscaleUniforms {
    /// Full resolution (width, height, 1/width, 1/height).
    pub full_res: [f32; 4],
    /// Half resolution (width, height, 1/width, 1/height).
    pub half_res: [f32; 4],
    /// Depth threshold, normal threshold, temporal blend, scale.
    pub params: [f32; 4],
}

impl UpscaleUniforms {
    /// Creates uniforms from dimensions and config.
    #[must_use]
    pub fn new(dims: HalfResDimensions, config: &HalfResConfig) -> Self {
        Self {
            full_res: [
                dims.full_width as f32,
                dims.full_height as f32,
                1.0 / dims.full_width as f32,
                1.0 / dims.full_height as f32,
            ],
            half_res: [
                dims.half_width as f32,
                dims.half_height as f32,
                1.0 / dims.half_width as f32,
                1.0 / dims.half_height as f32,
            ],
            params: [
                config.depth_threshold,
                config.normal_threshold,
                config.temporal_blend,
                config.scale,
            ],
        }
    }
}

/// Manager for half-resolution effect rendering.
pub struct HalfResRenderer {
    /// Current configuration.
    config: HalfResConfig,
    /// Current dimensions.
    dims: HalfResDimensions,
    /// Frame index for temporal jitter.
    frame_index: u32,
}

impl HalfResRenderer {
    /// Creates a new half-res renderer.
    #[must_use]
    pub fn new(full_width: u32, full_height: u32) -> Self {
        let config = HalfResConfig::default();
        Self {
            dims: HalfResDimensions::new(full_width, full_height, config.scale),
            config,
            frame_index: 0,
        }
    }
    
    /// Updates for a new frame.
    pub fn begin_frame(&mut self) {
        self.frame_index = self.frame_index.wrapping_add(1);
    }
    
    /// Resizes the renderer.
    pub fn resize(&mut self, full_width: u32, full_height: u32) {
        self.dims = HalfResDimensions::new(full_width, full_height, self.config.scale);
    }
    
    /// Returns the half-resolution dimensions.
    #[must_use]
    pub fn half_dimensions(&self) -> (u32, u32) {
        (self.dims.half_width, self.dims.half_height)
    }
    
    /// Returns temporal jitter offset for current frame (sub-pixel).
    ///
    /// This is used to add slight offset each frame for temporal accumulation.
    #[must_use]
    pub fn jitter_offset(&self) -> (f32, f32) {
        if !self.config.temporal {
            return (0.0, 0.0);
        }
        
        // Halton sequence for good coverage
        let halton_x = halton(self.frame_index, 2);
        let halton_y = halton(self.frame_index, 3);
        
        // Scale to pixel offset
        let pixel_offset_x = (halton_x - 0.5) / self.dims.half_width as f32;
        let pixel_offset_y = (halton_y - 0.5) / self.dims.half_height as f32;
        
        (pixel_offset_x, pixel_offset_y)
    }
    
    /// Returns upscale uniforms.
    #[must_use]
    pub fn upscale_uniforms(&self) -> UpscaleUniforms {
        UpscaleUniforms::new(self.dims, &self.config)
    }
    
    /// Returns the WGSL source for bilateral upscale.
    #[must_use]
    pub fn upscale_shader() -> &'static str {
        include_str!("../../shaders/bilateral_upscale.wgsl")
    }
    
    /// Returns memory savings percentage.
    #[must_use]
    pub fn memory_savings(&self) -> f32 {
        self.dims.savings_ratio() * 100.0
    }
    
    /// Returns performance estimate (multiplier vs full res).
    #[must_use]
    pub fn performance_multiplier(&self) -> f32 {
        // Rendering cost scales with pixel count
        let full_pixels = self.dims.full_width * self.dims.full_height;
        let half_pixels = self.dims.half_width * self.dims.half_height;
        full_pixels as f32 / half_pixels as f32
    }
}

impl Default for HalfResRenderer {
    fn default() -> Self {
        Self::new(1920, 1080)
    }
}

/// Halton sequence for low-discrepancy sampling.
fn halton(index: u32, base: u32) -> f32 {
    let mut f = 1.0;
    let mut r = 0.0;
    let mut i = index;
    
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_half_res_savings() {
        let dims = HalfResDimensions::new(3840, 2160, 0.5);
        
        // 4K -> ~2K
        assert_eq!(dims.half_width, 1920);
        assert_eq!(dims.half_height, 1080);
        
        // 75% memory savings
        let savings = dims.savings_ratio();
        assert!((savings - 0.75).abs() < 0.01);
    }
    
    #[test]
    fn test_performance_multiplier() {
        let renderer = HalfResRenderer::new(3840, 2160);
        
        // Half res = 4x performance gain
        let multiplier = renderer.performance_multiplier();
        assert!((multiplier - 4.0).abs() < 0.1);
    }
}
