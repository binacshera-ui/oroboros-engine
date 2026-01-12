//! Voxel world management.
//!
//! Manages multiple chunks and provides efficient access patterns
//! for rendering and physics.

use parking_lot::RwLock;
use std::collections::HashMap;
use super::chunk::{VoxelChunk, ChunkCoord, Voxel};

/// Maximum chunks that can be loaded at once.
/// This determines pre-allocated hash map capacity.
const MAX_LOADED_CHUNKS: usize = 4096;

/// Voxel world containing multiple chunks.
///
/// Thread-safe for concurrent read access from rendering thread
/// while game logic writes updates.
pub struct VoxelWorld {
    /// Chunks indexed by coordinate.
    chunks: RwLock<HashMap<ChunkCoord, VoxelChunk>>,
    
    /// List of dirty chunks that need re-meshing.
    dirty_chunks: RwLock<Vec<ChunkCoord>>,
}

impl VoxelWorld {
    /// Creates a new empty voxel world.
    ///
    /// Pre-allocates storage for efficient chunk management.
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunks: RwLock::new(HashMap::with_capacity(MAX_LOADED_CHUNKS)),
            dirty_chunks: RwLock::new(Vec::with_capacity(256)),
        }
    }
    
    /// Gets a chunk by coordinate, returning None if not loaded.
    #[must_use]
    pub fn get_chunk(&self, coord: ChunkCoord) -> Option<VoxelChunk> {
        // Note: This clones the chunk. For read-only access, use with_chunk instead.
        self.chunks.read().get(&coord).cloned()
    }
    
    /// Executes a closure with read access to a chunk.
    ///
    /// More efficient than `get_chunk` when you don't need ownership.
    pub fn with_chunk<F, R>(&self, coord: ChunkCoord, f: F) -> Option<R>
    where
        F: FnOnce(&VoxelChunk) -> R,
    {
        self.chunks.read().get(&coord).map(f)
    }
    
    /// Loads or creates a chunk at the given coordinate.
    ///
    /// Returns true if the chunk was newly created.
    pub fn load_chunk(&self, coord: ChunkCoord) -> bool {
        let mut chunks = self.chunks.write();
        if chunks.contains_key(&coord) {
            return false;
        }
        
        let chunk = VoxelChunk::new(coord);
        chunks.insert(coord, chunk);
        self.dirty_chunks.write().push(coord);
        true
    }
    
    /// Unloads a chunk, returning it if it existed.
    pub fn unload_chunk(&self, coord: ChunkCoord) -> Option<VoxelChunk> {
        self.chunks.write().remove(&coord)
    }
    
    /// Sets a voxel at world coordinates.
    ///
    /// Automatically marks the containing chunk as dirty.
    pub fn set_voxel(&self, world_x: i32, world_y: i32, world_z: i32, voxel: Voxel) {
        let chunk_coord = ChunkCoord::from_world_pos(world_x, world_y, world_z);
        
        let local_x = world_x.rem_euclid(super::chunk::CHUNK_SIZE as i32) as usize;
        let local_y = world_y.rem_euclid(super::chunk::CHUNK_SIZE as i32) as usize;
        let local_z = world_z.rem_euclid(super::chunk::CHUNK_SIZE as i32) as usize;
        
        let mut chunks = self.chunks.write();
        if let Some(chunk) = chunks.get_mut(&chunk_coord) {
            chunk.set(local_x, local_y, local_z, voxel);
            
            // Add to dirty list if not already there
            let mut dirty = self.dirty_chunks.write();
            if !dirty.contains(&chunk_coord) {
                dirty.push(chunk_coord);
            }
        }
    }
    
    /// Gets a voxel at world coordinates.
    #[must_use]
    pub fn get_voxel(&self, world_x: i32, world_y: i32, world_z: i32) -> Voxel {
        let chunk_coord = ChunkCoord::from_world_pos(world_x, world_y, world_z);
        
        let local_x = world_x.rem_euclid(super::chunk::CHUNK_SIZE as i32) as usize;
        let local_y = world_y.rem_euclid(super::chunk::CHUNK_SIZE as i32) as usize;
        let local_z = world_z.rem_euclid(super::chunk::CHUNK_SIZE as i32) as usize;
        
        self.chunks
            .read()
            .get(&chunk_coord)
            .map(|c| c.get(local_x, local_y, local_z))
            .unwrap_or(Voxel::AIR)
    }
    
    /// Returns and clears the list of dirty chunks.
    ///
    /// Call this from the render thread to get chunks that need re-meshing.
    pub fn take_dirty_chunks(&self) -> Vec<ChunkCoord> {
        std::mem::take(&mut *self.dirty_chunks.write())
    }
    
    /// Returns the number of loaded chunks.
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.chunks.read().len()
    }
    
    /// Returns an iterator over all loaded chunk coordinates.
    pub fn chunk_coords(&self) -> Vec<ChunkCoord> {
        self.chunks.read().keys().copied().collect()
    }
}

impl Default for VoxelWorld {
    fn default() -> Self {
        Self::new()
    }
}

// VoxelChunk needs Clone for the get_chunk method
impl Clone for VoxelChunk {
    fn clone(&self) -> Self {
        Self::clone_from_ref(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_world_operations() {
        let world = VoxelWorld::new();
        
        // Load a chunk
        assert!(world.load_chunk(ChunkCoord::new(0, 0, 0)));
        assert!(!world.load_chunk(ChunkCoord::new(0, 0, 0))); // Already loaded
        
        // Set and get voxels
        world.set_voxel(5, 10, 15, Voxel::new(42));
        assert_eq!(world.get_voxel(5, 10, 15).material_id(), 42);
        
        // Dirty chunks
        let dirty = world.take_dirty_chunks();
        assert!(!dirty.is_empty());
        
        // After taking, dirty list should be empty
        let dirty = world.take_dirty_chunks();
        assert!(dirty.is_empty());
    }
    
    #[test]
    fn test_negative_coordinates() {
        let world = VoxelWorld::new();
        
        world.load_chunk(ChunkCoord::new(-1, 0, 0));
        world.set_voxel(-5, 0, 0, Voxel::new(1));
        
        assert_eq!(world.get_voxel(-5, 0, 0).material_id(), 1);
    }
}
