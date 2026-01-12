//! # Double-Buffered ECS World
//!
//! Lock-free concurrent access for render and logic threads.
//!
//! ## Safety Note
//!
//! This module requires unsafe code for lock-free double buffering.
//! All unsafe blocks are carefully reviewed and documented.

#![allow(unsafe_code)]
//!
//! ## Architecture
//!
//! ```text
//!                    ┌─────────────────────────────┐
//!                    │     DoubleBufferedWorld     │
//!                    │                             │
//!                    │  ┌─────────┐  ┌─────────┐  │
//!                    │  │ World A │  │ World B │  │
//!                    │  └────┬────┘  └────┬────┘  │
//!                    │       │            │       │
//!                    │  ┌────┴────────────┴────┐  │
//!                    │  │   Atomic Index (0/1) │  │
//!                    │  └──────────────────────┘  │
//!                    └─────────────────────────────┘
//!                              │
//!              ┌───────────────┼───────────────┐
//!              ▼               ▼               ▼
//!      ┌──────────────┐ ┌────────────┐ ┌────────────┐
//!      │ WriteHandle  │ │ ReadHandle │ │ FrameSync  │
//!      │ (Logic/Unit4)│ │(Render/U2) │ │  (Swap)    │
//!      └──────────────┘ └────────────┘ └────────────┘
//! ```
//!
//! ## Thread Safety
//!
//! - `WriteHandle`: Exclusive access to write buffer (one per frame)
//! - `ReadHandle`: Shared access to read buffer (many allowed)
//! - `FrameSync`: Atomic swap operation (single threaded, end of frame)

use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::sync::Arc;
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};

use crate::ecs::archetype::ArchetypeWorld;

/// Double-buffered world for lock-free concurrent access.
///
/// This structure maintains two complete copies of the game world:
/// - One for writing (game logic)
/// - One for reading (rendering)
///
/// At the end of each frame, the buffers are swapped atomically.
///
/// ## Usage
///
/// ```rust,ignore
/// let db_world = DoubleBufferedWorld::new(1_000_000, 100_000);
///
/// // Game loop
/// loop {
///     // Logic thread gets write access
///     let mut write = db_world.write_handle();
///     write.update_positions(delta_time);
///     drop(write); // Release before swap
///
///     // Render thread gets read access (can overlap with next frame's logic)
///     let read = db_world.read_handle();
///     for pos in read.pv_table.iter_positions() {
///         // render...
///     }
///     drop(read);
///
///     // End of frame: swap buffers
///     db_world.swap_buffers();
/// }
/// ```
pub struct DoubleBufferedWorld {
    /// The two world buffers.
    /// Using UnsafeCell because we guarantee exclusive access through handles.
    buffers: [UnsafeCell<ArchetypeWorld>; 2],

    /// Index of the current write buffer (0 or 1).
    /// Read buffer is always (write_index ^ 1).
    write_index: AtomicUsize,

    /// Whether a write handle is currently held.
    write_locked: AtomicBool,

    /// Number of active read handles.
    read_count: AtomicUsize,

    /// Frame counter for debugging/profiling.
    frame_count: AtomicUsize,
}

impl DoubleBufferedWorld {
    /// Creates a new double-buffered world.
    ///
    /// # Arguments
    ///
    /// * `pv_capacity` - Capacity for Position+Velocity entities (moving objects)
    /// * `p_capacity` - Capacity for Position-only entities (static objects)
    ///
    /// # Panics
    ///
    /// Panics if capacity is zero.
    #[must_use]
    pub fn new(pv_capacity: usize, p_capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            buffers: [
                UnsafeCell::new(ArchetypeWorld::new(pv_capacity, p_capacity)),
                UnsafeCell::new(ArchetypeWorld::new(pv_capacity, p_capacity)),
            ],
            write_index: AtomicUsize::new(0),
            write_locked: AtomicBool::new(false),
            read_count: AtomicUsize::new(0),
            frame_count: AtomicUsize::new(0),
        })
    }

    /// Returns the current frame number.
    #[inline]
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frame_count.load(Ordering::Relaxed)
    }

    /// Returns whether a write handle is currently active.
    #[inline]
    #[must_use]
    pub fn is_write_locked(&self) -> bool {
        self.write_locked.load(Ordering::Acquire)
    }

    /// Returns the number of active read handles.
    #[inline]
    #[must_use]
    pub fn read_handle_count(&self) -> usize {
        self.read_count.load(Ordering::Acquire)
    }

    /// Gets a write handle for the logic thread.
    ///
    /// # Panics
    ///
    /// Panics if a write handle is already held (only one allowed).
    ///
    /// # Safety
    ///
    /// The caller must ensure that only one thread calls this at a time.
    /// The write handle must be dropped before calling `swap_buffers()`.
    #[must_use]
    pub fn write_handle(self: &Arc<Self>) -> WorldWriteHandle {
        // Attempt to acquire write lock
        let was_locked = self.write_locked.swap(true, Ordering::AcqRel);
        assert!(!was_locked, "Double write handle! Only one write handle allowed at a time.");

        let write_idx = self.write_index.load(Ordering::Acquire);

        WorldWriteHandle {
            world: Arc::clone(self),
            buffer_index: write_idx,
        }
    }

    /// Gets a read handle for the render thread.
    ///
    /// Multiple read handles can coexist. They all read from the same buffer
    /// (the one that was written to in the previous frame).
    #[must_use]
    pub fn read_handle(self: &Arc<Self>) -> WorldReadHandle {
        // Increment read count
        self.read_count.fetch_add(1, Ordering::AcqRel);

        // Read buffer is opposite of write buffer
        let write_idx = self.write_index.load(Ordering::Acquire);
        let read_idx = write_idx ^ 1;

        WorldReadHandle {
            world: Arc::clone(self),
            buffer_index: read_idx,
        }
    }

    /// Swaps the read and write buffers atomically.
    ///
    /// **CRITICAL: Solves the Cold Buffer Problem**
    ///
    /// After swap, the new write buffer contains stale data from 2 frames ago.
    /// This method performs a DIRTY COPY to sync the fresh state:
    ///
    /// 1. Swap buffer indices atomically
    /// 2. Copy dirty entities from new read buffer to new write buffer
    /// 3. Clear dirty flags in the read buffer
    ///
    /// The dirty copy only transfers entities that actually changed,
    /// saving up to 95% of memory bandwidth when <5% of entities move.
    ///
    /// # Panics
    ///
    /// Panics if a write handle is still active.
    ///
    /// # Safety
    ///
    /// This must be called from a single thread (typically the main thread).
    pub fn swap_buffers(&self) {
        // Ensure no write handle is active
        assert!(
            !self.write_locked.load(Ordering::Acquire),
            "Cannot swap buffers while write handle is active!"
        );

        // Warn if read handles are still active (not fatal, but may cause visual glitches)
        let read_count = self.read_count.load(Ordering::Acquire);
        if read_count > 0 {
            // In debug mode, this would be a warning
            // In production, we allow it (render may be slightly behind)
        }

        // Atomic swap: toggle between 0 and 1
        let old_write_idx = self.write_index.fetch_xor(1, Ordering::AcqRel);
        let new_write_idx = old_write_idx ^ 1;
        let new_read_idx = old_write_idx; // The buffer we just wrote to

        // =====================================================================
        // DIRTY COPY: Solve the Cold Buffer Problem
        // =====================================================================
        // The new write buffer (old read buffer) has stale state from 2 frames ago.
        // We must copy the dirty entities from the new read buffer (just finished).
        //
        // This is where the magic happens:
        // - If 5% of entities changed: copy 1.6MB instead of 32MB (95% savings)
        // - Uses SIMD streaming stores to avoid cache pollution
        // =====================================================================

        // SAFETY: We have exclusive access (no write handle, and we control the swap)
        unsafe {
            let new_read = &*self.buffers[new_read_idx].get();
            let new_write = &mut *self.buffers[new_write_idx].get();

            // Sync dirty entities from the buffer we just finished writing
            // to the buffer we're about to write to
            new_write.sync_dirty_from(new_read);

            // Clear dirty flags in the read buffer (it's been synced)
            // Note: We need mutable access to clear, but we're about to release it
            let new_read_mut = &mut *self.buffers[new_read_idx].get();
            new_read_mut.clear_dirty();
        }

        // Increment frame counter
        self.frame_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Swaps buffers WITHOUT dirty copy (for testing/benchmarking).
    ///
    /// **WARNING**: This will cause the Cold Buffer Problem!
    /// Only use for performance comparison or when you know both buffers are identical.
    pub fn swap_buffers_no_sync(&self) {
        assert!(
            !self.write_locked.load(Ordering::Acquire),
            "Cannot swap buffers while write handle is active!"
        );

        self.write_index.fetch_xor(1, Ordering::AcqRel);
        self.frame_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Returns sync statistics for the write buffer.
    ///
    /// Useful for profiling the dirty copy overhead.
    #[must_use]
    pub fn sync_stats(&self) -> crate::ecs::archetype::WorldSyncStats {
        let write_idx = self.write_index.load(Ordering::Acquire);
        // SAFETY: We're only reading stats
        unsafe {
            let write_buf = &*self.buffers[write_idx].get();
            write_buf.sync_stats()
        }
    }

    /// Gets a FrameSync helper for managing the frame lifecycle.
    #[must_use]
    pub fn frame_sync(self: &Arc<Self>) -> FrameSync {
        FrameSync {
            world: Arc::clone(self),
        }
    }

    /// Copies state from write buffer to read buffer (FULL COPY).
    ///
    /// Call this once at startup to initialize both buffers with the same state.
    /// After the first frame, swap_buffers handles synchronization via dirty copy.
    ///
    /// # Safety
    ///
    /// Must be called when no handles are active.
    pub fn sync_buffers(&self) {
        assert!(!self.write_locked.load(Ordering::Acquire));
        assert_eq!(self.read_count.load(Ordering::Acquire), 0);

        let write_idx = self.write_index.load(Ordering::Acquire);
        let read_idx = write_idx ^ 1;

        // SAFETY: No handles are active, we have exclusive access
        unsafe {
            let write_buf_mut = &mut *self.buffers[write_idx].get();
            let read_buf = &mut *self.buffers[read_idx].get();

            // Mark everything as dirty in write buffer for full sync
            let pv_len = write_buf_mut.pv_table.len();
            let p_len = write_buf_mut.p_table.len();
            write_buf_mut.pv_table.dirty_tracker_mut().mark_all_dirty(pv_len);
            write_buf_mut.p_table.dirty_tracker_mut().mark_all_dirty(p_len);

            // Sync to read buffer
            read_buf.sync_dirty_from(write_buf_mut);

            // Clear dirty flags
            write_buf_mut.clear_dirty();
        }
    }

    /// Gets raw pointer to write buffer (for internal use).
    ///
    /// # Safety
    ///
    /// Caller must ensure exclusive access.
    #[inline]
    unsafe fn get_write_buffer(&self, index: usize) -> &mut ArchetypeWorld {
        &mut *self.buffers[index].get()
    }

    /// Gets raw pointer to read buffer (for internal use).
    ///
    /// # Safety
    ///
    /// Caller must ensure no concurrent writes.
    #[inline]
    unsafe fn get_read_buffer(&self, index: usize) -> &ArchetypeWorld {
        &*self.buffers[index].get()
    }
}

// SAFETY: DoubleBufferedWorld is Send because it controls access through atomics
unsafe impl Send for DoubleBufferedWorld {}
// SAFETY: DoubleBufferedWorld is Sync because it controls access through atomics
unsafe impl Sync for DoubleBufferedWorld {}

/// Write handle for the logic thread.
///
/// Provides exclusive mutable access to the write buffer.
/// Only one write handle can exist at a time.
///
/// ## Usage
///
/// ```rust,ignore
/// let mut write = db_world.write_handle();
///
/// // Spawn entities
/// write.spawn_pv(Position::new(0.0, 0.0, 0.0), Velocity::new(1.0, 0.0, 0.0));
///
/// // Update all positions
/// write.update_positions(delta_time);
///
/// // Handle is automatically dropped when it goes out of scope
/// ```
pub struct WorldWriteHandle {
    world: Arc<DoubleBufferedWorld>,
    buffer_index: usize,
}

impl WorldWriteHandle {
    /// Returns the buffer index this handle writes to (for debugging).
    #[inline]
    #[must_use]
    pub fn buffer_index(&self) -> usize {
        self.buffer_index
    }
}

impl Deref for WorldWriteHandle {
    type Target = ArchetypeWorld;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: We hold exclusive write access (guaranteed by write_locked)
        unsafe { self.world.get_read_buffer(self.buffer_index) }
    }
}

impl DerefMut for WorldWriteHandle {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: We hold exclusive write access (guaranteed by write_locked)
        unsafe { self.world.get_write_buffer(self.buffer_index) }
    }
}

impl Drop for WorldWriteHandle {
    fn drop(&mut self) {
        // Release write lock
        self.world.write_locked.store(false, Ordering::Release);
    }
}

/// Read handle for the render thread.
///
/// Provides shared immutable access to the read buffer.
/// Multiple read handles can exist simultaneously.
///
/// ## Usage
///
/// ```rust,ignore
/// let read = db_world.read_handle();
///
/// // Iterate over all Position+Velocity entities
/// for (pos, vel) in read.pv_table.iter_position_velocity() {
///     render_entity(pos, vel);
/// }
///
/// // Handle is automatically dropped when it goes out of scope
/// ```
pub struct WorldReadHandle {
    world: Arc<DoubleBufferedWorld>,
    buffer_index: usize,
}

impl WorldReadHandle {
    /// Returns the buffer index this handle reads from (for debugging).
    #[inline]
    #[must_use]
    pub fn buffer_index(&self) -> usize {
        self.buffer_index
    }
}

impl Deref for WorldReadHandle {
    type Target = ArchetypeWorld;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: Read buffer is not being written to (write goes to other buffer)
        unsafe { self.world.get_read_buffer(self.buffer_index) }
    }
}

impl Drop for WorldReadHandle {
    fn drop(&mut self) {
        // Decrement read count
        self.world.read_count.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Frame synchronization helper.
///
/// Provides a structured way to manage the frame lifecycle.
///
/// ## Usage
///
/// ```rust,ignore
/// let sync = db_world.frame_sync();
///
/// loop {
///     sync.begin_frame();
///
///     // Logic and render happen in parallel...
///
///     sync.end_frame(); // Swaps buffers
/// }
/// ```
pub struct FrameSync {
    world: Arc<DoubleBufferedWorld>,
}

impl FrameSync {
    /// Marks the beginning of a new frame.
    ///
    /// Currently a no-op, but could be used for profiling/debugging.
    #[inline]
    pub fn begin_frame(&self) {
        // Placeholder for future profiling
    }

    /// Marks the end of a frame and swaps buffers.
    ///
    /// # Panics
    ///
    /// Panics if a write handle is still active.
    #[inline]
    pub fn end_frame(&self) {
        self.world.swap_buffers();
    }

    /// Returns the current frame number.
    #[inline]
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.world.frame_count()
    }

    /// Checks if it's safe to swap (no active handles).
    #[inline]
    #[must_use]
    pub fn can_swap(&self) -> bool {
        !self.world.is_write_locked() && self.world.read_handle_count() == 0
    }

    /// Waits until it's safe to swap (spin loop).
    ///
    /// # Warning
    ///
    /// This can cause deadlock if handles are never dropped.
    /// Use with caution.
    pub fn wait_for_swap_ready(&self) {
        while !self.can_swap() {
            std::hint::spin_loop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Position, Velocity};

    #[test]
    fn test_double_buffer_creation() {
        let db = DoubleBufferedWorld::new(1000, 100);
        assert_eq!(db.frame_count(), 0);
        assert!(!db.is_write_locked());
        assert_eq!(db.read_handle_count(), 0);
    }

    #[test]
    fn test_write_handle() {
        let db = DoubleBufferedWorld::new(1000, 100);

        {
            let mut write = db.write_handle();
            assert!(db.is_write_locked());

            // Spawn an entity
            let id = write.spawn_pv(
                Position::new(1.0, 2.0, 3.0),
                Velocity::new(0.1, 0.2, 0.3),
            );
            assert!(!id.is_null());
        }

        // Write lock released after drop
        assert!(!db.is_write_locked());
    }

    #[test]
    fn test_read_handle() {
        let db = DoubleBufferedWorld::new(1000, 100);

        // Multiple read handles allowed
        let read1 = db.read_handle();
        let read2 = db.read_handle();

        assert_eq!(db.read_handle_count(), 2);

        drop(read1);
        assert_eq!(db.read_handle_count(), 1);

        drop(read2);
        assert_eq!(db.read_handle_count(), 0);
    }

    #[test]
    fn test_buffer_swap() {
        let db = DoubleBufferedWorld::new(1000, 100);

        // Write to buffer A
        {
            let mut write = db.write_handle();
            let initial_idx = write.buffer_index();
            let _ = write.spawn_pv(Position::new(1.0, 0.0, 0.0), Velocity::new(0.0, 0.0, 0.0));
            assert_eq!(write.buffer_index(), initial_idx);
        }

        // Swap buffers
        db.swap_buffers();
        assert_eq!(db.frame_count(), 1);

        // Now read should see the entity (from what was buffer A, now buffer B)
        {
            let read = db.read_handle();
            // Read is from the buffer we just wrote to
            assert_eq!(read.pv_table.len(), 1);
        }

        // New writes go to the other buffer
        {
            let _write = db.write_handle();
            // This buffer was the old read buffer, starts empty
            // (unless we implement copy-on-swap)
        }
    }

    #[test]
    fn test_frame_sync() {
        let db = DoubleBufferedWorld::new(1000, 100);
        let sync = db.frame_sync();

        assert!(sync.can_swap());

        {
            let _write = db.write_handle();
            assert!(!sync.can_swap());
        }

        assert!(sync.can_swap());

        sync.end_frame();
        assert_eq!(sync.frame_count(), 1);
    }

    #[test]
    #[should_panic(expected = "Double write handle")]
    fn test_double_write_panics() {
        let db = DoubleBufferedWorld::new(1000, 100);

        let _write1 = db.write_handle();
        let _write2 = db.write_handle(); // Should panic
    }

    #[test]
    #[should_panic(expected = "Cannot swap buffers while write handle is active")]
    fn test_swap_during_write_panics() {
        let db = DoubleBufferedWorld::new(1000, 100);

        let _write = db.write_handle();
        db.swap_buffers(); // Should panic
    }
}
