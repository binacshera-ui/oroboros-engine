//! # Inventory System
//!
//! Pre-allocated inventory slots for items.
//! Zero allocations during gameplay - all slots are pre-allocated.

use crate::error::{EconomyError, EconomyResult};
use crate::fixed_point::FixedPoint;

/// Unique identifier for an item type.
pub type ItemId = u32;

/// An item definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Unique identifier.
    pub id: ItemId,
    /// Maximum stack size for this item type.
    pub max_stack: u32,
    /// Base value in the economy (fixed-point).
    pub base_value: FixedPoint,
    /// Item flags (tradeable, consumable, etc.).
    pub flags: ItemFlags,
}

/// Flags for item properties.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemFlags(u32);

impl ItemFlags {
    /// No flags set.
    pub const NONE: Self = Self(0);
    /// Item can be traded between players.
    pub const TRADEABLE: Self = Self(1 << 0);
    /// Item is consumed on use.
    pub const CONSUMABLE: Self = Self(1 << 1);
    /// Item is a crafting material.
    pub const MATERIAL: Self = Self(1 << 2);
    /// Item is equipment.
    pub const EQUIPMENT: Self = Self(1 << 3);
    /// Item is bound to player (non-transferable).
    pub const SOULBOUND: Self = Self(1 << 4);

    /// Creates flags from raw value.
    #[inline]
    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw value.
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Checks if a specific flag is set.
    #[inline]
    #[must_use]
    pub const fn has(self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }

    /// Combines two flag sets.
    #[inline]
    #[must_use]
    pub const fn with(self, flag: Self) -> Self {
        Self(self.0 | flag.0)
    }
}

/// A stack of items in an inventory slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemStack {
    /// The item type ID, or 0 for empty slot.
    pub item_id: ItemId,
    /// Number of items in this stack.
    pub count: u32,
}

impl ItemStack {
    /// Creates an empty item stack.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            item_id: 0,
            count: 0,
        }
    }

    /// Creates a new item stack.
    #[inline]
    #[must_use]
    pub const fn new(item_id: ItemId, count: u32) -> Self {
        Self { item_id, count }
    }

    /// Returns true if this slot is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0 || self.item_id == 0
    }

    /// Clears this slot.
    #[inline]
    pub fn clear(&mut self) {
        self.item_id = 0;
        self.count = 0;
    }
}

/// Maximum inventory slots.
pub const MAX_INVENTORY_SLOTS: usize = 64;

/// A pre-allocated inventory.
///
/// All slots are allocated at creation time.
/// No allocations occur during add/remove operations.
#[derive(Clone, Debug)]
pub struct Inventory {
    /// Pre-allocated slots.
    slots: [ItemStack; MAX_INVENTORY_SLOTS],
    /// Number of slots currently in use.
    used_slots: u32,
}

impl Inventory {
    /// Creates a new empty inventory with pre-allocated slots.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: [ItemStack::empty(); MAX_INVENTORY_SLOTS],
            used_slots: 0,
        }
    }

    /// Returns the number of used slots.
    #[inline]
    #[must_use]
    pub const fn used_slots(&self) -> u32 {
        self.used_slots
    }

    /// Returns the total capacity.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        MAX_INVENTORY_SLOTS
    }

    /// Checks if the inventory is full.
    #[inline]
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.used_slots as usize >= MAX_INVENTORY_SLOTS
    }

    /// Gets an item stack at a specific slot.
    ///
    /// # Arguments
    ///
    /// * `slot` - The slot index (0-63)
    #[inline]
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<&ItemStack> {
        self.slots.get(slot)
    }

    /// Gets a mutable reference to an item stack.
    #[inline]
    pub fn get_mut(&mut self, slot: usize) -> Option<&mut ItemStack> {
        self.slots.get_mut(slot)
    }

    /// Counts the total number of a specific item across all slots.
    ///
    /// # Arguments
    ///
    /// * `item_id` - The item type to count
    #[must_use]
    pub fn count_item(&self, item_id: ItemId) -> u32 {
        self.slots
            .iter()
            .filter(|s| s.item_id == item_id)
            .map(|s| s.count)
            .sum()
    }

    /// Finds the first slot containing a specific item.
    #[must_use]
    pub fn find_item(&self, item_id: ItemId) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| s.item_id == item_id && s.count > 0)
    }

    /// Finds the first empty slot.
    #[must_use]
    pub fn find_empty_slot(&self) -> Option<usize> {
        self.slots.iter().position(ItemStack::is_empty)
    }

    /// Adds items to the inventory.
    ///
    /// First tries to stack with existing items, then uses empty slots.
    ///
    /// # Arguments
    ///
    /// * `item_id` - The item type to add
    /// * `count` - Number of items to add
    /// * `max_stack` - Maximum stack size for this item type
    ///
    /// # Errors
    ///
    /// Returns `EconomyError::InventoryFull` if there's no space.
    pub fn add(&mut self, item_id: ItemId, count: u32, max_stack: u32) -> EconomyResult<()> {
        let mut remaining = count;

        // First, try to add to existing stacks
        for slot in &mut self.slots {
            if remaining == 0 {
                break;
            }

            if slot.item_id == item_id && slot.count < max_stack {
                let can_add = (max_stack - slot.count).min(remaining);
                slot.count += can_add;
                remaining -= can_add;
            }
        }

        // Then, use empty slots
        while remaining > 0 {
            if let Some(slot_idx) = self.find_empty_slot() {
                let add_count = remaining.min(max_stack);
                self.slots[slot_idx] = ItemStack::new(item_id, add_count);
                self.used_slots += 1;
                remaining -= add_count;
            } else {
                return Err(EconomyError::InventoryFull {
                    capacity: MAX_INVENTORY_SLOTS as u32,
                    amount: remaining,
                });
            }
        }

        Ok(())
    }

    /// Removes items from the inventory.
    ///
    /// # Arguments
    ///
    /// * `item_id` - The item type to remove
    /// * `count` - Number of items to remove
    ///
    /// # Errors
    ///
    /// Returns `EconomyError::InsufficientMaterials` if not enough items.
    pub fn remove(&mut self, item_id: ItemId, count: u32) -> EconomyResult<()> {
        let available = self.count_item(item_id);
        if available < count {
            return Err(EconomyError::InsufficientMaterials {
                item_id,
                required: count,
                available,
            });
        }

        let mut remaining = count;

        for slot in &mut self.slots {
            if remaining == 0 {
                break;
            }

            if slot.item_id == item_id {
                let remove_count = slot.count.min(remaining);
                slot.count -= remove_count;
                remaining -= remove_count;

                if slot.count == 0 {
                    slot.clear();
                    self.used_slots = self.used_slots.saturating_sub(1);
                }
            }
        }

        Ok(())
    }

    /// Creates a snapshot of the inventory for rollback.
    #[must_use]
    pub fn snapshot(&self) -> InventorySnapshot {
        InventorySnapshot {
            slots: self.slots,
            used_slots: self.used_slots,
        }
    }

    /// Restores inventory from a snapshot (rollback).
    pub fn restore(&mut self, snapshot: &InventorySnapshot) {
        self.slots = snapshot.slots;
        self.used_slots = snapshot.used_slots;
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of inventory state for transactional rollback.
#[derive(Clone, Debug)]
pub struct InventorySnapshot {
    slots: [ItemStack; MAX_INVENTORY_SLOTS],
    used_slots: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_items() {
        let mut inv = Inventory::new();
        inv.add(1, 10, 64).unwrap();
        assert_eq!(inv.count_item(1), 10);
        assert_eq!(inv.used_slots(), 1);
    }

    #[test]
    fn test_add_stacking() {
        let mut inv = Inventory::new();
        inv.add(1, 64, 64).unwrap();
        inv.add(1, 10, 64).unwrap();
        assert_eq!(inv.count_item(1), 74);
        assert_eq!(inv.used_slots(), 2);
    }

    #[test]
    fn test_remove_items() {
        let mut inv = Inventory::new();
        inv.add(1, 100, 64).unwrap();
        inv.remove(1, 30).unwrap();
        assert_eq!(inv.count_item(1), 70);
    }

    #[test]
    fn test_remove_insufficient() {
        let mut inv = Inventory::new();
        inv.add(1, 10, 64).unwrap();
        let result = inv.remove(1, 20);
        assert!(matches!(result, Err(EconomyError::InsufficientMaterials { .. })));
    }

    #[test]
    fn test_snapshot_restore() {
        let mut inv = Inventory::new();
        inv.add(1, 50, 64).unwrap();

        let snapshot = inv.snapshot();

        inv.add(2, 30, 64).unwrap();
        assert_eq!(inv.count_item(2), 30);

        inv.restore(&snapshot);
        assert_eq!(inv.count_item(2), 0);
        assert_eq!(inv.count_item(1), 50);
    }
}
