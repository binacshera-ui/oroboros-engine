//! # OROBOROS Event System
//!
//! Lock-free inter-unit communication for the OROBOROS game engine.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐      ┌─────────────┐      ┌─────────────┐
//! │   Unit 4    │─────>│   Event     │─────>│   Unit 2    │
//! │  (Logic)    │      │   Channel   │      │  (Render)   │
//! └─────────────┘      └─────────────┘      └─────────────┘
//!       │                    │                    │
//!       │              ┌─────┴─────┐              │
//!       └─────────────>│  Unit 3   │<────────────┘
//!                      │ (Economy) │
//!                      └───────────┘
//! ```
//!
//! Events flow FROM logic TO rendering and economy.
//! Uses crossbeam channels for zero-allocation in hot path.

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use oroboros_core::EntityId;

/// Events that flow between units.
///
/// These events are the "API" between units.
/// Each unit only processes events relevant to it.
#[derive(Clone, Debug)]
pub enum GameEvent {
    // =========================================================================
    // Block Events (Unit 4 → Unit 2, Unit 3)
    // =========================================================================
    /// A block was broken by a player.
    ///
    /// Emitted by: Unit 4 (after server validation)
    /// Consumed by: Unit 2 (particles), Unit 3 (loot)
    BlockBroken {
        /// Entity that broke the block.
        entity_id: EntityId,
        /// World position of the broken block.
        block_pos: [i32; 3],
        /// Block type that was broken.
        block_type: u32,
        /// Tool tier used.
        tool_tier: u8,
    },

    /// A block was placed by a player.
    BlockPlaced {
        /// Entity that placed the block.
        entity_id: EntityId,
        /// World position where block was placed.
        block_pos: [i32; 3],
        /// Block type that was placed.
        block_type: u32,
    },

    // =========================================================================
    // Loot Events (Unit 3 → Unit 2, Unit 4)
    // =========================================================================
    /// Loot was dropped from a block.
    ///
    /// Emitted by: Unit 3 (after WAL commit)
    /// Consumed by: Unit 2 (particle effect), Unit 4 (network broadcast)
    LootDropped {
        /// Entity that receives the loot.
        entity_id: EntityId,
        /// Position where loot dropped.
        position: [f32; 3],
        /// Item ID that dropped.
        item_id: u32,
        /// Quantity dropped.
        quantity: u32,
        /// Rarity (0=common, 1=uncommon, 2=rare, 3=epic, 4=legendary).
        rarity: u8,
    },

    /// Inventory changed for an entity.
    InventoryChanged {
        /// Entity whose inventory changed.
        entity_id: EntityId,
        /// Item that changed.
        item_id: u32,
        /// New total quantity (not delta).
        new_quantity: u32,
    },

    // =========================================================================
    // Entity Events (Unit 4 → Unit 1, Unit 2)
    // =========================================================================
    /// An entity spawned.
    EntitySpawned {
        /// Entity ID.
        entity_id: EntityId,
        /// Entity type.
        entity_type: u32,
        /// Initial position.
        position: [f32; 3],
    },

    /// An entity was despawned.
    EntityDespawned {
        /// Entity ID.
        entity_id: EntityId,
    },

    /// An entity took damage.
    EntityDamaged {
        /// Entity that took damage.
        entity_id: EntityId,
        /// Damage amount.
        damage: u32,
        /// Current health after damage.
        health_remaining: u32,
        /// Damage source position (for directional indicator).
        source_pos: [f32; 3],
    },

    /// An entity died.
    EntityDied {
        /// Entity that died.
        entity_id: EntityId,
        /// Position of death.
        position: [f32; 3],
        /// Entity that caused the death (if any).
        killer_id: Option<EntityId>,
    },

    // =========================================================================
    // Combat Events (Unit 4 → Unit 2)
    // =========================================================================
    /// An attack hit a target.
    AttackHit {
        /// Attacker entity.
        attacker_id: EntityId,
        /// Target entity.
        target_id: EntityId,
        /// Hit position.
        position: [f32; 3],
        /// Damage dealt.
        damage: u32,
        /// Is critical hit?
        is_critical: bool,
    },

    /// An attack missed.
    AttackMissed {
        /// Attacker entity.
        attacker_id: EntityId,
        /// Attack direction.
        direction: [f32; 3],
    },

    // =========================================================================
    // World Events (Unit 3 → Unit 2)
    // =========================================================================
    /// Dragon behavior changed (based on market).
    DragonStateChanged {
        /// Dragon entity.
        dragon_id: EntityId,
        /// New aggression level (0-100).
        aggression: u8,
        /// Market correlation (-1 to 1 as i8 * 100).
        market_correlation: i8,
    },

    /// Weather changed.
    WeatherChanged {
        /// New weather intensity (0-100).
        intensity: u8,
        /// Weather type (0=clear, 1=rain, 2=storm, 3=fog).
        weather_type: u8,
    },
}

/// Event bus for inter-unit communication.
///
/// Pre-allocates channels with bounded capacity to prevent
/// memory growth in the hot path.
pub struct EventBus {
    /// Sender end - held by event producers.
    sender: Sender<GameEvent>,
    /// Receiver end - held by event consumers.
    receiver: Receiver<GameEvent>,
}

impl EventBus {
    /// Creates a new event bus.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum events in flight before blocking.
    ///               Use 1024 for typical game loop.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        Self { sender, receiver }
    }

    /// Creates a sender handle (clone for multiple producers).
    #[must_use]
    pub fn sender(&self) -> EventSender {
        EventSender {
            sender: self.sender.clone(),
        }
    }

    /// Creates a receiver handle (clone for multiple consumers).
    #[must_use]
    pub fn receiver(&self) -> EventReceiver {
        EventReceiver {
            receiver: self.receiver.clone(),
        }
    }

    /// Creates a new pair of sender and receiver.
    ///
    /// Convenience method for creating paired handles.
    #[must_use]
    pub fn create_pair(capacity: usize) -> (EventSender, EventReceiver) {
        let bus = Self::new(capacity);
        (bus.sender(), bus.receiver())
    }
}

/// Handle for sending events.
#[derive(Clone)]
pub struct EventSender {
    sender: Sender<GameEvent>,
}

impl EventSender {
    /// Sends an event (non-blocking).
    ///
    /// Returns `false` if the channel is full (events will be dropped).
    /// In production, this should be logged as a performance warning.
    #[inline]
    pub fn send(&self, event: GameEvent) -> bool {
        match self.sender.try_send(event) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                // Channel full - this is a performance problem
                // In release, we drop the event to maintain frame rate
                false
            }
            Err(TrySendError::Disconnected(_)) => {
                // Receiver dropped - should not happen in normal operation
                false
            }
        }
    }

    /// Sends an event (blocking).
    ///
    /// Use only for critical events that MUST be delivered.
    #[inline]
    pub fn send_blocking(&self, event: GameEvent) -> bool {
        self.sender.send(event).is_ok()
    }
}

/// Handle for receiving events.
#[derive(Clone)]
pub struct EventReceiver {
    receiver: Receiver<GameEvent>,
}

impl EventReceiver {
    /// Receives all pending events (non-blocking).
    ///
    /// Returns a vector of events. Empty if no events pending.
    /// Use this in the render loop to process events without blocking.
    #[inline]
    pub fn drain(&self) -> Vec<GameEvent> {
        let mut events = Vec::with_capacity(64);
        while let Ok(event) = self.receiver.try_recv() {
            events.push(event);
        }
        events
    }

    /// Receives one event (non-blocking).
    ///
    /// Returns `None` if no events pending.
    #[inline]
    pub fn try_recv(&self) -> Option<GameEvent> {
        self.receiver.try_recv().ok()
    }

    /// Returns the number of pending events.
    #[inline]
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.receiver.len()
    }

    /// Checks if there are pending events.
    #[inline]
    #[must_use]
    pub fn has_events(&self) -> bool {
        !self.receiver.is_empty()
    }
}

/// Builder for the complete event system.
///
/// Creates all the channels needed for the 4-unit architecture.
pub struct EventSystemBuilder {
    /// Capacity for each channel.
    capacity: usize,
}

impl EventSystemBuilder {
    /// Creates a new builder with default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self { capacity: 1024 }
    }

    /// Sets the channel capacity.
    #[must_use]
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Builds the event system.
    #[must_use]
    pub fn build(self) -> EventSystem {
        // Main event bus: Logic → Render
        let logic_to_render = EventBus::new(self.capacity);

        // Economy event bus: Economy → Others
        let economy_to_others = EventBus::new(self.capacity);

        // Network event bus: Network → Logic (server-side)
        let network_to_logic = EventBus::new(self.capacity);

        EventSystem {
            // Logic (Unit 4) sends to Render (Unit 2)
            logic_sender: logic_to_render.sender(),
            render_receiver: logic_to_render.receiver(),

            // Economy (Unit 3) sends to others
            economy_sender: economy_to_others.sender(),
            economy_receiver: economy_to_others.receiver(),

            // Network sends to Logic (for client input on server)
            network_sender: network_to_logic.sender(),
            logic_receiver: network_to_logic.receiver(),
        }
    }
}

impl Default for EventSystemBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete event system for the game.
///
/// Contains all channels for inter-unit communication.
pub struct EventSystem {
    // Logic → Render
    /// Sender for logic thread to emit visual events.
    pub logic_sender: EventSender,
    /// Receiver for render thread to consume visual events.
    pub render_receiver: EventReceiver,

    // Economy → Others
    /// Sender for economy system to emit loot/inventory events.
    pub economy_sender: EventSender,
    /// Receiver for others to consume economy events.
    pub economy_receiver: EventReceiver,

    // Network → Logic
    /// Sender for network to emit input events.
    pub network_sender: EventSender,
    /// Receiver for logic to consume network events.
    pub logic_receiver: EventReceiver,
}

impl EventSystem {
    /// Creates a new event system with default settings.
    #[must_use]
    pub fn new() -> Self {
        EventSystemBuilder::new().build()
    }
}

impl Default for EventSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oroboros_core::EntityId;

    #[test]
    fn test_event_send_receive() {
        let bus = EventBus::new(100);
        let sender = bus.sender();
        let receiver = bus.receiver();

        let event = GameEvent::BlockBroken {
            entity_id: EntityId::new(1, 0),
            block_pos: [10, 20, 30],
            block_type: 5,
            tool_tier: 3,
        };

        assert!(sender.send(event));
        assert!(receiver.has_events());

        let received = receiver.try_recv().unwrap();
        if let GameEvent::BlockBroken { block_pos, .. } = received {
            assert_eq!(block_pos, [10, 20, 30]);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_event_drain() {
        let bus = EventBus::new(100);
        let sender = bus.sender();
        let receiver = bus.receiver();

        // Send multiple events
        for i in 0..10 {
            let _ = sender.send(GameEvent::EntitySpawned {
                entity_id: EntityId::new(i, 0),
                entity_type: 1,
                position: [0.0, 0.0, 0.0],
            });
        }

        let events = receiver.drain();
        assert_eq!(events.len(), 10);
        assert!(!receiver.has_events());
    }

    #[test]
    fn test_event_system_creation() {
        let system = EventSystem::new();

        // Logic can send
        let _ = system.logic_sender.send(GameEvent::AttackHit {
            attacker_id: EntityId::new(1, 0),
            target_id: EntityId::new(2, 0),
            position: [0.0, 0.0, 0.0],
            damage: 50,
            is_critical: true,
        });

        // Render can receive
        assert!(system.render_receiver.has_events());
    }
}
