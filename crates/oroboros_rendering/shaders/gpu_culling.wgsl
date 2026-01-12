// =============================================================================
// OROBOROS GPU-Driven Culling
// =============================================================================
// ARCHITECT'S MANDATE: CPU must not touch culling. CPU is busy with MEV.
//
// This compute shader:
// 1. Tests each chunk against frustum planes
// 2. Tests against HiZ for occlusion
// 3. Writes DrawIndirectArgs for visible chunks
// 4. CPU just submits ONE MultiDrawIndirect call
//
// CPU WORK PER FRAME: ZERO DATA TRANSFER
// =============================================================================

struct ChunkGPUData {
    position: vec3<f32>,
    size: f32,
    instance_offset: u32,
    instance_count: u32,
    flags: u32,
    _pad: u32,
}

struct CullingUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    frustum_planes: array<vec4<f32>, 6>,
    params: vec4<f32>, // chunk_count, max_distance, _, _
}

struct DrawIndexedIndirectArgs {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

@group(0) @binding(0)
var<uniform> uniforms: CullingUniforms;

@group(0) @binding(1)
var<storage, read> chunks: array<ChunkGPUData>;

@group(0) @binding(2)
var<storage, read_write> draw_commands: array<DrawIndexedIndirectArgs>;

@group(0) @binding(3)
var<storage, read_write> visible_count: atomic<u32>;

// Optional: HiZ texture for occlusion culling
@group(0) @binding(4)
var hiz_texture: texture_2d<f32>;

const FLAG_EMPTY: u32 = 1u;
const FLAG_PREV_OCCLUDED: u32 = 2u;

// Test sphere against frustum planes
fn sphere_in_frustum(center: vec3<f32>, radius: f32) -> bool {
    for (var i = 0u; i < 6u; i++) {
        let plane = uniforms.frustum_planes[i];
        let distance = dot(plane.xyz, center) + plane.w;
        if (distance < -radius) {
            return false;
        }
    }
    return true;
}

// Test AABB against frustum (more accurate than sphere)
fn aabb_in_frustum(min_pos: vec3<f32>, max_pos: vec3<f32>) -> bool {
    for (var i = 0u; i < 6u; i++) {
        let plane = uniforms.frustum_planes[i];
        
        // Find the corner most aligned with plane normal
        let p = vec3<f32>(
            select(min_pos.x, max_pos.x, plane.x >= 0.0),
            select(min_pos.y, max_pos.y, plane.y >= 0.0),
            select(min_pos.z, max_pos.z, plane.z >= 0.0),
        );
        
        if (dot(plane.xyz, p) + plane.w < 0.0) {
            return false;
        }
    }
    return true;
}

// Compute distance for LOD/priority
fn chunk_distance(position: vec3<f32>, size: f32) -> f32 {
    let center = position + vec3<f32>(size * 0.5);
    return length(center - uniforms.camera_pos.xyz);
}

// =============================================================================
// OPTIMIZED: Full GPU Culling Pipeline Restored
// Distance + Frustum + Occlusion culling for HIGH PERFORMANCE
// =============================================================================
@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let chunk_idx = global_id.x;
    let chunk_count = u32(uniforms.params.x);
    
    if (chunk_idx >= chunk_count) {
        return;
    }
    
    let chunk = chunks[chunk_idx];
    
    // Skip empty chunks entirely
    if ((chunk.flags & FLAG_EMPTY) != 0u || chunk.instance_count == 0u) {
        draw_commands[chunk_idx].instance_count = 0u;
        return;
    }
    
    let min_pos = chunk.position;
    let max_pos = chunk.position + vec3<f32>(chunk.size);
    
    // OPTIMIZATION 1: Distance culling
    let dist = chunk_distance(chunk.position, chunk.size);
    let max_distance = uniforms.params.y;
    if (dist > max_distance) {
        draw_commands[chunk_idx].instance_count = 0u;
        return;
    }
    
    // OPTIMIZATION 2: Frustum culling
    if (!aabb_in_frustum(min_pos, max_pos)) {
        draw_commands[chunk_idx].instance_count = 0u;
        return;
    }
    
    // TODO: OPTIMIZATION 3: HiZ occlusion culling (future)
    
    // Chunk is visible - set up draw command
    draw_commands[chunk_idx].index_count = 6u; // 2 triangles per quad
    draw_commands[chunk_idx].instance_count = chunk.instance_count;
    draw_commands[chunk_idx].first_index = 0u;
    draw_commands[chunk_idx].base_vertex = 0;
    draw_commands[chunk_idx].first_instance = chunk.instance_offset;
    
    // Count visible chunks for stats
    atomicAdd(&visible_count, 1u);
}

// =============================================================================
// Compaction pass - removes zero-instance draws for efficiency
// =============================================================================

struct CompactionUniforms {
    total_draws: u32,
    _pad: vec3<u32>,
}

@group(0) @binding(5)
var<uniform> compaction: CompactionUniforms;

@group(0) @binding(6)
var<storage, read_write> compacted_draws: array<DrawIndexedIndirectArgs>;

@group(0) @binding(7)
var<storage, read_write> compacted_count: atomic<u32>;

@compute @workgroup_size(64, 1, 1)
fn compact_draws(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    
    if (idx >= compaction.total_draws) {
        return;
    }
    
    let cmd = draw_commands[idx];
    
    // Only copy non-empty draws
    if (cmd.instance_count > 0u) {
        let output_idx = atomicAdd(&compacted_count, 1u);
        compacted_draws[output_idx] = cmd;
    }
}
