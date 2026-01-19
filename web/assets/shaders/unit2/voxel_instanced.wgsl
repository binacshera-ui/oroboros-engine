// =============================================================================
// OROBOROS Terrain Vertex Shader
// =============================================================================
// INDUSTRIAL STANDARD - Vertex + Index Buffer Rendering
// 
// MANDATE: 1 million voxels @ 120 FPS @ 4K
// METHOD: Standard vertex buffer with indexed drawing
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

// =============================================================================
// NEW: Standard Vertex Input (matches TerrainVertex in Rust)
// =============================================================================
struct VertexInput {
    @location(0) position: vec3<f32>,      // World position
    @location(1) normal: vec3<f32>,        // Face normal
    @location(2) uv: vec2<f32>,            // Texture UV
    @location(3) material_ao: vec4<f32>,   // [material_id, ao, 0, 0]
}

// Vertex output to fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material_id: f32,
    @location(4) ao: f32,
}

// =============================================================================
// NEW: Standard Vertex Shader (draw_indexed compatible)
// =============================================================================
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Direct vertex position - no instance transformation needed
    let world_pos = in.position;
    
    // Transform to clip space
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_position = world_pos;
    out.normal = in.normal;
    out.uv = in.uv;
    out.material_id = in.material_ao.x;
    out.ao = in.material_ao.y;
    
    return out;
}

// =============================================================================
// NOTE: Texture-based rendering (fs_main) disabled
// Reason: @group(1) bindings require texture setup in Rust
// Using fs_solid for now - calculates colors from material_id
// =============================================================================

// =============================================================================
// SOLID COLOR SHADER - INDUSTRIAL STANDARD
// =============================================================================
// Uses material_id from vertex buffer to determine color
// AO comes from vertex data (calculated by block-mesh-rs)

@fragment
fn fs_solid(in: VertexOutput) -> @location(0) vec4<f32> {
    let mat_id = u32(in.material_id);
    
    // Forest Palette Colors based on material_id
    var material_color = vec3<f32>(0.5, 0.5, 0.5); // Default gray
    
    if mat_id == 1u { // Grass
        material_color = vec3<f32>(0.22, 0.55, 0.18);
    } else if mat_id == 2u { // Dirt
        material_color = vec3<f32>(0.45, 0.30, 0.15);
    } else if mat_id == 3u { // Stone
        material_color = vec3<f32>(0.40, 0.40, 0.42);
    } else if mat_id == 4u { // Bedrock
        material_color = vec3<f32>(0.18, 0.18, 0.20);
    } else if mat_id == 5u { // Sand
        material_color = vec3<f32>(0.76, 0.70, 0.50);
    } else if mat_id == 6u { // Water
        material_color = vec3<f32>(0.20, 0.40, 0.70);
    } else if mat_id == 7u { // Neon
        // Emission handled below
        material_color = vec3<f32>(1.0, 0.2, 0.8);
    }
    
    // Simple directional lighting (sun from upper-right)
    let light_dir = normalize(vec3<f32>(0.4, 0.75, 0.35));
    let ndotl = max(dot(in.normal, light_dir), 0.0);
    
    // Ambient + diffuse
    let ambient = 0.35;
    let diffuse = 0.65 * ndotl;
    var lit_color = material_color * (ambient + diffuse);
    
    // Face-based shading (top faces brighter)
    var face_factor = 1.0;
    if in.normal.y > 0.5 {
        face_factor = 1.15; // Top face - brightest (sun overhead)
    } else if in.normal.y < -0.5 {
        face_factor = 0.55; // Bottom face - darkest (shadow)
    } else if abs(in.normal.x) > 0.5 {
        face_factor = 0.80; // East/West faces
    } else {
        face_factor = 0.70; // North/South faces
    }
    
    // Apply vertex AO from block-mesh-rs (0 = dark, 1 = bright)
    let vertex_ao = clamp(in.ao, 0.3, 1.0);
    
    // Combined lighting
    var final_color = lit_color * face_factor * vertex_ao;
    
    // Emission for neon blocks
    if mat_id == 7u {
        final_color = material_color * 2.0; // Glow
    }
    
    // Distance fog
    let camera_dist = length(in.world_position - camera.camera_pos.xyz);
    let fog_start = 50.0;
    let fog_end = camera.camera_params.y * 0.9; // 90% of far plane
    let fog_factor = clamp((camera_dist - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    let fog_color = vec3<f32>(0.55, 0.65, 0.82); // Soft sky blue
    
    final_color = mix(final_color, fog_color, fog_factor * 0.75);
    
    return vec4<f32>(final_color, 1.0);
}

// =============================================================================
// Wireframe debug shader variant
// =============================================================================

@fragment
fn fs_wireframe(in: VertexOutput) -> @location(0) vec4<f32> {
    // Edge detection using derivatives
    let edge_width = 0.02;
    let uv_frac = fract(in.uv);
    let edge = smoothstep(0.0, edge_width, uv_frac.x) * 
               smoothstep(0.0, edge_width, uv_frac.y) *
               smoothstep(0.0, edge_width, 1.0 - uv_frac.x) *
               smoothstep(0.0, edge_width, 1.0 - uv_frac.y);
    
    let wire_color = vec3<f32>(0.0, 1.0, 0.8); // Neon cyan
    let fill_color = vec3<f32>(0.02, 0.02, 0.05);
    
    let color = mix(wire_color, fill_color, edge);
    return vec4<f32>(color, 1.0);
}
