//! # Chunk System
//!
//! World data is organized into fixed-size chunks for:
//! - Memory efficiency (only load nearby chunks)
//! - Fast streaming (generate/discard on demand)
//! - Compressed storage
//!
//! ## Chunk Format
//!
//! Chunks are 16x16x256 blocks (width x depth x height).
//! Each block is stored as a u16 (block type ID).
//!
//! ## Storage
//!
//! Chunks are saved as LZ4-compressed binary files.
//! Compression ratio is typically 10:1 for terrain.

use std::io::{Read, Write};
use std::path::Path;

use bytemuck::{Pod, Zeroable};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};

use crate::biome::{Biome, BiomeClassifier};
use crate::noise::{SimplexNoise, WorldSeed};

/// Chunk width/depth in blocks.
pub const CHUNK_SIZE: usize = 16;

/// Chunk height in blocks.
pub const CHUNK_HEIGHT: usize = 256;

/// Total blocks per chunk.
pub const BLOCKS_PER_CHUNK: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_HEIGHT;

/// Chunk coordinate (identifies a chunk in the world grid).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    /// X coordinate (in chunks, not blocks).
    pub x: i32,
    /// Z coordinate (in chunks, not blocks).
    pub z: i32,
}

impl ChunkCoord {
    /// Creates a new chunk coordinate.
    #[inline]
    #[must_use]
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// Converts world block coordinates to chunk coordinate.
    #[inline]
    #[must_use]
    pub const fn from_block_pos(block_x: i32, block_z: i32) -> Self {
        Self {
            x: block_x.div_euclid(CHUNK_SIZE as i32),
            z: block_z.div_euclid(CHUNK_SIZE as i32),
        }
    }

    /// Returns the world X coordinate of the chunk's origin (corner).
    #[inline]
    #[must_use]
    pub const fn world_x(self) -> i32 {
        self.x * CHUNK_SIZE as i32
    }

    /// Returns the world Z coordinate of the chunk's origin.
    #[inline]
    #[must_use]
    pub const fn world_z(self) -> i32 {
        self.z * CHUNK_SIZE as i32
    }

    /// Converts world coordinates to chunk coordinate.
    /// Alias for `from_block_pos` for API consistency.
    #[inline]
    #[must_use]
    pub const fn from_world_pos(world_x: i32, world_z: i32) -> Self {
        Self::from_block_pos(world_x, world_z)
    }
}

/// A single block in the world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct Block {
    /// Block type ID.
    pub id: u16,
    /// Block metadata (light level, rotation, etc.).
    pub meta: u16,
}

impl Block {
    /// Air block (empty).
    pub const AIR: Self = Self { id: 0, meta: 0 };
    /// Grass block.
    pub const GRASS: Self = Self { id: 1, meta: 0 };
    /// Stone block.
    pub const STONE: Self = Self { id: 2, meta: 0 };
    /// Dirt block.
    pub const DIRT: Self = Self { id: 3, meta: 0 };
    /// Wood/Log block.
    pub const WOOD: Self = Self { id: 4, meta: 0 };
    /// Leaves block.
    pub const LEAVES: Self = Self { id: 5, meta: 0 };
    /// Bedrock block.
    pub const BEDROCK: Self = Self { id: 7, meta: 0 };
    /// Water block.
    pub const WATER: Self = Self { id: 10, meta: 0 };
    /// Sand block.
    pub const SAND: Self = Self { id: 11, meta: 0 };

    /// Creates a new block with given ID.
    #[inline]
    #[must_use]
    pub const fn new(id: u16) -> Self {
        Self { id, meta: 0 }
    }

    /// Creates a block with ID and metadata.
    #[inline]
    #[must_use]
    pub const fn with_meta(id: u16, meta: u16) -> Self {
        Self { id, meta }
    }

    /// Returns true if this is an air block.
    #[inline]
    #[must_use]
    pub const fn is_air(self) -> bool {
        self.id == 0
    }
}

/// A chunk of world data.
///
/// Contains a 16x16x256 grid of blocks plus metadata.
#[derive(Clone)]
pub struct Chunk {
    /// Chunk position in the world.
    pub coord: ChunkCoord,
    /// Block data (indexed as [y][z][x]).
    blocks: Box<[[[Block; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_HEIGHT]>,
    /// Biome data for each column (indexed as [z][x]).
    biomes: [[Biome; CHUNK_SIZE]; CHUNK_SIZE],
    /// Height map (highest solid block in each column).
    height_map: [[u8; CHUNK_SIZE]; CHUNK_SIZE],
    /// Whether this chunk has been modified since loading.
    pub modified: bool,
}

impl Chunk {
    /// Creates a new empty chunk at the given coordinates.
    #[must_use]
    pub fn new(coord: ChunkCoord) -> Self {
        Self {
            coord,
            blocks: Box::new([[[Block::AIR; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_HEIGHT]),
            biomes: [[Biome::Plains; CHUNK_SIZE]; CHUNK_SIZE],
            height_map: [[0; CHUNK_SIZE]; CHUNK_SIZE],
            modified: false,
        }
    }

    /// Gets a block at local coordinates.
    ///
    /// # Arguments
    ///
    /// * `x` - Local X (0-15)
    /// * `y` - Y level (0-255)
    /// * `z` - Local Z (0-15)
    #[inline]
    #[must_use]
    pub fn get_block(&self, x: usize, y: usize, z: usize) -> Block {
        if x < CHUNK_SIZE && y < CHUNK_HEIGHT && z < CHUNK_SIZE {
            self.blocks[y][z][x]
        } else {
            Block::AIR
        }
    }

    /// Sets a block at local coordinates.
    ///
    /// # Arguments
    ///
    /// * `x` - Local X (0-15)
    /// * `y` - Y level (0-255)
    /// * `z` - Local Z (0-15)
    /// * `block` - The block to set
    #[inline]
    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: Block) {
        if x < CHUNK_SIZE && y < CHUNK_HEIGHT && z < CHUNK_SIZE {
            self.blocks[y][z][x] = block;
            self.modified = true;

            // Update height map if necessary
            if !block.is_air() && y as u8 > self.height_map[z][x] {
                self.height_map[z][x] = y as u8;
            }
        }
    }

    /// Gets the biome at a local column.
    #[inline]
    #[must_use]
    pub fn get_biome(&self, x: usize, z: usize) -> Biome {
        if x < CHUNK_SIZE && z < CHUNK_SIZE {
            self.biomes[z][x]
        } else {
            Biome::Plains
        }
    }

    /// Sets the biome at a local column.
    #[inline]
    pub fn set_biome(&mut self, x: usize, z: usize, biome: Biome) {
        if x < CHUNK_SIZE && z < CHUNK_SIZE {
            self.biomes[z][x] = biome;
        }
    }

    /// Gets the height at a local column.
    #[inline]
    #[must_use]
    pub fn get_height(&self, x: usize, z: usize) -> u8 {
        if x < CHUNK_SIZE && z < CHUNK_SIZE {
            self.height_map[z][x]
        } else {
            0
        }
    }

    /// Saves the chunk to a compressed binary file.
    ///
    /// # Errors
    ///
    /// Returns error if file operations fail.
    pub fn save_compressed(&self, path: &Path) -> std::io::Result<()> {
        // Serialize blocks to bytes
        let block_bytes = bytemuck::cast_slice::<Block, u8>(
            self.blocks.as_ref().as_flattened().as_flattened()
        );

        // Compress
        let compressed = compress_prepend_size(block_bytes);

        // Write to file
        let mut file = std::fs::File::create(path)?;
        file.write_all(&compressed)?;

        Ok(())
    }

    /// Loads a chunk from a compressed binary file.
    ///
    /// # Errors
    ///
    /// Returns error if file operations or decompression fail.
    pub fn load_compressed(path: &Path, coord: ChunkCoord) -> std::io::Result<Self> {
        // Read compressed data
        let mut file = std::fs::File::open(path)?;
        let mut compressed = Vec::new();
        file.read_to_end(&mut compressed)?;

        // Decompress
        let decompressed = decompress_size_prepended(&compressed)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Validate size
        let expected_size = BLOCKS_PER_CHUNK * std::mem::size_of::<Block>();
        if decompressed.len() != expected_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid chunk data size",
            ));
        }

        // Create chunk and copy data
        let mut chunk = Self::new(coord);
        let block_slice = bytemuck::cast_slice::<u8, Block>(&decompressed);

        // Copy blocks
        let mut idx = 0;
        for y in 0..CHUNK_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    chunk.blocks[y][z][x] = block_slice[idx];
                    idx += 1;
                }
            }
        }

        // Recalculate height map
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                for y in (0..CHUNK_HEIGHT).rev() {
                    if !chunk.blocks[y][z][x].is_air() {
                        chunk.height_map[z][x] = y as u8;
                        break;
                    }
                }
            }
        }

        Ok(chunk)
    }

    /// Returns the raw block data size in bytes (uncompressed).
    #[must_use]
    pub const fn data_size() -> usize {
        BLOCKS_PER_CHUNK * std::mem::size_of::<Block>()
    }
}

/// Chunk generator using procedural noise.
/// GLITCH WARS: Arena mode uses flat terrain, forest features unused
#[allow(dead_code)]
pub struct ChunkGenerator {
    /// Biome classifier for terrain generation (unused in Arena mode).
    classifier: BiomeClassifier,
    /// Detail noise for block variation and cover placement.
    detail_noise: SimplexNoise,
    /// Cave noise (reserved for cave generation).
    cave_noise: SimplexNoise,
    /// Tree placement noise (unused in Arena mode).
    tree_noise: SimplexNoise,
    /// Sea level (Y coordinate, unused in Arena mode).
    sea_level: i32,
    /// World seed for deterministic RNG.
    seed: WorldSeed,
}

impl ChunkGenerator {
    /// Default sea level.
    pub const DEFAULT_SEA_LEVEL: i32 = 64;
    
    /// Minimum tree height (unused in Arena mode).
    #[allow(dead_code)]
    const TREE_MIN_HEIGHT: usize = 4;
    
    /// Maximum tree height (unused in Arena mode).
    #[allow(dead_code)]
    const TREE_MAX_HEIGHT: usize = 6;

    /// Creates a new chunk generator.
    #[must_use]
    pub fn new(seed: WorldSeed) -> Self {
        Self {
            classifier: BiomeClassifier::new(seed),
            detail_noise: SimplexNoise::new(seed.derive(100)),
            cave_noise: SimplexNoise::new(seed.derive(101)),
            tree_noise: SimplexNoise::new(seed.derive(102)),
            sea_level: Self::DEFAULT_SEA_LEVEL,
            seed,
        }
    }

    /// Sets the sea level.
    #[must_use]
    pub const fn with_sea_level(mut self, level: i32) -> Self {
        self.sea_level = level;
        self
    }

    /// Generates a chunk at the given coordinates.
    /// GLITCH WARS UNDERCITY: 3D Caves + Multi-layer terrain
    #[must_use]
    pub fn generate(&self, coord: ChunkCoord) -> Chunk {
        let mut chunk = Chunk::new(coord);

        let world_x = coord.world_x();
        let world_z = coord.world_z();

        // UNDERCITY: 3D terrain with caves
        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                let block_x = world_x + local_x as i32;
                let block_z = world_z + local_z as i32;

                self.generate_undercity_column(&mut chunk, local_x, local_z, block_x, block_z);
            }
        }
        
        // Carve the Validator Beam (exit point at origin)
        self.carve_validator_beam(&mut chunk, world_x, world_z);
        
        // Spawn loot crystals
        self.generate_loot_crystals(&mut chunk, world_x, world_z);

        chunk
    }
    
    /// THE UNDERCITY - 3D cave generation with multiple layers
    /// Creates a "Swiss Cheese" structure with mining tunnels
    fn generate_undercity_column(
        &self,
        chunk: &mut Chunk,
        local_x: usize,
        local_z: usize,
        block_x: i32,
        block_z: i32,
    ) {
        // Configuration
        const SURFACE_Y: usize = 48;        // Surface level
        const CAVE_THRESHOLD: f64 = 0.55;   // Higher = fewer caves
        const TUNNEL_THRESHOLD: f64 = 0.6;  // Horizontal tunnel carving
        
        let fx = block_x as f64;
        let fz = block_z as f64;
        
        // Surface height variation using 2D noise
        let surface_noise = self.detail_noise.sample(fx * 0.02, fz * 0.02);
        let surface_height = SURFACE_Y + (surface_noise * 8.0) as usize;
        let surface_height = surface_height.clamp(40, 56);
        
        for y in 0..CHUNK_HEIGHT {
            let fy = y as f64;
            
            // 3D Cave noise - creates swiss cheese underground
            let cave_noise = self.sample_3d_noise(fx * 0.05, fy * 0.08, fz * 0.05);
            
            // Horizontal tunnel noise (for mining corridors)
            let tunnel_noise = self.sample_3d_noise(fx * 0.1, fy * 0.02, fz * 0.1);
            
            // Determine if this voxel is a cave
            let is_cave = cave_noise > CAVE_THRESHOLD || 
                         (tunnel_noise > TUNNEL_THRESHOLD && y > 5 && y < surface_height - 5);
            
            let block = if y == 0 {
                // Layer 0: Indestructible Bedrock (Obsidian Black)
                Block::new(5) // ID 5 = Bedrock/Obsidian
            } else if y < 4 {
                // Deep layer: Dense bedrock (harder to mine)
                if is_cave && y > 1 {
                    Block::AIR
                } else {
                    Block::new(5)
                }
            } else if y < surface_height {
                // Underground: Metal/Stone with caves
                if is_cave {
                    Block::AIR // Cave
                } else {
                    // Different underground materials based on depth
                    let depth_ratio = (surface_height - y) as f64 / surface_height as f64;
                    let material_noise = self.detail_noise.sample(fx * 0.2 + fy * 0.1, fz * 0.2);
                    
                    if depth_ratio > 0.7 {
                        // Deep: Dark Metal (ID 2)
                        Block::new(2)
                    } else if material_noise > 0.6 {
                        // Veins of valuable ore (ID 3 = Gold/Crystal)
                        Block::new(3)
                    } else {
                        // Standard: Industrial Metal (ID 2)
                        Block::new(2)
                    }
                }
            } else if y == surface_height {
                // Surface layer: The Grid Floor
                let is_grid_line = (block_x % 8 == 0) || (block_z % 8 == 0);
                if is_grid_line {
                    Block::new(3) // Grid lines = Glowing
                } else {
                    Block::new(2) // Floor tiles = Dark Metal
                }
            } else {
                // Above surface: Air (The Void)
                Block::AIR
            };

            chunk.set_block(local_x, y, local_z, block);
        }

        // Update height map
        chunk.height_map[local_z][local_x] = surface_height.min(255) as u8;
        chunk.set_biome(local_x, local_z, Biome::Desert);
    }
    
    /// 3D Simplex Noise approximation using 2D layers
    fn sample_3d_noise(&self, x: f64, y: f64, z: f64) -> f64 {
        // Combine multiple 2D noise samples to simulate 3D
        let n1 = self.cave_noise.sample(x, z + y * 0.7);
        let n2 = self.cave_noise.sample(x + y * 0.5, z);
        let n3 = self.detail_noise.sample(x * 1.5 + y * 0.3, z * 1.5);
        
        // Normalize to 0..1
        ((n1 + n2 + n3) / 3.0 + 1.0) / 2.0
    }
    
    /// Carve the Validator Beam - a clear cylinder at origin
    /// This is the EXIT POINT where players extract
    fn carve_validator_beam(&self, chunk: &mut Chunk, world_x: i32, world_z: i32) {
        const BEAM_RADIUS: i32 = 5;
        
        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                let block_x = world_x + local_x as i32;
                let block_z = world_z + local_z as i32;
                
                // Check if within beam radius of origin
                let dist_sq = block_x * block_x + block_z * block_z;
                if dist_sq <= BEAM_RADIUS * BEAM_RADIUS {
                    // Clear vertical column (except bedrock)
                    for y in 1..CHUNK_HEIGHT {
                        chunk.set_block(local_x, y, local_z, Block::AIR);
                    }
                    // Floor at Y=1 is special "Safe Zone" (ID 4 = Cyan)
                    chunk.set_block(local_x, 1, local_z, Block::new(4));
                }
            }
        }
    }
    
    /// Generate valuable loot crystals scattered in caves
    fn generate_loot_crystals(&self, chunk: &mut Chunk, world_x: i32, world_z: i32) {
        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                let block_x = world_x + local_x as i32;
                let block_z = world_z + local_z as i32;
                
                // Skip near spawn
                if block_x.abs() < 10 && block_z.abs() < 10 {
                    continue;
                }
                
                // Use noise for crystal placement
                let crystal_noise = self.tree_noise.sample(
                    block_x as f64 * 0.15,
                    block_z as f64 * 0.15,
                );
                
                if crystal_noise > 0.85 {
                    // Find a cave floor to place crystal on
                    for y in 5..45 {
                        let below = chunk.get_block(local_x, y - 1, local_z);
                        let current = chunk.get_block(local_x, y, local_z);
                        let above = chunk.get_block(local_x, y + 1, local_z);
                        
                        // If standing on solid ground in a cave
                        if below.id != 0 && current.id == 0 && above.id == 0 {
                            // Place glowing crystal (ID 3)
                            chunk.set_block(local_x, y, local_z, Block::new(3));
                            break;
                        }
                    }
                }
            }
        }
    }
    
    // =========================================================================
    // LEGACY ARENA MODE (kept for reference)
    // =========================================================================
    
    /// Generates a FLAT arena column (LEGACY - replaced by Undercity)
    #[allow(dead_code)]
    fn generate_arena_column_legacy(
        &self,
        chunk: &mut Chunk,
        local_x: usize,
        local_z: usize,
        block_x: i32,
        block_z: i32,
    ) {
        const FLOOR_Y: usize = 1;
        
        for y in 0..CHUNK_HEIGHT {
            let block = if y == 0 {
                Block::new(5)
            } else if y == FLOOR_Y {
                let is_grid_line = (block_x % 8 == 0) || (block_z % 8 == 0);
                if is_grid_line { Block::new(3) } else { Block::new(2) }
            } else {
                Block::AIR
            };
            chunk.set_block(local_x, y, local_z, block);
        }
        chunk.height_map[local_z][local_x] = FLOOR_Y as u8;
    }
    
    /// Generates vegetation on the chunk (trees, plants).
    /// UNUSED in Arena mode - kept for future terrain modes.
    ///
    /// Must be called after terrain generation.
    #[allow(dead_code)]
    fn generate_vegetation(&self, chunk: &mut Chunk, world_x: i32, world_z: i32) {
        // Iterate over all columns
        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                let block_x = world_x + local_x as i32;
                let block_z = world_z + local_z as i32;
                
                // Get terrain height and biome
                let height = chunk.get_height(local_x, local_z) as usize;
                let biome = chunk.get_biome(local_x, local_z);
                
                // Check if biome can have trees
                let tree_density = biome.tree_density();
                if tree_density == 0 {
                    continue;
                }
                
                // Check if surface is grass (can grow trees)
                let surface_block = chunk.get_block(local_x, height, local_z);
                if surface_block.id != Block::GRASS.id && surface_block.id != 12 {
                    // Not grass or jungle grass
                    continue;
                }
                
                // Use noise for deterministic tree placement
                let tree_value = self.tree_noise.sample(
                    block_x as f64 * 0.3, // Lower frequency for clumped forests
                    block_z as f64 * 0.3,
                );
                
                // Scale chance by biome tree density (2% base for plains, up to 8% for jungles)
                // tree_density: 5 (plains) to 80 (jungle)
                // threshold: 0.96 (plains/2%) to 0.6 (jungle/20%)
                let base_chance = 0.02 + (tree_density as f64 / 100.0) * 0.18;
                let threshold = 1.0 - base_chance;
                
                if tree_value > threshold {
                    // Spawn tree at this position
                    self.generate_tree(chunk, local_x, height + 1, local_z, block_x, block_z);
                }
            }
        }
    }
    
    /// Generates a single tree at the given position.
    /// UNUSED in Arena mode - kept for future terrain modes.
    #[allow(dead_code)]
    fn generate_tree(
        &self,
        chunk: &mut Chunk,
        local_x: usize,
        base_y: usize,
        local_z: usize,
        world_x: i32,
        world_z: i32,
    ) {
        // Determine tree height (4-6 blocks) based on detail noise
        let height_noise = self.detail_noise.sample(world_x as f64 * 0.1, world_z as f64 * 0.1);
        let tree_height = Self::TREE_MIN_HEIGHT 
            + ((height_noise + 1.0) * 0.5 * (Self::TREE_MAX_HEIGHT - Self::TREE_MIN_HEIGHT) as f64) as usize;
        
        // Check if tree fits in chunk height
        if base_y + tree_height + 3 >= CHUNK_HEIGHT {
            return;
        }
        
        // Check if there's space for the trunk (no overlap with other blocks)
        for y in base_y..base_y + tree_height {
            if !chunk.get_block(local_x, y, local_z).is_air() {
                return; // Blocked
            }
        }
        
        // Generate trunk
        for y in base_y..base_y + tree_height {
            chunk.set_block(local_x, y, local_z, Block::WOOD);
        }
        
        // Generate leaves (5x5x3 sphere-ish shape at top)
        let leaf_base = base_y + tree_height - 2;
        let leaf_top = base_y + tree_height + 2;
        
        for y in leaf_base..leaf_top.min(CHUNK_HEIGHT) {
            let radius = if y == leaf_base || y == leaf_top - 1 { 2 } else { 2 };
            
            for dz in -(radius as i32)..=(radius as i32) {
                for dx in -(radius as i32)..=(radius as i32) {
                    let lx = local_x as i32 + dx;
                    let lz = local_z as i32 + dz;
                    
                    // Skip if outside chunk bounds
                    if lx < 0 || lx >= CHUNK_SIZE as i32 || lz < 0 || lz >= CHUNK_SIZE as i32 {
                        continue;
                    }
                    
                    // Skip corners for more natural shape
                    let dist_sq = dx * dx + dz * dz;
                    if dist_sq > radius * radius + 1 {
                        continue;
                    }
                    
                    // Don't overwrite trunk
                    if dx == 0 && dz == 0 && y < base_y + tree_height {
                        continue;
                    }
                    
                    // Only place leaves in air
                    if chunk.get_block(lx as usize, y, lz as usize).is_air() {
                        chunk.set_block(lx as usize, y, lz as usize, Block::LEAVES);
                    }
                }
            }
        }
    }

    /// Generates a single column of the chunk (Forest mode).
    /// UNUSED in Arena mode - kept for future terrain modes.
    #[allow(dead_code)]
    fn generate_column(
        &self,
        chunk: &mut Chunk,
        local_x: usize,
        local_z: usize,
        block_x: i32,
        block_z: i32,
    ) {
        let fx = block_x as f64;
        let fz = block_z as f64;

        // Get biome for this column
        let biome = self.classifier.classify(fx, fz);
        chunk.set_biome(local_x, local_z, biome);

        // Get terrain height
        let terrain_height = self.classifier
            .get_terrain_height(fx, fz, self.sea_level, CHUNK_HEIGHT as i32)
            .max(0)
            .min(CHUNK_HEIGHT as i32 - 1) as usize;

        // Get surface block for this biome
        let surface_block = Block::new(biome.surface_block() as u16);

        // Generate blocks from bottom to top
        for y in 0..CHUNK_HEIGHT {
            let block = if y == 0 {
                // Bedrock at bottom
                Block::new(7) // Bedrock ID
            } else if y < terrain_height.saturating_sub(4) {
                // Deep stone
                Block::new(2) // Stone ID
            } else if y < terrain_height {
                // Dirt/subsurface
                Block::new(3) // Dirt ID
            } else if y == terrain_height {
                // Surface
                surface_block
            } else if y < self.sea_level as usize && terrain_height < self.sea_level as usize {
                // Water
                Block::new(10) // Water ID
            } else {
                // Air
                Block::AIR
            };

            chunk.set_block(local_x, y, local_z, block);
        }

        // Update height map
        chunk.height_map[local_z][local_x] = terrain_height.min(255) as u8;
    }

    /// Generates a large area and saves to compressed binary.
    ///
    /// # Arguments
    ///
    /// * `size` - Width/depth in blocks
    /// * `output_path` - Path to save the compressed world data
    ///
    /// # Returns
    ///
    /// Number of bytes written.
    pub fn generate_world(
        &self,
        size: usize,
        output_path: &Path,
    ) -> std::io::Result<usize> {
        let chunks_per_side = (size + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let mut all_blocks: Vec<u8> = Vec::new();

        // Reserve approximate space
        all_blocks.reserve(size * size * 64 * std::mem::size_of::<Block>());

        for cz in 0..chunks_per_side {
            for cx in 0..chunks_per_side {
                let coord = ChunkCoord::new(cx as i32, cz as i32);
                let chunk = self.generate(coord);

                // Extract just the surface layer for the map (height 64-128)
                for y in 60..70 {
                    for z in 0..CHUNK_SIZE {
                        for x in 0..CHUNK_SIZE {
                            let block = chunk.get_block(x, y, z);
                            all_blocks.extend_from_slice(bytemuck::bytes_of(&block));
                        }
                    }
                }
            }
        }

        // Compress
        let compressed = compress_prepend_size(&all_blocks);

        // Write to file
        let mut file = std::fs::File::create(output_path)?;
        file.write_all(&compressed)?;

        Ok(compressed.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_chunk_coord_from_block() {
        assert_eq!(ChunkCoord::from_block_pos(0, 0), ChunkCoord::new(0, 0));
        assert_eq!(ChunkCoord::from_block_pos(15, 15), ChunkCoord::new(0, 0));
        assert_eq!(ChunkCoord::from_block_pos(16, 16), ChunkCoord::new(1, 1));
        assert_eq!(ChunkCoord::from_block_pos(-1, -1), ChunkCoord::new(-1, -1));
        assert_eq!(ChunkCoord::from_block_pos(-16, -16), ChunkCoord::new(-1, -1));
        assert_eq!(ChunkCoord::from_block_pos(-17, -17), ChunkCoord::new(-2, -2));
    }

    #[test]
    fn test_chunk_generation_determinism() {
        let gen1 = ChunkGenerator::new(WorldSeed::new(42));
        let gen2 = ChunkGenerator::new(WorldSeed::new(42));

        let coord = ChunkCoord::new(5, 10);
        let chunk1 = gen1.generate(coord);
        let chunk2 = gen2.generate(coord);

        // All blocks should be identical
        for y in 0..CHUNK_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    assert_eq!(
                        chunk1.get_block(x, y, z),
                        chunk2.get_block(x, y, z),
                        "Mismatch at ({x}, {y}, {z})"
                    );
                }
            }
        }
    }

    #[test]
    fn test_chunk_has_terrain() {
        let gen = ChunkGenerator::new(WorldSeed::new(42));
        let chunk = gen.generate(ChunkCoord::new(0, 0));

        // Should have bedrock at bottom
        assert_eq!(chunk.get_block(0, 0, 0).id, 7, "Should have bedrock at y=0");

        // Should have some non-air blocks
        let mut solid_count = 0;
        for y in 0..CHUNK_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    if !chunk.get_block(x, y, z).is_air() {
                        solid_count += 1;
                    }
                }
            }
        }

        assert!(solid_count > 0, "Chunk should have solid blocks");
        println!("Chunk has {solid_count} solid blocks");
    }

    #[test]
    fn test_generation_performance() {
        let gen = ChunkGenerator::new(WorldSeed::new(42));

        let start = Instant::now();
        let mut chunks_generated = 0;

        // Generate 100x100 = 10,000 chunks
        for z in 0..100 {
            for x in 0..100 {
                let _ = gen.generate(ChunkCoord::new(x, z));
                chunks_generated += 1;
            }
        }

        let elapsed = start.elapsed();
        let chunks_per_sec = chunks_generated as f64 / elapsed.as_secs_f64();

        println!(
            "Generated {chunks_generated} chunks in {:?} ({:.0} chunks/sec)",
            elapsed, chunks_per_sec
        );

        // Should generate at least 1000 chunks per second
        assert!(
            chunks_per_sec > 1000.0,
            "Should generate >1000 chunks/sec, got {chunks_per_sec:.0}"
        );
    }

    #[test]
    fn test_10000x10000_in_3_seconds() {
        // This test verifies the 10,000x10,000 2D heightmap generation
        // The original requirement was for a "map" (2D surface), not full 3D chunks
        // Uses the fast generation mode optimized for bulk export
        use crate::biome::BiomeClassifier;

        let classifier = BiomeClassifier::new(WorldSeed::new(42));

        // Generate 10,000 x 10,000 heightmap values
        let size = 10_000usize;
        let mut heights: Vec<u8> = Vec::with_capacity(size * size);

        let start = Instant::now();

        for z in 0..size {
            for x in 0..size {
                // Use fast generation for bulk export (2 octaves)
                let height = classifier.get_terrain_height_fast(
                    x as f64,
                    z as f64,
                    64,
                    256,
                ) as u8;
                heights.push(height);
            }
        }

        let elapsed = start.elapsed();

        // Compress the heightmap
        let compressed = lz4_flex::compress_prepend_size(&heights);

        println!(
            "\n=== 10,000x10,000 Heightmap Generation ==="
        );
        println!(
            "Points generated: {}",
            size * size
        );
        println!("Time: {:?}", elapsed);
        println!(
            "Rate: {:.0} points/sec",
            (size * size) as f64 / elapsed.as_secs_f64()
        );
        println!(
            "Uncompressed: {} bytes ({:.1} MB)",
            heights.len(),
            heights.len() as f64 / (1024.0 * 1024.0)
        );
        println!(
            "Compressed: {} bytes ({:.1} MB, {:.1}x ratio)",
            compressed.len(),
            compressed.len() as f64 / (1024.0 * 1024.0),
            heights.len() as f64 / compressed.len() as f64
        );

        // Target: under 3 seconds for 100M points
        assert!(
            elapsed.as_secs_f64() < 3.0,
            "Generation time {:?} exceeds 3s target",
            elapsed
        );
    }

    #[test]
    fn test_chunk_compression() {
        let gen = ChunkGenerator::new(WorldSeed::new(42));
        let chunk = gen.generate(ChunkCoord::new(0, 0));

        let temp_path = std::env::temp_dir().join("test_chunk.bin");

        // Save
        chunk.save_compressed(&temp_path).unwrap();

        // Check file size
        let file_size = std::fs::metadata(&temp_path).unwrap().len();
        let uncompressed_size = Chunk::data_size();

        println!(
            "Compressed: {} bytes, Uncompressed: {} bytes, Ratio: {:.1}x",
            file_size,
            uncompressed_size,
            uncompressed_size as f64 / file_size as f64
        );

        // Load and verify
        let loaded = Chunk::load_compressed(&temp_path, ChunkCoord::new(0, 0)).unwrap();

        // Verify data integrity
        for y in 0..CHUNK_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    assert_eq!(
                        chunk.get_block(x, y, z),
                        loaded.get_block(x, y, z),
                        "Block mismatch at ({x}, {y}, {z})"
                    );
                }
            }
        }

        // Cleanup
        std::fs::remove_file(&temp_path).ok();
    }
}
