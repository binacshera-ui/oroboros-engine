//! Procedural Voxel Models - "Data as Art"
//!
//! Static library of voxel models constructed using 3D arrays in Rust code.
//! No artistic talent required - only math.
//!
//! ## ARCHITECT'S MANDATE
//!
//! - Fast, precise, zero external dependency
//! - All models defined in code using loops and arrays
//! - Convert to engine Instance format
//! - Thread-safe, no runtime allocation for access

use crate::instancing::InstanceData;
use crate::voxel::Voxel;

/// Model bounds in voxels.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModelBounds {
    /// Width (X axis).
    pub width: u8,
    /// Height (Y axis).
    pub height: u8,
    /// Depth (Z axis).
    pub depth: u8,
}

impl ModelBounds {
    /// Creates new bounds.
    #[inline]
    #[must_use]
    pub const fn new(width: u8, height: u8, depth: u8) -> Self {
        Self { width, height, depth }
    }
    
    /// Total volume in voxels.
    #[inline]
    #[must_use]
    pub const fn volume(&self) -> usize {
        self.width as usize * self.height as usize * self.depth as usize
    }
}

/// Single voxel in a model with position and material.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModelVoxel {
    /// Local X position (0-31).
    pub x: u8,
    /// Local Y position (0-31).
    pub y: u8,
    /// Local Z position (0-31).
    pub z: u8,
    /// Material ID for this voxel.
    pub material_id: u8,
    /// Optional emission color (R, G, B, intensity).
    pub emission: Option<[f32; 4]>,
}

impl ModelVoxel {
    /// Creates a solid voxel.
    #[inline]
    #[must_use]
    pub const fn solid(x: u8, y: u8, z: u8, material_id: u8) -> Self {
        Self { x, y, z, material_id, emission: None }
    }
    
    /// Creates an emissive voxel (for neon effects).
    #[inline]
    #[must_use]
    pub fn emissive(x: u8, y: u8, z: u8, material_id: u8, r: f32, g: f32, b: f32, intensity: f32) -> Self {
        Self { x, y, z, material_id, emission: Some([r, g, b, intensity]) }
    }
    
    /// Converts to engine Voxel type.
    #[must_use]
    pub fn to_voxel(&self) -> Voxel {
        if let Some(emission) = self.emission {
            let r = (emission[0] * 255.0) as u8;
            let g = (emission[1] * 255.0) as u8;
            let b = (emission[2] * 255.0) as u8;
            Voxel::neon(self.material_id, r, g, b)
        } else {
            Voxel::new(self.material_id)
        }
    }
}

/// A complete voxel model - sparse representation.
#[derive(Debug, Clone)]
pub struct VoxelModel {
    /// Model name/identifier.
    pub name: &'static str,
    /// Model bounds.
    pub bounds: ModelBounds,
    /// All solid voxels in the model.
    pub voxels: Vec<ModelVoxel>,
    /// Origin offset (for centering/positioning).
    pub origin: [f32; 3],
}

impl VoxelModel {
    /// Creates instances from this model at a world position.
    ///
    /// Returns vector of `InstanceData` for GPU upload.
    #[must_use]
    pub fn to_instances(&self, world_x: f32, world_y: f32, world_z: f32) -> Vec<InstanceData> {
        let mut instances = Vec::with_capacity(self.voxels.len() * 6); // Max 6 faces per voxel
        
        for voxel in &self.voxels {
            let x = world_x + voxel.x as f32 - self.origin[0];
            let y = world_y + voxel.y as f32 - self.origin[1];
            let z = world_z + voxel.z as f32 - self.origin[2];
            
            // Check each face for visibility (simplified - no neighbor checking)
            // In production, use greedy meshing for optimization
            for face in 0..6u32 {
                let instance = if let Some(emission) = voxel.emission {
                    InstanceData::neon(
                        x, y, z,
                        1.0, 1.0, // Unit voxel
                        face,
                        voxel.material_id as u32,
                        emission[0], emission[1], emission[2], emission[3],
                    )
                } else {
                    InstanceData::from_quad(
                        x, y, z,
                        1.0, 1.0,
                        face,
                        voxel.material_id as u32,
                        255, // Default light level
                    )
                };
                instances.push(instance);
            }
        }
        
        instances
    }
    
    /// Returns the number of solid voxels.
    #[inline]
    #[must_use]
    pub fn voxel_count(&self) -> usize {
        self.voxels.len()
    }
    
    /// Checks if model contains a voxel at position.
    #[must_use]
    pub fn has_voxel_at(&self, x: u8, y: u8, z: u8) -> bool {
        self.voxels.iter().any(|v| v.x == x && v.y == y && v.z == z)
    }
}

/// Builder for constructing voxel models programmatically.
pub struct VoxelModelBuilder {
    name: &'static str,
    voxels: Vec<ModelVoxel>,
    bounds: ModelBounds,
    origin: [f32; 3],
}

impl VoxelModelBuilder {
    /// Creates a new builder.
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            voxels: Vec::new(),
            bounds: ModelBounds::default(),
            origin: [0.0, 0.0, 0.0],
        }
    }
    
    /// Sets the origin offset.
    #[must_use]
    pub fn with_origin(mut self, x: f32, y: f32, z: f32) -> Self {
        self.origin = [x, y, z];
        self
    }
    
    /// Adds a single voxel.
    pub fn add_voxel(&mut self, x: u8, y: u8, z: u8, material_id: u8) {
        self.voxels.push(ModelVoxel::solid(x, y, z, material_id));
        self.update_bounds(x, y, z);
    }
    
    /// Adds an emissive voxel.
    pub fn add_emissive(&mut self, x: u8, y: u8, z: u8, material_id: u8, r: f32, g: f32, b: f32, intensity: f32) {
        self.voxels.push(ModelVoxel::emissive(x, y, z, material_id, r, g, b, intensity));
        self.update_bounds(x, y, z);
    }
    
    /// Fills a box with voxels.
    pub fn fill_box(&mut self, x1: u8, y1: u8, z1: u8, x2: u8, y2: u8, z2: u8, material_id: u8) {
        let (min_x, max_x) = (x1.min(x2), x1.max(x2));
        let (min_y, max_y) = (y1.min(y2), y1.max(y2));
        let (min_z, max_z) = (z1.min(z2), z1.max(z2));
        
        for z in min_z..=max_z {
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    self.add_voxel(x, y, z, material_id);
                }
            }
        }
    }
    
    /// Fills a hollow box (shell only).
    pub fn fill_hollow_box(&mut self, x1: u8, y1: u8, z1: u8, x2: u8, y2: u8, z2: u8, material_id: u8) {
        let (min_x, max_x) = (x1.min(x2), x1.max(x2));
        let (min_y, max_y) = (y1.min(y2), y1.max(y2));
        let (min_z, max_z) = (z1.min(z2), z1.max(z2));
        
        for z in min_z..=max_z {
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let on_surface = x == min_x || x == max_x 
                        || y == min_y || y == max_y 
                        || z == min_z || z == max_z;
                    if on_surface {
                        self.add_voxel(x, y, z, material_id);
                    }
                }
            }
        }
    }
    
    /// Fills a sphere.
    pub fn fill_sphere(&mut self, cx: u8, cy: u8, cz: u8, radius: u8, material_id: u8) {
        let r = radius as i32;
        let r_sq = r * r;
        
        for dz in -r..=r {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx*dx + dy*dy + dz*dz <= r_sq {
                        let x = (cx as i32 + dx) as u8;
                        let y = (cy as i32 + dy) as u8;
                        let z = (cz as i32 + dz) as u8;
                        self.add_voxel(x, y, z, material_id);
                    }
                }
            }
        }
    }
    
    /// Fills a cylinder along Y axis.
    pub fn fill_cylinder_y(&mut self, cx: u8, cz: u8, y1: u8, y2: u8, radius: u8, material_id: u8) {
        let r = radius as i32;
        let r_sq = r * r;
        let (min_y, max_y) = (y1.min(y2), y1.max(y2));
        
        for y in min_y..=max_y {
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx*dx + dz*dz <= r_sq {
                        let x = (cx as i32 + dx) as u8;
                        let z = (cz as i32 + dz) as u8;
                        self.add_voxel(x, y, z, material_id);
                    }
                }
            }
        }
    }
    
    /// Updates bounds based on new voxel position.
    fn update_bounds(&mut self, x: u8, y: u8, z: u8) {
        self.bounds.width = self.bounds.width.max(x + 1);
        self.bounds.height = self.bounds.height.max(y + 1);
        self.bounds.depth = self.bounds.depth.max(z + 1);
    }
    
    /// Builds the final model.
    #[must_use]
    pub fn build(self) -> VoxelModel {
        VoxelModel {
            name: self.name,
            bounds: self.bounds,
            voxels: self.voxels,
            origin: self.origin,
        }
    }
}

// ============================================================================
// MATERIAL IDS FOR PROCEDURAL MODELS
// ============================================================================

/// Material IDs for procedural model colors.
/// These IDs match the MaterialRegistry entries (IDs 10-99 and 100-103).
pub mod colors {
    // Basic colors (matching MaterialRegistry IDs 10-21)
    /// Skin tone (ID 10).
    pub const SKIN: u8 = 10;
    /// Blue clothing (ID 11).
    pub const BLUE: u8 = 11;
    /// Red clothing (ID 12).
    pub const RED: u8 = 12;
    /// Dark gray metal/armor (ID 13).
    pub const DARK_GRAY: u8 = 13;
    /// Light gray metal (ID 14).
    pub const LIGHT_GRAY: u8 = 14;
    /// Brown leather/wood (ID 15).
    pub const BROWN: u8 = 15;
    /// Green vegetation (ID 16).
    pub const GREEN: u8 = 16;
    /// Yellow accent (ID 17).
    #[allow(dead_code)]
    pub const YELLOW: u8 = 17;
    /// Orange accent (ID 18).
    pub const ORANGE: u8 = 18;
    /// Purple enemy accent (ID 19).
    pub const PURPLE: u8 = 19;
    /// Pure black (ID 20).
    #[allow(dead_code)]
    pub const BLACK: u8 = 20;
    /// Pure white (ID 21).
    #[allow(dead_code)]
    pub const WHITE: u8 = 21;
    
    // Neon colors for emissive effects (IDs 100-103)
    /// Neon cyan glow (ID 100).
    pub const NEON_CYAN: u8 = 100;
    /// Neon pink glow (ID 101).
    pub const NEON_PINK: u8 = 101;
    /// Neon green glow (ID 102).
    #[allow(dead_code)]
    pub const NEON_GREEN: u8 = 102;
    /// Neon purple glow (ID 103).
    pub const NEON_PURPLE: u8 = 103;
}

// ============================================================================
// PROCEDURAL MODEL LIBRARY
// ============================================================================

/// Static library of all procedural models.
pub struct ProceduralModels;

impl ProceduralModels {
    /// Creates the player humanoid model.
    ///
    /// Structure:
    /// - Head: 4x4x4 sphere at top
    /// - Torso: 4x6x2 box
    /// - Arms: 2x6x2 cylinders on sides
    /// - Legs: 2x6x2 cylinders at bottom
    #[must_use]
    pub fn player() -> VoxelModel {
        let mut builder = VoxelModelBuilder::new("player")
            .with_origin(4.0, 0.0, 2.0); // Center on feet
        
        // === LEGS (Y: 0-5) ===
        // Left leg
        builder.fill_box(1, 0, 1, 2, 5, 2, colors::BLUE);
        // Right leg
        builder.fill_box(5, 0, 1, 6, 5, 2, colors::BLUE);
        
        // === TORSO (Y: 6-13) ===
        builder.fill_box(1, 6, 0, 6, 13, 3, colors::RED);
        
        // === ARMS (Y: 8-13) ===
        // Left arm
        builder.fill_box(0, 8, 1, 0, 13, 2, colors::SKIN);
        // Right arm
        builder.fill_box(7, 8, 1, 7, 13, 2, colors::SKIN);
        
        // === NECK (Y: 14) ===
        builder.fill_box(3, 14, 1, 4, 14, 2, colors::SKIN);
        
        // === HEAD (Y: 15-18) ===
        builder.fill_sphere(4, 17, 2, 2, colors::SKIN);
        
        // === EYES ===
        builder.add_emissive(3, 17, 0, colors::NEON_CYAN, 0.2, 0.9, 1.0, 2.0);
        builder.add_emissive(5, 17, 0, colors::NEON_CYAN, 0.2, 0.9, 1.0, 2.0);
        
        builder.build()
    }
    
    /// Creates an enemy model - spiky aggressive shape.
    ///
    /// Structure:
    /// - Core: Irregular angular body
    /// - Spikes: Protruding in multiple directions
    /// - Eyes: Glowing red/purple
    #[must_use]
    pub fn enemy() -> VoxelModel {
        let mut builder = VoxelModelBuilder::new("enemy")
            .with_origin(6.0, 0.0, 6.0); // Center
        
        // === BASE/BODY CORE (Y: 0-6) ===
        // Main body - angular, threatening shape
        builder.fill_box(3, 0, 3, 8, 6, 8, colors::DARK_GRAY);
        
        // === ARMORED PLATES ===
        builder.fill_box(2, 2, 4, 2, 5, 7, colors::PURPLE);
        builder.fill_box(9, 2, 4, 9, 5, 7, colors::PURPLE);
        builder.fill_box(4, 2, 2, 7, 5, 2, colors::PURPLE);
        builder.fill_box(4, 2, 9, 7, 5, 9, colors::PURPLE);
        
        // === HEAD/TOP (Y: 7-10) ===
        builder.fill_box(4, 7, 4, 7, 10, 7, colors::DARK_GRAY);
        
        // === SPIKES ===
        // Top spike
        builder.add_voxel(5, 11, 5, colors::PURPLE);
        builder.add_voxel(5, 12, 5, colors::PURPLE);
        builder.add_voxel(5, 13, 5, colors::PURPLE);
        builder.add_voxel(6, 11, 6, colors::PURPLE);
        builder.add_voxel(6, 12, 6, colors::PURPLE);
        
        // Side spikes (left)
        builder.add_voxel(1, 4, 5, colors::PURPLE);
        builder.add_voxel(0, 4, 5, colors::PURPLE);
        builder.add_voxel(1, 4, 6, colors::PURPLE);
        
        // Side spikes (right)
        builder.add_voxel(10, 4, 5, colors::PURPLE);
        builder.add_voxel(11, 4, 5, colors::PURPLE);
        builder.add_voxel(10, 4, 6, colors::PURPLE);
        
        // Front spikes
        builder.add_voxel(5, 4, 1, colors::PURPLE);
        builder.add_voxel(5, 4, 0, colors::PURPLE);
        builder.add_voxel(6, 4, 1, colors::PURPLE);
        
        // Back spikes
        builder.add_voxel(5, 4, 10, colors::PURPLE);
        builder.add_voxel(5, 4, 11, colors::PURPLE);
        builder.add_voxel(6, 4, 10, colors::PURPLE);
        
        // Corner spikes (diagonal aggression)
        builder.add_voxel(2, 3, 2, colors::PURPLE);
        builder.add_voxel(1, 3, 1, colors::PURPLE);
        builder.add_voxel(9, 3, 2, colors::PURPLE);
        builder.add_voxel(10, 3, 1, colors::PURPLE);
        builder.add_voxel(2, 3, 9, colors::PURPLE);
        builder.add_voxel(1, 3, 10, colors::PURPLE);
        builder.add_voxel(9, 3, 9, colors::PURPLE);
        builder.add_voxel(10, 3, 10, colors::PURPLE);
        
        // === GLOWING EYES ===
        builder.add_emissive(4, 8, 3, colors::NEON_PINK, 1.0, 0.2, 0.4, 5.0);
        builder.add_emissive(7, 8, 3, colors::NEON_PINK, 1.0, 0.2, 0.4, 5.0);
        
        // === CORE ENERGY (center glow) ===
        builder.add_emissive(5, 3, 5, colors::NEON_PURPLE, 0.6, 0.2, 1.0, 3.0);
        builder.add_emissive(6, 3, 6, colors::NEON_PURPLE, 0.6, 0.2, 1.0, 3.0);
        
        builder.build()
    }
    
    /// Creates a simple sword weapon model.
    #[must_use]
    pub fn sword() -> VoxelModel {
        let mut builder = VoxelModelBuilder::new("sword")
            .with_origin(1.0, 0.0, 0.5);
        
        // Handle
        builder.fill_box(0, 0, 0, 1, 3, 0, colors::BROWN);
        
        // Guard
        builder.fill_box(0, 4, 0, 2, 4, 1, colors::DARK_GRAY);
        
        // Blade
        builder.fill_box(0, 5, 0, 1, 14, 0, colors::LIGHT_GRAY);
        builder.add_voxel(0, 15, 0, colors::LIGHT_GRAY);
        
        builder.build()
    }
    
    /// Creates a simple shield model.
    #[must_use]
    pub fn shield() -> VoxelModel {
        let mut builder = VoxelModelBuilder::new("shield")
            .with_origin(3.0, 4.0, 0.5);
        
        // Shield face
        builder.fill_box(0, 0, 0, 5, 7, 0, colors::DARK_GRAY);
        
        // Border
        for x in 0..=5 {
            builder.add_voxel(x, 0, 1, colors::BROWN);
            builder.add_voxel(x, 7, 1, colors::BROWN);
        }
        for y in 0..=7 {
            builder.add_voxel(0, y, 1, colors::BROWN);
            builder.add_voxel(5, y, 1, colors::BROWN);
        }
        
        // Emblem (center cross)
        builder.add_emissive(2, 3, 0, colors::NEON_CYAN, 0.2, 0.9, 1.0, 2.0);
        builder.add_emissive(3, 3, 0, colors::NEON_CYAN, 0.2, 0.9, 1.0, 2.0);
        builder.add_emissive(2, 4, 0, colors::NEON_CYAN, 0.2, 0.9, 1.0, 2.0);
        builder.add_emissive(3, 4, 0, colors::NEON_CYAN, 0.2, 0.9, 1.0, 2.0);
        
        builder.build()
    }
    
    /// Creates a basic crate/box prop.
    #[must_use]
    pub fn crate_box() -> VoxelModel {
        let mut builder = VoxelModelBuilder::new("crate")
            .with_origin(2.0, 0.0, 2.0);
        
        // Wooden crate
        builder.fill_hollow_box(0, 0, 0, 3, 3, 3, colors::BROWN);
        
        // Metal bands
        for y in [0, 3] {
            builder.add_voxel(0, y, 0, colors::DARK_GRAY);
            builder.add_voxel(3, y, 0, colors::DARK_GRAY);
            builder.add_voxel(0, y, 3, colors::DARK_GRAY);
            builder.add_voxel(3, y, 3, colors::DARK_GRAY);
        }
        
        builder.build()
    }
    
    /// Creates a barrel prop.
    #[must_use]
    pub fn barrel() -> VoxelModel {
        let mut builder = VoxelModelBuilder::new("barrel")
            .with_origin(2.0, 0.0, 2.0);
        
        // Main body (cylinder)
        builder.fill_cylinder_y(2, 2, 0, 5, 2, colors::BROWN);
        
        // Metal bands
        builder.fill_cylinder_y(2, 2, 0, 0, 2, colors::DARK_GRAY);
        builder.fill_cylinder_y(2, 2, 2, 2, 2, colors::DARK_GRAY);
        builder.fill_cylinder_y(2, 2, 5, 5, 2, colors::DARK_GRAY);
        
        builder.build()
    }
    
    /// Creates a tree model for Veridia world.
    #[must_use]
    pub fn tree() -> VoxelModel {
        let mut builder = VoxelModelBuilder::new("tree")
            .with_origin(4.0, 0.0, 4.0);
        
        // Trunk
        builder.fill_cylinder_y(4, 4, 0, 8, 1, colors::BROWN);
        
        // Foliage (layered spheres)
        builder.fill_sphere(4, 11, 4, 3, colors::GREEN);
        builder.fill_sphere(4, 14, 4, 2, colors::GREEN);
        
        // Some leaves stick out
        builder.add_voxel(1, 10, 4, colors::GREEN);
        builder.add_voxel(7, 10, 4, colors::GREEN);
        builder.add_voxel(4, 10, 1, colors::GREEN);
        builder.add_voxel(4, 10, 7, colors::GREEN);
        
        builder.build()
    }
    
    /// Creates a crystal prop for caves/dungeons.
    #[must_use]
    pub fn crystal() -> VoxelModel {
        let mut builder = VoxelModelBuilder::new("crystal")
            .with_origin(2.0, 0.0, 2.0);
        
        // Base
        builder.fill_box(1, 0, 1, 2, 0, 2, colors::DARK_GRAY);
        
        // Crystal spire
        builder.add_emissive(1, 1, 1, colors::NEON_CYAN, 0.2, 0.9, 1.0, 4.0);
        builder.add_emissive(2, 1, 2, colors::NEON_CYAN, 0.2, 0.9, 1.0, 4.0);
        builder.add_emissive(1, 2, 1, colors::NEON_CYAN, 0.2, 0.9, 1.0, 5.0);
        builder.add_emissive(2, 2, 2, colors::NEON_CYAN, 0.2, 0.9, 1.0, 5.0);
        builder.add_emissive(1, 3, 1, colors::NEON_CYAN, 0.2, 0.9, 1.0, 6.0);
        builder.add_emissive(1, 4, 1, colors::NEON_CYAN, 0.2, 0.9, 1.0, 7.0);
        builder.add_emissive(1, 5, 1, colors::NEON_CYAN, 0.2, 0.9, 1.0, 8.0);
        
        builder.build()
    }
    
    /// Creates a dragon model for Inferno world boss.
    /// 
    /// This is a simplified low-poly dragon for placeholder purposes.
    #[must_use]
    pub fn dragon() -> VoxelModel {
        let mut builder = VoxelModelBuilder::new("dragon")
            .with_origin(8.0, 0.0, 16.0);
        
        // === BODY (main mass) ===
        builder.fill_box(4, 2, 8, 11, 8, 24, colors::RED);
        
        // === NECK ===
        builder.fill_box(6, 6, 4, 9, 10, 8, colors::RED);
        builder.fill_box(6, 8, 0, 9, 12, 4, colors::RED);
        
        // === HEAD ===
        builder.fill_box(5, 10, 0, 10, 14, 4, colors::RED);
        // Snout
        builder.fill_box(6, 10, 0, 9, 12, 0, colors::DARK_GRAY);
        // Horns
        builder.add_voxel(5, 15, 2, colors::DARK_GRAY);
        builder.add_voxel(5, 16, 2, colors::DARK_GRAY);
        builder.add_voxel(10, 15, 2, colors::DARK_GRAY);
        builder.add_voxel(10, 16, 2, colors::DARK_GRAY);
        // Eyes
        builder.add_emissive(5, 12, 0, colors::NEON_PINK, 1.0, 0.3, 0.0, 8.0);
        builder.add_emissive(10, 12, 0, colors::NEON_PINK, 1.0, 0.3, 0.0, 8.0);
        
        // === TAIL ===
        builder.fill_box(6, 3, 24, 9, 6, 28, colors::RED);
        builder.fill_box(7, 3, 28, 8, 5, 31, colors::RED);
        builder.add_voxel(7, 4, 31, colors::RED);
        
        // === LEGS ===
        // Front left
        builder.fill_box(3, 0, 10, 4, 2, 12, colors::RED);
        // Front right
        builder.fill_box(11, 0, 10, 12, 2, 12, colors::RED);
        // Back left
        builder.fill_box(3, 0, 20, 4, 2, 22, colors::RED);
        // Back right
        builder.fill_box(11, 0, 20, 12, 2, 22, colors::RED);
        
        // === WINGS (folded) ===
        // Left wing
        builder.fill_box(0, 6, 10, 4, 10, 20, colors::DARK_GRAY);
        builder.fill_box(0, 10, 12, 2, 12, 18, colors::DARK_GRAY);
        // Right wing
        builder.fill_box(11, 6, 10, 15, 10, 20, colors::DARK_GRAY);
        builder.fill_box(13, 10, 12, 15, 12, 18, colors::DARK_GRAY);
        
        // === FIRE BREATH ORIGIN (for VFX attachment point) ===
        builder.add_emissive(7, 10, 0, colors::ORANGE, 1.0, 0.5, 0.0, 10.0);
        builder.add_emissive(8, 10, 0, colors::ORANGE, 1.0, 0.5, 0.0, 10.0);
        
        // === BELLY SCALES (different color) ===
        builder.fill_box(5, 2, 10, 10, 2, 22, colors::ORANGE);
        
        builder.build()
    }
    
    /// Gets all available procedural models.
    #[must_use]
    pub fn all() -> Vec<VoxelModel> {
        vec![
            Self::player(),
            Self::enemy(),
            Self::sword(),
            Self::shield(),
            Self::crate_box(),
            Self::barrel(),
            Self::tree(),
            Self::crystal(),
            Self::dragon(),
        ]
    }
    
    /// Gets a model by name.
    #[must_use]
    pub fn by_name(name: &str) -> Option<VoxelModel> {
        match name {
            "player" => Some(Self::player()),
            "enemy" => Some(Self::enemy()),
            "sword" => Some(Self::sword()),
            "shield" => Some(Self::shield()),
            "crate" => Some(Self::crate_box()),
            "barrel" => Some(Self::barrel()),
            "tree" => Some(Self::tree()),
            "crystal" => Some(Self::crystal()),
            "dragon" => Some(Self::dragon()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_player_model() {
        let player = ProceduralModels::player();
        assert_eq!(player.name, "player");
        assert!(player.voxel_count() > 0);
        assert!(player.bounds.height > 10); // Player should be tall
    }
    
    #[test]
    fn test_enemy_model() {
        let enemy = ProceduralModels::enemy();
        assert_eq!(enemy.name, "enemy");
        assert!(enemy.voxel_count() > 0);
        // Enemy should have spikes (width > core body)
        assert!(enemy.bounds.width > 8);
    }
    
    #[test]
    fn test_to_instances() {
        let crate_model = ProceduralModels::crate_box();
        let instances = crate_model.to_instances(0.0, 0.0, 0.0);
        // Should have instances for each voxel face
        assert!(!instances.is_empty());
    }
    
    #[test]
    fn test_model_builder() {
        let mut builder = VoxelModelBuilder::new("test");
        builder.fill_box(0, 0, 0, 2, 2, 2, 1);
        let model = builder.build();
        
        assert_eq!(model.bounds.width, 3);
        assert_eq!(model.bounds.height, 3);
        assert_eq!(model.bounds.depth, 3);
        assert_eq!(model.voxel_count(), 27); // 3x3x3
    }
    
    #[test]
    fn test_all_models() {
        let models = ProceduralModels::all();
        assert!(!models.is_empty());
        
        for model in models {
            assert!(model.voxel_count() > 0, "Model {} has no voxels", model.name);
        }
    }
    
    #[test]
    fn test_by_name() {
        assert!(ProceduralModels::by_name("player").is_some());
        assert!(ProceduralModels::by_name("enemy").is_some());
        assert!(ProceduralModels::by_name("nonexistent").is_none());
    }
    
    #[test]
    fn test_dragon_model() {
        let dragon = ProceduralModels::dragon();
        assert_eq!(dragon.name, "dragon");
        // Dragon should be big
        assert!(dragon.bounds.width > 10);
        assert!(dragon.bounds.depth > 20);
    }
}
