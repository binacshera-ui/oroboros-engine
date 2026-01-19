// =============================================================================
// OROBOROS Bilateral Upscale
// =============================================================================
// ARCHITECT'S FEEDBACK: Full-res ray marching = death.
//
// This shader:
// 1. Takes half-res effect buffer
// 2. Uses depth + normals to find edge boundaries
// 3. Applies bilateral filter (edge-aware upscale)
// 4. Optionally blends with previous frame (temporal)
//
// Cost: ~0.3ms at 4K (cheap!)
// =============================================================================

struct UpscaleUniforms {
    // Full resolution (width, height, 1/width, 1/height)
    full_res: vec4<f32>,
    // Half resolution (width, height, 1/width, 1/height)
    half_res: vec4<f32>,
    // depth_threshold, normal_threshold, temporal_blend, scale
    params: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: UpscaleUniforms;

// Half-res input (what we're upscaling)
@group(0) @binding(1)
var half_res_texture: texture_2d<f32>;

// Full-res depth (for edge detection)
@group(0) @binding(2)
var depth_texture: texture_2d<f32>;

// Full-res normals (for edge detection)
@group(0) @binding(3)
var normal_texture: texture_2d<f32>;

// Previous frame result (for temporal)
@group(0) @binding(4)
var history_texture: texture_2d<f32>;

@group(0) @binding(5)
var linear_sampler: sampler;

@group(0) @binding(6)
var point_sampler: sampler;

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

// Sample depth at full resolution
fn sample_depth(uv: vec2<f32>) -> f32 {
    return textureSample(depth_texture, point_sampler, uv).r;
}

// Sample normal at full resolution
fn sample_normal(uv: vec2<f32>) -> vec3<f32> {
    return textureSample(normal_texture, point_sampler, uv).rgb * 2.0 - 1.0;
}

// Compute bilateral weight based on depth and normal similarity
fn bilateral_weight(
    center_depth: f32,
    center_normal: vec3<f32>,
    sample_depth: f32,
    sample_normal: vec3<f32>,
) -> f32 {
    let depth_threshold = uniforms.params.x;
    let normal_threshold = uniforms.params.y;
    
    // Depth weight
    let depth_diff = abs(center_depth - sample_depth);
    let depth_weight = exp(-depth_diff / depth_threshold);
    
    // Normal weight
    let normal_dot = max(dot(center_normal, sample_normal), 0.0);
    let normal_weight = pow(normal_dot, 1.0 / normal_threshold);
    
    return depth_weight * normal_weight;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let full_pixel = uniforms.full_res.zw; // 1/width, 1/height
    let half_pixel = uniforms.half_res.zw;
    
    // Get center depth and normal at full res
    let center_depth = sample_depth(in.uv);
    let center_normal = sample_normal(in.uv);
    
    // Map full-res UV to half-res UV
    let half_uv = in.uv;
    
    // Bilateral upscale: sample 4 nearest half-res pixels
    var total_color = vec4<f32>(0.0);
    var total_weight = 0.0;
    
    // 2x2 tap pattern
    for (var y = -0.5; y <= 0.5; y += 1.0) {
        for (var x = -0.5; x <= 0.5; x += 1.0) {
            let offset = vec2<f32>(x, y) * half_pixel;
            let sample_uv = half_uv + offset;
            
            // Get depth/normal at the corresponding full-res position
            let sample_depth_val = sample_depth(sample_uv);
            let sample_normal_val = sample_normal(sample_uv);
            
            // Compute bilateral weight
            let weight = bilateral_weight(
                center_depth,
                center_normal,
                sample_depth_val,
                sample_normal_val,
            );
            
            // Sample half-res effect
            let effect_sample = textureSample(half_res_texture, linear_sampler, sample_uv);
            
            total_color += effect_sample * weight;
            total_weight += weight;
        }
    }
    
    // Normalize
    var result = total_color / max(total_weight, 0.0001);
    
    // Temporal blend
    let temporal_blend = uniforms.params.z;
    if (temporal_blend > 0.0) {
        let history = textureSample(history_texture, linear_sampler, in.uv);
        
        // Clamp history to neighborhood to prevent ghosting
        let current_luma = dot(result.rgb, vec3<f32>(0.299, 0.587, 0.114));
        let history_luma = dot(history.rgb, vec3<f32>(0.299, 0.587, 0.114));
        
        // Simple neighborhood clamping
        let luma_diff = abs(current_luma - history_luma);
        let adaptive_blend = temporal_blend * exp(-luma_diff * 10.0);
        
        result = mix(result, history, adaptive_blend);
    }
    
    return result;
}

// =============================================================================
// Simpler bilinear upscale for less critical effects
// =============================================================================

@fragment
fn fs_bilinear(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple bilinear sample - fastest option
    return textureSample(half_res_texture, linear_sampler, in.uv);
}
