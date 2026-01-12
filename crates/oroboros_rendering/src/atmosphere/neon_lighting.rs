//! Dynamic neon lighting system.
//!
//! Every neon sign in the city is a real light source that affects
//! the fog, reflections, and nearby surfaces.

use bytemuck::{Pod, Zeroable};

/// Maximum number of neon lights per frame.
/// Kept low for GPU efficiency - use clustering for more lights.
pub const MAX_NEON_LIGHTS: usize = 256;

/// A single neon light source.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct NeonLight {
    /// Position in world space.
    pub position: [f32; 3],
    /// Light radius.
    pub radius: f32,
    /// Light color (HDR, can exceed 1.0).
    pub color: [f32; 3],
    /// Light intensity.
    pub intensity: f32,
    /// Direction for spotlights (0 = point light).
    pub direction: [f32; 3],
    /// Spotlight angle (radians, 0 = point light).
    pub spot_angle: f32,
    /// Flicker phase (for animated signs).
    pub flicker_phase: f32,
    /// Flicker speed.
    pub flicker_speed: f32,
    /// Padding for alignment.
    pub _pad: [f32; 2],
}

impl NeonLight {
    /// Creates a point light.
    #[must_use]
    pub fn point(position: [f32; 3], color: [f32; 3], radius: f32, intensity: f32) -> Self {
        Self {
            position,
            radius,
            color,
            intensity,
            direction: [0.0, 0.0, 0.0],
            spot_angle: 0.0,
            flicker_phase: 0.0,
            flicker_speed: 0.0,
            _pad: [0.0; 2],
        }
    }
    
    /// Creates a flickering neon sign light.
    #[must_use]
    pub fn neon_sign(
        position: [f32; 3],
        color: [f32; 3],
        radius: f32,
        intensity: f32,
        flicker_speed: f32,
    ) -> Self {
        Self {
            position,
            radius,
            color,
            intensity,
            direction: [0.0, 0.0, 0.0],
            spot_angle: 0.0,
            flicker_phase: 0.0,
            flicker_speed,
            _pad: [0.0; 2],
        }
    }
    
    /// Creates a spotlight.
    #[must_use]
    pub fn spot(
        position: [f32; 3],
        direction: [f32; 3],
        color: [f32; 3],
        radius: f32,
        intensity: f32,
        angle: f32,
    ) -> Self {
        Self {
            position,
            radius,
            color,
            intensity,
            direction,
            spot_angle: angle,
            flicker_phase: 0.0,
            flicker_speed: 0.0,
            _pad: [0.0; 2],
        }
    }
    
    /// Standard neon colors.
    pub const PINK: [f32; 3] = [1.0, 0.2, 0.6];
    /// Cyan neon.
    pub const CYAN: [f32; 3] = [0.2, 0.9, 1.0];
    /// Purple neon.
    pub const PURPLE: [f32; 3] = [0.6, 0.2, 1.0];
    /// Gold neon (for winning effects).
    pub const GOLD: [f32; 3] = [1.0, 0.8, 0.2];
    /// Red neon (for danger/loss).
    pub const RED: [f32; 3] = [1.0, 0.1, 0.1];
    /// Green neon (for profit).
    pub const GREEN: [f32; 3] = [0.2, 1.0, 0.3];
}

/// Light buffer for GPU upload.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct NeonLightBuffer {
    /// Active lights.
    pub lights: [NeonLight; MAX_NEON_LIGHTS],
    /// Number of active lights.
    pub light_count: u32,
    /// Current time for flicker animation.
    pub time: f32,
    /// Padding.
    pub _pad: [f32; 2],
}

impl Default for NeonLightBuffer {
    fn default() -> Self {
        Self {
            lights: [NeonLight::default(); MAX_NEON_LIGHTS],
            light_count: 0,
            time: 0.0,
            _pad: [0.0; 2],
        }
    }
}

/// Neon lighting manager.
pub struct NeonLighting {
    /// Buffer for GPU upload.
    buffer: NeonLightBuffer,
    /// Current time accumulator.
    time: f32,
}

impl NeonLighting {
    /// Creates a new neon lighting system.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: NeonLightBuffer::default(),
            time: 0.0,
        }
    }
    
    /// Returns the WGSL source for the neon lighting shader.
    #[must_use]
    pub fn shader_source() -> &'static str {
        include_str!("../../shaders/neon_lighting.wgsl")
    }
    
    /// Clears all lights for new frame.
    pub fn begin_frame(&mut self, delta_time: f32) {
        self.time += delta_time;
        self.buffer.light_count = 0;
        self.buffer.time = self.time;
    }
    
    /// Adds a light to the buffer.
    ///
    /// Returns false if buffer is full.
    pub fn add_light(&mut self, mut light: NeonLight) -> bool {
        if self.buffer.light_count as usize >= MAX_NEON_LIGHTS {
            return false;
        }
        
        // Initialize flicker phase from position hash for variation
        if light.flicker_speed > 0.0 {
            light.flicker_phase = (light.position[0] * 12.9898 
                + light.position[1] * 78.233 
                + light.position[2] * 45.164)
                .sin()
                .abs();
        }
        
        self.buffer.lights[self.buffer.light_count as usize] = light;
        self.buffer.light_count += 1;
        true
    }
    
    /// Returns the buffer for GPU upload.
    #[must_use]
    pub fn buffer(&self) -> &NeonLightBuffer {
        &self.buffer
    }
    
    /// Returns the buffer as bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(&self.buffer)
    }
    
    /// Returns the number of active lights.
    #[must_use]
    pub fn light_count(&self) -> u32 {
        self.buffer.light_count
    }
}

impl Default for NeonLighting {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_light_creation() {
        let light = NeonLight::point([0.0, 10.0, 0.0], NeonLight::PINK, 20.0, 5.0);
        assert_eq!(light.radius, 20.0);
        assert_eq!(light.spot_angle, 0.0); // Point light
    }
    
    #[test]
    fn test_buffer_capacity() {
        let mut lighting = NeonLighting::new();
        lighting.begin_frame(0.016);
        
        // Fill to capacity
        for i in 0..MAX_NEON_LIGHTS {
            let pos = [i as f32, 0.0, 0.0];
            assert!(lighting.add_light(NeonLight::point(pos, NeonLight::CYAN, 10.0, 1.0)));
        }
        
        // Should fail when full
        assert!(!lighting.add_light(NeonLight::point([0.0, 0.0, 0.0], NeonLight::RED, 10.0, 1.0)));
    }
}
