//! # Server World State
//!
//! The authoritative game state maintained by the server.
//!
//! ## Design
//!
//! - All entities pre-allocated
//! - Dragon state machine
//! - Client management

use std::net::SocketAddr;
use oroboros_core::{Position, Velocity};
use crate::protocol::{EntityState, WorldSnapshot, DragonState};
use super::connection::{ClientConnection, ConnectionId};
use crate::MAX_CLIENTS;

/// Maximum number of entities in the world.
const MAX_ENTITIES: usize = 1000;

/// Entity in the world.
#[derive(Clone, Copy, Debug, Default)]
pub struct WorldEntity {
    /// Is this entity active?
    pub active: bool,
    /// Entity ID.
    pub id: u32,
    /// Position.
    pub position: Position,
    /// Velocity.
    pub velocity: Velocity,
    /// Health (0-255).
    pub health: u8,
    /// Owner connection (if player-owned).
    pub owner: ConnectionId,
    /// Entity type.
    pub entity_type: EntityType,
}

/// Type of entity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum EntityType {
    /// Empty slot.
    #[default]
    None = 0,
    /// Player character.
    Player = 1,
    /// NPC enemy.
    Enemy = 2,
    /// Projectile.
    Projectile = 3,
    /// Boss (the dragon).
    Boss = 4,
}

/// Server world state.
///
/// Contains all game state that needs to be synchronized.
pub struct ServerState {
    /// Connected clients.
    clients: Box<[ClientConnection]>,
    /// Number of active clients.
    active_clients: usize,
    /// World entities.
    entities: Box<[WorldEntity]>,
    /// Number of active entities.
    active_entities: usize,
    /// Next entity ID to allocate.
    next_entity_id: u32,
    /// Dragon state.
    dragon: DragonState,
    /// Current server tick.
    current_tick: u32,
}

impl ServerState {
    /// Creates a new server state with pre-allocated capacity.
    #[must_use]
    pub fn new(max_clients: usize) -> Self {
        let _ = max_clients; // Use MAX_CLIENTS constant instead
        
        let clients: Vec<ClientConnection> = (0..MAX_CLIENTS)
            .map(|_| ClientConnection::new_empty())
            .collect();
        let entities: Vec<WorldEntity> = (0..MAX_ENTITIES)
            .map(|_| WorldEntity::default())
            .collect();
        
        Self {
            clients: clients.into_boxed_slice(),
            active_clients: 0,
            entities: entities.into_boxed_slice(),
            active_entities: 0,
            next_entity_id: 1,
            dragon: DragonState::new(0, DragonState::STATE_SLEEP),
            current_tick: 0,
        }
    }

    /// Adds a new client.
    ///
    /// Returns the connection ID, or None if server is full.
    pub fn add_client(&mut self, addr: SocketAddr) -> Option<ConnectionId> {
        // Find free slot
        let slot = self.clients.iter().position(|c| !c.is_active())?;
        
        // Allocate entity for player
        let entity_id = self.spawn_entity(EntityType::Player)?;
        
        // Initialize entity
        let entity = &mut self.entities[entity_id as usize];
        entity.owner = ConnectionId(slot as u32);
        entity.health = 100;
        entity.position = Position::new(0.0, 0.0, 0.0); // Spawn point
        
        // Initialize connection
        self.clients[slot].init(
            ConnectionId(slot as u32),
            addr,
            entity_id,
            self.current_tick,
        );
        
        self.active_clients += 1;
        
        Some(ConnectionId(slot as u32))
    }

    /// Removes a client.
    pub fn remove_client(&mut self, id: ConnectionId) {
        if id.is_null() || id.0 as usize >= MAX_CLIENTS {
            return;
        }
        
        let client = &mut self.clients[id.0 as usize];
        if !client.is_active() {
            return;
        }
        
        // Remove player entity
        let entity_id = client.entity_id;
        if (entity_id as usize) < MAX_ENTITIES {
            self.entities[entity_id as usize].active = false;
            self.active_entities = self.active_entities.saturating_sub(1);
        }
        
        client.disconnect();
        self.active_clients = self.active_clients.saturating_sub(1);
    }

    /// Finds a client by address.
    #[must_use]
    pub fn find_client_by_addr(&self, addr: SocketAddr) -> Option<ConnectionId> {
        self.clients
            .iter()
            .position(|c| c.is_active() && c.addr == addr)
            .map(|i| ConnectionId(i as u32))
    }

    /// Gets a mutable reference to a client by address.
    pub fn find_client_by_addr_mut(&mut self, addr: SocketAddr) -> Option<&mut ClientConnection> {
        self.clients
            .iter_mut()
            .find(|c| c.is_active() && c.addr == addr)
    }

    /// Gets a client by ID.
    #[must_use]
    pub fn get_client(&self, id: ConnectionId) -> Option<&ClientConnection> {
        if id.is_null() || id.0 as usize >= MAX_CLIENTS {
            return None;
        }
        let client = &self.clients[id.0 as usize];
        if client.is_active() {
            Some(client)
        } else {
            None
        }
    }

    /// Gets a mutable client reference.
    pub fn get_client_mut(&mut self, id: ConnectionId) -> Option<&mut ClientConnection> {
        if id.is_null() || id.0 as usize >= MAX_CLIENTS {
            return None;
        }
        let client = &mut self.clients[id.0 as usize];
        if client.is_active() {
            Some(client)
        } else {
            None
        }
    }

    /// Spawns a new entity.
    ///
    /// Returns the entity ID, or None if no slots available.
    pub fn spawn_entity(&mut self, entity_type: EntityType) -> Option<u32> {
        // Find free slot
        let slot = self.entities.iter().position(|e| !e.active)?;
        
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        
        self.entities[slot] = WorldEntity {
            active: true,
            id,
            position: Position::default(),
            velocity: Velocity::default(),
            health: 100,
            owner: ConnectionId::NULL,
            entity_type,
        };
        
        self.active_entities += 1;
        
        Some(slot as u32)
    }

    /// Gets an entity by slot index.
    #[must_use]
    pub fn get_entity(&self, index: u32) -> Option<&WorldEntity> {
        let entity = self.entities.get(index as usize)?;
        if entity.active {
            Some(entity)
        } else {
            None
        }
    }

    /// Gets a mutable entity reference.
    pub fn get_entity_mut(&mut self, index: u32) -> Option<&mut WorldEntity> {
        let entity = self.entities.get_mut(index as usize)?;
        if entity.active {
            Some(entity)
        } else {
            None
        }
    }

    /// Updates the world state for one tick.
    ///
    /// This is the hot path - ZERO ALLOCATIONS.
    pub fn update(&mut self) {
        self.current_tick += 1;
        
        // Process player inputs
        self.process_inputs();
        
        // Update physics
        self.update_physics();
        
        // Update dragon AI
        self.update_dragon();
        
        // Check for timeouts
        self.check_timeouts();
    }

    /// Processes all player inputs for this tick.
    fn process_inputs(&mut self) {
        for client in &mut self.clients {
            if !client.is_active() {
                continue;
            }
            
            if let Some(input) = client.latest_input() {
                let entity_idx = client.entity_id as usize;
                if entity_idx < MAX_ENTITIES && self.entities[entity_idx].active {
                    let entity = &mut self.entities[entity_idx];
                    
                    // Apply movement (server validates)
                    let speed = if input.is_sprinting() { 10.0 } else { 5.0 };
                    entity.velocity.x = input.move_x as f32 / 127.0 * speed;
                    entity.velocity.z = input.move_z as f32 / 127.0 * speed;
                    
                    if input.is_jumping() && entity.position.y <= 0.1 {
                        entity.velocity.y = 8.0; // Jump velocity
                    }
                }
            }
        }
    }

    /// Updates physics for all entities.
    fn update_physics(&mut self) {
        const DT: f32 = 1.0 / 60.0; // 60Hz tick rate
        const GRAVITY: f32 = -20.0;
        
        for entity in &mut self.entities {
            if !entity.active {
                continue;
            }
            
            // Apply velocity
            entity.position.x += entity.velocity.x * DT;
            entity.position.y += entity.velocity.y * DT;
            entity.position.z += entity.velocity.z * DT;
            
            // Apply gravity to non-grounded entities
            if entity.position.y > 0.0 {
                entity.velocity.y += GRAVITY * DT;
            } else {
                entity.position.y = 0.0;
                entity.velocity.y = 0.0;
            }
            
            // Apply friction
            entity.velocity.x *= 0.9;
            entity.velocity.z *= 0.9;
        }
    }

    /// Updates the dragon state machine.
    fn update_dragon(&mut self) {
        self.dragon.tick = self.current_tick;
        // Dragon AI updates happen in the dragon module
    }

    /// Checks for client timeouts.
    fn check_timeouts(&mut self) {
        const TIMEOUT_TICKS: u32 = 300; // 5 seconds at 60Hz
        
        for i in 0..MAX_CLIENTS {
            if self.clients[i].is_active() 
                && self.clients[i].is_timed_out(self.current_tick, TIMEOUT_TICKS)
            {
                self.remove_client(ConnectionId(i as u32));
            }
        }
    }

    /// Generates a world snapshot for network transmission.
    #[must_use]
    pub fn generate_snapshot(&self, tick: u32) -> WorldSnapshot {
        let mut snapshot = WorldSnapshot::empty(tick);
        snapshot.dragon = self.dragon;
        
        for entity in &self.entities {
            if !entity.active {
                continue;
            }
            
            let state = EntityState::from_components(
                entity.id,
                entity.position,
                entity.velocity,
                entity.health,
            );
            
            if !snapshot.add_entity(state) {
                // Snapshot full
                break;
            }
        }
        
        snapshot
    }

    /// Returns the current dragon state.
    #[must_use]
    pub const fn dragon(&self) -> &DragonState {
        &self.dragon
    }

    /// Returns a mutable reference to the dragon state.
    pub fn dragon_mut(&mut self) -> &mut DragonState {
        &mut self.dragon
    }

    /// Returns the number of active clients.
    #[must_use]
    pub const fn active_clients(&self) -> usize {
        self.active_clients
    }

    /// Returns the number of active entities.
    #[must_use]
    pub const fn active_entities(&self) -> usize {
        self.active_entities
    }

    /// Returns the current tick.
    #[must_use]
    pub const fn current_tick(&self) -> u32 {
        self.current_tick
    }

    /// Iterates over active clients.
    pub fn iter_clients(&self) -> impl Iterator<Item = &ClientConnection> {
        self.clients.iter().filter(|c| c.is_active())
    }

    /// Iterates over active entities.
    pub fn iter_entities(&self) -> impl Iterator<Item = &WorldEntity> {
        self.entities.iter().filter(|e| e.active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_creation() {
        let state = ServerState::new(500);
        assert_eq!(state.active_clients(), 0);
        assert_eq!(state.active_entities(), 0);
    }

    #[test]
    fn test_client_management() {
        let mut state = ServerState::new(500);
        
        let addr: SocketAddr = "192.168.1.1:12345".parse().unwrap();
        let id = state.add_client(addr).unwrap();
        
        assert_eq!(state.active_clients(), 1);
        assert_eq!(state.active_entities(), 1); // Player entity
        
        let client = state.get_client(id).unwrap();
        assert_eq!(client.addr, addr);
        
        state.remove_client(id);
        assert_eq!(state.active_clients(), 0);
    }

    #[test]
    fn test_entity_spawning() {
        let mut state = ServerState::new(500);
        
        let id1 = state.spawn_entity(EntityType::Enemy).unwrap();
        let id2 = state.spawn_entity(EntityType::Enemy).unwrap();
        
        assert_ne!(id1, id2);
        assert_eq!(state.active_entities(), 2);
        
        let entity = state.get_entity(id1).unwrap();
        assert_eq!(entity.entity_type, EntityType::Enemy);
    }

    #[test]
    fn test_snapshot_generation() {
        let mut state = ServerState::new(500);
        
        // Add some entities
        for _ in 0..5 {
            state.spawn_entity(EntityType::Enemy);
        }
        
        let snapshot = state.generate_snapshot(42);
        assert_eq!(snapshot.tick, 42);
        assert_eq!(snapshot.entity_count, 5);
    }

    #[test]
    fn test_physics_update() {
        let mut state = ServerState::new(500);
        
        let id = state.spawn_entity(EntityType::Player).unwrap();
        {
            let entity = state.get_entity_mut(id).unwrap();
            entity.velocity = Velocity::new(10.0, 5.0, 0.0);
            entity.position = Position::new(0.0, 10.0, 0.0);
        }
        
        // Run physics
        state.update_physics();
        
        let entity = state.get_entity(id).unwrap();
        // Position should have changed
        assert!(entity.position.x > 0.0);
        // Gravity should have affected y velocity
        assert!(entity.velocity.y < 5.0);
    }
}
