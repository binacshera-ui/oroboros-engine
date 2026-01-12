//! Paletted voxel compression.
//!
//! ARCHITECT'S FEEDBACK: 32-bit per voxel is VRAM bloat.
//! Solution: 8-bit palette index + global material lookup.
//!
//! Memory savings:
//! - Before: 32 bits/voxel = 131KB per chunk
//! - After: 8 bits/voxel = 32KB per chunk
//! - Savings: 75% VRAM reduction
//!
//! With 32-chunk render distance (32^3 chunks visible):
//! - Before: 32,768 chunks × 131KB = 4.3 GB VRAM
//! - After: 32,768 chunks × 32KB = 1.1 GB VRAM

use bytemuck::{Pod, Zeroable};

/// Maximum materials in the global palette.
/// 256 is enough for any realistic voxel game.
pub const MAX_MATERIALS: usize = 256;

/// Compressed voxel - only 8 bits!
///
/// The material ID indexes into a global palette that contains
/// all the expensive data (color, emission, roughness, etc.)
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable, PartialEq, Eq)]
pub struct CompressedVoxel(pub u8);

impl CompressedVoxel {
    /// Air (empty voxel).
    pub const AIR: Self = Self(0);
    
    /// Creates a voxel with the given palette index.
    #[inline]
    #[must_use]
    pub const fn new(palette_index: u8) -> Self {
        Self(palette_index)
    }
    
    /// Returns the palette index.
    #[inline]
    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }
    
    /// Returns true if this is air.
    #[inline]
    #[must_use]
    pub const fn is_air(self) -> bool {
        self.0 == 0
    }
    
    /// Returns true if this is solid.
    #[inline]
    #[must_use]
    pub const fn is_solid(self) -> bool {
        self.0 != 0
    }
}

/// Material data in the global palette.
///
/// This lives in a single GPU buffer, NOT per-voxel.
/// 256 materials × 32 bytes = 8KB total (nothing!).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct PaletteMaterial {
    /// Base color (RGB) + roughness in alpha.
    pub color_roughness: [f32; 4],
    /// Emission color (RGB) + metallic in alpha.
    pub emission_metallic: [f32; 4],
}

impl PaletteMaterial {
    /// Air material (invisible).
    pub const AIR: Self = Self {
        color_roughness: [0.0, 0.0, 0.0, 0.0],
        emission_metallic: [0.0, 0.0, 0.0, 0.0],
    };
    
    /// Creates a solid material.
    #[must_use]
    pub const fn solid(r: f32, g: f32, b: f32, roughness: f32) -> Self {
        Self {
            color_roughness: [r, g, b, roughness],
            emission_metallic: [0.0, 0.0, 0.0, 0.0],
        }
    }
    
    /// Creates a neon/emissive material.
    #[must_use]
    pub const fn neon(r: f32, g: f32, b: f32, emission_intensity: f32) -> Self {
        Self {
            color_roughness: [r, g, b, 0.1], // Neon is smooth
            emission_metallic: [r * emission_intensity, g * emission_intensity, b * emission_intensity, 0.0],
        }
    }
    
    /// Creates a metallic material.
    #[must_use]
    pub const fn metal(r: f32, g: f32, b: f32, roughness: f32) -> Self {
        Self {
            color_roughness: [r, g, b, roughness],
            emission_metallic: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

/// Global material palette.
///
/// Uploaded to GPU once and referenced by all voxels.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MaterialPalette {
    /// All materials indexed by voxel data.
    pub materials: [PaletteMaterial; MAX_MATERIALS],
}

impl Default for MaterialPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialPalette {
    /// Creates a new palette with default materials.
    #[must_use]
    pub fn new() -> Self {
        let mut palette = Self {
            materials: [PaletteMaterial::AIR; MAX_MATERIALS],
        };
        
        // Index 0 is always air
        palette.materials[0] = PaletteMaterial::AIR;
        
        // Basic materials
        palette.materials[1] = PaletteMaterial::solid(0.5, 0.5, 0.5, 0.8);  // Stone
        palette.materials[2] = PaletteMaterial::solid(0.4, 0.25, 0.1, 0.9); // Dirt
        palette.materials[3] = PaletteMaterial::solid(0.2, 0.6, 0.2, 0.95); // Grass
        palette.materials[4] = PaletteMaterial::solid(0.6, 0.5, 0.3, 0.85); // Wood
        
        // Neon materials for Neon Prime
        palette.materials[10] = PaletteMaterial::neon(1.0, 0.2, 0.6, 5.0);  // Pink neon
        palette.materials[11] = PaletteMaterial::neon(0.2, 0.9, 1.0, 5.0);  // Cyan neon
        palette.materials[12] = PaletteMaterial::neon(0.6, 0.2, 1.0, 5.0);  // Purple neon
        palette.materials[13] = PaletteMaterial::neon(0.2, 1.0, 0.3, 5.0);  // Green neon
        palette.materials[14] = PaletteMaterial::neon(1.0, 0.8, 0.2, 5.0);  // Gold neon
        
        // Metals
        palette.materials[20] = PaletteMaterial::metal(0.9, 0.9, 0.9, 0.2); // Chrome
        palette.materials[21] = PaletteMaterial::metal(0.8, 0.5, 0.2, 0.3); // Copper
        palette.materials[22] = PaletteMaterial::metal(0.3, 0.3, 0.35, 0.4); // Dark steel
        
        palette
    }
    
    /// Sets a material at the given index.
    pub fn set(&mut self, index: u8, material: PaletteMaterial) {
        self.materials[index as usize] = material;
    }
    
    /// Gets a material by index.
    #[must_use]
    pub fn get(&self, index: u8) -> &PaletteMaterial {
        &self.materials[index as usize]
    }
    
    /// Returns the palette as bytes for GPU upload.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
    
    /// Size of the palette in bytes.
    pub const SIZE: usize = MAX_MATERIALS * std::mem::size_of::<PaletteMaterial>();
}

/// Compressed chunk using 8-bit voxels.
///
/// Memory: 32KB instead of 131KB!
pub struct CompressedChunk {
    /// Chunk coordinate.
    coord: super::chunk::ChunkCoord,
    /// Voxel data - 8 bits each.
    voxels: Box<[CompressedVoxel; super::chunk::CHUNK_VOLUME]>,
    /// Number of solid voxels.
    solid_count: u32,
    /// Dirty flag.
    dirty: bool,
}

impl CompressedChunk {
    /// Memory usage per chunk.
    pub const MEMORY_SIZE: usize = super::chunk::CHUNK_VOLUME; // 32KB
    
    /// Creates a new empty compressed chunk.
    #[must_use]
    pub fn new(coord: super::chunk::ChunkCoord) -> Self {
        Self {
            coord,
            voxels: Box::new([CompressedVoxel::AIR; super::chunk::CHUNK_VOLUME]),
            solid_count: 0,
            dirty: true,
        }
    }
    
    /// Returns the chunk coordinate.
    #[must_use]
    pub const fn coord(&self) -> super::chunk::ChunkCoord {
        self.coord
    }
    
    /// Gets a voxel.
    #[inline]
    #[must_use]
    pub fn get(&self, x: usize, y: usize, z: usize) -> CompressedVoxel {
        let idx = z * super::chunk::CHUNK_SIZE * super::chunk::CHUNK_SIZE 
                + y * super::chunk::CHUNK_SIZE + x;
        self.voxels[idx]
    }
    
    /// Sets a voxel.
    pub fn set(&mut self, x: usize, y: usize, z: usize, voxel: CompressedVoxel) {
        let idx = z * super::chunk::CHUNK_SIZE * super::chunk::CHUNK_SIZE 
                + y * super::chunk::CHUNK_SIZE + x;
        let old = self.voxels[idx];
        
        if old.is_solid() && voxel.is_air() {
            self.solid_count -= 1;
        } else if old.is_air() && voxel.is_solid() {
            self.solid_count += 1;
        }
        
        self.voxels[idx] = voxel;
        self.dirty = true;
    }
    
    /// Returns true if empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.solid_count == 0
    }
    
    /// Returns true if dirty.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }
    
    /// Clears dirty flag.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
    
    /// Returns raw bytes for GPU upload.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&*self.voxels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memory_savings() {
        // Old: 4 bytes per voxel
        let old_size = 32 * 32 * 32 * 4;
        // New: 1 byte per voxel  
        let new_size = CompressedChunk::MEMORY_SIZE;
        
        assert_eq!(old_size, 131072); // 131KB
        assert_eq!(new_size, 32768);  // 32KB
        assert_eq!(old_size / new_size, 4); // 4x savings
    }
    
    #[test]
    fn test_palette_size() {
        // Palette is tiny - shared across ALL chunks
        assert_eq!(MaterialPalette::SIZE, 8192); // 8KB total
    }
}
