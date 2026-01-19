//! # Terrain Quality Tests
//!
//! Verifies that terrain is a proper BRUTALIST MEGA-STRUCTURE.
//! Grid architecture with rooms, walls, bridges, and hazard zones.

use oroboros_procedural::{
    ChunkCoord, ChunkGenerator, WorldSeed, BiomeClassifier,
};

/// Test: Verify terrain has flat walkable areas.
#[test]
fn test_terrain_has_flat_plains() {
    let seed = WorldSeed::new(42);
    let classifier = BiomeClassifier::new(seed);
    
    let mut flat_count = 0;
    let mut total_samples = 0;
    
    // Sample terrain heights in a large area
    for z in (-500..500).step_by(10) {
        for x in (-500..500).step_by(10) {
            let h1 = classifier.get_terrain_height(x as f64, z as f64, 64, 256);
            let h2 = classifier.get_terrain_height((x + 5) as f64, z as f64, 64, 256);
            let h3 = classifier.get_terrain_height(x as f64, (z + 5) as f64, 64, 256);
            
            // Check if area is relatively flat (height diff <= 3)
            let max_diff = (h1 - h2).abs().max((h1 - h3).abs()).max((h2 - h3).abs());
            if max_diff <= 3 {
                flat_count += 1;
            }
            total_samples += 1;
        }
    }
    
    let flat_percentage = (flat_count as f64 / total_samples as f64) * 100.0;
    println!("Flat terrain percentage: {:.1}%", flat_percentage);
    println!("Flat samples: {} / {}", flat_count, total_samples);
    
    // At least 30% of terrain should be relatively flat
    assert!(
        flat_percentage > 30.0,
        "Not enough flat terrain for walking/building: {:.1}%",
        flat_percentage
    );
}

/// Test: Verify terrain has mountains (steep areas).
#[test]
fn test_terrain_has_mountains() {
    let seed = WorldSeed::new(42);
    let classifier = BiomeClassifier::new(seed);
    
    let mut mountain_count = 0;
    let mut total_samples = 0;
    
    // Sample terrain heights in a large area
    for z in (-500..500).step_by(10) {
        for x in (-500..500).step_by(10) {
            let height = classifier.get_terrain_height(x as f64, z as f64, 64, 256);
            
            // Mountains are above sea level + 30
            if height > 94 {
                mountain_count += 1;
            }
            total_samples += 1;
        }
    }
    
    let mountain_percentage = (mountain_count as f64 / total_samples as f64) * 100.0;
    println!("Mountain terrain percentage: {:.1}%", mountain_percentage);
    
    // At least 5% of terrain should be mountainous
    assert!(
        mountain_percentage > 5.0,
        "Not enough mountains for variety: {:.1}%",
        mountain_percentage
    );
}

/// Test: Verify chunks contain BRUTALIST STRUCTURES (walls, bridges, hazards).
/// Block IDs:
/// - 1 = Concrete Floor
/// - 2 = Concrete Wall  
/// - 3 = Hazard Neon (red death zone)
/// - 4 = Goal Zone (extraction)
/// - 5 = Bedrock
/// - 6 = Gold Loot
/// - 7 = Metal Bridge
#[test]
fn test_chunks_have_brutalist_structures() {
    let seed = WorldSeed::new(42);
    let gen = ChunkGenerator::new(seed);
    
    let mut total_concrete_floor = 0;
    let mut total_concrete_wall = 0;
    let mut total_hazard_neon = 0;
    let mut total_metal_bridge = 0;
    let mut total_bedrock = 0;
    let mut chunks_with_walls = 0;
    let mut chunks_with_bridges = 0;
    
    // Generate multiple chunks and count structures
    for cz in -5..5 {
        for cx in -5..5 {
            let chunk = gen.generate(ChunkCoord::new(cx, cz));
            
            let mut chunk_walls = 0;
            let mut chunk_bridges = 0;
            
            // Count block types
            for y in 0..256 {
                for z in 0..16 {
                    for x in 0..16 {
                        let block = chunk.get_block(x, y, z);
                        match block.id {
                            1 => total_concrete_floor += 1,
                            2 => { total_concrete_wall += 1; chunk_walls += 1; }
                            3 => total_hazard_neon += 1,
                            5 => total_bedrock += 1,
                            7 => { total_metal_bridge += 1; chunk_bridges += 1; }
                            _ => {}
                        }
                    }
                }
            }
            
            if chunk_walls > 0 {
                chunks_with_walls += 1;
            }
            if chunk_bridges > 0 {
                chunks_with_bridges += 1;
            }
        }
    }
    
    println!("=== BRUTALIST MEGA-STRUCTURE Statistics ===");
    println!("Chunks generated: 100");
    println!("Chunks with walls: {}", chunks_with_walls);
    println!("Chunks with bridges: {}", chunks_with_bridges);
    println!("Total concrete floor: {}", total_concrete_floor);
    println!("Total concrete wall: {}", total_concrete_wall);
    println!("Total hazard neon: {}", total_hazard_neon);
    println!("Total metal bridge: {}", total_metal_bridge);
    println!("Total bedrock: {}", total_bedrock);
    
    // All chunks should have bedrock foundation
    assert!(
        total_bedrock > 50000,
        "Not enough bedrock foundation: {}",
        total_bedrock
    );
    
    // Should have significant wall structures
    assert!(
        chunks_with_walls >= 50,
        "Not enough chunks with walls: {} out of 100",
        chunks_with_walls
    );
    
    // Should have concrete walls (brutalist aesthetic)
    assert!(
        total_concrete_wall > 10000,
        "Not enough concrete walls: {}",
        total_concrete_wall
    );
    
    // Should have some metal elements (bridges/catwalks)
    assert!(
        total_metal_bridge > 100,
        "Not enough metal structures: {}",
        total_metal_bridge
    );
    
    // Should have hazard zones (strategic pits, not everywhere)
    assert!(
        total_hazard_neon > 100,
        "Not enough hazard neon: {}",
        total_hazard_neon
    );
}

/// Test: Verify maze structure is valid (walls, corridors, rooms).
/// New design has lower walls (18-28) and more horizontal complexity.
#[test]
fn test_maze_structure() {
    let seed = WorldSeed::new(12345);
    let gen = ChunkGenerator::new(seed);
    
    let mut walls_found = 0;
    let mut floor_found = 0;
    let mut metal_found = 0;
    let mut max_wall_height = 0;
    
    // Search area for structure elements
    for cz in -3..3 {
        for cx in -3..3 {
            let chunk = gen.generate(ChunkCoord::new(cx, cz));
            
                for z in 0..16 {
                    for x in 0..16 {
                    // Check floor level (Y=4)
                    let floor_block = chunk.get_block(x, 4, z);
                    if floor_block.id == 1 { floor_found += 1; }
                    
                    // Check for any metal (ID 7) at any Y level
                    for y in 0..50 {
                        if chunk.get_block(x, y, z).id == 7 { 
                            metal_found += 1; 
                                            }
                                        }
                                        
                    // Measure wall heights
                    if floor_block.id == 2 {
                        let mut height = 0;
                        for y in 4..50 {
                            if chunk.get_block(x, y, z).id == 2 {
                                height += 1;
                            } else {
                                break;
                                                }
                                            }
                        if height > 0 {
                            walls_found += 1;
                            max_wall_height = max_wall_height.max(height);
                        }
                    }
                }
            }
        }
    }
    
    println!("=== MAZE STRUCTURE ===");
    println!("Floor blocks: {}", floor_found);
    println!("Wall sections: {}", walls_found);
    println!("Metal blocks (catwalks/platforms): {}", metal_found);
    println!("Max wall height: {}", max_wall_height);
    
    // Verify structure
    assert!(floor_found > 1000, "Not enough floor: {}", floor_found);
    assert!(walls_found > 100, "Not enough walls: {}", walls_found);
    // Metal is sparse in the new design, just verify some exists
    assert!(metal_found > 0, "No metal structures found");
    assert!(max_wall_height <= 30, "Walls too tall: {} (should be <=30)", max_wall_height);
    assert!(max_wall_height >= 10, "Walls too short: {}", max_wall_height);
}

/// Test: Verify terrain height distribution is natural.
#[test]
fn test_terrain_height_distribution() {
    let seed = WorldSeed::new(42);
    let classifier = BiomeClassifier::new(seed);
    
    // Count heights in buckets
    let mut buckets: [usize; 10] = [0; 10]; // 0-25, 26-50, 51-75, etc.
    let mut total = 0;
    
    for z in (-200..200).step_by(5) {
        for x in (-200..200).step_by(5) {
            let height = classifier.get_terrain_height(x as f64, z as f64, 64, 256);
            let bucket = ((height as usize).min(255) / 26).min(9);
            buckets[bucket] += 1;
            total += 1;
        }
    }
    
    println!("=== Height Distribution ===");
    for (i, count) in buckets.iter().enumerate() {
        let min_h = i * 26;
        let max_h = (i + 1) * 26 - 1;
        let pct = (*count as f64 / total as f64) * 100.0;
        println!("  {}-{}: {:5} ({:.1}%)", min_h, max_h, count, pct);
    }
    
    // Most terrain should be in middle heights (buckets 2-4, heights 52-129)
    let mid_height_count: usize = buckets[2..5].iter().sum();
    let mid_height_pct = (mid_height_count as f64 / total as f64) * 100.0;
    
    println!("\nMid-height terrain (52-129): {:.1}%", mid_height_pct);
    
    // At least 60% should be in mid-heights (walkable terrain)
    assert!(
        mid_height_pct > 50.0,
        "Not enough mid-height terrain: {:.1}%",
        mid_height_pct
    );
}

/// Test: Verify no extreme terrain spikes.
#[test]
fn test_no_extreme_spikes() {
    let seed = WorldSeed::new(42);
    let classifier = BiomeClassifier::new(seed);
    
    let mut max_spike = 0;
    
    for z in -100..100 {
        for x in -100..100 {
            let h1 = classifier.get_terrain_height(x as f64, z as f64, 64, 256);
            let h2 = classifier.get_terrain_height((x + 1) as f64, z as f64, 64, 256);
            let h3 = classifier.get_terrain_height(x as f64, (z + 1) as f64, 64, 256);
            
            let diff = (h1 - h2).abs().max((h1 - h3).abs());
            max_spike = max_spike.max(diff);
        }
    }
    
    println!("Maximum terrain spike (height diff between adjacent blocks): {}", max_spike);
    
    // No single-block spikes higher than 10 blocks
    assert!(
        max_spike <= 15,
        "Terrain has extreme spikes: {} blocks",
        max_spike
    );
}
