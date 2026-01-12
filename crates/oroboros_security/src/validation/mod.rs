//! # Hit Validation
//!
//! Server-side validation of shots and hits.
//!
//! ## Philosophy
//!
//! NEVER trust the client. The client says "I shot at position X".
//! We verify:
//! 1. Could they see the target?
//! 2. Was the target actually there?
//! 3. Is the shot geometrically possible?

use oroboros_core::Position;
use oroboros_networking::protocol::{ShotFired, EntityState};

/// Result of hit validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationResult {
    /// Shot is valid and hit the target.
    ValidHit,
    /// Shot is valid but missed.
    ValidMiss,
    /// Shot is invalid (cheating suspected).
    Invalid,
    /// Cannot determine (insufficient data).
    Inconclusive,
}

/// Hitbox for an entity.
#[derive(Clone, Copy, Debug)]
pub struct Hitbox {
    /// Center position.
    pub center: Position,
    /// Half-extents (width/2, height/2, depth/2).
    pub half_extents: (f32, f32, f32),
}

impl Hitbox {
    /// Creates a new hitbox.
    #[must_use]
    pub const fn new(center: Position, half_extents: (f32, f32, f32)) -> Self {
        Self { center, half_extents }
    }

    /// Creates a standard player hitbox.
    #[must_use]
    pub fn player(position: Position) -> Self {
        Self {
            center: Position::new(position.x, position.y + 1.0, position.z),
            half_extents: (0.4, 1.0, 0.4), // Player is ~0.8m wide, 2m tall
        }
    }

    /// Checks if a ray intersects this hitbox.
    #[must_use]
    pub fn ray_intersects(&self, origin: Position, direction: (f32, f32, f32)) -> Option<f32> {
        // AABB ray intersection using slab method
        let inv_dir = (
            1.0 / direction.0,
            1.0 / direction.1,
            1.0 / direction.2,
        );

        let min = Position::new(
            self.center.x - self.half_extents.0,
            self.center.y - self.half_extents.1,
            self.center.z - self.half_extents.2,
        );
        let max = Position::new(
            self.center.x + self.half_extents.0,
            self.center.y + self.half_extents.1,
            self.center.z + self.half_extents.2,
        );

        let t1 = (min.x - origin.x) * inv_dir.0;
        let t2 = (max.x - origin.x) * inv_dir.0;
        let t3 = (min.y - origin.y) * inv_dir.1;
        let t4 = (max.y - origin.y) * inv_dir.1;
        let t5 = (min.z - origin.z) * inv_dir.2;
        let t6 = (max.z - origin.z) * inv_dir.2;

        let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
        let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

        if tmax < 0.0 || tmin > tmax {
            None
        } else {
            Some(tmin.max(0.0))
        }
    }

    /// Returns true if a point is inside the hitbox.
    #[must_use]
    pub fn contains(&self, point: Position) -> bool {
        let dx = (point.x - self.center.x).abs();
        let dy = (point.y - self.center.y).abs();
        let dz = (point.z - self.center.z).abs();
        
        dx <= self.half_extents.0 
            && dy <= self.half_extents.1 
            && dz <= self.half_extents.2
    }
}

/// Hitbox validator for shot verification.
pub struct HitboxValidator {
    /// Maximum weapon range.
    max_range: f32,
    /// Position tolerance for lag compensation.
    position_tolerance: f32,
    /// Time window for lag compensation (ticks).
    lag_compensation_ticks: u32,
}

impl HitboxValidator {
    /// Creates a new validator.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_range: 500.0,
            position_tolerance: 0.5,
            lag_compensation_ticks: 6, // 100ms at 60Hz
        }
    }

    /// Sets the maximum weapon range.
    pub fn set_max_range(&mut self, range: f32) {
        self.max_range = range;
    }

    /// Sets the position tolerance.
    pub fn set_position_tolerance(&mut self, tolerance: f32) {
        self.position_tolerance = tolerance;
    }

    /// Sets the lag compensation window.
    pub fn set_lag_compensation(&mut self, ticks: u32) {
        self.lag_compensation_ticks = ticks;
    }

    /// Validates a shot against a target.
    ///
    /// # Arguments
    ///
    /// * `shot` - The shot fired by the client
    /// * `shooter_actual` - Server's record of shooter position
    /// * `target_states` - Historical positions of the target (for lag compensation)
    pub fn validate_shot(
        &self,
        shot: &ShotFired,
        shooter_actual: Position,
        target_states: &[EntityState],
    ) -> (ValidationResult, Option<u32>) {
        // 1. Verify shooter position isn't too far from server's record
        let shooter_claim = Position::new(shot.origin_x, shot.origin_y, shot.origin_z);
        let shooter_diff = calculate_distance(shooter_claim, shooter_actual);
        
        if shooter_diff > self.position_tolerance * 2.0 {
            // Shooter position doesn't match - suspicious
            return (ValidationResult::Invalid, None);
        }

        // 2. Normalize direction
        let dir_len = (shot.dir_x * shot.dir_x 
            + shot.dir_y * shot.dir_y 
            + shot.dir_z * shot.dir_z).sqrt();
        
        if dir_len < 0.01 {
            return (ValidationResult::Invalid, None);
        }
        
        let dir = (
            shot.dir_x / dir_len,
            shot.dir_y / dir_len,
            shot.dir_z / dir_len,
        );

        // 3. Check against historical target positions (lag compensation)
        let mut best_hit: Option<(f32, usize)> = None;

        for (i, state) in target_states.iter().enumerate() {
            let hitbox = Hitbox::player(state.position());
            
            if let Some(t) = hitbox.ray_intersects(shooter_claim, dir) {
                if t <= self.max_range {
                    match best_hit {
                        None => best_hit = Some((t, i)),
                        Some((best_t, _)) if t < best_t => best_hit = Some((t, i)),
                        _ => {}
                    }
                }
            }
        }

        match best_hit {
            Some((_, idx)) => {
                let entity_id = target_states[idx].entity_id;
                (ValidationResult::ValidHit, Some(entity_id))
            }
            None => (ValidationResult::ValidMiss, None),
        }
    }

    /// Performs detailed analysis for replay verification.
    pub fn analyze_hit(
        &self,
        shot: &ShotFired,
        shooter_pos: Position,
        target_pos: Position,
    ) -> HitAnalysis {
        let shooter_claim = Position::new(shot.origin_x, shot.origin_y, shot.origin_z);
        let dir = (shot.dir_x, shot.dir_y, shot.dir_z);
        
        let hitbox = Hitbox::player(target_pos);
        let intersection = hitbox.ray_intersects(shooter_claim, dir);

        HitAnalysis {
            shooter_claimed_pos: shooter_claim,
            shooter_actual_pos: shooter_pos,
            position_discrepancy: calculate_distance(shooter_claim, shooter_pos),
            target_hitbox: hitbox,
            ray_hit_distance: intersection,
            would_hit: intersection.map(|t| t <= self.max_range).unwrap_or(false),
        }
    }
}

impl Default for HitboxValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Detailed hit analysis for replay verification.
#[derive(Clone, Debug)]
pub struct HitAnalysis {
    /// Position the shooter claimed to be at.
    pub shooter_claimed_pos: Position,
    /// Position the server recorded.
    pub shooter_actual_pos: Position,
    /// Distance between claimed and actual position.
    pub position_discrepancy: f32,
    /// Target's hitbox.
    pub target_hitbox: Hitbox,
    /// Distance along ray to hitbox intersection (if any).
    pub ray_hit_distance: Option<f32>,
    /// Whether the shot would hit.
    pub would_hit: bool,
}

/// Calculates distance between positions.
fn calculate_distance(a: Position, b: Position) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hitbox_contains() {
        let hitbox = Hitbox::new(
            Position::new(0.0, 1.0, 0.0),
            (1.0, 1.0, 1.0),
        );

        assert!(hitbox.contains(Position::new(0.0, 1.0, 0.0)));
        assert!(hitbox.contains(Position::new(0.5, 1.5, 0.5)));
        assert!(!hitbox.contains(Position::new(2.0, 1.0, 0.0)));
    }

    #[test]
    fn test_ray_intersection() {
        let hitbox = Hitbox::new(
            Position::new(10.0, 1.0, 0.0),
            (1.0, 1.0, 1.0),
        );

        // Ray pointing at hitbox
        let origin = Position::new(0.0, 1.0, 0.0);
        let direction = (1.0, 0.0, 0.0);
        
        let t = hitbox.ray_intersects(origin, direction);
        assert!(t.is_some());
        assert!((t.unwrap() - 9.0).abs() < 0.01); // Hit at x=9 (10-1)

        // Ray pointing away
        let direction = (-1.0, 0.0, 0.0);
        let t = hitbox.ray_intersects(origin, direction);
        assert!(t.is_none());
    }

    #[test]
    fn test_shot_validation() {
        let validator = HitboxValidator::new();

        let shot = ShotFired {
            tick: 100,
            origin_x: 0.0,
            origin_y: 1.0,
            origin_z: 0.0,
            dir_x: 1.0,
            dir_y: 0.0,
            dir_z: 0.0,
            weapon_id: 1,
            _padding: [0; 3],
        };

        let shooter_actual = Position::new(0.0, 1.0, 0.0);
        let target_states = vec![
            EntityState {
                entity_id: 2,
                pos_x: 10.0,
                pos_y: 0.0,
                pos_z: 0.0,
                ..Default::default()
            },
        ];

        let (result, target) = validator.validate_shot(&shot, shooter_actual, &target_states);
        
        assert_eq!(result, ValidationResult::ValidHit);
        assert_eq!(target, Some(2));
    }

    #[test]
    fn test_invalid_position() {
        let validator = HitboxValidator::new();

        // Shooter claims to be far from actual position
        let shot = ShotFired {
            tick: 100,
            origin_x: 100.0, // Way off
            origin_y: 1.0,
            origin_z: 0.0,
            dir_x: 1.0,
            dir_y: 0.0,
            dir_z: 0.0,
            weapon_id: 1,
            _padding: [0; 3],
        };

        let shooter_actual = Position::new(0.0, 1.0, 0.0);
        let target_states = vec![];

        let (result, _) = validator.validate_shot(&shot, shooter_actual, &target_states);
        
        assert_eq!(result, ValidationResult::Invalid);
    }
}
