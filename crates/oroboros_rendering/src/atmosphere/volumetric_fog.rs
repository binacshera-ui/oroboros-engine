//! Volumetric fog system for atmospheric depth.
//!
//! Creates the cyberpunk atmosphere of Neon Prime with
//! colored fog that interacts with neon light sources.

use bytemuck::{Pod, Zeroable};

/// Configuration for volumetric fog.
#[derive(Debug, Clone, Copy)]
pub struct VolumetricFogConfig {
    /// Base fog color (RGB).
    pub base_color: [f32; 3],
    /// Fog density (higher = thicker fog).
    pub density: f32,
    /// Height at which fog is densest.
    pub ground_level: f32,
    /// How quickly fog fades with height.
    pub height_falloff: f32,
    /// How much fog scatters light (0-1).
    pub scattering: f32,
    /// How much neon lights affect fog color.
    pub neon_influence: f32,
    /// Number of ray marching steps (quality vs performance).
    pub ray_steps: u32,
    /// Maximum ray distance.
    pub max_distance: f32,
}

impl Default for VolumetricFogConfig {
    fn default() -> Self {
        Self {
            base_color: [0.02, 0.02, 0.05], // Dark blue-gray
            density: 0.03,
            ground_level: 0.0,
            height_falloff: 0.02,
            scattering: 0.7,
            neon_influence: 0.8,
            ray_steps: 32,
            max_distance: 500.0,
        }
    }
}

/// Uniform buffer data for volumetric fog shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct VolumetricFogUniforms {
    /// Fog color with density in alpha.
    pub color_density: [f32; 4],
    /// Ground level, height falloff, scattering, neon influence.
    pub parameters: [f32; 4],
    /// Ray steps, max distance, padding.
    pub ray_config: [f32; 4],
    /// Camera position for ray origin.
    pub camera_pos: [f32; 4],
    /// Inverse view-projection matrix (for ray direction reconstruction).
    pub inv_view_proj: [[f32; 4]; 4],
}

impl VolumetricFogUniforms {
    /// Creates uniforms from config and camera data.
    #[must_use]
    pub fn from_config(
        config: &VolumetricFogConfig,
        camera_pos: [f32; 3],
        inv_view_proj: [[f32; 4]; 4],
    ) -> Self {
        Self {
            color_density: [
                config.base_color[0],
                config.base_color[1],
                config.base_color[2],
                config.density,
            ],
            parameters: [
                config.ground_level,
                config.height_falloff,
                config.scattering,
                config.neon_influence,
            ],
            ray_config: [
                config.ray_steps as f32,
                config.max_distance,
                0.0,
                0.0,
            ],
            camera_pos: [camera_pos[0], camera_pos[1], camera_pos[2], 1.0],
            inv_view_proj,
        }
    }
}

/// Volumetric fog effect manager.
pub struct VolumetricFog {
    /// Current configuration.
    config: VolumetricFogConfig,
    /// Weather intensity (0 = clear, 1 = heavy rain/fog).
    weather_intensity: f32,
}

impl VolumetricFog {
    /// Creates a new volumetric fog system.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: VolumetricFogConfig::default(),
            weather_intensity: 0.5,
        }
    }
    
    /// Creates with custom configuration.
    #[must_use]
    pub fn with_config(config: VolumetricFogConfig) -> Self {
        Self {
            config,
            weather_intensity: 0.5,
        }
    }
    
    /// Returns the WGSL source for the volumetric fog shader.
    #[must_use]
    pub fn shader_source() -> &'static str {
        include_str!("../../shaders/volumetric_fog.wgsl")
    }
    
    /// Sets the weather intensity.
    pub fn set_weather(&mut self, intensity: f32) {
        self.weather_intensity = intensity.clamp(0.0, 1.0);
    }
    
    /// Sets the fog color (for time-of-day effects).
    pub fn set_color(&mut self, r: f32, g: f32, b: f32) {
        self.config.base_color = [r, g, b];
    }
    
    /// Sets fog density.
    pub fn set_density(&mut self, density: f32) {
        self.config.density = density.max(0.0);
    }
    
    /// Returns the current config with weather applied.
    #[must_use]
    pub fn effective_config(&self) -> VolumetricFogConfig {
        let mut config = self.config;
        // Weather increases density and scattering
        config.density *= 1.0 + self.weather_intensity * 2.0;
        config.scattering = (config.scattering + self.weather_intensity * 0.3).min(1.0);
        config
    }
    
    /// Creates uniform data for shader upload.
    #[must_use]
    pub fn create_uniforms(
        &self,
        camera_pos: [f32; 3],
        inv_view_proj: [[f32; 4]; 4],
    ) -> VolumetricFogUniforms {
        VolumetricFogUniforms::from_config(&self.effective_config(), camera_pos, inv_view_proj)
    }
    
}


impl Default for VolumetricFog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_weather_affects_density() {
        let mut fog = VolumetricFog::new();
        let base_density = fog.config.density;
        
        fog.set_weather(1.0);
        let heavy_config = fog.effective_config();
        
        assert!(heavy_config.density > base_density);
    }
    
    #[test]
    fn test_uniforms_size() {
        // Must be aligned for GPU buffer
        // 4x[f32;4] + 1x[[f32;4];4] = 16*4 + 64 = 128 bytes
        assert_eq!(std::mem::size_of::<VolumetricFogUniforms>(), 128);
    }
}
