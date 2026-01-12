//! # Client Connection Management
//!
//! Track connected clients, their state, and their input history.
//!
//! ## Design
//!
//! - Fixed-size connection slots (no allocations)
//! - Ring buffer for input history
//! - Sequence number tracking for reliable delivery

use std::net::SocketAddr;
use crate::protocol::{PlayerInput, SequenceNumber, AckBitfield};

/// Unique identifier for a client connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConnectionId(pub u32);

impl ConnectionId {
    /// Invalid/null connection ID.
    pub const NULL: Self = Self(u32::MAX);

    /// Returns true if this is a null/invalid ID.
    #[inline]
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == u32::MAX
    }
}

impl Default for ConnectionId {
    fn default() -> Self {
        Self::NULL
    }
}

/// State of a client connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ConnectionState {
    /// Slot is free.
    Disconnected = 0,
    /// Connection handshake in progress.
    Connecting = 1,
    /// Fully connected and active.
    Connected = 2,
    /// Timed out, pending cleanup.
    TimedOut = 3,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self::Disconnected
    }
}

/// Size of input history ring buffer.
const INPUT_HISTORY_SIZE: usize = 64;

/// Client connection data.
///
/// Fixed-size structure for zero-allocation client management.
#[derive(Clone, Debug)]
pub struct ClientConnection {
    /// Connection ID.
    pub id: ConnectionId,
    /// Connection state.
    pub state: ConnectionState,
    /// Client's network address.
    pub addr: SocketAddr,
    /// Last received sequence number.
    pub last_recv_sequence: SequenceNumber,
    /// Next sequence number to send.
    pub next_send_sequence: SequenceNumber,
    /// Acknowledgment bitmask.
    pub ack_bits: AckBitfield,
    /// Last acknowledged sequence.
    pub last_ack: SequenceNumber,
    /// Round-trip time estimate (microseconds).
    pub rtt_us: u32,
    /// Last time we received a packet (tick number).
    pub last_recv_tick: u32,
    /// Last time we sent a packet (tick number).
    pub last_send_tick: u32,
    /// Player entity ID in the world.
    pub entity_id: u32,
    /// Input history ring buffer.
    pub input_history: [PlayerInput; INPUT_HISTORY_SIZE],
    /// Index of latest input in ring buffer.
    pub input_write_index: usize,
    /// Number of inputs in buffer.
    pub input_count: usize,
}

impl ClientConnection {
    /// Creates a new disconnected client slot.
    #[must_use]
    pub fn new_empty() -> Self {
        Self {
            id: ConnectionId::NULL,
            state: ConnectionState::Disconnected,
            addr: "0.0.0.0:0".parse().expect("valid zero address"),
            last_recv_sequence: 0,
            next_send_sequence: 0,
            ack_bits: 0,
            last_ack: 0,
            rtt_us: 0,
            last_recv_tick: 0,
            last_send_tick: 0,
            entity_id: u32::MAX,
            input_history: [PlayerInput::new(0, 0); INPUT_HISTORY_SIZE],
            input_write_index: 0,
            input_count: 0,
        }
    }

    /// Initializes this slot for a new connection.
    pub fn init(&mut self, id: ConnectionId, addr: SocketAddr, entity_id: u32, tick: u32) {
        self.id = id;
        self.state = ConnectionState::Connected;
        self.addr = addr;
        self.last_recv_sequence = 0;
        self.next_send_sequence = 0;
        self.ack_bits = 0;
        self.last_ack = 0;
        self.rtt_us = 100_000; // Start with 100ms estimate
        self.last_recv_tick = tick;
        self.last_send_tick = tick;
        self.entity_id = entity_id;
        self.input_write_index = 0;
        self.input_count = 0;
    }

    /// Resets this slot to disconnected state.
    pub fn disconnect(&mut self) {
        self.state = ConnectionState::Disconnected;
        self.id = ConnectionId::NULL;
    }

    /// Returns true if this slot is active.
    #[inline]
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self.state, ConnectionState::Connected | ConnectionState::Connecting)
    }

    /// Adds an input to the history.
    pub fn add_input(&mut self, input: PlayerInput) {
        self.input_history[self.input_write_index] = input;
        self.input_write_index = (self.input_write_index + 1) % INPUT_HISTORY_SIZE;
        self.input_count = self.input_count.saturating_add(1).min(INPUT_HISTORY_SIZE);
    }

    /// Gets the latest input.
    #[must_use]
    pub fn latest_input(&self) -> Option<&PlayerInput> {
        if self.input_count == 0 {
            return None;
        }
        let index = if self.input_write_index == 0 {
            INPUT_HISTORY_SIZE - 1
        } else {
            self.input_write_index - 1
        };
        Some(&self.input_history[index])
    }

    /// Gets an input by tick number.
    #[must_use]
    pub fn get_input_for_tick(&self, tick: u32) -> Option<&PlayerInput> {
        for i in 0..self.input_count {
            let index = (self.input_write_index + INPUT_HISTORY_SIZE - 1 - i) % INPUT_HISTORY_SIZE;
            if self.input_history[index].tick == tick {
                return Some(&self.input_history[index]);
            }
        }
        None
    }

    /// Updates acknowledgment state from received packet.
    pub fn update_ack(&mut self, ack: SequenceNumber, ack_bits: AckBitfield) {
        self.last_ack = ack;
        self.ack_bits = ack_bits;
    }

    /// Gets the next sequence number and increments it.
    #[inline]
    pub fn next_sequence(&mut self) -> SequenceNumber {
        let seq = self.next_send_sequence;
        self.next_send_sequence = self.next_send_sequence.wrapping_add(1);
        seq
    }

    /// Records packet reception for RTT calculation.
    pub fn record_recv(&mut self, sequence: SequenceNumber, tick: u32) {
        // Calculate if this sequence is newer than last
        let diff = sequence.wrapping_sub(self.last_recv_sequence);
        if diff < 32768 {
            // Newer packet
            self.last_recv_sequence = sequence;
        }
        self.last_recv_tick = tick;
    }

    /// Checks if connection has timed out.
    #[must_use]
    pub fn is_timed_out(&self, current_tick: u32, timeout_ticks: u32) -> bool {
        current_tick.saturating_sub(self.last_recv_tick) > timeout_ticks
    }
}

impl Default for ClientConnection {
    fn default() -> Self {
        Self::new_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_lifecycle() {
        let mut conn = ClientConnection::new_empty();
        assert!(!conn.is_active());
        
        let addr: SocketAddr = "192.168.1.1:12345".parse().unwrap();
        conn.init(ConnectionId(1), addr, 100, 0);
        
        assert!(conn.is_active());
        assert_eq!(conn.id.0, 1);
        assert_eq!(conn.entity_id, 100);
        
        conn.disconnect();
        assert!(!conn.is_active());
    }

    #[test]
    fn test_input_history() {
        let mut conn = ClientConnection::new_empty();
        conn.init(ConnectionId(1), "127.0.0.1:1234".parse().unwrap(), 0, 0);
        
        // Add inputs
        for i in 0..10 {
            let input = PlayerInput {
                tick: i,
                input_sequence: i,
                ..Default::default()
            };
            conn.add_input(input);
        }
        
        // Latest should be tick 9
        let latest = conn.latest_input().unwrap();
        assert_eq!(latest.tick, 9);
        
        // Get specific tick
        let tick5 = conn.get_input_for_tick(5).unwrap();
        assert_eq!(tick5.tick, 5);
    }

    #[test]
    fn test_input_history_overflow() {
        let mut conn = ClientConnection::new_empty();
        conn.init(ConnectionId(1), "127.0.0.1:1234".parse().unwrap(), 0, 0);
        
        // Overflow the buffer
        for i in 0..100 {
            let input = PlayerInput {
                tick: i,
                input_sequence: i,
                ..Default::default()
            };
            conn.add_input(input);
        }
        
        // Should still work, latest is 99
        let latest = conn.latest_input().unwrap();
        assert_eq!(latest.tick, 99);
        
        // Old inputs should be gone
        assert!(conn.get_input_for_tick(0).is_none());
    }

    #[test]
    fn test_timeout() {
        let mut conn = ClientConnection::new_empty();
        conn.init(ConnectionId(1), "127.0.0.1:1234".parse().unwrap(), 0, 100);
        
        // Not timed out yet
        assert!(!conn.is_timed_out(100, 60));
        assert!(!conn.is_timed_out(150, 60));
        
        // Timed out
        assert!(conn.is_timed_out(200, 60));
    }
}
