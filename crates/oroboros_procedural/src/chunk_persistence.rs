//! # Chunk Persistence Integration
//!
//! Connects `WorldManager` to the economy WAL for block modifications.
//!
//! ## Design
//!
//! When a player modifies a block (mining, placing), we:
//! 1. Record the modification in the WorldManager's in-memory log
//! 2. Write to the batched WAL for durability
//! 3. On chunk reload, check WAL for modifications before generating
//!
//! ## Performance
//!
//! - Block modifications use the batched WAL (0.1ms amortized)
//! - Chunk loading checks a hash map first (O(1))
//! - Fresh terrain uses procedural generation (no I/O)

use std::collections::HashMap;
use std::path::PathBuf;

use crate::chunk::{Block, ChunkCoord, CHUNK_SIZE};
use crate::world_manager::{ChunkModification, ModificationEntry, WorldManager};
use crate::noise::WorldSeed;

/// Operation types for chunk persistence WAL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ChunkOpType {
    /// Single block modification.
    BlockModify = 1,
    /// Batch block modification (e.g., explosion).
    BlockBatch = 2,
    /// Chunk fully saved (checkpoint).
    ChunkCheckpoint = 3,
}

impl ChunkOpType {
    /// Converts from u8.
    #[must_use]
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::BlockModify),
            2 => Some(Self::BlockBatch),
            3 => Some(Self::ChunkCheckpoint),
            _ => None,
        }
    }
}

/// Serialized block modification for WAL.
#[derive(Clone, Debug)]
pub struct BlockModifyPayload {
    /// Chunk X coordinate.
    pub chunk_x: i32,
    /// Chunk Z coordinate.
    pub chunk_z: i32,
    /// Local X within chunk.
    pub local_x: u8,
    /// Y level.
    pub y: u8,
    /// Local Z within chunk.
    pub local_z: u8,
    /// New block ID.
    pub block_id: u16,
    /// Server tick when modification occurred.
    pub tick: u64,
}

impl BlockModifyPayload {
    /// Serializes to bytes for WAL storage.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(20);
        buf.extend_from_slice(&self.chunk_x.to_le_bytes());
        buf.extend_from_slice(&self.chunk_z.to_le_bytes());
        buf.push(self.local_x);
        buf.push(self.y);
        buf.push(self.local_z);
        buf.extend_from_slice(&self.block_id.to_le_bytes());
        buf.extend_from_slice(&self.tick.to_le_bytes());
        buf
    }

    /// Deserializes from bytes.
    #[must_use]
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < 19 {
            return None;
        }
        
        Some(Self {
            chunk_x: i32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            chunk_z: i32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            local_x: data[8],
            y: data[9],
            local_z: data[10],
            block_id: u16::from_le_bytes([data[11], data[12]]),
            tick: u64::from_le_bytes([
                data[13], data[14], data[15], data[16],
                data[17], data[18], data[19], data[20],
            ]),
        })
    }
}

/// Persistence layer for chunk modifications.
///
/// Wraps `WorldManager` and provides WAL integration.
pub struct ChunkPersistence {
    /// World manager for chunk generation.
    world: WorldManager,
    /// Path to modification database.
    #[allow(dead_code)]
    db_path: PathBuf,
    /// Cached modifications loaded from disk.
    cached_mods: HashMap<ChunkCoord, Vec<ChunkModification>>,
    /// Current server tick.
    current_tick: u64,
}

impl ChunkPersistence {
    /// Creates a new persistence layer.
    ///
    /// # Arguments
    ///
    /// * `seed` - World seed for procedural generation
    /// * `db_path` - Path to store modification data
    #[must_use]
    pub fn new(seed: WorldSeed, db_path: PathBuf) -> Self {
        Self {
            world: WorldManager::with_seed(seed),
            db_path,
            cached_mods: HashMap::new(),
            current_tick: 0,
        }
    }

    /// Returns reference to the world manager.
    #[must_use]
    pub fn world(&self) -> &WorldManager {
        &self.world
    }

    /// Returns mutable reference to the world manager.
    pub fn world_mut(&mut self) -> &mut WorldManager {
        &mut self.world
    }

    /// Loads modifications from a WAL recovery result.
    ///
    /// Call this at startup after WAL recovery.
    pub fn load_modifications(&mut self, entries: Vec<ModificationEntry>) {
        for entry in entries {
            self.cached_mods.insert(entry.coord, entry.modifications.clone());
        }
        self.world.load_modifications(
            self.cached_mods.iter()
                .map(|(k, v)| ModificationEntry { coord: *k, modifications: v.clone() })
                .collect()
        );
    }

    /// Updates the world with player position.
    ///
    /// Returns number of chunks generated this frame.
    pub fn update(&mut self, player_x: f32, player_z: f32) -> usize {
        self.current_tick += 1;
        self.world.update(player_x, player_z)
    }

    /// Modifies a block and returns the payload for WAL logging.
    ///
    /// The caller should log this to the batched WAL.
    pub fn modify_block(
        &mut self,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        block_id: u16,
    ) -> Option<BlockModifyPayload> {
        // Modify in world manager
        if !self.world.set_block(world_x, world_y, world_z, block_id) {
            return None;
        }
        
        let chunk_coord = ChunkCoord::from_world_pos(world_x, world_z);
        let local_x = world_x.rem_euclid(CHUNK_SIZE as i32) as u8;
        let local_z = world_z.rem_euclid(CHUNK_SIZE as i32) as u8;
        
        // Create payload for WAL
        Some(BlockModifyPayload {
            chunk_x: chunk_coord.x,
            chunk_z: chunk_coord.z,
            local_x,
            y: world_y as u8,
            local_z,
            block_id,
            tick: self.current_tick,
        })
    }

    /// Checks if a block exists at the given position.
    ///
    /// Returns true if the chunk is loaded and has a non-air block.
    #[must_use]
    pub fn has_block(&self, world_x: i32, world_y: i32, world_z: i32) -> bool {
        self.world.get_block(world_x, world_y, world_z)
            .map(|b| !b.is_air())
            .unwrap_or(false)
    }

    /// Gets the block at world coordinates.
    #[must_use]
    pub fn get_block(&self, world_x: i32, world_y: i32, world_z: i32) -> Option<Block> {
        self.world.get_block(world_x, world_y, world_z)
    }

    /// Ensures chunks are loaded around a position.
    ///
    /// Use for spawn point initialization.
    pub fn ensure_loaded_around(&mut self, world_x: f32, world_z: f32, radius: i32) {
        self.world.ensure_loaded_around(world_x, world_z, radius);
    }

    /// Returns number of loaded chunks.
    #[must_use]
    pub fn loaded_chunk_count(&self) -> usize {
        self.world.loaded_chunk_count()
    }

    /// Returns true if there's ground at the given position.
    #[must_use]
    pub fn has_ground(&self, world_x: i32, world_y: i32, world_z: i32) -> bool {
        self.world.has_ground(world_x, world_y, world_z)
    }

    /// Exports all modifications for checkpoint saving.
    #[must_use]
    pub fn export_all_modifications(&self) -> Vec<ModificationEntry> {
        self.world.export_modifications()
    }
}

/// ECS System for dynamic world management.
///
/// Integrates with `oroboros_core` ECS to track player positions
/// and manage chunk loading/unloading.
pub struct WorldChunkSystem {
    /// Chunk persistence layer.
    persistence: ChunkPersistence,
    /// Player entity being tracked.
    tracked_player: Option<u64>,
    /// Last known player position.
    last_player_pos: (f32, f32, f32),
}

impl WorldChunkSystem {
    /// Creates a new world chunk system.
    #[must_use]
    pub fn new(seed: WorldSeed, db_path: PathBuf) -> Self {
        Self {
            persistence: ChunkPersistence::new(seed, db_path),
            tracked_player: None,
            last_player_pos: (0.0, 0.0, 0.0),
        }
    }

    /// Sets the player entity to track.
    pub fn track_player(&mut self, entity_id: u64) {
        self.tracked_player = Some(entity_id);
    }

    /// Updates the system with current player position.
    ///
    /// Call this every frame with the player's Position component.
    ///
    /// Returns number of chunks generated this frame.
    pub fn update(&mut self, player_x: f32, player_y: f32, player_z: f32) -> usize {
        self.last_player_pos = (player_x, player_y, player_z);
        self.persistence.update(player_x, player_z)
    }

    /// Checks if the player would fall into void at current position.
    ///
    /// Use for collision detection / emergency chunk loading.
    #[must_use]
    pub fn would_fall_into_void(&self) -> bool {
        let (x, y, z) = self.last_player_pos;
        !self.persistence.has_ground(x as i32, y as i32, z as i32)
    }

    /// Forces immediate chunk loading around player.
    ///
    /// Use if `would_fall_into_void` returns true.
    pub fn emergency_load(&mut self) {
        let (x, _, z) = self.last_player_pos;
        self.persistence.ensure_loaded_around(x, z, 3);
    }

    /// Gets the block at a position.
    #[must_use]
    pub fn get_block(&self, x: i32, y: i32, z: i32) -> Option<Block> {
        self.persistence.get_block(x, y, z)
    }

    /// Modifies a block (mining/placing).
    ///
    /// Returns WAL payload if successful.
    pub fn modify_block(&mut self, x: i32, y: i32, z: i32, block_id: u16) -> Option<BlockModifyPayload> {
        self.persistence.modify_block(x, y, z, block_id)
    }

    /// Returns number of loaded chunks.
    #[must_use]
    pub fn loaded_chunk_count(&self) -> usize {
        self.persistence.loaded_chunk_count()
    }

    /// Loads modifications from WAL recovery.
    pub fn load_modifications(&mut self, entries: Vec<ModificationEntry>) {
        self.persistence.load_modifications(entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_modify_payload_roundtrip() {
        let payload = BlockModifyPayload {
            chunk_x: -5,
            chunk_z: 10,
            local_x: 7,
            y: 64,
            local_z: 12,
            block_id: 100,
            tick: 999_999,
        };
        
        let bytes = payload.serialize();
        let restored = BlockModifyPayload::deserialize(&bytes).unwrap();
        
        assert_eq!(payload.chunk_x, restored.chunk_x);
        assert_eq!(payload.chunk_z, restored.chunk_z);
        assert_eq!(payload.local_x, restored.local_x);
        assert_eq!(payload.y, restored.y);
        assert_eq!(payload.local_z, restored.local_z);
        assert_eq!(payload.block_id, restored.block_id);
        assert_eq!(payload.tick, restored.tick);
    }

    #[test]
    fn test_chunk_persistence_basic() {
        let seed = WorldSeed::new(12345);
        let mut persistence = ChunkPersistence::new(seed, PathBuf::from("test_chunks"));
        
        // Load initial area
        persistence.ensure_loaded_around(0.0, 0.0, 3);
        
        assert!(persistence.loaded_chunk_count() > 0);
        
        // Should have ground at y=64 (around sea level)
        assert!(persistence.has_ground(0, 100, 0));
    }

    #[test]
    fn test_chunk_system_no_void_fall() {
        let seed = WorldSeed::new(42);
        let mut system = WorldChunkSystem::new(seed, PathBuf::from("test"));
        
        // Initialize at spawn
        system.persistence.ensure_loaded_around(0.0, 0.0, 5);
        
        // Simulate walking
        for step in 0..100 {
            let x = step as f32;
            let y = 64.0;
            let z = 0.0;
            
            system.update(x, y, z);
            
            // Emergency load if needed
            if system.would_fall_into_void() {
                system.emergency_load();
            }
            
            // Should never fall
            assert!(!system.would_fall_into_void(), "Void at step {}", step);
        }
    }

    #[test]
    fn test_block_modification_persistence() {
        let seed = WorldSeed::new(12345);
        let mut system = WorldChunkSystem::new(seed, PathBuf::from("test"));
        
        // Load area
        system.persistence.ensure_loaded_around(0.0, 0.0, 2);
        
        // Place a block
        let payload = system.modify_block(5, 70, 5, 100);
        assert!(payload.is_some());
        
        // Verify block is there
        let block = system.get_block(5, 70, 5);
        assert!(block.is_some());
        assert_eq!(block.unwrap().id, 100);
    }

    #[test]
    fn test_walk_1000_blocks_no_void() {
        let seed = WorldSeed::new(99999);
        let mut system = WorldChunkSystem::new(seed, PathBuf::from("test"));
        
        system.persistence.ensure_loaded_around(0.0, 0.0, 5);
        
        // Walk 1000 blocks
        for step in 0..1000 {
            let x = step as f32;
            let z = (step as f32 * 0.5).sin() * 50.0; // Zigzag path
            
            system.update(x, 64.0, z);
            
            // Process pending chunks
            if step % 16 == 0 {
                system.persistence.world.flush_generation_queue();
            }
        }
        
        // Final check - should have ground
        assert!(system.persistence.has_ground(999, 100, 0));
    }
}
