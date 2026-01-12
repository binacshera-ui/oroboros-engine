// =============================================================================
// OROBOROS Volumetric Fog Shader
// =============================================================================
// SQUAD NEON - Cyberpunk atmosphere for Neon Prime
//
// Creates depth and atmosphere with colored fog that responds to neon lights.
// Uses ray marching for physically-based light scattering.
// =============================================================================

struct FogUniforms {
    // RGB color + density
    color_density: vec4<f32>,
    // ground_level, height_falloff, scattering, neon_influence
    parameters: vec4<f32>,
    // ray_steps, max_distance, padding
    ray_config: vec4<f32>,
    // Camera position
    camera_pos: vec4<f32>,
    // Inverse view-projection matrix
    inv_view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> fog: FogUniforms;

@group(0) @binding(1)
var depth_texture: texture_2d<f32>;

@group(0) @binding(2)
var scene_texture: texture_2d<f32>;

@group(0) @binding(3)
var scene_sampler: sampler;

// Neon light structure (must match Rust NeonLight struct)
struct NeonLight {
    position: vec3<f32>,
    radius: f32,
    color: vec3<f32>,
    intensity: f32,
    direction: vec3<f32>,
    spot_angle: f32,
    flicker_phase: f32,
    flicker_speed: f32,
    _pad: vec2<f32>,
}

struct LightBuffer {
    lights: array<NeonLight, 256>,
    light_count: u32,
    time: f32,
    _pad: vec2<f32>,
}

@group(0) @binding(4)
var<storage, read> light_buffer: LightBuffer;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Full-screen triangle
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Generate full-screen triangle
    let x = f32((vertex_index & 1u) << 1u) - 1.0;
    let y = f32((vertex_index & 2u)) - 1.0;
    
    out.position = vec4<f32>(x, -y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, y * 0.5 + 0.5);
    
    return out;
}

// Reconstruct world position from depth
fn world_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let world = fog.inv_view_proj * ndc;
    return world.xyz / world.w;
}

// Height-based fog density
fn fog_density_at(pos: vec3<f32>) -> f32 {
    let ground = fog.parameters.x;
    let falloff = fog.parameters.y;
    let base_density = fog.color_density.w;
    
    let height = pos.y - ground;
    return base_density * exp(-height * falloff);
}

// Mie scattering phase function (light scattering in fog)
fn mie_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = 1.0 + g2 - 2.0 * g * cos_theta;
    return (1.0 - g2) / (4.0 * 3.14159 * pow(denom, 1.5));
}

// Calculate light contribution at a point
fn light_at_point(pos: vec3<f32>, view_dir: vec3<f32>) -> vec3<f32> {
    var total_light = vec3<f32>(0.0);
    let scattering = fog.parameters.z;
    let neon_influence = fog.parameters.w;
    let time = light_buffer.time;
    
    for (var i = 0u; i < light_buffer.light_count; i++) {
        let light = light_buffer.lights[i];
        
        let to_light = light.position - pos;
        let dist = length(to_light);
        
        if (dist > light.radius) {
            continue;
        }
        
        let light_dir = to_light / dist;
        
        // Attenuation
        let attenuation = 1.0 - (dist / light.radius);
        let attenuation2 = attenuation * attenuation;
        
        // Spotlight factor
        var spot_factor = 1.0;
        if (light.spot_angle > 0.0) {
            let spot_cos = dot(-light_dir, normalize(light.direction));
            let spot_threshold = cos(light.spot_angle);
            spot_factor = smoothstep(spot_threshold - 0.1, spot_threshold, spot_cos);
        }
        
        // Flicker effect
        var flicker = 1.0;
        if (light.flicker_speed > 0.0) {
            let flicker_t = time * light.flicker_speed + light.flicker_phase * 6.28318;
            flicker = 0.8 + 0.2 * sin(flicker_t) + 0.1 * sin(flicker_t * 3.7);
        }
        
        // Phase function for light scattering
        let cos_theta = dot(view_dir, light_dir);
        let phase = mie_phase(cos_theta, 0.5);
        
        let contribution = light.color * light.intensity * attenuation2 * 
                          spot_factor * flicker * phase * scattering * neon_influence;
        
        total_light += contribution;
    }
    
    return total_light;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let depth = textureSample(depth_texture, scene_sampler, in.uv).r;
    let scene_color = textureSample(scene_texture, scene_sampler, in.uv).rgb;
    
    // Skip sky (depth = 1.0)
    if (depth >= 1.0) {
        return vec4<f32>(scene_color, 1.0);
    }
    
    let world_pos = world_from_depth(in.uv, depth);
    let ray_start = fog.camera_pos.xyz;
    let ray_end = world_pos;
    let ray_dir = normalize(ray_end - ray_start);
    let ray_length = min(length(ray_end - ray_start), fog.ray_config.y);
    
    let steps = u32(fog.ray_config.x);
    let step_size = ray_length / f32(steps);
    
    var accumulated_fog = vec3<f32>(0.0);
    var accumulated_alpha = 0.0;
    
    // Ray march through fog
    for (var i = 0u; i < steps; i++) {
        let t = (f32(i) + 0.5) / f32(steps);
        let sample_pos = ray_start + ray_dir * t * ray_length;
        
        let density = fog_density_at(sample_pos);
        
        if (density > 0.001) {
            // Base fog color
            var fog_color = fog.color_density.rgb;
            
            // Add neon light contribution
            fog_color += light_at_point(sample_pos, ray_dir);
            
            // Accumulate fog
            let sample_alpha = density * step_size;
            accumulated_fog += fog_color * sample_alpha * (1.0 - accumulated_alpha);
            accumulated_alpha += sample_alpha * (1.0 - accumulated_alpha);
            
            // Early exit if fully opaque
            if (accumulated_alpha > 0.99) {
                break;
            }
        }
    }
    
    // Blend fog with scene
    let final_color = mix(scene_color, accumulated_fog, accumulated_alpha);
    
    return vec4<f32>(final_color, 1.0);
}
