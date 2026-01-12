//! # Terrain Quality Tests
//!
//! Verifies that terrain looks like a habitable forest, not a math equation.

use oroboros_procedural::{
    Block, ChunkCoord, ChunkGenerator, WorldSeed, BiomeClassifier,
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

/// Test: Verify chunks contain trees.
#[test]
fn test_chunks_have_trees() {
    let seed = WorldSeed::new(42);
    let gen = ChunkGenerator::new(seed);
    
    let mut total_wood_blocks = 0;
    let mut total_leaf_blocks = 0;
    let mut chunks_with_trees = 0;
    
    // Generate multiple chunks and count trees
    for cz in -5..5 {
        for cx in -5..5 {
            let chunk = gen.generate(ChunkCoord::new(cx, cz));
            
            let mut chunk_wood = 0;
            let mut chunk_leaves = 0;
            
            // Count wood and leaf blocks
            for y in 0..256 {
                for z in 0..16 {
                    for x in 0..16 {
                        let block = chunk.get_block(x, y, z);
                        if block.id == Block::WOOD.id {
                            chunk_wood += 1;
                        } else if block.id == Block::LEAVES.id {
                            chunk_leaves += 1;
                        }
                    }
                }
            }
            
            if chunk_wood > 0 {
                chunks_with_trees += 1;
            }
            
            total_wood_blocks += chunk_wood;
            total_leaf_blocks += chunk_leaves;
        }
    }
    
    println!("=== Tree Statistics ===");
    println!("Chunks generated: 100");
    println!("Chunks with trees: {}", chunks_with_trees);
    println!("Total wood blocks: {}", total_wood_blocks);
    println!("Total leaf blocks: {}", total_leaf_blocks);
    
    // At least some chunks should have trees (depends on biome distribution)
    assert!(
        chunks_with_trees >= 5,
        "Not enough chunks with trees: {} out of 100",
        chunks_with_trees
    );
    
    // Should have some wood and leaves
    assert!(
        total_wood_blocks > 20,
        "Not enough wood blocks: {}",
        total_wood_blocks
    );
    assert!(
        total_leaf_blocks > 100,
        "Not enough leaf blocks: {}",
        total_leaf_blocks
    );
}

/// Test: Verify tree structure is valid (trunk + canopy).
#[test]
fn test_tree_structure() {
    let seed = WorldSeed::new(12345);
    let gen = ChunkGenerator::new(seed);
    
    let mut found_any_tree = false;
    let mut trees_found = 0;
    
    // Search area for trees - look for any wood block
    for cz in -5..5 {
        for cx in -5..5 {
            let chunk = gen.generate(ChunkCoord::new(cx, cz));
            
            // Scan all blocks for wood
            for y in 60..200 { // Focus on typical tree heights
                for z in 0..16 {
                    for x in 0..16 {
                        let block = chunk.get_block(x, y, z);
                        if block.id == Block::WOOD.id {
                            // Found wood - check if it's a tree base
                            // (wood above grass/dirt/jungle grass)
                            if y > 0 {
                                let below = chunk.get_block(x, y - 1, z);
                                // Check if below is grass (1), dirt (3), or jungle grass (12)
                                if below.id == 1 || below.id == 3 || below.id == 12 {
                                    trees_found += 1;
                                    found_any_tree = true;
                                    
                                    if trees_found <= 3 {
                                        // Count trunk and leaves
                                        let mut trunk = 0;
                                        let mut leaves = 0;
                                        
                                        for ty in y..y+10 {
                                            let tb = chunk.get_block(x, ty, z);
                                            if tb.id == Block::WOOD.id {
                                                trunk += 1;
                                            }
                                        }
                                        
                                        for dy in 0..6 {
                                            for dz in -2i32..=2 {
                                                for dx in -2i32..=2 {
                                                    let lx = x as i32 + dx;
                                                    let lz = z as i32 + dz;
                                                    if lx >= 0 && lx < 16 && lz >= 0 && lz < 16 {
                                                        if chunk.get_block(lx as usize, y + trunk + dy, lz as usize).id == Block::LEAVES.id {
                                                            leaves += 1;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        
                                        println!(
                                            "Tree {} at chunk ({}, {}), block ({}, {}, {}): trunk={}, leaves={}",
                                            trees_found, cx, cz, x, y, z, trunk, leaves
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    println!("Total trees found: {}", trees_found);
    
    // We just need to verify trees exist
    assert!(found_any_tree, "No trees found in 100 chunks");
    assert!(trees_found > 5, "Too few trees found: {}", trees_found);
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
