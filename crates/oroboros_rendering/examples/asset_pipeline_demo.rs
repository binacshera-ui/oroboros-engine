//! Asset Pipeline Demo - Unit 6
//!
//! Demonstrates loading procedural models and VOX files.
//!
//! Run with: cargo run -p oroboros_rendering --example asset_pipeline_demo

use oroboros_rendering::assets::{
    ProceduralModels, VoxelModelBuilder,
    ModelAssetLoader, colors,
};

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║           OROBOROS ASSET PIPELINE - UNIT 6                 ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║  Bridge between Art & Code                                 ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    
    // === PART 1: PROCEDURAL MODELS ===
    println!("┌─────────────────────────────────────────────┐");
    println!("│  STRATEGY 1: HARDCODED MODELS (The Quick Fix) │");
    println!("└─────────────────────────────────────────────┘");
    println!();
    
    // Load all procedural models
    let all_models = ProceduralModels::all();
    println!("Available procedural models:");
    for model in &all_models {
        println!("  • {} - {} voxels ({}x{}x{})",
            model.name,
            model.voxel_count(),
            model.bounds.width,
            model.bounds.height,
            model.bounds.depth,
        );
    }
    println!();
    
    // Get specific models
    let player = ProceduralModels::player();
    let enemy = ProceduralModels::enemy();
    let dragon = ProceduralModels::dragon();
    
    println!("Player Model Details:");
    println!("  - Name: {}", player.name);
    println!("  - Bounds: {}x{}x{}", player.bounds.width, player.bounds.height, player.bounds.depth);
    println!("  - Total voxels: {}", player.voxel_count());
    println!("  - Origin offset: {:?}", player.origin);
    println!();
    
    println!("Enemy Model Details:");
    println!("  - Name: {}", enemy.name);
    println!("  - Bounds: {}x{}x{}", enemy.bounds.width, enemy.bounds.height, enemy.bounds.depth);
    println!("  - Total voxels: {}", enemy.voxel_count());
    println!();
    
    println!("Dragon Model Details (INFERNO WORLD BOSS):");
    println!("  - Name: {}", dragon.name);
    println!("  - Bounds: {}x{}x{}", dragon.bounds.width, dragon.bounds.height, dragon.bounds.depth);
    println!("  - Total voxels: {}", dragon.voxel_count());
    println!();
    
    // Convert to GPU instances
    let player_instances = player.to_instances(0.0, 0.0, 0.0);
    println!("Player converted to {} GPU instances (6 faces per voxel)", player_instances.len());
    println!();
    
    // === PART 2: MODEL BUILDER ===
    println!("┌─────────────────────────────────────────────┐");
    println!("│  CUSTOM MODEL BUILDING (Runtime Creation)    │");
    println!("└─────────────────────────────────────────────┘");
    println!();
    
    // Create a custom model at runtime
    let mut builder = VoxelModelBuilder::new("custom_tower")
        .with_origin(4.0, 0.0, 4.0);
    
    // Base
    builder.fill_box(0, 0, 0, 7, 2, 7, colors::DARK_GRAY);
    
    // Middle section
    builder.fill_box(1, 3, 1, 6, 10, 6, colors::BROWN);
    
    // Top (sphere)
    builder.fill_sphere(4, 13, 4, 3, colors::GREEN);
    
    // Add some neon accents
    builder.add_emissive(0, 0, 0, colors::NEON_CYAN, 0.2, 0.9, 1.0, 3.0);
    builder.add_emissive(7, 0, 0, colors::NEON_CYAN, 0.2, 0.9, 1.0, 3.0);
    builder.add_emissive(0, 0, 7, colors::NEON_CYAN, 0.2, 0.9, 1.0, 3.0);
    builder.add_emissive(7, 0, 7, colors::NEON_CYAN, 0.2, 0.9, 1.0, 3.0);
    
    let custom_model = builder.build();
    println!("Custom Tower Model:");
    println!("  - Name: {}", custom_model.name);
    println!("  - Bounds: {}x{}x{}", custom_model.bounds.width, custom_model.bounds.height, custom_model.bounds.depth);
    println!("  - Total voxels: {}", custom_model.voxel_count());
    println!();
    
    // === PART 3: VOX LOADER ===
    println!("┌─────────────────────────────────────────────┐");
    println!("│  STRATEGY 2: VOX LOADER (The Future)         │");
    println!("└─────────────────────────────────────────────┘");
    println!();
    
    // Create asset loader pointing to models directory
    let loader = ModelAssetLoader::new("assets/models/vox");
    
    // List available VOX files
    let vox_files = loader.list_vox_files();
    if vox_files.is_empty() {
        println!("No .vox files found in assets/models/vox/");
        println!("→ Drop MagicaVoxel files there for automatic loading!");
        println!();
        println!("To create .vox files:");
        println!("  1. Download MagicaVoxel from https://ephtracy.github.io/");
        println!("  2. Create your model");
        println!("  3. Export as .vox");
        println!("  4. Drop into assets/models/vox/");
    } else {
        println!("Found .vox files:");
        for file in &vox_files {
            println!("  • {}.vox", file);
        }
        
        // Try to load first file
        if let Some(first) = vox_files.first() {
            match loader.load_vox(first) {
                Ok(vox_file) => {
                    println!();
                    println!("Loaded '{}' successfully!", first);
                    println!("  - Size: {}x{}x{}", vox_file.size_x, vox_file.size_y, vox_file.size_z);
                    println!("  - Voxels: {}", vox_file.voxel_count());
                    
                    // Convert to engine format
                    let model = vox_file.to_voxel_model();
                    println!("  - Converted to VoxelModel with {} voxels", model.voxel_count());
                }
                Err(e) => {
                    println!("Error loading '{}': {}", first, e);
                }
            }
        }
    }
    println!();
    
    // === SUMMARY ===
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                    ASSET PIPELINE READY                    ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║  ✓ Procedural Models: 9 built-in models                   ║");
    println!("║  ✓ Model Builder: Runtime model creation                   ║");
    println!("║  ✓ VOX Loader: MagicaVoxel format support                  ║");
    println!("║  ✓ GPU Integration: to_instances() for rendering           ║");
    println!("╚════════════════════════════════════════════════════════════╝");
}
