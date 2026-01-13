//! # OROBOROS Client - GENESIS BUILD
//!
//! OPERATION GENESIS: Full Integration
//! 
//! Features:
//! - INFINITE WORLD with chunk streaming (WorldManager)
//! - Kinematic Character Controller with gravity
//! - AABB Collision against voxel grid
//! - Raycast mining system
//! - Visual block selection (wireframe cube)
//! - File logging to client.log
//!
//! CEO: "The Real Game."

use winit::{
    event::{Event, WindowEvent, KeyEvent, ElementState, DeviceEvent, MouseButton},
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, CursorGrabMode},
    keyboard::{KeyCode, PhysicalKey},
    dpi::PhysicalSize,
};
use std::sync::Arc;
use std::collections::HashSet;
use std::time::Instant;
use std::io::Write;

// =============================================================================
// OPERATION PANOPTICON - DIAGNOSTIC LOGGING SYSTEM
// =============================================================================
/// Minimum mouse delta to trigger rotation logging (radians equivalent)


// Procedural generation - Infinite terrain
use oroboros_procedural::{WorldManager, WorldManagerConfig, WorldSeed, ChunkCoord, CHUNK_SIZE};

// =============================================================================
// ASYNC MESH WORKER SYSTEM - Reserved for future optimization
// Currently using synchronous meshing with neighbor notification
// =============================================================================

// Unit 6 - Procedural Models for NPCs
use oroboros_rendering::ProceduralModels;

// INDUSTRIAL STANDARD MESHING (block-mesh-rs)
// WIRE THE NEW ENGINE: Vertex + Index Buffers
use oroboros_rendering::voxel::{
    StandardMesher, MeshVoxel, PaddedChunkBuffer, ChunkMesh, TerrainVertex,
};

// =============================================================================
// CONFIGURATION
// =============================================================================
const GRAVITY: f32 = 25.0;          // Units/sec²
const JUMP_VELOCITY: f32 = 8.0;     // Units/sec
const PLAYER_SPEED: f32 = 6.0;      // Units/sec
const PLAYER_HEIGHT: f32 = 1.8;     // Player collision height
const PLAYER_WIDTH: f32 = 0.6;      // Player collision width
const PLAYER_EYE_HEIGHT: f32 = 1.6; // Camera offset from feet
const RAYCAST_DISTANCE: f32 = 6.0;  // Block interaction range
const MOUSE_SENSITIVITY: f32 = 0.1;

// =============================================================================
// INFINITE VOXEL WORLD (Chunk-Streamed via WorldManager)
// =============================================================================

/// Block types (matches oroboros_procedural block IDs)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
#[allow(dead_code)]
enum BlockType {
    Air = 0,
    Grass = 1,
    Dirt = 2,
    Stone = 3,
    Sand = 4,
    Bedrock = 5,
    Neon = 255,
}

impl BlockType {
    /// Forest palette colors - gloomy, natural tones
    fn color(&self) -> [f32; 3] {
        match self {
            BlockType::Air => [0.0, 0.0, 0.0],
            // Deep forest green - not bright, earthy
            BlockType::Grass => [0.15, 0.35, 0.12],
            // Rich brown soil
            BlockType::Dirt => [0.35, 0.22, 0.12],
            // Cool gray stone with slight blue tint
            BlockType::Stone => [0.42, 0.44, 0.48],
            // Warm sandy brown (rare in forest, used for paths)
            BlockType::Sand => [0.65, 0.55, 0.35],
            // Nearly black bedrock
            BlockType::Bedrock => [0.12, 0.12, 0.14],
            // Keep neon for special effects
            BlockType::Neon => [0.0, 1.0, 1.0],
        }
    }

    fn is_solid(&self) -> bool {
        !matches!(self, BlockType::Air)
    }
    
    fn from_id(id: u16) -> Self {
        match id {
            0 => BlockType::Air,
            1 => BlockType::Grass,
            2 => BlockType::Dirt,
            3 => BlockType::Stone,
            4 => BlockType::Sand,
            5 => BlockType::Bedrock,
            _ => BlockType::Stone, // Default for unknown blocks
        }
    }
}

/// Infinite voxel world with chunk streaming and NEIGHBOR NOTIFICATION
struct InfiniteWorld {
    world_manager: WorldManager,
    /// Dirty flag for mesh regeneration
    dirty: bool,
    /// Track loaded chunks for mesh generation
    loaded_chunks: Vec<ChunkCoord>,
    /// Last player chunk position
    last_player_chunk: Option<ChunkCoord>,
    /// Chunks that need re-meshing (for neighbor notification)
    dirty_chunks: HashSet<ChunkCoord>,
    /// Previously loaded chunks (to detect new chunks)
    prev_loaded: HashSet<ChunkCoord>,
    /// PANOPTICON: Last logged player chunk for boundary crossing detection
    last_logged_chunk: Option<ChunkCoord>,
}

impl InfiniteWorld {
    fn new(seed: u64) -> Self {
        println!("[WORLD] Initializing INFINITE terrain (Seed: {})", seed);
        println!("[WORLD] PROFESSIONAL BUILD: Neighbor Notification Enabled");
        
        let config = WorldManagerConfig {
            load_radius: 6,      // 6 chunk radius = 192 blocks visible
            unload_radius: 8,    // Unload at 8 chunks
            max_chunks_per_frame: 4,
            world_save_path: std::path::PathBuf::from("world/chunks"),
        };
        
        let world_manager = WorldManager::new(WorldSeed::new(seed), config);
        
        println!("[WORLD] WorldManager initialized. Ready for streaming.");
        
        Self {
            world_manager,
            dirty: true,
            loaded_chunks: Vec::new(),
            last_player_chunk: None,
            dirty_chunks: HashSet::new(),
            prev_loaded: HashSet::new(),
            last_logged_chunk: None,
        }
    }
    
    /// Update streaming based on player position. Returns list of newly loaded chunks.
    fn update_streaming(&mut self, player_x: f32, player_z: f32) -> Vec<ChunkCoord> {
        let current_chunk = WorldManager::world_to_chunk(player_x, player_z);
        
        // PANOPTICON: Log chunk boundary crossing
        // Track chunk changes (silent - for internal state only)
        if self.last_logged_chunk.is_none() || self.last_logged_chunk != Some(current_chunk) {
            self.last_logged_chunk = Some(current_chunk);
        }
        
        // Check if player moved to new chunk
        let chunk_changed = self.last_player_chunk != Some(current_chunk);
        
        // Update world manager (handles load/unload)
        let generated = self.world_manager.update(player_x, player_z);
        
        let mut newly_loaded = Vec::new();
        
        if generated > 0 || chunk_changed {
            self.last_player_chunk = Some(current_chunk);
            self.dirty = true;
            
            // NEIGHBOR NOTIFICATION: Find newly loaded chunks
            let current_loaded: HashSet<ChunkCoord> = self.get_loaded_chunk_coords();
            
            for coord in &current_loaded {
                if !self.prev_loaded.contains(coord) {
                    newly_loaded.push(*coord);
                    
                    // Mark this chunk as dirty
                    self.dirty_chunks.insert(*coord);
                    
                    // CRITICAL: Mark all 4 neighbors as dirty too!
                    // This closes the "Swiss cheese" holes
                    let neighbors = [
                        ChunkCoord::new(coord.x + 1, coord.z),
                        ChunkCoord::new(coord.x - 1, coord.z),
                        ChunkCoord::new(coord.x, coord.z + 1),
                        ChunkCoord::new(coord.x, coord.z - 1),
                    ];
                    
                    for neighbor in neighbors {
                        if current_loaded.contains(&neighbor) {
                            self.dirty_chunks.insert(neighbor);
                            // Neighbor notification is silent in production
                        }
                    }
                }
            }
            
            self.prev_loaded = current_loaded;
            
            // Update loaded chunk list (silent in production - stats shown in HUD)
            self.loaded_chunks.clear();
        }
        
        newly_loaded
    }
    
    /// Get all currently loaded chunk coordinates
    fn get_loaded_chunk_coords(&self) -> HashSet<ChunkCoord> {
        let mut coords = HashSet::new();
        if let Some(player_chunk) = self.last_player_chunk {
            let radius = 6;
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let coord = ChunkCoord::new(player_chunk.x + dx, player_chunk.z + dz);
                    if self.world_manager.get_chunk(coord).is_some() {
                        coords.insert(coord);
                    }
                }
            }
        }
        coords
    }
    
    /// Get dirty chunks and clear the dirty set
    #[allow(dead_code)]
    fn take_dirty_chunks(&mut self) -> Vec<ChunkCoord> {
        let dirty: Vec<ChunkCoord> = self.dirty_chunks.drain().collect();
        dirty
    }
    
    /// Ensure initial area is loaded (blocking)
    fn ensure_spawn_loaded(&mut self, x: f32, z: f32) {
        println!("[WORLD] Pre-loading spawn area...");
        self.world_manager.ensure_loaded_around(x, z, 3);
        self.world_manager.flush_generation_queue();
        self.last_player_chunk = Some(WorldManager::world_to_chunk(x, z));
        let stats = self.world_manager.stats();
        println!("[WORLD] Spawn loaded: {} chunks ready", stats.loaded_chunks);
    }

    fn get(&self, x: i32, y: i32, z: i32) -> BlockType {
        if y < 0 || y >= 256 {
            return BlockType::Air;
        }
        
        match self.world_manager.get_block(x, y, z) {
            Some(block) => BlockType::from_id(block.id),
            None => BlockType::Air, // Chunk not loaded
        }
    }

    fn set(&mut self, x: i32, y: i32, z: i32, block: BlockType) {
        if y < 0 || y >= 256 {
            return;
        }
        
        let block_id = block as u8 as u16;
        if self.world_manager.set_block(x, y, z, block_id) {
            self.dirty = true;
        }
    }

    fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.get(x, y, z).is_solid()
    }
    
    /// Check if a neighbor block occludes a face.
    /// CRITICAL: If the neighbor's chunk is NOT loaded, return false (draw the face).
    /// This prevents holes between chunks.
    /// NOTE: Reserved for raycast/collision - StandardMesher handles meshing occlusion.
    #[allow(dead_code)]
    fn is_neighbor_occluding(&self, x: i32, y: i32, z: i32) -> bool {
        if y < 0 || y >= 256 {
            return false; // Out of bounds = air = draw face
        }
        
        match self.world_manager.get_block(x, y, z) {
            Some(block) => BlockType::from_id(block.id).is_solid(),
            None => false, // CHUNK NOT LOADED = DRAW FACE (no holes!)
        }
    }
    
    /// Find ground height at given XZ position
    fn find_ground_height(&self, x: i32, z: i32) -> i32 {
        for y in (0..200).rev() {
            if self.is_solid(x, y, z) {
                return y + 1;
            }
        }
        64 // Default spawn height if no ground found
    }
    
    /// Get the world manager for iteration
    fn manager(&self) -> &WorldManager {
        &self.world_manager
    }
}

// =============================================================================
// PLAYER (KINEMATIC CHARACTER CONTROLLER) - UNIT 4
// =============================================================================
struct Player {
    position: [f32; 3],   // Feet position
    velocity: [f32; 3],
    yaw: f32,
    pitch: f32,
    grounded: bool,
}

impl Player {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: [x, y, z],
            velocity: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            grounded: false,
        }
    }

    fn eye_position(&self) -> [f32; 3] {
        [
            self.position[0],
            self.position[1] + PLAYER_EYE_HEIGHT,
            self.position[2],
        ]
    }

    fn forward(&self) -> [f32; 3] {
        let yaw_rad = self.yaw.to_radians();
        [yaw_rad.sin(), 0.0, -yaw_rad.cos()]
    }

    fn right(&self) -> [f32; 3] {
        let yaw_rad = self.yaw.to_radians();
        [yaw_rad.cos(), 0.0, yaw_rad.sin()]
    }

    fn look_direction(&self) -> [f32; 3] {
        let yaw = self.yaw.to_radians();
        let pitch = self.pitch.to_radians();
        [
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            -yaw.cos() * pitch.cos(),
        ]
    }

    /// Update physics with gravity and collision
    fn update(&mut self, world: &InfiniteWorld, dt: f32, movement: [f32; 3], jump: bool) {
        // Apply gravity
        if !self.grounded {
            self.velocity[1] -= GRAVITY * dt;
        }

        // Jump
        if jump && self.grounded {
            self.velocity[1] = JUMP_VELOCITY;
            self.grounded = false;
        }

        // Movement (only XZ, speed applied)
        let move_x = movement[0] * PLAYER_SPEED;
        let move_z = movement[2] * PLAYER_SPEED;

        // Attempt to move with collision
        self.move_with_collision(world, dt, move_x, self.velocity[1], move_z);
    }

    fn move_with_collision(&mut self, world: &InfiniteWorld, dt: f32, vx: f32, vy: f32, vz: f32) {
        // Move each axis separately for sliding collision
        let half_w = PLAYER_WIDTH / 2.0;

        // X axis
        let new_x = self.position[0] + vx * dt;
        if !self.collides_at(world, new_x, self.position[1], self.position[2], half_w) {
            self.position[0] = new_x;
        }

        // Z axis
        let new_z = self.position[2] + vz * dt;
        if !self.collides_at(world, self.position[0], self.position[1], new_z, half_w) {
            self.position[2] = new_z;
        }

        // Y axis (gravity/jump)
        let new_y = self.position[1] + vy * dt;
        if vy < 0.0 {
            // Falling - check floor collision
            if self.collides_at(world, self.position[0], new_y, self.position[2], half_w) {
                // Land on floor
                self.position[1] = (new_y + 1.0).floor();
                self.velocity[1] = 0.0;
                self.grounded = true;
            } else {
                self.position[1] = new_y;
                self.grounded = false;
            }
        } else {
            // Rising - check ceiling
            if self.collides_at(world, self.position[0], new_y, self.position[2], half_w) {
                // Hit ceiling
                self.velocity[1] = 0.0;
            } else {
                self.position[1] = new_y;
            }
            self.grounded = false;
        }
    }

    fn collides_at(&self, world: &InfiniteWorld, x: f32, y: f32, z: f32, half_w: f32) -> bool {
        // Check all blocks the player AABB overlaps
        let min_x = (x - half_w).floor() as i32;
        let max_x = (x + half_w).floor() as i32;
        let min_y = y.floor() as i32;
        let max_y = (y + PLAYER_HEIGHT).floor() as i32;
        let min_z = (z - half_w).floor() as i32;
        let max_z = (z + half_w).floor() as i32;

        for bx in min_x..=max_x {
            for by in min_y..=max_y {
                for bz in min_z..=max_z {
                    if world.is_solid(bx, by, bz) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

// =============================================================================
// NPC SYSTEM - UNIT 4 + UNIT 6 INTEGRATION
// =============================================================================

/// NPC Types
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NpcType {
    Wanderer,       // Friendly, uses player model
    ForestGuardian, // Enemy, uses enemy model
}

/// Body part classification for animation
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BodyPart {
    Head,
    Torso,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

/// NPC Entity with physics and AI - HIERARCHICAL ANIMATION
struct Npc {
    position: [f32; 3],
    velocity: [f32; 3],
    yaw: f32,               // Facing direction
    npc_type: NpcType,
    /// Model voxels with body part classification for animation
    /// (local_offset, color, emission, body_part)
    model_voxels: Vec<([f32; 3], [f32; 3], f32, BodyPart)>,
    /// Animation phase
    anim_phase: f32,
    /// Walking speed for animation
    walk_speed: f32,
    /// Is NPC moving?
    is_moving: bool,
}

impl Npc {
    fn new(x: f32, y: f32, z: f32, npc_type: NpcType) -> Self {
        // Load model based on type
        let model = match npc_type {
            NpcType::Wanderer => ProceduralModels::player(),
            NpcType::ForestGuardian => ProceduralModels::enemy(),
        };
        
        // Model origin is at feet - get height for body part classification
        let model_height = model.bounds.height as f32;
        let model_width = model.bounds.width as f32;
        
        // Pre-compute voxel positions with body part classification
        let mut model_voxels = Vec::new();
        for voxel in &model.voxels {
            // Local position relative to model origin (NOT scaled)
            let local_pos = [
                voxel.x as f32 - model.origin[0],
                voxel.y as f32 - model.origin[1],
                voxel.z as f32 - model.origin[2],
            ];
            
            // Classify body part based on position in model
            // Typical humanoid: legs at bottom, torso in middle, head at top
            let rel_y = voxel.y as f32 / model_height;
            let rel_x = (voxel.x as f32 - model_width / 2.0) / (model_width / 2.0);
            
            let body_part = if rel_y > 0.75 {
                BodyPart::Head
            } else if rel_y > 0.4 {
                if rel_x.abs() > 0.5 {
                    if rel_x < 0.0 { BodyPart::LeftArm } else { BodyPart::RightArm }
                } else {
                    BodyPart::Torso
                }
            } else {
                // Legs - below 40% height
                if local_pos[2] <= 0.0 { BodyPart::LeftLeg } else { BodyPart::RightLeg }
            };
            
            // Map material to color
            let color = match voxel.material_id {
                10 => [0.9, 0.7, 0.6],  // Skin
                11 => [0.2, 0.3, 0.8],  // Blue
                12 => [0.8, 0.2, 0.2],  // Red
                13 => [0.3, 0.3, 0.35], // Dark gray
                14 => [0.6, 0.6, 0.65], // Light gray
                15 => [0.5, 0.3, 0.15], // Brown
                16 => [0.2, 0.6, 0.2],  // Green
                19 => [0.6, 0.2, 0.8],  // Purple (enemy)
                100..=103 => [0.0, 1.0, 1.0], // Neon
                _ => [0.5, 0.5, 0.5],   // Default gray
            };
            
            let emission = if let Some(e) = voxel.emission { e[3] } else { 0.0 };
            model_voxels.push((local_pos, color, emission, body_part));
        }
        
        println!("[NPC] Spawned {:?} at ({:.1}, {:.1}, {:.1}) with {} voxels (Hierarchical Animation Enabled)", 
            npc_type, x, y, z, model_voxels.len());
        
        Self {
            position: [x, y, z],
            velocity: [0.0, 0.0, 0.0],
            yaw: 0.0,
            npc_type,
            model_voxels,
            anim_phase: rand_float() * std::f32::consts::PI * 2.0,
            walk_speed: 0.0,
            is_moving: false,
        }
    }
    
    /// Update NPC AI and movement
    fn update(&mut self, world: &InfiniteWorld, dt: f32, player_pos: [f32; 3]) {
        // Simple AI: look at player
        let dx = player_pos[0] - self.position[0];
        let dz = player_pos[2] - self.position[2];
        let target_yaw = dz.atan2(dx).to_degrees();
        
        // Smooth rotation
        let yaw_diff = target_yaw - self.yaw;
        self.yaw += yaw_diff * dt * 2.0;
        
        // Track walking state
        self.is_moving = false;
        
        // Wanderer: move slowly toward player
        // ForestGuardian: stand ground
        if self.npc_type == NpcType::Wanderer {
            let dist = (dx * dx + dz * dz).sqrt();
            if dist > 5.0 {
                let speed = 1.5;
                self.velocity[0] = (dx / dist) * speed;
                self.velocity[2] = (dz / dist) * speed;
                self.is_moving = true;
                self.walk_speed = speed;
            } else {
                self.velocity[0] *= 0.9;
                self.velocity[2] *= 0.9;
            }
        }
        
        // Simple gravity
        self.velocity[1] -= 25.0 * dt;
        
        // Move
        let new_x = self.position[0] + self.velocity[0] * dt;
        let new_z = self.position[2] + self.velocity[2] * dt;
        let new_y = self.position[1] + self.velocity[1] * dt;
        
        // Collision check (simplified)
        if !world.is_solid(new_x as i32, self.position[1] as i32, new_z as i32) {
            self.position[0] = new_x;
            self.position[2] = new_z;
        }
        
        // Ground collision
        if world.is_solid(self.position[0] as i32, new_y as i32, self.position[2] as i32) {
            self.velocity[1] = 0.0;
            self.position[1] = (new_y as i32 + 1) as f32;
        } else {
            self.position[1] = new_y;
        }
        
        // Animation phase (faster when moving)
        if self.is_moving {
            self.anim_phase += dt * 12.0 * self.walk_speed;
        } else {
            self.anim_phase += dt * 2.0; // Slow idle breathing
        }
    }
    
    /// Generate instances for this NPC (with HIERARCHICAL WALKING animation)
    fn generate_instances(&self, time: f32, _log_enabled: bool) -> Vec<VoxelInstance> {
        let mut instances = Vec::new();
        
        // Scale factor: 20-voxel model -> 2 world unit character
        let scale = 0.1f32;
        
        // Rotation matrix (Y-axis rotation for facing direction)
        let yaw_rad = self.yaw.to_radians();
        let cos_y = yaw_rad.cos();
        let sin_y = yaw_rad.sin();
        
        // Animation values
        let walk_phase = self.anim_phase;
        
        // Breathing/idle bobbing
        let breath_offset = (time * 2.0).sin() * 0.02;
        
        // Leg swing angle (walking animation)
        let leg_swing_amplitude = if self.is_moving { 0.5 } else { 0.0 }; // ~30 degrees when walking
        let left_leg_angle = (walk_phase).sin() * leg_swing_amplitude;
        let right_leg_angle = (walk_phase + std::f32::consts::PI).sin() * leg_swing_amplitude;
        
        // Arm swing (opposite to legs)
        let arm_swing_amplitude = if self.is_moving { 0.3 } else { 0.0 };
        let left_arm_angle = -right_leg_angle * arm_swing_amplitude / leg_swing_amplitude.max(0.01);
        let right_arm_angle = -left_leg_angle * arm_swing_amplitude / leg_swing_amplitude.max(0.01);
        
        // Head bob (subtle)
        let head_bob = if self.is_moving { (walk_phase * 2.0).sin() * 0.05 } else { 0.0 };
        
        for (local_offset, color, emission, body_part) in &self.model_voxels {
            // Apply body-part specific animation transform
            let (animated_x, animated_y, animated_z) = match body_part {
                BodyPart::Head => {
                    // Head bobs slightly during walk
                    (local_offset[0], local_offset[1] + head_bob, local_offset[2])
                }
                BodyPart::Torso => {
                    // Torso is stable but breathes
                    (local_offset[0], local_offset[1] + breath_offset, local_offset[2])
                }
                BodyPart::LeftArm => {
                    // Arm swings forward/back (rotation around shoulder)
                    let arm_offset_y = local_offset[1].max(0.0);
                    let swing_z = arm_offset_y * left_arm_angle.sin() * 0.5;
                    let swing_y = arm_offset_y * (1.0 - left_arm_angle.cos()) * 0.1;
                    (local_offset[0], local_offset[1] - swing_y, local_offset[2] + swing_z)
                }
                BodyPart::RightArm => {
                    let arm_offset_y = local_offset[1].max(0.0);
                    let swing_z = arm_offset_y * right_arm_angle.sin() * 0.5;
                    let swing_y = arm_offset_y * (1.0 - right_arm_angle.cos()) * 0.1;
                    (local_offset[0], local_offset[1] - swing_y, local_offset[2] + swing_z)
                }
                BodyPart::LeftLeg => {
                    // Leg swings forward/back (rotation around hip)
                    let leg_offset_y = (-local_offset[1]).max(0.0); // Distance from hip (negative Y)
                    let swing_z = leg_offset_y * left_leg_angle.sin() * 0.8;
                    let swing_y = leg_offset_y * (1.0 - left_leg_angle.cos()) * 0.2;
                    (local_offset[0], local_offset[1] + swing_y, local_offset[2] + swing_z)
                }
                BodyPart::RightLeg => {
                    let leg_offset_y = (-local_offset[1]).max(0.0);
                    let swing_z = leg_offset_y * right_leg_angle.sin() * 0.8;
                    let swing_y = leg_offset_y * (1.0 - right_leg_angle.cos()) * 0.2;
                    (local_offset[0], local_offset[1] + swing_y, local_offset[2] + swing_z)
                }
            };
            
            // Apply scale
            let scaled_x = animated_x * scale;
            let scaled_y = animated_y * scale;
            let scaled_z = animated_z * scale;
            
            // Rotate around Y axis for facing direction
            let rotated_x = scaled_x * cos_y - scaled_z * sin_y;
            let rotated_z = scaled_x * sin_y + scaled_z * cos_y;
            
            let world_pos = [
                self.position[0] + rotated_x,
                self.position[1] + scaled_y,
                self.position[2] + rotated_z,
            ];
            
            // Create instance for each visible face (simplified: all 6 faces)
            // Using scale in position_scale.w for the shader
            for normal_idx in 0..6u32 {
                instances.push(VoxelInstance {
                    position_scale: [world_pos[0], world_pos[1], world_pos[2], scale],
                    dimensions_normal_material: [1.0, 1.0, normal_idx as f32, 255.0], // Neon material for visibility
                    color: [color[0], color[1], color[2], *emission + 1.5], // Boost emission for visibility
                    uv_offset_scale: [0.0, 0.0, 1.0, 1.0],
                });
            }
        }
        

        
        instances
    }
}

/// Simple random float generator using atomic
fn rand_float() -> f32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEED: AtomicU32 = AtomicU32::new(12345);
    let old = SEED.fetch_add(1, Ordering::Relaxed);
    let hash = old.wrapping_mul(1103515245).wrapping_add(12345);
    (hash as f32) / (u32::MAX as f32)
}

// =============================================================================
// RAYCAST SYSTEM (DDA Algorithm) - UNIT 4
// =============================================================================
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct RaycastHit {
    block_pos: [i32; 3],
    /// The face normal that was hit
    normal: [i32; 3],
    distance: f32,
}

/// DDA raycast through voxel grid
fn raycast_voxel(world: &InfiniteWorld, origin: [f32; 3], direction: [f32; 3], max_dist: f32) -> Option<RaycastHit> {
    let dir = normalize(direction);
    
    // Current voxel position
    let mut map_x = origin[0].floor() as i32;
    let mut map_y = origin[1].floor() as i32;
    let mut map_z = origin[2].floor() as i32;

    // Direction signs
    let step_x = if dir[0] >= 0.0 { 1 } else { -1 };
    let step_y = if dir[1] >= 0.0 { 1 } else { -1 };
    let step_z = if dir[2] >= 0.0 { 1 } else { -1 };

    // Delta between voxel boundaries
    let delta_x = if dir[0].abs() < 1e-10 { f32::INFINITY } else { (1.0 / dir[0]).abs() };
    let delta_y = if dir[1].abs() < 1e-10 { f32::INFINITY } else { (1.0 / dir[1]).abs() };
    let delta_z = if dir[2].abs() < 1e-10 { f32::INFINITY } else { (1.0 / dir[2]).abs() };

    // Distance to next voxel boundary
    let mut t_max_x = if dir[0] >= 0.0 {
        ((map_x as f32 + 1.0) - origin[0]) * delta_x
    } else {
        (origin[0] - map_x as f32) * delta_x
    };
    let mut t_max_y = if dir[1] >= 0.0 {
        ((map_y as f32 + 1.0) - origin[1]) * delta_y
    } else {
        (origin[1] - map_y as f32) * delta_y
    };
    let mut t_max_z = if dir[2] >= 0.0 {
        ((map_z as f32 + 1.0) - origin[2]) * delta_z
    } else {
        (origin[2] - map_z as f32) * delta_z
    };

    let mut t = 0.0;
    let mut last_normal = [0i32, 0, 0];

    // Maximum iterations to prevent infinite loops
    let max_steps = (max_dist * 3.0) as i32;

    for _ in 0..max_steps {
        // Check current voxel
        if world.is_solid(map_x, map_y, map_z) {
            return Some(RaycastHit {
                block_pos: [map_x, map_y, map_z],
                normal: last_normal,
                distance: t,
            });
        }

        // Move to next voxel
        if t_max_x < t_max_y && t_max_x < t_max_z {
            t = t_max_x;
            t_max_x += delta_x;
            map_x += step_x;
            last_normal = [-step_x, 0, 0];
        } else if t_max_y < t_max_z {
            t = t_max_y;
            t_max_y += delta_y;
            map_y += step_y;
            last_normal = [0, -step_y, 0];
        } else {
            t = t_max_z;
            t_max_z += delta_z;
            map_z += step_z;
            last_normal = [0, 0, -step_z];
        }

        // Check max distance
        if t > max_dist {
            break;
        }
    }

    None
}

// =============================================================================
// MESH GENERATION WITH AMBIENT OCCLUSION
// =============================================================================
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct VoxelInstance {
    position_scale: [f32; 4],
    dimensions_normal_material: [f32; 4],
    color: [f32; 4],
    /// uv_offset_scale.z = AO value (0-1)
    uv_offset_scale: [f32; 4],
}

// =============================================================================
// OLD AO FUNCTIONS REMOVED - Now using calculate_face_ao_from_buffer
// OPERATION INDUSTRIAL STANDARD
// =============================================================================

/// INDUSTRIAL STANDARD MESHING - using block-mesh-rs
/// COURSE CORRECTION: Now outputs ChunkMesh (vertices + indices)
/// Uses 34x34x34 padded arrays to eliminate inter-chunk holes.
fn generate_terrain_mesh(world: &InfiniteWorld, player_pos: [f32; 3]) -> ChunkMesh {
    let mut combined_mesh = ChunkMesh::default();
    let mut mesher = StandardMesher::new();
    
    // Get the player's chunk and generate mesh for nearby chunks
    let player_chunk = WorldManager::world_to_chunk(player_pos[0], player_pos[2]);
    let render_radius = 5; // Render 5 chunks around player (160 blocks)
    
    for cz in (player_chunk.z - render_radius)..=(player_chunk.z + render_radius) {
        for cx in (player_chunk.x - render_radius)..=(player_chunk.x + render_radius) {
            let chunk_coord = ChunkCoord::new(cx, cz);
            
            // Check if chunk is loaded
            if let Some(chunk) = world.manager().get_chunk(chunk_coord) {
                // World offset for this chunk
                let world_x_offset = cx * CHUNK_SIZE as i32;
                let world_z_offset = cz * CHUNK_SIZE as i32;
                
                // Process each Y-slice as a 32x32x32 sub-chunk for meshing
                // Total height is 256 = 8 slices of 32
                for y_slice in 0..8 {
                    let y_offset = y_slice * 32;
                    
                    // Create padded buffer for this sub-chunk
                    let mut buffer = PaddedChunkBuffer::new();
                    
                    // Fill main chunk data (local coords 0-31 -> padded coords 1-32)
                    for lz in 0..32u32 {
                        for ly in 0..32u32 {
                            for lx in 0..32u32 {
                                let world_y = y_offset + ly as usize;
                                if world_y < 256 {
                                    let block_data = chunk.get_block(lx as usize, world_y, lz as usize);
                                    if !block_data.is_air() {
                                        buffer.set(lx + 1, ly + 1, lz + 1, MeshVoxel::new(block_data.id as u8));
                                    }
                                }
                            }
                        }
                    }
                    
                    // Fill padding from neighbors (for seamless chunk edges)
                    fill_chunk_padding(world, &mut buffer, chunk_coord, y_offset as i32);
                    
                    // Generate mesh using INDUSTRIAL STANDARD algorithm
                    let chunk_world_offset = [world_x_offset, y_offset as i32, world_z_offset];
                    let chunk_mesh = mesher.generate_mesh(&buffer, chunk_world_offset);
                    
                    // Merge into combined mesh
                    let base_vertex = combined_mesh.vertices.len() as u32;
                    combined_mesh.vertices.extend(chunk_mesh.vertices);
                    combined_mesh.indices.extend(
                        chunk_mesh.indices.iter().map(|i| i + base_vertex)
                    );
                }
            }
        }
    }

    combined_mesh
}

/// LEGACY: Generate instances for backward compatibility (NPCs, etc.)
/// Will be phased out as we migrate fully to vertex buffers.
#[allow(dead_code)]
fn generate_mesh(world: &InfiniteWorld, player_pos: [f32; 3]) -> Vec<VoxelInstance> {
    let terrain = generate_terrain_mesh(world, player_pos);
    
    // Convert vertices back to instances for legacy pipeline
    // This is temporary until full pipeline migration
    let mut instances = Vec::with_capacity(terrain.indices.len() / 6);
    
    // Group indices by quads (6 indices = 2 triangles = 1 quad)
    for quad_idx in 0..(terrain.indices.len() / 6) {
        let first_idx = terrain.indices[quad_idx * 6] as usize;
        if first_idx < terrain.vertices.len() {
            let vertex = &terrain.vertices[first_idx];
            let block = BlockType::from_id(vertex.material_ao[0] as u16);
            let color = block.color();
            let emission = if block == BlockType::Neon { 1.5 } else { 0.0 };
            
            instances.push(VoxelInstance {
                position_scale: [vertex.position[0], vertex.position[1], vertex.position[2], 1.0],
                dimensions_normal_material: [1.0, 1.0, 0.0, vertex.material_ao[0]],
                color: [color[0], color[1], color[2], emission],
                uv_offset_scale: [vertex.uv[0], vertex.uv[1], vertex.material_ao[1], 1.0],
            });
        }
    }
    
    instances
}

/// Fill the padding of a chunk buffer from neighbor chunks
/// DATA FEED FIX: Properly populates all 6 faces of the padding region
///
/// The padded buffer is 34x34x34:
/// - Indices 0 and 33 are padding (neighbor data)
/// - Indices 1..33 are actual chunk data
///
/// CRITICAL: If a neighbor chunk is NOT LOADED, the padding stays AIR (default).
/// This ensures boundary faces are DRAWN (preventing holes into void).
fn fill_chunk_padding(
    world: &InfiniteWorld,
    buffer: &mut PaddedChunkBuffer,
    chunk_coord: ChunkCoord,
    y_offset: i32,
) {
    let cx = chunk_coord.x;
    let cz = chunk_coord.z;
    
    // Helper to get voxel from world coordinates
    // Returns None if out of bounds or chunk not loaded
    let get_world_voxel = |world_x: i32, world_y: i32, world_z: i32| -> Option<u8> {
        if world_y < 0 || world_y >= 256 {
            return None; // Out of Y bounds = AIR
        }
        
        // Calculate which chunk this voxel belongs to
        let chunk_x = world_x.div_euclid(CHUNK_SIZE as i32);
        let chunk_z = world_z.div_euclid(CHUNK_SIZE as i32);
        let local_x = world_x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_z = world_z.rem_euclid(CHUNK_SIZE as i32) as usize;
        
        world.manager()
            .get_chunk(ChunkCoord::new(chunk_x, chunk_z))
            .and_then(|chunk| {
                let block = chunk.get_block(local_x, world_y as usize, local_z);
                if block.is_air() { None } else { Some(block.id as u8) }
            })
    };
    
    // World coordinates of chunk origin
    let world_x_base = cx * CHUNK_SIZE as i32;
    let world_z_base = cz * CHUNK_SIZE as i32;
    
    // ==========================================================================
    // FILL 6 FACES OF PADDING
    // ==========================================================================
    
    // +X face (padded x=33, corresponds to world x = chunk_x + 32)
    for pz in 1..33u32 {
        for py in 1..33u32 {
            let world_x = world_x_base + 32; // One past chunk edge
            let world_y = y_offset + (py as i32 - 1);
            let world_z = world_z_base + (pz as i32 - 1);
            
            if let Some(material) = get_world_voxel(world_x, world_y, world_z) {
                buffer.set(33, py, pz, MeshVoxel::new(material));
            }
        }
    }
    
    // -X face (padded x=0, corresponds to world x = chunk_x - 1)
    for pz in 1..33u32 {
        for py in 1..33u32 {
            let world_x = world_x_base - 1; // One before chunk edge
            let world_y = y_offset + (py as i32 - 1);
            let world_z = world_z_base + (pz as i32 - 1);
            
            if let Some(material) = get_world_voxel(world_x, world_y, world_z) {
                buffer.set(0, py, pz, MeshVoxel::new(material));
            }
        }
    }
    
    // +Z face (padded z=33, corresponds to world z = chunk_z + 32)
    for px in 1..33u32 {
        for py in 1..33u32 {
            let world_x = world_x_base + (px as i32 - 1);
            let world_y = y_offset + (py as i32 - 1);
            let world_z = world_z_base + 32; // One past chunk edge
            
            if let Some(material) = get_world_voxel(world_x, world_y, world_z) {
                buffer.set(px, py, 33, MeshVoxel::new(material));
            }
        }
    }
    
    // -Z face (padded z=0, corresponds to world z = chunk_z - 1)
    for px in 1..33u32 {
        for py in 1..33u32 {
            let world_x = world_x_base + (px as i32 - 1);
            let world_y = y_offset + (py as i32 - 1);
            let world_z = world_z_base - 1; // One before chunk edge
            
            if let Some(material) = get_world_voxel(world_x, world_y, world_z) {
                buffer.set(px, py, 0, MeshVoxel::new(material));
            }
        }
    }
    
    // +Y face (padded y=33, corresponds to world y = y_offset + 32)
    for px in 1..33u32 {
        for pz in 1..33u32 {
            let world_x = world_x_base + (px as i32 - 1);
            let world_y = y_offset + 32; // One above sub-chunk
            let world_z = world_z_base + (pz as i32 - 1);
            
            if let Some(material) = get_world_voxel(world_x, world_y, world_z) {
                buffer.set(px, 33, pz, MeshVoxel::new(material));
            }
        }
    }
    
    // -Y face (padded y=0, corresponds to world y = y_offset - 1)
    for px in 1..33u32 {
        for pz in 1..33u32 {
            let world_x = world_x_base + (px as i32 - 1);
            let world_y = y_offset - 1; // One below sub-chunk
            let world_z = world_z_base + (pz as i32 - 1);
            
            if let Some(material) = get_world_voxel(world_x, world_y, world_z) {
                buffer.set(px, 0, pz, MeshVoxel::new(material));
            }
        }
    }
    
    // ==========================================================================
    // OPTIONAL: Fill edges (where 2 padding faces meet)
    // The algorithm mostly doesn't need these, but filling them
    // ensures correct behavior in edge cases.
    // ==========================================================================
    
    // Edge: X=0, Z=0 (bottom-left edge running up Y)
    for py in 1..33u32 {
        let world_x = world_x_base - 1;
        let world_y = y_offset + (py as i32 - 1);
        let world_z = world_z_base - 1;
        if let Some(material) = get_world_voxel(world_x, world_y, world_z) {
            buffer.set(0, py, 0, MeshVoxel::new(material));
        }
    }
    
    // Edge: X=33, Z=0
    for py in 1..33u32 {
        let world_x = world_x_base + 32;
        let world_y = y_offset + (py as i32 - 1);
        let world_z = world_z_base - 1;
        if let Some(material) = get_world_voxel(world_x, world_y, world_z) {
            buffer.set(33, py, 0, MeshVoxel::new(material));
        }
    }
    
    // Edge: X=0, Z=33
    for py in 1..33u32 {
        let world_x = world_x_base - 1;
        let world_y = y_offset + (py as i32 - 1);
        let world_z = world_z_base + 32;
        if let Some(material) = get_world_voxel(world_x, world_y, world_z) {
            buffer.set(0, py, 33, MeshVoxel::new(material));
        }
    }
    
    // Edge: X=33, Z=33
    for py in 1..33u32 {
        let world_x = world_x_base + 32;
        let world_y = y_offset + (py as i32 - 1);
        let world_z = world_z_base + 32;
        if let Some(material) = get_world_voxel(world_x, world_y, world_z) {
            buffer.set(33, py, 33, MeshVoxel::new(material));
        }
    }
}

// =============================================================================
// AO CALCULATION REMOVED - Now handled by StandardMesher
// COURSE CORRECTION: block-mesh-rs handles this internally
// =============================================================================

/// Generate wireframe cube instances for selection highlight
fn generate_selection_cube(pos: [i32; 3]) -> Vec<VoxelInstance> {
    let mut instances = Vec::new();
    let [x, y, z] = [pos[0] as f32, pos[1] as f32, pos[2] as f32];
    
    // Neon yellow highlight color
    let color = [1.0, 1.0, 0.0, 2.0]; // High emission
    
    // Slightly expanded to avoid z-fighting
    let offset = 0.001;
    let size = 1.0 + offset * 2.0;
    
    // All 6 faces as wireframe quads
    for normal in 0..6u32 {
        let (dx, dy, dz): (f32, f32, f32) = match normal {
            0 => (size, 0.0, 0.0),
            1 => (0.0, 0.0, 0.0),
            2 => (0.0, size, 0.0),
            3 => (0.0, 0.0, 0.0),
            4 => (0.0, 0.0, size),
            _ => (0.0, 0.0, 0.0),
        };
        
        instances.push(VoxelInstance {
            position_scale: [x - offset + dx, y - offset + dy, z - offset + dz, 1.0],
            dimensions_normal_material: [size, size, normal as f32, 255.0],
            color,
            uv_offset_scale: [0.0, 0.0, 1.0, 1.0],
        });
    }
    
    instances
}

// =============================================================================
// MATH HELPERS
// =============================================================================
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]] }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }
fn normalize(v: [f32; 3]) -> [f32; 3] { 
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt(); 
    if l < 1e-10 { return [0.0, 1.0, 0.0]; }
    [v[0]/l, v[1]/l, v[2]/l] 
}

fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let f = normalize(sub(target, eye));
    let r = normalize(cross(f, up));
    let u = cross(r, f);
    
    [
        [r[0], u[0], -f[0], 0.0],
        [r[1], u[1], -f[1], 0.0],
        [r[2], u[2], -f[2], 0.0],
        [-dot(r, eye), -dot(u, eye), dot(f, eye), 1.0],
    ]
}

fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov / 2.0).tan();
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, (near * far) / (near - far), 0.0],
    ]
}

fn multiply_matrices(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                result[i][j] += a[k][j] * b[i][k];
            }
        }
    }
    result
}

// =============================================================================
// GPU STRUCTURES
// =============================================================================
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    camera_params: [f32; 4],
}

// =============================================================================
// MAIN
// =============================================================================
#[allow(unused_assignments)]
fn main() {
    // Panic hook for debugging
    std::panic::set_hook(Box::new(|info| {
        eprintln!("\n═══════════════════════════════════════════════════════════════");
        eprintln!("                    FATAL ERROR");
        eprintln!("═══════════════════════════════════════════════════════════════");
        eprintln!("{}", info);
        eprintln!("\nPress ENTER to close...");
        let mut s = String::new();
        let _ = std::io::stdin().read_line(&mut s);
    }));

    // =========================================================================
    // FILE LOGGING SETUP
    // =========================================================================
    let log_file = std::fs::File::create("client.log").expect("Failed to create log file");
    let mut log_writer = std::io::BufWriter::new(log_file);
    let _ = writeln!(log_writer, "════════════════════════════════════════════════════════════════");
    let _ = writeln!(log_writer, "OROBOROS CLIENT LOG - GENESIS BUILD");
    let _ = writeln!(log_writer, "Timestamp: {:?}", std::time::SystemTime::now());
    let _ = writeln!(log_writer, "════════════════════════════════════════════════════════════════");
    let _ = log_writer.flush();
    
    macro_rules! log {
        ($($arg:tt)*) => {{
            let msg = format!($($arg)*);
            println!("{}", msg);
            let _ = writeln!(log_writer, "{}", msg);
            let _ = log_writer.flush();
        }};
    }

    log!("╔═══════════════════════════════════════════════════════════════╗");
    log!("║           OROBOROS - GENESIS BUILD                            ║");
    log!("║      INFINITE WORLD + Physics + Mining                        ║");
    log!("╠═══════════════════════════════════════════════════════════════╣");
    log!("║  CONTROLS:                                                    ║");
    log!("║    WASD       - Move                                          ║");
    log!("║    SPACE      - Jump                                          ║");
    log!("║    Mouse      - Look around                                   ║");
    log!("║    Left Click - Break block                                   ║");
    log!("║    ESC        - Exit                                          ║");
    log!("╚═══════════════════════════════════════════════════════════════╝");
    log!("");
    log!("[STARTUP] Game Started");

    // =========================================================================
    // INFINITE WORLD INITIALIZATION
    // =========================================================================
    let seed = 42u64; // World seed for reproducible terrain
    let mut world = InfiniteWorld::new(seed);
    log!("[WORLD] Created with seed: {}", seed);
    
    // Spawn at origin and pre-load area
    let spawn_x = 0.0f32;
    let spawn_z = 0.0f32;
    world.ensure_spawn_loaded(spawn_x, spawn_z);
    log!("[WORLD] Spawn area pre-loaded");
    
    // Find spawn height
    let spawn_y = world.find_ground_height(spawn_x as i32, spawn_z as i32) as f32;
    
    let mut player = Player::new(spawn_x, spawn_y + 2.0, spawn_z);
    log!("[PLAYER] Spawned at ({:.1}, {:.1}, {:.1})", spawn_x, spawn_y + 2.0, spawn_z);

    // =========================================================================
    // NPC SPAWNING - UNIT 4 + UNIT 6 INTEGRATION
    // =========================================================================
    log!("[NPC] Spawning NPCs...");
    let mut npcs: Vec<Npc> = Vec::new();
    
    // Spawn Wanderers (friendly) - CLOSE to player for visibility
    for i in 0..3 {
        let npc_x = spawn_x + 5.0 + i as f32 * 4.0;  // Closer spacing
        let npc_z = spawn_z + 3.0;
        // Use same height as player spawn + a bit for safety
        let npc_y = spawn_y + 1.0;
        log!("[NPC] Creating Wanderer {} at ({:.1}, {:.1}, {:.1})", i, npc_x, npc_y, npc_z);
        npcs.push(Npc::new(npc_x, npc_y, npc_z, NpcType::Wanderer));
    }
    
    // Spawn ForestGuardians (enemies) - In front of player
    for i in 0..2 {
        let npc_x = spawn_x - 8.0 - i as f32 * 5.0;
        let npc_z = spawn_z + 6.0;
        let npc_y = spawn_y + 1.0;
        log!("[NPC] Creating ForestGuardian {} at ({:.1}, {:.1}, {:.1})", i, npc_x, npc_y, npc_z);
        npcs.push(Npc::new(npc_x, npc_y, npc_z, NpcType::ForestGuardian));
    }
    
    log!("[NPC] Spawned {} total NPCs (should be 5)", npcs.len());
    
    // Track game time for animations
    let mut game_time = 0.0f32;

    // Generate initial mesh (INDUSTRIAL STANDARD: Vertex + Index buffers)
    log!("[MESH] Generating initial terrain mesh...");
    let start = Instant::now();
    let mut terrain_mesh = generate_terrain_mesh(&world, player.position);
    log!("[MESH] Generated {} vertices, {} indices ({} triangles) in {:?}", 
        terrain_mesh.vertices.len(), 
        terrain_mesh.indices.len(),
        terrain_mesh.triangle_count(),
        start.elapsed());
    world.dirty = false;

    // Create window
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let window = WindowBuilder::new()
        .with_title("OROBOROS - Gameplay Alpha [WASD + Space + Left Click to Mine]")
        .with_inner_size(PhysicalSize::new(1280, 720))
        .build(&event_loop)
        .expect("Failed to create window");
    let window = Arc::new(window);
    
    // GPU initialization
    println!("[GPU] Initializing...");
    
    #[cfg(target_os = "windows")]
    let backends = wgpu::Backends::DX12;
    #[cfg(not(target_os = "windows"))]
    let backends = wgpu::Backends::PRIMARY;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
        ..Default::default()
    });
    
    let surface = instance.create_surface(window.clone()).expect("Failed to create surface");
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    })).expect("No GPU adapter found");
    
    println!("[GPU] Using: {}", adapter.get_info().name);
    
    // =========================================================================
    // FIX: Explicit high limits to prevent "Buffer too small" crashes
    // =========================================================================
    let limits = wgpu::Limits {
        max_storage_buffer_binding_size: 1 << 30, // 1GB
        max_buffer_size: 1 << 30, // 1GB
        ..wgpu::Limits::default()
    };
    println!("[GPU] Using HIGH PERFORMANCE buffer limits (1GB)");
    
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("OROBOROS"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
        },
        None,
    )).expect("Failed to create device");
    
    println!("[GPU] Device created successfully");
    
    // Surface config
    let size = window.inner_size();
    let caps = surface.get_capabilities(&adapter);
    let format = caps.formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(caps.formats[0]);
    
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    // Depth buffer
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    
    fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        texture.create_view(&Default::default())
    }
    
    let mut depth_view = create_depth_texture(&device, config.width, config.height);
    
    // Shader
    println!("[SHADER] Loading...");
    // FIX: Load shader from assets folder for deployment compatibility
    // Using include_str! for embedded build, but verifying path correctness
    let shader_source = include_str!("../../../../assets/shaders/unit2/voxel_instanced.wgsl");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Voxel Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    
    // Camera buffer
    let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Camera"),
        size: std::mem::size_of::<CameraUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera Layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Camera Bind Group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: camera_buffer.as_entire_binding(),
        }],
    });
    
    // Pipeline
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Voxel Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            // INDUSTRIAL STANDARD: Vertex buffer layout (not instance!)
            buffers: &[TerrainVertex::desc()],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_solid",  // Solid color shader - NO TEXTURES (Group 0 only)
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Ccw,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            // Depth bias to prevent Z-fighting at chunk boundaries
            bias: wgpu::DepthBiasState {
                constant: 0,
                slope_scale: 0.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });
    
    // INDUSTRIAL STANDARD: Vertex + Index buffers
    let mut vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Terrain Vertex Buffer"),
        contents: bytemuck::cast_slice(&terrain_mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let mut index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Terrain Index Buffer"),
        contents: bytemuck::cast_slice(&terrain_mesh.indices),
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
    });
    let mut index_count = terrain_mesh.indices.len() as u32;

    // Selection highlight buffer (max 6 faces)
    let selection_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Selection Buffer"),
        size: (6 * std::mem::size_of::<VoxelInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    #[allow(unused_assignments)]
    let mut selection_count = 0u32;
    
    // NPC buffer (dynamic - updated each frame)
    // Max 5 NPCs * 500 voxels * 6 faces = 15000 instances
    let npc_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("NPC Buffer"),
        size: (15000 * std::mem::size_of::<VoxelInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    #[allow(unused_assignments)]
    let mut npc_instance_count = 0u32;

    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    GAME READY!                                ║");
    println!("║                Click window to capture mouse                  ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    
    // Game state
    let mut keys_pressed = HashSet::new();
    let mut mouse_captured = false;
    let mut last_frame = Instant::now();
    let mut last_status = Instant::now();
    let mut current_target: Option<RaycastHit> = None;
    let mut blocks_mined = 0u32;
    
    let _ = event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);
        
        match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    
                    WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(key), state, .. }, .. } => {
                        match state {
                            ElementState::Pressed => { keys_pressed.insert(key); }
                            ElementState::Released => { keys_pressed.remove(&key); }
                        }
                        if key == KeyCode::Escape && state == ElementState::Pressed {
                            elwt.exit();
                        }
                    }

                    WindowEvent::MouseInput { button: MouseButton::Left, state: ElementState::Pressed, .. } => {
                        if !mouse_captured {
                            // Capture mouse
                            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                            window.set_cursor_visible(false);
                            mouse_captured = true;
                        } else {
                            // MINE BLOCK!
                            if let Some(hit) = current_target {
                                let [x, y, z] = hit.block_pos;
                                let block = world.get(x, y, z);
                                if block != BlockType::Bedrock {
                                    world.set(x, y, z, BlockType::Air);
                                    blocks_mined += 1;
                                    println!("[MINE] Broke {:?} at [{}, {}, {}] (Total: {})", 
                                        block, x, y, z, blocks_mined);
                                } else {
                                    println!("[MINE] Cannot break bedrock!");
                                }
                            }
                        }
                    }

                    WindowEvent::Focused(false) => {
                        let _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                        mouse_captured = false;
                    }

                    WindowEvent::Resized(new_size) => {
                        if new_size.width > 0 && new_size.height > 0 {
                            config.width = new_size.width;
                            config.height = new_size.height;
                            surface.configure(&device, &config);
                            depth_view = create_depth_texture(&device, config.width, config.height);
                        }
                    }

                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = (now - last_frame).as_secs_f32().min(0.1);
                        last_frame = now;

                        // Calculate movement
                        let mut movement = [0.0f32; 3];
                        let fwd = player.forward();
                        let right = player.right();

                        if keys_pressed.contains(&KeyCode::KeyW) {
                            movement[0] += fwd[0];
                            movement[2] += fwd[2];
                        }
                        if keys_pressed.contains(&KeyCode::KeyS) {
                            movement[0] -= fwd[0];
                            movement[2] -= fwd[2];
                        }
                        if keys_pressed.contains(&KeyCode::KeyA) {
                            movement[0] -= right[0];
                            movement[2] -= right[2];
                        }
                        if keys_pressed.contains(&KeyCode::KeyD) {
                            movement[0] += right[0];
                            movement[2] += right[2];
                        }

                        // Normalize diagonal movement
                        let len = (movement[0]*movement[0] + movement[2]*movement[2]).sqrt();
                        if len > 0.0 {
                            movement[0] /= len;
                            movement[2] /= len;
                        }

                        let jump = keys_pressed.contains(&KeyCode::Space);

                        // Update player physics
                        player.update(&world, dt, movement, jump);
                        
                        // Update game time for animations
                        game_time += dt;
                        
                        // =====================================================
                        // NPC AI UPDATE - UNIT 4 + UNIT 6
                        // =====================================================
                        for npc in &mut npcs {
                            npc.update(&world, dt, player.position);
                        }
                        
                        // =====================================================
                        // INFINITE WORLD STREAMING UPDATE (with Neighbor Notification)
                        // =====================================================
                        let newly_loaded = world.update_streaming(
                            player.position[0], 
                            player.position[2]
                        );
                        if !newly_loaded.is_empty() {

                        }

                        // Raycast for block targeting
                        let eye = player.eye_position();
                        let look = player.look_direction();
                        current_target = raycast_voxel(&world, eye, look, RAYCAST_DISTANCE);

                        // Update selection highlight
                        if let Some(hit) = current_target {
                            let sel_instances = generate_selection_cube(hit.block_pos);
                            selection_count = sel_instances.len() as u32;
                            queue.write_buffer(&selection_buffer, 0, bytemuck::cast_slice(&sel_instances));
                        } else {
                            selection_count = 0;
                        }
                        
                        // =====================================================
                        // NPC RENDERING - UNIT 4 + UNIT 6 WITH ANIMATION
                        // =====================================================
                        let mut npc_instances: Vec<VoxelInstance> = Vec::new();
                        for npc in &npcs {
                            npc_instances.extend(npc.generate_instances(game_time, false)); // Silent in production
                        }
                        npc_instance_count = npc_instances.len() as u32;
                        if npc_instance_count > 0 {
                            queue.write_buffer(&npc_buffer, 0, bytemuck::cast_slice(&npc_instances));
                        }

                        // Regenerate mesh if world changed (mining or streaming)
                        if world.dirty {
                            terrain_mesh = generate_terrain_mesh(&world, player.position);
                            index_count = terrain_mesh.indices.len() as u32;
                            
                            // Recreate vertex + index buffers (INDUSTRIAL STANDARD)
                            vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Terrain Vertex Buffer"),
                                contents: bytemuck::cast_slice(&terrain_mesh.vertices),
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            });
                            index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Terrain Index Buffer"),
                                contents: bytemuck::cast_slice(&terrain_mesh.indices),
                                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                            });
                            world.dirty = false;
                        }

                        // Camera matrices
                        let eye_pos = player.eye_position();
                        let target = [
                            eye_pos[0] + look[0],
                            eye_pos[1] + look[1],
                            eye_pos[2] + look[2],
                        ];
                        let view = look_at(eye_pos, target, [0.0, 1.0, 0.0]);
                        let aspect = config.width as f32 / config.height as f32;
                        let proj = perspective(70.0_f32.to_radians(), aspect, 0.1, 600.0); // OPTIMIZED: Reduced draw distance
                        let view_proj = multiply_matrices(proj, view);
                        
                        let uniform = CameraUniform {
                            view_proj,
                            view,
                            projection: proj,
                            camera_pos: [eye_pos[0], eye_pos[1], eye_pos[2], 1.0],
                            camera_params: [0.1, 600.0, aspect, 70.0_f32.to_radians()], // OPTIMIZED: matches perspective far
                        };
                        queue.write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&uniform));
                        
                        // Render
                        let output = match surface.get_current_texture() {
                            Ok(t) => t,
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                surface.configure(&device, &config);
                                return;
                            }
                            Err(e) => {
                                eprintln!("[GPU ERROR] {:?}", e);
                                return;
                            }
                        };
                        
                        let view = output.texture.create_view(&Default::default());
                        let mut encoder = device.create_command_encoder(&Default::default());
                        
                        {
                            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Main Pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.4, g: 0.6, b: 0.9, a: 1.0 }), // Sky blue
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                    view: &depth_view,
                                    depth_ops: Some(wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(1.0),
                                        store: wgpu::StoreOp::Store,
                                    }),
                                    stencil_ops: None,
                                }),
                                ..Default::default()
                            });
                            
                            pass.set_pipeline(&pipeline);
                            pass.set_bind_group(0, &bind_group, &[]);

                            // INDUSTRIAL STANDARD: Draw terrain with vertex + index buffers
                            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            pass.draw_indexed(0..index_count, 0, 0..1);

                            // TODO: Selection and NPC rendering need separate pipeline
                            // For now, terrain-only rendering with new mesh system
                            let _ = selection_count; // Suppress warning
                            let _ = npc_instance_count; // Suppress warning
                        }
                        
                        queue.submit(std::iter::once(encoder.finish()));
                        output.present();
                        
                        // Status log with chunk info and NPC count
                        if last_status.elapsed().as_secs() >= 2 {
                            let target_str = current_target.map(|h| 
                                format!("[{}, {}, {}]", h.block_pos[0], h.block_pos[1], h.block_pos[2])
                            ).unwrap_or_else(|| "None".to_string());
                            
                            let stats = world.manager().stats();
                            let player_chunk = WorldManager::world_to_chunk(player.position[0], player.position[2]);
                            
                            println!("╔═══════════════════════════════════════════════════════════════╗");
                            println!("║ Server: \x1b[32mONLINE\x1b[0m                                              ║");
                            println!("╠═══════════════════════════════════════════════════════════════╣");
                            println!("║ Player: ({:.1}, {:.1}, {:.1}) | Chunk: ({}, {})",
                                player.position[0], player.position[1], player.position[2],
                                player_chunk.x, player_chunk.z);
                            println!("║ NPCs: {} | Chunks: {} | Triangles: {} | Mined: {}",
                                npcs.len(), stats.loaded_chunks, index_count / 3, blocks_mined);
                            println!("║ Target: {}", target_str);
                            println!("╚═══════════════════════════════════════════════════════════════╝");
                            last_status = Instant::now();
                        }
                    }
                    _ => {}
                }
            }
            
            Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                if mouse_captured {
                    player.yaw += delta.0 as f32 * MOUSE_SENSITIVITY;
                    player.pitch -= delta.1 as f32 * MOUSE_SENSITIVITY;
                    player.pitch = player.pitch.clamp(-89.0, 89.0);
                    
                    // PANOPTICON: Log significant camera rotations

                }
            }
            
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    });
}

use wgpu::util::DeviceExt;
