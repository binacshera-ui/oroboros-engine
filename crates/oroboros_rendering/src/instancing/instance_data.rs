//! Instance data structures for GPU upload.

use bytemuck::{Pod, Zeroable};

/// Per-instance data sent to the GPU.
///
/// This struct is uploaded to the instance buffer and consumed by the vertex shader.
/// Memory layout is optimized for GPU cache efficiency (16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct InstanceData {
    /// World position (x, y, z) + scale packed in w.
    pub position_scale: [f32; 4],
    
    /// Quad dimensions (width, height) + normal index + material ID.
    pub dimensions_normal_material: [f32; 4],
    
    /// Color/emission data for neon effects.
    /// RGB in xyz, intensity in w.
    pub emission: [f32; 4],
    
    /// UV coordinates and atlas info.
    /// UV offset in xy, UV scale in zw.
    pub uv_offset_scale: [f32; 4],
}

impl InstanceData {
    /// Size in bytes.
    pub const SIZE: usize = std::mem::size_of::<Self>();
    
    /// Creates a new instance from mesh quad data.
    #[must_use]
    pub fn from_quad(
        x: f32,
        y: f32,
        z: f32,
        width: f32,
        height: f32,
        normal: u32,
        material_id: u32,
        light_level: u32,
    ) -> Self {
        Self {
            position_scale: [x, y, z, 1.0],
            dimensions_normal_material: [width, height, normal as f32, material_id as f32],
            emission: [0.0, 0.0, 0.0, light_level as f32 / 255.0],
            uv_offset_scale: [0.0, 0.0, 1.0, 1.0],
        }
    }
    
    /// Creates a neon instance with emission.
    #[must_use]
    pub fn neon(
        x: f32,
        y: f32,
        z: f32,
        width: f32,
        height: f32,
        normal: u32,
        material_id: u32,
        emission_r: f32,
        emission_g: f32,
        emission_b: f32,
        intensity: f32,
    ) -> Self {
        Self {
            position_scale: [x, y, z, 1.0],
            dimensions_normal_material: [width, height, normal as f32, material_id as f32],
            emission: [emission_r, emission_g, emission_b, intensity],
            uv_offset_scale: [0.0, 0.0, 1.0, 1.0],
        }
    }
    
    /// Sets emission color for this instance.
    pub fn with_emission(mut self, r: f32, g: f32, b: f32, intensity: f32) -> Self {
        self.emission = [r, g, b, intensity];
        self
    }
    
    /// Sets UV coordinates for texture atlas.
    pub fn with_uv(mut self, u_offset: f32, v_offset: f32, u_scale: f32, v_scale: f32) -> Self {
        self.uv_offset_scale = [u_offset, v_offset, u_scale, v_scale];
        self
    }
}

/// Indirect draw command for GPU-driven rendering.
///
/// This is used with `draw_indexed_indirect` to let the GPU
/// control how many instances to draw after culling.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct DrawIndexedIndirectCommand {
    /// Number of indices per instance (6 for a quad = 2 triangles).
    pub index_count: u32,
    /// Number of instances to draw (filled by GPU culling shader).
    pub instance_count: u32,
    /// First index in the index buffer.
    pub first_index: u32,
    /// Vertex offset added to each index.
    pub base_vertex: i32,
    /// First instance ID.
    pub first_instance: u32,
}

impl DrawIndexedIndirectCommand {
    /// Creates a command for drawing quads.
    #[must_use]
    pub const fn for_quads(instance_count: u32) -> Self {
        Self {
            index_count: 6, // 2 triangles per quad
            instance_count,
            first_index: 0,
            base_vertex: 0,
            first_instance: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_instance_data_size() {
        // Should be 64 bytes (4 vec4s * 16 bytes each)
        assert_eq!(InstanceData::SIZE, 64);
    }
    
    #[test]
    fn test_instance_alignment() {
        // Ensure proper alignment for GPU
        assert_eq!(std::mem::align_of::<InstanceData>(), 4);
    }
}
