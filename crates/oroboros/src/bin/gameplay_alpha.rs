//! # OROBOROS Gameplay Alpha
//!
//! JOINT MISSION: UNIT 2 + UNIT 4
//!
//! Features:
//! - Gravity & AABB Collision (Unit 4)
//! - Kinematic Character Controller (Unit 4)
//! - Voxel Raycasting (Unit 4)
//! - Block Breaking on Left Click (Unit 4)
//! - Wireframe Selection Cube (Unit 2)
//! - **NPC System with AI** (Unit 4)
//!
//! CEO FEEDBACK: "I want physics, gravity, and mining."
//! CEO FEEDBACK: "I want to see characters."

use winit::{
    event::{Event, WindowEvent, KeyEvent, ElementState, DeviceEvent, MouseButton},
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, CursorGrabMode},
    keyboard::{KeyCode, PhysicalKey},
    dpi::PhysicalSize,
};
use std::sync::Arc;
use std::time::Instant;
use std::collections::HashSet;

// Physics from Unit 4
use oroboros::physics::{
    CharacterController, VoxelWorld,
    get_look_direction, generate_wireframe_cube, RaycastHit,
};

// NPC System - LIFE INJECTION
use oroboros::gameplay::NpcManager;

// ============================================================================
// CONSTANTS
// ============================================================================
const RAYCAST_DISTANCE: f32 = 8.0; // How far player can reach
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

// ============================================================================
// VOXEL WORLD STATE (Mutable for block breaking)
// ============================================================================
struct GameWorld {
    /// Set of removed blocks (for mining)
    removed_blocks: HashSet<(i32, i32, i32)>,
    /// The voxel world for collision
    voxel_world: VoxelWorld,
}

impl GameWorld {
    fn new() -> Self {
        Self {
            removed_blocks: HashSet::new(),
            voxel_world: VoxelWorld::new(),
        }
    }

    /// Checks if a voxel is solid (considering removed blocks).
    fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        if self.removed_blocks.contains(&(x, y, z)) {
            return false;
        }
        self.voxel_world.is_solid(x, y, z)
    }

    /// Breaks a block at the given coordinates.
    fn break_block(&mut self, x: i32, y: i32, z: i32) -> bool {
        if self.is_solid(x, y, z) {
            self.removed_blocks.insert((x, y, z));
            println!("[MINING] ⛏️ Block broken at [{}, {}, {}]", x, y, z);
            true
        } else {
            false
        }
    }

    /// Gets the voxel world for physics/rendering.
    fn voxel_world(&self) -> &VoxelWorld {
        &self.voxel_world
    }
}

// ============================================================================
// GPU DATA STRUCTURES
// ============================================================================

/// Per-instance data for voxel rendering
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct VoxelInstance {
    position_scale: [f32; 4],
    dimensions_normal_material: [f32; 4],
    emission: [f32; 4],
    uv_offset_scale: [f32; 4],
}

/// Camera uniform buffer
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    camera_params: [f32; 4],
}

/// Wireframe vertex
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct WireframeVertex {
    position: [f32; 3],
    _padding: f32,
}

/// NPC render instance (colored box)
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct NpcRenderInstance {
    position_scale: [f32; 4],  // x, y, z, scale
    color: [f32; 4],           // r, g, b, a
}

// ============================================================================
// MATH HELPERS
// ============================================================================
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]] }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }
fn normalize(v: [f32; 3]) -> [f32; 3] { let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt(); if l > 0.0001 { [v[0]/l, v[1]/l, v[2]/l] } else { [0.0, 0.0, 0.0] } }

fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let f = normalize(sub(target, eye));
    let r = normalize(cross(f, up));
    let u = cross(r, f);
    let tx = -dot(r, eye);
    let ty = -dot(u, eye);
    let tz = dot(f, eye);
    [
        [r[0], r[1], r[2], 0.0],
        [u[0], u[1], u[2], 0.0],
        [-f[0], -f[1], -f[2], 0.0],
        [tx, ty, tz, 1.0],
    ]
}

fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov / 2.0).tan();
    let a = far / (near - far);
    let b = (near * far) / (near - far);
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, a, -1.0],
        [0.0, 0.0, b, 0.0],
    ]
}

fn multiply_matrices(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            for k in 0..4 {
                result[col][row] += a[k][row] * b[col][k];
            }
        }
    }
    result
}

// ============================================================================
// VOXEL GENERATION (Same as original but considers removed blocks)
// ============================================================================
fn generate_voxel_instances(world: &GameWorld) -> Vec<VoxelInstance> {
    let mut instances = Vec::new();
    
    println!("[VOXEL] ╔═══════════════════════════════════════════════════════════╗");
    println!("[VOXEL] ║     GAMEPLAY ALPHA - VOXEL WORLD GENERATION               ║");
    println!("[VOXEL] ╚═══════════════════════════════════════════════════════════╝");
    
    // Generate terrain chunks
    let chunk_range = 3;
    let chunk_size = 32;
    
    for cx in -chunk_range..=chunk_range {
        for cz in -chunk_range..=chunk_range {
            let chunk_instances = generate_chunk(cx, cz, chunk_size, world);
            instances.extend(chunk_instances);
        }
    }
    
    println!("[VOXEL] Generated {} instances", instances.len());
    instances
}

fn generate_chunk(cx: i32, cz: i32, chunk_size: i32, world: &GameWorld) -> Vec<VoxelInstance> {
    let mut instances = Vec::new();

    // Generate individual voxels (for mining we need individual blocks)
    for lx in 0..chunk_size {
        for lz in 0..chunk_size {
            let x = cx * chunk_size + lx;
            let z = cz * chunk_size + lz;
            let height = world.voxel_world().get_height(x, z);

            for y in 0..height {
                if !world.is_solid(x, y, z) {
                    continue; // Skip removed blocks
                }

                // Add top face if exposed
                if !world.is_solid(x, y + 1, z) {
                    let material = if y == height - 1 { 1.0 } else { 10.0 }; // Grass on top, stone below
                    instances.push(VoxelInstance {
                        position_scale: [x as f32, (y + 1) as f32, z as f32, 1.0],
                        dimensions_normal_material: [1.0, 1.0, 2.0, material], // Top face
                        emission: [0.0, 0.0, 0.0, 0.0],
                        uv_offset_scale: [0.0, 0.0, 1.0, 1.0],
                    });
                }

                // Add side faces if exposed
                if !world.is_solid(x + 1, y, z) {
                    instances.push(VoxelInstance {
                        position_scale: [(x + 1) as f32, y as f32, z as f32, 1.0],
                        dimensions_normal_material: [1.0, 1.0, 0.0, 11.0], // +X face
                        emission: [0.0, 0.0, 0.0, 0.0],
                        uv_offset_scale: [0.0, 0.0, 1.0, 1.0],
                    });
                }
                if !world.is_solid(x - 1, y, z) {
                    instances.push(VoxelInstance {
                        position_scale: [x as f32, y as f32, z as f32, 1.0],
                        dimensions_normal_material: [1.0, 1.0, 1.0, 11.0], // -X face
                        emission: [0.0, 0.0, 0.0, 0.0],
                        uv_offset_scale: [0.0, 0.0, 1.0, 1.0],
                    });
                }
                if !world.is_solid(x, y, z + 1) {
                    instances.push(VoxelInstance {
                        position_scale: [x as f32, y as f32, (z + 1) as f32, 1.0],
                        dimensions_normal_material: [1.0, 1.0, 4.0, 11.0], // +Z face
                        emission: [0.0, 0.0, 0.0, 0.0],
                        uv_offset_scale: [0.0, 0.0, 1.0, 1.0],
                    });
                }
                if !world.is_solid(x, y, z - 1) {
                    instances.push(VoxelInstance {
                        position_scale: [x as f32, y as f32, z as f32, 1.0],
                        dimensions_normal_material: [1.0, 1.0, 5.0, 11.0], // -Z face
                        emission: [0.0, 0.0, 0.0, 0.0],
                        uv_offset_scale: [0.0, 0.0, 1.0, 1.0],
                    });
                }
            }
        }
    }

    instances
}

fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&Default::default())
}

// ============================================================================
// MAIN
// ============================================================================
fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║           OROBOROS GAMEPLAY ALPHA v1.2                            ║");
    println!("║           JOINT MISSION: UNIT 2 + UNIT 4                          ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Features:                                                        ║");
    println!("║    ✓ Gravity & Physics (Unit 4)                                   ║");
    println!("║    ✓ AABB Collision (Unit 4)                                      ║");
    println!("║    ✓ Block Raycasting (Unit 4)                                    ║");
    println!("║    ✓ Mining on Left Click (Unit 4)                                ║");
    println!("║    ✓ Wireframe Selection (Unit 2)                                 ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  NEW - LIFE INJECTION (NPCs):                                     ║");
    println!("║    ✓ NPC Entity System with AI                                    ║");
    println!("║    ✓ State Machine: Idle / Wander / LookAtPlayer                  ║");
    println!("║    ✓ NPC Physics (gravity + collision)                            ║");
    println!("║    ✓ Colored boxes moving on their own!                           ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    // Create window
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("OROBOROS Gameplay Alpha")
            .with_inner_size(PhysicalSize::new(1280, 720))
            .build(&event_loop)
            .expect("Failed to create window")
    );

    // Initialize wgpu
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let surface = instance.create_surface(window.clone()).expect("Failed to create surface");
    
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    })).expect("Failed to find adapter");

    println!("[GPU] Using: {}", adapter.get_info().name);

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        },
        None,
    )).expect("Failed to create device");

    let size = window.inner_size();
    let format = surface.get_capabilities(&adapter).formats[0];
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    // Create depth texture
    let mut depth_view = create_depth_texture(&device, config.width, config.height);

    // Create game world
    let mut game_world = GameWorld::new();

    // Create NPC manager and spawn initial NPCs
    let mut npc_manager = NpcManager::new();
    
    // Spawn NPCs in chunks around spawn
    println!("[NPC] 🌱 Spawning NPCs in world...");
    let chunk_size = 32;
    for cx in -2..=2 {
        for cz in -2..=2 {
            let get_height = |x: i32, z: i32| game_world.voxel_world().get_height(x, z);
            npc_manager.try_spawn_for_chunk(cx, cz, chunk_size, get_height);
        }
    }
    println!("[NPC] ✓ Spawned {} NPCs", npc_manager.count());

    // Generate voxels
    let all_instances = generate_voxel_instances(&game_world);
    let instance_count = all_instances.len() as u32;

    // Create shader
    let shader_source = include_str!("../../assets/shaders/gameplay_alpha.wgsl");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Voxel Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    // Camera uniform buffer
    let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Camera Buffer"),
        size: std::mem::size_of::<CameraUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Bind group layout
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            },
        ],
    });

    // Pipeline layout
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    // Voxel render pipeline
    let voxel_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Voxel Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<VoxelInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute { offset: 0, shader_location: 5, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 16, shader_location: 6, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 32, shader_location: 7, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 48, shader_location: 8, format: wgpu::VertexFormat::Float32x4 },
                    ],
                },
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Ccw,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    // Create instance buffer
    use wgpu::util::DeviceExt;
    let mut instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Instance Buffer"),
        contents: bytemuck::cast_slice(&all_instances),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });

    // Create wireframe pipeline for selection cube
    let wireframe_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Wireframe Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_wireframe",
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<WireframeVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
                    ],
                },
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_wireframe",
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    // Wireframe buffer (updated each frame)
    let wireframe_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Wireframe Buffer"),
        size: std::mem::size_of::<WireframeVertex>() as u64 * 24,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // NPC render pipeline (colored boxes)
    let npc_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("NPC Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_npc",
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<NpcRenderInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 16, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
                    ],
                },
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_npc",
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Ccw,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    // NPC instance buffer (will be updated each frame)
    let max_npcs = 256;
    let npc_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("NPC Buffer"),
        size: std::mem::size_of::<NpcRenderInstance>() as u64 * max_npcs as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                      RENDER ENGINE READY                         ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Voxel Instances: {:>8}                                      ║", instance_count);
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  CONTROLS:                                                       ║");
    println!("║    WASD   - Move                                                 ║");
    println!("║    SPACE  - Jump                                                 ║");
    println!("║    SHIFT  - Sprint                                               ║");
    println!("║    Mouse  - Look around                                          ║");
    println!("║    LMB    - Break block                                          ║");
    println!("║    Click  - Capture mouse                                        ║");
    println!("║    ESC    - Exit                                                 ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    // Game state
    let mut character = CharacterController::new([0.0, 20.0, 0.0]);
    let mut yaw: f32 = 0.0;
    let mut pitch: f32 = 0.0;
    let mut keys_pressed = [false; 6]; // W, A, S, D, Space, Shift
    let mut mouse_captured = false;
    let mut last_frame = Instant::now();
    let mut frame_count: u64 = 0;
    let mut last_fps_print = Instant::now();
    let start_time = Instant::now();
    let mut current_raycast_hit: Option<RaycastHit> = None;
    let mut needs_mesh_update = false;
    let mut current_instance_count = instance_count;

    // Run
    let _ = event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(key), state, .. }, .. } => {
                        let pressed = state == ElementState::Pressed;
                        match key {
                            KeyCode::Escape => elwt.exit(),
                            KeyCode::KeyW => keys_pressed[0] = pressed,
                            KeyCode::KeyA => keys_pressed[1] = pressed,
                            KeyCode::KeyS => keys_pressed[2] = pressed,
                            KeyCode::KeyD => keys_pressed[3] = pressed,
                            KeyCode::Space => {
                                keys_pressed[4] = pressed;
                                if pressed {
                                    character.jump();
                                }
                            }
                            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                                keys_pressed[5] = pressed;
                                character.sprinting = pressed;
                            }
                            _ => {}
                        }
                    }
                    WindowEvent::MouseInput { button: MouseButton::Left, state: ElementState::Pressed, .. } => {
                        if !mouse_captured {
                            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                            window.set_cursor_visible(false);
                            mouse_captured = true;
                        } else {
                            // Break block!
                            if let Some(hit) = current_raycast_hit {
                                if game_world.break_block(hit.voxel[0], hit.voxel[1], hit.voxel[2]) {
                                    needs_mesh_update = true;
                                }
                            }
                        }
                    }
                    WindowEvent::Focused(false) => {
                        let _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                        mouse_captured = false;
                    }
                    WindowEvent::Resized(new_size) => {
                        if new_size.width > 0 && new_size.height > 0 {
                            config.width = new_size.width;
                            config.height = new_size.height;
                            surface.configure(&device, &config);
                            depth_view = create_depth_texture(&device, config.width, config.height);
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = (now - last_frame).as_secs_f32().min(0.1);
                        last_frame = now;
                        frame_count += 1;

                        // Apply movement input
                        let forward = if keys_pressed[0] { 1.0 } else { 0.0 } - if keys_pressed[2] { 1.0 } else { 0.0 };
                        let right = if keys_pressed[3] { 1.0 } else { 0.0 } - if keys_pressed[1] { 1.0 } else { 0.0 };
                        character.apply_input(forward, right, yaw);

                        // Update player physics
                        character.update(dt, game_world.voxel_world());

                        // Update all NPCs (AI + Physics)
                        npc_manager.update(dt, game_world.voxel_world(), character.position);

                        // Get camera position
                        let eye = character.eye_position();
                        let look_dir = get_look_direction(yaw, pitch);
                        let target = [eye[0] + look_dir[0], eye[1] + look_dir[1], eye[2] + look_dir[2]];

                        // Raycast for block selection (uses game_world which respects removed blocks)
                        current_raycast_hit = raycast_with_removed(
                            eye,
                            look_dir,
                            RAYCAST_DISTANCE,
                            &game_world,
                        );

                        // Update wireframe buffer if we have a hit
                        if let Some(hit) = current_raycast_hit {
                            let wireframe_verts = generate_wireframe_cube(hit.voxel);
                            let wireframe_data: Vec<WireframeVertex> = wireframe_verts
                                .iter()
                                .map(|&pos| WireframeVertex { position: pos, _padding: 0.0 })
                                .collect();
                            queue.write_buffer(&wireframe_buffer, 0, bytemuck::cast_slice(&wireframe_data));

                            // Print raycast hit occasionally
                            if frame_count % 60 == 1 {
                                println!("[RAYCAST] Looking at block [{}, {}, {}]", 
                                    hit.voxel[0], hit.voxel[1], hit.voxel[2]);
                            }
                        }

                        // Update mesh if blocks were broken
                        if needs_mesh_update {
                            let new_instances = generate_voxel_instances(&game_world);
                            current_instance_count = new_instances.len() as u32;
                            
                            instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Instance Buffer"),
                                contents: bytemuck::cast_slice(&new_instances),
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            });
                            
                            needs_mesh_update = false;
                            println!("[MESH] Rebuilt with {} instances", current_instance_count);
                        }

                        // Compute matrices
                        let view = look_at(eye, target, [0.0, 1.0, 0.0]);
                        let aspect = config.width as f32 / config.height as f32;
                        let proj = perspective(70.0_f32.to_radians(), aspect, 0.1, 1000.0);
                        let view_proj = multiply_matrices(proj, view);

                        let uniform = CameraUniform {
                            view_proj,
                            view,
                            projection: proj,
                            camera_pos: [eye[0], eye[1], eye[2], 1.0],
                            camera_params: [0.1, 1000.0, aspect, 70.0_f32.to_radians()],
                        };
                        queue.write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&uniform));

                        // Render
                        let output = match surface.get_current_texture() {
                            Ok(t) => t,
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                surface.configure(&device, &config);
                                return;
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                eprintln!("[FATAL] Out of GPU memory!");
                                elwt.exit();
                                return;
                            }
                            Err(wgpu::SurfaceError::Timeout) => return,
                        };

                        let view = output.texture.create_view(&Default::default());
                        let mut encoder = device.create_command_encoder(&Default::default());

                        {
                            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Main Pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.4, g: 0.6, b: 0.9, a: 1.0 }),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                    view: &depth_view,
                                    depth_ops: Some(wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(1.0),
                                        store: wgpu::StoreOp::Store,
                                    }),
                                    stencil_ops: None,
                                }),
                                ..Default::default()
                            });

                            // Draw voxels
                            pass.set_pipeline(&voxel_pipeline);
                            pass.set_bind_group(0, &bind_group, &[]);
                            pass.set_vertex_buffer(0, instance_buffer.slice(..));
                            pass.draw(0..6, 0..current_instance_count);

                            // Draw NPCs (colored boxes)
                            let npc_count = npc_manager.count() as u32;
                            if npc_count > 0 {
                                // Update NPC buffer
                                let npc_instances: Vec<NpcRenderInstance> = npc_manager.npcs()
                                    .iter()
                                    .map(|npc| {
                                        let center = npc.center();
                                        NpcRenderInstance {
                                            position_scale: [center[0], center[1], center[2], 1.0],
                                            color: npc.npc_type.color(),
                                        }
                                    })
                                    .collect();
                                queue.write_buffer(&npc_buffer, 0, bytemuck::cast_slice(&npc_instances));

                                pass.set_pipeline(&npc_pipeline);
                                pass.set_bind_group(0, &bind_group, &[]);
                                pass.set_vertex_buffer(0, npc_buffer.slice(..));
                                pass.draw(0..36, 0..npc_count); // 36 vertices for a cube (6 faces * 2 triangles * 3 verts)
                            }

                            // Draw selection wireframe
                            if current_raycast_hit.is_some() {
                                pass.set_pipeline(&wireframe_pipeline);
                                pass.set_bind_group(0, &bind_group, &[]);
                                pass.set_vertex_buffer(0, wireframe_buffer.slice(..));
                                pass.draw(0..24, 0..1);
                            }
                        }

                        queue.submit(std::iter::once(encoder.finish()));
                        output.present();

                        // FPS and status
                        if last_fps_print.elapsed().as_secs() >= 2 {
                            let fps = frame_count as f64 / start_time.elapsed().as_secs_f64();
                            let ground_status = if character.on_ground { "GROUNDED" } else { "AIRBORNE" };
                            let npc_count = npc_manager.count();
                            println!("[STATUS] FPS: {:.0} | Pos: ({:.1}, {:.1}, {:.1}) | {} | NPCs: {} | Instances: {}",
                                fps, character.position[0], character.position[1], character.position[2],
                                ground_status, npc_count, current_instance_count);
                            last_fps_print = Instant::now();
                        }
                    }
                    _ => {}
                }
            }
            Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                if mouse_captured {
                    yaw += delta.0 as f32 * 0.1;
                    pitch -= delta.1 as f32 * 0.1;
                    pitch = pitch.clamp(-89.0, 89.0);
                }
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    });
}

/// Raycast that respects removed blocks
fn raycast_with_removed(
    origin: [f32; 3],
    direction: [f32; 3],
    max_distance: f32,
    world: &GameWorld,
) -> Option<RaycastHit> {
    let len = (direction[0].powi(2) + direction[1].powi(2) + direction[2].powi(2)).sqrt();
    if len < 0.0001 {
        return None;
    }
    let dir = [direction[0] / len, direction[1] / len, direction[2] / len];

    let mut voxel = [
        origin[0].floor() as i32,
        origin[1].floor() as i32,
        origin[2].floor() as i32,
    ];

    let step = [
        if dir[0] >= 0.0 { 1 } else { -1 },
        if dir[1] >= 0.0 { 1 } else { -1 },
        if dir[2] >= 0.0 { 1 } else { -1 },
    ];

    let t_delta = [
        if dir[0].abs() < 0.0001 { f32::MAX } else { (1.0 / dir[0]).abs() },
        if dir[1].abs() < 0.0001 { f32::MAX } else { (1.0 / dir[1]).abs() },
        if dir[2].abs() < 0.0001 { f32::MAX } else { (1.0 / dir[2]).abs() },
    ];

    let mut t_max = [
        if dir[0] >= 0.0 {
            ((voxel[0] + 1) as f32 - origin[0]) / dir[0].max(0.0001)
        } else {
            (voxel[0] as f32 - origin[0]) / dir[0].min(-0.0001)
        },
        if dir[1] >= 0.0 {
            ((voxel[1] + 1) as f32 - origin[1]) / dir[1].max(0.0001)
        } else {
            (voxel[1] as f32 - origin[1]) / dir[1].min(-0.0001)
        },
        if dir[2] >= 0.0 {
            ((voxel[2] + 1) as f32 - origin[2]) / dir[2].max(0.0001)
        } else {
            (voxel[2] as f32 - origin[2]) / dir[2].min(-0.0001)
        },
    ];

    let mut distance = 0.0;
    let mut last_normal = [0, 0, 0];

    while distance < max_distance {
        // Use game_world.is_solid which respects removed blocks
        if world.is_solid(voxel[0], voxel[1], voxel[2]) {
            let hit_point = [
                origin[0] + dir[0] * distance,
                origin[1] + dir[1] * distance,
                origin[2] + dir[2] * distance,
            ];

            return Some(RaycastHit {
                voxel,
                normal: last_normal,
                distance,
                hit_point,
            });
        }

        if t_max[0] < t_max[1] && t_max[0] < t_max[2] {
            distance = t_max[0];
            t_max[0] += t_delta[0];
            voxel[0] += step[0];
            last_normal = [-step[0], 0, 0];
        } else if t_max[1] < t_max[2] {
            distance = t_max[1];
            t_max[1] += t_delta[1];
            voxel[1] += step[1];
            last_normal = [0, -step[1], 0];
        } else {
            distance = t_max[2];
            t_max[2] += t_delta[2];
            voxel[2] += step[2];
            last_normal = [0, 0, -step[2]];
        }
    }

    None
}
