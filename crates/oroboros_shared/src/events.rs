//! Shared event types between client and server.
//!
//! These events can be sent across the network and processed by both sides.
//! The CLIENT uses them for visual feedback.
//! The SERVER uses them for authoritative game logic.

use crate::math::Vec3;
use crate::protocol::{BlockUpdate, DamageEvent, ItemDrop, Rarity};
use serde::{Deserialize, Serialize};

/// Event type discriminator
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    /// Block broken
    BlockBroken = 0,
    /// Block placed
    BlockPlaced = 1,
    /// Item dropped
    ItemDropped = 2,
    /// Damage dealt
    DamageDealt = 3,
    /// Entity died
    EntityDied = 4,
    /// Player connected
    PlayerConnected = 5,
    /// Player disconnected
    PlayerDisconnected = 6,
    /// Transaction completed
    TransactionComplete = 7,
}

/// Events that can be shared between client and server
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SharedEvent {
    /// Block was broken
    BlockBroken {
        /// Block position
        position: Vec3,
        /// Block type that was broken
        block_type: u16,
        /// Player who broke it
        player_id: u32,
        /// Server tick
        tick: u32,
    },

    /// Block was placed
    BlockPlaced {
        /// Block data
        block: BlockUpdate,
    },

    /// Item was dropped (loot)
    ItemDropped {
        /// Drop data
        drop: ItemDrop,
        /// Rarity for visual effects
        rarity: Rarity,
    },

    /// Entity took damage
    DamageTaken {
        /// Damage data
        damage: DamageEvent,
    },

    /// Entity died
    EntityDied {
        /// Entity ID
        entity_id: u64,
        /// Position where they died
        position: Vec3,
        /// Killer ID (0 = environment)
        killer_id: u64,
        /// Was it a player?
        is_player: bool,
    },

    /// Transaction completed (trade, purchase)
    TransactionComplete {
        /// Player ID
        player_id: u32,
        /// Amount (negative = spent)
        amount: i64,
        /// Item involved (if any)
        item_id: Option<u32>,
    },
}

impl SharedEvent {
    /// Returns the event type
    #[must_use]
    pub const fn event_type(&self) -> EventType {
        match self {
            Self::BlockBroken { .. } => EventType::BlockBroken,
            Self::BlockPlaced { .. } => EventType::BlockPlaced,
            Self::ItemDropped { .. } => EventType::ItemDropped,
            Self::DamageTaken { .. } => EventType::DamageDealt,
            Self::EntityDied { .. } => EventType::EntityDied,
            Self::TransactionComplete { .. } => EventType::TransactionComplete,
        }
    }

    /// Returns the position where this event occurred (if applicable)
    #[must_use]
    pub fn position(&self) -> Option<Vec3> {
        match self {
            Self::BlockBroken { position, .. } => Some(*position),
            Self::BlockPlaced { block } => Some(Vec3::new(
                block.position[0] as f32,
                block.position[1] as f32,
                block.position[2] as f32,
            )),
            Self::ItemDropped { drop, .. } => Some(drop.position),
            Self::DamageTaken { damage } => Some(damage.position),
            Self::EntityDied { position, .. } => Some(*position),
            Self::TransactionComplete { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type() {
        let event = SharedEvent::BlockBroken {
            position: Vec3::ZERO,
            block_type: 1,
            player_id: 1,
            tick: 100,
        };
        assert_eq!(event.event_type(), EventType::BlockBroken);
    }

    #[test]
    fn test_event_position() {
        let event = SharedEvent::ItemDropped {
            drop: ItemDrop {
                position: Vec3::new(10.0, 20.0, 30.0),
                item_id: 1,
                rarity: 4,
                quantity: 1,
                _pad: [0; 2],
                player_id: 1,
                tick: 100,
            },
            rarity: Rarity::Legendary,
        };
        assert_eq!(event.position(), Some(Vec3::new(10.0, 20.0, 30.0)));
    }
}
