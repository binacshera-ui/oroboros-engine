//! GPU Compute Particle System
//!
//! ARCHITECT'S MANDATE: 10,000 particles, GPU-only, SAME FRAME
//!
//! Architecture:
//! 1. CPU pushes spawn commands to a ring buffer
//! 2. Compute Shader 1: Spawns new particles (reads ring buffer)
//! 3. Compute Shader 2: Updates all particles (physics)
//! 4. Compute Shader 3: Culls dead particles (compaction)
//! 5. Vertex/Fragment: Renders surviving particles
//!
//! CPU work per frame: Push spawn commands (~64 bytes per emitter)
//! GPU work per frame: Everything else

use bytemuck::{Pod, Zeroable};

/// Maximum particles in the system
pub const MAX_PARTICLES: usize = 1_000_000;
/// Maximum concurrent emitters
pub const MAX_EMITTERS: usize = 256;

/// A single particle (lives entirely on GPU)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct Particle {
    /// Position (xyz) + age (w, 0-1 normalized)
    pub position_age: [f32; 4],
    /// Velocity (xyz) + lifetime (w, in seconds)
    pub velocity_lifetime: [f32; 4],
    /// Color start (rgba)
    pub color_start: [f32; 4],
    /// Color end (rgba)
    pub color_end: [f32; 4],
    /// Size (start, end, current, emission)
    pub size_emission: [f32; 4],
    /// Flags (alive, emitter_id, random_seed, custom)
    pub flags: [u32; 4],
}

impl Particle {
    /// Size of a particle in bytes
    pub const SIZE: usize = std::mem::size_of::<Self>();
    
    /// Creates a dead particle (used for pool initialization)
    #[must_use]
    pub const fn dead() -> Self {
        Self {
            position_age: [0.0, 0.0, 0.0, 1.0], // age=1 means dead
            velocity_lifetime: [0.0; 4],
            color_start: [0.0; 4],
            color_end: [0.0; 4],
            size_emission: [0.0; 4],
            flags: [0; 4], // flags[0]=0 means dead
        }
    }
    
    /// Is this particle alive?
    #[inline]
    #[must_use]
    pub const fn is_alive(&self) -> bool {
        self.flags[0] != 0
    }
}

/// Emitter types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmitterType {
    /// All particles spawn at once
    Burst,
    /// Particles spawn over time
    Stream {
        /// Duration in seconds
        duration: f32,
    },
    /// Continuous emission
    Continuous,
}

/// Configuration for particle spawning
#[derive(Debug, Clone)]
pub struct ParticleConfig {
    /// Number of particles to spawn
    pub spawn_count: u32,
    /// Minimum lifetime in seconds
    pub lifetime_min: f32,
    /// Maximum lifetime in seconds
    pub lifetime_max: f32,
    /// Minimum initial velocity
    pub velocity_min: [f32; 3],
    /// Maximum initial velocity
    pub velocity_max: [f32; 3],
    /// Starting color (RGBA)
    pub color_start: [f32; 4],
    /// Ending color (RGBA)
    pub color_end: [f32; 4],
    /// Starting size
    pub size_start: f32,
    /// Ending size
    pub size_end: f32,
    /// Emission intensity (for bloom)
    pub emission_intensity: f32,
    /// Gravity (negative = down)
    pub gravity: f32,
    /// Drag coefficient
    pub drag: f32,
    /// Turbulence strength
    pub turbulence: f32,
    /// Spawn rate (particles per second, 0 = burst)
    pub spawn_rate: f32,
    /// Is this in screen space?
    pub is_screen_space: bool,
    /// Screen distortion amount
    pub distortion: f32,
}

impl Default for ParticleConfig {
    fn default() -> Self {
        Self {
            spawn_count: 100,
            lifetime_min: 1.0,
            lifetime_max: 2.0,
            velocity_min: [-1.0, 0.0, -1.0],
            velocity_max: [1.0, 5.0, 1.0],
            color_start: [1.0, 1.0, 1.0, 1.0],
            color_end: [1.0, 1.0, 1.0, 0.0],
            size_start: 0.1,
            size_end: 0.01,
            emission_intensity: 1.0,
            gravity: -9.8,
            drag: 0.1,
            turbulence: 0.0,
            spawn_rate: 0.0,
            is_screen_space: false,
            distortion: 0.0,
        }
    }
}

/// GPU-side emitter data
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct GPUEmitterData {
    /// Position (xyz) + spawn_count (w as u32 bits)
    pub position_count: [f32; 4],
    /// Velocity min (xyz) + lifetime_min (w)
    pub velocity_min_lifetime_min: [f32; 4],
    /// Velocity max (xyz) + lifetime_max (w)
    pub velocity_max_lifetime_max: [f32; 4],
    /// Color start (rgba)
    pub color_start: [f32; 4],
    /// Color end (rgba)
    pub color_end: [f32; 4],
    /// Size (start, end, emission, gravity)
    pub size_params: [f32; 4],
    /// Physics (drag, turbulence, distortion, is_screen_space)
    pub physics_params: [f32; 4],
    /// Random seed + emitter_id + flags
    pub meta: [u32; 4],
}

impl GPUEmitterData {
    /// Creates GPU data from a particle config
    #[must_use]
    pub fn from_config(
        position: [f32; 3],
        config: &ParticleConfig,
        emitter_id: u32,
        random_seed: u32,
    ) -> Self {
        Self {
            position_count: [
                position[0],
                position[1],
                position[2],
                f32::from_bits(config.spawn_count),
            ],
            velocity_min_lifetime_min: [
                config.velocity_min[0],
                config.velocity_min[1],
                config.velocity_min[2],
                config.lifetime_min,
            ],
            velocity_max_lifetime_max: [
                config.velocity_max[0],
                config.velocity_max[1],
                config.velocity_max[2],
                config.lifetime_max,
            ],
            color_start: config.color_start,
            color_end: config.color_end,
            size_params: [
                config.size_start,
                config.size_end,
                config.emission_intensity,
                config.gravity,
            ],
            physics_params: [
                config.drag,
                config.turbulence,
                config.distortion,
                if config.is_screen_space { 1.0 } else { 0.0 },
            ],
            meta: [random_seed, emitter_id, 0, 0],
        }
    }
}

/// A particle emitter (CPU side)
#[derive(Debug, Clone)]
pub struct ParticleEmitter {
    /// World position
    pub position: [f32; 3],
    /// Configuration
    pub config: ParticleConfig,
    /// Emitter type
    pub emitter_type: EmitterType,
    /// Unique ID
    id: u32,
    /// Time alive
    age: f32,
    /// Is this emitter still active?
    alive: bool,
}

impl ParticleEmitter {
    /// Creates a new emitter
    #[must_use]
    pub fn new(position: [f32; 3], config: ParticleConfig, emitter_type: EmitterType) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        
        Self {
            position,
            config,
            emitter_type,
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            age: 0.0,
            alive: true,
        }
    }
    
    /// Updates the emitter, returns true if still alive
    pub fn update(&mut self, dt: f32) -> bool {
        if !self.alive {
            return false;
        }
        
        self.age += dt;
        
        match self.emitter_type {
            EmitterType::Burst => {
                // Burst emitters die after spawning
                self.alive = false;
            }
            EmitterType::Stream { duration } => {
                if self.age >= duration {
                    self.alive = false;
                }
            }
            EmitterType::Continuous => {
                // Lives forever
            }
        }
        
        self.alive
    }
    
    /// Returns the emitter ID
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id
    }
    
    /// Is this emitter alive?
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.alive
    }
    
    /// Converts to GPU data
    #[must_use]
    pub fn to_gpu_data(&self, random_seed: u32) -> GPUEmitterData {
        GPUEmitterData::from_config(self.position, &self.config, self.id, random_seed)
    }
}

/// Statistics from the particle system
#[derive(Debug, Clone, Copy, Default)]
pub struct ParticleStats {
    /// Total particles in pool
    pub pool_size: u32,
    /// Currently alive particles
    pub alive_count: u32,
    /// Particles spawned this frame
    pub spawned_this_frame: u32,
    /// Particles died this frame
    pub died_this_frame: u32,
    /// Active emitters
    pub active_emitters: u32,
    /// GPU compute time (microseconds)
    pub compute_time_us: u32,
}

/// The main GPU particle system
pub struct ParticleSystem {
    /// Pre-allocated particle pool (conceptual, actual data on GPU)
    pool_size: usize,
    /// Active emitters
    emitters: Vec<ParticleEmitter>,
    /// Pending spawn commands
    spawn_queue: Vec<GPUEmitterData>,
    /// Current statistics
    stats: ParticleStats,
    /// Global simulation time
    time: f32,
    /// Delta time accumulator
    dt_accumulator: f32,
}

impl ParticleSystem {
    /// Creates a new particle system
    #[must_use]
    pub fn new(max_particles: usize) -> Self {
        Self {
            pool_size: max_particles,
            emitters: Vec::with_capacity(MAX_EMITTERS),
            spawn_queue: Vec::with_capacity(MAX_EMITTERS),
            stats: ParticleStats {
                pool_size: max_particles as u32,
                ..Default::default()
            },
            time: 0.0,
            dt_accumulator: 0.0,
        }
    }
    
    /// Adds an emitter to the system
    pub fn add_emitter(&mut self, emitter: ParticleEmitter) {
        if self.emitters.len() < MAX_EMITTERS {
            self.spawn_queue.push(emitter.to_gpu_data(self.random_seed()));
            self.stats.spawned_this_frame += emitter.config.spawn_count;
            self.emitters.push(emitter);
        }
    }
    
    /// Adds multiple emitters at once
    pub fn add_emitters(&mut self, emitters: Vec<ParticleEmitter>) {
        for emitter in emitters {
            self.add_emitter(emitter);
        }
    }
    
    /// Updates the particle system
    ///
    /// Call this once per frame BEFORE rendering.
    /// Returns the spawn commands to send to the GPU.
    pub fn update(&mut self, dt: f32) -> &[GPUEmitterData] {
        self.time += dt;
        self.dt_accumulator += dt;
        
        // Reset per-frame stats
        self.stats.spawned_this_frame = 0;
        self.stats.died_this_frame = 0;
        self.spawn_queue.clear();
        
        // Update emitters and collect spawn commands
        let mut dead_count = 0;
        let mut spawn_commands = Vec::new();
        
        for emitter in &mut self.emitters {
            if !emitter.update(dt) {
                dead_count += 1;
            } else if emitter.emitter_type == EmitterType::Continuous 
                || matches!(emitter.emitter_type, EmitterType::Stream { .. }) 
            {
                // Stream/continuous emitters spawn particles over time
                let spawn_count = (emitter.config.spawn_rate * dt) as u32;
                if spawn_count > 0 {
                    let mut spawn_config = emitter.config.clone();
                    spawn_config.spawn_count = spawn_count;
                    spawn_commands.push((emitter.position, spawn_config, emitter.id()));
                }
            }
        }
        
        // Now process spawn commands outside the borrow
        for (position, config, emitter_id) in spawn_commands {
            let gpu_data = GPUEmitterData::from_config(
                position,
                &config,
                emitter_id,
                self.random_seed(),
            );
            self.stats.spawned_this_frame += config.spawn_count;
            self.spawn_queue.push(gpu_data);
        }
        
        // Remove dead emitters
        self.emitters.retain(|e| e.is_alive());
        self.stats.died_this_frame = dead_count as u32;
        self.stats.active_emitters = self.emitters.len() as u32;
        
        &self.spawn_queue
    }
    
    /// Returns spawn commands as bytes for GPU upload
    #[must_use]
    pub fn spawn_commands_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.spawn_queue)
    }
    
    /// Returns the number of pending spawn commands
    #[must_use]
    pub fn spawn_command_count(&self) -> u32 {
        self.spawn_queue.len() as u32
    }
    
    /// Returns current statistics
    #[must_use]
    pub fn stats(&self) -> ParticleStats {
        self.stats
    }
    
    /// Updates stats from GPU readback
    pub fn update_stats_from_gpu(&mut self, alive_count: u32, compute_time_us: u32) {
        self.stats.alive_count = alive_count;
        self.stats.compute_time_us = compute_time_us;
    }
    
    /// Clears all emitters
    pub fn clear(&mut self) {
        self.emitters.clear();
        self.spawn_queue.clear();
        self.stats = ParticleStats {
            pool_size: self.pool_size as u32,
            ..Default::default()
        };
    }
    
    /// Returns the current simulation time
    #[must_use]
    pub fn time(&self) -> f32 {
        self.time
    }
    
    /// Generates a pseudo-random seed for particle variation
    fn random_seed(&self) -> u32 {
        // Simple LCG based on time
        let seed = (self.time * 1000.0) as u32;
        seed.wrapping_mul(1664525).wrapping_add(1013904223)
    }
}

impl Default for ParticleSystem {
    fn default() -> Self {
        Self::new(MAX_PARTICLES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_particle_size() {
        // Must be aligned for GPU
        assert_eq!(Particle::SIZE, 96);
        assert_eq!(Particle::SIZE % 16, 0);
    }
    
    #[test]
    fn test_emitter_burst() {
        let mut emitter = ParticleEmitter::new(
            [0.0, 0.0, 0.0],
            ParticleConfig::default(),
            EmitterType::Burst,
        );
        
        // Burst emitters die immediately after update
        assert!(emitter.is_alive());
        emitter.update(0.016);
        assert!(!emitter.is_alive());
    }
    
    #[test]
    fn test_emitter_stream() {
        let mut emitter = ParticleEmitter::new(
            [0.0, 0.0, 0.0],
            ParticleConfig::default(),
            EmitterType::Stream { duration: 1.0 },
        );
        
        // Stream emitters live for their duration
        emitter.update(0.5);
        assert!(emitter.is_alive());
        emitter.update(0.6);
        assert!(!emitter.is_alive());
    }
    
    #[test]
    fn test_system_spawn_queue() {
        let mut system = ParticleSystem::new(10000);
        
        system.add_emitter(ParticleEmitter::new(
            [1.0, 2.0, 3.0],
            ParticleConfig {
                spawn_count: 1000,
                ..Default::default()
            },
            EmitterType::Burst,
        ));
        
        let commands = system.update(0.016);
        assert_eq!(commands.len(), 0); // Burst already added to queue before update
        
        // After one update, burst emitter is dead
        assert_eq!(system.stats().active_emitters, 0);
    }
    
    #[test]
    fn test_gpu_emitter_data_size() {
        // 7 vec4<f32> + 1 vec4<u32> = 8 * 16 = 128 bytes
        let size = std::mem::size_of::<GPUEmitterData>();
        assert_eq!(size, 128);
        assert_eq!(size % 16, 0); // Must be 16-byte aligned
    }
}
