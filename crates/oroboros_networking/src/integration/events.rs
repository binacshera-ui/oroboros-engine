//! # Cross-Unit Event System
//!
//! Events that flow between units during the Golden Path.
//!
//! ## Event Flow for Block Break:
//! ```text
//! 1. Client: PlayerAction::BreakBlock
//! 2. Server: Validate hit (Raycast)
//! 3. Server → Unit 3: BlockBreakRequest
//! 4. Unit 3 → Server: BlockBreakResult (with loot)
//! 5. Server → Unit 1: InventoryUpdate
//! 6. Server → Client: GameEvent::BlockBroken
//! 7. Client → Unit 2: RenderEvent::SpawnParticles
//! ```

use oroboros_core::Position;

/// Unique identifier for entities across all units.
pub type EntityId = u32;

/// Unique identifier for players.
pub type PlayerId = u32;

/// Block type identifier.
pub type BlockId = u16;

/// Item type identifier.
pub type ItemId = u16;

// ============================================================================
// PLAYER ACTIONS (Client → Server via Unit 4)
// ============================================================================

/// Actions a player can perform, sent from client to server.
#[derive(Clone, Debug)]
pub enum PlayerAction {
    /// Player moved (predicted locally, validated on server).
    Move {
        /// Input sequence for reconciliation.
        sequence: u32,
        /// Movement direction (-1.0 to 1.0).
        direction: (f32, f32, f32),
        /// Is player sprinting?
        sprint: bool,
    },
    
    /// Player attacked (melee or ranged).
    Attack {
        /// Input sequence.
        sequence: u32,
        /// Direction of attack.
        direction: (f32, f32, f32),
        /// Target entity if any.
        target: Option<EntityId>,
    },
    
    /// Player wants to break a block.
    BreakBlock {
        /// Input sequence.
        sequence: u32,
        /// Block position in world.
        block_pos: (i32, i32, i32),
    },
    
    /// Player wants to place a block.
    PlaceBlock {
        /// Input sequence.
        sequence: u32,
        /// Block position in world.
        block_pos: (i32, i32, i32),
        /// Block type to place.
        block_type: BlockId,
    },
    
    /// Player wants to use an item.
    UseItem {
        /// Input sequence.
        sequence: u32,
        /// Inventory slot.
        slot: u8,
        /// Target position or entity.
        target: UseTarget,
    },
}

/// Target for item use.
#[derive(Clone, Debug)]
pub enum UseTarget {
    /// Use on self.
    OnSelf,
    /// Use on a position.
    Position(Position),
    /// Use on an entity.
    Entity(EntityId),
}

// ============================================================================
// GAME EVENTS (Server → Client via Unit 4)
// ============================================================================

/// Events broadcast from server to clients.
#[derive(Clone, Debug)]
pub enum GameEvent {
    /// A block was broken.
    BlockBroken {
        /// Who broke it.
        player_id: PlayerId,
        /// Where.
        position: (i32, i32, i32),
        /// What block type.
        block_type: BlockId,
        /// What dropped.
        loot: Vec<LootDrop>,
    },
    
    /// A block was placed.
    BlockPlaced {
        /// Who placed it.
        player_id: PlayerId,
        /// Where.
        position: (i32, i32, i32),
        /// What block type.
        block_type: BlockId,
    },
    
    /// An entity took damage.
    DamageTaken {
        /// Who took damage.
        entity_id: EntityId,
        /// How much.
        amount: u32,
        /// From whom.
        source: Option<EntityId>,
        /// Remaining health.
        health_remaining: u32,
    },
    
    /// An entity died.
    EntityDied {
        /// Who died.
        entity_id: EntityId,
        /// Who killed them.
        killer: Option<EntityId>,
        /// What dropped.
        loot: Vec<LootDrop>,
    },
    
    /// Player's inventory changed.
    InventoryUpdate {
        /// Which slot.
        slot: u8,
        /// New item (None = empty).
        item: Option<InventoryItem>,
    },
    
    /// Dragon state changed (for UI/atmosphere).
    DragonStateChanged {
        /// New state.
        state: DragonState,
        /// Aggression level (0-100).
        aggression: u8,
    },
}

/// An item that dropped as loot.
#[derive(Clone, Debug)]
pub struct LootDrop {
    /// Item type.
    pub item_id: ItemId,
    /// Quantity.
    pub quantity: u32,
    /// Position where it dropped.
    pub position: Position,
    /// Unique drop ID for tracking.
    pub drop_id: u64,
}

/// An item in player inventory.
#[derive(Clone, Debug)]
pub struct InventoryItem {
    /// Item type.
    pub item_id: ItemId,
    /// Quantity.
    pub quantity: u32,
    /// Item metadata (durability, enchants, etc.).
    pub metadata: u64,
}

/// Dragon behavioral state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragonState {
    /// Dragon is sleeping (low ETH volatility).
    Sleep,
    /// Dragon is watching (medium activity).
    Stalk,
    /// Dragon is attacking (high volatility/price crash).
    Inferno,
}

// ============================================================================
// UNIT 3 (ECONOMY) REQUESTS
// ============================================================================

/// Request to Unit 3's economy system.
#[derive(Clone, Debug)]
pub enum EconomyRequest {
    /// Request to break a block and get loot.
    BreakBlock {
        /// Who's breaking.
        player_id: PlayerId,
        /// Block position.
        position: (i32, i32, i32),
        /// Block type.
        block_type: BlockId,
        /// Player's tool (affects drop rates).
        tool_id: Option<ItemId>,
    },
    
    /// Request to calculate damage.
    CalculateDamage {
        /// Attacker.
        attacker_id: EntityId,
        /// Defender.
        defender_id: EntityId,
        /// Base damage.
        base_damage: u32,
        /// Attack type.
        attack_type: AttackType,
    },
    
    /// Request to craft an item.
    CraftItem {
        /// Who's crafting.
        player_id: PlayerId,
        /// Recipe ID.
        recipe_id: u32,
    },
    
    /// Request to trade between players.
    Trade {
        /// From player.
        from_player: PlayerId,
        /// To player.
        to_player: PlayerId,
        /// Items offered.
        offer: Vec<InventoryItem>,
        /// Items requested.
        request: Vec<InventoryItem>,
    },
}

/// Attack type for damage calculation.
#[derive(Clone, Copy, Debug)]
pub enum AttackType {
    /// Physical melee.
    Melee,
    /// Physical ranged.
    Ranged,
    /// Magic damage.
    Magic,
    /// Dragon fire (affected by ETH price).
    DragonFire,
}

/// Response from Unit 3's economy system.
#[derive(Clone, Debug)]
pub enum EconomyResponse {
    /// Block break result.
    BlockBroken {
        /// Success or failure.
        success: bool,
        /// Loot generated.
        loot: Vec<LootDrop>,
        /// Experience gained.
        experience: u32,
        /// Transaction ID for audit.
        transaction_id: u64,
    },
    
    /// Damage calculation result.
    DamageCalculated {
        /// Final damage after modifiers.
        final_damage: u32,
        /// Was it a critical hit?
        critical: bool,
        /// Any status effects applied.
        effects: Vec<StatusEffect>,
    },
    
    /// Craft result.
    CraftResult {
        /// Success or failure.
        success: bool,
        /// Crafted item if successful.
        item: Option<InventoryItem>,
        /// Consumed materials.
        consumed: Vec<InventoryItem>,
    },
    
    /// Trade result.
    TradeResult {
        /// Success or failure.
        success: bool,
        /// Reason if failed.
        failure_reason: Option<String>,
    },
}

/// Status effect from combat.
#[derive(Clone, Debug)]
pub struct StatusEffect {
    /// Effect type.
    pub effect_type: StatusEffectType,
    /// Duration in ticks.
    pub duration_ticks: u32,
    /// Magnitude.
    pub magnitude: f32,
}

/// Types of status effects.
#[derive(Clone, Copy, Debug)]
pub enum StatusEffectType {
    /// Damage over time.
    Burning,
    /// Slowed movement.
    Slowed,
    /// Increased damage taken.
    Vulnerable,
    /// Healing over time.
    Regeneration,
    /// Increased speed.
    Haste,
}

// ============================================================================
// UNIT 1 (MEMORY) COMMANDS
// ============================================================================

/// Commands to Unit 1's memory system.
#[derive(Clone, Debug)]
pub enum MemoryCommand {
    /// Update entity position.
    UpdatePosition {
        /// Entity ID.
        entity_id: EntityId,
        /// New position.
        position: Position,
    },
    
    /// Update entity health.
    UpdateHealth {
        /// Entity ID.
        entity_id: EntityId,
        /// New health.
        health: u32,
    },
    
    /// Update player inventory.
    UpdateInventory {
        /// Player ID.
        player_id: PlayerId,
        /// Slot to update.
        slot: u8,
        /// New item.
        item: Option<InventoryItem>,
    },
    
    /// Spawn a new entity.
    SpawnEntity {
        /// Entity ID.
        entity_id: EntityId,
        /// Entity type.
        entity_type: EntityType,
        /// Position.
        position: Position,
    },
    
    /// Despawn an entity.
    DespawnEntity {
        /// Entity ID.
        entity_id: EntityId,
    },
    
    /// Update block in world.
    UpdateBlock {
        /// Position.
        position: (i32, i32, i32),
        /// New block type (0 = air).
        block_type: BlockId,
    },
}

/// Types of entities.
#[derive(Clone, Copy, Debug)]
pub enum EntityType {
    /// Player character.
    Player,
    /// NPC enemy.
    Enemy,
    /// Dropped item.
    DroppedItem,
    /// Projectile.
    Projectile,
    /// The Dragon.
    Dragon,
}

// ============================================================================
// UNIT 2 (RENDER) EVENTS
// ============================================================================

/// Events sent to Unit 2 for visual feedback.
#[derive(Clone, Debug)]
pub enum RenderEvent {
    /// Spawn particles at position.
    SpawnParticles {
        /// Particle type.
        particle_type: ParticleType,
        /// Position.
        position: Position,
        /// Number of particles.
        count: u32,
        /// Color (RGBA).
        color: [u8; 4],
    },
    
    /// Show floating text (damage numbers, "+1 Diamond").
    FloatingText {
        /// Text to show.
        text: String,
        /// Position.
        position: Position,
        /// Color.
        color: [u8; 4],
        /// Duration in ms.
        duration_ms: u32,
    },
    
    /// Play sound effect.
    PlaySound {
        /// Sound ID.
        sound_id: u32,
        /// Position (for 3D audio).
        position: Position,
        /// Volume (0.0 - 1.0).
        volume: f32,
    },
    
    /// Screen shake.
    ScreenShake {
        /// Intensity (0.0 - 1.0).
        intensity: f32,
        /// Duration in ms.
        duration_ms: u32,
    },
    
    /// Update UI element.
    UIUpdate {
        /// UI element ID.
        element: UIElement,
        /// New value.
        value: String,
    },
}

/// Particle types for visual effects.
#[derive(Clone, Copy, Debug)]
pub enum ParticleType {
    /// Block break particles.
    BlockBreak,
    /// Diamond sparkles.
    DiamondSparkle,
    /// Blood splatter.
    Blood,
    /// Fire.
    Fire,
    /// Smoke.
    Smoke,
    /// Magic sparkles.
    Magic,
}

/// UI elements that can be updated.
#[derive(Clone, Copy, Debug)]
pub enum UIElement {
    /// Health bar.
    HealthBar,
    /// Mana bar.
    ManaBar,
    /// Inventory slot.
    InventorySlot(u8),
    /// Chat message.
    ChatMessage,
    /// Dragon warning indicator.
    DragonWarning,
}

// ============================================================================
// EVENT CHANNELS
// ============================================================================

/// Channel for sending events between units.
/// Uses crossbeam for lock-free communication.
pub struct EventChannel<T> {
    sender: crossbeam_channel::Sender<T>,
    receiver: crossbeam_channel::Receiver<T>,
}

impl<T> EventChannel<T> {
    /// Creates a new bounded event channel.
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(capacity);
        Self { sender, receiver }
    }
    
    /// Creates a new unbounded event channel.
    pub fn unbounded() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { sender, receiver }
    }
    
    /// Sends an event (non-blocking).
    pub fn send(&self, event: T) -> Result<(), crossbeam_channel::SendError<T>> {
        self.sender.send(event)
    }
    
    /// Tries to send an event (returns immediately).
    pub fn try_send(&self, event: T) -> Result<(), crossbeam_channel::TrySendError<T>> {
        self.sender.try_send(event)
    }
    
    /// Receives an event (blocking).
    pub fn recv(&self) -> Result<T, crossbeam_channel::RecvError> {
        self.receiver.recv()
    }
    
    /// Tries to receive an event (non-blocking).
    pub fn try_recv(&self) -> Result<T, crossbeam_channel::TryRecvError> {
        self.receiver.try_recv()
    }
    
    /// Gets a clone of the sender for another thread.
    pub fn sender(&self) -> crossbeam_channel::Sender<T> {
        self.sender.clone()
    }
    
    /// Gets a clone of the receiver for another thread.
    pub fn receiver(&self) -> crossbeam_channel::Receiver<T> {
        self.receiver.clone()
    }
}

impl<T> Default for EventChannel<T> {
    fn default() -> Self {
        Self::new(1024)
    }
}
