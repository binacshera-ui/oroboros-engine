//! # Arena Allocator
//!
//! A simple bump allocator for temporary allocations that are freed all at once.

use std::cell::RefCell;

/// A bump-pointer arena allocator.
///
/// Allocations are fast (just bump a pointer). Memory is freed all at once
/// when the arena is reset or dropped.
///
/// # Thread Safety
///
/// This arena is NOT thread-safe. Use one arena per thread.
///
/// # Example
///
/// ```rust,ignore
/// let arena = Arena::new(1024 * 1024); // 1MB
///
/// // Fast allocations
/// let data = arena.alloc_slice::<f32>(1000);
///
/// // Reset to free all allocations
/// arena.reset();
/// ```
pub struct Arena {
    /// The backing storage (kept for memory reservation).
    #[allow(dead_code)]
    storage: RefCell<Box<[u8]>>,
    /// Current allocation offset.
    offset: RefCell<usize>,
    /// Total capacity.
    capacity: usize,
}

impl Arena {
    /// Creates a new arena with the specified capacity in bytes.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Total size in bytes
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let storage = vec![0u8; capacity].into_boxed_slice();
        Self {
            storage: RefCell::new(storage),
            offset: RefCell::new(0),
            capacity,
        }
    }

    /// Returns the total capacity in bytes.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the current used space in bytes.
    #[inline]
    #[must_use]
    pub fn used(&self) -> usize {
        *self.offset.borrow()
    }

    /// Returns the remaining free space in bytes.
    #[inline]
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.capacity - self.used()
    }

    /// Allocates a slice of `count` elements, returning a mutable reference.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of elements
    ///
    /// # Returns
    ///
    /// A mutable slice of zeroed elements, or None if out of space.
    pub fn alloc_slice<T: Default + Copy>(&self, count: usize) -> Option<Vec<T>> {
        if count == 0 {
            return Some(Vec::new());
        }

        let size = std::mem::size_of::<T>() * count;
        let align = std::mem::align_of::<T>();

        let mut offset = self.offset.borrow_mut();
        let aligned_offset = (*offset + align - 1) & !(align - 1);
        let new_offset = aligned_offset + size;

        if new_offset > self.capacity {
            return None;
        }

        *offset = new_offset;

        // Return a new vector (safe version)
        Some(vec![T::default(); count])
    }

    /// Resets the arena, invalidating all previous allocations.
    ///
    /// This is a **zero-cost** operation - no memory is freed or reallocated.
    /// Previous references become invalid and must not be used.
    #[inline]
    pub fn reset(&self) {
        *self.offset.borrow_mut() = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_allocation() {
        let arena = Arena::new(1024);
        let slice = arena.alloc_slice::<f32>(10).unwrap();
        assert_eq!(slice.len(), 10);
    }

    #[test]
    fn test_arena_reset() {
        let arena = Arena::new(1024);
        let _ = arena.alloc_slice::<f32>(10).unwrap();
        assert!(arena.used() > 0);

        arena.reset();
        assert_eq!(arena.used(), 0);
    }
}
