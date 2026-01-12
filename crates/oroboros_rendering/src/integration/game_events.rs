//! Game Event System - Receives events from Unit 3 and Unit 4
//!
//! Events trigger visual effects without Unit 2 touching game logic.
//!
//! ## Event Flow
//!
//! ```text
//! Unit 4 (Network)  ──► BlockBroken ──┐
//! Unit 3 (Economy)  ──► ItemDrop    ──┼──► GameEventQueue ──► EventVisualizer
//! Unit 4 (Network)  ──► Damage      ──┤                           │
//! Unit 4 (Network)  ──► Death       ──┘                           ▼
//!                                                          ParticleSystem
//! ```

use std::collections::VecDeque;

/// Maximum events to queue per frame
const MAX_EVENTS_PER_FRAME: usize = 256;

/// Category of game event (for filtering/prioritization)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventCategory {
    /// Block/terrain events
    Terrain = 0,
    /// Item/loot events
    Items = 1,
    /// Combat events
    Combat = 2,
    /// Player state events
    Player = 3,
    /// Economy/transaction events
    Economy = 4,
    /// Network sync events
    Network = 5,
}

/// Block break event (from Unit 4)
#[derive(Debug, Clone, Copy)]
pub struct BlockBreakEvent {
    /// World position of the broken block
    pub position: [f32; 3],
    /// Block type that was broken
    pub block_type: u16,
    /// Player who broke it (for multi-color particles)
    pub player_id: u32,
    /// Timestamp (server tick)
    pub tick: u32,
}

/// Item drop event (from Unit 3 via Unit 4)
#[derive(Debug, Clone, Copy)]
pub struct ItemDropEvent {
    /// World position where item dropped
    pub position: [f32; 3],
    /// Item ID
    pub item_id: u32,
    /// Item rarity (0-5, maps to Rarity enum)
    pub rarity: u8,
    /// Quantity dropped
    pub quantity: u32,
    /// Player who received the drop
    pub player_id: u32,
}

/// Damage event (from Unit 4)
#[derive(Debug, Clone, Copy)]
pub struct DamageEvent {
    /// Position of the hit
    pub position: [f32; 3],
    /// Direction of the hit (for directional effects)
    pub direction: [f32; 3],
    /// Damage amount
    pub damage: u32,
    /// Is critical hit?
    pub is_critical: bool,
    /// Damage type (0=physical, 1=fire, 2=ice, etc)
    pub damage_type: u8,
    /// Entity that was hit
    pub target_id: u64,
    /// Entity that dealt damage (0 = environment)
    pub source_id: u64,
}

/// Death event (from Unit 4)
#[derive(Debug, Clone, Copy)]
pub struct DeathEvent {
    /// Position where entity died
    pub position: [f32; 3],
    /// Entity that died
    pub entity_id: u64,
    /// Was it a player?
    pub is_player: bool,
    /// Killer ID (0 = environment)
    pub killer_id: u64,
}

/// Transaction event (from Unit 3)
#[derive(Debug, Clone, Copy)]
pub struct TransactionEvent {
    /// Screen position for UI effect
    pub screen_pos: [f32; 2],
    /// Amount (negative = loss)
    pub amount: i64,
    /// Transaction type
    pub transaction_type: u8,
}

/// All possible game events
#[derive(Debug, Clone, Copy)]
pub enum GameEvent {
    /// Block was broken
    BlockBreak(BlockBreakEvent),
    /// Item dropped
    ItemDrop(ItemDropEvent),
    /// Entity took damage
    Damage(DamageEvent),
    /// Entity died
    Death(DeathEvent),
    /// Transaction completed
    Transaction(TransactionEvent),
}

impl GameEvent {
    /// Returns the category of this event
    #[must_use]
    pub const fn category(&self) -> EventCategory {
        match self {
            Self::BlockBreak(_) => EventCategory::Terrain,
            Self::ItemDrop(_) => EventCategory::Items,
            Self::Damage(_) => EventCategory::Combat,
            Self::Death(_) => EventCategory::Combat,
            Self::Transaction(_) => EventCategory::Economy,
        }
    }

    /// Returns the world position of this event (if applicable)
    #[must_use]
    pub fn position(&self) -> Option<[f32; 3]> {
        match self {
            Self::BlockBreak(e) => Some(e.position),
            Self::ItemDrop(e) => Some(e.position),
            Self::Damage(e) => Some(e.position),
            Self::Death(e) => Some(e.position),
            Self::Transaction(_) => None,
        }
    }
}

/// Queue for receiving game events from other units
///
/// Thread-safe for cross-unit communication.
/// Events are consumed each frame by the render loop.
pub struct GameEventQueue {
    /// Pending events
    events: VecDeque<GameEvent>,
    /// Statistics
    stats: EventQueueStats,
    /// Event counts by category (for throttling)
    category_counts: [u32; 6],
}

/// Statistics from the event queue
#[derive(Debug, Clone, Copy, Default)]
pub struct EventQueueStats {
    /// Events received this frame
    pub received: u32,
    /// Events processed this frame
    pub processed: u32,
    /// Events dropped (overflow)
    pub dropped: u32,
}

impl GameEventQueue {
    /// Creates a new event queue
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(MAX_EVENTS_PER_FRAME),
            stats: EventQueueStats::default(),
            category_counts: [0; 6],
        }
    }

    /// Pushes an event to the queue
    ///
    /// Returns false if the queue is full (event dropped).
    pub fn push(&mut self, event: GameEvent) -> bool {
        if self.events.len() >= MAX_EVENTS_PER_FRAME {
            self.stats.dropped += 1;
            return false;
        }

        let cat = event.category() as usize;
        self.category_counts[cat] += 1;
        self.events.push_back(event);
        self.stats.received += 1;
        true
    }

    /// Pushes a block break event
    pub fn push_block_break(&mut self, position: [f32; 3], block_type: u16, player_id: u32, tick: u32) {
        self.push(GameEvent::BlockBreak(BlockBreakEvent {
            position,
            block_type,
            player_id,
            tick,
        }));
    }

    /// Pushes an item drop event
    pub fn push_item_drop(
        &mut self,
        position: [f32; 3],
        item_id: u32,
        rarity: u8,
        quantity: u32,
        player_id: u32,
    ) {
        self.push(GameEvent::ItemDrop(ItemDropEvent {
            position,
            item_id,
            rarity,
            quantity,
            player_id,
        }));
    }

    /// Pushes a damage event
    pub fn push_damage(
        &mut self,
        position: [f32; 3],
        direction: [f32; 3],
        damage: u32,
        is_critical: bool,
        target_id: u64,
        source_id: u64,
    ) {
        self.push(GameEvent::Damage(DamageEvent {
            position,
            direction,
            damage,
            is_critical,
            damage_type: 0,
            target_id,
            source_id,
        }));
    }

    /// Pushes a death event
    pub fn push_death(&mut self, position: [f32; 3], entity_id: u64, is_player: bool, killer_id: u64) {
        self.push(GameEvent::Death(DeathEvent {
            position,
            entity_id,
            is_player,
            killer_id,
        }));
    }

    /// Drains all events for processing
    ///
    /// Call this at the start of each render frame.
    pub fn drain(&mut self) -> impl Iterator<Item = GameEvent> + '_ {
        self.stats.processed = self.events.len() as u32;
        self.events.drain(..)
    }

    /// Returns the number of pending events
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns true if the queue is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns statistics
    #[must_use]
    pub fn stats(&self) -> EventQueueStats {
        self.stats
    }

    /// Resets statistics for a new frame
    pub fn reset_stats(&mut self) {
        self.stats = EventQueueStats::default();
        self.category_counts = [0; 6];
    }

    /// Returns event count for a category
    #[must_use]
    pub fn count_for_category(&self, category: EventCategory) -> u32 {
        self.category_counts[category as usize]
    }
}

impl Default for GameEventQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_queue() {
        let mut queue = GameEventQueue::new();

        queue.push_block_break([0.0, 0.0, 0.0], 1, 100, 1);
        queue.push_item_drop([1.0, 2.0, 3.0], 500, 4, 1, 100);
        queue.push_damage([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 50, true, 1, 2);

        assert_eq!(queue.len(), 3);

        let events: Vec<_> = queue.drain().collect();
        assert_eq!(events.len(), 3);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_event_categories() {
        let block = GameEvent::BlockBreak(BlockBreakEvent {
            position: [0.0; 3],
            block_type: 1,
            player_id: 1,
            tick: 1,
        });
        assert_eq!(block.category(), EventCategory::Terrain);

        let damage = GameEvent::Damage(DamageEvent {
            position: [0.0; 3],
            direction: [0.0; 3],
            damage: 10,
            is_critical: false,
            damage_type: 0,
            target_id: 1,
            source_id: 2,
        });
        assert_eq!(damage.category(), EventCategory::Combat);
    }
}
