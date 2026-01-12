//! Frustum culling for view-dependent rendering.
//!
//! Extracts frustum planes from the view-projection matrix and tests
//! bounding volumes against them.

use bytemuck::{Pod, Zeroable};
use std::sync::atomic::{AtomicU32, AtomicBool, Ordering};

// =============================================================================
// OPERATION PANOPTICON - CULLING DIAGNOSTICS
// =============================================================================
/// Global counters for culling statistics
static CULL_VISIBLE_COUNT: AtomicU32 = AtomicU32::new(0);
static CULL_HIDDEN_COUNT: AtomicU32 = AtomicU32::new(0);
static CULL_LOG_ENABLED: AtomicBool = AtomicBool::new(true);

/// A plane in 3D space (Ax + By + Cz + D = 0).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct Plane {
    /// Normal X component.
    pub a: f32,
    /// Normal Y component.
    pub b: f32,
    /// Normal Z component.
    pub c: f32,
    /// Distance from origin.
    pub d: f32,
}

impl Plane {
    /// Creates a new plane.
    #[must_use]
    pub const fn new(a: f32, b: f32, c: f32, d: f32) -> Self {
        Self { a, b, c, d }
    }
    
    /// Normalizes the plane.
    #[must_use]
    pub fn normalized(self) -> Self {
        let len = (self.a * self.a + self.b * self.b + self.c * self.c).sqrt();
        if len > 0.0 {
            Self {
                a: self.a / len,
                b: self.b / len,
                c: self.c / len,
                d: self.d / len,
            }
        } else {
            self
        }
    }
    
    /// Returns the signed distance from a point to the plane.
    #[inline]
    #[must_use]
    pub fn distance_to_point(&self, x: f32, y: f32, z: f32) -> f32 {
        self.a * x + self.b * y + self.c * z + self.d
    }
    
    /// Converts to array format.
    #[must_use]
    pub const fn as_array(&self) -> [f32; 4] {
        [self.a, self.b, self.c, self.d]
    }
}

/// View frustum for culling.
#[derive(Debug, Clone, Copy, Default)]
pub struct Frustum {
    /// Left, right, bottom, top, near, far planes.
    pub planes: [Plane; 6],
}

impl Frustum {
    /// Plane indices.
    pub const LEFT: usize = 0;
    /// Right plane index.
    pub const RIGHT: usize = 1;
    /// Bottom plane index.
    pub const BOTTOM: usize = 2;
    /// Top plane index.
    pub const TOP: usize = 3;
    /// Near plane index.
    pub const NEAR: usize = 4;
    /// Far plane index.
    pub const FAR: usize = 5;
    
    /// Extracts frustum planes from a view-projection matrix.
    ///
    /// The matrix should be in column-major order (OpenGL/WGPU convention).
    #[must_use]
    pub fn from_view_projection(m: &[[f32; 4]; 4]) -> Self {
        let mut planes = [Plane::default(); 6];
        
        // Left plane: row3 + row0
        planes[Self::LEFT] = Plane::new(
            m[0][3] + m[0][0],
            m[1][3] + m[1][0],
            m[2][3] + m[2][0],
            m[3][3] + m[3][0],
        ).normalized();
        
        // Right plane: row3 - row0
        planes[Self::RIGHT] = Plane::new(
            m[0][3] - m[0][0],
            m[1][3] - m[1][0],
            m[2][3] - m[2][0],
            m[3][3] - m[3][0],
        ).normalized();
        
        // Bottom plane: row3 + row1
        planes[Self::BOTTOM] = Plane::new(
            m[0][3] + m[0][1],
            m[1][3] + m[1][1],
            m[2][3] + m[2][1],
            m[3][3] + m[3][1],
        ).normalized();
        
        // Top plane: row3 - row1
        planes[Self::TOP] = Plane::new(
            m[0][3] - m[0][1],
            m[1][3] - m[1][1],
            m[2][3] - m[2][1],
            m[3][3] - m[3][1],
        ).normalized();
        
        // Near plane: row3 + row2
        planes[Self::NEAR] = Plane::new(
            m[0][3] + m[0][2],
            m[1][3] + m[1][2],
            m[2][3] + m[2][2],
            m[3][3] + m[3][2],
        ).normalized();
        
        // Far plane: row3 - row2
        planes[Self::FAR] = Plane::new(
            m[0][3] - m[0][2],
            m[1][3] - m[1][2],
            m[2][3] - m[2][2],
            m[3][3] - m[3][2],
        ).normalized();
        
        Self { planes }
    }
    
    /// Converts planes to array format for GPU upload.
    #[must_use]
    pub fn as_arrays(&self) -> [[f32; 4]; 6] {
        [
            self.planes[0].as_array(),
            self.planes[1].as_array(),
            self.planes[2].as_array(),
            self.planes[3].as_array(),
            self.planes[4].as_array(),
            self.planes[5].as_array(),
        ]
    }
}

/// Axis-aligned bounding box for culling.
#[derive(Debug, Clone, Copy, Default)]
pub struct AABB {
    /// Minimum corner.
    pub min: [f32; 3],
    /// Maximum corner.
    pub max: [f32; 3],
}

impl AABB {
    /// Creates a new AABB.
    #[must_use]
    pub const fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Self { min, max }
    }
    
    /// Creates an AABB for a chunk at the given coordinate.
    #[must_use]
    pub fn for_chunk(chunk_x: i32, chunk_y: i32, chunk_z: i32, chunk_size: f32) -> Self {
        let min_x = chunk_x as f32 * chunk_size;
        let min_y = chunk_y as f32 * chunk_size;
        let min_z = chunk_z as f32 * chunk_size;
        
        Self {
            min: [min_x, min_y, min_z],
            max: [min_x + chunk_size, min_y + chunk_size, min_z + chunk_size],
        }
    }
    
    /// Returns the center of the AABB.
    #[must_use]
    pub fn center(&self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }
    
    /// Returns the half-extents of the AABB.
    #[must_use]
    pub fn half_extents(&self) -> [f32; 3] {
        [
            (self.max[0] - self.min[0]) * 0.5,
            (self.max[1] - self.min[1]) * 0.5,
            (self.max[2] - self.min[2]) * 0.5,
        ]
    }
}

/// Frustum culler for efficient visibility testing.
pub struct FrustumCuller {
    /// Current frustum.
    frustum: Frustum,
}

impl FrustumCuller {
    /// Creates a new frustum culler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            frustum: Frustum::default(),
        }
    }
    
    /// Updates the frustum from a view-projection matrix.
    pub fn update(&mut self, view_projection: &[[f32; 4]; 4]) {
        self.frustum = Frustum::from_view_projection(view_projection);
    }
    
    /// Returns the current frustum planes for GPU upload.
    #[must_use]
    pub fn planes(&self) -> [[f32; 4]; 6] {
        self.frustum.as_arrays()
    }
    
    /// Tests if a sphere is visible (intersects the frustum).
    #[must_use]
    pub fn test_sphere(&self, center: [f32; 3], radius: f32) -> bool {
        for plane in &self.frustum.planes {
            let distance = plane.distance_to_point(center[0], center[1], center[2]);
            if distance < -radius {
                return false;
            }
        }
        true
    }
    
    /// Tests if an AABB is visible (intersects the frustum).
    /// OPTIMIZED: Full frustum plane testing for maximum culling
    #[must_use]
    pub fn test_aabb(&self, aabb: &AABB) -> bool {
        let center = aabb.center();
        let half = aabb.half_extents();
        
        for plane in &self.frustum.planes {
            // Compute the projection interval radius
            let r = half[0] * plane.a.abs()
                + half[1] * plane.b.abs()
                + half[2] * plane.c.abs();
            
            // Compute distance from center to plane
            let d = plane.distance_to_point(center[0], center[1], center[2]);
            
            // If distance is less than -radius, AABB is outside
            if d < -r {
                return false;
            }
        }
        
        true
    }
    
    /// Tests if a chunk is visible.
    /// OPTIMIZED: Chunk-specific frustum test with PANOPTICON logging
    #[must_use]
    pub fn test_chunk(&self, chunk_x: i32, chunk_y: i32, chunk_z: i32) -> bool {
        let aabb = AABB::for_chunk(chunk_x, chunk_y, chunk_z, 32.0);
        let visible = self.test_aabb(&aabb);
        
        // PANOPTICON: Track visibility statistics
        if visible {
            CULL_VISIBLE_COUNT.fetch_add(1, Ordering::Relaxed);
        } else {
            CULL_HIDDEN_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        
        visible
    }
    
    /// PANOPTICON: Log visibility change for a chunk
    pub fn log_visibility_change(chunk_x: i32, chunk_z: i32, was_visible: bool, is_visible: bool) {
        if CULL_LOG_ENABLED.load(Ordering::Relaxed) && was_visible != is_visible {
            let status = if is_visible { "VISIBLE" } else { "HIDDEN" };
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            println!("[{:>12}] [CULL] Chunk [{},{}] changed visibility: {}",
                timestamp, chunk_x, chunk_z, status);
        }
    }
    
    /// PANOPTICON: Get and reset culling statistics
    pub fn get_cull_stats() -> (u32, u32) {
        let visible = CULL_VISIBLE_COUNT.swap(0, Ordering::Relaxed);
        let hidden = CULL_HIDDEN_COUNT.swap(0, Ordering::Relaxed);
        (visible, hidden)
    }
    
    /// PANOPTICON: Print culling summary
    pub fn print_cull_summary() {
        let (visible, hidden) = Self::get_cull_stats();
        if visible > 0 || hidden > 0 {
            let total = visible + hidden;
            let cull_percent = if total > 0 { (hidden as f32 / total as f32) * 100.0 } else { 0.0 };
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            println!("[{:>12}] [CULL STATS] Visible: {}, Hidden: {}, Cull Rate: {:.1}%",
                timestamp, visible, hidden, cull_percent);
        }
    }
}

impl Default for FrustumCuller {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plane_normalization() {
        let plane = Plane::new(3.0, 4.0, 0.0, 10.0);
        let normalized = plane.normalized();
        
        // 3-4-5 triangle, so length is 5
        assert!((normalized.a - 0.6).abs() < 0.001);
        assert!((normalized.b - 0.8).abs() < 0.001);
    }
    
    #[test]
    fn test_aabb_center() {
        let aabb = AABB::new([0.0, 0.0, 0.0], [32.0, 32.0, 32.0]);
        let center = aabb.center();
        
        assert_eq!(center, [16.0, 16.0, 16.0]);
    }
}
