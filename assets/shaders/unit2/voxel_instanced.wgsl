// =============================================================================
// OROBOROS Voxel Instanced Renderer
// =============================================================================
// SQUAD NEON - GPU-bound voxel rendering
// 
// MANDATE: 1 million voxels @ 120 FPS @ 4K
// CONSTRAINT: Single draw call for all visible geometry
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

// Per-instance data (matches InstanceData struct in Rust)
struct InstanceInput {
    @location(5) position_scale: vec4<f32>,
    @location(6) dimensions_normal_material: vec4<f32>,
    @location(7) emission: vec4<f32>,
    @location(8) uv_offset_scale: vec4<f32>,
}

// Vertex output
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material_id: f32,
    @location(4) emission: vec4<f32>,
}

// Normal lookup table (6 directions)
fn get_normal(index: u32) -> vec3<f32> {
    switch index {
        case 0u: { return vec3<f32>(1.0, 0.0, 0.0); }   // +X
        case 1u: { return vec3<f32>(-1.0, 0.0, 0.0); }  // -X
        case 2u: { return vec3<f32>(0.0, 1.0, 0.0); }   // +Y
        case 3u: { return vec3<f32>(0.0, -1.0, 0.0); }  // -Y
        case 4u: { return vec3<f32>(0.0, 0.0, 1.0); }   // +Z
        case 5u: { return vec3<f32>(0.0, 0.0, -1.0); }  // -Z
        default: { return vec3<f32>(0.0, 1.0, 0.0); }
    }
}

// Get tangent for UV calculation
fn get_tangent(normal_index: u32) -> vec3<f32> {
    switch normal_index {
        case 0u, 1u: { return vec3<f32>(0.0, 1.0, 0.0); }
        case 2u, 3u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 4u, 5u: { return vec3<f32>(1.0, 0.0, 0.0); }
        default: { return vec3<f32>(1.0, 0.0, 0.0); }
    }
}

fn get_bitangent(normal_index: u32) -> vec3<f32> {
    switch normal_index {
        case 0u, 1u: { return vec3<f32>(0.0, 0.0, 1.0); }
        case 2u, 3u: { return vec3<f32>(0.0, 0.0, 1.0); }
        case 4u, 5u: { return vec3<f32>(0.0, 1.0, 0.0); }
        default: { return vec3<f32>(0.0, 1.0, 0.0); }
    }
}

// Quad vertices (2 triangles, CCW winding)
// 0--1
// |\ |
// | \|
// 3--2
// NOTE: Using switch function instead of const array because WGPU doesn't support
// runtime indexing of const arrays ("may only be indexed by a constant")
fn get_quad_vertex(index: u32) -> vec2<f32> {
    switch index {
        case 0u: { return vec2<f32>(0.0, 0.0); } // Tri 1: bottom-left
        case 1u: { return vec2<f32>(1.0, 0.0); } // Tri 1: bottom-right
        case 2u: { return vec2<f32>(1.0, 1.0); } // Tri 1: top-right
        case 3u: { return vec2<f32>(0.0, 0.0); } // Tri 2: bottom-left
        case 4u: { return vec2<f32>(1.0, 1.0); } // Tri 2: top-right
        case 5u: { return vec2<f32>(0.0, 1.0); } // Tri 2: top-left
        default: { return vec2<f32>(0.0, 0.0); }
    }
}

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
    
    // Get basis vectors for this face
    let normal = get_normal(normal_index);
    let tangent = get_tangent(normal_index);
    let bitangent = get_bitangent(normal_index);
    
    // Get local vertex position on quad
    let local_uv = get_quad_vertex(vertex_index);
    
    // Scale to quad size
    let scaled_uv = vec2<f32>(local_uv.x * width, local_uv.y * height);
    
    // Calculate world position
    let world_pos = position + tangent * scaled_uv.x * scale + bitangent * scaled_uv.y * scale;
    
    // Transform to clip space
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_position = world_pos;
    out.normal = normal;
    out.uv = local_uv * vec2<f32>(width, height);
    out.material_id = material_id;
    out.emission = instance.emission;
    
    return out;
}

// Material texture atlas
@group(1) @binding(0)
var material_texture: texture_2d<f32>;
@group(1) @binding(1)
var material_sampler: sampler;

// Fragment output (G-Buffer for deferred rendering)
struct FragmentOutput {
    @location(0) albedo: vec4<f32>,      // RGB + roughness
    @location(1) normal: vec4<f32>,       // RGB normal + metallic
    @location(2) emission: vec4<f32>,     // RGB emission + intensity
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    
    // Sample material from atlas (16x16 grid of 16x16 tiles)
    let atlas_size = 256.0;
    let tile_size = 16.0;
    let tiles_per_row = atlas_size / tile_size;
    
    let material_x = f32(u32(in.material_id) % u32(tiles_per_row));
    let material_y = f32(u32(in.material_id) / u32(tiles_per_row));
    
    let tile_uv = fract(in.uv) * (tile_size / atlas_size);
    let atlas_uv = vec2<f32>(material_x, material_y) / tiles_per_row + tile_uv;
    
    var albedo = textureSample(material_texture, material_sampler, atlas_uv);
    
    // Apply voxel-style shading (subtle face-based ambient occlusion)
    let ao = select(1.0, 0.8, abs(in.normal.y) < 0.5);
    albedo = vec4<f32>(albedo.rgb * ao, albedo.a);
    
    // Pack output
    out.albedo = vec4<f32>(albedo.rgb, 0.5); // Default roughness 0.5
    out.normal = vec4<f32>(in.normal * 0.5 + 0.5, 0.0); // Packed normal + metallic
    out.emission = in.emission;
    
    return out;
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
