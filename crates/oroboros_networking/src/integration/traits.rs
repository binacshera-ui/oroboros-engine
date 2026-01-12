//! # Integration Traits
//!
//! Traits that other units must implement to integrate with Unit 4's game loop.
//!
//! ## Architecture (Glass Walls Policy)
//!
//! Unit 4 DOES NOT modify code in other units.
//! Instead, we define traits here that other units implement.
//!
//! ```text
//! Unit 4 defines:    Unit X implements:
//! ┌─────────────┐    ┌─────────────┐
//! │ trait Foo   │ ←─ │ impl Foo    │
//! └─────────────┘    └─────────────┘
//! ```

use crate::integration::events::*;
use oroboros_core::Position;

// ============================================================================
// UNIT 1 (CORE) - Memory Owner Interface
// ============================================================================

/// Interface to Unit 1's ECS memory system.
///
/// Unit 1 implements this trait to allow Unit 4 to write game state.
/// All writes go through this interface to maintain the "Glass Walls" policy.
pub trait MemoryOwner: Send + Sync {
    /// Updates an entity's position in the ECS.
    fn update_position(&mut self, entity_id: EntityId, position: Position);
    
    /// Updates an entity's velocity.
    fn update_velocity(&mut self, entity_id: EntityId, velocity: (f32, f32, f32));
    
    /// Updates an entity's health.
    fn update_health(&mut self, entity_id: EntityId, health: u32);
    
    /// Updates a player's inventory slot.
    fn update_inventory(&mut self, player_id: PlayerId, slot: u8, item: Option<InventoryItem>);
    
    /// Spawns a new entity, returns the assigned EntityId.
    fn spawn_entity(&mut self, entity_type: EntityType, position: Position) -> EntityId;
    
    /// Despawns an entity.
    fn despawn_entity(&mut self, entity_id: EntityId);
    
    /// Updates a block in the world.
    fn update_block(&mut self, position: (i32, i32, i32), block_type: BlockId);
    
    /// Gets an entity's position (read).
    fn get_position(&self, entity_id: EntityId) -> Option<Position>;
    
    /// Gets an entity's health (read).
    fn get_health(&self, entity_id: EntityId) -> Option<u32>;
    
    /// Gets a player's inventory (read).
    fn get_inventory(&self, player_id: PlayerId) -> Option<Vec<Option<InventoryItem>>>;
    
    /// Gets a block type at position (read).
    fn get_block(&self, position: (i32, i32, i32)) -> BlockId;
    
    /// Swaps the double buffers (end of frame).
    fn swap_buffers(&mut self);
    
    /// Marks entity as dirty for network sync.
    fn mark_dirty(&mut self, entity_id: EntityId);
    
    /// Gets all dirty entities for this frame.
    fn get_dirty_entities(&self) -> Vec<EntityId>;
    
    /// Clears dirty flags after sync.
    fn clear_dirty(&mut self);
}

// ============================================================================
// UNIT 3 (ECONOMY) - Auditor Interface
// ============================================================================

/// Interface to Unit 3's economy system.
///
/// Unit 3 implements this trait to handle all economic calculations.
/// Unit 4 NEVER calculates loot or damage directly - always calls Unit 3.
pub trait EconomyAuditor: Send + Sync {
    /// Called when a block is about to be broken.
    /// Returns the loot that should drop.
    ///
    /// # Arguments
    /// * `player_id` - Who's breaking the block
    /// * `position` - Block position
    /// * `block_type` - Type of block being broken
    /// * `tool_id` - Tool being used (affects drop rates)
    ///
    /// # Returns
    /// * `EconomyResponse::BlockBroken` with loot list and transaction ID
    fn on_block_break(
        &mut self,
        player_id: PlayerId,
        position: (i32, i32, i32),
        block_type: BlockId,
        tool_id: Option<ItemId>,
    ) -> EconomyResponse;
    
    /// Called when damage is about to be dealt.
    /// Calculates final damage with all modifiers.
    ///
    /// # Arguments
    /// * `attacker_id` - Who's attacking
    /// * `defender_id` - Who's being attacked
    /// * `base_damage` - Base damage before modifiers
    /// * `attack_type` - Type of attack
    ///
    /// # Returns
    /// * `EconomyResponse::DamageCalculated` with final damage and effects
    fn calculate_damage(
        &mut self,
        attacker_id: EntityId,
        defender_id: EntityId,
        base_damage: u32,
        attack_type: AttackType,
    ) -> EconomyResponse;
    
    /// Called when an entity dies.
    /// Calculates death loot.
    ///
    /// # Arguments
    /// * `entity_id` - Who died
    /// * `killer_id` - Who killed them (None for environmental)
    ///
    /// # Returns
    /// * Loot drops from the death
    fn on_entity_death(
        &mut self,
        entity_id: EntityId,
        killer_id: Option<EntityId>,
    ) -> Vec<LootDrop>;
    
    /// Called when player picks up a dropped item.
    /// Validates and processes the pickup.
    ///
    /// # Returns
    /// * `true` if pickup was valid and processed
    fn on_item_pickup(
        &mut self,
        player_id: PlayerId,
        drop_id: u64,
    ) -> bool;
    
    /// Gets the current Dragon aggression modifier based on market.
    /// This affects dragon damage calculations.
    fn get_dragon_modifier(&self) -> f32;
    
    /// Called at start of frame to sync with market data.
    fn tick(&mut self);
}

// ============================================================================
// UNIT 2 (RENDER) - Visual Feedback Interface
// ============================================================================

/// Interface to Unit 2's rendering system.
///
/// Unit 2 implements this trait to receive visual feedback events.
/// Unit 4 calls these when game events need visual representation.
pub trait VisualFeedback: Send + Sync {
    /// Spawn particles at a position.
    fn spawn_particles(
        &mut self,
        particle_type: ParticleType,
        position: Position,
        count: u32,
        color: [u8; 4],
    );
    
    /// Show floating text (damage numbers, loot notifications).
    fn show_floating_text(
        &mut self,
        text: &str,
        position: Position,
        color: [u8; 4],
        duration_ms: u32,
    );
    
    /// Play a sound effect.
    fn play_sound(&mut self, sound_id: u32, position: Position, volume: f32);
    
    /// Trigger screen shake.
    fn screen_shake(&mut self, intensity: f32, duration_ms: u32);
    
    /// Update a UI element.
    fn update_ui(&mut self, element: UIElement, value: &str);
    
    /// Queue a render event for batch processing.
    fn queue_event(&mut self, event: RenderEvent);
    
    /// Process all queued events (called at end of frame).
    fn flush_events(&mut self);
}

// ============================================================================
// MOCK IMPLEMENTATIONS (For Testing)
// ============================================================================

/// Mock implementation of MemoryOwner for testing.
pub struct MockMemoryOwner {
    positions: std::collections::HashMap<EntityId, Position>,
    health: std::collections::HashMap<EntityId, u32>,
    blocks: std::collections::HashMap<(i32, i32, i32), BlockId>,
    next_entity_id: EntityId,
    dirty: Vec<EntityId>,
}

impl MockMemoryOwner {
    /// Creates a new mock memory owner.
    pub fn new() -> Self {
        Self {
            positions: std::collections::HashMap::new(),
            health: std::collections::HashMap::new(),
            blocks: std::collections::HashMap::new(),
            next_entity_id: 1,
            dirty: Vec::new(),
        }
    }
}

impl Default for MockMemoryOwner {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryOwner for MockMemoryOwner {
    fn update_position(&mut self, entity_id: EntityId, position: Position) {
        self.positions.insert(entity_id, position);
        self.mark_dirty(entity_id);
    }
    
    fn update_velocity(&mut self, _entity_id: EntityId, _velocity: (f32, f32, f32)) {
        // Mock: no-op
    }
    
    fn update_health(&mut self, entity_id: EntityId, health: u32) {
        self.health.insert(entity_id, health);
        self.mark_dirty(entity_id);
    }
    
    fn update_inventory(&mut self, _player_id: PlayerId, _slot: u8, _item: Option<InventoryItem>) {
        // Mock: no-op
    }
    
    fn spawn_entity(&mut self, _entity_type: EntityType, position: Position) -> EntityId {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        self.positions.insert(id, position);
        self.health.insert(id, 100);
        id
    }
    
    fn despawn_entity(&mut self, entity_id: EntityId) {
        self.positions.remove(&entity_id);
        self.health.remove(&entity_id);
    }
    
    fn update_block(&mut self, position: (i32, i32, i32), block_type: BlockId) {
        self.blocks.insert(position, block_type);
    }
    
    fn get_position(&self, entity_id: EntityId) -> Option<Position> {
        self.positions.get(&entity_id).copied()
    }
    
    fn get_health(&self, entity_id: EntityId) -> Option<u32> {
        self.health.get(&entity_id).copied()
    }
    
    fn get_inventory(&self, _player_id: PlayerId) -> Option<Vec<Option<InventoryItem>>> {
        Some(vec![None; 36]) // Mock: empty 36-slot inventory
    }
    
    fn get_block(&self, position: (i32, i32, i32)) -> BlockId {
        *self.blocks.get(&position).unwrap_or(&0)
    }
    
    fn swap_buffers(&mut self) {
        // Mock: no-op (no double buffer in mock)
    }
    
    fn mark_dirty(&mut self, entity_id: EntityId) {
        if !self.dirty.contains(&entity_id) {
            self.dirty.push(entity_id);
        }
    }
    
    fn get_dirty_entities(&self) -> Vec<EntityId> {
        self.dirty.clone()
    }
    
    fn clear_dirty(&mut self) {
        self.dirty.clear();
    }
}

/// Mock implementation of EconomyAuditor for testing.
pub struct MockEconomyAuditor {
    transaction_counter: u64,
    dragon_modifier: f32,
}

impl MockEconomyAuditor {
    /// Creates a new mock economy auditor.
    pub fn new() -> Self {
        Self {
            transaction_counter: 0,
            dragon_modifier: 1.0,
        }
    }
}

impl Default for MockEconomyAuditor {
    fn default() -> Self {
        Self::new()
    }
}

impl EconomyAuditor for MockEconomyAuditor {
    fn on_block_break(
        &mut self,
        _player_id: PlayerId,
        position: (i32, i32, i32),
        block_type: BlockId,
        _tool_id: Option<ItemId>,
    ) -> EconomyResponse {
        self.transaction_counter += 1;
        
        // Mock loot: diamond block (type 1) drops diamond item (type 1)
        let loot = if block_type == 1 {
            vec![LootDrop {
                item_id: 1, // Diamond
                quantity: 1,
                position: Position::new(position.0 as f32, position.1 as f32, position.2 as f32),
                drop_id: self.transaction_counter,
            }]
        } else {
            vec![]
        };
        
        EconomyResponse::BlockBroken {
            success: true,
            loot,
            experience: 10,
            transaction_id: self.transaction_counter,
        }
    }
    
    fn calculate_damage(
        &mut self,
        _attacker_id: EntityId,
        _defender_id: EntityId,
        base_damage: u32,
        _attack_type: AttackType,
    ) -> EconomyResponse {
        EconomyResponse::DamageCalculated {
            final_damage: base_damage,
            critical: false,
            effects: vec![],
        }
    }
    
    fn on_entity_death(
        &mut self,
        _entity_id: EntityId,
        _killer_id: Option<EntityId>,
    ) -> Vec<LootDrop> {
        vec![]
    }
    
    fn on_item_pickup(
        &mut self,
        _player_id: PlayerId,
        _drop_id: u64,
    ) -> bool {
        true
    }
    
    fn get_dragon_modifier(&self) -> f32 {
        self.dragon_modifier
    }
    
    fn tick(&mut self) {
        // Mock: no-op
    }
}

/// Mock implementation of VisualFeedback for testing.
pub struct MockVisualFeedback {
    events: Vec<RenderEvent>,
}

impl MockVisualFeedback {
    /// Creates a new mock visual feedback.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }
    
    /// Gets all queued events.
    pub fn get_events(&self) -> &[RenderEvent] {
        &self.events
    }
}

impl Default for MockVisualFeedback {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualFeedback for MockVisualFeedback {
    fn spawn_particles(
        &mut self,
        particle_type: ParticleType,
        position: Position,
        count: u32,
        color: [u8; 4],
    ) {
        self.events.push(RenderEvent::SpawnParticles {
            particle_type,
            position,
            count,
            color,
        });
    }
    
    fn show_floating_text(
        &mut self,
        text: &str,
        position: Position,
        color: [u8; 4],
        duration_ms: u32,
    ) {
        self.events.push(RenderEvent::FloatingText {
            text: text.to_string(),
            position,
            color,
            duration_ms,
        });
    }
    
    fn play_sound(&mut self, sound_id: u32, position: Position, volume: f32) {
        self.events.push(RenderEvent::PlaySound {
            sound_id,
            position,
            volume,
        });
    }
    
    fn screen_shake(&mut self, intensity: f32, duration_ms: u32) {
        self.events.push(RenderEvent::ScreenShake {
            intensity,
            duration_ms,
        });
    }
    
    fn update_ui(&mut self, element: UIElement, value: &str) {
        self.events.push(RenderEvent::UIUpdate {
            element,
            value: value.to_string(),
        });
    }
    
    fn queue_event(&mut self, event: RenderEvent) {
        self.events.push(event);
    }
    
    fn flush_events(&mut self) {
        // Mock: events stay in queue for inspection
    }
}
