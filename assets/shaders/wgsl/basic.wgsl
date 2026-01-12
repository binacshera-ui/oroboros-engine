// =============================================================================
// OROBOROS BETA RELEASE SHADER
// =============================================================================
// Professional shader with proper lighting and fog effects.
// Matrix convention: Column-major (WGSL native)
// Coordinate system: Right-handed, Y-up
// =============================================================================

struct CameraUniform {
    view_proj: mat4x4<f32>,     // 64 bytes @ offset 0
    camera_pos: vec3<f32>,      // 12 bytes @ offset 64
    // _pad0: 4 bytes implicit
    time: f32,                   // 4 bytes @ offset 80
    // _pad1: 12 bytes implicit for 16-byte alignment
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) view_dist: f32,
};

// =============================================================================
// VERTEX SHADER
// =============================================================================
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform to clip space
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    
    // Pass through data for fragment shader
    out.color = in.color;
    out.world_pos = in.position;
    out.view_dist = length(in.position - camera.camera_pos);
    
    return out;
}

// =============================================================================
// FRAGMENT SHADER
// =============================================================================
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Compute face normal from derivatives
    let dx = dpdx(in.world_pos);
    let dy = dpdy(in.world_pos);
    let normal = normalize(cross(dx, dy));
    
    // Directional light (sun-like, from above-right)
    let light_dir = normalize(vec3<f32>(0.3, 0.8, 0.2));
    let ndotl = max(dot(normal, light_dir), 0.0);
    
    // Ambient + Diffuse lighting
    let ambient = 0.25;
    let diffuse = ndotl * 0.75;
    let lit_color = in.color * (ambient + diffuse);
    
    // Distance fog
    let fog_start = 100.0;
    let fog_end = 500.0;
    let fog_factor = clamp((in.view_dist - fog_start) / (fog_end - fog_start), 0.0, 0.85);
    let fog_color = vec3<f32>(0.05, 0.08, 0.12);
    
    // Final color with fog
    let final_color = mix(lit_color, fog_color, fog_factor);
    
    return vec4<f32>(final_color, 1.0);
}
