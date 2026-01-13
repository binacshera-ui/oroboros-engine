//! # GLITCH WARS - Cyberpunk Voxel Client
//!
//! "Tron meets The Matrix in a collapsing Voxel Simulation"
//!
//! AESTHETIC: Neon vertex colors, HDR bloom, void atmosphere
//! PLATFORM: WASM/WebGL2 + Native
//! TARGET: 60 FPS on integrated graphics
//!
//! PHYSICS: bevy_xpbd_3d (Enterprise-grade, WASM compatible)

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::core_pipeline::bloom::BloomSettings;
use bevy::core_pipeline::tonemapping::Tonemapping;

// ENTERPRISE PHYSICS
use bevy_xpbd_3d::prelude::*;
// NOTE: PhysicsDebugPlugin removed - was showing debug wireframes

// NOTE: bevy_flycam REMOVED - was causing noclip/flying
// All movement is now physics-based via bevy_xpbd_3d

use oroboros_procedural::{WorldManager, WorldManagerConfig, WorldSeed, ChunkCoord, CHUNK_SIZE};

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

// =============================================================================
// CONFIGURATION
// =============================================================================

/// UNDERCITY: 9x9 chunks = 288x288 blocks with 3D caves
/// Large enough for exploration, small enough for WASM performance
const LOAD_RADIUS: i32 = 4; // -4..4 = 9x9 chunks centered at origin

/// World seed for procedural generation
const WORLD_SEED: u64 = 42;

/// Block scale - each voxel is 0.25 units (quarter size)
/// This makes the world feel larger and more detailed
const BLOCK_SCALE: f32 = 0.25;

// =============================================================================
// ECONOMY ENGINE - The Weight System & Staking Logic
// =============================================================================

/// Player's economic state
#[derive(Component, Default)]
#[allow(dead_code)]
struct PlayerEconomy {
    /// Number of crystals collected
    loot_count: u32,
    /// Total value in virtual currency
    net_worth: f32,
    /// Current load affecting movement
    current_load: f32,
    /// Stamina/Energy (depletes over time)
    stamina: f32,
}

/// Map tiers based on staking amount
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
enum MapTier {
    /// Low stake: Small map, low loot density
    Scavenger,
    /// High stake: Large map with caves, high loot
    HighRoller,
    /// VIP: Maximum map size, rare spawns
    Whale,
}

impl MapTier {
    /// Determines map tier based on staked amount (mockup)
    #[allow(dead_code)]
    fn from_stake(stake_amount: f32) -> Self {
        if stake_amount >= 10000.0 {
            MapTier::Whale
        } else if stake_amount >= 1000.0 {
            MapTier::HighRoller
        } else {
            MapTier::Scavenger
        }
    }
    
    /// Returns load radius for this tier
    #[allow(dead_code)]
    fn load_radius(&self) -> i32 {
        match self {
            MapTier::Scavenger => 2,  // 5x5 chunks
            MapTier::HighRoller => 4, // 9x9 chunks
            MapTier::Whale => 6,      // 13x13 chunks
        }
    }
}

/// Calculate movement damping based on carried loot
/// Rich players move like trucks!
#[allow(dead_code)]
fn calculate_load_damping(loot_count: u32) -> f32 {
    5.0 + (loot_count as f32 * 0.5)
}

/// Death tax calculation
/// On death: 20% burned, 30% dropped as loot
#[allow(dead_code)]
struct DeathTax {
    burn_amount: f32,   // Permanently destroyed
    drop_amount: f32,   // Becomes lootable
    kept_amount: f32,   // Player retains this
}

impl DeathTax {
    #[allow(dead_code)]
    fn calculate(wallet_value: f32) -> Self {
        Self {
            burn_amount: wallet_value * 0.2,
            drop_amount: wallet_value * 0.3,
            kept_amount: wallet_value * 0.5,
        }
    }
}

// =============================================================================
// BACKEND BRIDGE - Wraps our existing backend for Bevy
// =============================================================================

/// Inner data that requires mutex protection
struct BackendInner {
    /// The world manager from oroboros_procedural
    world_manager: WorldManager,
    
    /// Currently loaded chunks
    loaded_chunks: HashSet<ChunkCoord>,
    
    /// Chunks that need mesh regeneration
    dirty_chunks: HashSet<ChunkCoord>,
    
    /// Last player chunk position
    last_player_chunk: Option<ChunkCoord>,
}

/// The bridge between our custom backend and Bevy's ECS
/// 
/// Wrapped in Mutex because WorldManager contains non-Sync types
#[derive(Resource)]
pub struct BackendBridge {
    inner: Mutex<BackendInner>,
}

impl BackendBridge {
    /// Creates a new bridge with initialized world
    pub fn new(seed: u64) -> Self {
        let config = WorldManagerConfig {
            load_radius: LOAD_RADIUS,
            unload_radius: LOAD_RADIUS + 2,
            max_chunks_per_frame: 4,
            world_save_path: std::path::PathBuf::from("world/chunks"),
        };
        
        let world_manager = WorldManager::new(WorldSeed::new(seed), config);
        
        info!("BackendBridge initialized with seed {}", seed);
        
        Self {
            inner: Mutex::new(BackendInner {
                world_manager,
                loaded_chunks: HashSet::new(),
                dirty_chunks: HashSet::new(),
                last_player_chunk: None,
            }),
        }
    }
}

// =============================================================================
// CHUNK ENTITY TRACKING
// =============================================================================

/// Component to mark entities as chunk meshes
#[derive(Component)]
pub struct ChunkMesh {
    /// The chunk coordinate this mesh represents.
    pub coord: ChunkCoord,
}

/// Tracks which chunks have been rendered
#[derive(Resource, Default)]
pub struct RenderedChunks {
    /// Map of chunk coordinates to their entity IDs.
    pub chunks: HashMap<ChunkCoord, Entity>,
}

// =============================================================================
// BLOCK TYPES (from backend)
// =============================================================================

/// Block types matching oroboros_procedural
/// UNDERCITY MATERIAL TYPES
/// Dark, gritty, metallic cyberpunk aesthetic
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
enum BlockType {
    Air = 0,
    /// Surface Grid Floor - reflective metal panels
    GridFloor = 1,
    /// Dark Industrial Metal - cave walls and structures
    DarkMetal = 2,
    /// Glowing Crystal - valuable loot (HIGH EMISSION)
    Crystal = 3,
    /// Safe Zone Floor - extraction point marker
    SafeZone = 4,
    /// Obsidian Bedrock - indestructible base layer
    Obsidian = 5,
}

impl BlockType {
    fn from_id(id: u16) -> Self {
        match id {
            0 => BlockType::Air,
            1 => BlockType::GridFloor,
            2 => BlockType::DarkMetal,
            3 => BlockType::Crystal,
            4 => BlockType::SafeZone,
            5 => BlockType::Obsidian,
            _ => BlockType::DarkMetal,
        }
    }
    
    /// Returns RGBA vertex color for PBR METALLIC pipeline
    /// UNDERCITY PALETTE - Brighter Industrial Cyberpunk
    fn vertex_color(&self) -> [f32; 4] {
        match self {
            // ID 0: Air -> Invisible
            BlockType::Air => [0.0, 0.0, 0.0, 0.0],
            
            // ID 1: Grid Floor -> Light Steel Grey
            // Visible reflective floor
            BlockType::GridFloor => [0.5, 0.5, 0.55, 1.0],
            
            // ID 2: Dark Metal -> Visible Grey (not black)
            // Cave walls should be visible
            BlockType::DarkMetal => [0.25, 0.25, 0.28, 1.0],
            
            // ID 3: Crystal -> BRIGHT GOLD with EMISSION
            // HDR values >1.0 trigger bloom glow
            BlockType::Crystal => [4.0, 3.2, 0.5, 1.0],
            
            // ID 4: Safe Zone -> BRIGHT CYAN
            BlockType::SafeZone => [0.0, 4.0, 4.0, 1.0],
            
            // ID 5: Obsidian -> Dark Grey (visible)
            BlockType::Obsidian => [0.15, 0.15, 0.18, 1.0],
        }
    }
    
    /// Returns PBR Metallic value (0.0 = dielectric, 1.0 = metal)
    #[allow(dead_code)]
    fn metallic(&self) -> f32 {
        match self {
            BlockType::Air => 0.0,
            BlockType::GridFloor => 0.9,    // High metal
            BlockType::DarkMetal => 0.85,   // Industrial metal
            BlockType::Crystal => 0.2,      // Crystal is not metal
            BlockType::SafeZone => 0.3,     // Slight metallic sheen
            BlockType::Obsidian => 0.95,    // Polished obsidian
        }
    }
    
    /// Returns PBR Roughness value (0.0 = mirror, 1.0 = matte)
    #[allow(dead_code)]
    fn roughness(&self) -> f32 {
        match self {
            BlockType::Air => 1.0,
            BlockType::GridFloor => 0.3,    // Shiny floor
            BlockType::DarkMetal => 0.6,    // Slightly rough
            BlockType::Crystal => 0.1,      // Very shiny crystal
            BlockType::SafeZone => 0.4,     // Semi-gloss
            BlockType::Obsidian => 0.05,    // Mirror polish
        }
    }
    
    fn is_solid(&self) -> bool {
        !matches!(self, BlockType::Air)
    }
}

// =============================================================================
// MESH GENERATION - The Critical Fix
// =============================================================================

/// Face directions for cube generation
const FACE_NORMALS: [[f32; 3]; 6] = [
    [1.0, 0.0, 0.0],   // +X (Right)
    [-1.0, 0.0, 0.0],  // -X (Left)
    [0.0, 1.0, 0.0],   // +Y (Top)
    [0.0, -1.0, 0.0],  // -Y (Bottom)
    [0.0, 0.0, 1.0],   // +Z (Front)
    [0.0, 0.0, -1.0],  // -Z (Back)
];

/// Result of mesh generation (VERTEX COLORING: color is baked into vertices)
struct MeshResult {
    mesh: Mesh,
}

/// Generates a Bevy Mesh from chunk data using simple culled meshing
fn generate_chunk_mesh(
    inner: &mut BackendInner,
    coord: ChunkCoord,
) -> Option<MeshResult> {
    // Check if chunk exists
    if inner.world_manager.get_chunk(coord).is_none() {
        warn!("Chunk [{},{}] not found in world manager!", coord.x, coord.z);
        return None;
    }
    
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    // VERTEX COLORING: Colors per vertex (RGBA)
    let mut colors: Vec<[f32; 4]> = Vec::new();
    
    // Color tracking
    let mut color_counts: HashMap<u8, u32> = HashMap::new();
    let mut solid_blocks_found = 0u32;
    let mut faces_added = 0u32;
    
    let chunk_world_x = coord.x * CHUNK_SIZE as i32;
    let chunk_world_z = coord.z * CHUNK_SIZE as i32;
    
    // Iterate through all voxels in chunk
    for local_y in 0..256 {
        for local_z in 0..CHUNK_SIZE as i32 {
            for local_x in 0..CHUNK_SIZE as i32 {
                let world_x = chunk_world_x + local_x;
                let world_z = chunk_world_z + local_z;
                
                let block = match inner.world_manager.get_block(world_x, local_y, world_z) {
                    Some(b) => BlockType::from_id(b.id),
                    None => BlockType::Air,
                };
                
                if !block.is_solid() {
                    continue;
                }
                
                solid_blocks_found += 1;
                *color_counts.entry(block as u8).or_insert(0) += 1;
                
                let pos = [world_x as f32, local_y as f32, world_z as f32];
                // VERTEX COLORING: Get color for this block
                let block_color = block.vertex_color();
                
                // Check each face
                for face in 0..6 {
                    let (nx, ny, nz) = get_neighbor_offset(face);
                    let neighbor_x = world_x + nx;
                    let neighbor_y = local_y + ny;
                    let neighbor_z = world_z + nz;
                    
                    let neighbor_solid = is_solid_at(&inner.world_manager, neighbor_x, neighbor_y, neighbor_z);
                    
                    if !neighbor_solid {
                        add_face(&mut positions, &mut normals, &mut uvs, &mut indices, &mut colors, pos, face, block_color);
                        faces_added += 1;
                    }
                }
            }
        }
    }
    
    // DEBUG: Log what we found
    if solid_blocks_found == 0 {
        info!("Chunk [{},{}]: No solid blocks found (empty chunk)", coord.x, coord.z);
        return None;
    }
    
    if positions.is_empty() {
        warn!("Chunk [{},{}]: {} solid blocks but 0 visible faces!", 
              coord.x, coord.z, solid_blocks_found);
        return None;
    }
    
    info!("Chunk [{},{}]: {} solid blocks, {} faces, {} vertices", 
          coord.x, coord.z, solid_blocks_found, faces_added, positions.len());
    
    // Build Bevy Mesh with VERTEX COLORS
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    // VERTEX COLORING: Inject colors directly into mesh vertices
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    
    Some(MeshResult { mesh })
}

/// Get neighbor offset for a face
fn get_neighbor_offset(face: usize) -> (i32, i32, i32) {
    match face {
        0 => (1, 0, 0),   // +X
        1 => (-1, 0, 0),  // -X
        2 => (0, 1, 0),   // +Y
        3 => (0, -1, 0),  // -Y
        4 => (0, 0, 1),   // +Z
        5 => (0, 0, -1),  // -Z
        _ => (0, 0, 0),
    }
}

/// Check if position is solid
fn is_solid_at(world: &WorldManager, x: i32, y: i32, z: i32) -> bool {
    if y < 0 || y >= 256 {
        return false;
    }
    match world.get_block(x, y, z) {
        Some(b) => BlockType::from_id(b.id).is_solid(),
        None => false, // Unloaded = draw face
    }
}

/// Add a face to the mesh with VERTEX COLORING
fn add_face(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    colors: &mut Vec<[f32; 4]>,
    pos: [f32; 3],
    face: usize,
    block_color: [f32; 4],
) {
    let base_index = positions.len() as u32;
    let normal = FACE_NORMALS[face];
    let verts = get_face_vertices(pos, face);
    
    for vert in &verts {
        positions.push(*vert);
        normals.push(normal);
    }
    
    uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    
    // VERTEX COLORING: Push color for each of the 4 vertices
    for _ in 0..4 {
        colors.push(block_color);
    }
    
    // Counter-clockwise winding (Bevy default)
    indices.extend_from_slice(&[
        base_index, base_index + 1, base_index + 2,
        base_index, base_index + 2, base_index + 3,
    ]);
}

/// Get the 4 vertices for a face (CCW winding when viewed from OUTSIDE the cube)
/// Bevy uses CCW = front-face, RIGHT_HANDED_Y_UP
/// Uses BLOCK_SCALE for smaller voxels
fn get_face_vertices(pos: [f32; 3], face: usize) -> [[f32; 3]; 4] {
    let [x, y, z] = pos;
    let s = BLOCK_SCALE; // Block size
    
    // Scale the position
    let sx = x * s;
    let sy = y * s;
    let sz = z * s;
    
    // Each face: 4 vertices in CCW order when viewed from the direction of the normal
    match face {
        // +X (Right): Normal = (1,0,0), view from +X looking at -X
        0 => [
            [sx + s, sy, sz + s],       // bottom-front
            [sx + s, sy, sz],           // bottom-back
            [sx + s, sy + s, sz],       // top-back
            [sx + s, sy + s, sz + s],   // top-front
        ],
        // -X (Left): Normal = (-1,0,0), view from -X looking at +X
        1 => [
            [sx, sy, sz],               // bottom-back
            [sx, sy, sz + s],           // bottom-front
            [sx, sy + s, sz + s],       // top-front
            [sx, sy + s, sz],           // top-back
        ],
        // +Y (Top): Normal = (0,1,0), view from +Y looking at -Y
        2 => [
            [sx, sy + s, sz + s],       // front-left
            [sx + s, sy + s, sz + s],   // front-right
            [sx + s, sy + s, sz],       // back-right
            [sx, sy + s, sz],           // back-left
        ],
        // -Y (Bottom): Normal = (0,-1,0), view from -Y looking at +Y
        3 => [
            [sx, sy, sz],               // back-left
            [sx + s, sy, sz],           // back-right
            [sx + s, sy, sz + s],       // front-right
            [sx, sy, sz + s],           // front-left
        ],
        // +Z (Front): Normal = (0,0,1), view from +Z looking at -Z
        4 => [
            [sx, sy, sz + s],           // bottom-left
            [sx + s, sy, sz + s],       // bottom-right
            [sx + s, sy + s, sz + s],   // top-right
            [sx, sy + s, sz + s],       // top-left
        ],
        // -Z (Back): Normal = (0,0,-1), view from -Z looking at +Z
        5 => [
            [sx + s, sy, sz],           // bottom-right
            [sx, sy, sz],               // bottom-left
            [sx, sy + s, sz],           // top-left
            [sx + s, sy + s, sz],       // top-right
        ],
        _ => [[0.0; 3]; 4],
    }
}

// =============================================================================
// BEVY SYSTEMS
// =============================================================================

/// FIXED ARENA: No infinite chunk loading
/// Arena is pre-loaded in setup(), this system is now disabled
#[allow(dead_code)]
fn update_chunk_streaming(
    _bridge: Res<BackendBridge>,
    _player_query: Query<&Transform, With<Player>>,
) {
    // DISABLED: Arena is fixed size, no streaming needed
    // All chunks are loaded once in setup()
}

/// System to sync dirty chunks to Bevy meshes
fn sync_chunks_to_bevy(
    mut commands: Commands,
    bridge: Res<BackendBridge>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rendered_chunks: ResMut<RenderedChunks>,
) {
    let mut inner = bridge.inner.lock().unwrap();
    
    // Get dirty chunks
    let dirty_coords: Vec<_> = inner.dirty_chunks.drain().collect();
    
    if dirty_coords.is_empty() {
        return;
    }
    
    info!("Processing {} dirty chunks", dirty_coords.len());
    
    let mut total_vertices = 0;
    let mut empty_chunks = 0;
    let mut valid_chunks = 0;
    
    for coord in dirty_coords {
        if let Some(result) = generate_chunk_mesh(&mut inner, coord) {
            // SANITY CHECK: Is the mesh actually populated?
            let vertex_count = result.mesh.count_vertices();
            
            if vertex_count == 0 {
                warn!("⚠️ Chunk [{},{}] generated EMPTY mesh! World data not loaded?", 
                      coord.x, coord.z);
                empty_chunks += 1;
                continue;
            }
            
            total_vertices += vertex_count;
            valid_chunks += 1;
            
            // Remove old entity if exists
            if let Some(entity) = rendered_chunks.chunks.remove(&coord) {
                commands.entity(entity).despawn();
            }
            
            // CRITICAL: Create physics collider from mesh BEFORE adding to assets
            // This allows the player to walk on the terrain
            let collider = Collider::trimesh_from_mesh(&result.mesh);
            
            // Create new entity with UNDERCITY PBR material (dark metallic)
            let mut entity_commands = commands.spawn((
                PbrBundle {
                    mesh: meshes.add(result.mesh),
                    material: materials.add(StandardMaterial {
                        // VERTEX COLORING: Must be WHITE to multiply with vertex colors
                        base_color: Color::WHITE,
                        // UNDERCITY PBR: Dark Metallic Industrial
                        metallic: 0.8,              // High metallic for industrial look
                        perceptual_roughness: 0.35, // Shiny but not mirror
                        reflectance: 0.6,           // Strong reflections
                        // Emissive driven by vertex colors >1.0
                        emissive: Color::BLACK,
                        // Back-face culling ON for performance
                        cull_mode: Some(bevy::render::render_resource::Face::Back),
                        double_sided: false,
                        ..default()
                    }),
                    transform: Transform::IDENTITY,
                    ..default()
                },
                ChunkMesh { coord },
            ));
            
            // Add physics collider if mesh conversion succeeded
            if let Some(col) = collider {
                entity_commands.insert((
                    RigidBody::Static,  // Terrain doesn't move
                    col,                // Terrain is solid
                ));
            } else {
                warn!("⚠️ Failed to create collider for chunk [{},{}]", coord.x, coord.z);
            }
            
            let entity = entity_commands.id();
            rendered_chunks.chunks.insert(coord, entity);
        } else {
            empty_chunks += 1;
        }
    }
    
    if valid_chunks > 0 || empty_chunks > 0 {
        info!("✅ Mesh stats: {} chunks with {} total vertices, {} empty chunks", 
              valid_chunks, total_vertices, empty_chunks);
    }
}

/// System to unload distant chunks
fn unload_distant_chunks(
    mut commands: Commands,
    bridge: Res<BackendBridge>,
    mut rendered_chunks: ResMut<RenderedChunks>,
) {
    let inner = bridge.inner.lock().unwrap();
    let loaded = &inner.loaded_chunks;
    
    let to_unload: Vec<ChunkCoord> = rendered_chunks.chunks
        .keys()
        .filter(|coord| !loaded.contains(coord))
        .copied()
        .collect();
    
    drop(inner); // Release lock before despawning
    
    for coord in to_unload {
        if let Some(entity) = rendered_chunks.chunks.remove(&coord) {
            commands.entity(entity).despawn();
        }
    }
}

/// Marker component for the player entity
#[derive(Component)]
struct Player;

/// Marker component for the player camera
#[derive(Component)]
struct PlayerCamera;

/// Camera mode - first or third person
#[derive(Component)]
struct CameraMode {
    /// True = third person, False = first person
    third_person: bool,
    /// Distance from player in third person
    distance: f32,
}

/// Setup system - runs once at startup
/// GLITCH WARS aesthetic: Void background, neon glow, physics-based movement
fn setup(
    mut commands: Commands,
    bridge: Res<BackendBridge>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    info!("===========================================");
    info!("GLITCH WARS - Entering the Simulation");
    info!("===========================================");
    
    // Pre-load spawn area and mark all initial chunks as dirty
    let spawn_y = {
        let mut inner = bridge.inner.lock().unwrap();
        
        info!("Pre-loading spawn area...");
        inner.world_manager.ensure_loaded_around(0.0, 0.0, 3);
        inner.world_manager.flush_generation_queue();
        
        let player_chunk = WorldManager::world_to_chunk(0.0, 0.0);
        inner.last_player_chunk = Some(player_chunk);
        
        // CRITICAL: Mark ALL loaded chunks as dirty so they get meshed!
        for dz in -LOAD_RADIUS..=LOAD_RADIUS {
            for dx in -LOAD_RADIUS..=LOAD_RADIUS {
                let coord = ChunkCoord::new(player_chunk.x + dx, player_chunk.z + dz);
                if inner.world_manager.get_chunk(coord).is_some() {
                    inner.loaded_chunks.insert(coord);
                    inner.dirty_chunks.insert(coord);
                }
            }
        }
        
        let stats = inner.world_manager.stats();
        info!("Spawn loaded: {} chunks, {} marked dirty", 
              stats.loaded_chunks, inner.dirty_chunks.len());
        
        // Find ground height (scaled by BLOCK_SCALE)
        let ground_y = find_ground_height(&inner.world_manager, 0, 0) as f32;
        (ground_y + 3.0) * BLOCK_SCALE
    };
    
    info!("Spawn position: (0.0, {}, 0.0)", spawn_y);
    
    // =========================================================================
    // PLAYER - Physics-based Character Controller (NO NOCLIP!)
    // =========================================================================
    // Player size scaled with blocks (human ~7 blocks tall = 1.75m at 0.25 scale)
    let player_radius = 0.15;  // Smaller to fit in block scale
    let player_height = 0.4;   // Total capsule height
    
    let player_id = commands.spawn((
        Player,
        Name::new("Player"),
        // ECONOMY: Player's financial state
        PlayerEconomy {
            loot_count: 0,
            net_worth: 0.0,
            current_load: 0.0,
            stamina: 100.0,
        },
        // Physics components (Enterprise-grade) - MUST COLLIDE WITH TERRAIN
        RigidBody::Dynamic,
        Collider::capsule(player_height * 0.5, player_radius), // Smaller capsule
        LockedAxes::ROTATION_LOCKED,         // Don't tip over!
        Friction::new(0.3),                  // Some friction for better control
        Restitution::new(0.0),               // No bouncing
        LinearDamping(5.0),                  // Base damping (modified by load)
        GravityScale(1.5),                   // Good gravity feel
        // ShapeCaster for step climbing - allows climbing up small ledges
        ShapeCaster::new(
            Collider::sphere(player_radius * 0.9),
            Vec3::ZERO,
            Quat::IDENTITY,
            Direction3d::NEG_Y,
        ).with_max_time_of_impact(0.5), // Check 0.5 units below
        // Visual representation
        PbrBundle {
            mesh: meshes.add(Capsule3d::new(player_radius, player_height)),
            material: materials.add(StandardMaterial {
                base_color: Color::rgb(0.0, 1.0, 1.0), // Cyan player
                emissive: Color::rgb(0.0, 0.5, 0.5),   // Glow
                ..default()
            }),
            transform: Transform::from_xyz(0.0, spawn_y, 0.0),
            ..default()
        },
    )).id();
    
    // CAMERA - First/Third Person, attached to Player as child
    // Press V to toggle between first and third person views
    commands.spawn((
        PlayerCamera,
        CameraMode {
            third_person: true,  // Start in third person to see the player
            distance: 2.0,       // Distance behind player
        },
        Camera3dBundle {
            camera: Camera {
                hdr: true, // Required for bloom
                ..default()
            },
            tonemapping: Tonemapping::TonyMcMapface,
            transform: Transform::from_xyz(0.0, 1.0, 2.0)  // Behind and above player
                .looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
            ..default()
        },
        BloomSettings {
            intensity: 0.3,
            low_frequency_boost: 0.7,
            high_pass_frequency: 1.0,
            ..default()
        },
    )).set_parent(player_id);
    
    info!("Player spawned with Physics Character Controller");
    
    // =========================================================================
    // THE VALIDATOR BEAM - Exit Point Visual
    // A glowing cylinder at origin marking the extraction zone
    // =========================================================================
    commands.spawn((
        Name::new("ValidatorBeam"),
        PbrBundle {
            mesh: meshes.add(Cylinder::new(4.0, 200.0)), // Radius 4, Height 200
            material: materials.add(StandardMaterial {
                base_color: Color::rgba(0.0, 0.8, 1.0, 0.3), // Cyan transparent
                emissive: Color::rgb(0.0, 2.0, 3.0),         // Strong glow
                alpha_mode: bevy::pbr::AlphaMode::Blend,
                unlit: true, // Don't receive shadows
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 100.0, 0.0), // Center beam
            ..default()
        },
    ));
    info!("Validator Beam spawned at origin - this is the EXIT!");
    
    // =========================================================================
    // UNDERCITY LIGHTING - Dramatic 3-Point Setup
    // =========================================================================
    
    // Main light: Bright white from above
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 25000.0,  // MUCH brighter
            shadows_enabled: true,
            color: Color::rgb(0.9, 0.9, 1.0), // Near white
            ..default()
        },
        transform: Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -0.8,
            0.3,
            0.0,
        )),
        ..default()
    });
    
    // Accent Light 1: CYAN (bright)
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 500000.0,  // Much brighter
            color: Color::rgb(0.0, 1.0, 1.0),
            range: 150.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(-50.0, 70.0, -50.0),
        ..default()
    });
    
    // Accent Light 2: MAGENTA (bright)
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 400000.0,
            color: Color::rgb(1.0, 0.0, 0.8),
            range: 120.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(50.0, 65.0, 50.0),
        ..default()
    });
    
    // Accent Light 3: ORANGE (bright)
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 300000.0,
            color: Color::rgb(1.0, 0.6, 0.0),
            range: 100.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 60.0, -60.0),
        ..default()
    });
    
    // Player carried light - MUCH STRONGER flashlight
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 150000.0,  // Strong flashlight
            color: Color::rgb(1.0, 0.98, 0.9), // Warm white
            range: 50.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 2.0, 0.0),
        ..default()
    }).set_parent(player_id);
    
    // BRIGHTER ambient light - can see everywhere
    commands.insert_resource(AmbientLight {
        color: Color::rgb(0.3, 0.3, 0.35), // Light grey-blue
        brightness: 200.0,                  // Much brighter!
    });
    
    info!("UNDERCITY initialized. WASD to move, SPACE to jump, mouse to look.");
    info!("Find the CYAN BEAM at origin to EXTRACT!");
    info!("===========================================");
}

/// Find ground height at position (for UNDERCITY terrain)
fn find_ground_height(world: &WorldManager, x: i32, z: i32) -> i32 {
    // Search from top down to find first solid block
    for y in (0..128).rev() {
        if let Some(block) = world.get_block(x, y, z) {
            if BlockType::from_id(block.id).is_solid() {
                return y + 1;
            }
        }
    }
    // Default to surface level if nothing found (should be ~48)
    50
}

// =============================================================================
// MINING SYSTEM - Block Breaking & Placing
// =============================================================================

/// Maximum reach distance for mining (in blocks)
const MINING_REACH: f32 = 5.0;

/// System to handle block mining (breaking/placing)
fn handle_mining(
    mouse_button: Res<ButtonInput<MouseButton>>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    player_query: Query<&Transform, With<Player>>,
    bridge: Res<BackendBridge>,
    mut gizmos: Gizmos,
) {
    let Ok(camera_global) = camera_query.get_single() else {
        return;
    };
    let Ok(_player_transform) = player_query.get_single() else {
        return;
    };
    
    // Get camera world position and forward direction
    let ray_origin = camera_global.translation();
    let ray_direction = camera_global.forward(); // GlobalTransform::forward() returns Vec3
    
    // Simple voxel raycast (DDA algorithm)
    if let Some((hit_pos, hit_normal)) = voxel_raycast(&bridge, ray_origin, ray_direction, MINING_REACH) {
        // Draw crosshair at hit point
        let hit_world = Vec3::new(hit_pos.0 as f32 + 0.5, hit_pos.1 as f32 + 0.5, hit_pos.2 as f32 + 0.5);
        gizmos.cuboid(
            Transform::from_translation(hit_world).with_scale(Vec3::splat(1.02)),
            Color::rgba(1.0, 1.0, 0.0, 0.5),
        );
        
        // Left Click: Break block (set to Air - ID 0)
        if mouse_button.just_pressed(MouseButton::Left) {
            let mut inner = bridge.inner.lock().unwrap();
            if inner.world_manager.set_block(hit_pos.0, hit_pos.1 as i32, hit_pos.2, 0) {
                info!("Block broken at ({}, {}, {})", hit_pos.0, hit_pos.1, hit_pos.2);
                // Mark chunk as dirty for re-meshing
                let chunk_coord = ChunkCoord::new(
                    hit_pos.0.div_euclid(CHUNK_SIZE as i32),
                    hit_pos.2.div_euclid(CHUNK_SIZE as i32),
                );
                inner.dirty_chunks.insert(chunk_coord);
            }
        }
        
        // Right Click: Place block (Gold - ID 3)
        if mouse_button.just_pressed(MouseButton::Right) {
            // Place at adjacent position (using normal)
            let place_pos = (
                hit_pos.0 + hit_normal.0,
                (hit_pos.1 as i32 + hit_normal.1) as usize,
                hit_pos.2 + hit_normal.2,
            );
            
            let mut inner = bridge.inner.lock().unwrap();
            if inner.world_manager.set_block(place_pos.0, place_pos.1 as i32, place_pos.2, 3) {
                info!("Block placed at ({}, {}, {})", place_pos.0, place_pos.1, place_pos.2);
                // Mark chunk as dirty for re-meshing
                let chunk_coord = ChunkCoord::new(
                    place_pos.0.div_euclid(CHUNK_SIZE as i32),
                    place_pos.2.div_euclid(CHUNK_SIZE as i32),
                );
                inner.dirty_chunks.insert(chunk_coord);
            }
        }
    }
}

/// Simple voxel raycast using DDA algorithm
/// Returns (hit_position, hit_normal) or None
fn voxel_raycast(
    bridge: &BackendBridge,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<((i32, usize, i32), (i32, i32, i32))> {
    let inner = bridge.inner.lock().unwrap();
    
    // Current voxel position
    let mut x = origin.x.floor() as i32;
    let mut y = origin.y.floor() as i32;
    let mut z = origin.z.floor() as i32;
    
    // Direction signs
    let step_x = if direction.x > 0.0 { 1 } else { -1 };
    let step_y = if direction.y > 0.0 { 1 } else { -1 };
    let step_z = if direction.z > 0.0 { 1 } else { -1 };
    
    // Delta distances
    let delta_x = if direction.x.abs() < 0.0001 { f32::MAX } else { (1.0 / direction.x).abs() };
    let delta_y = if direction.y.abs() < 0.0001 { f32::MAX } else { (1.0 / direction.y).abs() };
    let delta_z = if direction.z.abs() < 0.0001 { f32::MAX } else { (1.0 / direction.z).abs() };
    
    // Initial t values
    let mut t_max_x = if direction.x > 0.0 {
        ((x + 1) as f32 - origin.x) * delta_x
    } else {
        (origin.x - x as f32) * delta_x
    };
    let mut t_max_y = if direction.y > 0.0 {
        ((y + 1) as f32 - origin.y) * delta_y
    } else {
        (origin.y - y as f32) * delta_y
    };
    let mut t_max_z = if direction.z > 0.0 {
        ((z + 1) as f32 - origin.z) * delta_z
    } else {
        (origin.z - z as f32) * delta_z
    };
    
    let mut distance = 0.0;
    let mut last_normal = (0, 0, 0);
    
    while distance < max_distance {
        // Check current voxel
        if y >= 0 && y < 256 {
            if let Some(block) = inner.world_manager.get_block(x, y, z) {
                if BlockType::from_id(block.id).is_solid() {
                    return Some(((x, y as usize, z), last_normal));
                }
            }
        }
        
        // Step to next voxel
        if t_max_x < t_max_y {
            if t_max_x < t_max_z {
                x += step_x;
                distance = t_max_x;
                t_max_x += delta_x;
                last_normal = (-step_x, 0, 0);
            } else {
                z += step_z;
                distance = t_max_z;
                t_max_z += delta_z;
                last_normal = (0, 0, -step_z);
            }
        } else {
            if t_max_y < t_max_z {
                y += step_y;
                distance = t_max_y;
                t_max_y += delta_y;
                last_normal = (0, -step_y, 0);
            } else {
                z += step_z;
                distance = t_max_z;
                t_max_z += delta_z;
                last_normal = (0, 0, -step_z);
            }
        }
    }
    
    None
}

// =============================================================================
// PHYSICS MOVEMENT CONTROLLER - WASD + Jump
// =============================================================================

/// Movement speed (units per second) - scaled for smaller blocks
const PLAYER_SPEED: f32 = 2.0;
/// Jump impulse (vertical velocity) - scaled
const JUMP_IMPULSE: f32 = 2.5;

/// Physics-based movement controller
/// Uses WASD for horizontal movement, SPACE for jump
fn movement_controller(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&Transform, &mut LinearVelocity), With<Player>>,
    camera_query: Query<&Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    let Ok((player_transform, mut velocity)) = player_query.get_single_mut() else {
        return;
    };
    
    // Get camera direction for movement relative to view
    let camera_forward = if let Ok(cam_transform) = camera_query.get_single() {
        // Use parent (player) transform combined with camera local transform
        let world_cam = player_transform.rotation * cam_transform.rotation;
        let forward = world_cam * Vec3::NEG_Z;
        Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero()
    } else {
        let fwd = player_transform.forward();
        Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero()
    };
    
    let camera_right = Vec3::new(camera_forward.z, 0.0, -camera_forward.x);
    
    // Calculate movement direction from input
    let mut move_dir = Vec3::ZERO;
    
    if keyboard.pressed(KeyCode::KeyW) {
        move_dir += camera_forward;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        move_dir -= camera_forward;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        move_dir -= camera_right;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        move_dir += camera_right;
    }
    
    // Normalize and apply horizontal movement (preserve vertical velocity)
    if move_dir.length_squared() > 0.0 {
        move_dir = move_dir.normalize();
        velocity.x = move_dir.x * PLAYER_SPEED;
        velocity.z = move_dir.z * PLAYER_SPEED;
    }
    
    // Ground check based on velocity (if not moving down fast, probably grounded)
    // This is a simple approximation; ShapeCaster provides better detection
    let is_grounded = velocity.y.abs() < 0.5;
    
    // Jump (only when grounded)
    if keyboard.just_pressed(KeyCode::Space) && is_grounded {
        velocity.y = JUMP_IMPULSE;
    }
}

// =============================================================================
// MOUSE LOOK - Rotate player/camera based on mouse movement
// =============================================================================

/// Mouse sensitivity for looking around
const MOUSE_SENSITIVITY: f32 = 0.003;

/// System to toggle camera mode (V key)
fn toggle_camera_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut camera_query: Query<(&mut CameraMode, &mut Transform), With<PlayerCamera>>,
) {
    if keyboard.just_pressed(KeyCode::KeyV) {
        if let Ok((mut mode, mut transform)) = camera_query.get_single_mut() {
            mode.third_person = !mode.third_person;
            
            if mode.third_person {
                // Third person: behind and above
                transform.translation = Vec3::new(0.0, 0.8, mode.distance);
                transform.look_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y);
                info!("Camera: Third Person (press V to change)");
            } else {
                // First person: at eye level
                transform.translation = Vec3::new(0.0, 0.35, 0.0);
                transform.rotation = Quat::IDENTITY;
                info!("Camera: First Person (press V to change)");
            }
        }
    }
}

/// System to handle mouse look (rotate player body for yaw, camera for pitch)
fn mouse_look(
    mut mouse_motion: EventReader<bevy::input::mouse::MouseMotion>,
    mut player_query: Query<&mut Transform, With<Player>>,
    mut camera_query: Query<(&mut Transform, &CameraMode), (With<PlayerCamera>, Without<Player>)>,
) {
    let mut delta = Vec2::ZERO;
    for event in mouse_motion.read() {
        delta += event.delta;
    }
    
    if delta == Vec2::ZERO {
        return;
    }
    
    // Rotate player body (yaw - left/right)
    if let Ok(mut player_transform) = player_query.get_single_mut() {
        player_transform.rotate_y(-delta.x * MOUSE_SENSITIVITY);
    }
    
    // Handle camera based on mode
    if let Ok((mut camera_transform, mode)) = camera_query.get_single_mut() {
        if mode.third_person {
            // Third person: orbit camera around player
            let current_pitch = camera_transform.rotation.to_euler(EulerRot::YXZ).1;
            let new_pitch = (current_pitch - delta.y * MOUSE_SENSITIVITY).clamp(-0.8, 1.2);
            
            // Update camera position based on pitch
            let distance = mode.distance;
            let height = 0.6 + distance * new_pitch.sin().abs();
            let back = distance * new_pitch.cos().max(0.3);
            
            camera_transform.translation = Vec3::new(0.0, height, back);
            camera_transform.look_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y);
        } else {
            // First person: rotate camera pitch (up/down)
            let pitch = (camera_transform.rotation.to_euler(EulerRot::YXZ).1 - delta.y * MOUSE_SENSITIVITY)
                .clamp(-1.5, 1.5); // ~85 degrees up/down
            camera_transform.rotation = Quat::from_rotation_x(pitch);
        }
    }
}

// =============================================================================
// VOID FALL RESPAWN - Teleport player if they fall off the map
// =============================================================================

/// If player falls below Y=-2.5 (scaled), respawn them at the center
fn check_void_fall(
    mut player_query: Query<(&mut Transform, &mut LinearVelocity), With<Player>>,
) {
    let Ok((mut transform, mut velocity)) = player_query.get_single_mut() else {
        return;
    };
    
    // Scaled: -10 blocks * 0.25 = -2.5 units
    if transform.translation.y < -2.5 {
        // Respawn at safe height above arena center (scaled)
        transform.translation = Vec3::new(0.0, 3.0 * BLOCK_SCALE, 0.0);
        // Reset velocity
        velocity.0 = Vec3::ZERO;
        info!("Player respawned (fell into void)");
    }
}

// =============================================================================
// WASM POINTER LOCK - Click to grab mouse
// =============================================================================

/// System to grab mouse on click (WASM requires user gesture for pointer lock)
#[cfg(target_arch = "wasm32")]
fn grab_mouse_on_click(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window>,
) {
    use bevy::window::CursorGrabMode;
    
    if mouse_button.just_pressed(MouseButton::Left) {
        if let Ok(mut window) = windows.get_single_mut() {
            // Only grab if not already grabbed
            if window.cursor.grab_mode == CursorGrabMode::None {
                window.cursor.grab_mode = CursorGrabMode::Locked;
                window.cursor.visible = false;
                info!("Mouse grabbed - pointer lock active");
            }
        }
    }
}

/// System to release mouse on Escape (WASM)
#[cfg(target_arch = "wasm32")]
fn release_mouse_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window>,
) {
    use bevy::window::CursorGrabMode;
    
    if keyboard.just_pressed(KeyCode::Escape) {
        if let Ok(mut window) = windows.get_single_mut() {
            window.cursor.grab_mode = CursorGrabMode::None;
            window.cursor.visible = true;
            info!("Mouse released - pointer lock disabled");
        }
    }
}

// =============================================================================
// MAIN - The Entry Point
// =============================================================================

fn main() {
    // WASM: Install panic hook for better error messages in browser console
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
    
    let mut app = App::new();
    
    // Configure plugins differently for WASM vs Native
    #[cfg(target_arch = "wasm32")]
    {
        // WASM: Single-threaded + Canvas binding + Cursor unlocked initially
        use bevy::core::TaskPoolPlugin;
        use bevy::window::CursorGrabMode;
        use bevy::window::WindowMode;
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "GLITCH WARS - The Simulation".into(),
                        // FULL SCREEN: Large resolution, CSS scales to viewport
                        mode: WindowMode::BorderlessFullscreen,
                        // CRITICAL: Bind to canvas element in index.html
                        canvas: Some("#bevy".into()),
                        prevent_default_event_handling: true,
                        // WASM: Start with cursor UNLOCKED (user must click first)
                        cursor: bevy::window::Cursor {
                            visible: true,
                            grab_mode: CursorGrabMode::None,
                            ..default()
                        },
                        ..default()
                    }),
                    ..default()
                })
                .set(TaskPoolPlugin {
                    task_pool_options: bevy::core::TaskPoolOptions::with_num_threads(1),
                })
        );
    }
    
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Native: Full multi-threading
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "GLITCH WARS - The Simulation".into(),
                resolution: (1280., 720.).into(),
                ..default()
            }),
            ..default()
        }));
    }
    
    // ENTERPRISE PHYSICS - bevy_xpbd_3d
    // Note: parallel disabled for WASM compatibility (handled by TaskPoolOptions above)
    app.add_plugins(PhysicsPlugins::default());
    
    // NOTE: PhysicsDebugPlugin removed - debug wireframes disabled
    
    // Configure physics timestep for smooth gameplay
    app.insert_resource(bevy_xpbd_3d::prelude::SubstepCount(4));
    
    app
        // THE VOID - Almost black background (#050505)
        .insert_resource(ClearColor(Color::rgb(0.02, 0.02, 0.02)))
        
        // Configure gravity (slightly stronger for snappy movement)
        .insert_resource(Gravity(Vec3::new(0.0, -20.0, 0.0)))
        
        // NOTE: AtmospherePlugin disabled for WASM compatibility
        // NOTE: bevy_flycam REMOVED - was causing noclip/flying
        // All camera movement is now physics-based
        
        // Our resources
        .insert_resource(BackendBridge::new(WORLD_SEED))
        .insert_resource(RenderedChunks::default())
        
        // Our systems
        .add_systems(Startup, setup)
        .add_systems(Update, update_chunk_streaming)
        .add_systems(Update, sync_chunks_to_bevy.after(update_chunk_streaming))
        .add_systems(Update, unload_distant_chunks.after(sync_chunks_to_bevy))
        // Physics-based movement (WASD + Jump)
        .add_systems(Update, movement_controller)
        // Camera mode toggle (V key)
        .add_systems(Update, toggle_camera_mode)
        // Mouse look (rotate player/camera)
        .add_systems(Update, mouse_look)
        // Void fall respawn
        .add_systems(Update, check_void_fall)
        // Mining system (block breaking/placing)
        .add_systems(Update, handle_mining);
    
    // WASM: Add pointer lock systems (click to grab, escape to release)
    #[cfg(target_arch = "wasm32")]
    {
        app.add_systems(Update, grab_mouse_on_click);
        app.add_systems(Update, release_mouse_on_escape);
    }
        
    app.run();
}
