//! Mathematical types shared between client and server.
//!
//! These are the canonical representations used in the network protocol.

use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

/// 3D Vector - position, velocity, direction
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
pub struct Vec3 {
    /// X component
    pub x: f32,
    /// Y component
    pub y: f32,
    /// Z component
    pub z: f32,
}

impl Vec3 {
    /// Creates a new Vec3
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Zero vector
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    /// Unit X vector
    pub const X: Self = Self::new(1.0, 0.0, 0.0);

    /// Unit Y vector
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);

    /// Unit Z vector
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);

    /// Converts to array
    #[must_use]
    pub const fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    /// Creates from array
    #[must_use]
    pub const fn from_array(arr: [f32; 3]) -> Self {
        Self::new(arr[0], arr[1], arr[2])
    }

    /// Dot product
    #[must_use]
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Length squared (avoids sqrt)
    #[must_use]
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    /// Length
    #[must_use]
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    /// Distance to another point
    #[must_use]
    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }

    /// Distance squared (avoids sqrt)
    #[must_use]
    pub fn distance_squared(self, other: Self) -> f32 {
        (self - other).length_squared()
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

/// 2D Vector - UI positions, texture coords
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
pub struct Vec2 {
    /// X component
    pub x: f32,
    /// Y component
    pub y: f32,
}

impl Vec2 {
    /// Creates a new Vec2
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Zero vector
    pub const ZERO: Self = Self::new(0.0, 0.0);

    /// Converts to array
    #[must_use]
    pub const fn to_array(self) -> [f32; 2] {
        [self.x, self.y]
    }
}

/// Quaternion for rotations
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
pub struct Quaternion {
    /// X component
    pub x: f32,
    /// Y component
    pub y: f32,
    /// Z component
    pub z: f32,
    /// W component
    pub w: f32,
}

impl Quaternion {
    /// Creates a new quaternion
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Identity rotation
    pub const IDENTITY: Self = Self::new(0.0, 0.0, 0.0, 1.0);
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Transform - position + rotation + scale
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
pub struct Transform {
    /// Position
    pub position: Vec3,
    /// Scale (uniform)
    pub scale: f32,
    /// Rotation
    pub rotation: Quaternion,
}

impl Transform {
    /// Creates a new transform
    #[must_use]
    pub const fn new(position: Vec3, rotation: Quaternion, scale: f32) -> Self {
        Self { position, scale, rotation }
    }

    /// Identity transform
    pub const IDENTITY: Self = Self::new(Vec3::ZERO, Quaternion::IDENTITY, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec3_operations() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);

        let sum = a + b;
        assert_eq!(sum.x, 5.0);
        assert_eq!(sum.y, 7.0);
        assert_eq!(sum.z, 9.0);

        let dot = a.dot(b);
        assert_eq!(dot, 32.0); // 1*4 + 2*5 + 3*6
    }

    #[test]
    fn test_vec3_bytemuck() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let bytes: &[u8] = bytemuck::bytes_of(&v);
        assert_eq!(bytes.len(), 12); // 3 * 4 bytes
    }
}
