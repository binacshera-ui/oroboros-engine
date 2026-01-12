//! Greedy Meshing algorithm for voxel optimization.
//!
//! Reduces polygon count by merging adjacent faces with the same material.
//! This is critical for achieving <1000 draw calls with millions of voxels.
//!
//! ## Algorithm
//!
//! 1. For each axis (X, Y, Z) and direction (+/-):
//! 2. Sweep through slices perpendicular to that axis
//! 3. Build a 2D mask of visible faces
//! 4. Greedily merge adjacent faces with same material
//! 5. Output merged quads for GPU instancing

use bytemuck::{Pod, Zeroable};
use super::chunk::{VoxelChunk, Voxel, CHUNK_SIZE};
use std::time::Instant;

// =============================================================================
// OPERATION PANOPTICON - MESH DIAGNOSTICS
// =============================================================================
/// Enable verbose mesh logging
const PANOPTICON_MESH_VERBOSE: bool = true;

/// A merged quad produced by greedy meshing.
///
/// This is the output format that feeds into GPU instancing.
/// Each quad represents multiple merged voxel faces.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct MeshQuad {
    /// Position of the quad's origin (bottom-left corner).
    pub x: f32,
    /// Position Y.
    pub y: f32,
    /// Position Z.
    pub z: f32,
    /// Width of the quad (number of merged voxels in U direction).
    pub width: f32,
    /// Height of the quad (number of merged voxels in V direction).
    pub height: f32,
    /// Material ID for texturing.
    pub material_id: u32,
    /// Face normal encoded as 0-5 (±X, ±Y, ±Z).
    pub normal: u32,
    /// Light level for this face.
    pub light_level: u32,
    /// Ambient Occlusion values for each corner (0-3 occlusion level).
    /// Order: [bottom-left, bottom-right, top-right, top-left]
    /// 0 = fully lit, 3 = fully occluded (corner in dark pocket)
    pub ao_corners: [u8; 4],
}

impl MeshQuad {
    /// Normal index for +X face.
    pub const NORMAL_POS_X: u32 = 0;
    /// Normal index for -X face.
    pub const NORMAL_NEG_X: u32 = 1;
    /// Normal index for +Y face.
    pub const NORMAL_POS_Y: u32 = 2;
    /// Normal index for -Y face.
    pub const NORMAL_NEG_Y: u32 = 3;
    /// Normal index for +Z face.
    pub const NORMAL_POS_Z: u32 = 4;
    /// Normal index for -Z face.
    pub const NORMAL_NEG_Z: u32 = 5;
}

/// Face mask entry for greedy meshing.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct FaceMask {
    material: u8,
    light: u8,
    /// AO values for the 4 corners of this single voxel face.
    /// Order: [bottom-left, bottom-right, top-right, top-left]
    ao: [u8; 4],
}

impl FaceMask {
    const EMPTY: Self = Self { material: 0, light: 0, ao: [0; 4] };
    
    fn from_voxel(voxel: Voxel, ao: [u8; 4]) -> Self {
        Self {
            material: voxel.material_id(),
            light: voxel.light_level(),
            ao,
        }
    }
    
    fn is_empty(self) -> bool {
        self.material == 0
    }
    
    /// Check if two masks can be merged (same material, light, AND matching AO on shared edge).
    #[allow(dead_code)]
    fn can_merge_horizontal(self, other: Self) -> bool {
        self.material == other.material 
            && self.light == other.light
            // For horizontal merge, right edge of self must match left edge of other
            && self.ao[1] == other.ao[0]  // bottom-right == bottom-left
            && self.ao[2] == other.ao[3]  // top-right == top-left
    }
    
    #[allow(dead_code)]
    fn can_merge_vertical(self, other: Self) -> bool {
        self.material == other.material 
            && self.light == other.light
            // For vertical merge, top edge of self must match bottom edge of other  
            && self.ao[2] == other.ao[1]  // top-right == bottom-right
            && self.ao[3] == other.ao[0]  // top-left == bottom-left
    }
}

/// Greedy meshing engine.
///
/// Pre-allocates all working memory to avoid allocations during meshing.
pub struct GreedyMesher {
    /// Working mask for face detection.
    mask: Box<[[FaceMask; CHUNK_SIZE]; CHUNK_SIZE]>,
    /// Output buffer for quads - pre-allocated for worst case.
    /// Worst case: every other voxel is solid = CHUNK_VOLUME / 2 * 6 faces
    output: Vec<MeshQuad>,
    /// Cached chunk reference for AO lookups (used during mesh operation).
    /// This is a workaround to avoid passing chunk to every function.
    #[allow(dead_code)]
    cached_chunk: Option<*const VoxelChunk>,
}

impl GreedyMesher {
    /// Maximum quads per chunk (theoretical worst case).
    const MAX_QUADS: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE * 3;
    
    /// Creates a new greedy mesher with pre-allocated buffers.
    ///
    /// Note: Call this once during initialization, not in hot path.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mask: Box::new([[FaceMask::EMPTY; CHUNK_SIZE]; CHUNK_SIZE]),
            output: Vec::with_capacity(Self::MAX_QUADS),
            cached_chunk: None,
        }
    }
    
    /// Meshes a chunk, returning the merged quads.
    ///
    /// This is the main entry point. Call this when a chunk becomes dirty.
    /// The returned slice is valid until the next call to `mesh`.
    pub fn mesh(&mut self, chunk: &VoxelChunk) -> &[MeshQuad] {
        let mesh_start = Instant::now();
        let coord = chunk.coord();
        
        // PANOPTICON: Log mesh start
        if PANOPTICON_MESH_VERBOSE {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            println!("[{:>12}] [MESH START] Building mesh for [{},{},{}]",
                timestamp, coord.x, coord.y, coord.z);
        }
        
        self.output.clear();
        
        // Skip empty chunks entirely
        if chunk.is_empty() {
            if PANOPTICON_MESH_VERBOSE {
                println!("[MESH] Chunk [{},{},{}] is empty - skipping", coord.x, coord.y, coord.z);
            }
            return &self.output;
        }
        
        // Process each axis
        self.mesh_axis::<0>(chunk); // X axis
        self.mesh_axis::<1>(chunk); // Y axis
        self.mesh_axis::<2>(chunk); // Z axis
        
        // PANOPTICON: Log mesh completion with timing
        if PANOPTICON_MESH_VERBOSE {
            let mesh_time = mesh_start.elapsed();
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            println!("[{:>12}] [MESH END] Built [{},{},{}] in {}ms. Faces generated: {}",
                timestamp, coord.x, coord.y, coord.z, mesh_time.as_millis(), self.output.len());
                
            if mesh_time.as_millis() > 10 {
                println!("[{:>12}] [MESH WARN] ⚠️ Slow mesh generation: {}ms for chunk [{},{},{}]",
                    timestamp, mesh_time.as_millis(), coord.x, coord.y, coord.z);
            }
        }
        
        &self.output
    }
    
    /// Meshes faces perpendicular to the given axis.
    fn mesh_axis<const AXIS: usize>(&mut self, chunk: &VoxelChunk) {
        // Axis indices for the perpendicular plane
        let (u_axis, v_axis) = match AXIS {
            0 => (1, 2), // X: sweep YZ planes
            1 => (0, 2), // Y: sweep XZ planes
            2 => (0, 1), // Z: sweep XY planes
            _ => unreachable!(),
        };
        
        // Sweep through slices
        for d in 0..CHUNK_SIZE {
            // Build mask for positive direction
            self.build_mask::<true>(chunk, AXIS, d, u_axis, v_axis);
            self.greedy_extract(d as f32, AXIS, true);
            
            // Build mask for negative direction
            self.build_mask::<false>(chunk, AXIS, d, u_axis, v_axis);
            self.greedy_extract(d as f32, AXIS, false);
        }
    }
    
    /// Builds a 2D mask of visible faces at a slice.
    fn build_mask<const POSITIVE: bool>(
        &mut self,
        chunk: &VoxelChunk,
        axis: usize,
        d: usize,
        u_axis: usize,
        v_axis: usize,
    ) {
        // Clear mask
        for row in self.mask.iter_mut() {
            row.fill(FaceMask::EMPTY);
        }
        
        for v in 0..CHUNK_SIZE {
            for u in 0..CHUNK_SIZE {
                // Build coordinates
                let mut pos = [0usize; 3];
                pos[axis] = d;
                pos[u_axis] = u;
                pos[v_axis] = v;
                
                let voxel = chunk.get(pos[0], pos[1], pos[2]);
                
                if voxel.is_air() {
                    continue;
                }
                
                // Check neighbor in the direction we're facing
                let neighbor_d = if POSITIVE { d + 1 } else { d.wrapping_sub(1) };
                
                let neighbor_is_solid = if neighbor_d < CHUNK_SIZE {
                    let mut neighbor_pos = pos;
                    neighbor_pos[axis] = neighbor_d;
                    chunk.get(neighbor_pos[0], neighbor_pos[1], neighbor_pos[2]).is_solid()
                } else {
                    false // Chunk boundary - assume air for now
                };
                
                // Face is visible if neighbor is air
                if !neighbor_is_solid {
                    // Default AO (no occlusion) - can be enhanced later
                    let ao = [0u8; 4];
                    self.mask[v][u] = FaceMask::from_voxel(voxel, ao);
                }
            }
        }
    }
    
    /// Greedily extracts quads from the mask.
    fn greedy_extract(&mut self, d: f32, axis: usize, positive: bool) {
        let normal = match (axis, positive) {
            (0, true) => MeshQuad::NORMAL_POS_X,
            (0, false) => MeshQuad::NORMAL_NEG_X,
            (1, true) => MeshQuad::NORMAL_POS_Y,
            (1, false) => MeshQuad::NORMAL_NEG_Y,
            (2, true) => MeshQuad::NORMAL_POS_Z,
            (2, false) => MeshQuad::NORMAL_NEG_Z,
            _ => unreachable!(),
        };
        
        for v in 0..CHUNK_SIZE {
            let mut u = 0;
            while u < CHUNK_SIZE {
                let face = self.mask[v][u];
                
                if face.is_empty() {
                    u += 1;
                    continue;
                }
                
                // Find width - extend as far as possible with same material
                let mut width = 1;
                while u + width < CHUNK_SIZE && self.mask[v][u + width] == face {
                    width += 1;
                }
                
                // Find height - extend rows with matching run
                let mut height = 1;
                'height: while v + height < CHUNK_SIZE {
                    for du in 0..width {
                        if self.mask[v + height][u + du] != face {
                            break 'height;
                        }
                    }
                    height += 1;
                }
                
                // Create quad
                let (x, y, z) = self.compute_position(d, u, v, axis, positive);
                
                self.output.push(MeshQuad {
                    x,
                    y,
                    z,
                    width: width as f32,
                    height: height as f32,
                    material_id: face.material as u32,
                    normal,
                    light_level: face.light as u32,
                    ao_corners: face.ao,
                });
                
                // Clear used cells from mask
                for dv in 0..height {
                    for du in 0..width {
                        self.mask[v + dv][u + du] = FaceMask::EMPTY;
                    }
                }
                
                u += width;
            }
        }
    }
    
    /// Computes 3D position from slice coordinates.
    fn compute_position(
        &self,
        d: f32,
        u: usize,
        v: usize,
        axis: usize,
        positive: bool,
    ) -> (f32, f32, f32) {
        let d_offset = if positive { d + 1.0 } else { d };
        
        match axis {
            0 => (d_offset, u as f32, v as f32),
            1 => (u as f32, d_offset, v as f32),
            2 => (u as f32, v as f32, d_offset),
            _ => unreachable!(),
        }
    }
    
    /// Returns the number of quads from the last mesh operation.
    #[must_use]
    pub fn quad_count(&self) -> usize {
        self.output.len()
    }
}

impl Default for GreedyMesher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::chunk::ChunkCoord;
    
    #[test]
    fn test_empty_chunk() {
        let mut mesher = GreedyMesher::new();
        let chunk = VoxelChunk::new(ChunkCoord::new(0, 0, 0));
        
        let quads = mesher.mesh(&chunk);
        assert!(quads.is_empty());
    }
    
    #[test]
    fn test_single_voxel() {
        let mut mesher = GreedyMesher::new();
        let mut chunk = VoxelChunk::new(ChunkCoord::new(0, 0, 0));
        chunk.set(0, 0, 0, Voxel::new(1));
        
        let quads = mesher.mesh(&chunk);
        // Single voxel should produce 6 faces (one per direction)
        // But some may be culled at chunk boundaries
        assert!(!quads.is_empty());
    }
    
    #[test]
    fn test_greedy_merging() {
        let mut mesher = GreedyMesher::new();
        let mut chunk = VoxelChunk::new(ChunkCoord::new(0, 0, 0));
        
        // Create a 2x2x1 block - should merge into fewer quads
        for x in 0..2 {
            for y in 0..2 {
                chunk.set(x, y, 0, Voxel::new(1));
            }
        }
        
        let quads = mesher.mesh(&chunk);
        
        // Top and bottom faces should each be 1 quad (2x2 merged)
        // Side faces should be 2 quads each (2x1 merged)
        // Total should be much less than 4 voxels * 6 faces = 24
        assert!(quads.len() < 24);
    }
}
