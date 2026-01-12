//! Instanced renderer implementation.
//!
//! Orchestrates the entire instanced rendering pipeline.

use super::buffer::InstanceBuffer;
use super::instance_data::InstanceData;
use crate::voxel::{GreedyMesher, VoxelWorld, ChunkCoord};

/// Cached mesh data for a chunk.
struct ChunkMesh {
    /// Chunk coordinate.
    #[allow(dead_code)]
    coord: ChunkCoord,
    /// Instance data for this chunk's quads.
    instances: Vec<InstanceData>,
}

/// GPU Instanced renderer for voxel worlds.
///
/// This is the main rendering interface that Squad Neon exposes.
pub struct InstancedRenderer {
    /// Instance buffer for GPU upload.
    instance_buffer: InstanceBuffer,
    
    /// Greedy mesher for converting chunks to quads.
    mesher: GreedyMesher,
    
    /// Cached mesh data per chunk.
    chunk_meshes: Vec<ChunkMesh>,
    
    /// Statistics from last frame.
    stats: RenderStats,
}

/// Rendering statistics for performance monitoring.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderStats {
    /// Number of draw calls this frame.
    pub draw_calls: u32,
    /// Number of instances (quads) rendered.
    pub instance_count: u32,
    /// Number of chunks processed.
    pub chunks_rendered: u32,
    /// Number of chunks culled.
    pub chunks_culled: u32,
    /// Number of quads culled by occlusion.
    pub quads_culled: u32,
}

impl InstancedRenderer {
    /// Creates a new instanced renderer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            instance_buffer: InstanceBuffer::new(),
            mesher: GreedyMesher::new(),
            chunk_meshes: Vec::with_capacity(4096),
            stats: RenderStats::default(),
        }
    }
    
    /// Updates chunk meshes for dirty chunks.
    ///
    /// Call this once per frame before rendering.
    pub fn update_meshes(&mut self, world: &VoxelWorld) {
        let dirty_chunks = world.take_dirty_chunks();
        
        for coord in dirty_chunks {
            // Mesh the chunk and store instances
            // Need to separate meshing from instance conversion to avoid borrow issues
            let quads_data: Option<Vec<crate::voxel::MeshQuad>> = world.with_chunk(coord, |chunk| {
                self.mesher.mesh(chunk).to_vec()
            });
            
            if let Some(quads) = quads_data {
                let instances = self.quads_to_instances(&quads, coord);
                // Find or create mesh entry
                if let Some(mesh) = self.chunk_meshes.iter_mut().find(|m| m.coord == coord) {
                    mesh.instances = instances;
                } else {
                    self.chunk_meshes.push(ChunkMesh {
                        coord,
                        instances,
                    });
                }
            }
        }
    }
    
    /// Converts mesh quads to instance data.
    fn quads_to_instances(&self, quads: &[crate::voxel::MeshQuad], coord: ChunkCoord) -> Vec<InstanceData> {
        let chunk_offset_x = coord.x as f32 * 32.0;
        let chunk_offset_y = coord.y as f32 * 32.0;
        let chunk_offset_z = coord.z as f32 * 32.0;
        
        quads.iter().map(|quad| {
            InstanceData::from_quad(
                quad.x + chunk_offset_x,
                quad.y + chunk_offset_y,
                quad.z + chunk_offset_z,
                quad.width,
                quad.height,
                quad.normal,
                quad.material_id,
                quad.light_level,
            )
        }).collect()
    }
    
    /// Prepares render data for the frame.
    ///
    /// Returns the instance buffer data ready for GPU upload.
    pub fn prepare_frame(
        &mut self,
        camera_pos: [f32; 3],
        frustum_planes: &[[f32; 4]; 6],
    ) -> &[u8] {
        self.instance_buffer.begin_frame();
        self.stats = RenderStats::default();
        
        for mesh in &self.chunk_meshes {
            // Frustum culling at chunk level
            if !self.chunk_in_frustum(mesh.coord, frustum_planes) {
                self.stats.chunks_culled += 1;
                continue;
            }
            
            // Distance-based LOD (future: use different mesh detail)
            let _distance = self.chunk_distance(mesh.coord, camera_pos);
            
            // Add all instances from visible chunks
            let added = self.instance_buffer.push_batch(&mesh.instances);
            self.stats.instance_count += added as u32;
            self.stats.chunks_rendered += 1;
        }
        
        // Single draw call for all instances!
        self.stats.draw_calls = 1;
        
        self.instance_buffer.as_bytes()
    }
    
    /// Checks if a chunk is within the view frustum.
    fn chunk_in_frustum(&self, coord: ChunkCoord, planes: &[[f32; 4]; 6]) -> bool {
        // Chunk center in world space
        let center = [
            coord.x as f32 * 32.0 + 16.0,
            coord.y as f32 * 32.0 + 16.0,
            coord.z as f32 * 32.0 + 16.0,
        ];
        
        // Chunk radius (half diagonal of 32x32x32 cube)
        let radius = 27.7; // sqrt(16^2 + 16^2 + 16^2)
        
        // Check against all 6 frustum planes
        for plane in planes {
            let distance = plane[0] * center[0]
                + plane[1] * center[1]
                + plane[2] * center[2]
                + plane[3];
            
            if distance < -radius {
                return false;
            }
        }
        
        true
    }
    
    /// Calculates distance from camera to chunk center.
    fn chunk_distance(&self, coord: ChunkCoord, camera_pos: [f32; 3]) -> f32 {
        let center = [
            coord.x as f32 * 32.0 + 16.0,
            coord.y as f32 * 32.0 + 16.0,
            coord.z as f32 * 32.0 + 16.0,
        ];
        
        let dx = center[0] - camera_pos[0];
        let dy = center[1] - camera_pos[1];
        let dz = center[2] - camera_pos[2];
        
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
    
    /// Returns the render statistics from the last frame.
    #[must_use]
    pub const fn stats(&self) -> RenderStats {
        self.stats
    }
    
    /// Returns the instance count for the current frame.
    #[must_use]
    pub fn instance_count(&self) -> usize {
        self.instance_buffer.instance_count()
    }
    
    /// Removes mesh data for unloaded chunks.
    pub fn unload_chunk(&mut self, coord: ChunkCoord) {
        self.chunk_meshes.retain(|m| m.coord != coord);
    }
}

impl Default for InstancedRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::Voxel;
    
    #[test]
    fn test_renderer_creation() {
        let renderer = InstancedRenderer::new();
        assert_eq!(renderer.instance_count(), 0);
    }
    
    #[test]
    fn test_mesh_update() {
        let mut renderer = InstancedRenderer::new();
        let world = VoxelWorld::new();
        
        // Load and populate a chunk
        world.load_chunk(ChunkCoord::new(0, 0, 0));
        world.set_voxel(5, 5, 5, Voxel::new(1));
        
        // Update meshes
        renderer.update_meshes(&world);
        
        // Prepare frame (no frustum culling for test)
        let identity_frustum = [[1.0, 0.0, 0.0, 1000.0]; 6];
        let _data = renderer.prepare_frame([0.0, 0.0, 0.0], &identity_frustum);
        
        // Should have some instances
        assert!(renderer.stats().instance_count > 0);
        // Single draw call
        assert_eq!(renderer.stats().draw_calls, 1);
    }
}
