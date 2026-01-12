//! GPU-Driven Rendering Pipeline.
//!
//! ARCHITECT'S FEEDBACK: CPU can't touch culling. CPU is busy with MEV.
//!
//! Solution: Full GPU-driven pipeline:
//! 1. Chunk bounds uploaded to GPU once (when chunk loads)
//! 2. Compute shader does frustum + occlusion culling
//! 3. Compute shader writes DrawIndirect arguments
//! 4. CPU submits ONE MultiDrawIndirect call (no data upload per frame)
//!
//! CPU work per frame: ZERO data transfer, just command submission.

use bytemuck::{Pod, Zeroable};

/// Per-chunk data for GPU culling.
///
/// Uploaded once when chunk is loaded, never touched again by CPU.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct ChunkGPUData {
    /// Chunk world position (min corner).
    pub position: [f32; 3],
    /// Chunk size (always 32 for now, but future-proof).
    pub size: f32,
    /// Offset into the global instance buffer.
    pub instance_offset: u32,
    /// Number of instances (quads) in this chunk.
    pub instance_count: u32,
    /// Flags (empty, fully occluded from last frame, etc).
    pub flags: u32,
    /// Padding for alignment.
    pub _pad: u32,
}

impl ChunkGPUData {
    /// Flag: chunk is empty (skip culling entirely).
    pub const FLAG_EMPTY: u32 = 1 << 0;
    /// Flag: chunk was occluded last frame (test with lower priority).
    pub const FLAG_PREV_OCCLUDED: u32 = 1 << 1;
    
    /// Creates GPU data for a chunk.
    #[must_use]
    pub fn new(
        x: i32,
        y: i32,
        z: i32,
        instance_offset: u32,
        instance_count: u32,
    ) -> Self {
        Self {
            position: [x as f32 * 32.0, y as f32 * 32.0, z as f32 * 32.0],
            size: 32.0,
            instance_offset,
            instance_count,
            flags: if instance_count == 0 { Self::FLAG_EMPTY } else { 0 },
            _pad: 0,
        }
    }
}

/// DrawIndexedIndirect arguments - filled by GPU compute shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct DrawIndexedIndirectArgs {
    /// Indices per instance (6 for quad).
    pub index_count: u32,
    /// Number of instances to draw (filled by GPU).
    pub instance_count: u32,
    /// First index.
    pub first_index: u32,
    /// Base vertex.
    pub base_vertex: i32,
    /// First instance.
    pub first_instance: u32,
}

/// Maximum chunks in multi-draw buffer.
pub const MAX_DRAW_COMMANDS: usize = 16384;

/// Multi-draw indirect buffer.
///
/// One entry per chunk. GPU culling shader sets instance_count to 0
/// for culled chunks, non-zero for visible chunks.
///
/// Note: Not Pod/Zeroable due to array size. Use heap allocation.
pub struct MultiDrawBuffer {
    /// Draw commands - one per chunk.
    /// Max 16K chunks visible at once.
    pub commands: Box<[DrawIndexedIndirectArgs; MAX_DRAW_COMMANDS]>,
    /// Number of actual draw commands.
    pub draw_count: u32,
}

impl Default for MultiDrawBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiDrawBuffer {
    /// Creates a new multi-draw buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: Box::new([DrawIndexedIndirectArgs::default(); MAX_DRAW_COMMANDS]),
            draw_count: 0,
        }
    }
    
    /// Returns commands as bytes for GPU upload.
    #[must_use]
    pub fn commands_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.commands.as_slice())
    }
}

/// GPU-driven culling uniforms.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct CullingUniforms {
    /// View-projection matrix for frustum extraction.
    pub view_proj: [[f32; 4]; 4],
    /// Camera position for distance sorting.
    pub camera_pos: [f32; 4],
    /// Frustum planes (6 planes, ABCD each).
    pub frustum_planes: [[f32; 4]; 6],
    /// Chunk count, max draw distance, LOD distances.
    pub params: [f32; 4],
}

impl CullingUniforms {
    /// Creates culling uniforms from matrices.
    #[must_use]
    pub fn new(
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
        chunk_count: u32,
        max_distance: f32,
    ) -> Self {
        let frustum_planes = extract_frustum_planes(&view_proj);
        
        Self {
            view_proj,
            camera_pos: [camera_pos[0], camera_pos[1], camera_pos[2], 1.0],
            frustum_planes,
            params: [chunk_count as f32, max_distance, 0.0, 0.0],
        }
    }
}

/// Extracts frustum planes from view-projection matrix.
fn extract_frustum_planes(m: &[[f32; 4]; 4]) -> [[f32; 4]; 6] {
    let mut planes = [[0.0f32; 4]; 6];
    
    // Left
    planes[0] = [
        m[0][3] + m[0][0], m[1][3] + m[1][0], 
        m[2][3] + m[2][0], m[3][3] + m[3][0]
    ];
    // Right
    planes[1] = [
        m[0][3] - m[0][0], m[1][3] - m[1][0], 
        m[2][3] - m[2][0], m[3][3] - m[3][0]
    ];
    // Bottom
    planes[2] = [
        m[0][3] + m[0][1], m[1][3] + m[1][1], 
        m[2][3] + m[2][1], m[3][3] + m[3][1]
    ];
    // Top
    planes[3] = [
        m[0][3] - m[0][1], m[1][3] - m[1][1], 
        m[2][3] - m[2][1], m[3][3] - m[3][1]
    ];
    // Near
    planes[4] = [
        m[0][3] + m[0][2], m[1][3] + m[1][2], 
        m[2][3] + m[2][2], m[3][3] + m[3][2]
    ];
    // Far
    planes[5] = [
        m[0][3] - m[0][2], m[1][3] - m[1][2], 
        m[2][3] - m[2][2], m[3][3] - m[3][2]
    ];
    
    // Normalize planes
    for plane in &mut planes {
        let len = (plane[0] * plane[0] + plane[1] * plane[1] + plane[2] * plane[2]).sqrt();
        if len > 0.0 {
            plane[0] /= len;
            plane[1] /= len;
            plane[2] /= len;
            plane[3] /= len;
        }
    }
    
    planes
}

/// GPU-driven render state.
///
/// This is the interface between CPU and GPU for rendering.
/// CPU uploads chunk data ONCE. GPU does all per-frame work.
pub struct GPUDrivenRenderer {
    /// Chunk GPU data (uploaded when chunks load/unload).
    chunk_data: Vec<ChunkGPUData>,
    /// Current uniforms.
    uniforms: CullingUniforms,
    /// Statistics.
    stats: GPUDrivenStats,
}

/// Statistics from GPU-driven rendering.
#[derive(Debug, Clone, Copy, Default)]
pub struct GPUDrivenStats {
    /// Total chunks in buffer.
    pub total_chunks: u32,
    /// Chunks that passed frustum culling (read back from GPU).
    pub visible_chunks: u32,
    /// Total draw calls (should be 1 with MultiDrawIndirect!).
    pub draw_calls: u32,
}

impl GPUDrivenRenderer {
    /// Creates a new GPU-driven renderer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunk_data: Vec::with_capacity(16384),
            uniforms: CullingUniforms::default(),
            stats: GPUDrivenStats::default(),
        }
    }
    
    /// Registers a chunk for GPU-driven rendering.
    ///
    /// Call this when a chunk is loaded/meshed. Returns the chunk index.
    pub fn register_chunk(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        instance_offset: u32,
        instance_count: u32,
    ) -> u32 {
        let index = self.chunk_data.len() as u32;
        self.chunk_data.push(ChunkGPUData::new(x, y, z, instance_offset, instance_count));
        index
    }
    
    /// Updates a chunk's instance data (after re-meshing).
    pub fn update_chunk(&mut self, index: u32, instance_offset: u32, instance_count: u32) {
        if let Some(data) = self.chunk_data.get_mut(index as usize) {
            data.instance_offset = instance_offset;
            data.instance_count = instance_count;
            data.flags = if instance_count == 0 { 
                ChunkGPUData::FLAG_EMPTY 
            } else { 
                data.flags & !ChunkGPUData::FLAG_EMPTY 
            };
        }
    }
    
    /// Unregisters a chunk (marks slot as empty, will be compacted later).
    pub fn unregister_chunk(&mut self, index: u32) {
        if let Some(data) = self.chunk_data.get_mut(index as usize) {
            data.instance_count = 0;
            data.flags = ChunkGPUData::FLAG_EMPTY;
        }
    }
    
    /// Updates culling uniforms for new frame.
    ///
    /// This is the ONLY per-frame CPU work - just updating a small uniform buffer.
    pub fn update_uniforms(
        &mut self,
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
        max_distance: f32,
    ) {
        self.uniforms = CullingUniforms::new(
            view_proj,
            camera_pos,
            self.chunk_data.len() as u32,
            max_distance,
        );
    }
    
    /// Returns chunk data for GPU upload (only when chunks change).
    #[must_use]
    pub fn chunk_data_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.chunk_data)
    }
    
    /// Returns uniforms for GPU upload (every frame, but tiny).
    #[must_use]
    pub fn uniforms_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(&self.uniforms)
    }
    
    /// Returns statistics (read back from GPU after frame).
    #[must_use]
    pub fn stats(&self) -> GPUDrivenStats {
        self.stats
    }
    
    /// Updates statistics from GPU readback.
    pub fn update_stats(&mut self, visible_chunks: u32) {
        self.stats = GPUDrivenStats {
            total_chunks: self.chunk_data.len() as u32,
            visible_chunks,
            draw_calls: 1, // Always 1 with MultiDrawIndirect!
        };
    }
    
    /// Returns the WGSL source for the GPU culling compute shader.
    #[must_use]
    pub fn culling_shader() -> &'static str {
        include_str!("../../shaders/gpu_culling.wgsl")
    }
}

impl Default for GPUDrivenRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gpu_data_size() {
        // Each chunk is 32 bytes on GPU
        assert_eq!(std::mem::size_of::<ChunkGPUData>(), 32);
        
        // 16K chunks = 512KB, easily fits in GPU
        let max_chunks = 16384;
        let total_size = max_chunks * std::mem::size_of::<ChunkGPUData>();
        assert_eq!(total_size, 524288); // 512KB
    }
    
    #[test]
    fn test_uniforms_size() {
        // Uniforms are small - updated every frame
        let size = std::mem::size_of::<CullingUniforms>();
        assert!(size < 512); // Under 512 bytes
    }
}
