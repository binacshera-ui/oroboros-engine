//! # Pool Allocator
//!
//! Fixed-size block allocator for objects that are frequently allocated and freed.

use std::marker::PhantomData;

/// A pool allocator for fixed-size objects.
///
/// Objects can be allocated and freed individually, but all objects
/// have the same size. This is perfect for things like particles,
/// network packets, or temporary game objects.
///
/// # Thread Safety
///
/// This pool is NOT thread-safe. Use one pool per thread or wrap in a mutex.
///
/// # Example
///
/// ```rust,ignore
/// struct Particle { x: f32, y: f32, life: f32 }
///
/// let mut pool: PoolAllocator<Particle> = PoolAllocator::new(10000);
///
/// // Allocate - O(1), no heap allocation
/// let handle = pool.allocate(Particle { x: 0.0, y: 0.0, life: 1.0 })?;
///
/// // Free - O(1), no heap deallocation
/// pool.free(handle);
/// ```
pub struct PoolAllocator<T> {
    /// The storage array.
    storage: Box<[Option<T>]>,
    /// Free list - indices of available slots.
    free_list: Vec<usize>,
    /// Number of allocated objects.
    allocated_count: usize,
    /// Total capacity.
    capacity: usize,
    /// Marker for T.
    _phantom: PhantomData<T>,
}

/// Handle to an allocated object in a pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PoolHandle {
    /// Index into the pool.
    index: usize,
}

impl<T> PoolAllocator<T> {
    /// Creates a new pool with the specified capacity.
    ///
    /// All memory is pre-allocated upfront.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of objects
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than zero");

        // Pre-allocate storage
        let storage: Vec<Option<T>> = (0..capacity).map(|_| None).collect();

        // Pre-allocate free list with all indices
        let free_list: Vec<usize> = (0..capacity).rev().collect();

        Self {
            storage: storage.into_boxed_slice(),
            free_list,
            allocated_count: 0,
            capacity,
            _phantom: PhantomData,
        }
    }

    /// Returns the total capacity.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of currently allocated objects.
    #[inline]
    #[must_use]
    pub const fn allocated_count(&self) -> usize {
        self.allocated_count
    }

    /// Returns the number of free slots.
    #[inline]
    #[must_use]
    pub fn free_count(&self) -> usize {
        self.capacity - self.allocated_count
    }

    /// Allocates a slot and stores the object.
    ///
    /// This is a **O(1)** operation with **zero heap allocations**.
    ///
    /// # Arguments
    ///
    /// * `value` - The object to store
    ///
    /// # Returns
    ///
    /// A handle to the allocated object, or None if pool is full.
    pub fn allocate(&mut self, value: T) -> Option<PoolHandle> {
        let index = self.free_list.pop()?;

        self.storage[index] = Some(value);
        self.allocated_count += 1;

        Some(PoolHandle { index })
    }

    /// Frees an allocated object.
    ///
    /// This is a **O(1)** operation with **zero heap deallocations**.
    ///
    /// # Arguments
    ///
    /// * `handle` - The handle to free
    ///
    /// # Returns
    ///
    /// The freed object, or None if handle was invalid.
    pub fn free(&mut self, handle: PoolHandle) -> Option<T> {
        if handle.index >= self.capacity {
            return None;
        }

        let value = self.storage[handle.index].take()?;
        self.free_list.push(handle.index);
        self.allocated_count -= 1;

        Some(value)
    }

    /// Gets a reference to an allocated object.
    ///
    /// # Arguments
    ///
    /// * `handle` - The handle to look up
    #[inline]
    #[must_use]
    pub fn get(&self, handle: PoolHandle) -> Option<&T> {
        self.storage.get(handle.index)?.as_ref()
    }

    /// Gets a mutable reference to an allocated object.
    ///
    /// # Arguments
    ///
    /// * `handle` - The handle to look up
    #[inline]
    pub fn get_mut(&mut self, handle: PoolHandle) -> Option<&mut T> {
        self.storage.get_mut(handle.index)?.as_mut()
    }

    /// Clears all allocations, resetting the pool.
    ///
    /// This is a **zero-heap-allocation** operation - memory is not freed.
    pub fn clear(&mut self) {
        for slot in self.storage.iter_mut() {
            *slot = None;
        }
        self.free_list.clear();
        self.free_list.extend((0..self.capacity).rev());
        self.allocated_count = 0;
    }

    /// Iterates over all allocated objects.
    pub fn iter(&self) -> impl Iterator<Item = (PoolHandle, &T)> {
        self.storage
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| slot.as_ref().map(|v| (PoolHandle { index }, v)))
    }

    /// Iterates mutably over all allocated objects.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (PoolHandle, &mut T)> {
        self.storage
            .iter_mut()
            .enumerate()
            .filter_map(|(index, slot)| slot.as_mut().map(|v| (PoolHandle { index }, v)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_allocate_free() {
        let mut pool: PoolAllocator<u32> = PoolAllocator::new(10);

        let h1 = pool.allocate(42).unwrap();
        assert_eq!(*pool.get(h1).unwrap(), 42);
        assert_eq!(pool.allocated_count(), 1);

        let freed = pool.free(h1).unwrap();
        assert_eq!(freed, 42);
        assert_eq!(pool.allocated_count(), 0);
    }

    #[test]
    fn test_pool_full() {
        let mut pool: PoolAllocator<u8> = PoolAllocator::new(2);

        let _ = pool.allocate(1).unwrap();
        let _ = pool.allocate(2).unwrap();
        assert!(pool.allocate(3).is_none());
    }

    #[test]
    fn test_pool_reuse() {
        let mut pool: PoolAllocator<u32> = PoolAllocator::new(1);

        let h1 = pool.allocate(1).unwrap();
        pool.free(h1);

        let h2 = pool.allocate(2).unwrap();
        assert_eq!(h1.index, h2.index); // Same slot reused
        assert_eq!(*pool.get(h2).unwrap(), 2);
    }
}
