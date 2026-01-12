//! Zero-copy position view into ECS storage.
//!
//! This module provides direct read access to ECS position data
//! without any copying. The rendering system can read positions
//! directly from the same memory that the game logic writes to.

use std::sync::atomic::{AtomicU64, Ordering};
use std::ptr::NonNull;

/// Zero-copy view into ECS position storage.
///
/// SAFETY: This struct provides raw pointer access to ECS data.
/// The caller must ensure:
/// 1. The underlying storage outlives this view
/// 2. No writes occur while reading (use generation for sync)
///
/// # Memory Layout
///
/// Position data is expected to be laid out as:
/// ```text
/// [x: f32, y: f32, z: f32, _pad: f32] × entity_count
/// ```
pub struct PositionView {
    /// Pointer to the start of position data.
    data: NonNull<f32>,
    /// Number of entities (positions).
    count: usize,
    /// Stride between positions in f32s (typically 4 for [x,y,z,pad]).
    stride: usize,
    /// Generation counter for synchronization.
    generation: *const AtomicU64,
    /// Last seen generation.
    last_generation: u64,
}

// SAFETY: Position data is Send+Sync in the ECS
unsafe impl Send for PositionView {}
unsafe impl Sync for PositionView {}

impl PositionView {
    /// Creates a new position view from raw ECS storage.
    ///
    /// # Safety
    ///
    /// - `data` must point to valid position data
    /// - `data` must remain valid for the lifetime of this view
    /// - `count * stride * sizeof(f32)` bytes must be readable from `data`
    /// - `generation` must point to a valid atomic counter
    #[must_use]
    pub unsafe fn new(
        data: *const f32,
        count: usize,
        stride: usize,
        generation: *const AtomicU64,
    ) -> Self {
        Self {
            data: NonNull::new_unchecked(data as *mut f32),
            count,
            stride,
            generation,
            last_generation: 0,
        }
    }
    
    /// Returns true if the data has been updated since last check.
    ///
    /// Call this before reading to ensure you're seeing fresh data.
    #[must_use]
    pub fn has_updates(&mut self) -> bool {
        // SAFETY: generation pointer was validated at construction
        let current = unsafe { (*self.generation).load(Ordering::Acquire) };
        if current != self.last_generation {
            self.last_generation = current;
            true
        } else {
            false
        }
    }
    
    /// Returns the current generation counter.
    #[must_use]
    pub fn generation(&self) -> u64 {
        // SAFETY: generation pointer was validated at construction
        unsafe { (*self.generation).load(Ordering::Acquire) }
    }
    
    /// Returns the number of positions.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }
    
    /// Gets a position by entity index.
    ///
    /// # Safety
    ///
    /// The caller must ensure no concurrent writes are happening.
    /// Use `has_updates()` + synchronization to ensure safety.
    #[inline]
    #[must_use]
    pub unsafe fn get(&self, index: usize) -> Option<[f32; 3]> {
        if index >= self.count {
            return None;
        }
        
        let offset = index * self.stride;
        let ptr = self.data.as_ptr().add(offset);
        
        Some([
            *ptr,
            *ptr.add(1),
            *ptr.add(2),
        ])
    }
    
    /// Gets a position by entity index (unchecked).
    ///
    /// # Safety
    ///
    /// - `index` must be less than `count`
    /// - No concurrent writes must be happening
    #[inline]
    #[must_use]
    pub unsafe fn get_unchecked(&self, index: usize) -> [f32; 3] {
        let offset = index * self.stride;
        let ptr = self.data.as_ptr().add(offset);
        
        [
            *ptr,
            *ptr.add(1),
            *ptr.add(2),
        ]
    }
    
    /// Iterates over all positions.
    ///
    /// # Safety
    ///
    /// No concurrent writes must be happening during iteration.
    pub unsafe fn iter(&self) -> impl Iterator<Item = [f32; 3]> + '_ {
        (0..self.count).map(move |i| self.get_unchecked(i))
    }
    
    /// Returns a raw pointer to the data for GPU upload.
    ///
    /// This can be used with mapped GPU buffers for true zero-copy.
    #[must_use]
    pub fn as_ptr(&self) -> *const f32 {
        self.data.as_ptr()
    }
    
    /// Returns the total size in bytes.
    #[must_use]
    pub fn size_bytes(&self) -> usize {
        self.count * self.stride * std::mem::size_of::<f32>()
    }
}

/// Safe wrapper that owns the generation counter.
pub struct OwnedPositionView {
    /// The underlying view.
    view: PositionView,
    /// Owned generation counter.
    #[allow(dead_code)]
    generation: Box<AtomicU64>,
}

impl OwnedPositionView {
    /// Creates a new owned view from a boxed slice of positions.
    ///
    /// # Safety
    ///
    /// The slice must be [f32; 4] per position (x, y, z, pad).
    #[must_use]
    pub fn from_slice(positions: &[f32], count: usize) -> Self {
        let generation = Box::new(AtomicU64::new(0));
        let gen_ptr = generation.as_ref() as *const AtomicU64;
        
        // SAFETY: We own the generation and the slice is valid
        let view = unsafe {
            PositionView::new(positions.as_ptr(), count, 4, gen_ptr)
        };
        
        Self { view, generation }
    }
    
    /// Increments the generation counter (call after updating positions).
    pub fn mark_updated(&self) {
        self.generation.fetch_add(1, Ordering::Release);
    }
    
    /// Returns a reference to the view.
    #[must_use]
    pub fn view(&mut self) -> &mut PositionView {
        &mut self.view
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_position_view_basics() {
        // Simulate ECS storage: 3 positions with stride 4
        let data: Vec<f32> = vec![
            1.0, 2.0, 3.0, 0.0,  // Entity 0
            4.0, 5.0, 6.0, 0.0,  // Entity 1
            7.0, 8.0, 9.0, 0.0,  // Entity 2
        ];
        
        let mut view = OwnedPositionView::from_slice(&data, 3);
        
        unsafe {
            assert_eq!(view.view().get(0), Some([1.0, 2.0, 3.0]));
            assert_eq!(view.view().get(1), Some([4.0, 5.0, 6.0]));
            assert_eq!(view.view().get(2), Some([7.0, 8.0, 9.0]));
            assert_eq!(view.view().get(3), None);
        }
    }
    
    #[test]
    fn test_generation_tracking() {
        let data: Vec<f32> = vec![0.0; 4];
        let mut view = OwnedPositionView::from_slice(&data, 1);
        
        // First check should return true (gen changed from 0 to 0)
        // Actually, initial last_gen is 0 and current is 0, so no update
        assert!(!view.view().has_updates());
        
        // Mark update
        view.mark_updated();
        
        // Now should have updates
        assert!(view.view().has_updates());
        
        // Subsequent check without update should return false
        assert!(!view.view().has_updates());
    }
}
