//! # NPC System - LIFE INJECTION
//!
//! CEO MANDATE: "I want to see characters."
//!
//! UNIT 4 Implementation:
//! - NPC entities with gravity and collision
//! - Simple AI state machine (Idle, Wander, LookAtPlayer)
//! - Spawning on chunk generation
//!
//! CRITICAL: NPCs must NOT fall through the floor!

use crate::physics::{VoxelWorld, AABB, GRAVITY, TERMINAL_VELOCITY};

// ============================================================================
// NPC CONSTANTS
// ============================================================================

/// NPC movement speed (blocks per second).
pub const NPC_MOVE_SPEED: f32 = 2.0;

/// NPC hitbox width (blocks).
pub const NPC_WIDTH: f32 = 0.8;

/// NPC hitbox height (blocks).
pub const NPC_HEIGHT: f32 = 2.0;

/// Detection range for player (blocks).
pub const NPC_DETECTION_RANGE: f32 = 10.0;

/// Wander radius (blocks).
pub const NPC_WANDER_RADIUS: f32 = 5.0;

/// Minimum idle time (seconds).
pub const NPC_IDLE_MIN: f32 = 2.0;

/// Maximum idle time (seconds).
pub const NPC_IDLE_MAX: f32 = 5.0;

/// NPC spawn chance per chunk (0.0-1.0).
pub const NPC_SPAWN_CHANCE: f32 = 0.3;

/// Maximum NPCs per chunk.
pub const NPC_MAX_PER_CHUNK: usize = 2;

// ============================================================================
// NPC TYPES
// ============================================================================

/// Types of NPCs that can exist in the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpcType {
    /// A guardian of the forest. Green color.
    ForestGuardian,
    /// A wandering merchant. Blue color.
    Wanderer,
    /// A hostile creature. Red color.
    Hostile,
}

impl NpcType {
    /// Returns the color for this NPC type (RGBA).
    pub fn color(&self) -> [f32; 4] {
        match self {
            NpcType::ForestGuardian => [0.2, 0.8, 0.3, 1.0], // Green
            NpcType::Wanderer => [0.3, 0.5, 0.9, 1.0],       // Blue
            NpcType::Hostile => [0.9, 0.2, 0.2, 1.0],        // Red
        }
    }

    /// Returns the display name.
    pub fn name(&self) -> &'static str {
        match self {
            NpcType::ForestGuardian => "Forest Guardian",
            NpcType::Wanderer => "Wanderer",
            NpcType::Hostile => "Hostile",
        }
    }
}

// ============================================================================
// AI STATE MACHINE
// ============================================================================

/// AI behavior states.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AiState {
    /// Standing still, waiting.
    Idle {
        /// Time remaining in idle state.
        time_remaining: f32,
    },
    /// Walking to a target position.
    Wander {
        /// Target position to walk to.
        target: [f32; 3],
    },
    /// Facing the player.
    LookAtPlayer {
        /// Yaw angle to face.
        target_yaw: f32,
    },
}

impl Default for AiState {
    fn default() -> Self {
        AiState::Idle { time_remaining: 3.0 }
    }
}

// ============================================================================
// NPC COMPONENT
// ============================================================================

/// A non-player character entity.
#[derive(Clone, Debug)]
pub struct Npc {
    /// Unique identifier.
    pub id: u32,
    /// NPC type.
    pub npc_type: NpcType,
    /// Current position (feet).
    pub position: [f32; 3],
    /// Current velocity.
    pub velocity: [f32; 3],
    /// Facing direction (degrees, 0 = +Z, 90 = +X).
    pub yaw: f32,
    /// Is on ground?
    pub on_ground: bool,
    /// Current AI state.
    pub ai_state: AiState,
    /// Home position (spawn point) for wander calculations.
    pub home: [f32; 3],
    /// Random seed for this NPC (deterministic behavior).
    seed: u32,
}

impl Npc {
    /// Creates a new NPC at the given position.
    pub fn new(id: u32, npc_type: NpcType, position: [f32; 3]) -> Self {
        // Generate deterministic seed from ID and position
        let seed = id.wrapping_mul(2654435769)
            ^ (position[0] as u32).wrapping_mul(73856093)
            ^ (position[2] as u32).wrapping_mul(19349663);

        Self {
            id,
            npc_type,
            position,
            velocity: [0.0, 0.0, 0.0],
            yaw: 0.0,
            on_ground: false,
            ai_state: AiState::Idle { time_remaining: 3.0 },
            home: position,
            seed,
        }
    }

    /// Gets the NPC's AABB for collision.
    pub fn get_aabb(&self) -> AABB {
        AABB::from_center(self.position, NPC_WIDTH, NPC_HEIGHT)
    }

    /// Gets the center position (for rendering).
    pub fn center(&self) -> [f32; 3] {
        [
            self.position[0],
            self.position[1] + NPC_HEIGHT / 2.0,
            self.position[2],
        ]
    }

    /// Generates a pseudo-random float [0, 1) using the NPC's seed.
    fn random(&mut self) -> f32 {
        // LCG random number generator
        self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        ((self.seed >> 16) & 0x7FFF) as f32 / 32768.0
    }

    /// Updates the NPC's AI and physics.
    pub fn update(&mut self, dt: f32, world: &VoxelWorld, player_pos: [f32; 3]) {
        // Update AI state machine
        self.update_ai(dt, player_pos);

        // Apply gravity
        if !self.on_ground {
            self.velocity[1] -= GRAVITY * dt;
            self.velocity[1] = self.velocity[1].max(-TERMINAL_VELOCITY);
        }

        // Calculate movement delta
        let delta = [
            self.velocity[0] * dt,
            self.velocity[1] * dt,
            self.velocity[2] * dt,
        ];

        // Move with collision (same as CharacterController)
        self.move_with_collision(delta, world);
    }

    /// Updates AI state machine.
    fn update_ai(&mut self, dt: f32, player_pos: [f32; 3]) {
        // Calculate distance to player
        let dx = player_pos[0] - self.position[0];
        let dz = player_pos[2] - self.position[2];
        let dist_to_player = (dx * dx + dz * dz).sqrt();

        // Check if should look at player (priority state)
        if dist_to_player < NPC_DETECTION_RANGE {
            let target_yaw = dx.atan2(dz).to_degrees();
            self.ai_state = AiState::LookAtPlayer { target_yaw };
        }

        match self.ai_state {
            AiState::Idle { time_remaining } => {
                // Stand still
                self.velocity[0] = 0.0;
                self.velocity[2] = 0.0;

                if time_remaining <= 0.0 {
                    // Pick a random wander target
                    let angle = self.random() * std::f32::consts::TAU;
                    let radius = self.random() * NPC_WANDER_RADIUS;
                    let target = [
                        self.home[0] + angle.cos() * radius,
                        self.position[1], // Keep same Y for now
                        self.home[2] + angle.sin() * radius,
                    ];
                    self.ai_state = AiState::Wander { target };
                } else {
                    self.ai_state = AiState::Idle {
                        time_remaining: time_remaining - dt,
                    };
                }
            }
            AiState::Wander { target } => {
                // Move toward target
                let dx = target[0] - self.position[0];
                let dz = target[2] - self.position[2];
                let dist = (dx * dx + dz * dz).sqrt();

                if dist < 0.5 {
                    // Reached target, go idle
                    let idle_time = NPC_IDLE_MIN + self.random() * (NPC_IDLE_MAX - NPC_IDLE_MIN);
                    self.ai_state = AiState::Idle { time_remaining: idle_time };
                } else {
                    // Walk toward target
                    let dir_x = dx / dist;
                    let dir_z = dz / dist;
                    self.velocity[0] = dir_x * NPC_MOVE_SPEED;
                    self.velocity[2] = dir_z * NPC_MOVE_SPEED;

                    // Update facing direction
                    self.yaw = dx.atan2(dz).to_degrees();
                }
            }
            AiState::LookAtPlayer { target_yaw } => {
                // Stop moving and rotate to face player
                self.velocity[0] = 0.0;
                self.velocity[2] = 0.0;

                // Smoothly rotate toward target yaw
                let mut diff = target_yaw - self.yaw;
                while diff > 180.0 { diff -= 360.0; }
                while diff < -180.0 { diff += 360.0; }

                let rotation_speed = 180.0; // degrees per second
                if diff.abs() < rotation_speed * dt {
                    self.yaw = target_yaw;
                } else {
                    self.yaw += diff.signum() * rotation_speed * dt;
                }

                // If player leaves range, go back to idle
                if dist_to_player > NPC_DETECTION_RANGE {
                    let idle_time = NPC_IDLE_MIN + self.random() * (NPC_IDLE_MAX - NPC_IDLE_MIN);
                    self.ai_state = AiState::Idle { time_remaining: idle_time };
                }
            }
        }
    }

    /// Moves with collision detection (same logic as CharacterController).
    fn move_with_collision(&mut self, delta: [f32; 3], world: &VoxelWorld) {
        // X axis
        if delta[0].abs() > 0.0001 {
            self.position[0] += delta[0];
            if self.check_collision_and_resolve(0, world) {
                self.velocity[0] = 0.0;
            }
        }

        // Y axis
        let was_falling = self.velocity[1] < 0.0;
        if delta[1].abs() > 0.0001 {
            self.position[1] += delta[1];
            if self.check_collision_and_resolve(1, world) {
                if was_falling {
                    self.on_ground = true;
                }
                self.velocity[1] = 0.0;
            } else {
                self.on_ground = false;
            }
        }

        // Z axis
        if delta[2].abs() > 0.0001 {
            self.position[2] += delta[2];
            if self.check_collision_and_resolve(2, world) {
                self.velocity[2] = 0.0;
            }
        }

        // Extra ground check
        if self.velocity[1] <= 0.0 {
            let feet_aabb = AABB {
                min: [
                    self.position[0] - NPC_WIDTH / 2.0,
                    self.position[1] - 0.1,
                    self.position[2] - NPC_WIDTH / 2.0,
                ],
                max: [
                    self.position[0] + NPC_WIDTH / 2.0,
                    self.position[1],
                    self.position[2] + NPC_WIDTH / 2.0,
                ],
            };

            for (vx, vy, vz) in world.get_colliding_voxels(&feet_aabb) {
                let voxel_aabb = AABB::from_voxel(vx, vy, vz);
                if feet_aabb.intersects(&voxel_aabb) {
                    self.on_ground = true;
                    break;
                }
            }
        }
    }

    /// Checks collision and resolves it.
    fn check_collision_and_resolve(&mut self, axis: usize, world: &VoxelWorld) -> bool {
        let aabb = self.get_aabb();
        let voxels = world.get_colliding_voxels(&aabb);

        let mut collided = false;

        for (vx, vy, vz) in voxels {
            let voxel_aabb = AABB::from_voxel(vx, vy, vz);

            if aabb.intersects(&voxel_aabb) {
                collided = true;

                let overlap = aabb.get_overlap(&voxel_aabb);

                let push = match axis {
                    0 => {
                        if self.position[0] < vx as f32 + 0.5 {
                            -overlap[0]
                        } else {
                            overlap[0]
                        }
                    }
                    1 => {
                        if self.position[1] + NPC_HEIGHT / 2.0 < vy as f32 + 0.5 {
                            -overlap[1]
                        } else {
                            overlap[1]
                        }
                    }
                    2 => {
                        if self.position[2] < vz as f32 + 0.5 {
                            -overlap[2]
                        } else {
                            overlap[2]
                        }
                    }
                    _ => 0.0,
                };

                self.position[axis] += push;
            }
        }

        collided
    }
}

// ============================================================================
// NPC MANAGER
// ============================================================================

/// Manages all NPCs in the world.
pub struct NpcManager {
    /// All active NPCs.
    npcs: Vec<Npc>,
    /// Next NPC ID to assign.
    next_id: u32,
}

impl NpcManager {
    /// Creates a new empty NPC manager.
    pub fn new() -> Self {
        Self {
            npcs: Vec::with_capacity(256),
            next_id: 1,
        }
    }

    /// Spawns an NPC at the given position.
    pub fn spawn(&mut self, npc_type: NpcType, position: [f32; 3]) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        let npc = Npc::new(id, npc_type, position);
        println!(
            "[NPC] 🧙 Spawned {} (ID: {}) at ({:.1}, {:.1}, {:.1})",
            npc_type.name(), id, position[0], position[1], position[2]
        );
        self.npcs.push(npc);

        id
    }

    /// Tries to spawn NPCs for a newly generated chunk.
    /// Uses deterministic random based on chunk coordinates.
    pub fn try_spawn_for_chunk(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        chunk_size: i32,
        get_height: impl Fn(i32, i32) -> i32,
    ) {
        // Deterministic seed from chunk position
        let seed = (chunk_x as u32).wrapping_mul(73856093)
            ^ (chunk_z as u32).wrapping_mul(19349663);

        // Check spawn chance
        let random_value = ((seed >> 8) & 0xFFFF) as f32 / 65536.0;
        if random_value > NPC_SPAWN_CHANCE {
            return; // No spawn this chunk
        }

        // How many to spawn
        let count_seed = seed.wrapping_mul(2654435769);
        let count = 1 + ((count_seed >> 16) as usize % NPC_MAX_PER_CHUNK);

        for i in 0..count {
            // Pick position within chunk
            let pos_seed = seed.wrapping_add(i as u32 * 31337);
            let local_x = ((pos_seed >> 4) & 0xFF) as i32 % chunk_size;
            let local_z = ((pos_seed >> 12) & 0xFF) as i32 % chunk_size;

            let world_x = chunk_x * chunk_size + local_x;
            let world_z = chunk_z * chunk_size + local_z;
            let ground_y = get_height(world_x, world_z);

            // Spawn 2 blocks above ground
            let spawn_y = ground_y + 2;

            // Pick NPC type
            let type_seed = (pos_seed >> 20) as usize % 3;
            let npc_type = match type_seed {
                0 => NpcType::ForestGuardian,
                1 => NpcType::Wanderer,
                _ => NpcType::Hostile,
            };

            self.spawn(npc_type, [world_x as f32, spawn_y as f32, world_z as f32]);
        }
    }

    /// Updates all NPCs.
    pub fn update(&mut self, dt: f32, world: &VoxelWorld, player_pos: [f32; 3]) {
        for npc in &mut self.npcs {
            npc.update(dt, world, player_pos);
        }
    }

    /// Returns all NPCs for rendering.
    pub fn npcs(&self) -> &[Npc] {
        &self.npcs
    }

    /// Returns mutable access to NPCs.
    pub fn npcs_mut(&mut self) -> &mut [Npc] {
        &mut self.npcs
    }

    /// Gets the number of NPCs.
    pub fn count(&self) -> usize {
        self.npcs.len()
    }

    /// Removes an NPC by ID.
    pub fn remove(&mut self, id: u32) -> bool {
        if let Some(idx) = self.npcs.iter().position(|n| n.id == id) {
            let npc = self.npcs.remove(idx);
            println!("[NPC] ☠️ Removed {} (ID: {})", npc.npc_type.name(), id);
            true
        } else {
            false
        }
    }

    /// Finds an NPC by ID.
    pub fn get(&self, id: u32) -> Option<&Npc> {
        self.npcs.iter().find(|n| n.id == id)
    }

    /// Finds a mutable NPC by ID.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Npc> {
        self.npcs.iter_mut().find(|n| n.id == id)
    }
}

impl Default for NpcManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// NPC RENDERING DATA
// ============================================================================

/// Instance data for rendering a single NPC.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NpcInstance {
    /// Position (x, y, z) + scale.
    pub position_scale: [f32; 4],
    /// Color (RGBA).
    pub color: [f32; 4],
}

impl From<&Npc> for NpcInstance {
    fn from(npc: &Npc) -> Self {
        let center = npc.center();
        Self {
            position_scale: [center[0], center[1], center[2], 1.0],
            color: npc.npc_type.color(),
        }
    }
}

/// Generates render instances from all NPCs.
pub fn generate_npc_instances(manager: &NpcManager) -> Vec<NpcInstance> {
    manager.npcs().iter().map(NpcInstance::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_npc_creation() {
        let npc = Npc::new(1, NpcType::ForestGuardian, [10.0, 20.0, 30.0]);
        assert_eq!(npc.id, 1);
        assert_eq!(npc.npc_type, NpcType::ForestGuardian);
        assert_eq!(npc.position, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn test_npc_manager_spawn() {
        let mut manager = NpcManager::new();
        let id1 = manager.spawn(NpcType::ForestGuardian, [0.0, 10.0, 0.0]);
        let id2 = manager.spawn(NpcType::Wanderer, [5.0, 10.0, 5.0]);

        assert_eq!(manager.count(), 2);
        assert!(manager.get(id1).is_some());
        assert!(manager.get(id2).is_some());
    }

    #[test]
    fn test_ai_state_default() {
        let state = AiState::default();
        matches!(state, AiState::Idle { .. });
    }
}
