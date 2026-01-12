//! # Packet Definitions
//!
//! All network packet types used in the Ghost Protocol.
//!
//! ## Zero-Allocation Design
//!
//! All packet types are `Copy` and fixed-size to enable:
//! - Pre-allocated packet buffers
//! - Zero-copy deserialization
//! - Cache-friendly iteration

use bytemuck::{Pod, Zeroable};
use oroboros_core::{Position, Velocity};


/// Packet header - present in every packet.
///
/// Total size: 8 bytes
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct PacketHeader {
    /// Sequence number of this packet.
    pub sequence: u16,
    /// Last received sequence number from remote.
    pub ack: u16,
    /// Bitmask of received packets before `ack` (ack-1 through ack-32).
    pub ack_bits: u32,
}

impl PacketHeader {
    /// Creates a new packet header.
    #[inline]
    #[must_use]
    pub const fn new(sequence: u16, ack: u16, ack_bits: u32) -> Self {
        Self { sequence, ack, ack_bits }
    }

    /// Size of the header in bytes.
    pub const SIZE: usize = 8;
}

/// Types of packets in the protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    /// Client -> Server: Player input for this tick.
    Input = 0,
    /// Server -> Client: Full world snapshot.
    Snapshot = 1,
    /// Server -> Client: Delta-compressed world state.
    DeltaSnapshot = 2,
    /// Server -> Client: Dragon state change broadcast.
    DragonBroadcast = 3,
    /// Server -> Client: Hit confirmation.
    HitConfirm = 4,
    /// Client -> Server: Connection request.
    Connect = 5,
    /// Server -> Client: Connection accepted.
    ConnectAck = 6,
    /// Bidirectional: Keep-alive heartbeat.
    Heartbeat = 7,
    /// Bidirectional: Disconnect notification.
    Disconnect = 8,
}

/// Player input packet - Client -> Server.
///
/// Contains the player's inputs for a single tick.
/// The server will validate and apply these.
///
/// Size: 24 bytes
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct PlayerInput {
    /// Server tick this input is for.
    pub tick: u32,
    /// Client's local sequence number for this input.
    pub input_sequence: u32,
    /// Movement direction (normalized, packed as i8 for bandwidth).
    pub move_x: i8,
    /// Movement direction Y.
    pub move_y: i8,
    /// Movement direction Z.
    pub move_z: i8,
    /// Input flags (jump, crouch, sprint, etc.).
    pub flags: u8,
    /// Aim direction X (euler angles, packed as i16).
    pub aim_yaw: i16,
    /// Aim direction Y.
    pub aim_pitch: i16,
    /// Action (0 = none, 1 = shoot, 2 = use, etc.).
    pub action: u8,
    /// Padding for alignment.
    pub _padding: [u8; 3],
    /// Timestamp (client's local time in ms, for latency calculation).
    pub timestamp: u32,
}

impl PlayerInput {
    /// Size in bytes.
    pub const SIZE: usize = 24;

    /// Input flag: Jump.
    pub const FLAG_JUMP: u8 = 1 << 0;
    /// Input flag: Crouch.
    pub const FLAG_CROUCH: u8 = 1 << 1;
    /// Input flag: Sprint.
    pub const FLAG_SPRINT: u8 = 1 << 2;
    /// Input flag: Primary fire.
    pub const FLAG_FIRE: u8 = 1 << 3;
    /// Input flag: Secondary fire/aim.
    pub const FLAG_AIM: u8 = 1 << 4;

    /// Action: None.
    pub const ACTION_NONE: u8 = 0;
    /// Action: Shoot.
    pub const ACTION_SHOOT: u8 = 1;
    /// Action: Use/interact.
    pub const ACTION_USE: u8 = 2;
    /// Action: Reload.
    pub const ACTION_RELOAD: u8 = 3;

    /// Creates a new input packet.
    #[inline]
    #[must_use]
    pub const fn new(tick: u32, input_sequence: u32) -> Self {
        Self {
            tick,
            input_sequence,
            move_x: 0,
            move_y: 0,
            move_z: 0,
            flags: 0,
            aim_yaw: 0,
            aim_pitch: 0,
            action: 0,
            _padding: [0; 3],
            timestamp: 0,
        }
    }

    /// Returns true if the jump flag is set.
    #[inline]
    #[must_use]
    pub const fn is_jumping(&self) -> bool {
        self.flags & Self::FLAG_JUMP != 0
    }

    /// Returns true if the crouch flag is set.
    #[inline]
    #[must_use]
    pub const fn is_crouching(&self) -> bool {
        self.flags & Self::FLAG_CROUCH != 0
    }

    /// Returns true if the sprint flag is set.
    #[inline]
    #[must_use]
    pub const fn is_sprinting(&self) -> bool {
        self.flags & Self::FLAG_SPRINT != 0
    }

    /// Returns true if shooting.
    #[inline]
    #[must_use]
    pub const fn is_shooting(&self) -> bool {
        self.action == Self::ACTION_SHOOT || (self.flags & Self::FLAG_FIRE != 0)
    }
}

/// Entity state in a snapshot.
///
/// Compact representation of an entity's network-relevant state.
///
/// Size: 32 bytes
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct EntityState {
    /// Entity ID (lower 32 bits of EntityId).
    pub entity_id: u32,
    /// Position X (world units).
    pub pos_x: f32,
    /// Position Y.
    pub pos_y: f32,
    /// Position Z.
    pub pos_z: f32,
    /// Velocity X (for interpolation).
    pub vel_x: f32,
    /// Velocity Y.
    pub vel_y: f32,
    /// Velocity Z.
    pub vel_z: f32,
    /// Rotation (yaw as i16 for bandwidth).
    pub rotation: i16,
    /// Health (0-255).
    pub health: u8,
    /// State flags.
    pub flags: u8,
}

impl EntityState {
    /// Size in bytes.
    pub const SIZE: usize = 32;

    /// Creates an entity state from components.
    #[inline]
    #[must_use]
    pub fn from_components(id: u32, pos: Position, vel: Velocity, health: u8) -> Self {
        Self {
            entity_id: id,
            pos_x: pos.x,
            pos_y: pos.y,
            pos_z: pos.z,
            vel_x: vel.x,
            vel_y: vel.y,
            vel_z: vel.z,
            rotation: 0,
            health,
            flags: 0,
        }
    }

    /// Extracts position.
    #[inline]
    #[must_use]
    pub const fn position(&self) -> Position {
        Position::new(self.pos_x, self.pos_y, self.pos_z)
    }

    /// Extracts velocity.
    #[inline]
    #[must_use]
    pub const fn velocity(&self) -> Velocity {
        Velocity::new(self.vel_x, self.vel_y, self.vel_z)
    }
}

/// Dragon state for broadcast.
///
/// The Dragon is the algorithmic boss that responds to market data.
///
/// Size: 16 bytes
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct DragonState {
    /// Server tick when this state was set.
    pub tick: u32,
    /// Dragon behavior state.
    pub state: u8,
    /// Aggression level (0-255).
    pub aggression: u8,
    /// Target player ID (0 = none).
    pub target_id: u16,
    /// Dragon position X.
    pub pos_x: f32,
    /// Dragon position Z (Y is fixed for dragon).
    pub pos_z: f32,
}

impl DragonState {
    /// Size in bytes.
    pub const SIZE: usize = 16;

    /// State: Sleeping (market calm).
    pub const STATE_SLEEP: u8 = 0;
    /// State: Stalking (volatility rising).
    pub const STATE_STALK: u8 = 1;
    /// State: Inferno (market crash/spike).
    pub const STATE_INFERNO: u8 = 2;

    /// Creates a new dragon state.
    #[inline]
    #[must_use]
    pub const fn new(tick: u32, state: u8) -> Self {
        Self {
            tick,
            state,
            aggression: 0,
            target_id: 0,
            pos_x: 0.0,
            pos_z: 0.0,
        }
    }
}

/// World snapshot - full state of all entities.
///
/// Maximum entities: 36 (to fit in MTU with header).
/// For more entities, use delta compression.
///
/// Size: 8 + 4 + (32 * entity_count) bytes
#[derive(Clone, Copy, Debug)]
pub struct WorldSnapshot {
    /// Server tick this snapshot represents.
    pub tick: u32,
    /// Number of entities in this snapshot.
    pub entity_count: u16,
    /// Dragon state.
    pub dragon: DragonState,
    /// Entity states (pre-allocated array).
    pub entities: [EntityState; Self::MAX_ENTITIES],
}

impl WorldSnapshot {
    /// Maximum entities in a single snapshot packet.
    pub const MAX_ENTITIES: usize = 36;

    /// Creates an empty snapshot.
    #[must_use]
    pub const fn empty(tick: u32) -> Self {
        Self {
            tick,
            entity_count: 0,
            dragon: DragonState::new(tick, DragonState::STATE_SLEEP),
            entities: [EntityState {
                entity_id: 0,
                pos_x: 0.0,
                pos_y: 0.0,
                pos_z: 0.0,
                vel_x: 0.0,
                vel_y: 0.0,
                vel_z: 0.0,
                rotation: 0,
                health: 0,
                flags: 0,
            }; Self::MAX_ENTITIES],
        }
    }

    /// Adds an entity to the snapshot.
    ///
    /// Returns false if snapshot is full.
    #[inline]
    pub fn add_entity(&mut self, state: EntityState) -> bool {
        if self.entity_count as usize >= Self::MAX_ENTITIES {
            return false;
        }
        self.entities[self.entity_count as usize] = state;
        self.entity_count += 1;
        true
    }

    /// Returns a slice of valid entities.
    #[inline]
    #[must_use]
    pub fn entities(&self) -> &[EntityState] {
        &self.entities[..self.entity_count as usize]
    }
}

impl Default for WorldSnapshot {
    fn default() -> Self {
        Self::empty(0)
    }
}

/// Delta snapshot - only changed entities.
///
/// Used after initial full snapshot to reduce bandwidth.
#[derive(Clone, Copy, Debug)]
pub struct DeltaSnapshot {
    /// Server tick this delta is for.
    pub tick: u32,
    /// Base tick this delta is relative to.
    pub base_tick: u32,
    /// Number of changed entities.
    pub changed_count: u16,
    /// Number of removed entities.
    pub removed_count: u16,
    /// Changed entity states.
    pub changed: [EntityState; Self::MAX_CHANGES],
    /// Removed entity IDs.
    pub removed: [u32; Self::MAX_REMOVED],
}

impl DeltaSnapshot {
    /// Maximum changed entities per delta.
    pub const MAX_CHANGES: usize = 30;
    /// Maximum removed entities per delta.
    pub const MAX_REMOVED: usize = 16;

    /// Creates an empty delta snapshot.
    #[must_use]
    pub const fn empty(tick: u32, base_tick: u32) -> Self {
        Self {
            tick,
            base_tick,
            changed_count: 0,
            removed_count: 0,
            changed: [EntityState {
                entity_id: 0,
                pos_x: 0.0,
                pos_y: 0.0,
                pos_z: 0.0,
                vel_x: 0.0,
                vel_y: 0.0,
                vel_z: 0.0,
                rotation: 0,
                health: 0,
                flags: 0,
            }; Self::MAX_CHANGES],
            removed: [0; Self::MAX_REMOVED],
        }
    }
}

impl Default for DeltaSnapshot {
    fn default() -> Self {
        Self::empty(0, 0)
    }
}

/// Shot fired report - Client -> Server.
///
/// Client reports "I shot in this direction".
/// Server validates and determines hit/miss.
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct ShotFired {
    /// Tick when shot was fired.
    pub tick: u32,
    /// Origin position X.
    pub origin_x: f32,
    /// Origin position Y.
    pub origin_y: f32,
    /// Origin position Z.
    pub origin_z: f32,
    /// Direction X (normalized).
    pub dir_x: f32,
    /// Direction Y.
    pub dir_y: f32,
    /// Direction Z.
    pub dir_z: f32,
    /// Weapon ID.
    pub weapon_id: u8,
    /// Padding.
    pub _padding: [u8; 3],
}

impl ShotFired {
    /// Size in bytes.
    pub const SIZE: usize = 32;
}

/// Hit confirmation - Server -> Client.
///
/// Server tells client if their shot hit.
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct HitReport {
    /// Tick of the original shot.
    pub shot_tick: u32,
    /// Did the shot hit?
    pub hit: u8,
    /// Target entity ID (if hit).
    pub target_id: u8,
    /// Damage dealt.
    pub damage: u16,
    /// Target's new health (after damage).
    pub target_health: u8,
    /// Was it a kill?
    pub killed: u8,
    /// Padding.
    pub _padding: [u8; 2],
}

impl HitReport {
    /// Size in bytes.
    pub const SIZE: usize = 12;
}

/// Generic packet container.
#[derive(Clone, Copy, Debug)]
pub enum Packet {
    /// Player input.
    Input(PacketHeader, PlayerInput),
    /// World snapshot.
    Snapshot(PacketHeader, WorldSnapshot),
    /// Delta snapshot.
    Delta(PacketHeader, DeltaSnapshot),
    /// Dragon broadcast.
    Dragon(PacketHeader, DragonState),
    /// Hit confirmation.
    Hit(PacketHeader, HitReport),
    /// Connection request.
    Connect(PacketHeader),
    /// Connection acknowledgment.
    ConnectAck(PacketHeader, u32), // client_id
    /// Heartbeat.
    Heartbeat(PacketHeader),
    /// Disconnect.
    Disconnect(PacketHeader),
}

impl Packet {
    /// Returns the packet type.
    #[must_use]
    pub const fn packet_type(&self) -> PacketType {
        match self {
            Self::Input(..) => PacketType::Input,
            Self::Snapshot(..) => PacketType::Snapshot,
            Self::Delta(..) => PacketType::DeltaSnapshot,
            Self::Dragon(..) => PacketType::DragonBroadcast,
            Self::Hit(..) => PacketType::HitConfirm,
            Self::Connect(..) => PacketType::Connect,
            Self::ConnectAck(..) => PacketType::ConnectAck,
            Self::Heartbeat(..) => PacketType::Heartbeat,
            Self::Disconnect(..) => PacketType::Disconnect,
        }
    }

    /// Returns the header.
    #[must_use]
    pub const fn header(&self) -> &PacketHeader {
        match self {
            Self::Input(h, _)
            | Self::Snapshot(h, _)
            | Self::Delta(h, _)
            | Self::Dragon(h, _)
            | Self::Hit(h, _)
            | Self::Connect(h)
            | Self::ConnectAck(h, _)
            | Self::Heartbeat(h)
            | Self::Disconnect(h) => h,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_sizes() {
        assert_eq!(std::mem::size_of::<PacketHeader>(), 8);
        assert_eq!(std::mem::size_of::<PlayerInput>(), PlayerInput::SIZE);
        assert_eq!(std::mem::size_of::<EntityState>(), EntityState::SIZE);
        assert_eq!(std::mem::size_of::<DragonState>(), DragonState::SIZE);
        assert_eq!(std::mem::size_of::<ShotFired>(), ShotFired::SIZE);
        assert_eq!(std::mem::size_of::<HitReport>(), HitReport::SIZE);
    }

    #[test]
    fn test_snapshot_add_entity() {
        let mut snapshot = WorldSnapshot::empty(1);
        
        for i in 0..WorldSnapshot::MAX_ENTITIES {
            let state = EntityState {
                entity_id: i as u32,
                ..Default::default()
            };
            assert!(snapshot.add_entity(state));
        }
        
        // Should fail - snapshot full
        let state = EntityState::default();
        assert!(!snapshot.add_entity(state));
    }

    #[test]
    fn test_player_input_flags() {
        let mut input = PlayerInput::new(1, 1);
        
        assert!(!input.is_jumping());
        input.flags |= PlayerInput::FLAG_JUMP;
        assert!(input.is_jumping());
        
        assert!(!input.is_shooting());
        input.action = PlayerInput::ACTION_SHOOT;
        assert!(input.is_shooting());
    }
}
