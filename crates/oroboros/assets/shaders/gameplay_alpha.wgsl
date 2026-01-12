// ============================================================================
// OROBOROS GAMEPLAY ALPHA SHADER
// JOINT MISSION: UNIT 2 + UNIT 4
// ============================================================================
// Features:
// - Instanced voxel rendering with materials
// - Wireframe selection cube rendering
// - Basic lighting
// ============================================================================

// Camera uniform
struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    camera_pos: vec4<f32>,
    camera_params: vec4<f32>, // near, far, aspect, fov
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// ============================================================================
// VOXEL VERTEX SHADER (INSTANCED)
// ============================================================================

struct VoxelInstance {
    @location(5) position_scale: vec4<f32>,
    @location(6) dimensions_normal_material: vec4<f32>,
    @location(7) emission: vec4<f32>,
    @location(8) uv_offset_scale: vec4<f32>,
}

struct VoxelVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) material_id: f32,
    @location(3) emission: vec4<f32>,
}

// Quad vertices (6 vertices for 2 triangles)
const QUAD_VERTICES: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), // Triangle 1
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0), // Triangle 2
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 1.0),
);

// Normal directions
const NORMALS: array<vec3<f32>, 6> = array<vec3<f32>, 6>(
    vec3<f32>(1.0, 0.0, 0.0),   // 0: +X
    vec3<f32>(-1.0, 0.0, 0.0),  // 1: -X
    vec3<f32>(0.0, 1.0, 0.0),   // 2: +Y (top)
    vec3<f32>(0.0, -1.0, 0.0),  // 3: -Y (bottom)
    vec3<f32>(0.0, 0.0, 1.0),   // 4: +Z
    vec3<f32>(0.0, 0.0, -1.0),  // 5: -Z
);

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: VoxelInstance,
) -> VoxelVertexOutput {
    var output: VoxelVertexOutput;
    
    let quad_pos = QUAD_VERTICES[vertex_index];
    let normal_index = u32(instance.dimensions_normal_material.z);
    let normal = NORMALS[normal_index % 6u];
    
    let width = instance.dimensions_normal_material.x;
    let height = instance.dimensions_normal_material.y;
    let base_pos = instance.position_scale.xyz;
    
    // Calculate world position based on face orientation
    var world_pos: vec3<f32>;
    
    if normal_index == 0u { // +X
        world_pos = base_pos + vec3<f32>(0.0, quad_pos.x * height, quad_pos.y * width);
    } else if normal_index == 1u { // -X
        world_pos = base_pos + vec3<f32>(0.0, quad_pos.x * height, quad_pos.y * width);
    } else if normal_index == 2u { // +Y (top)
        world_pos = base_pos + vec3<f32>(quad_pos.x * width, 0.0, quad_pos.y * height);
    } else if normal_index == 3u { // -Y
        world_pos = base_pos + vec3<f32>(quad_pos.x * width, 0.0, quad_pos.y * height);
    } else if normal_index == 4u { // +Z
        world_pos = base_pos + vec3<f32>(quad_pos.x * width, quad_pos.y * height, 0.0);
    } else { // -Z
        world_pos = base_pos + vec3<f32>(quad_pos.x * width, quad_pos.y * height, 0.0);
    }
    
    output.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    output.world_pos = world_pos;
    output.normal = normal;
    output.material_id = instance.dimensions_normal_material.w;
    output.emission = instance.emission;
    
    return output;
}

// ============================================================================
// VOXEL FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(input: VoxelVertexOutput) -> @location(0) vec4<f32> {
    // Get material color
    let material_id = u32(input.material_id);
    var color = get_material_color(material_id);
    
    // Simple directional lighting
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let ndotl = max(dot(input.normal, light_dir), 0.0);
    let ambient = 0.4;
    let lighting = ambient + (1.0 - ambient) * ndotl;
    
    // Apply lighting
    color = vec4<f32>(color.rgb * lighting, color.a);
    
    // Add emission (neon glow)
    if input.emission.a > 0.0 {
        color = vec4<f32>(
            color.rgb + input.emission.rgb * input.emission.a,
            1.0
        );
    }
    
    return color;
}

// Material color lookup
fn get_material_color(material_id: u32) -> vec4<f32> {
    // Grass
    if material_id == 1u {
        return vec4<f32>(0.3, 0.7, 0.2, 1.0);
    }
    // Grass variant
    if material_id == 2u {
        return vec4<f32>(0.25, 0.65, 0.15, 1.0);
    }
    // Dark rock
    if material_id == 10u {
        return vec4<f32>(0.3, 0.3, 0.35, 1.0);
    }
    // Medium rock / stone
    if material_id == 11u {
        return vec4<f32>(0.5, 0.5, 0.55, 1.0);
    }
    // Light rock / snow
    if material_id == 12u {
        return vec4<f32>(0.85, 0.85, 0.9, 1.0);
    }
    // Pillar
    if material_id == 200u {
        return vec4<f32>(0.6, 0.6, 0.7, 1.0);
    }
    if material_id == 201u {
        return vec4<f32>(0.4, 0.4, 0.5, 1.0);
    }
    // Neon materials (200+)
    if material_id >= 250u {
        return vec4<f32>(0.0, 1.0, 1.0, 1.0); // Cyan neon
    }
    // Default: gray
    return vec4<f32>(0.7, 0.7, 0.7, 1.0);
}

// ============================================================================
// WIREFRAME VERTEX SHADER (Selection Cube)
// ============================================================================

struct WireframeVertexInput {
    @location(0) position: vec3<f32>,
}

struct WireframeVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_wireframe(input: WireframeVertexInput) -> WireframeVertexOutput {
    var output: WireframeVertexOutput;
    output.clip_position = camera.view_proj * vec4<f32>(input.position, 1.0);
    // White with some transparency
    output.color = vec4<f32>(1.0, 1.0, 1.0, 0.8);
    return output;
}

// ============================================================================
// WIREFRAME FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_wireframe(input: WireframeVertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}

// ============================================================================
// NPC VERTEX SHADER (Colored Boxes)
// ============================================================================

struct NpcInstance {
    @location(0) position_scale: vec4<f32>,  // x, y, z, scale
    @location(1) color: vec4<f32>,           // r, g, b, a
}

struct NpcVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

// Cube vertices (36 vertices for 12 triangles)
const CUBE_VERTICES: array<vec3<f32>, 36> = array<vec3<f32>, 36>(
    // Front face (+Z)
    vec3<f32>(-0.4, -0.9, 0.4), vec3<f32>(0.4, -0.9, 0.4), vec3<f32>(0.4, 0.9, 0.4),
    vec3<f32>(-0.4, -0.9, 0.4), vec3<f32>(0.4, 0.9, 0.4), vec3<f32>(-0.4, 0.9, 0.4),
    // Back face (-Z)
    vec3<f32>(0.4, -0.9, -0.4), vec3<f32>(-0.4, -0.9, -0.4), vec3<f32>(-0.4, 0.9, -0.4),
    vec3<f32>(0.4, -0.9, -0.4), vec3<f32>(-0.4, 0.9, -0.4), vec3<f32>(0.4, 0.9, -0.4),
    // Top face (+Y)
    vec3<f32>(-0.4, 0.9, 0.4), vec3<f32>(0.4, 0.9, 0.4), vec3<f32>(0.4, 0.9, -0.4),
    vec3<f32>(-0.4, 0.9, 0.4), vec3<f32>(0.4, 0.9, -0.4), vec3<f32>(-0.4, 0.9, -0.4),
    // Bottom face (-Y)
    vec3<f32>(-0.4, -0.9, -0.4), vec3<f32>(0.4, -0.9, -0.4), vec3<f32>(0.4, -0.9, 0.4),
    vec3<f32>(-0.4, -0.9, -0.4), vec3<f32>(0.4, -0.9, 0.4), vec3<f32>(-0.4, -0.9, 0.4),
    // Right face (+X)
    vec3<f32>(0.4, -0.9, 0.4), vec3<f32>(0.4, -0.9, -0.4), vec3<f32>(0.4, 0.9, -0.4),
    vec3<f32>(0.4, -0.9, 0.4), vec3<f32>(0.4, 0.9, -0.4), vec3<f32>(0.4, 0.9, 0.4),
    // Left face (-X)
    vec3<f32>(-0.4, -0.9, -0.4), vec3<f32>(-0.4, -0.9, 0.4), vec3<f32>(-0.4, 0.9, 0.4),
    vec3<f32>(-0.4, -0.9, -0.4), vec3<f32>(-0.4, 0.9, 0.4), vec3<f32>(-0.4, 0.9, -0.4),
);

// Cube normals for each face (6 faces * 6 vertices each)
const CUBE_NORMALS: array<vec3<f32>, 36> = array<vec3<f32>, 36>(
    // Front face (+Z)
    vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, 1.0),
    vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, 1.0),
    // Back face (-Z)
    vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, 0.0, -1.0),
    vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, 0.0, -1.0),
    // Top face (+Y)
    vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 0.0),
    vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 0.0),
    // Bottom face (-Y)
    vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0),
    vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0),
    // Right face (+X)
    vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 0.0, 0.0),
    vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 0.0, 0.0),
    // Left face (-X)
    vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0),
    vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0),
);

@vertex
fn vs_npc(
    @builtin(vertex_index) vertex_index: u32,
    instance: NpcInstance,
) -> NpcVertexOutput {
    var output: NpcVertexOutput;
    
    let local_pos = CUBE_VERTICES[vertex_index];
    let normal = CUBE_NORMALS[vertex_index];
    let scale = instance.position_scale.w;
    
    // Transform to world space
    let world_pos = instance.position_scale.xyz + local_pos * scale;
    
    output.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    output.world_pos = world_pos;
    output.normal = normal;
    output.color = instance.color;
    
    return output;
}

// ============================================================================
// NPC FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_npc(input: NpcVertexOutput) -> @location(0) vec4<f32> {
    // Simple directional lighting
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let ndotl = max(dot(input.normal, light_dir), 0.0);
    let ambient = 0.4;
    let lighting = ambient + (1.0 - ambient) * ndotl;
    
    // Apply lighting to color
    return vec4<f32>(input.color.rgb * lighting, input.color.a);
}
