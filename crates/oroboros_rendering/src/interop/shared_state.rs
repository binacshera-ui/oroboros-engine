//! Shared world state for lock-free ECS-Rendering communication.
//!
//! This module provides a triple-buffered world state that allows:
//! - Game logic to write without blocking
//! - Rendering to read without blocking
//! - No data races, no frame lag

use std::sync::atomic::{AtomicUsize, Ordering};
use parking_lot::RwLock;

/// Snapshot of world state for rendering.
///
/// Contains everything rendering needs to know about the world
/// at a specific moment in time.
#[derive(Clone, Default)]
pub struct WorldStateSnapshot {
    /// Frame number this snapshot was taken.
    pub frame: u64,
    /// Number of active entities.
    pub entity_count: u32,
    /// Camera position.
    pub camera_pos: [f32; 3],
    /// Camera view-projection matrix.
    pub view_proj: [[f32; 4]; 4],
    /// Time of day (0-1).
    pub time_of_day: f32,
    /// Weather intensity (0-1).
    pub weather: f32,
    /// Dirty chunks that need re-mesh.
    pub dirty_chunks: Vec<(i32, i32, i32)>,
}

impl WorldStateSnapshot {
    /// Creates a new empty snapshot.
    #[must_use]
    pub fn new() -> Self {
        Self {
            frame: 0,
            entity_count: 0,
            camera_pos: [0.0; 3],
            view_proj: [[0.0; 4]; 4],
            time_of_day: 0.5,
            weather: 0.0,
            dirty_chunks: Vec::new(),
        }
    }
}

/// Triple-buffered world state for lock-free access.
///
/// Game logic writes to one buffer while rendering reads from another.
/// A third buffer is used as a staging area for atomic swaps.
///
/// ```text
/// Game Logic → [Write Buffer] → swap → [Ready Buffer] → swap → [Read Buffer] ← Rendering
/// ```
pub struct SharedWorldState {
    /// Triple buffer of world states.
    buffers: [RwLock<WorldStateSnapshot>; 3],
    /// Index of buffer currently being written.
    write_idx: AtomicUsize,
    /// Index of buffer ready for reading.
    ready_idx: AtomicUsize,
    /// Index of buffer currently being read.
    read_idx: AtomicUsize,
    /// Generation counter for synchronization.
    generation: AtomicUsize,
}

impl SharedWorldState {
    /// Creates a new shared world state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: [
                RwLock::new(WorldStateSnapshot::new()),
                RwLock::new(WorldStateSnapshot::new()),
                RwLock::new(WorldStateSnapshot::new()),
            ],
            write_idx: AtomicUsize::new(0),
            ready_idx: AtomicUsize::new(1),
            read_idx: AtomicUsize::new(2),
            generation: AtomicUsize::new(0),
        }
    }
    
    /// Begins a write operation.
    ///
    /// Returns a guard that allows writing to the current write buffer.
    /// When dropped, the write is published for reading.
    #[must_use]
    pub fn begin_write(&self) -> WriteGuard<'_> {
        let idx = self.write_idx.load(Ordering::Acquire);
        WriteGuard {
            state: self,
            idx,
            guard: self.buffers[idx].write(),
        }
    }
    
    /// Gets the current snapshot for reading.
    ///
    /// This is non-blocking and always returns immediately.
    #[must_use]
    pub fn read(&self) -> impl std::ops::Deref<Target = WorldStateSnapshot> + '_ {
        // Swap ready and read if there's new data
        let ready = self.ready_idx.load(Ordering::Acquire);
        let read = self.read_idx.load(Ordering::Acquire);
        
        // Try to swap ready → read
        let _ = self.read_idx.compare_exchange(
            read,
            ready,
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
        
        let idx = self.read_idx.load(Ordering::Acquire);
        self.buffers[idx].read()
    }
    
    /// Returns the current generation (incremented on each publish).
    #[must_use]
    pub fn generation(&self) -> usize {
        self.generation.load(Ordering::Acquire)
    }
    
    /// Publishes the write buffer (makes it available for reading).
    fn publish(&self) {
        let write = self.write_idx.load(Ordering::Acquire);
        let ready = self.ready_idx.load(Ordering::Acquire);
        
        // Swap write → ready
        self.ready_idx.store(write, Ordering::Release);
        self.write_idx.store(ready, Ordering::Release);
        
        self.generation.fetch_add(1, Ordering::Release);
    }
}

impl Default for SharedWorldState {
    fn default() -> Self {
        Self::new()
    }
}

/// Guard for writing to world state.
///
/// When dropped, publishes the changes for reading.
pub struct WriteGuard<'a> {
    state: &'a SharedWorldState,
    #[allow(dead_code)]
    idx: usize,
    guard: parking_lot::RwLockWriteGuard<'a, WorldStateSnapshot>,
}

impl std::ops::Deref for WriteGuard<'_> {
    type Target = WorldStateSnapshot;
    
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl std::ops::DerefMut for WriteGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl Drop for WriteGuard<'_> {
    fn drop(&mut self) {
        self.state.publish();
    }
}

/// Convenience struct for passing world state between threads.
pub struct WorldStateHandle {
    state: std::sync::Arc<SharedWorldState>,
}

impl WorldStateHandle {
    /// Creates a new handle wrapping a shared state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: std::sync::Arc::new(SharedWorldState::new()),
        }
    }
    
    /// Clones the handle (cheap, just increments refcount).
    #[must_use]
    pub fn clone_handle(&self) -> Self {
        Self {
            state: std::sync::Arc::clone(&self.state),
        }
    }
    
    /// Gets the shared state.
    #[must_use]
    pub fn state(&self) -> &SharedWorldState {
        &self.state
    }
}

impl Default for WorldStateHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for WorldStateHandle {
    fn clone(&self) -> Self {
        self.clone_handle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_triple_buffer() {
        let state = SharedWorldState::new();
        
        // Write frame 1
        {
            let mut write = state.begin_write();
            write.frame = 1;
            write.camera_pos = [1.0, 2.0, 3.0];
        }
        
        // Read should see frame 1
        {
            let read = state.read();
            assert_eq!(read.frame, 1);
            assert_eq!(read.camera_pos, [1.0, 2.0, 3.0]);
        }
        
        // Write frame 2
        {
            let mut write = state.begin_write();
            write.frame = 2;
            write.camera_pos = [4.0, 5.0, 6.0];
        }
        
        // Read should see frame 2
        {
            let read = state.read();
            assert_eq!(read.frame, 2);
        }
    }
    
    #[test]
    fn test_generation_tracking() {
        let state = SharedWorldState::new();
        
        let gen1 = state.generation();
        
        {
            let _write = state.begin_write();
        }
        
        let gen2 = state.generation();
        assert!(gen2 > gen1);
    }
}
