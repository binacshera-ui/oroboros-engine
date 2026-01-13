//! Standard Mesher - Industry Standard Greedy Meshing using block-mesh-rs
//!
//! OPERATION INDUSTRIAL STANDARD + COURSE CORRECTION
//! 
//! KEY CHANGES:
//! - Uses Vertex Buffer + Index Buffer (NOT instances)
//! - Proper UV tiling for greedy quads (textures repeat, not stretch)
//! - Implements required Voxel + MergeVoxel traits correctly

use block_mesh::{
    greedy_quads, 
    GreedyQuadsBuffer, 
    MergeVoxel, 
    Voxel as BlockMeshVoxel,
    VoxelVisibility,
    RIGHT_HANDED_Y_UP_CONFIG,
};
use ndshape::{ConstShape, ConstShape3u32};
use bytemuck::{Pod, Zeroable};

/// Padded chunk size (32 + 2 for neighbor data on each edge)
pub const PADDED_CHUNK_SIZE: u32 = 34;

/// Shape for padded chunk array (34x34x34)
pub type PaddedChunkShape = ConstShape3u32<PADDED_CHUNK_SIZE, PADDED_CHUNK_SIZE, PADDED_CHUNK_SIZE>;

/// Total voxels in padded array
pub const PADDED_CHUNK_VOLUME: usize = (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize;

// =============================================================================
// VERTEX FORMAT - Standard vertex buffer layout
// =============================================================================

/// Vertex for terrain mesh - packed for GPU efficiency
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TerrainVertex {
    /// Position in world space [x, y, z]
    pub position: [f32; 3],
    /// Normal direction [nx, ny, nz]
    pub normal: [f32; 3],
    /// UV coordinates for texture tiling [u, v]
    pub uv: [f32; 2],
    /// Material ID and AO packed: [material_id, ao, 0, 0]
    pub material_ao: [f32; 4],
}

impl TerrainVertex {
    /// Vertex buffer layout for WGPU
    pub const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3,  // position
        1 => Float32x3,  // normal
        2 => Float32x2,  // uv
        3 => Float32x4,  // material_ao
    ];
    
    /// Vertex buffer layout descriptor
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TerrainVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

// =============================================================================
// MESH VOXEL - Implements required block-mesh traits
// =============================================================================

/// Simple voxel type for block-mesh compatibility
/// IMPORTANT: merge_value() uses material_id so only same-material blocks merge!
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct MeshVoxel {
    /// Material ID (0 = air, 1+ = solid materials)
    pub material_id: u8,
}

impl MeshVoxel {
    /// Create a new voxel with given material
    #[inline]
    pub const fn new(material_id: u8) -> Self {
        Self { material_id }
    }
    
    /// Air voxel (empty)
    pub const AIR: Self = Self { material_id: 0 };
    
    /// Check if voxel is air
    #[inline]
    pub const fn is_air(&self) -> bool {
        self.material_id == 0
    }
    
    /// Check if voxel is solid
    #[inline]
    pub const fn is_solid(&self) -> bool {
        self.material_id != 0
    }
}

// REQUIRED TRAIT: Voxel - defines visibility
impl BlockMeshVoxel for MeshVoxel {
    fn get_visibility(&self) -> VoxelVisibility {
        if self.is_air() {
            VoxelVisibility::Empty
        } else {
            VoxelVisibility::Opaque
        }
    }
}

// REQUIRED TRAIT: MergeVoxel - defines which voxels can be merged
// CRITICAL: Returns material_id so only same-texture blocks merge!
impl MergeVoxel for MeshVoxel {
    type MergeValue = u8;
    
    fn merge_value(&self) -> Self::MergeValue {
        self.material_id  // Only merge blocks with same material!
    }
}

// =============================================================================
// MESH OUTPUT - Final mesh ready for GPU
// =============================================================================

/// Complete mesh data for a chunk (vertices + indices)
#[derive(Default)]
pub struct ChunkMesh {
    /// Vertex buffer data
    pub vertices: Vec<TerrainVertex>,
    /// Index buffer data (u32 for large meshes)
    pub indices: Vec<u32>,
}

impl ChunkMesh {
    /// Check if mesh is empty
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
    
    /// Get triangle count
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
    
    /// Get vertex count
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }
}

// =============================================================================
// PADDED CHUNK BUFFER - 34x34x34 with neighbor data
// =============================================================================

/// Padded voxel buffer for a single chunk + its neighbors
pub struct PaddedChunkBuffer {
    /// Voxel data with padding (34x34x34)
    pub voxels: [MeshVoxel; PADDED_CHUNK_VOLUME],
}

impl Default for PaddedChunkBuffer {
    fn default() -> Self {
        Self {
            voxels: [MeshVoxel::AIR; PADDED_CHUNK_VOLUME],
        }
    }
}

impl PaddedChunkBuffer {
    /// Create a new empty buffer
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Get index in padded buffer
    #[inline]
    pub fn index(x: u32, y: u32, z: u32) -> usize {
        PaddedChunkShape::linearize([x, y, z]) as usize
    }
    
    /// Set voxel at padded coordinates (0-33 range)
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, z: u32, voxel: MeshVoxel) {
        let idx = Self::index(x, y, z);
        if idx < PADDED_CHUNK_VOLUME {
            self.voxels[idx] = voxel;
        }
    }
    
    /// Get voxel at padded coordinates
    #[inline]
    pub fn get(&self, x: u32, y: u32, z: u32) -> MeshVoxel {
        let idx = Self::index(x, y, z);
        if idx < PADDED_CHUNK_VOLUME {
            self.voxels[idx]
        } else {
            MeshVoxel::AIR
        }
    }
}

// =============================================================================
// STANDARD MESHER - Main meshing implementation
// =============================================================================

/// Standard Mesher using block-mesh library
/// COURSE CORRECTION: Outputs Vertex + Index buffers, not instances
pub struct StandardMesher {
    /// Reusable buffer for greedy quads algorithm
    buffer: GreedyQuadsBuffer,
}

impl Default for StandardMesher {
    fn default() -> Self {
        Self::new()
    }
}

impl StandardMesher {
    /// Create a new mesher with pre-allocated buffer
    pub fn new() -> Self {
        Self {
            buffer: GreedyQuadsBuffer::new(PADDED_CHUNK_VOLUME),
        }
    }
    
    /// Generate mesh from a padded chunk buffer
    ///
    /// # Arguments
    /// * `chunk` - Padded voxel buffer (34x34x34 with neighbor data)
    /// * `chunk_world_offset` - World position offset for this chunk
    ///
    /// # Returns
    /// ChunkMesh with vertices and indices ready for GPU
    ///
    /// # DATA FEED FIX
    /// The padded buffer is 34x34x34:
    /// - Index 0 and 33 = neighbor data (for visibility checks)
    /// - Index 1..33 = actual chunk data (32x32x32)
    /// We process the ENTIRE buffer so the algorithm can see neighbors!
    pub fn generate_mesh(
        &mut self,
        chunk: &PaddedChunkBuffer,
        chunk_world_offset: [i32; 3],
    ) -> ChunkMesh {
        // Clear previous buffer data
        self.buffer.reset(PADDED_CHUNK_VOLUME);
        
        // Run greedy quads algorithm
        // CRITICAL: Process the FULL padded buffer [0..34)
        // The algorithm uses indices 0 and 33 to check neighbor visibility
        // but only generates quads for solid voxels (which are in 1..33)
        greedy_quads(
            &chunk.voxels,
            &PaddedChunkShape {},
            [0; 3],                           // Minimum corner (inclusive)
            [PADDED_CHUNK_SIZE - 1; 3],       // Maximum corner (inclusive) = 33
            &RIGHT_HANDED_Y_UP_CONFIG.faces,
            &mut self.buffer,
        );
        
        let mut mesh = ChunkMesh::default();
        
        // Process each face group (6 directions)
        for (group_idx, group) in self.buffer.quads.groups.iter().enumerate() {
            let face = &RIGHT_HANDED_Y_UP_CONFIG.faces[group_idx];
            
            for quad in group.iter() {
                // Get the voxel at quad position for material
                let voxel_pos = quad.minimum;
                let voxel_idx = PaddedChunkShape::linearize([voxel_pos[0], voxel_pos[1], voxel_pos[2]]) as usize;
                let material_id = chunk.voxels.get(voxel_idx)
                    .map(|v| v.material_id)
                    .unwrap_or(1);
                
                // Calculate world position (subtract 1 for padding offset)
                let world_pos = [
                    (voxel_pos[0] as i32 - 1) + chunk_world_offset[0],
                    (voxel_pos[1] as i32 - 1) + chunk_world_offset[1],
                    (voxel_pos[2] as i32 - 1) + chunk_world_offset[2],
                ];
                
                // Get normal from face
                let normal = face.signed_normal();
                let normal_f32 = [normal.x as f32, normal.y as f32, normal.z as f32];
                
                // Generate 4 vertices for this quad using library helper
                let positions = face.quad_mesh_positions(&quad, 1.0);
                
                // CRITICAL: Generate UVs that TILE, not stretch!
                // UV coordinates should repeat based on quad size
                let quad_width = quad.width as f32;
                let quad_height = quad.height as f32;
                
                // UVs that tile: (0,0) to (width, height)
                let uvs = [
                    [0.0, 0.0],
                    [quad_width, 0.0],
                    [0.0, quad_height],
                    [quad_width, quad_height],
                ];
                
                // Calculate AO (simplified for now)
                let ao = 1.0;
                
                // Add 4 vertices
                let base_vertex = mesh.vertices.len() as u32;
                for (i, pos) in positions.iter().enumerate() {
                    mesh.vertices.push(TerrainVertex {
                        position: [
                            pos[0] + world_pos[0] as f32,
                            pos[1] + world_pos[1] as f32,
                            pos[2] + world_pos[2] as f32,
                        ],
                        normal: normal_f32,
                        uv: uvs[i],
                        material_ao: [material_id as f32, ao, 0.0, 0.0],
                    });
                }
                
                // Add 6 indices for 2 triangles (using library order)
                let indices = face.quad_mesh_indices(base_vertex);
                mesh.indices.extend_from_slice(&indices);
            }
        }
        
        mesh
    }
    
    /// Get the number of quads generated in the last mesh operation
    pub fn last_quad_count(&self) -> usize {
        self.buffer.quads.num_quads()
    }
}

// =============================================================================
// LEGACY COMPATIBILITY - Keep old types for gradual migration
// =============================================================================

/// Output quad from the meshing process (legacy compatibility)
#[derive(Debug, Clone, Copy)]
pub struct MeshQuad {
    /// World position (x, y, z)
    pub position: [f32; 3],
    /// Quad dimensions (width, height)
    pub size: [f32; 2],
    /// Normal direction index (0-5: +X, -X, +Y, -Y, +Z, -Z)
    pub normal_index: u32,
    /// Material ID for texturing
    pub material_id: u8,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_padded_buffer_indexing() {
        let mut buffer = PaddedChunkBuffer::new();
        
        // Test setting and getting with padding offset
        buffer.set(1, 1, 1, MeshVoxel::new(5));
        assert_eq!(buffer.get(1, 1, 1).material_id, 5);
        
        buffer.set(32, 32, 32, MeshVoxel::new(10));
        assert_eq!(buffer.get(32, 32, 32).material_id, 10);
    }
    
    #[test]
    fn test_mesh_generation() {
        let mut buffer = PaddedChunkBuffer::new();
        let mut mesher = StandardMesher::new();
        
        // Create a single solid voxel (at padded coord 5,5,5 = local 4,4,4)
        buffer.set(5, 5, 5, MeshVoxel::new(1));
        
        let mesh = mesher.generate_mesh(&buffer, [0, 0, 0]);
        
        // Single voxel should produce 6 quads = 6*4 vertices, 6*6 indices
        assert_eq!(mesh.vertex_count(), 24);
        assert_eq!(mesh.indices.len(), 36);
        assert_eq!(mesh.triangle_count(), 12);
    }
    
    #[test]
    fn test_uv_tiling() {
        let mut buffer = PaddedChunkBuffer::new();
        let mut mesher = StandardMesher::new();
        
        // Create a 2x2 slab of same material (should merge)
        for x in 5..7 {
            for z in 5..7 {
                buffer.set(x, 5, z, MeshVoxel::new(1));
            }
        }
        
        let mesh = mesher.generate_mesh(&buffer, [0, 0, 0]);
        
        // Check that merged quad has UVs that tile (not 0-1)
        // Top face should have UVs up to 2.0 for a 2x2 quad
        let mut found_tiling_uv = false;
        for vertex in &mesh.vertices {
            if vertex.uv[0] > 1.0 || vertex.uv[1] > 1.0 {
                found_tiling_uv = true;
                break;
            }
        }
        assert!(found_tiling_uv, "Merged quads should have tiling UVs > 1.0");
    }
    
    #[test]
    fn test_material_separation() {
        let mut buffer = PaddedChunkBuffer::new();
        let mut mesher = StandardMesher::new();
        
        // Create 2 adjacent voxels with DIFFERENT materials
        buffer.set(5, 5, 5, MeshVoxel::new(1)); // Grass
        buffer.set(6, 5, 5, MeshVoxel::new(2)); // Dirt
        
        let mesh = mesher.generate_mesh(&buffer, [0, 0, 0]);
        
        // Different materials should NOT merge - expect 2 separate quads per shared face
        // Total: 12 quads - shared internal faces = ~10+ quads
        assert!(mesh.vertex_count() >= 40, "Different materials should not merge");
    }
}
