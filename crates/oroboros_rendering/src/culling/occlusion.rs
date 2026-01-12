//! GPU-accelerated occlusion culling.
//!
//! Uses hierarchical Z-buffer (HiZ) for efficient visibility testing.
//! This runs entirely on the GPU via compute shaders.

/// Configuration for occlusion culling.
#[derive(Debug, Clone, Copy)]
pub struct OcclusionConfig {
    /// Resolution of the HiZ buffer (power of 2).
    pub hiz_resolution: u32,
    /// Number of mip levels for HiZ.
    pub mip_levels: u32,
    /// Enable temporal reprojection for smoother culling.
    pub temporal_reprojection: bool,
}

impl Default for OcclusionConfig {
    fn default() -> Self {
        Self {
            hiz_resolution: 512,
            mip_levels: 9, // 512 -> 256 -> ... -> 1
            temporal_reprojection: true,
        }
    }
}

/// GPU occlusion culler using Hierarchical Z-buffer.
///
/// This is designed to run entirely on the GPU:
/// 1. Render depth from previous frame
/// 2. Generate HiZ mip chain (compute shader)
/// 3. Test bounding boxes against HiZ (compute shader)
/// 4. Output visible instance indices
pub struct OcclusionCuller {
    /// Configuration.
    config: OcclusionConfig,
    
    /// Statistics from last frame.
    stats: OcclusionStats,
}

/// Statistics from occlusion culling.
#[derive(Debug, Clone, Copy, Default)]
pub struct OcclusionStats {
    /// Total objects tested.
    pub objects_tested: u32,
    /// Objects that passed (visible).
    pub objects_visible: u32,
    /// Objects culled (occluded).
    pub objects_culled: u32,
}

impl OcclusionCuller {
    /// Creates a new occlusion culler with default config.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(OcclusionConfig::default())
    }
    
    /// Creates a new occlusion culler with custom config.
    #[must_use]
    pub fn with_config(config: OcclusionConfig) -> Self {
        Self {
            config,
            stats: OcclusionStats::default(),
        }
    }
    
    /// Returns the WGSL source for the HiZ generation compute shader.
    #[must_use]
    pub fn hiz_generate_shader() -> &'static str {
        include_str!("../../shaders/occlusion_hiz_generate.wgsl")
    }
    
    /// Returns the WGSL source for the occlusion test compute shader.
    #[must_use]
    pub fn occlusion_test_shader() -> &'static str {
        include_str!("../../shaders/occlusion_test.wgsl")
    }
    
    /// Returns the configuration.
    #[must_use]
    pub const fn config(&self) -> OcclusionConfig {
        self.config
    }
    
    /// Returns the statistics from the last frame.
    #[must_use]
    pub const fn stats(&self) -> OcclusionStats {
        self.stats
    }
    
    /// Updates statistics (called after GPU readback).
    pub fn update_stats(&mut self, tested: u32, visible: u32) {
        self.stats = OcclusionStats {
            objects_tested: tested,
            objects_visible: visible,
            objects_culled: tested.saturating_sub(visible),
        };
    }
}

impl Default for OcclusionCuller {
    fn default() -> Self {
        Self::new()
    }
}

/// Bounding box data for GPU occlusion testing.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OcclusionBounds {
    /// Minimum corner.
    pub min_x: f32,
    /// Minimum Y.
    pub min_y: f32,
    /// Minimum Z.
    pub min_z: f32,
    /// Object index.
    pub index: u32,
    /// Maximum corner.
    pub max_x: f32,
    /// Maximum Y.
    pub max_y: f32,
    /// Maximum Z.
    pub max_z: f32,
    /// Padding.
    pub _pad: u32,
}

impl OcclusionBounds {
    /// Creates bounds for a chunk.
    #[must_use]
    #[allow(dead_code)]
    pub fn for_chunk(chunk_x: i32, chunk_y: i32, chunk_z: i32, index: u32) -> Self {
        let min_x = chunk_x as f32 * 32.0;
        let min_y = chunk_y as f32 * 32.0;
        let min_z = chunk_z as f32 * 32.0;
        
        Self {
            min_x,
            min_y,
            min_z,
            index,
            max_x: min_x + 32.0,
            max_y: min_y + 32.0,
            max_z: min_z + 32.0,
            _pad: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_occlusion_bounds() {
        let bounds = OcclusionBounds::for_chunk(1, 2, 3, 42);
        
        assert_eq!(bounds.min_x, 32.0);
        assert_eq!(bounds.min_y, 64.0);
        assert_eq!(bounds.min_z, 96.0);
        assert_eq!(bounds.index, 42);
    }
    
    #[test]
    fn test_config_defaults() {
        let config = OcclusionConfig::default();
        assert_eq!(config.hiz_resolution, 512);
        assert!(config.temporal_reprojection);
    }
}
