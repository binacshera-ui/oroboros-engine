//! # Server Game Loop
//!
//! The main game loop that orchestrates all units according to the Golden Path.
//!
//! ## Tick Order (60Hz)
//!
//! ```text
//! 1. Receive inputs from clients (Unit 4)
//! 2. Validate inputs (Server Authority)
//! 3. Process actions:
//!    a. Movement → Update Unit 1
//!    b. Attacks → Call Unit 3 → Update Unit 1
//!    c. Block breaks → Call Unit 3 → Update Unit 1
//! 4. Sync Dragon with market (Unit 3)
//! 5. Broadcast events to clients (Unit 4)
//! 6. Swap buffers (Unit 1)
//! ```

use std::time::{Duration, Instant};
use std::collections::HashMap;

use oroboros_core::Position;

use crate::integration::events::*;
use crate::integration::traits::*;

/// Server tick rate (60 Hz).
pub const SERVER_TICK_RATE: u32 = 60;

/// Duration of one tick in microseconds.
pub const TICK_DURATION_MICROS: u64 = 1_000_000 / SERVER_TICK_RATE as u64;

/// Configuration for the game server.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Tick rate in Hz.
    pub tick_rate: u32,
    /// Maximum clients.
    pub max_clients: usize,
    /// Map seed.
    pub map_seed: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            tick_rate: 60,
            max_clients: 500,
            map_seed: 12345,
        }
    }
}

/// Connected client state.
pub struct ConnectedClient {
    /// Client's player entity ID.
    pub entity_id: EntityId,
    /// Last received input sequence.
    pub last_input_seq: u32,
    /// Connection time.
    pub connected_at: Instant,
    /// Last activity time.
    pub last_activity: Instant,
}

/// Pending action to process.
#[derive(Clone, Debug)]
pub struct PendingAction {
    /// Player who initiated.
    pub player_id: PlayerId,
    /// The action.
    pub action: PlayerAction,
    /// Time received.
    pub received_at: Instant,
}

/// The integrated game server.
///
/// This is the main orchestrator that connects all units.
pub struct GameServer<M: MemoryOwner, E: EconomyAuditor, V: VisualFeedback> {
    /// Configuration.
    #[allow(dead_code)]
    config: ServerConfig,
    /// Unit 1 interface (Memory Owner) - PRIVATE, access via methods only.
    memory: M,
    /// Unit 3 interface (Economy Auditor) - PRIVATE, access via methods only.
    economy: E,
    /// Unit 2 interface (Visual Feedback) - PRIVATE, access via methods only.
    visuals: V,
    /// Connected clients.
    clients: HashMap<PlayerId, ConnectedClient>,
    /// Pending actions to process.
    pending_actions: Vec<PendingAction>,
    /// Events to broadcast this tick.
    pending_events: Vec<(PlayerId, GameEvent)>,
    /// Current tick number.
    tick: u64,
    /// Server start time.
    #[allow(dead_code)]
    start_time: Instant,
    /// Stats.
    stats: ServerStats,
}

/// Server statistics.
#[derive(Clone, Debug, Default)]
pub struct ServerStats {
    /// Total ticks processed.
    pub ticks_processed: u64,
    /// Actions processed this tick.
    pub actions_this_tick: u32,
    /// Events broadcast this tick.
    pub events_this_tick: u32,
    /// Average tick duration (microseconds).
    pub avg_tick_duration_us: u64,
    /// Max tick duration (microseconds).
    pub max_tick_duration_us: u64,
    /// Total blocks broken.
    pub blocks_broken: u64,
    /// Total damage dealt.
    pub damage_dealt: u64,
}

impl<M: MemoryOwner, E: EconomyAuditor, V: VisualFeedback> GameServer<M, E, V> {
    /// Creates a new game server with the given unit implementations.
    pub fn new(config: ServerConfig, memory: M, economy: E, visuals: V) -> Self {
        Self {
            config,
            memory,
            economy,
            visuals,
            clients: HashMap::new(),
            pending_actions: Vec::new(),
            pending_events: Vec::new(),
            tick: 0,
            start_time: Instant::now(),
            stats: ServerStats::default(),
        }
    }
    
    /// Connects a new player.
    pub fn connect_player(&mut self, player_id: PlayerId, spawn_position: Position) -> EntityId {
        let entity_id = self.memory.spawn_entity(EntityType::Player, spawn_position);
        
        self.clients.insert(player_id, ConnectedClient {
            entity_id,
            last_input_seq: 0,
            connected_at: Instant::now(),
            last_activity: Instant::now(),
        });
        
        entity_id
    }
    
    /// Disconnects a player.
    pub fn disconnect_player(&mut self, player_id: PlayerId) {
        if let Some(client) = self.clients.remove(&player_id) {
            self.memory.despawn_entity(client.entity_id);
        }
    }
    
    // =========================================================================
    // CONTROLLED ACCESSORS - Read-only access to subsystems
    // =========================================================================
    
    /// Gets read-only access to a block in the world.
    /// This is the ONLY way external code can read world state.
    pub fn get_block(&self, position: (i32, i32, i32)) -> BlockId {
        self.memory.get_block(position)
    }
    
    /// Gets read-only access to an entity's position.
    pub fn get_entity_position(&self, entity_id: EntityId) -> Option<Position> {
        self.memory.get_position(entity_id)
    }
    
    /// Gets read-only access to an entity's health.
    pub fn get_entity_health(&self, entity_id: EntityId) -> Option<u32> {
        self.memory.get_health(entity_id)
    }
    
    // =========================================================================
    // TEST-ONLY METHODS - Marked with cfg(test) or #[doc(hidden)]
    // =========================================================================
    
    /// Sets up a block for testing purposes.
    /// 
    /// # Safety
    /// This bypasses normal game logic and should ONLY be used in tests.
    /// In production, blocks are modified through player actions only.
    #[doc(hidden)]
    pub fn test_set_block(&mut self, position: (i32, i32, i32), block_type: BlockId) {
        self.memory.update_block(position, block_type);
    }
    
    /// Queues a player action for processing.
    pub fn queue_action(&mut self, player_id: PlayerId, action: PlayerAction) {
        self.pending_actions.push(PendingAction {
            player_id,
            action,
            received_at: Instant::now(),
        });
    }
    
    /// Processes one server tick.
    ///
    /// This is the heart of the Golden Path.
    pub fn tick(&mut self) -> Vec<(PlayerId, GameEvent)> {
        let tick_start = Instant::now();
        self.tick += 1;
        self.stats.actions_this_tick = 0;
        self.stats.events_this_tick = 0;
        
        // Step 1: Process pending actions
        let actions = std::mem::take(&mut self.pending_actions);
        for pending in actions {
            self.process_action(pending);
        }
        
        // Step 2: Tick the economy (sync with market)
        self.economy.tick();
        
        // Step 3: Collect events to broadcast
        let events = std::mem::take(&mut self.pending_events);
        self.stats.events_this_tick = events.len() as u32;
        
        // Step 4: Swap buffers (Unit 1)
        self.memory.swap_buffers();
        self.memory.clear_dirty();
        
        // Step 5: Update stats
        let tick_duration = tick_start.elapsed().as_micros() as u64;
        self.stats.ticks_processed += 1;
        self.stats.avg_tick_duration_us = 
            (self.stats.avg_tick_duration_us * (self.stats.ticks_processed - 1) + tick_duration) 
            / self.stats.ticks_processed;
        if tick_duration > self.stats.max_tick_duration_us {
            self.stats.max_tick_duration_us = tick_duration;
        }
        
        events
    }
    
    /// Processes a single action.
    fn process_action(&mut self, pending: PendingAction) {
        // Extract entity_id first to avoid borrow issues
        let entity_id = {
            let Some(client) = self.clients.get_mut(&pending.player_id) else {
                return;
            };
            client.last_activity = Instant::now();
            client.entity_id
        };
        
        self.stats.actions_this_tick += 1;
        
        match pending.action {
            PlayerAction::Move { sequence, direction, sprint } => {
                self.process_move(pending.player_id, entity_id, sequence, direction, sprint);
            }
            PlayerAction::Attack { sequence, direction, target } => {
                self.process_attack(pending.player_id, entity_id, sequence, direction, target);
            }
            PlayerAction::BreakBlock { sequence, block_pos } => {
                self.process_block_break(pending.player_id, sequence, block_pos);
            }
            PlayerAction::PlaceBlock { sequence, block_pos, block_type } => {
                self.process_block_place(pending.player_id, sequence, block_pos, block_type);
            }
            PlayerAction::UseItem { sequence, slot, target } => {
                self.process_use_item(pending.player_id, sequence, slot, target);
            }
        }
    }
    
    /// Processes a move action.
    fn process_move(
        &mut self,
        player_id: PlayerId,
        entity_id: EntityId,
        _sequence: u32,
        direction: (f32, f32, f32),
        sprint: bool,
    ) {
        let Some(current_pos) = self.memory.get_position(entity_id) else {
            return;
        };
        
        // Calculate new position (server authoritative)
        let speed = if sprint { 8.0 } else { 5.0 };
        let dt = 1.0 / 60.0;
        
        let new_pos = Position::new(
            current_pos.x + direction.0 * speed * dt,
            current_pos.y + direction.1 * speed * dt,
            current_pos.z + direction.2 * speed * dt,
        );
        
        // TODO: Collision detection with world
        
        // Update in memory (Unit 1)
        self.memory.update_position(entity_id, new_pos);
        self.memory.update_velocity(entity_id, (direction.0 * speed, direction.1 * speed, direction.2 * speed));
        
        // No event needed for movement - clients handle via snapshots
        let _ = player_id; // Silence unused warning
    }
    
    /// Processes an attack action.
    fn process_attack(
        &mut self,
        player_id: PlayerId,
        attacker_id: EntityId,
        _sequence: u32,
        direction: (f32, f32, f32),
        target: Option<EntityId>,
    ) {
        // If no explicit target, do raycast to find one
        let defender_id = target.or_else(|| {
            // TODO: Actual raycast through world
            // For now, just check if any entity is in front
            self.find_entity_in_direction(attacker_id, direction)
        });
        
        let Some(defender_id) = defender_id else {
            // No target hit
            return;
        };
        
        // Call Unit 3 to calculate damage
        let base_damage = 10; // TODO: Get from weapon stats
        let response = self.economy.calculate_damage(
            attacker_id,
            defender_id,
            base_damage,
            AttackType::Melee,
        );
        
        let EconomyResponse::DamageCalculated { final_damage, critical: _, effects: _ } = response else {
            return;
        };
        
        // Apply damage (Unit 1)
        let current_health = self.memory.get_health(defender_id).unwrap_or(100);
        let new_health = current_health.saturating_sub(final_damage);
        self.memory.update_health(defender_id, new_health);
        
        self.stats.damage_dealt += u64::from(final_damage);
        
        // Broadcast damage event
        self.pending_events.push((player_id, GameEvent::DamageTaken {
            entity_id: defender_id,
            amount: final_damage,
            source: Some(attacker_id),
            health_remaining: new_health,
        }));
        
        // Check for death
        if new_health == 0 {
            let loot = self.economy.on_entity_death(defender_id, Some(attacker_id));
            
            self.pending_events.push((player_id, GameEvent::EntityDied {
                entity_id: defender_id,
                killer: Some(attacker_id),
                loot: loot.clone(),
            }));
            
            // Spawn loot drops in world
            for drop in loot {
                self.memory.spawn_entity(EntityType::DroppedItem, drop.position);
            }
            
            self.memory.despawn_entity(defender_id);
        }
    }
    
    /// Processes a block break action.
    ///
    /// THIS IS THE GOLDEN PATH for block breaking:
    /// 1. Validate block exists
    /// 2. Call Unit 3 for loot calculation
    /// 3. Update world (Unit 1)
    /// 4. Update inventory (Unit 1)
    /// 5. Broadcast event
    fn process_block_break(
        &mut self,
        player_id: PlayerId,
        _sequence: u32,
        block_pos: (i32, i32, i32),
    ) {
        // Step 1: Validate block exists
        let block_type = self.memory.get_block(block_pos);
        if block_type == 0 {
            // Air - nothing to break
            return;
        }
        
        // Step 2: Call Unit 3 for loot calculation
        // TODO: Get tool from player inventory
        let tool_id = None;
        let response = self.economy.on_block_break(
            player_id,
            block_pos,
            block_type,
            tool_id,
        );
        
        let EconomyResponse::BlockBroken { success, loot, experience: _, transaction_id: _ } = response else {
            return;
        };
        
        if !success {
            return;
        }
        
        // Step 3: Update world - remove block (Unit 1)
        self.memory.update_block(block_pos, 0); // 0 = air
        
        // Step 4: Add loot to inventory (Unit 1)
        for (i, drop) in loot.iter().enumerate() {
            self.memory.update_inventory(
                player_id,
                i as u8,
                Some(InventoryItem {
                    item_id: drop.item_id,
                    quantity: drop.quantity,
                    metadata: 0,
                }),
            );
        }
        
        self.stats.blocks_broken += 1;
        
        // Step 5: Broadcast event for visual feedback
        self.pending_events.push((player_id, GameEvent::BlockBroken {
            player_id,
            position: block_pos,
            block_type,
            loot: loot.clone(),
        }));
        
        // Queue visual feedback
        let pos = Position::new(
            block_pos.0 as f32 + 0.5,
            block_pos.1 as f32 + 0.5,
            block_pos.2 as f32 + 0.5,
        );
        
        // Particles (1000 diamond sparkles as per spec)
        self.visuals.spawn_particles(
            ParticleType::DiamondSparkle,
            pos,
            1000,
            [100, 200, 255, 255], // Diamond blue
        );
        
        // Floating text
        if !loot.is_empty() {
            self.visuals.show_floating_text(
                "+1 Diamond",
                pos,
                [255, 255, 100, 255], // Gold
                2000,
            );
        }
        
        // Sound
        self.visuals.play_sound(1, pos, 1.0); // Block break sound
    }
    
    /// Processes a block place action.
    fn process_block_place(
        &mut self,
        player_id: PlayerId,
        _sequence: u32,
        block_pos: (i32, i32, i32),
        block_type: BlockId,
    ) {
        // Validate position is air
        let current = self.memory.get_block(block_pos);
        if current != 0 {
            return; // Can't place on existing block
        }
        
        // TODO: Check player has block in inventory
        
        // Update world (Unit 1)
        self.memory.update_block(block_pos, block_type);
        
        // Broadcast event
        self.pending_events.push((player_id, GameEvent::BlockPlaced {
            player_id,
            position: block_pos,
            block_type,
        }));
    }
    
    /// Processes an item use action.
    fn process_use_item(
        &mut self,
        _player_id: PlayerId,
        _sequence: u32,
        _slot: u8,
        _target: UseTarget,
    ) {
        // TODO: Implement item use
    }
    
    /// Finds an entity in the given direction from attacker.
    fn find_entity_in_direction(
        &self,
        attacker_id: EntityId,
        _direction: (f32, f32, f32),
    ) -> Option<EntityId> {
        // TODO: Actual raycast implementation
        // For now, find nearest entity within attack range
        let attacker_pos = self.memory.get_position(attacker_id)?;
        let attack_range = 3.0;
        
        for (player_id, client) in &self.clients {
            if client.entity_id == attacker_id {
                continue;
            }
            
            if let Some(other_pos) = self.memory.get_position(client.entity_id) {
                let dist = ((other_pos.x - attacker_pos.x).powi(2)
                    + (other_pos.y - attacker_pos.y).powi(2)
                    + (other_pos.z - attacker_pos.z).powi(2))
                .sqrt();
                
                if dist <= attack_range {
                    return Some(client.entity_id);
                }
            }
            
            let _ = player_id; // Silence unused warning
        }
        
        None
    }
    
    /// Gets server statistics.
    pub fn stats(&self) -> &ServerStats {
        &self.stats
    }
    
    /// Gets current tick number.
    pub fn current_tick(&self) -> u64 {
        self.tick
    }
    
    /// Gets number of connected clients.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }
}

/// Runs the server game loop.
///
/// This is a blocking function that runs at the configured tick rate.
pub fn run_server_loop<M, E, V>(
    server: &mut GameServer<M, E, V>,
    duration: Option<Duration>,
) where
    M: MemoryOwner,
    E: EconomyAuditor,
    V: VisualFeedback,
{
    let tick_duration = Duration::from_micros(TICK_DURATION_MICROS);
    let start = Instant::now();
    let mut next_tick = start;
    
    loop {
        // Check if we should stop
        if let Some(d) = duration {
            if start.elapsed() >= d {
                break;
            }
        }
        
        // Wait until next tick
        let now = Instant::now();
        if now < next_tick {
            std::thread::sleep(next_tick - now);
        }
        next_tick += tick_duration;
        
        // Process tick
        let _events = server.tick();
        
        // TODO: Send events to clients via network
    }
}
