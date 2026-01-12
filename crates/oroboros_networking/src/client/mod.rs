//! # Game Client
//!
//! Client-side networking with prediction and interpolation.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      GAME CLIENT                            │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
//! │  │ Prediction   │  │ Interpolation│  │ Network I/O  │      │
//! │  │ (Local)      │  │ (Remote)     │  │ (Async)      │      │
//! │  └──────────────┘  └──────────────┘  └──────────────┘      │
//! │         │                 │                 │               │
//! │         └────────────────┼─────────────────┘               │
//! │                          │                                  │
//! │              ┌───────────▼───────────┐                     │
//! │              │  Local World View     │                     │
//! │              │  (Rendered State)     │                     │
//! │              └───────────────────────┘                     │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use std::net::SocketAddr;
use crate::protocol::{
    PacketHeader, PlayerInput, WorldSnapshot, DragonState,
    PacketSerializer, PacketDeserializer, Packet,
};
use crate::snapshot::SnapshotBuffer;
use crate::prediction::PredictionBuffer;
use crate::MAX_PACKET_SIZE;

/// Client state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientState {
    /// Not connected.
    Disconnected,
    /// Connection handshake in progress.
    Connecting,
    /// Connected and receiving snapshots.
    Connected,
    /// Connection lost, attempting reconnect.
    Reconnecting,
}

impl Default for ClientState {
    fn default() -> Self {
        Self::Disconnected
    }
}

/// Client configuration.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// Server address.
    pub server_addr: SocketAddr,
    /// Connection timeout in seconds.
    pub timeout_secs: f32,
    /// Input buffer size (ticks).
    pub input_buffer_size: usize,
    /// Snapshot buffer size.
    pub snapshot_buffer_size: usize,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            // PRODUCTION SERVER - German Datacenter (Hetzner)
            server_addr: "162.55.2.222:7777".parse().expect("valid address"),
            timeout_secs: 5.0,
            input_buffer_size: 64,
            snapshot_buffer_size: 32,
        }
    }
}

/// Game client for OROBOROS.
pub struct GameClient {
    /// Client configuration.
    #[allow(dead_code)]
    config: ClientConfig,
    /// Current state.
    state: ClientState,
    /// Assigned client ID from server.
    client_id: Option<u32>,
    /// Our entity ID in the world.
    entity_id: Option<u32>,
    /// Sequence number for packets.
    send_sequence: u16,
    /// Last acknowledged sequence from server.
    recv_ack: u16,
    /// Ack bitfield.
    ack_bits: u32,
    /// Input sequence counter.
    input_sequence: u32,
    /// Snapshot buffer for interpolation.
    snapshots: SnapshotBuffer,
    /// Prediction buffer for reconciliation.
    predictions: PredictionBuffer,
    /// Last server tick we received.
    last_server_tick: u32,
    /// RTT estimate in milliseconds.
    rtt_ms: f32,
    /// Last dragon state.
    dragon_state: DragonState,
    /// Packet serializer (reused).
    serializer: PacketSerializer,
}

impl GameClient {
    /// Creates a new client with the given configuration.
    #[must_use]
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            state: ClientState::Disconnected,
            client_id: None,
            entity_id: None,
            send_sequence: 0,
            recv_ack: 0,
            ack_bits: 0,
            input_sequence: 0,
            snapshots: SnapshotBuffer::new(32),
            predictions: PredictionBuffer::new(64),
            last_server_tick: 0,
            rtt_ms: 100.0,
            dragon_state: DragonState::new(0, DragonState::STATE_SLEEP),
            serializer: PacketSerializer::new(),
        }
    }

    /// Returns the current client state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> ClientState {
        self.state
    }

    /// Returns the assigned client ID.
    #[inline]
    #[must_use]
    pub const fn client_id(&self) -> Option<u32> {
        self.client_id
    }

    /// Returns our entity ID.
    #[inline]
    #[must_use]
    pub const fn entity_id(&self) -> Option<u32> {
        self.entity_id
    }

    /// Returns the estimated RTT in milliseconds.
    #[inline]
    #[must_use]
    pub const fn rtt_ms(&self) -> f32 {
        self.rtt_ms
    }

    /// Returns the last known dragon state.
    #[inline]
    #[must_use]
    pub const fn dragon_state(&self) -> &DragonState {
        &self.dragon_state
    }

    /// Creates a connection packet.
    #[must_use]
    pub fn create_connect_packet(&mut self) -> Option<([u8; MAX_PACKET_SIZE], usize)> {
        let header = PacketHeader::new(self.next_sequence(), self.recv_ack, self.ack_bits);
        
        if self.serializer.serialize_connect(&header) {
            let mut data = [0u8; MAX_PACKET_SIZE];
            let len = self.serializer.len();
            data[..len].copy_from_slice(self.serializer.as_slice());
            self.state = ClientState::Connecting;
            Some((data, len))
        } else {
            None
        }
    }

    /// Creates an input packet.
    #[must_use]
    pub fn create_input_packet(&mut self, input: &PlayerInput) -> Option<([u8; MAX_PACKET_SIZE], usize)> {
        if self.state != ClientState::Connected {
            return None;
        }

        let header = PacketHeader::new(self.next_sequence(), self.recv_ack, self.ack_bits);
        
        // Store input for prediction
        self.predictions.add_input(self.input_sequence, *input);
        self.input_sequence += 1;

        if self.serializer.serialize_input(&header, input) {
            let mut data = [0u8; MAX_PACKET_SIZE];
            let len = self.serializer.len();
            data[..len].copy_from_slice(self.serializer.as_slice());
            Some((data, len))
        } else {
            None
        }
    }

    /// Creates a heartbeat packet.
    #[must_use]
    pub fn create_heartbeat_packet(&mut self) -> Option<([u8; MAX_PACKET_SIZE], usize)> {
        let header = PacketHeader::new(self.next_sequence(), self.recv_ack, self.ack_bits);
        
        if self.serializer.serialize_heartbeat(&header) {
            let mut data = [0u8; MAX_PACKET_SIZE];
            let len = self.serializer.len();
            data[..len].copy_from_slice(self.serializer.as_slice());
            Some((data, len))
        } else {
            None
        }
    }

    /// Handles a received packet.
    pub fn handle_packet(&mut self, data: &[u8]) {
        let mut deserializer = PacketDeserializer::new(data);
        
        if let Some(packet) = deserializer.deserialize() {
            // Update ack state
            let header = packet.header();
            self.update_ack(header.sequence);
            
            match packet {
                Packet::ConnectAck(_, client_id) => {
                    self.client_id = Some(client_id);
                    self.entity_id = Some(client_id); // Server assigns entity_id = client_id
                    self.state = ClientState::Connected;
                    tracing::info!("Connected with client_id: {}", client_id);
                }
                Packet::Snapshot(_, snapshot) => {
                    self.handle_snapshot(snapshot);
                }
                Packet::Dragon(_, dragon) => {
                    self.dragon_state = dragon;
                }
                Packet::Hit(_, hit) => {
                    // Handle hit confirmation
                    tracing::debug!("Hit confirmation: tick={}, hit={}", hit.shot_tick, hit.hit);
                }
                Packet::Disconnect(_) => {
                    self.state = ClientState::Disconnected;
                    self.client_id = None;
                }
                _ => {}
            }
        }
    }

    /// Handles a received snapshot.
    fn handle_snapshot(&mut self, snapshot: WorldSnapshot) {
        // Store snapshot for interpolation
        self.snapshots.add_snapshot(snapshot);
        self.last_server_tick = snapshot.tick;
        
        // Reconcile predictions
        if let Some(entity_id) = self.entity_id {
            // Find our entity in snapshot
            for entity in snapshot.entities() {
                if entity.entity_id == entity_id {
                    // Server position - reconcile with predictions
                    self.predictions.reconcile(snapshot.tick, entity.position());
                    break;
                }
            }
        }
        
        // Update dragon state
        self.dragon_state = snapshot.dragon;
    }

    /// Gets the interpolated world state for rendering.
    #[must_use]
    pub fn interpolated_snapshot(&self, render_time: f64) -> Option<WorldSnapshot> {
        self.snapshots.interpolate(render_time)
    }

    /// Gets the predicted position for the local player.
    #[must_use]
    pub fn predicted_position(&self) -> Option<oroboros_core::Position> {
        self.predictions.predicted_position()
    }

    /// Gets the next sequence number.
    fn next_sequence(&mut self) -> u16 {
        let seq = self.send_sequence;
        self.send_sequence = self.send_sequence.wrapping_add(1);
        seq
    }

    /// Updates ack state from received sequence.
    fn update_ack(&mut self, sequence: u16) {
        let diff = sequence.wrapping_sub(self.recv_ack);
        
        if diff == 0 {
            return;
        }
        
        if diff < 32768 {
            // Newer packet
            if diff <= 32 {
                self.ack_bits = (self.ack_bits << diff) | 1;
            } else {
                self.ack_bits = 1;
            }
            self.recv_ack = sequence;
        } else {
            // Older packet
            let back = self.recv_ack.wrapping_sub(sequence);
            if back <= 32 {
                self.ack_bits |= 1 << back;
            }
        }
    }

    /// Disconnects from the server.
    pub fn disconnect(&mut self) {
        self.state = ClientState::Disconnected;
        self.client_id = None;
        self.entity_id = None;
    }
}

impl Default for GameClient {
    fn default() -> Self {
        Self::new(ClientConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = GameClient::new(ClientConfig::default());
        assert_eq!(client.state(), ClientState::Disconnected);
        assert!(client.client_id().is_none());
    }

    #[test]
    fn test_connect_packet() {
        let mut client = GameClient::new(ClientConfig::default());
        let packet = client.create_connect_packet();
        
        assert!(packet.is_some());
        assert_eq!(client.state(), ClientState::Connecting);
    }

    #[test]
    fn test_ack_update() {
        let mut client = GameClient::new(ClientConfig::default());
        
        client.update_ack(1);
        assert_eq!(client.recv_ack, 1);
        
        client.update_ack(2);
        assert_eq!(client.recv_ack, 2);
        assert_eq!(client.ack_bits & 0b11, 0b11);
        
        // Old packet
        client.update_ack(1);
        assert_eq!(client.recv_ack, 2); // Shouldn't change
    }
}
