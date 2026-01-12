//! # Component System
//!
//! Components are pure data containers with no behavior.
//! They must be Copy and have a fixed size for zero-allocation storage.

use bytemuck::{Pod, Zeroable};

/// Marker trait for ECS components.
///
/// Components must be:
/// - `Copy`: No heap allocations, bitwise copyable
/// - `Pod`: Plain old data, safe to transmute
/// - `Zeroable`: Can be safely zeroed
/// - `Default`: Must have a default value for pre-allocation
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Clone, Copy, Default, Pod, Zeroable)]
/// #[repr(C)]
/// struct Position {
///     x: f32,
///     y: f32,
///     z: f32,
/// }
///
/// impl Component for Position {
///     const ID: u8 = 0;
/// }
/// ```
pub trait Component: Copy + Pod + Zeroable + Default + Send + Sync + 'static {
    /// Unique identifier for this component type (0-63).
    ///
    /// This ID is used for the component bitmask in entities.
    const ID: u8;
}

/// Position component for entities.
///
/// Represents a 3D position in world space.
/// Used for voxels, characters, projectiles, etc.
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Position {
    /// X coordinate in world space.
    pub x: f32,
    /// Y coordinate in world space.
    pub y: f32,
    /// Z coordinate in world space.
    pub z: f32,
    /// Padding for alignment (ensures 16-byte alignment for SIMD).
    pub _padding: f32,
}

impl Component for Position {
    const ID: u8 = 0;
}

impl Position {
    /// Creates a new position.
    #[inline]
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z,
            _padding: 0.0,
        }
    }

    /// Returns the squared distance to another position.
    ///
    /// This avoids the sqrt call for distance comparisons.
    #[inline]
    #[must_use]
    pub fn distance_squared(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }
}

/// Velocity component for entities.
///
/// Represents movement speed in world units per second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Velocity {
    /// X velocity component.
    pub x: f32,
    /// Y velocity component.
    pub y: f32,
    /// Z velocity component.
    pub z: f32,
    /// Padding for alignment.
    pub _padding: f32,
}

impl Component for Velocity {
    const ID: u8 = 1;
}

impl Velocity {
    /// Creates a new velocity.
    #[inline]
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z,
            _padding: 0.0,
        }
    }
}

/// Voxel component for terrain/blocks.
///
/// Minimal data for voxel representation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct Voxel {
    /// Material/block type ID.
    pub material_id: u16,
    /// Flags (solid, transparent, etc.).
    pub flags: u8,
    /// Light level (0-15).
    pub light_level: u8,
}

impl Component for Voxel {
    const ID: u8 = 2;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_distance() {
        let a = Position::new(0.0, 0.0, 0.0);
        let b = Position::new(3.0, 4.0, 0.0);
        assert!((a.distance_squared(b) - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_component_sizes() {
        // Ensure alignment for SIMD
        assert_eq!(std::mem::size_of::<Position>(), 16);
        assert_eq!(std::mem::size_of::<Velocity>(), 16);
        assert_eq!(std::mem::size_of::<Voxel>(), 4);
    }
}
