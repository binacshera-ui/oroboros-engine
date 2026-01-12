// =============================================================================
// OROBOROS Occlusion Test Shader
// =============================================================================
// SQUAD NEON - GPU-driven visibility testing
//
// Tests bounding boxes against the HiZ pyramid to determine visibility.
// Outputs visible object indices for indirect drawing.
// =============================================================================

struct OcclusionBounds {
    min_x: f32,
    min_y: f32,
    min_z: f32,
    index: u32,
    max_x: f32,
    max_y: f32,
    max_z: f32,
    _pad: u32,
}

struct CameraData {
    view_proj: mat4x4<f32>,
    near: f32,
    far: f32,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraData;

@group(0) @binding(1)
var hiz_texture: texture_2d<f32>;

@group(0) @binding(2)
var<storage, read> bounds_buffer: array<OcclusionBounds>;

@group(0) @binding(3)
var<storage, read_write> visibility_buffer: array<u32>;

@group(0) @binding(4)
var<storage, read_write> draw_count: atomic<u32>;

struct PushConstants {
    object_count: u32,
    hiz_width: u32,
    hiz_height: u32,
    hiz_mip_count: u32,
}

var<push_constant> pc: PushConstants;

// Project a world-space point to NDC
fn project_point(p: vec3<f32>) -> vec4<f32> {
    let clip = camera.view_proj * vec4<f32>(p, 1.0);
    return vec4<f32>(
        clip.xy / clip.w * 0.5 + 0.5,
        clip.z / clip.w,
        clip.w
    );
}

// Test if an AABB is visible against the HiZ
fn test_bounds(bounds: OcclusionBounds) -> bool {
    let min_p = vec3<f32>(bounds.min_x, bounds.min_y, bounds.min_z);
    let max_p = vec3<f32>(bounds.max_x, bounds.max_y, bounds.max_z);
    
    // Project all 8 corners of the AABB
    var screen_min = vec2<f32>(1.0);
    var screen_max = vec2<f32>(0.0);
    var min_z = 1.0;
    var any_in_front = false;
    
    for (var i = 0u; i < 8u; i++) {
        let corner = vec3<f32>(
            select(min_p.x, max_p.x, (i & 1u) != 0u),
            select(min_p.y, max_p.y, (i & 2u) != 0u),
            select(min_p.z, max_p.z, (i & 4u) != 0u),
        );
        
        let projected = project_point(corner);
        
        // Check if point is in front of camera
        if (projected.w > 0.0) {
            any_in_front = true;
            screen_min = min(screen_min, projected.xy);
            screen_max = max(screen_max, projected.xy);
            min_z = min(min_z, projected.z);
        }
    }
    
    // If nothing is in front of camera, object is behind us
    if (!any_in_front) {
        return false;
    }
    
    // Clamp to screen bounds
    screen_min = clamp(screen_min, vec2<f32>(0.0), vec2<f32>(1.0));
    screen_max = clamp(screen_max, vec2<f32>(0.0), vec2<f32>(1.0));
    
    // If AABB projects to nothing (completely off-screen), cull it
    if (screen_min.x >= screen_max.x || screen_min.y >= screen_max.y) {
        return false;
    }
    
    // Calculate screen-space size to choose HiZ mip level
    let screen_size = screen_max - screen_min;
    let hiz_size = vec2<f32>(f32(pc.hiz_width), f32(pc.hiz_height));
    let pixel_size = screen_size * hiz_size;
    
    // Choose mip level based on projected size (larger objects use lower mip)
    let max_extent = max(pixel_size.x, pixel_size.y);
    let mip_level = clamp(u32(log2(max_extent)), 0u, pc.hiz_mip_count - 1u);
    
    // Sample HiZ at the center of the projected AABB
    let center_uv = (screen_min + screen_max) * 0.5;
    let hiz_coord = vec2<i32>(center_uv * hiz_size) >> vec2<u32>(mip_level);
    
    let hiz_depth = textureLoad(hiz_texture, hiz_coord, i32(mip_level)).r;
    
    // Object is visible if its closest point is in front of the HiZ depth
    return min_z <= hiz_depth;
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let object_idx = global_id.x;
    
    if (object_idx >= pc.object_count) {
        return;
    }
    
    let bounds = bounds_buffer[object_idx];
    
    if (test_bounds(bounds)) {
        // Object is visible - add to output list
        let output_idx = atomicAdd(&draw_count, 1u);
        visibility_buffer[output_idx] = bounds.index;
    }
}

// =============================================================================
// Compact visible indices for indirect draw
// =============================================================================

struct DrawIndexedIndirectCommand {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

@group(0) @binding(5)
var<storage, read_write> indirect_commands: array<DrawIndexedIndirectCommand>;

@compute @workgroup_size(1, 1, 1)
fn finalize_draw() {
    // Update the indirect draw command with final visible count
    let visible_count = atomicLoad(&draw_count);
    
    indirect_commands[0].index_count = 6u; // 2 triangles per quad
    indirect_commands[0].instance_count = visible_count;
    indirect_commands[0].first_index = 0u;
    indirect_commands[0].base_vertex = 0;
    indirect_commands[0].first_instance = 0u;
}
