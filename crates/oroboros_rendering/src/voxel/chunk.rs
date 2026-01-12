//! Voxel chunk data structures.
//!
//! Chunks are 32x32x32 voxels - optimized for cache efficiency and GPU upload.

use bytemuck::{Pod, Zeroable};

/// Chunk dimension - 32 voxels per axis.
/// This gives us 32,768 voxels per chunk which fits nicely in GPU memory.
pub const CHUNK_SIZE: usize = 32;

/// Total voxels per chunk.
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

/// A single voxel - packed into 4 bytes for GPU efficiency.
///
/// Layout:
/// - Bits 0-7: Material ID (256 materials)
/// - Bits 8-15: Light level (0-255)
/// - Bits 16-23: Emission R (for neon signs)
/// - Bits 24-31: Emission GB packed (4 bits each)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable, PartialEq, Eq)]
pub struct Voxel {
    /// Packed voxel data.
    pub data: u32,
}

impl Voxel {
    /// Air voxel - completely empty.
    pub const AIR: Self = Self { data: 0 };
    
    /// Creates a new voxel with the given material.
    #[inline]
    #[must_use]
    pub const fn new(material_id: u8) -> Self {
        Self { data: material_id as u32 }
    }
    
    /// Creates a neon voxel with emission color.
    #[inline]
    #[must_use]
    pub const fn neon(material_id: u8, r: u8, g: u8, b: u8) -> Self {
        let emission_r = r as u32;
        let emission_gb = (((g >> 4) << 4) | (b >> 4)) as u32;
        Self {
            data: material_id as u32 
                | (255 << 8)  // Max light level for neon
                | (emission_r << 16)
                | (emission_gb << 24),
        }
    }
    
    /// Returns the material ID.
    #[inline]
    #[must_use]
    pub const fn material_id(self) -> u8 {
        (self.data & 0xFF) as u8
    }
    
    /// Returns true if this voxel is air (empty).
    #[inline]
    #[must_use]
    pub const fn is_air(self) -> bool {
        self.material_id() == 0
    }
    
    /// Returns true if this voxel is solid (not air).
    #[inline]
    #[must_use]
    pub const fn is_solid(self) -> bool {
        self.material_id() != 0
    }
    
    /// Returns the light level (0-255).
    #[inline]
    #[must_use]
    pub const fn light_level(self) -> u8 {
        ((self.data >> 8) & 0xFF) as u8
    }
    
    /// Returns true if this voxel emits light.
    #[inline]
    #[must_use]
    pub const fn is_emissive(self) -> bool {
        self.light_level() > 0
    }
}

/// Chunk coordinate in world space.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
    /// Z coordinate.
    pub z: i32,
    /// Padding for alignment.
    pub _pad: i32,
}

impl ChunkCoord {
    /// Creates a new chunk coordinate.
    #[inline]
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z, _pad: 0 }
    }
    
    /// Converts world position to chunk coordinate.
    #[inline]
    #[must_use]
    pub const fn from_world_pos(x: i32, y: i32, z: i32) -> Self {
        Self::new(
            x.div_euclid(CHUNK_SIZE as i32),
            y.div_euclid(CHUNK_SIZE as i32),
            z.div_euclid(CHUNK_SIZE as i32),
        )
    }
}

/// A chunk of voxels - 32x32x32 = 32,768 voxels.
///
/// Memory layout is optimized for cache-friendly iteration and GPU upload.
/// Voxels are stored in Z-Y-X order for optimal memory access patterns.
pub struct VoxelChunk {
    /// The coordinate of this chunk in world space.
    coord: ChunkCoord,
    
    /// Voxel data - stored as contiguous array for GPU upload.
    /// Layout: voxels[z * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + x]
    voxels: Box<[Voxel; CHUNK_VOLUME]>,
    
    /// Dirty flag - set when chunk needs re-meshing.
    dirty: bool,
    
    /// Number of solid voxels (for quick empty/full checks).
    solid_count: u32,
    
    /// Number of emissive voxels (for lighting optimization).
    emissive_count: u32,
}

impl VoxelChunk {
    /// Creates a new empty chunk at the given coordinate.
    ///
    /// Note: This allocates memory. Only call during loading, never in hot path.
    #[must_use]
    pub fn new(coord: ChunkCoord) -> Self {
        Self {
            coord,
            voxels: Box::new([Voxel::AIR; CHUNK_VOLUME]),
            dirty: true,
            solid_count: 0,
            emissive_count: 0,
        }
    }
    
    /// Returns the chunk coordinate.
    #[inline]
    #[must_use]
    pub const fn coord(&self) -> ChunkCoord {
        self.coord
    }
    
    /// Returns true if the chunk needs re-meshing.
    #[inline]
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }
    
    /// Clears the dirty flag.
    #[inline]
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
    
    /// Returns true if the chunk is completely empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.solid_count == 0
    }
    
    /// Returns true if the chunk is completely solid.
    #[inline]
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.solid_count == CHUNK_VOLUME as u32
    }
    
    /// Returns the number of solid voxels.
    #[inline]
    #[must_use]
    pub const fn solid_count(&self) -> u32 {
        self.solid_count
    }
    
    /// Returns true if the chunk contains emissive voxels.
    #[inline]
    #[must_use]
    pub const fn has_emissive(&self) -> bool {
        self.emissive_count > 0
    }
    
    /// Calculates the linear index for a voxel position.
    #[inline]
    const fn index(x: usize, y: usize, z: usize) -> usize {
        debug_assert!(x < CHUNK_SIZE);
        debug_assert!(y < CHUNK_SIZE);
        debug_assert!(z < CHUNK_SIZE);
        z * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + x
    }
    
    /// Gets a voxel at the given local position.
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds (debug builds only).
    #[inline]
    #[must_use]
    pub fn get(&self, x: usize, y: usize, z: usize) -> Voxel {
        self.voxels[Self::index(x, y, z)]
    }
    
    /// Gets a voxel at the given local position, or None if out of bounds.
    #[inline]
    #[must_use]
    pub fn try_get(&self, x: usize, y: usize, z: usize) -> Option<Voxel> {
        if x < CHUNK_SIZE && y < CHUNK_SIZE && z < CHUNK_SIZE {
            Some(self.voxels[Self::index(x, y, z)])
        } else {
            None
        }
    }
    
    /// Sets a voxel at the given local position.
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds (debug builds only).
    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, voxel: Voxel) {
        let idx = Self::index(x, y, z);
        let old = self.voxels[idx];
        
        // Update solid count
        if old.is_solid() && voxel.is_air() {
            self.solid_count -= 1;
        } else if old.is_air() && voxel.is_solid() {
            self.solid_count += 1;
        }
        
        // Update emissive count
        if old.is_emissive() && !voxel.is_emissive() {
            self.emissive_count -= 1;
        } else if !old.is_emissive() && voxel.is_emissive() {
            self.emissive_count += 1;
        }
        
        self.voxels[idx] = voxel;
        self.dirty = true;
    }
    
    /// Returns a raw pointer to the voxel data for GPU upload.
    ///
    /// # Safety
    /// The returned pointer is valid for the lifetime of this chunk.
    #[inline]
    #[must_use]
    pub fn as_ptr(&self) -> *const Voxel {
        self.voxels.as_ptr()
    }
    
    /// Returns the voxel data as a byte slice for GPU upload.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&*self.voxels)
    }
    
    /// Creates a clone from a reference (used by Clone impl).
    pub(crate) fn clone_from_ref(other: &Self) -> Self {
        Self {
            coord: other.coord,
            voxels: other.voxels.clone(),
            dirty: other.dirty,
            solid_count: other.solid_count,
            emissive_count: other.emissive_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_voxel_packing() {
        let voxel = Voxel::new(42);
        assert_eq!(voxel.material_id(), 42);
        assert!(!voxel.is_air());
        assert!(voxel.is_solid());
    }
    
    #[test]
    fn test_neon_voxel() {
        let neon = Voxel::neon(1, 255, 0, 255);
        assert_eq!(neon.material_id(), 1);
        assert!(neon.is_emissive());
        assert_eq!(neon.light_level(), 255);
    }
    
    #[test]
    fn test_chunk_operations() {
        let mut chunk = VoxelChunk::new(ChunkCoord::new(0, 0, 0));
        assert!(chunk.is_empty());
        
        chunk.set(0, 0, 0, Voxel::new(1));
        assert!(!chunk.is_empty());
        assert_eq!(chunk.solid_count(), 1);
        assert!(chunk.is_dirty());
        
        chunk.set(0, 0, 0, Voxel::AIR);
        assert!(chunk.is_empty());
    }
}
