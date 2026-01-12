//! # Component Storage
//!
//! Pre-allocated, dense component storage with zero runtime allocations.
//!
//! The storage uses a dense array strategy:
//! - All component slots are pre-allocated at creation
//! - Access is O(1) via entity index
//! - Iteration is cache-friendly (contiguous memory)

use super::component::Component;
use std::marker::PhantomData;

/// Pre-allocated storage for a single component type.
///
/// This storage guarantees:
/// - Zero allocations after initialization
/// - O(1) access by entity index
/// - Cache-friendly iteration
///
/// # Type Parameters
///
/// * `C` - The component type to store
///
/// # Example
///
/// ```rust,ignore
/// let mut storage: ComponentStorage<Position> = ComponentStorage::new(1_000_000);
/// storage.set(0, Position::new(1.0, 2.0, 3.0));
/// ```
pub struct ComponentStorage<C: Component> {
    /// The dense array of components.
    data: Box<[C]>,
    /// Capacity (max entities).
    capacity: usize,
    /// Marker for component type.
    _phantom: PhantomData<C>,
}

impl<C: Component> ComponentStorage<C> {
    /// Creates new component storage with the specified capacity.
    ///
    /// All slots are initialized to the component's default value.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entities this storage can hold
    ///
    /// # Panics
    ///
    /// Panics if capacity is zero.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than zero");

        // Pre-allocate all memory upfront
        let data = vec![C::default(); capacity].into_boxed_slice();

        Self {
            data,
            capacity,
            _phantom: PhantomData,
        }
    }

    /// Returns the capacity of this storage.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Gets a component by entity index.
    ///
    /// # Arguments
    ///
    /// * `index` - The entity index (must be less than capacity)
    ///
    /// # Returns
    ///
    /// Reference to the component, or None if index is out of bounds.
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&C> {
        self.data.get(index)
    }

    /// Gets a mutable component by entity index.
    ///
    /// # Arguments
    ///
    /// * `index` - The entity index (must be less than capacity)
    ///
    /// # Returns
    ///
    /// Mutable reference to the component, or None if index is out of bounds.
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut C> {
        self.data.get_mut(index)
    }

    /// Sets a component at the specified index.
    ///
    /// This is a **zero-allocation** operation - it simply overwrites
    /// the existing pre-allocated slot.
    ///
    /// # Arguments
    ///
    /// * `index` - The entity index (must be less than capacity)
    /// * `component` - The component value to set
    ///
    /// # Returns
    ///
    /// `true` if the component was set, `false` if index was out of bounds.
    #[inline]
    pub fn set(&mut self, index: usize, component: C) -> bool {
        if let Some(slot) = self.data.get_mut(index) {
            *slot = component;
            true
        } else {
            false
        }
    }

    /// Returns a slice of all components.
    ///
    /// Useful for batch processing.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[C] {
        &self.data
    }

    /// Returns a mutable slice of all components.
    ///
    /// Useful for batch processing.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [C] {
        &mut self.data
    }

    /// Iterates over all components with their indices.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (usize, &C)> {
        self.data.iter().enumerate()
    }

    /// Iterates mutably over all components with their indices.
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut C)> {
        self.data.iter_mut().enumerate()
    }

    /// Resets a component slot to its default value.
    ///
    /// This is a **zero-allocation** operation.
    ///
    /// # Arguments
    ///
    /// * `index` - The entity index to reset
    #[inline]
    pub fn reset(&mut self, index: usize) {
        if let Some(slot) = self.data.get_mut(index) {
            *slot = C::default();
        }
    }

    /// Clears all components to their default values.
    ///
    /// This is a **zero-allocation** operation - no memory is freed or allocated.
    pub fn clear(&mut self) {
        for slot in self.data.iter_mut() {
            *slot = C::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::component::Position;

    #[test]
    fn test_storage_creation() {
        let storage: ComponentStorage<Position> = ComponentStorage::new(1000);
        assert_eq!(storage.capacity(), 1000);
    }

    #[test]
    fn test_storage_get_set() {
        let mut storage: ComponentStorage<Position> = ComponentStorage::new(100);

        let pos = Position::new(1.0, 2.0, 3.0);
        assert!(storage.set(50, pos));

        let retrieved = storage.get(50).unwrap();
        assert_eq!(*retrieved, pos);
    }

    #[test]
    fn test_storage_bounds() {
        let storage: ComponentStorage<Position> = ComponentStorage::new(100);
        assert!(storage.get(100).is_none());
        assert!(storage.get(99).is_some());
    }
}
