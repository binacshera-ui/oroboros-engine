// =============================================================================
// OROBOROS HiZ Pyramid Generation
// =============================================================================
// SQUAD NEON - GPU-side occlusion culling
//
// Generates a hierarchical depth buffer (HiZ) for efficient occlusion testing.
// Each mip level contains the maximum depth of the 2x2 texels below it.
// =============================================================================

@group(0) @binding(0)
var input_depth: texture_2d<f32>;

@group(0) @binding(1)
var output_depth: texture_storage_2d<r32float, write>;

struct PushConstants {
    // Input mip level
    input_mip: u32,
    // Output dimensions
    output_width: u32,
    output_height: u32,
}

var<push_constant> pc: PushConstants;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let output_size = vec2<u32>(pc.output_width, pc.output_height);
    
    if (global_id.x >= output_size.x || global_id.y >= output_size.y) {
        return;
    }
    
    // Sample 4 texels from the input mip
    let base_coord = global_id.xy * 2u;
    
    let d00 = textureLoad(input_depth, vec2<i32>(base_coord + vec2<u32>(0u, 0u)), i32(pc.input_mip)).r;
    let d10 = textureLoad(input_depth, vec2<i32>(base_coord + vec2<u32>(1u, 0u)), i32(pc.input_mip)).r;
    let d01 = textureLoad(input_depth, vec2<i32>(base_coord + vec2<u32>(0u, 1u)), i32(pc.input_mip)).r;
    let d11 = textureLoad(input_depth, vec2<i32>(base_coord + vec2<u32>(1u, 1u)), i32(pc.input_mip)).r;
    
    // Take maximum (furthest depth) for conservative culling
    // Objects are occluded only if they're behind ALL depth samples
    let max_depth = max(max(d00, d10), max(d01, d11));
    
    textureStore(output_depth, vec2<i32>(global_id.xy), vec4<f32>(max_depth, 0.0, 0.0, 1.0));
}

// =============================================================================
// Initial mip from full resolution depth
// =============================================================================

@group(0) @binding(0)
var full_depth: texture_2d<f32>;

@group(0) @binding(1)
var mip0_depth: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8, 1)
fn main_initial(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let size = textureDimensions(full_depth);
    
    if (global_id.x >= size.x || global_id.y >= size.y) {
        return;
    }
    
    // Just copy depth - this is the base of the pyramid
    let depth = textureLoad(full_depth, vec2<i32>(global_id.xy), 0).r;
    textureStore(mip0_depth, vec2<i32>(global_id.xy), vec4<f32>(depth, 0.0, 0.0, 1.0));
}
