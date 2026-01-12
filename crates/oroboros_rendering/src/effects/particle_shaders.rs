//! Particle System Shaders
//!
//! Contains WGSL source for the GPU particle system:
//! 1. Spawn Shader - Creates new particles from emitter data
//! 2. Update Shader - Physics simulation (position, velocity, lifetime)
//! 3. Compact Shader - Removes dead particles
//! 4. Render Shader - Draws particles as billboards
//!
//! ## ARCHITECT'S OVERDRAW SOLUTION
//!
//! Problem: 25,000 particles with alpha blending = GPU fill rate death
//!
//! Solution: **ADDITIVE BLENDING** (ONE + ONE)
//! - No sorting required (additive is commutative: A+B = B+A)
//! - No alpha test complexity
//! - Perfect for glowing effects (fire, neon, sparks)
//! - Aggressive early discard to minimize fragment writes
//!
//! Blend State Configuration:
//! ```text
//! color_blend:
//!   src_factor: ONE
//!   dst_factor: ONE
//!   operation: ADD
//!
//! alpha_blend:
//!   src_factor: ONE  
//!   dst_factor: ONE
//!   operation: ADD
//!
//! write_mask: ALL
//! ```

/// Blend mode for particle rendering
///
/// ARCHITECT'S LAW: Two render passes, two blend modes.
/// - Emissive Pass (fire, neon): Additive, no sorting
/// - Volumetric Pass (smoke, ash): Alpha, with sorting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleBlendMode {
    /// Additive blending (ONE + ONE) - for GLOWING effects only
    /// Best for: fire, sparks, neon, magic, explosions
    /// Overdraw safe: YES (no sorting needed)
    /// Render pass: EMISSIVE
    Additive,
    
    /// Classic alpha blending (SRC_ALPHA + ONE_MINUS_SRC_ALPHA)
    /// Best for: smoke, ash, fog, dust - things that BLOCK light
    /// Overdraw safe: NO - REQUIRES SORTING (back-to-front)
    /// Render pass: VOLUMETRIC
    ///
    /// WARNING: Black smoke with Additive = invisible!
    /// Dark particles MUST use this mode to occlude background.
    AlphaBlend,
    
    /// Pre-multiplied alpha (ONE + ONE_MINUS_SRC_ALPHA)
    /// Best for: decals, UI particles, mixed opacity
    /// Overdraw safe: PARTIAL
    /// Render pass: VOLUMETRIC
    Premultiplied,
}

impl ParticleBlendMode {
    /// Returns WGPU blend state for this mode
    #[must_use]
    pub const fn blend_state(&self) -> BlendStateConfig {
        match self {
            Self::Additive => BlendStateConfig {
                color_src: BlendFactor::One,
                color_dst: BlendFactor::One,
                color_op: BlendOp::Add,
                alpha_src: BlendFactor::One,
                alpha_dst: BlendFactor::One,
                alpha_op: BlendOp::Add,
            },
            Self::AlphaBlend => BlendStateConfig {
                color_src: BlendFactor::SrcAlpha,
                color_dst: BlendFactor::OneMinusSrcAlpha,
                color_op: BlendOp::Add,
                alpha_src: BlendFactor::SrcAlpha,
                alpha_dst: BlendFactor::OneMinusSrcAlpha,
                alpha_op: BlendOp::Add,
            },
            Self::Premultiplied => BlendStateConfig {
                color_src: BlendFactor::One,
                color_dst: BlendFactor::OneMinusSrcAlpha,
                color_op: BlendOp::Add,
                alpha_src: BlendFactor::One,
                alpha_dst: BlendFactor::OneMinusSrcAlpha,
                alpha_op: BlendOp::Add,
            },
        }
    }
    
    /// Returns true if this mode requires back-to-front sorting
    #[must_use]
    pub const fn requires_sorting(&self) -> bool {
        match self {
            Self::Additive => false,
            Self::AlphaBlend => true,
            Self::Premultiplied => true,
        }
    }
    
    /// Returns the render pass this mode belongs to
    #[must_use]
    pub const fn render_pass(&self) -> ParticleRenderPass {
        match self {
            Self::Additive => ParticleRenderPass::Emissive,
            Self::AlphaBlend => ParticleRenderPass::Volumetric,
            Self::Premultiplied => ParticleRenderPass::Volumetric,
        }
    }
}

/// Which render pass a particle effect belongs to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleRenderPass {
    /// Emissive pass - additive blending, no sorting
    /// Rendered FIRST (fire behind smoke)
    Emissive,
    /// Volumetric pass - alpha blending, sorted back-to-front
    /// Rendered SECOND (smoke in front of fire)
    Volumetric,
}

/// Blend factor (mirrors WGPU)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendFactor {
    /// 0
    Zero,
    /// 1  
    One,
    /// src.a
    SrcAlpha,
    /// 1 - src.a
    OneMinusSrcAlpha,
}

/// Blend operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendOp {
    /// src + dst
    Add,
}

/// Complete blend state configuration
#[derive(Debug, Clone, Copy)]
pub struct BlendStateConfig {
    /// Source factor for color
    pub color_src: BlendFactor,
    /// Destination factor for color
    pub color_dst: BlendFactor,
    /// Operation for color
    pub color_op: BlendOp,
    /// Source factor for alpha
    pub alpha_src: BlendFactor,
    /// Destination factor for alpha
    pub alpha_dst: BlendFactor,
    /// Operation for alpha
    pub alpha_op: BlendOp,
}

/// Depth testing strategy for particles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleDepthMode {
    /// Read depth, don't write (particles occluded by geometry, but don't occlude each other)
    /// Best for most particle effects
    ReadOnly,
    
    /// No depth testing (particles always visible)
    /// Use for screen-space effects only
    Disabled,
    
    /// Soft depth (fade based on depth difference)
    /// Requires depth texture sampling in shader
    SoftDepth,
}

/// Render configuration for particle systems
#[derive(Debug, Clone)]
pub struct ParticleRenderConfig {
    /// Blend mode for particles
    pub blend_mode: ParticleBlendMode,
    /// Depth testing mode
    pub depth_mode: ParticleDepthMode,
    /// LOD distance thresholds [full, 75%, 50%, 25%]
    pub lod_distances: [f32; 4],
    /// Minimum pixel size before culling
    pub min_pixel_size: f32,
    /// Maximum pixel size (overdraw cap)
    pub max_pixel_size: f32,
}

impl Default for ParticleRenderConfig {
    fn default() -> Self {
        Self {
            blend_mode: ParticleBlendMode::Additive,
            depth_mode: ParticleDepthMode::ReadOnly,
            lod_distances: [50.0, 100.0, 200.0, 400.0],
            min_pixel_size: 1.5,
            max_pixel_size: 256.0,
        }
    }
}

impl ParticleRenderConfig {
    /// High performance config (aggressive culling)
    #[must_use]
    pub fn high_performance() -> Self {
        Self {
            blend_mode: ParticleBlendMode::Additive,
            depth_mode: ParticleDepthMode::ReadOnly,
            lod_distances: [25.0, 50.0, 100.0, 200.0],
            min_pixel_size: 2.0,
            max_pixel_size: 128.0,
        }
    }
    
    /// Quality config (less culling, more particles visible)
    #[must_use]
    pub fn high_quality() -> Self {
        Self {
            blend_mode: ParticleBlendMode::Additive,
            depth_mode: ParticleDepthMode::ReadOnly,
            lod_distances: [100.0, 200.0, 400.0, 800.0],
            min_pixel_size: 0.5,
            max_pixel_size: 512.0,
        }
    }
    
    /// Config for fire/explosion effects
    #[must_use]
    pub fn fire() -> Self {
        Self {
            blend_mode: ParticleBlendMode::Additive,
            depth_mode: ParticleDepthMode::ReadOnly,
            lod_distances: [50.0, 100.0, 200.0, 400.0],
            min_pixel_size: 1.0,
            max_pixel_size: 256.0,
        }
    }
    
    /// Config for smoke/ash/fog effects (alpha blended, SORTED)
    ///
    /// WARNING: This mode REQUIRES back-to-front sorting!
    /// Dark particles block light - they cannot use additive.
    #[must_use]
    pub fn smoke() -> Self {
        Self {
            blend_mode: ParticleBlendMode::AlphaBlend,
            depth_mode: ParticleDepthMode::SoftDepth,
            lod_distances: [25.0, 50.0, 100.0, 150.0],
            min_pixel_size: 2.0,
            max_pixel_size: 512.0,
        }
    }
    
    /// Config for volcanic ash (Inferno world)
    ///
    /// Dark, heavy ash that OCCLUDES the fiery glow beneath.
    /// Uses alpha blend with aggressive sorting.
    #[must_use]
    pub fn volcanic_ash() -> Self {
        Self {
            blend_mode: ParticleBlendMode::AlphaBlend,
            depth_mode: ParticleDepthMode::SoftDepth,
            lod_distances: [50.0, 100.0, 200.0, 300.0],
            min_pixel_size: 3.0,
            max_pixel_size: 400.0,
        }
    }
}

/// Container for all particle system shaders
pub struct ParticleShaders;

impl ParticleShaders {
    /// Returns the particle spawn compute shader source
    #[must_use]
    pub fn spawn_shader() -> &'static str {
        PARTICLE_SPAWN_WGSL
    }
    
    /// Returns the particle update compute shader source
    #[must_use]
    pub fn update_shader() -> &'static str {
        PARTICLE_UPDATE_WGSL
    }
    
    /// Returns the particle render vertex shader source
    #[must_use]
    pub fn render_vertex_shader() -> &'static str {
        PARTICLE_RENDER_VERTEX_WGSL
    }
    
    /// Returns the particle render fragment shader source
    #[must_use]
    pub fn render_fragment_shader() -> &'static str {
        PARTICLE_RENDER_FRAGMENT_WGSL
    }
}

/// Particle spawn compute shader
const PARTICLE_SPAWN_WGSL: &str = r#"
// Particle Spawn Compute Shader
// Spawns new particles from emitter commands

struct Particle {
    position_age: vec4<f32>,      // xyz = position, w = age (0-1)
    velocity_lifetime: vec4<f32>, // xyz = velocity, w = lifetime (seconds)
    color_start: vec4<f32>,       // rgba
    color_end: vec4<f32>,         // rgba
    size_emission: vec4<f32>,     // start, end, current, emission
    flags: vec4<u32>,             // alive, emitter_id, seed, custom
}

struct EmitterData {
    position_count: vec4<f32>,           // xyz = pos, w = count as bits
    velocity_min_lifetime_min: vec4<f32>,
    velocity_max_lifetime_max: vec4<f32>,
    color_start: vec4<f32>,
    color_end: vec4<f32>,
    size_params: vec4<f32>,              // start, end, emission, gravity
    physics_params: vec4<f32>,           // drag, turbulence, distortion, screen_space
    meta: vec4<u32>,                     // seed, emitter_id, flags, _
}

struct SpawnUniforms {
    time: f32,
    dt: f32,
    emitter_count: u32,
    free_slot_count: u32,
}

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<storage, read> emitters: array<EmitterData>;
@group(0) @binding(2) var<storage, read> free_slots: array<u32>;
@group(0) @binding(3) var<uniform> uniforms: SpawnUniforms;

// PCG random number generator
fn pcg(input: u32) -> u32 {
    let state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn random_float(seed: u32) -> f32 {
    return f32(pcg(seed)) / 4294967295.0;
}

fn random_range(seed: u32, min_val: f32, max_val: f32) -> f32 {
    return min_val + random_float(seed) * (max_val - min_val);
}

fn random_vec3_range(seed: u32, min_v: vec3<f32>, max_v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        random_range(seed, min_v.x, max_v.x),
        random_range(seed + 1u, min_v.y, max_v.y),
        random_range(seed + 2u, min_v.z, max_v.z)
    );
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    
    // Iterate through emitters and spawn particles
    var particle_offset = 0u;
    
    for (var e = 0u; e < uniforms.emitter_count; e++) {
        let emitter = emitters[e];
        let spawn_count = bitcast<u32>(emitter.position_count.w);
        
        // Check if this thread should spawn a particle for this emitter
        if idx >= particle_offset && idx < particle_offset + spawn_count {
            let local_idx = idx - particle_offset;
            
            // Find a free slot
            if local_idx < uniforms.free_slot_count {
                let slot = free_slots[local_idx];
                
                // Generate unique seed for this particle
                let seed = emitter.meta.x ^ (local_idx * 1664525u) ^ bitcast<u32>(uniforms.time);
                
                // Spawn particle
                var p: Particle;
                
                // Random position spread (small jitter around emitter)
                let jitter = random_vec3_range(seed, vec3<f32>(-0.1), vec3<f32>(0.1));
                p.position_age = vec4<f32>(
                    emitter.position_count.xyz + jitter,
                    0.0  // age = 0 (just born)
                );
                
                // Random velocity in range
                let velocity = random_vec3_range(
                    seed + 100u,
                    emitter.velocity_min_lifetime_min.xyz,
                    emitter.velocity_max_lifetime_max.xyz
                );
                let lifetime = random_range(
                    seed + 200u,
                    emitter.velocity_min_lifetime_min.w,
                    emitter.velocity_max_lifetime_max.w
                );
                p.velocity_lifetime = vec4<f32>(velocity, lifetime);
                
                // Colors
                p.color_start = emitter.color_start;
                p.color_end = emitter.color_end;
                
                // Size and emission
                p.size_emission = vec4<f32>(
                    emitter.size_params.x,  // start size
                    emitter.size_params.y,  // end size
                    emitter.size_params.x,  // current size (starts at start)
                    emitter.size_params.z   // emission intensity
                );
                
                // Flags: alive, emitter_id, seed, physics packed
                let gravity_bits = bitcast<u32>(emitter.size_params.w);
                let drag_bits = bitcast<u32>(emitter.physics_params.x);
                p.flags = vec4<u32>(
                    1u,                    // alive
                    emitter.meta.y,        // emitter_id
                    seed,                  // random seed for turbulence
                    gravity_bits           // gravity in w
                );
                
                particles[slot] = p;
            }
        }
        
        particle_offset += spawn_count;
    }
}
"#;

/// Particle update compute shader
const PARTICLE_UPDATE_WGSL: &str = r#"
// Particle Update Compute Shader
// Physics simulation for all particles

struct Particle {
    position_age: vec4<f32>,
    velocity_lifetime: vec4<f32>,
    color_start: vec4<f32>,
    color_end: vec4<f32>,
    size_emission: vec4<f32>,
    flags: vec4<u32>,
}

struct UpdateUniforms {
    time: f32,
    dt: f32,
    particle_count: u32,
    _pad: u32,
    camera_pos: vec4<f32>,
    gravity_default: f32,
    drag_default: f32,
    turbulence_strength: f32,
    _pad2: f32,
}

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<uniform> uniforms: UpdateUniforms;
@group(0) @binding(2) var<storage, read_write> alive_count: atomic<u32>;

// Simplex noise for turbulence
fn hash(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    
    return mix(
        mix(
            mix(hash(i + vec3<f32>(0.0, 0.0, 0.0)), hash(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash(i + vec3<f32>(0.0, 1.0, 0.0)), hash(i + vec3<f32>(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(hash(i + vec3<f32>(0.0, 0.0, 1.0)), hash(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash(i + vec3<f32>(0.0, 1.0, 1.0)), hash(i + vec3<f32>(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

fn turbulence(p: vec3<f32>, seed: f32) -> vec3<f32> {
    let offset = vec3<f32>(seed * 0.1, seed * 0.2, seed * 0.3);
    return vec3<f32>(
        noise(p + offset) - 0.5,
        noise(p + offset + vec3<f32>(100.0, 0.0, 0.0)) - 0.5,
        noise(p + offset + vec3<f32>(0.0, 100.0, 0.0)) - 0.5
    ) * 2.0;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    
    if idx >= uniforms.particle_count {
        return;
    }
    
    var p = particles[idx];
    
    // Skip dead particles
    if p.flags.x == 0u {
        return;
    }
    
    let dt = uniforms.dt;
    let lifetime = p.velocity_lifetime.w;
    
    // Update age
    let new_age = p.position_age.w + dt / lifetime;
    
    // Kill if too old
    if new_age >= 1.0 {
        p.flags.x = 0u;
        particles[idx] = p;
        return;
    }
    
    // Physics update
    var velocity = p.velocity_lifetime.xyz;
    
    // Gravity (stored in flags.w as bits)
    let gravity = bitcast<f32>(p.flags.w);
    velocity.y -= gravity * dt;
    
    // Drag
    let drag = uniforms.drag_default;
    velocity *= 1.0 - drag * dt;
    
    // Turbulence (based on position and time)
    if uniforms.turbulence_strength > 0.0 {
        let turb = turbulence(
            p.position_age.xyz * 0.5 + vec3<f32>(uniforms.time * 0.5),
            f32(p.flags.z) * 0.001
        );
        velocity += turb * uniforms.turbulence_strength * dt;
    }
    
    // Update position
    let new_pos = p.position_age.xyz + velocity * dt;
    
    // Interpolate size
    let t = new_age;
    let current_size = mix(p.size_emission.x, p.size_emission.y, t);
    
    // Store updated particle
    p.position_age = vec4<f32>(new_pos, new_age);
    p.velocity_lifetime = vec4<f32>(velocity, lifetime);
    p.size_emission.z = current_size;
    
    particles[idx] = p;
    
    // Count alive particles
    atomicAdd(&alive_count, 1u);
}
"#;

/// Particle render vertex shader
///
/// OVERDRAW OPTIMIZATIONS:
/// 1. Dead particle culling (degenerate triangle)
/// 2. Distance-based LOD (smaller particles far away = less fill)
/// 3. Sub-pixel culling (don't render particles smaller than 1 pixel)
/// 4. Screen-space size limiting (cap max screen size)
const PARTICLE_RENDER_VERTEX_WGSL: &str = r#"
// Particle Render Vertex Shader
// Creates billboarded quads with aggressive LOD

struct Particle {
    position_age: vec4<f32>,
    velocity_lifetime: vec4<f32>,
    color_start: vec4<f32>,
    color_end: vec4<f32>,
    size_emission: vec4<f32>,
    flags: vec4<u32>,
}

struct CameraUniforms {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    camera_right: vec4<f32>,
    camera_up: vec4<f32>,
    // Screen dimensions for pixel culling (width, height, 1/width, 1/height)
    screen_params: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) emission: f32,
}

@group(0) @binding(0) var<storage, read> particles: array<Particle>;
@group(0) @binding(1) var<uniform> camera: CameraUniforms;

// Quad vertices (2 triangles)
const QUAD_POSITIONS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-0.5, -0.5),
    vec2<f32>(0.5, -0.5),
    vec2<f32>(0.5, 0.5),
    vec2<f32>(-0.5, -0.5),
    vec2<f32>(0.5, 0.5),
    vec2<f32>(-0.5, 0.5),
);

const QUAD_UVS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 0.0),
);

// LOD thresholds (distance in world units)
const LOD_DISTANCE_1: f32 = 50.0;   // Full detail
const LOD_DISTANCE_2: f32 = 100.0;  // 75% size
const LOD_DISTANCE_3: f32 = 200.0;  // 50% size
const LOD_DISTANCE_4: f32 = 400.0;  // 25% size - beyond this, cull

// Minimum screen-space size in pixels (below this, cull entirely)
const MIN_PIXEL_SIZE: f32 = 1.5;

// Maximum screen-space size in pixels (cap to prevent overdraw monsters)
const MAX_PIXEL_SIZE: f32 = 256.0;

@vertex
fn main(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32,
) -> VertexOutput {
    var out: VertexOutput;
    
    let particle = particles[instance_idx];
    
    // === OPTIMIZATION 1: Dead particle culling ===
    if particle.flags.x == 0u {
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        out.uv = vec2<f32>(0.0);
        out.color = vec4<f32>(0.0);
        out.emission = 0.0;
        return out;
    }
    
    // Get particle properties
    let world_pos = particle.position_age.xyz;
    let age = particle.position_age.w;
    var size = particle.size_emission.z;
    
    // === OPTIMIZATION 2: Distance-based LOD ===
    let to_camera = camera.camera_pos.xyz - world_pos;
    let distance_sq = dot(to_camera, to_camera);
    let distance = sqrt(distance_sq);
    
    // Apply LOD scale factor based on distance
    var lod_scale = 1.0;
    if distance > LOD_DISTANCE_4 {
        // Beyond max LOD - cull entirely
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        out.uv = vec2<f32>(0.0);
        out.color = vec4<f32>(0.0);
        out.emission = 0.0;
        return out;
    } else if distance > LOD_DISTANCE_3 {
        lod_scale = 0.25;
    } else if distance > LOD_DISTANCE_2 {
        lod_scale = 0.5;
    } else if distance > LOD_DISTANCE_1 {
        lod_scale = 0.75;
    }
    
    size *= lod_scale;
    
    // === OPTIMIZATION 3: Screen-space size calculation ===
    // Approximate screen size in pixels
    let proj_scale = camera.proj[1][1];  // Vertical FOV factor
    let screen_height = camera.screen_params.y;
    let screen_size_pixels = (size * proj_scale * screen_height) / distance;
    
    // Cull sub-pixel particles
    if screen_size_pixels < MIN_PIXEL_SIZE {
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        out.uv = vec2<f32>(0.0);
        out.color = vec4<f32>(0.0);
        out.emission = 0.0;
        return out;
    }
    
    // === OPTIMIZATION 4: Cap maximum screen size ===
    if screen_size_pixels > MAX_PIXEL_SIZE {
        let scale_down = MAX_PIXEL_SIZE / screen_size_pixels;
        size *= scale_down;
    }
    
    // === Billboard creation ===
    let quad_idx = vertex_idx % 6u;
    let quad_pos = QUAD_POSITIONS[quad_idx];
    
    let right = camera.camera_right.xyz;
    let up = camera.camera_up.xyz;
    
    let vertex_pos = world_pos 
        + right * quad_pos.x * size 
        + up * quad_pos.y * size;
    
    out.position = camera.view_proj * vec4<f32>(vertex_pos, 1.0);
    out.uv = QUAD_UVS[quad_idx];
    
    // Interpolate color based on age
    // Also fade out based on LOD for smoother transitions
    var color = mix(particle.color_start, particle.color_end, age);
    color.a *= lod_scale;  // Fade with LOD
    
    out.color = color;
    out.emission = particle.size_emission.w * lod_scale;
    
    return out;
}
"#;

/// Particle render fragment shader
///
/// ARCHITECT'S WARNING: NO ALPHA SORTING.
/// Uses ADDITIVE BLENDING to avoid overdraw death.
/// Blend mode: ONE + ONE (src + dst)
const PARTICLE_RENDER_FRAGMENT_WGSL: &str = r#"
// Particle Render Fragment Shader
// ADDITIVE BLENDING - No sorting required, commutative operation
// Perfect for glowing effects: fire, neon, sparks, explosions
//
// Blend State (set in pipeline):
//   color: src=ONE, dst=ONE, op=ADD
//   alpha: src=ONE, dst=ONE, op=ADD
//
// This means: final = existing_pixel + new_pixel
// No overdraw cost from alpha sorting!

struct FragmentInput {
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) emission: f32,
}

@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    // Soft circular particle with aggressive falloff
    let center = vec2<f32>(0.5);
    let dist = distance(in.uv, center);
    
    // Aggressive early discard to reduce fill rate
    // Particles outside radius 0.5 are invisible
    if dist > 0.5 {
        discard;
    }
    
    // Soft edge with exponential falloff (more aggressive than smoothstep)
    // This reduces the "bright core" problem with additive blending
    let falloff = 1.0 - dist * 2.0;  // 0 at edge, 1 at center
    let intensity = falloff * falloff;  // Quadratic falloff
    
    // Very aggressive discard for near-zero contributions
    // This is CRITICAL for overdraw - don't write pixels that won't be seen
    if intensity < 0.02 {
        discard;
    }
    
    // For additive blending, output is just color * intensity
    // The blend hardware does: framebuffer += output
    let final_color = in.color.rgb * intensity * in.color.a * in.emission;
    
    // Output pre-multiplied for additive blend
    // Alpha channel is ignored in ONE+ONE blending, but we output intensity
    // for potential soft particle depth testing
    return vec4<f32>(final_color, intensity);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_shader_sources_not_empty() {
        assert!(!ParticleShaders::spawn_shader().is_empty());
        assert!(!ParticleShaders::update_shader().is_empty());
        assert!(!ParticleShaders::render_vertex_shader().is_empty());
        assert!(!ParticleShaders::render_fragment_shader().is_empty());
    }
}
