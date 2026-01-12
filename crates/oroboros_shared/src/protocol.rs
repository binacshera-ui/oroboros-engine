//! Network protocol types shared between client and server.
//!
//! These types are serialized and sent over the network.
//! Both client and server must agree on these definitions.

use crate::math::Vec3;
use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

/// Packet type identifier
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacketType {
    /// Client sends input to server
    PlayerInput = 0,
    /// Server sends entity updates to client
    EntityUpdate = 1,
    /// Server sends block changes
    BlockUpdate = 2,
    /// Server sends item drop event
    ItemDrop = 3,
    /// Server sends damage event
    DamageEvent = 4,
    /// Server sends death event
    DeathEvent = 5,
    /// Server sends snapshot
    WorldSnapshot = 6,
    /// Client sends acknowledgment
    Ack = 7,
    /// Server sends tick sync
    TickSync = 8,
}

/// Entity update packet
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, Serialize, Deserialize)]
pub struct EntityUpdate {
    /// Entity ID
    pub entity_id: u64,
    /// Position
    pub position: Vec3,
    /// Velocity
    pub velocity: Vec3,
    /// Rotation (yaw, pitch in radians)
    pub rotation: [f32; 2],
    /// Server tick when this was generated
    pub tick: u32,
    /// Entity type
    pub entity_type: u16,
    /// Flags (alive, grounded, etc)
    pub flags: u16,
}

/// Block update packet
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, Serialize, Deserialize)]
pub struct BlockUpdate {
    /// Block position (chunk-relative)
    pub position: [i32; 3],
    /// Chunk coordinates
    pub chunk: [i32; 3],
    /// New block type (0 = air)
    pub block_type: u16,
    /// Metadata
    pub metadata: u16,
    /// Player who caused the change
    pub player_id: u32,
    /// Server tick
    pub tick: u32,
}

/// Item drop packet (from server to client)
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, Serialize, Deserialize)]
pub struct ItemDrop {
    /// World position
    pub position: Vec3,
    /// Item ID
    pub item_id: u32,
    /// Player who gets the item
    pub player_id: u32,
    /// Server tick
    pub tick: u32,
    /// Item rarity (0-5)
    pub rarity: u8,
    /// Quantity
    pub quantity: u8,
    /// Padding
    pub _pad: [u8; 2],
}

/// Damage event packet
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, Serialize, Deserialize)]
pub struct DamageEvent {
    /// Impact position
    pub position: Vec3,
    /// Damage direction
    pub direction: Vec3,
    /// Target entity
    pub target_id: u64,
    /// Source entity (0 = environment)
    pub source_id: u64,
    /// Damage amount
    pub damage: u32,
    /// Server tick
    pub tick: u32,
    /// Damage type
    pub damage_type: u8,
    /// Is critical hit?
    pub is_critical: u8,
    /// Padding
    pub _pad: [u8; 6],
}

/// Rarity levels (matches economy crate)
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rarity {
    /// Common (gray)
    Common = 0,
    /// Uncommon (green)
    Uncommon = 1,
    /// Rare (blue)
    Rare = 2,
    /// Epic (purple)
    Epic = 3,
    /// Legendary (gold)
    Legendary = 4,
    /// Mythic (red)
    Mythic = 5,
}

impl Default for Rarity {
    fn default() -> Self {
        Self::Common
    }
}

impl Rarity {
    /// Converts from u8
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Common,
            1 => Self::Uncommon,
            2 => Self::Rare,
            3 => Self::Epic,
            4 => Self::Legendary,
            _ => Self::Mythic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_update_size() {
        // Ensure fixed size for network protocol
        assert_eq!(std::mem::size_of::<EntityUpdate>(), 48);
    }

    #[test]
    fn test_block_update_size() {
        assert_eq!(std::mem::size_of::<BlockUpdate>(), 36);
    }

    #[test]
    fn test_item_drop_size() {
        assert_eq!(std::mem::size_of::<ItemDrop>(), 28);
    }
}
