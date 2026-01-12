//! Screen-Space Reflections for wet surfaces.
//!
//! Creates realistic reflections on wet roads and glossy surfaces
//! using ray marching in screen space.

use bytemuck::{Pod, Zeroable};

/// Configuration for screen-space reflections.
#[derive(Debug, Clone, Copy)]
pub struct SSRConfig {
    /// Maximum ray length in pixels.
    pub max_ray_length: f32,
    /// Ray step size in pixels.
    pub step_size: f32,
    /// Maximum number of steps.
    pub max_steps: u32,
    /// Thickness threshold for hit detection.
    pub thickness: f32,
    /// Reflection intensity.
    pub intensity: f32,
    /// Edge fade distance (prevents artifacts at screen edges).
    pub edge_fade: f32,
    /// Roughness threshold (surfaces rougher than this don't reflect).
    pub roughness_cutoff: f32,
}

impl Default for SSRConfig {
    fn default() -> Self {
        Self {
            max_ray_length: 1000.0,
            step_size: 2.0,
            max_steps: 128,
            thickness: 0.5,
            intensity: 0.8,
            edge_fade: 0.1,
            roughness_cutoff: 0.5,
        }
    }
}

/// Uniform buffer for SSR shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct SSRUniforms {
    /// Projection matrix.
    pub projection: [[f32; 4]; 4],
    /// Inverse projection matrix.
    pub inv_projection: [[f32; 4]; 4],
    /// View matrix.
    pub view: [[f32; 4]; 4],
    /// Screen resolution (width, height, 1/width, 1/height).
    pub resolution: [f32; 4],
    /// Ray config (max_length, step_size, max_steps, thickness).
    pub ray_config: [f32; 4],
    /// Effect params (intensity, edge_fade, roughness_cutoff, padding).
    pub effect_params: [f32; 4],
}

impl SSRUniforms {
    /// Creates uniforms from config and matrices.
    #[must_use]
    pub fn from_config(
        config: &SSRConfig,
        projection: [[f32; 4]; 4],
        inv_projection: [[f32; 4]; 4],
        view: [[f32; 4]; 4],
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            projection,
            inv_projection,
            view,
            resolution: [
                width as f32,
                height as f32,
                1.0 / width as f32,
                1.0 / height as f32,
            ],
            ray_config: [
                config.max_ray_length,
                config.step_size,
                config.max_steps as f32,
                config.thickness,
            ],
            effect_params: [
                config.intensity,
                config.edge_fade,
                config.roughness_cutoff,
                0.0,
            ],
        }
    }
}

/// Screen-space reflections manager.
pub struct ScreenSpaceReflections {
    /// Current configuration.
    config: SSRConfig,
    /// Wetness factor (0 = dry, 1 = soaked).
    wetness: f32,
}

impl ScreenSpaceReflections {
    /// Creates a new SSR system.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SSRConfig::default(),
            wetness: 0.0,
        }
    }
    
    /// Creates with custom configuration.
    #[must_use]
    pub fn with_config(config: SSRConfig) -> Self {
        Self {
            config,
            wetness: 0.0,
        }
    }
    
    /// Returns the WGSL source for the SSR shader.
    #[must_use]
    pub fn shader_source() -> &'static str {
        include_str!("../../shaders/ssr.wgsl")
    }
    
    /// Sets the wetness level (for rain effects).
    pub fn set_wetness(&mut self, wetness: f32) {
        self.wetness = wetness.clamp(0.0, 1.0);
    }
    
    /// Sets reflection intensity.
    pub fn set_intensity(&mut self, intensity: f32) {
        self.config.intensity = intensity.clamp(0.0, 1.0);
    }
    
    /// Returns effective config with wetness applied.
    #[must_use]
    pub fn effective_config(&self) -> SSRConfig {
        let mut config = self.config;
        // Wetness increases reflection intensity and decreases roughness cutoff
        config.intensity = (config.intensity + self.wetness * 0.3).min(1.0);
        config.roughness_cutoff = (config.roughness_cutoff + self.wetness * 0.3).min(1.0);
        config
    }
    
    /// Creates uniform data for shader.
    #[must_use]
    pub fn create_uniforms(
        &self,
        projection: [[f32; 4]; 4],
        inv_projection: [[f32; 4]; 4],
        view: [[f32; 4]; 4],
        width: u32,
        height: u32,
    ) -> SSRUniforms {
        SSRUniforms::from_config(
            &self.effective_config(),
            projection,
            inv_projection,
            view,
            width,
            height,
        )
    }
    
    /// Returns the current wetness level.
    #[must_use]
    pub fn wetness(&self) -> f32 {
        self.wetness
    }
}

impl Default for ScreenSpaceReflections {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_wetness_affects_config() {
        let mut ssr = ScreenSpaceReflections::new();
        let dry_config = ssr.effective_config();
        
        ssr.set_wetness(1.0);
        let wet_config = ssr.effective_config();
        
        assert!(wet_config.intensity >= dry_config.intensity);
    }
    
    #[test]
    fn test_uniforms_size() {
        // Must be properly aligned for GPU
        let size = std::mem::size_of::<SSRUniforms>();
        assert_eq!(size % 16, 0); // 16-byte aligned
    }
}
