//! Entity synchronization system.
//!
//! Syncs ECS entity positions to GPU instance data.
//! This runs SAME TICK as logic - no frame delay.

use super::PositionView;
use crate::instancing::InstanceData;
use std::sync::atomic::{AtomicU64, Ordering};

/// Configuration for entity sync.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Maximum entities to sync per frame.
    pub max_entities: usize,
    /// Whether to use SIMD for bulk copying.
    pub use_simd: bool,
    /// Minimum entities before parallel sync kicks in.
    pub parallel_threshold: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            max_entities: 100_000,
            use_simd: true,
            parallel_threshold: 1000,
        }
    }
}

/// System for syncing ECS entities to GPU instances.
///
/// This system:
/// 1. Reads positions directly from ECS storage (zero-copy view)
/// 2. Writes to GPU instance buffer (mapped memory when possible)
/// 3. Tracks what changed for partial updates
pub struct EntitySyncSystem {
    /// Configuration.
    config: SyncConfig,
    /// Last synced generation.
    last_sync_generation: u64,
    /// Dirty entity tracking (bitfield).
    dirty_entities: Vec<u64>,
    /// Statistics.
    stats: SyncStats,
}

/// Statistics from sync operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct SyncStats {
    /// Entities synced this frame.
    pub entities_synced: u32,
    /// Bytes transferred.
    pub bytes_transferred: u64,
    /// Sync time in microseconds.
    pub sync_time_us: u32,
    /// Whether full sync was needed.
    pub was_full_sync: bool,
}

impl EntitySyncSystem {
    /// Creates a new sync system.
    #[must_use]
    pub fn new(config: SyncConfig) -> Self {
        let bitfield_size = (config.max_entities + 63) / 64;
        
        Self {
            config,
            last_sync_generation: 0,
            dirty_entities: vec![0; bitfield_size],
            stats: SyncStats::default(),
        }
    }
    
    /// Marks an entity as dirty (needs sync).
    pub fn mark_dirty(&mut self, entity_index: usize) {
        if entity_index < self.config.max_entities {
            let word = entity_index / 64;
            let bit = entity_index % 64;
            self.dirty_entities[word] |= 1 << bit;
        }
    }
    
    /// Marks all entities as dirty (full sync needed).
    pub fn mark_all_dirty(&mut self) {
        for word in &mut self.dirty_entities {
            *word = u64::MAX;
        }
    }
    
    /// Clears all dirty flags.
    fn clear_dirty(&mut self) {
        for word in &mut self.dirty_entities {
            *word = 0;
        }
    }
    
    /// Syncs entity positions to instance buffer.
    ///
    /// # Safety
    ///
    /// - `position_view` must be valid and no concurrent writes
    /// - `output` must have capacity for all entities
    ///
    /// # Returns
    ///
    /// Number of instances written.
    pub unsafe fn sync(
        &mut self,
        position_view: &mut PositionView,
        active_mask: &[u64],  // Bitfield of active entities
        output: &mut [InstanceData],
    ) -> usize {
        let start = std::time::Instant::now();
        
        let current_gen = position_view.generation();
        let need_full_sync = current_gen != self.last_sync_generation;
        
        let count = position_view.count().min(output.len());
        let synced;
        
        if need_full_sync {
            // Full sync: copy all active entities
            synced = self.full_sync(position_view, active_mask, output, count);
            self.clear_dirty();
        } else {
            // Partial sync: only dirty entities
            synced = self.partial_sync(position_view, active_mask, output, count);
        }
        
        self.last_sync_generation = current_gen;
        
        let elapsed = start.elapsed();
        self.stats = SyncStats {
            entities_synced: synced as u32,
            bytes_transferred: (synced * std::mem::size_of::<InstanceData>()) as u64,
            sync_time_us: elapsed.as_micros() as u32,
            was_full_sync: need_full_sync,
        };
        
        synced
    }
    
    /// Full sync of all active entities.
    unsafe fn full_sync(
        &self,
        position_view: &PositionView,
        active_mask: &[u64],
        output: &mut [InstanceData],
        count: usize,
    ) -> usize {
        let mut output_idx = 0;
        
        for (word_idx, &active_word) in active_mask.iter().enumerate() {
            if active_word == 0 {
                continue;
            }
            
            let base_entity = word_idx * 64;
            let mut word = active_word;
            
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                let entity_idx = base_entity + bit;
                
                if entity_idx >= count {
                    break;
                }
                
                if output_idx >= output.len() {
                    return output_idx;
                }
                
                let pos = position_view.get_unchecked(entity_idx);
                output[output_idx] = InstanceData {
                    position_scale: [pos[0], pos[1], pos[2], 1.0],
                    dimensions_normal_material: [1.0, 1.0, 0.0, 0.0],
                    emission: [0.0, 0.0, 0.0, 0.0],
                    uv_offset_scale: [0.0, 0.0, 1.0, 1.0],
                };
                
                output_idx += 1;
                word &= word - 1; // Clear lowest set bit
            }
        }
        
        output_idx
    }
    
    /// Partial sync of only dirty entities.
    unsafe fn partial_sync(
        &self,
        position_view: &PositionView,
        active_mask: &[u64],
        output: &mut [InstanceData],
        count: usize,
    ) -> usize {
        let mut synced = 0;
        
        for (word_idx, (&dirty_word, &active_word)) in 
            self.dirty_entities.iter().zip(active_mask.iter()).enumerate() 
        {
            let need_sync = dirty_word & active_word;
            if need_sync == 0 {
                continue;
            }
            
            let base_entity = word_idx * 64;
            let mut word = need_sync;
            
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                let entity_idx = base_entity + bit;
                
                if entity_idx >= count || entity_idx >= output.len() {
                    break;
                }
                
                let pos = position_view.get_unchecked(entity_idx);
                output[entity_idx].position_scale = [pos[0], pos[1], pos[2], 1.0];
                
                synced += 1;
                word &= word - 1;
            }
        }
        
        synced
    }
    
    /// Returns statistics from last sync.
    #[must_use]
    pub fn stats(&self) -> SyncStats {
        self.stats
    }
    
    /// Returns true if sync took longer than target (1ms).
    #[must_use]
    pub fn is_over_budget(&self) -> bool {
        self.stats.sync_time_us > 1000
    }
}

impl Default for EntitySyncSystem {
    fn default() -> Self {
        Self::new(SyncConfig::default())
    }
}

/// Atomic generation counter for sync coordination.
///
/// Game logic increments this after updating positions.
/// Rendering checks this to know when to resync.
pub struct SyncGeneration {
    counter: AtomicU64,
}

impl SyncGeneration {
    /// Creates a new generation counter.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }
    
    /// Increments the generation (call after logic update).
    pub fn increment(&self) {
        self.counter.fetch_add(1, Ordering::Release);
    }
    
    /// Returns the current generation.
    #[must_use]
    pub fn current(&self) -> u64 {
        self.counter.load(Ordering::Acquire)
    }
    
    /// Returns a raw pointer for use with PositionView.
    #[must_use]
    pub fn as_ptr(&self) -> *const AtomicU64 {
        &self.counter
    }
}

impl Default for SyncGeneration {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sync_system() {
        let mut system = EntitySyncSystem::new(SyncConfig {
            max_entities: 100,
            ..Default::default()
        });
        
        // Simulate 3 active entities
        let positions: Vec<f32> = vec![
            1.0, 2.0, 3.0, 0.0,
            4.0, 5.0, 6.0, 0.0,
            7.0, 8.0, 9.0, 0.0,
        ];
        
        let gen = SyncGeneration::new();
        gen.increment();
        
        let view = unsafe {
            super::super::PositionView::new(
                positions.as_ptr(),
                3,
                4,
                gen.as_ptr(),
            )
        };
        
        let mut view = view;
        let active_mask = [0b111u64]; // Entities 0, 1, 2 active
        let mut output = vec![InstanceData::default(); 10];
        
        let synced = unsafe {
            system.sync(&mut view, &active_mask, &mut output)
        };
        
        assert_eq!(synced, 3);
        assert_eq!(output[0].position_scale, [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(output[1].position_scale, [4.0, 5.0, 6.0, 1.0]);
        assert_eq!(output[2].position_scale, [7.0, 8.0, 9.0, 1.0]);
    }
}
