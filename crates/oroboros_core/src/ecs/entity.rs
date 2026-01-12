//! # Entity Management
//!
//! Entities are lightweight identifiers consisting of:
//! - An index into component arrays
//! - A generation counter for safe reuse

/// Unique identifier for an entity.
///
/// The ID is split into two parts:
/// - Lower 32 bits: Index into component arrays
/// - Upper 32 bits: Generation counter for detecting stale references
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct EntityId(u64);

impl EntityId {
    /// Creates a new entity ID from index and generation.
    ///
    /// # Arguments
    ///
    /// * `index` - The index into component arrays (0 to 2^32-1)
    /// * `generation` - The generation counter (0 to 2^32-1)
    #[inline]
    #[must_use]
    pub const fn new(index: u32, generation: u32) -> Self {
        Self(((generation as u64) << 32) | (index as u64))
    }

    /// Returns the index portion of the entity ID.
    #[inline]
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0 as u32
    }

    /// Returns the generation portion of the entity ID.
    #[inline]
    #[must_use]
    pub const fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Null/invalid entity ID.
    pub const NULL: Self = Self(u64::MAX);

    /// Checks if this entity ID is null/invalid.
    #[inline]
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == u64::MAX
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::NULL
    }
}

/// Entity with its components' validity flags.
///
/// This is the main entity type used in the ECS.
/// It tracks which components are attached via a bitmask.
#[derive(Clone, Copy, Debug)]
pub struct Entity {
    /// The unique identifier for this entity.
    pub id: EntityId,
    /// Bitmask of attached components (up to 64 component types).
    pub component_mask: u64,
    /// Whether this entity slot is currently alive.
    pub alive: bool,
}

impl Entity {
    /// Creates a new entity.
    #[inline]
    #[must_use]
    pub const fn new(id: EntityId) -> Self {
        Self {
            id,
            component_mask: 0,
            alive: true,
        }
    }

    /// Creates a dead/empty entity slot.
    #[inline]
    #[must_use]
    pub const fn dead() -> Self {
        Self {
            id: EntityId::NULL,
            component_mask: 0,
            alive: false,
        }
    }

    /// Checks if this entity has a specific component.
    ///
    /// # Arguments
    ///
    /// * `component_id` - The component type ID (0-63)
    #[inline]
    #[must_use]
    pub const fn has_component(self, component_id: u8) -> bool {
        (self.component_mask & (1 << component_id)) != 0
    }

    /// Adds a component flag to this entity.
    ///
    /// # Arguments
    ///
    /// * `component_id` - The component type ID (0-63)
    #[inline]
    pub fn add_component(&mut self, component_id: u8) {
        self.component_mask |= 1 << component_id;
    }

    /// Removes a component flag from this entity.
    ///
    /// # Arguments
    ///
    /// * `component_id` - The component type ID (0-63)
    #[inline]
    pub fn remove_component(&mut self, component_id: u8) {
        self.component_mask &= !(1 << component_id);
    }
}

impl Default for Entity {
    fn default() -> Self {
        Self::dead()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_id_roundtrip() {
        let id = EntityId::new(12345, 67890);
        assert_eq!(id.index(), 12345);
        assert_eq!(id.generation(), 67890);
    }

    #[test]
    fn test_entity_component_mask() {
        let mut entity = Entity::new(EntityId::new(0, 0));
        assert!(!entity.has_component(5));

        entity.add_component(5);
        assert!(entity.has_component(5));

        entity.remove_component(5);
        assert!(!entity.has_component(5));
    }
}
