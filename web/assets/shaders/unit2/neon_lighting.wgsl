// =============================================================================
// OROBOROS Neon Lighting Shader
// =============================================================================
// SQUAD NEON - Dynamic lighting from neon signs
//
// Every neon sign is a real light source with:
// - Realistic attenuation
// - Flickering animation
// - Color bleeding into fog and surfaces
// =============================================================================

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    camera_pos: vec4<f32>,
    camera_params: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

// G-Buffer inputs
@group(1) @binding(0)
var gbuffer_albedo: texture_2d<f32>;
@group(1) @binding(1)
var gbuffer_normal: texture_2d<f32>;
@group(1) @binding(2)
var gbuffer_emission: texture_2d<f32>;
@group(1) @binding(3)
var gbuffer_depth: texture_2d<f32>;
@group(1) @binding(4)
var gbuffer_sampler: sampler;

// Neon light structure
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

@group(2) @binding(0)
var<storage, read> light_buffer: LightBuffer;

struct LightingUniforms {
    inv_view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    ambient_intensity: f32,
    exposure: f32,
}

@group(2) @binding(1)
var<uniform> lighting: LightingUniforms;

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

// Reconstruct world position from depth
fn world_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let world = lighting.inv_view_proj * ndc;
    return world.xyz / world.w;
}

// Decode normal from G-Buffer
fn decode_normal(packed: vec3<f32>) -> vec3<f32> {
    return normalize(packed * 2.0 - 1.0);
}

// GGX/Trowbridge-Reitz normal distribution
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h2 = n_dot_h * n_dot_h;
    
    let denom = n_dot_h2 * (a2 - 1.0) + 1.0;
    return a2 / (3.14159 * denom * denom);
}

// Schlick-GGX geometry function
fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

// Smith geometry function
fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}

// Fresnel-Schlick approximation
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}

// Calculate PBR lighting from a single light
fn calculate_light(
    light: NeonLight,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    view_dir: vec3<f32>,
    albedo: vec3<f32>,
    roughness: f32,
    metallic: f32,
    time: f32,
) -> vec3<f32> {
    let to_light = light.position - world_pos;
    let dist = length(to_light);
    
    if (dist > light.radius) {
        return vec3<f32>(0.0);
    }
    
    let light_dir = to_light / dist;
    let halfway = normalize(view_dir + light_dir);
    
    // Attenuation (quadratic falloff)
    let attenuation = pow(1.0 - saturate(dist / light.radius), 2.0);
    
    // Spotlight factor
    var spot_factor = 1.0;
    if (light.spot_angle > 0.0) {
        let spot_cos = dot(-light_dir, normalize(light.direction));
        let spot_threshold = cos(light.spot_angle);
        spot_factor = smoothstep(spot_threshold - 0.1, spot_threshold, spot_cos);
    }
    
    // Flicker animation
    var flicker = 1.0;
    if (light.flicker_speed > 0.0) {
        let t = time * light.flicker_speed + light.flicker_phase * 6.28318;
        // Multiple frequencies for realistic neon flicker
        flicker = 0.85 + 0.15 * sin(t) + 0.08 * sin(t * 2.3) + 0.05 * sin(t * 7.1);
        flicker = max(flicker, 0.0);
    }
    
    let radiance = light.color * light.intensity * attenuation * spot_factor * flicker;
    
    // PBR calculations
    let n_dot_l = max(dot(normal, light_dir), 0.0);
    let n_dot_v = max(dot(normal, view_dir), 0.001);
    let n_dot_h = max(dot(normal, halfway), 0.0);
    
    // Fresnel reflectance at normal incidence
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    
    // Cook-Torrance BRDF
    let d = distribution_ggx(n_dot_h, roughness);
    let g = geometry_smith(n_dot_v, n_dot_l, roughness);
    let f = fresnel_schlick(max(dot(halfway, view_dir), 0.0), f0);
    
    let numerator = d * g * f;
    let denominator = 4.0 * n_dot_v * n_dot_l + 0.0001;
    let specular = numerator / denominator;
    
    let ks = f;
    let kd = (vec3<f32>(1.0) - ks) * (1.0 - metallic);
    
    return (kd * albedo / 3.14159 + specular) * radiance * n_dot_l;
}

// ACES tonemapping
fn aces_tonemap(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return saturate((color * (a * color + b)) / (color * (c * color + d) + e));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample G-Buffer
    let albedo_roughness = textureSample(gbuffer_albedo, gbuffer_sampler, in.uv);
    let normal_metallic = textureSample(gbuffer_normal, gbuffer_sampler, in.uv);
    let emission = textureSample(gbuffer_emission, gbuffer_sampler, in.uv);
    let depth = textureSample(gbuffer_depth, gbuffer_sampler, in.uv).r;
    
    // Early exit for sky
    if (depth >= 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    
    // Unpack G-Buffer
    let albedo = albedo_roughness.rgb;
    let roughness = albedo_roughness.a;
    let normal = decode_normal(normal_metallic.rgb);
    let metallic = normal_metallic.a;
    
    // Reconstruct position
    let world_pos = world_from_depth(in.uv, depth);
    let view_dir = normalize(camera.camera_pos.xyz - world_pos);
    
    // Ambient light (very dark for cyberpunk atmosphere)
    var total_light = albedo * lighting.ambient_intensity * 0.02;
    
    // Add emission
    total_light += emission.rgb * emission.a;
    
    // Accumulate light from all neon sources
    for (var i = 0u; i < light_buffer.light_count; i++) {
        total_light += calculate_light(
            light_buffer.lights[i],
            world_pos,
            normal,
            view_dir,
            albedo,
            roughness,
            metallic,
            light_buffer.time,
        );
    }
    
    // Exposure and tonemapping
    total_light *= lighting.exposure;
    total_light = aces_tonemap(total_light);
    
    // Gamma correction
    total_light = pow(total_light, vec3<f32>(1.0 / 2.2));
    
    return vec4<f32>(total_light, 1.0);
}
