//! # OROBOROS Physics System
//!
//! UNIT 4 - Kinematic Character Controller with Voxel Collision
//!
//! Features:
//! - Gravity simulation
//! - AABB collision against voxel grid
//! - Voxel raycasting for block selection
//! - Ground detection and jumping

/// Gravity acceleration (blocks per second squared).
pub const GRAVITY: f32 = 32.0;

/// Terminal velocity (blocks per second).
pub const TERMINAL_VELOCITY: f32 = 50.0;

/// Jump velocity (blocks per second).
pub const JUMP_VELOCITY: f32 = 10.0;

/// Player hitbox width (blocks).
pub const PLAYER_WIDTH: f32 = 0.6;
/// Player hitbox height (blocks).
pub const PLAYER_HEIGHT: f32 = 1.8;
/// Player eye height offset from feet (blocks).
pub const PLAYER_EYE_HEIGHT: f32 = 1.6;

/// Minimum world coordinate for collision checking.
pub const WORLD_MIN: i32 = -256;
/// Maximum world coordinate for collision checking.
pub const WORLD_MAX: i32 = 256;

// ============================================================================
// AABB (Axis-Aligned Bounding Box)
// ============================================================================

/// Axis-Aligned Bounding Box for collision detection.
#[derive(Clone, Copy, Debug)]
pub struct AABB {
    /// Minimum corner of the box (x, y, z).
    pub min: [f32; 3],
    /// Maximum corner of the box (x, y, z).
    pub max: [f32; 3],
}

impl AABB {
    /// Creates a new AABB.
    pub fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Self { min, max }
    }

    /// Creates an AABB centered at position with given dimensions.
    pub fn from_center(center: [f32; 3], width: f32, height: f32) -> Self {
        let half_w = width / 2.0;
        Self {
            min: [center[0] - half_w, center[1], center[2] - half_w],
            max: [center[0] + half_w, center[1] + height, center[2] + half_w],
        }
    }

    /// Creates an AABB for a single voxel at integer coordinates.
    pub fn from_voxel(x: i32, y: i32, z: i32) -> Self {
        Self {
            min: [x as f32, y as f32, z as f32],
            max: [(x + 1) as f32, (y + 1) as f32, (z + 1) as f32],
        }
    }

    /// Checks if this AABB intersects another.
    pub fn intersects(&self, other: &AABB) -> bool {
        self.min[0] < other.max[0] && self.max[0] > other.min[0] &&
        self.min[1] < other.max[1] && self.max[1] > other.min[1] &&
        self.min[2] < other.max[2] && self.max[2] > other.min[2]
    }

    /// Returns the overlap amount on each axis. Positive = overlap, Negative = gap.
    pub fn get_overlap(&self, other: &AABB) -> [f32; 3] {
        [
            (self.max[0].min(other.max[0]) - self.min[0].max(other.min[0])),
            (self.max[1].min(other.max[1]) - self.min[1].max(other.min[1])),
            (self.max[2].min(other.max[2]) - self.min[2].max(other.min[2])),
        ]
    }

    /// Moves the AABB by delta.
    pub fn translate(&self, delta: [f32; 3]) -> Self {
        Self {
            min: [self.min[0] + delta[0], self.min[1] + delta[1], self.min[2] + delta[2]],
            max: [self.max[0] + delta[0], self.max[1] + delta[1], self.max[2] + delta[2]],
        }
    }
}

// ============================================================================
// VOXEL WORLD (Collision Data)
// ============================================================================

/// Callback to check if a voxel exists at given coordinates.
/// Returns `true` if solid (blocks movement).
pub type VoxelQueryFn = fn(x: i32, y: i32, z: i32) -> bool;

/// Simple voxel world for collision testing.
/// Uses a height-based terrain model.
pub struct VoxelWorld {
    /// Custom query function (if set).
    custom_query: Option<VoxelQueryFn>,
}

impl VoxelWorld {
    /// Creates a new voxel world with procedural terrain.
    pub fn new() -> Self {
        Self { custom_query: None }
    }

    /// Sets a custom query function for collision checking.
    pub fn with_query(query: VoxelQueryFn) -> Self {
        Self { custom_query: Some(query) }
    }

    /// Checks if a voxel exists at the given coordinates.
    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        if let Some(query) = self.custom_query {
            return query(x, y, z);
        }

        // Default: procedural terrain
        // Ground level with some hills
        let ground_height = self.get_height(x, z);
        y < ground_height && y >= 0
    }

    /// Gets the terrain height at (x, z).
    pub fn get_height(&self, x: i32, z: i32) -> i32 {
        // Simple noise-based height
        let seed = ((x.abs() % 97) * 7919 + (z.abs() % 97) * 4363) as f32;
        let noise = (seed * 0.01).sin() * 0.5 + 0.5;
        let base_height = 1;
        let hill_height = (noise * 10.0) as i32;
        
        // Add some features
        let dist_from_center = ((x * x + z * z) as f32).sqrt();
        if dist_from_center < 20.0 {
            base_height // Flat spawn area
        } else {
            base_height + hill_height
        }
    }

    /// Gets all solid voxels that might collide with an AABB.
    pub fn get_colliding_voxels(&self, aabb: &AABB) -> Vec<(i32, i32, i32)> {
        let mut voxels = Vec::new();

        let min_x = aabb.min[0].floor() as i32;
        let max_x = aabb.max[0].ceil() as i32;
        let min_y = aabb.min[1].floor() as i32;
        let max_y = aabb.max[1].ceil() as i32;
        let min_z = aabb.min[2].floor() as i32;
        let max_z = aabb.max[2].ceil() as i32;

        for y in min_y..max_y {
            for z in min_z..max_z {
                for x in min_x..max_x {
                    if self.is_solid(x, y, z) {
                        voxels.push((x, y, z));
                    }
                }
            }
        }

        voxels
    }
}

impl Default for VoxelWorld {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// CHARACTER CONTROLLER
// ============================================================================

/// Kinematic character controller with physics.
pub struct CharacterController {
    /// Position (feet position).
    pub position: [f32; 3],
    /// Velocity (blocks per second).
    pub velocity: [f32; 3],
    /// Is the character on the ground?
    pub on_ground: bool,
    /// Movement speed (blocks per second).
    pub move_speed: f32,
    /// Sprint multiplier.
    pub sprint_multiplier: f32,
    /// Is sprinting?
    pub sprinting: bool,
}

impl CharacterController {
    /// Creates a new character controller at the given position.
    pub fn new(position: [f32; 3]) -> Self {
        Self {
            position,
            velocity: [0.0, 0.0, 0.0],
            on_ground: false,
            move_speed: 6.0,
            sprint_multiplier: 1.5,
            sprinting: false,
        }
    }

    /// Gets the eye position (for camera).
    pub fn eye_position(&self) -> [f32; 3] {
        [
            self.position[0],
            self.position[1] + PLAYER_EYE_HEIGHT,
            self.position[2],
        ]
    }

    /// Gets the player's AABB.
    pub fn get_aabb(&self) -> AABB {
        AABB::from_center(self.position, PLAYER_WIDTH, PLAYER_HEIGHT)
    }

    /// Applies movement input (normalized direction).
    pub fn apply_input(&mut self, forward: f32, right: f32, yaw: f32) {
        let yaw_rad = yaw.to_radians();
        let sin_yaw = yaw_rad.sin();
        let cos_yaw = yaw_rad.cos();

        let speed = self.move_speed * if self.sprinting { self.sprint_multiplier } else { 1.0 };

        // Calculate movement direction in world space
        let move_x = (sin_yaw * forward + cos_yaw * right) * speed;
        let move_z = (-cos_yaw * forward + sin_yaw * right) * speed;

        self.velocity[0] = move_x;
        self.velocity[2] = move_z;
    }

    /// Attempts to jump.
    pub fn jump(&mut self) {
        if self.on_ground {
            self.velocity[1] = JUMP_VELOCITY;
            self.on_ground = false;
        }
    }

    /// Updates physics (gravity, collision).
    pub fn update(&mut self, dt: f32, world: &VoxelWorld) {
        // Apply gravity
        if !self.on_ground {
            self.velocity[1] -= GRAVITY * dt;
            self.velocity[1] = self.velocity[1].max(-TERMINAL_VELOCITY);
        }

        // Calculate desired movement
        let delta = [
            self.velocity[0] * dt,
            self.velocity[1] * dt,
            self.velocity[2] * dt,
        ];

        // Move with collision detection (sweep each axis separately)
        self.move_with_collision(delta, world);
    }

    /// Moves the character with collision detection.
    fn move_with_collision(&mut self, delta: [f32; 3], world: &VoxelWorld) {
        // Move on each axis separately for stable collision response
        
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
                min: [self.position[0] - PLAYER_WIDTH / 2.0, self.position[1] - 0.1, self.position[2] - PLAYER_WIDTH / 2.0],
                max: [self.position[0] + PLAYER_WIDTH / 2.0, self.position[1], self.position[2] + PLAYER_WIDTH / 2.0],
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

    /// Checks for collision on a specific axis and resolves it.
    /// Returns true if collision occurred.
    fn check_collision_and_resolve(&mut self, axis: usize, world: &VoxelWorld) -> bool {
        let aabb = self.get_aabb();
        let voxels = world.get_colliding_voxels(&aabb);

        let mut collided = false;

        for (vx, vy, vz) in voxels {
            let voxel_aabb = AABB::from_voxel(vx, vy, vz);
            
            if aabb.intersects(&voxel_aabb) {
                collided = true;

                // Calculate push-out direction
                let overlap = aabb.get_overlap(&voxel_aabb);
                
                // Push out along the specified axis
                let push = match axis {
                    0 => {
                        // X axis
                        if self.position[0] < vx as f32 + 0.5 {
                            -overlap[0]
                        } else {
                            overlap[0]
                        }
                    }
                    1 => {
                        // Y axis
                        if self.position[1] + PLAYER_HEIGHT / 2.0 < vy as f32 + 0.5 {
                            -overlap[1]
                        } else {
                            overlap[1]
                        }
                    }
                    2 => {
                        // Z axis
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
// RAYCAST SYSTEM
// ============================================================================

/// Result of a raycast against the voxel world.
#[derive(Clone, Copy, Debug)]
pub struct RaycastHit {
    /// The voxel coordinates that were hit.
    pub voxel: [i32; 3],
    /// The face normal of the hit (-1, 0, or 1 for each axis).
    pub normal: [i32; 3],
    /// Distance from ray origin to hit point.
    pub distance: f32,
    /// Exact hit position in world space.
    pub hit_point: [f32; 3],
}

/// Performs a raycast against the voxel world.
/// Uses DDA (Digital Differential Analyzer) algorithm for efficiency.
pub fn raycast(
    origin: [f32; 3],
    direction: [f32; 3],
    max_distance: f32,
    world: &VoxelWorld,
) -> Option<RaycastHit> {
    // Normalize direction
    let len = (direction[0].powi(2) + direction[1].powi(2) + direction[2].powi(2)).sqrt();
    if len < 0.0001 {
        return None;
    }
    let dir = [direction[0] / len, direction[1] / len, direction[2] / len];

    // Current voxel position
    let mut voxel = [
        origin[0].floor() as i32,
        origin[1].floor() as i32,
        origin[2].floor() as i32,
    ];

    // Step direction for each axis
    let step = [
        if dir[0] >= 0.0 { 1 } else { -1 },
        if dir[1] >= 0.0 { 1 } else { -1 },
        if dir[2] >= 0.0 { 1 } else { -1 },
    ];

    // Distance to next voxel boundary for each axis
    let t_delta = [
        if dir[0].abs() < 0.0001 { f32::MAX } else { (1.0 / dir[0]).abs() },
        if dir[1].abs() < 0.0001 { f32::MAX } else { (1.0 / dir[1]).abs() },
        if dir[2].abs() < 0.0001 { f32::MAX } else { (1.0 / dir[2]).abs() },
    ];

    // Distance to first voxel boundary
    let mut t_max = [
        if dir[0] >= 0.0 {
            ((voxel[0] + 1) as f32 - origin[0]) / dir[0].max(0.0001)
        } else {
            (voxel[0] as f32 - origin[0]) / dir[0].min(-0.0001)
        },
        if dir[1] >= 0.0 {
            ((voxel[1] + 1) as f32 - origin[1]) / dir[1].max(0.0001)
        } else {
            (voxel[1] as f32 - origin[1]) / dir[1].min(-0.0001)
        },
        if dir[2] >= 0.0 {
            ((voxel[2] + 1) as f32 - origin[2]) / dir[2].max(0.0001)
        } else {
            (voxel[2] as f32 - origin[2]) / dir[2].min(-0.0001)
        },
    ];

    let mut distance = 0.0;
    let mut last_normal = [0, 0, 0];

    while distance < max_distance {
        // Check current voxel
        if world.is_solid(voxel[0], voxel[1], voxel[2]) {
            let hit_point = [
                origin[0] + dir[0] * distance,
                origin[1] + dir[1] * distance,
                origin[2] + dir[2] * distance,
            ];

            return Some(RaycastHit {
                voxel,
                normal: last_normal,
                distance,
                hit_point,
            });
        }

        // Step to next voxel
        if t_max[0] < t_max[1] && t_max[0] < t_max[2] {
            distance = t_max[0];
            t_max[0] += t_delta[0];
            voxel[0] += step[0];
            last_normal = [-step[0], 0, 0];
        } else if t_max[1] < t_max[2] {
            distance = t_max[1];
            t_max[1] += t_delta[1];
            voxel[1] += step[1];
            last_normal = [0, -step[1], 0];
        } else {
            distance = t_max[2];
            t_max[2] += t_delta[2];
            voxel[2] += step[2];
            last_normal = [0, 0, -step[2]];
        }
    }

    None
}

/// Gets the look direction from camera yaw and pitch.
pub fn get_look_direction(yaw: f32, pitch: f32) -> [f32; 3] {
    let yaw_rad = yaw.to_radians();
    let pitch_rad = pitch.to_radians();
    [
        yaw_rad.sin() * pitch_rad.cos(),
        pitch_rad.sin(),
        -yaw_rad.cos() * pitch_rad.cos(),
    ]
}

// ============================================================================
// WIREFRAME CUBE (for selection highlight)
// ============================================================================

/// Generates line vertices for a wireframe cube around a voxel.
/// Returns 24 vertices (12 lines * 2 vertices per line).
pub fn generate_wireframe_cube(voxel: [i32; 3]) -> [[f32; 3]; 24] {
    let x = voxel[0] as f32;
    let y = voxel[1] as f32;
    let z = voxel[2] as f32;

    // Slightly expanded to avoid z-fighting
    let e = 0.005;
    let x0 = x - e;
    let x1 = x + 1.0 + e;
    let y0 = y - e;
    let y1 = y + 1.0 + e;
    let z0 = z - e;
    let z1 = z + 1.0 + e;

    // 12 edges of a cube (24 vertices for line list)
    [
        // Bottom face
        [x0, y0, z0], [x1, y0, z0],
        [x1, y0, z0], [x1, y0, z1],
        [x1, y0, z1], [x0, y0, z1],
        [x0, y0, z1], [x0, y0, z0],
        // Top face
        [x0, y1, z0], [x1, y1, z0],
        [x1, y1, z0], [x1, y1, z1],
        [x1, y1, z1], [x0, y1, z1],
        [x0, y1, z1], [x0, y1, z0],
        // Vertical edges
        [x0, y0, z0], [x0, y1, z0],
        [x1, y0, z0], [x1, y1, z0],
        [x1, y0, z1], [x1, y1, z1],
        [x0, y0, z1], [x0, y1, z1],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb_intersection() {
        let a = AABB::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let b = AABB::new([0.5, 0.5, 0.5], [1.5, 1.5, 1.5]);
        let c = AABB::new([2.0, 2.0, 2.0], [3.0, 3.0, 3.0]);

        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn test_raycast_hits_ground() {
        let world = VoxelWorld::new();
        let origin = [0.0, 10.0, 0.0];
        let direction = [0.0, -1.0, 0.0];
        
        let hit = raycast(origin, direction, 100.0, &world);
        assert!(hit.is_some());
        
        let hit = hit.unwrap();
        assert!(hit.voxel[1] >= 0);
        assert_eq!(hit.normal, [0, 1, 0]); // Hit from above
    }
}
