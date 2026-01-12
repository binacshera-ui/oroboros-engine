// =============================================================================
// OROBOROS Screen-Space Reflections
// =============================================================================
// SQUAD NEON - Wet road reflections for Neon Prime
//
// Creates realistic reflections on wet surfaces using screen-space ray marching.
// Essential for the "rainy cyberpunk" aesthetic.
// =============================================================================

struct SSRUniforms {
    projection: mat4x4<f32>,
    inv_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    // width, height, 1/width, 1/height
    resolution: vec4<f32>,
    // max_ray_length, step_size, max_steps, thickness
    ray_config: vec4<f32>,
    // intensity, edge_fade, roughness_cutoff, wetness
    effect_params: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> ssr: SSRUniforms;

@group(0) @binding(1)
var color_texture: texture_2d<f32>;

@group(0) @binding(2)
var depth_texture: texture_2d<f32>;

@group(0) @binding(3)
var normal_texture: texture_2d<f32>;

@group(0) @binding(4)
var roughness_texture: texture_2d<f32>;

@group(0) @binding(5)
var linear_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    let x = f32((vertex_index & 1u) << 1u) - 1.0;
    let y = f32((vertex_index & 2u)) - 1.0;
    
    out.position = vec4<f32>(x, -y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, y * 0.5 + 0.5);
    
    return out;
}

// Linearize depth from NDC
fn linear_depth(ndc_depth: f32) -> f32 {
    let near = 0.1;
    let far = 1000.0;
    return near * far / (far - ndc_depth * (far - near));
}

// Get view-space position from UV and depth
fn view_pos_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let view = ssr.inv_projection * ndc;
    return view.xyz / view.w;
}

// Project view-space position to screen UV
fn project_to_uv(view_pos: vec3<f32>) -> vec3<f32> {
    let clip = ssr.projection * vec4<f32>(view_pos, 1.0);
    let ndc = clip.xyz / clip.w;
    return vec3<f32>(ndc.xy * 0.5 + 0.5, ndc.z);
}

// Decode normal from G-Buffer
fn decode_normal(packed: vec3<f32>) -> vec3<f32> {
    return normalize(packed * 2.0 - 1.0);
}

// Hash function for dithering
fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

// Binary search refinement for hit point
fn binary_search(
    ray_origin: vec3<f32>,
    ray_dir: vec3<f32>,
    t_min: f32,
    t_max: f32,
) -> vec3<f32> {
    var t_lo = t_min;
    var t_hi = t_max;
    
    for (var i = 0; i < 8; i++) {
        let t_mid = (t_lo + t_hi) * 0.5;
        let pos = ray_origin + ray_dir * t_mid;
        let projected = project_to_uv(pos);
        
        if (projected.x < 0.0 || projected.x > 1.0 || 
            projected.y < 0.0 || projected.y > 1.0) {
            break;
        }
        
        let scene_depth = textureSample(depth_texture, linear_sampler, projected.xy).r;
        let scene_z = view_pos_from_depth(projected.xy, scene_depth).z;
        
        if (pos.z < scene_z) {
            t_hi = t_mid;
        } else {
            t_lo = t_mid;
        }
    }
    
    return ray_origin + ray_dir * t_lo;
}

// Ray march in screen space
fn ray_march(
    ray_origin: vec3<f32>,
    ray_dir: vec3<f32>,
    dither: f32,
) -> vec4<f32> {
    let max_steps = u32(ssr.ray_config.z);
    let step_size = ssr.ray_config.y;
    let thickness = ssr.ray_config.w;
    let max_distance = ssr.ray_config.x;
    
    var t = step_size * dither;
    var prev_t = 0.0;
    var prev_z_diff = 0.0;
    
    for (var i = 0u; i < max_steps; i++) {
        let pos = ray_origin + ray_dir * t;
        let projected = project_to_uv(pos);
        
        // Check screen bounds
        if (projected.x < 0.0 || projected.x > 1.0 || 
            projected.y < 0.0 || projected.y > 1.0) {
            return vec4<f32>(0.0);
        }
        
        let scene_depth = textureSample(depth_texture, linear_sampler, projected.xy).r;
        
        // Skip sky
        if (scene_depth >= 1.0) {
            prev_t = t;
            t += step_size;
            continue;
        }
        
        let scene_z = view_pos_from_depth(projected.xy, scene_depth).z;
        let z_diff = pos.z - scene_z;
        
        // Hit detection
        if (z_diff > 0.0 && z_diff < thickness) {
            // Binary search for precise hit
            let hit_pos = binary_search(ray_origin, ray_dir, prev_t, t);
            let hit_uv = project_to_uv(hit_pos).xy;
            
            // Edge fade to prevent artifacts at screen borders
            let edge_fade = ssr.effect_params.y;
            var fade = 1.0;
            fade *= smoothstep(0.0, edge_fade, hit_uv.x);
            fade *= smoothstep(0.0, edge_fade, hit_uv.y);
            fade *= smoothstep(0.0, edge_fade, 1.0 - hit_uv.x);
            fade *= smoothstep(0.0, edge_fade, 1.0 - hit_uv.y);
            
            // Distance fade
            fade *= 1.0 - saturate(t / max_distance);
            
            return vec4<f32>(hit_uv, t, fade);
        }
        
        prev_z_diff = z_diff;
        prev_t = t;
        
        // Adaptive step size (larger steps for distant rays)
        t += step_size * (1.0 + t * 0.01);
        
        if (t > max_distance) {
            break;
        }
    }
    
    return vec4<f32>(0.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let original_color = textureSample(color_texture, linear_sampler, in.uv).rgb;
    let depth = textureSample(depth_texture, linear_sampler, in.uv).r;
    
    // Skip sky
    if (depth >= 1.0) {
        return vec4<f32>(original_color, 1.0);
    }
    
    let roughness = textureSample(roughness_texture, linear_sampler, in.uv).r;
    let roughness_cutoff = ssr.effect_params.z;
    
    // Skip rough surfaces
    if (roughness > roughness_cutoff) {
        return vec4<f32>(original_color, 1.0);
    }
    
    // Get view-space position and normal
    let view_pos = view_pos_from_depth(in.uv, depth);
    let world_normal = decode_normal(textureSample(normal_texture, linear_sampler, in.uv).rgb);
    let view_normal = (ssr.view * vec4<f32>(world_normal, 0.0)).xyz;
    
    // Calculate reflection ray
    let view_dir = normalize(view_pos);
    let reflect_dir = reflect(view_dir, view_normal);
    
    // Dither to reduce banding
    let dither = hash(in.uv * ssr.resolution.xy);
    
    // Ray march
    let hit = ray_march(view_pos, reflect_dir, dither);
    
    if (hit.w > 0.0) {
        let reflection = textureSample(color_texture, linear_sampler, hit.xy).rgb;
        
        // Roughness-based blur (approximate with mip level in production)
        let blur_factor = roughness * 0.3;
        
        // Fresnel effect - stronger reflections at grazing angles
        let fresnel = pow(1.0 - saturate(dot(-view_dir, view_normal)), 5.0);
        let reflection_strength = mix(0.04, 1.0, fresnel);
        
        // Apply intensity and fade
        let intensity = ssr.effect_params.x * hit.w * reflection_strength;
        let final_color = mix(original_color, reflection, intensity * (1.0 - roughness));
        
        return vec4<f32>(final_color, 1.0);
    }
    
    return vec4<f32>(original_color, 1.0);
}
