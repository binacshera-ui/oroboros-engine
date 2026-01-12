// =============================================================================
// OROBOROS Voxel Instanced Renderer - VISUAL UPGRADE
// =============================================================================
// SQUAD NEON - GPU-bound voxel rendering
// 
// MANDATE: 1 million voxels @ 120 FPS @ 4K
// CONSTRAINT: Single draw call for all visible geometry
//
// VISUAL UPGRADE v2.0:
// - Vertex-based Ambient Occlusion (AO)
// - Per-material procedural noise texturing
// - Multi-octave noise for natural look
// - Enhanced lighting with soft shadows
// =============================================================================

// Camera uniforms
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    camera_pos: vec4<f32>,
    // Near, far, aspect, fov
    camera_params: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

// Per-instance data (matches VoxelInstance struct in Rust)
// uv_offset_scale.xy = UV offset
// uv_offset_scale.z = AO value (0-1, computed per-face)
// uv_offset_scale.w = reserved
struct InstanceInput {
    @location(5) position_scale: vec4<f32>,
    @location(6) dimensions_normal_material: vec4<f32>,
    @location(7) color: vec4<f32>,        // RGB color + emission intensity
    @location(8) uv_offset_scale: vec4<f32>,
}

// Vertex output with AO
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material_id: f32,
    @location(4) color: vec4<f32>,
    @location(5) ao: f32,              // Ambient occlusion value
    @location(6) local_uv: vec2<f32>,  // UV within single face (0-1)
}

// =============================================================================
// NOISE FUNCTIONS - Multi-octave Simplex-like noise
// =============================================================================

// Hash function for procedural noise
fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, vec3<f32>(p3.y + 33.33, p3.z + 33.33, p3.x + 33.33));
    return fract((p3.x + p3.y) * p3.z);
}

fn hash31(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 = p3 + dot(p3, vec3<f32>(p3.y + 33.33, p3.z + 33.33, p3.x + 33.33));
    return fract((p3.x + p3.y) * p3.z);
}

// Value noise with smooth interpolation
fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    
    // Smooth Hermite interpolation
    let u = f * f * (3.0 - 2.0 * f);
    
    let a = hash21(i + vec2<f32>(0.0, 0.0));
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// 3D value noise for volumetric effects
fn value_noise_3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    
    return mix(
        mix(
            mix(hash31(i + vec3<f32>(0.0, 0.0, 0.0)), hash31(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash31(i + vec3<f32>(0.0, 1.0, 0.0)), hash31(i + vec3<f32>(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(hash31(i + vec3<f32>(0.0, 0.0, 1.0)), hash31(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash31(i + vec3<f32>(0.0, 1.0, 1.0)), hash31(i + vec3<f32>(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

// Fractional Brownian Motion (multi-octave noise)
fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pos = p;
    
    for (var i = 0; i < octaves; i = i + 1) {
        value = value + amplitude * value_noise(pos * frequency);
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }
    
    return value;
}

fn fbm_3d(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pos = p;
    
    for (var i = 0; i < octaves; i = i + 1) {
        value = value + amplitude * value_noise_3d(pos * frequency);
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }
    
    return value;
}

// =============================================================================
// NORMAL & BASIS FUNCTIONS
// =============================================================================

fn get_normal(index: u32) -> vec3<f32> {
    switch index {
        case 0u: { return vec3<f32>(1.0, 0.0, 0.0); }   // +X
        case 1u: { return vec3<f32>(-1.0, 0.0, 0.0); }  // -X
        case 2u: { return vec3<f32>(0.0, 1.0, 0.0); }   // +Y (top)
        case 3u: { return vec3<f32>(0.0, -1.0, 0.0); }  // -Y (bottom)
        case 4u: { return vec3<f32>(0.0, 0.0, 1.0); }   // +Z
        case 5u: { return vec3<f32>(0.0, 0.0, -1.0); }  // -Z
        default: { return vec3<f32>(0.0, 1.0, 0.0); }
    }
}

fn get_tangent(normal_index: u32) -> vec3<f32> {
    switch normal_index {
        case 0u, 1u: { return vec3<f32>(0.0, 1.0, 0.0); }  // X faces: Y tangent
        case 2u, 3u: { return vec3<f32>(1.0, 0.0, 0.0); }  // Y faces: X tangent
        case 4u, 5u: { return vec3<f32>(1.0, 0.0, 0.0); }  // Z faces: X tangent
        default: { return vec3<f32>(1.0, 0.0, 0.0); }
    }
}

fn get_bitangent(normal_index: u32) -> vec3<f32> {
    switch normal_index {
        case 0u, 1u: { return vec3<f32>(0.0, 0.0, 1.0); }  // X faces: Z bitangent
        case 2u, 3u: { return vec3<f32>(0.0, 0.0, 1.0); }  // Y faces: Z bitangent
        case 4u, 5u: { return vec3<f32>(0.0, 1.0, 0.0); }  // Z faces: Y bitangent
        default: { return vec3<f32>(0.0, 1.0, 0.0); }
    }
}

// Quad vertices (2 triangles, CCW winding)
fn get_quad_vertex(index: u32) -> vec2<f32> {
    switch index {
        case 0u: { return vec2<f32>(0.0, 0.0); }
        case 1u: { return vec2<f32>(1.0, 0.0); }
        case 2u: { return vec2<f32>(1.0, 1.0); }
        case 3u: { return vec2<f32>(0.0, 0.0); }
        case 4u: { return vec2<f32>(1.0, 1.0); }
        case 5u: { return vec2<f32>(0.0, 1.0); }
        default: { return vec2<f32>(0.0, 0.0); }
    }
}

// Per-vertex AO lookup based on corner position
// AO is pre-computed on CPU and packed into uv_offset_scale.z
fn get_vertex_ao(instance_ao: f32, local_uv: vec2<f32>) -> f32 {
    // If AO value is provided from CPU, use it
    // Otherwise compute basic corner darkening
    if instance_ao > 0.0 {
        return instance_ao;
    }
    
    // Fallback: smooth corner darkening
    let corner_dist = min(
        min(local_uv.x, 1.0 - local_uv.x),
        min(local_uv.y, 1.0 - local_uv.y)
    );
    return smoothstep(0.0, 0.15, corner_dist);
}

// =============================================================================
// VERTEX SHADER
// =============================================================================

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    
    // Extract instance data
    let position = instance.position_scale.xyz;
    let scale = instance.position_scale.w;
    let width = instance.dimensions_normal_material.x;
    let height = instance.dimensions_normal_material.y;
    let normal_index = u32(instance.dimensions_normal_material.z);
    let material_id = instance.dimensions_normal_material.w;
    let instance_ao = instance.uv_offset_scale.z;
    
    // Get basis vectors for this face
    let normal = get_normal(normal_index);
    let tangent = get_tangent(normal_index);
    let bitangent = get_bitangent(normal_index);
    
    // Get local vertex position on quad (0-1 range)
    let local_uv = get_quad_vertex(vertex_index);
    
    // Scale to quad size
    let scaled_uv = vec2<f32>(local_uv.x * width, local_uv.y * height);
    
    // Calculate world position
    let world_pos = position + tangent * scaled_uv.x * scale + bitangent * scaled_uv.y * scale;
    
    // Calculate AO based on corner position
    let ao = get_vertex_ao(instance_ao, local_uv);
    
    // Transform to clip space
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_position = world_pos;
    out.normal = normal;
    out.uv = local_uv * vec2<f32>(width, height);
    out.local_uv = local_uv;
    out.material_id = material_id;
    out.color = instance.color;
    out.ao = ao;
    
    return out;
}

// =============================================================================
// FOREST PALETTE - Gloomy Natural Terrain
// =============================================================================

// Material IDs:
// 0 = Air (unused)
// 1 = Grass (forest floor, mossy)
// 2 = Dirt (rich brown soil)
// 3 = Stone (cool gray with cracks)
// 4 = Wood (tree bark - vertical grain)
// 5 = Bedrock (dark foundation)
// 255 = Neon (special FX)

fn get_grass_color(world_pos: vec3<f32>, base: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    // Forest floor grass - mossy, varied greens
    let noise1 = fbm(world_pos.xz * 0.3, 4);
    let noise2 = fbm(world_pos.xz * 1.5 + vec2<f32>(50.0, 50.0), 3);
    let moss_noise = fbm(world_pos.xz * 0.8, 2);
    
    // Dark forest green base
    let dark_green = vec3<f32>(0.08, 0.18, 0.06);
    // Mossy yellow-green highlights
    let moss_green = vec3<f32>(0.2, 0.35, 0.1);
    // Muddy patches
    let muddy = vec3<f32>(0.15, 0.12, 0.08);
    
    var color = mix(dark_green, moss_green, noise1);
    
    // Add moss patches
    color = mix(color, moss_green * 1.2, moss_noise * 0.4);
    
    // Muddy spots near dirt transitions
    if noise2 > 0.7 {
        color = mix(color, muddy, (noise2 - 0.7) * 2.0);
    }
    
    // Top face (looking up at sky) slightly brighter
    if normal.y > 0.5 {
        color = color * 1.15;
    } else {
        // Side faces darker (like grass sides/dirt showing through)
        color = mix(color, base * 0.6, 0.5);
    }
    
    return color;
}

fn get_dirt_color(world_pos: vec3<f32>, base: vec3<f32>) -> vec3<f32> {
    // Rich forest soil with roots and pebbles
    let noise1 = fbm_3d(world_pos * 0.6, 4);
    let noise2 = value_noise(world_pos.xz * 4.0);
    let root_noise = fbm(world_pos.xz * 2.0, 2);
    
    // Dark loamy soil
    let dark_soil = vec3<f32>(0.18, 0.12, 0.06);
    // Reddish-brown clay
    let clay = vec3<f32>(0.35, 0.18, 0.1);
    
    var color = mix(dark_soil, clay, noise1);
    
    // Occasional roots (darker streaks)
    if root_noise < 0.2 {
        color = color * 0.6;
    }
    
    // Small pebbles (lighter spots)
    if noise2 > 0.9 {
        color = color * 1.4 + vec3<f32>(0.1, 0.08, 0.06);
    }
    
    return color;
}

fn get_stone_color(world_pos: vec3<f32>, base: vec3<f32>) -> vec3<f32> {
    // Cool gray stone with moss and cracks
    let noise1 = fbm_3d(world_pos * 0.25, 4);
    let crack_noise = fbm(world_pos.xy * 3.0, 3);
    let moss_noise = fbm(world_pos.xz * 0.5, 2);
    
    // Blue-gray stone
    let cold_stone = vec3<f32>(0.35, 0.38, 0.45);
    // Warm gray highlights
    let warm_stone = vec3<f32>(0.5, 0.48, 0.45);
    // Moss on stone
    let stone_moss = vec3<f32>(0.2, 0.3, 0.15);
    
    var color = mix(cold_stone, warm_stone, noise1);
    
    // Deep cracks
    if crack_noise < 0.12 {
        color = color * 0.4;
    } else if crack_noise < 0.2 {
        color = color * 0.7;
    }
    
    // Moss growing on exposed stone
    let moss_amount = smoothstep(0.5, 0.8, moss_noise);
    color = mix(color, stone_moss, moss_amount * 0.3);
    
    return color;
}

fn get_sand_color(world_pos: vec3<f32>, base: vec3<f32>) -> vec3<f32> {
    // Forest path / sandy soil - wood-like grain for tree bark
    let noise1 = fbm(world_pos.xz * 0.3, 3);
    let grain = fbm(vec2<f32>(world_pos.x * 0.5, world_pos.y * 2.0), 4); // Vertical grain
    
    // Tree bark brown
    let dark_bark = vec3<f32>(0.25, 0.15, 0.08);
    let light_bark = vec3<f32>(0.45, 0.32, 0.2);
    
    var color = mix(dark_bark, light_bark, noise1);
    
    // Vertical bark texture
    color = color * (0.8 + grain * 0.4);
    
    return color;
}

fn get_bedrock_color(world_pos: vec3<f32>, base: vec3<f32>) -> vec3<f32> {
    // Deep underground stone - almost black
    let noise = fbm_3d(world_pos * 0.4, 4);
    let crystal = value_noise(world_pos.xz * 6.0);
    
    // Very dark with slight blue tint
    let deep_rock = vec3<f32>(0.08, 0.08, 0.12);
    
    var color = deep_rock * (0.7 + noise * 0.6);
    
    // Rare mineral sparkles
    if crystal > 0.97 {
        color = color + vec3<f32>(0.1, 0.15, 0.2);
    }
    
    return color;
}

fn get_neon_color(world_pos: vec3<f32>, base: vec3<f32>, emission: f32) -> vec3<f32> {
    // Magical forest glow (fireflies, mushrooms)
    let pulse = sin(world_pos.y * 3.0 + world_pos.x * 1.5) * 0.5 + 0.5;
    let energy = fbm(world_pos.xz * 1.5, 2);
    
    // Bioluminescent green instead of cyan
    let glow_color = vec3<f32>(0.3, 1.0, 0.4);
    
    var color = glow_color * (0.6 + emission * 0.4);
    color = color * (0.7 + pulse * 0.5 + energy * 0.3);
    
    return color;
}

// =============================================================================
// MAIN FRAGMENT SHADER - VISUAL UPGRADE
// =============================================================================

// NOTE: This is a HELPER function, NOT an entry point.
// Entry points (@fragment) cannot be called from other functions in WGSL.
fn fs_main_visual(in: VertexOutput) -> vec4<f32> {
    let base_color = in.color.rgb;
    let emission = in.color.a;
    let material_id = u32(in.material_id);
    
    // Get material-specific procedural color
    var textured_color: vec3<f32>;
    
    switch material_id {
        case 1u: { // Grass
            textured_color = get_grass_color(in.world_position, base_color, in.normal);
        }
        case 2u: { // Dirt
            textured_color = get_dirt_color(in.world_position, base_color);
        }
        case 3u: { // Stone
            textured_color = get_stone_color(in.world_position, base_color);
        }
        case 4u: { // Sand
            textured_color = get_sand_color(in.world_position, base_color);
        }
        case 5u: { // Bedrock
            textured_color = get_bedrock_color(in.world_position, base_color);
        }
        case 255u: { // Neon
            textured_color = get_neon_color(in.world_position, base_color, emission);
        }
        default: {
            // Generic noise for unknown materials
            let noise = fbm_3d(in.world_position * 0.5, 2);
            textured_color = base_color * (0.85 + noise * 0.3);
        }
    }
    
    // =========================================================================
    // FOREST LIGHTING - Sun through trees
    // =========================================================================
    
    // Sun position: (50, 100, 50) normalized = high afternoon sun
    let sun_dir = normalize(vec3<f32>(50.0, 100.0, 50.0));
    // Warm golden sunlight filtering through leaves
    let sun_color = vec3<f32>(1.0, 0.92, 0.75);
    
    // Forest ambient (blue sky filtered through canopy)
    let sky_color = vec3<f32>(0.35, 0.45, 0.55); // Muted blue
    let ground_color = vec3<f32>(0.1, 0.08, 0.06); // Dark forest floor
    let sky_factor = in.normal.y * 0.5 + 0.5;
    let ambient_color = mix(ground_color, sky_color, sky_factor);
    
    // Diffuse lighting
    let ndotl = max(dot(in.normal, sun_dir), 0.0);
    
    // Soft wrap lighting for less harsh shadows
    let wrap = 0.3;
    let wrapped_diffuse = max((ndotl + wrap) / (1.0 + wrap), 0.0);
    
    // =========================================================================
    // AMBIENT OCCLUSION
    // =========================================================================
    
    // Vertex AO (interpolated across face)
    let vertex_ao = in.ao;
    
    // Edge AO (darken corners and edges)
    let edge_ao = smoothstep(0.0, 0.1, in.local_uv.x) * 
                  smoothstep(0.0, 0.1, in.local_uv.y) *
                  smoothstep(0.0, 0.1, 1.0 - in.local_uv.x) *
                  smoothstep(0.0, 0.1, 1.0 - in.local_uv.y);
    
    // Combine AO sources
    let combined_ao = vertex_ao * (0.7 + edge_ao * 0.3);
    
    // Face-direction AO boost (bottom faces darker)
    let face_ao = select(1.0, 0.75, in.normal.y < -0.5);
    
    let final_ao = combined_ao * face_ao;
    
    // =========================================================================
    // FINAL COLOR COMPOSITION
    // =========================================================================
    
    // Ambient contribution (affected by AO)
    let ambient = ambient_color * 0.35 * final_ao;
    
    // Diffuse contribution
    let diffuse = sun_color * wrapped_diffuse * 0.65;
    
    // Combine lighting
    var lit_color = textured_color * (ambient + diffuse);
    
    // Apply AO to final result for extra depth
    lit_color = lit_color * (0.5 + final_ao * 0.5);
    
    // =========================================================================
    // EMISSION (for Neon blocks)
    // =========================================================================
    
    if emission > 0.5 {
        // Emissive glow bypasses lighting
        let glow_strength = emission;
        lit_color = lit_color + textured_color * glow_strength;
    }
    
    // =========================================================================
    // FOREST ATMOSPHERIC EFFECTS
    // =========================================================================
    
    // Forest mist - starts closer, blends to sky blue
    let dist = length(in.world_position - camera.camera_pos.xyz);
    let fog_start = 80.0;   // Fog begins at 80m
    let fog_end = 550.0;    // Matches far plane - 50m buffer
    let fog_factor = clamp((dist - fog_start) / (fog_end - fog_start), 0.0, 0.85);
    
    // Misty blue-gray fog (matches sky)
    let fog_color = vec3<f32>(0.45, 0.55, 0.7);
    
    let final_color = mix(lit_color, fog_color, fog_factor);
    
    // Desaturate distant objects (atmospheric perspective)
    let gray = dot(final_color, vec3<f32>(0.299, 0.587, 0.114));
    let desaturated = mix(final_color, vec3<f32>(gray), fog_factor * 0.4);
    
    return vec4<f32>(desaturated, 1.0);
}

// =============================================================================
// FRAGMENT ENTRY POINT (The ONLY callable entry point for main rendering)
// =============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return fs_main_visual(in);
}

// =============================================================================
// WIREFRAME SHADER (for selection highlight)
// =============================================================================

@fragment
fn fs_wireframe(in: VertexOutput) -> @location(0) vec4<f32> {
    let edge_width = 0.03;
    let uv_frac = in.local_uv;
    
    // Smooth edge detection
    let edge = smoothstep(0.0, edge_width, uv_frac.x) * 
               smoothstep(0.0, edge_width, uv_frac.y) *
               smoothstep(0.0, edge_width, 1.0 - uv_frac.x) *
               smoothstep(0.0, edge_width, 1.0 - uv_frac.y);
    
    let base_color = in.color.rgb;
    let emission = in.color.a;
    
    // Selection highlight: bright outline
    var wire_color = vec3<f32>(1.0, 1.0, 0.3); // Yellow
    if emission > 1.5 {
        wire_color = base_color * 2.0; // Use block color for high emission
    }
    
    // Transparent fill for selection
    let fill_alpha = 0.1;
    let wire_alpha = 1.0 - edge;
    
    let color = mix(base_color * 0.3, wire_color, wire_alpha);
    let alpha = fill_alpha + wire_alpha * 0.9;
    
    return vec4<f32>(color, alpha);
}

// =============================================================================
// LEGACY SHADERS (reimplemented - cannot call entry points in WGSL)
// =============================================================================

@fragment
fn fs_solid(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple solid color with forest lighting
    let base_color = in.color.rgb;
    let sun_dir = normalize(vec3<f32>(50.0, 100.0, 50.0));
    let ndotl = max(dot(in.normal, sun_dir), 0.0);
    let ambient = 0.25;
    let lit_color = base_color * (ambient + ndotl * 0.75) * in.ao;
    return vec4<f32>(lit_color, 1.0);
}

@fragment
fn fs_textured(in: VertexOutput) -> @location(0) vec4<f32> {
    // Same as solid for now (forest lighting)
    let base_color = in.color.rgb;
    let sun_dir = normalize(vec3<f32>(50.0, 100.0, 50.0));
    let ndotl = max(dot(in.normal, sun_dir), 0.0);
    let ambient = 0.25;
    let lit_color = base_color * (ambient + ndotl * 0.75) * in.ao;
    return vec4<f32>(lit_color, 1.0);
}
