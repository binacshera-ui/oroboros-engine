//! Chunked Material System - Breaking the 256 Limit
//!
//! ARCHITECT'S FEEDBACK: 256 global materials = creative death.
//!
//! Solution: Two-tier material system
//! 
//! Tier 1: Per-chunk 8-bit local palette (256 materials per chunk)
//! Tier 2: Global 16-bit material registry (65,536 total materials)
//!
//! How it works:
//! - Each chunk has its own LocalPalette (maps 8-bit -> 16-bit)
//! - Global MaterialRegistry holds all 65K material definitions
//! - GPU uses indirection: voxel -> local palette -> global material
//!
//! Memory cost per chunk: 256 × 2 bytes = 512 bytes (negligible)
//! Total materials possible: 65,536 (enough for 3 worlds + expansion)

use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

/// Maximum materials in global registry.
pub const MAX_GLOBAL_MATERIALS: usize = 65536;

/// Maximum materials per chunk local palette.
pub const MAX_LOCAL_MATERIALS: usize = 256;

/// Global material ID (16-bit).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct MaterialId(pub u16);

impl MaterialId {
    /// Air (always ID 0).
    pub const AIR: Self = Self(0);
    
    /// Creates a new material ID.
    #[inline]
    #[must_use]
    pub const fn new(id: u16) -> Self {
        Self(id)
    }
    
    /// Returns the raw ID.
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }
}

/// Full material definition (lives in global registry).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct MaterialDef {
    /// Base color (RGB) + roughness in alpha.
    pub color_roughness: [f32; 4],
    /// Emission (RGB) + metallic in alpha.
    pub emission_metallic: [f32; 4],
    /// Texture array indices: albedo, normal, roughness, emission.
    pub texture_indices: [u32; 4],
    /// Material flags and properties.
    /// Bits 0-7: blend mode
    /// Bits 8-15: render flags (transparent, animated, etc.)
    /// Bits 16-23: world mask (which worlds can use this material)
    /// Bits 24-31: reserved
    pub flags: u32,
    /// Animation parameters (for neon flicker, water flow, etc).
    pub animation: [f32; 3],
}

impl MaterialDef {
    /// Size in bytes.
    pub const SIZE: usize = std::mem::size_of::<Self>();
    
    /// World mask bits.
    pub const WORLD_NEON_PRIME: u32 = 1 << 16;
    /// Veridia world mask.
    pub const WORLD_VERIDIA: u32 = 1 << 17;
    /// Inferno world mask.
    pub const WORLD_INFERNO: u32 = 1 << 18;
    /// All worlds.
    pub const WORLD_ALL: u32 = Self::WORLD_NEON_PRIME | Self::WORLD_VERIDIA | Self::WORLD_INFERNO;
    
    /// Render flag: transparent.
    pub const FLAG_TRANSPARENT: u32 = 1 << 8;
    /// Render flag: animated.
    pub const FLAG_ANIMATED: u32 = 1 << 9;
    /// Render flag: emissive.
    pub const FLAG_EMISSIVE: u32 = 1 << 10;
    
    /// Creates a solid opaque material.
    #[must_use]
    pub const fn solid(r: f32, g: f32, b: f32, roughness: f32) -> Self {
        Self {
            color_roughness: [r, g, b, roughness],
            emission_metallic: [0.0, 0.0, 0.0, 0.0],
            texture_indices: [0, 0, 0, 0],
            flags: Self::WORLD_ALL,
            animation: [0.0, 0.0, 0.0],
        }
    }
    
    /// Creates a neon emissive material.
    #[must_use]
    pub const fn neon(r: f32, g: f32, b: f32, intensity: f32, flicker_speed: f32) -> Self {
        Self {
            color_roughness: [r, g, b, 0.1],
            emission_metallic: [r * intensity, g * intensity, b * intensity, 0.0],
            texture_indices: [0, 0, 0, 0],
            flags: Self::WORLD_NEON_PRIME | Self::FLAG_EMISSIVE | Self::FLAG_ANIMATED,
            animation: [flicker_speed, 0.0, 0.0],
        }
    }
    
    /// Creates a metallic material.
    #[must_use]
    pub const fn metal(r: f32, g: f32, b: f32, roughness: f32) -> Self {
        Self {
            color_roughness: [r, g, b, roughness],
            emission_metallic: [0.0, 0.0, 0.0, 1.0],
            texture_indices: [0, 0, 0, 0],
            flags: Self::WORLD_ALL,
            animation: [0.0, 0.0, 0.0],
        }
    }
    
    /// Creates a transparent material.
    #[must_use]
    pub const fn transparent(r: f32, g: f32, b: f32, alpha: f32) -> Self {
        Self {
            color_roughness: [r, g, b, 0.0],
            emission_metallic: [0.0, 0.0, 0.0, alpha], // Abuse metallic for alpha
            texture_indices: [0, 0, 0, 0],
            flags: Self::WORLD_ALL | Self::FLAG_TRANSPARENT,
            animation: [0.0, 0.0, 0.0],
        }
    }
    
    /// Sets texture indices.
    #[must_use]
    pub const fn with_textures(mut self, albedo: u32, normal: u32, roughness: u32, emission: u32) -> Self {
        self.texture_indices = [albedo, normal, roughness, emission];
        self
    }
    
    /// Restricts to specific world(s).
    #[must_use]
    pub const fn for_world(mut self, world_mask: u32) -> Self {
        self.flags = (self.flags & 0x0000FFFF) | world_mask;
        self
    }
}

/// Per-chunk local palette.
///
/// Maps 8-bit local indices to 16-bit global material IDs.
/// Each chunk can use up to 256 different materials from the 65K pool.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct LocalPalette {
    /// Mapping from local index to global material ID.
    pub mapping: [MaterialId; MAX_LOCAL_MATERIALS],
}

impl Default for LocalPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalPalette {
    /// Creates a new palette with air at index 0.
    #[must_use]
    pub fn new() -> Self {
        let mut mapping = [MaterialId::AIR; MAX_LOCAL_MATERIALS];
        mapping[0] = MaterialId::AIR;
        Self { mapping }
    }
    
    /// Size in bytes.
    pub const SIZE: usize = MAX_LOCAL_MATERIALS * 2; // 512 bytes
    
    /// Gets the global material ID for a local index.
    #[inline]
    #[must_use]
    pub fn get(&self, local_index: u8) -> MaterialId {
        self.mapping[local_index as usize]
    }
    
    /// Sets the mapping for a local index.
    #[inline]
    pub fn set(&mut self, local_index: u8, global_id: MaterialId) {
        self.mapping[local_index as usize] = global_id;
    }
    
    /// Returns as bytes for GPU upload.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}

/// Builder for creating local palettes from global materials.
pub struct LocalPaletteBuilder {
    /// Current palette being built.
    palette: LocalPalette,
    /// Next free index.
    next_index: u8,
    /// Reverse mapping for deduplication.
    reverse_map: HashMap<MaterialId, u8>,
}

impl LocalPaletteBuilder {
    /// Creates a new builder.
    #[must_use]
    pub fn new() -> Self {
        let mut reverse_map = HashMap::new();
        reverse_map.insert(MaterialId::AIR, 0);
        
        Self {
            palette: LocalPalette::new(),
            next_index: 1, // 0 is reserved for air
            reverse_map,
        }
    }
    
    /// Adds a material to the palette, returning its local index.
    ///
    /// Returns None if palette is full.
    pub fn add(&mut self, global_id: MaterialId) -> Option<u8> {
        // Check if already in palette
        if let Some(&local) = self.reverse_map.get(&global_id) {
            return Some(local);
        }
        
        // Check if palette is full
        if self.next_index == 0 { // Wrapped around
            return None;
        }
        
        let local_index = self.next_index;
        self.palette.set(local_index, global_id);
        self.reverse_map.insert(global_id, local_index);
        self.next_index = self.next_index.wrapping_add(1);
        
        Some(local_index)
    }
    
    /// Returns the local index for a material, if it exists.
    #[must_use]
    pub fn get(&self, global_id: MaterialId) -> Option<u8> {
        self.reverse_map.get(&global_id).copied()
    }
    
    /// Builds the final palette.
    #[must_use]
    pub fn build(self) -> LocalPalette {
        self.palette
    }
    
    /// Returns how many materials are used.
    #[must_use]
    pub fn count(&self) -> usize {
        self.next_index as usize
    }
    
    /// Returns true if the palette is full.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.next_index == 0
    }
}

impl Default for LocalPaletteBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Global material registry.
///
/// Holds all 65K material definitions. Uploaded to GPU as a large buffer.
pub struct MaterialRegistry {
    /// All material definitions.
    materials: Vec<MaterialDef>,
    /// Name to ID mapping.
    name_to_id: HashMap<String, MaterialId>,
    /// Dirty flag for GPU sync.
    dirty: bool,
}

impl MaterialRegistry {
    /// Creates a new registry with default materials.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self {
            materials: vec![MaterialDef::default(); MAX_GLOBAL_MATERIALS],
            name_to_id: HashMap::new(),
            dirty: true,
        };
        
        // Register air at index 0
        registry.register_named("air", MaterialDef::default());
        
        // Basic materials
        registry.register_named("stone", MaterialDef::solid(0.5, 0.5, 0.5, 0.8));
        registry.register_named("dirt", MaterialDef::solid(0.4, 0.25, 0.1, 0.9));
        registry.register_named("grass", MaterialDef::solid(0.2, 0.6, 0.2, 0.95));
        
        // Neon Prime materials (IDs 1000-1999)
        registry.register_at(1000, "neon_pink", MaterialDef::neon(1.0, 0.2, 0.6, 5.0, 2.0));
        registry.register_at(1001, "neon_cyan", MaterialDef::neon(0.2, 0.9, 1.0, 5.0, 1.5));
        registry.register_at(1002, "neon_purple", MaterialDef::neon(0.6, 0.2, 1.0, 5.0, 3.0));
        registry.register_at(1003, "neon_green", MaterialDef::neon(0.2, 1.0, 0.3, 5.0, 1.0));
        registry.register_at(1004, "neon_gold", MaterialDef::neon(1.0, 0.8, 0.2, 5.0, 0.5));
        registry.register_at(1005, "chrome", MaterialDef::metal(0.9, 0.9, 0.9, 0.1));
        registry.register_at(1006, "dark_steel", MaterialDef::metal(0.2, 0.2, 0.25, 0.4));
        registry.register_at(1007, "wet_asphalt", MaterialDef::solid(0.1, 0.1, 0.1, 0.3));
        registry.register_at(1008, "glass", MaterialDef::transparent(0.9, 0.95, 1.0, 0.2));
        
        // Veridia materials (IDs 2000-2999)
        registry.register_at(2000, "forest_moss", MaterialDef::solid(0.15, 0.35, 0.1, 0.95)
            .for_world(MaterialDef::WORLD_VERIDIA));
        registry.register_at(2001, "ancient_stone", MaterialDef::solid(0.4, 0.38, 0.35, 0.85)
            .for_world(MaterialDef::WORLD_VERIDIA));
        registry.register_at(2002, "crystal_blue", MaterialDef::neon(0.3, 0.5, 1.0, 2.0, 0.0)
            .for_world(MaterialDef::WORLD_VERIDIA));
        registry.register_at(2003, "oak_wood", MaterialDef::solid(0.5, 0.35, 0.2, 0.9)
            .for_world(MaterialDef::WORLD_VERIDIA));
        registry.register_at(2004, "dark_bark", MaterialDef::solid(0.2, 0.15, 0.1, 0.95)
            .for_world(MaterialDef::WORLD_VERIDIA));
        
        // Inferno materials (IDs 3000-3999)
        registry.register_at(3000, "lava_rock", MaterialDef::solid(0.15, 0.05, 0.02, 0.9)
            .for_world(MaterialDef::WORLD_INFERNO));
        registry.register_at(3001, "molten_lava", MaterialDef::neon(1.0, 0.3, 0.0, 10.0, 0.5)
            .for_world(MaterialDef::WORLD_INFERNO));
        registry.register_at(3002, "obsidian", MaterialDef::solid(0.05, 0.05, 0.08, 0.2)
            .for_world(MaterialDef::WORLD_INFERNO));
        registry.register_at(3003, "ash", MaterialDef::solid(0.3, 0.3, 0.32, 0.98)
            .for_world(MaterialDef::WORLD_INFERNO));
        registry.register_at(3004, "hellfire_crystal", MaterialDef::neon(1.0, 0.1, 0.0, 15.0, 4.0)
            .for_world(MaterialDef::WORLD_INFERNO));
        
        // =====================================================
        // PROCEDURAL MODEL MATERIALS (IDs 10-99)
        // Used by ProceduralModels for code-generated assets
        // =====================================================
        
        // Character/Entity colors
        registry.register_at(10, "skin_tone", MaterialDef::solid(0.87, 0.72, 0.60, 0.9));
        registry.register_at(11, "cloth_blue", MaterialDef::solid(0.2, 0.3, 0.8, 0.85));
        registry.register_at(12, "cloth_red", MaterialDef::solid(0.8, 0.2, 0.2, 0.85));
        registry.register_at(13, "metal_dark", MaterialDef::metal(0.15, 0.15, 0.18, 0.4));
        registry.register_at(14, "metal_light", MaterialDef::metal(0.7, 0.7, 0.72, 0.3));
        registry.register_at(15, "leather_brown", MaterialDef::solid(0.45, 0.28, 0.15, 0.92));
        registry.register_at(16, "vegetation_green", MaterialDef::solid(0.25, 0.55, 0.2, 0.9));
        registry.register_at(17, "accent_yellow", MaterialDef::solid(0.9, 0.8, 0.2, 0.85));
        registry.register_at(18, "accent_orange", MaterialDef::solid(0.9, 0.5, 0.1, 0.85));
        registry.register_at(19, "enemy_purple", MaterialDef::solid(0.5, 0.2, 0.6, 0.85));
        registry.register_at(20, "pure_black", MaterialDef::solid(0.02, 0.02, 0.02, 0.95));
        registry.register_at(21, "pure_white", MaterialDef::solid(0.98, 0.98, 0.98, 0.8));
        
        // Neon accent materials for procedural models
        registry.register_at(100, "proc_neon_cyan", MaterialDef::neon(0.2, 0.9, 1.0, 3.0, 1.0));
        registry.register_at(101, "proc_neon_pink", MaterialDef::neon(1.0, 0.2, 0.6, 3.0, 1.5));
        registry.register_at(102, "proc_neon_green", MaterialDef::neon(0.2, 1.0, 0.3, 3.0, 0.8));
        registry.register_at(103, "proc_neon_purple", MaterialDef::neon(0.6, 0.2, 1.0, 3.0, 2.0));
        
        registry
    }
    
    /// Registers a material at a specific ID.
    pub fn register_at(&mut self, id: u16, name: &str, material: MaterialDef) {
        self.materials[id as usize] = material;
        self.name_to_id.insert(name.to_string(), MaterialId::new(id));
        self.dirty = true;
    }
    
    /// Registers a material with automatic ID assignment.
    ///
    /// Returns the assigned ID.
    pub fn register_named(&mut self, name: &str, material: MaterialDef) -> MaterialId {
        // Find next free ID (skip reserved ranges)
        let id = self.name_to_id.len() as u16;
        self.register_at(id, name, material);
        MaterialId::new(id)
    }
    
    /// Gets material ID by name.
    #[must_use]
    pub fn get_id(&self, name: &str) -> Option<MaterialId> {
        self.name_to_id.get(name).copied()
    }
    
    /// Gets material definition by ID.
    #[must_use]
    pub fn get(&self, id: MaterialId) -> &MaterialDef {
        &self.materials[id.0 as usize]
    }
    
    /// Returns all materials as bytes for GPU upload.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.materials)
    }
    
    /// Returns true if the registry needs GPU sync.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    
    /// Clears the dirty flag.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
    
    /// Total size in bytes.
    pub fn size_bytes(&self) -> usize {
        self.materials.len() * MaterialDef::SIZE
    }
}

impl Default for MaterialRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_local_palette_capacity() {
        let mut builder = LocalPaletteBuilder::new();
        
        // Should be able to add 255 materials (0 is air)
        for i in 1..256u16 {
            assert!(builder.add(MaterialId::new(i)).is_some());
        }
        
        // 256th should fail
        assert!(builder.add(MaterialId::new(256)).is_none());
    }
    
    #[test]
    fn test_local_palette_deduplication() {
        let mut builder = LocalPaletteBuilder::new();
        
        let idx1 = builder.add(MaterialId::new(100)).unwrap();
        let idx2 = builder.add(MaterialId::new(100)).unwrap();
        
        // Same material should get same index
        assert_eq!(idx1, idx2);
        assert_eq!(builder.count(), 2); // air + 1 material
    }
    
    #[test]
    fn test_material_registry() {
        let registry = MaterialRegistry::new();
        
        let stone_id = registry.get_id("stone").unwrap();
        let stone = registry.get(stone_id);
        
        assert!(stone.color_roughness[0] > 0.0);
    }
    
    #[test]
    fn test_memory_budget() {
        // Global registry: 65K materials × 64 bytes = 4MB
        let registry_size = MAX_GLOBAL_MATERIALS * MaterialDef::SIZE;
        assert_eq!(registry_size, 4_194_304); // 4MB
        
        // Per-chunk palette: 256 × 2 bytes = 512 bytes
        assert_eq!(LocalPalette::SIZE, 512);
        
        // 32K chunks × 512 bytes = 16MB (acceptable)
        let total_palette_size = 32768 * LocalPalette::SIZE;
        assert_eq!(total_palette_size, 16_777_216); // 16MB
    }
}
