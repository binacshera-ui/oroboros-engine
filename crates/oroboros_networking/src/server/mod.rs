//! # Inferno Server
//!
//! The authoritative game server for OROBOROS Inferno.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     INFERNO SERVER                          │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
//! │  │ Network I/O  │  │ Game Loop    │  │ Broadcast    │      │
//! │  │ (Async)      │──│ (60Hz Tick)  │──│ (Lock-Free)  │      │
//! │  └──────────────┘  └──────────────┘  └──────────────┘      │
//! │         │                 │                 │               │
//! │         └────────────────┼─────────────────┘               │
//! │                          │                                  │
//! │              ┌───────────▼───────────┐                     │
//! │              │ World State (Memory)  │                     │
//! │              │ - Entity positions    │                     │
//! │              │ - Dragon state        │                     │
//! │              │ - Player inputs       │                     │
//! │              └───────────────────────┘                     │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Performance Requirements
//!
//! - 60Hz tick rate (16.67ms per tick)
//! - 500 concurrent clients
//! - Sub-millisecond packet processing
//! - Zero allocations in tick loop

mod connection;
mod state;
mod tick;

pub use connection::{ClientConnection, ConnectionId, ConnectionState};
pub use state::ServerState;
pub use tick::TickLoop;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use crossbeam_channel::{bounded, Receiver, Sender};
use crate::protocol::{PacketHeader, PlayerInput, WorldSnapshot};
use crate::{INFERNO_TICK_RATE, MAX_CLIENTS, MAX_PACKET_SIZE};

/// Server configuration.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Server tick rate (updates per second).
    pub tick_rate: u32,
    /// Maximum number of concurrent clients.
    pub max_clients: usize,
    /// UDP port to bind.
    pub port: u16,
    /// Address to bind to.
    pub bind_address: SocketAddr,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            tick_rate: INFERNO_TICK_RATE,
            max_clients: MAX_CLIENTS,
            port: 7777,
            bind_address: "0.0.0.0:7777".parse().expect("valid address"),
        }
    }
}

/// Network event from I/O thread.
#[derive(Clone, Debug)]
pub enum NetworkEvent {
    /// New packet received.
    PacketReceived {
        /// Source address.
        addr: SocketAddr,
        /// Packet data.
        data: [u8; MAX_PACKET_SIZE],
        /// Actual length.
        len: usize,
    },
    /// Client connected.
    ClientConnected(SocketAddr),
    /// Client disconnected.
    ClientDisconnected(ConnectionId),
}

/// Command to send to I/O thread.
#[derive(Clone, Debug)]
pub enum NetworkCommand {
    /// Send packet to client.
    Send {
        /// Target address.
        addr: SocketAddr,
        /// Packet data.
        data: [u8; MAX_PACKET_SIZE],
        /// Actual length.
        len: usize,
    },
    /// Broadcast packet to all clients.
    Broadcast {
        /// Packet data.
        data: [u8; MAX_PACKET_SIZE],
        /// Actual length.
        len: usize,
    },
    /// Shutdown server.
    Shutdown,
}

/// The Inferno game server.
///
/// This is the main entry point for running the server.
pub struct InfernoServer {
    /// Server configuration.
    #[allow(dead_code)]
    config: ServerConfig,
    /// Server state.
    state: ServerState,
    /// Channel for receiving network events.
    event_rx: Receiver<NetworkEvent>,
    /// Channel for sending network commands.
    command_tx: Sender<NetworkCommand>,
    /// Running flag.
    running: AtomicBool,
    /// Current tick number.
    tick: AtomicU64,
    /// Number of connected clients.
    client_count: AtomicU32,
}

impl InfernoServer {
    /// Creates a new server with the given configuration.
    #[must_use]
    pub fn new(config: ServerConfig) -> Self {
        // Create channels for lock-free communication
        let (event_tx, event_rx) = bounded(10000);
        let (command_tx, command_rx) = bounded(10000);
        
        // Store for I/O thread to use
        let _ = (event_tx, command_rx); // Will be used by I/O thread
        
        Self {
            config: config.clone(),
            state: ServerState::new(config.max_clients),
            event_rx,
            command_tx,
            running: AtomicBool::new(false),
            tick: AtomicU64::new(0),
            client_count: AtomicU32::new(0),
        }
    }

    /// Returns the current tick number.
    #[inline]
    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.tick.load(Ordering::Relaxed)
    }

    /// Returns the number of connected clients.
    #[inline]
    #[must_use]
    pub fn client_count(&self) -> u32 {
        self.client_count.load(Ordering::Relaxed)
    }

    /// Returns whether the server is running.
    #[inline]
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Returns a reference to the server state.
    #[inline]
    #[must_use]
    pub fn state(&self) -> &ServerState {
        &self.state
    }

    /// Returns a mutable reference to the server state.
    #[inline]
    pub fn state_mut(&mut self) -> &mut ServerState {
        &mut self.state
    }

    /// Processes a single tick.
    ///
    /// This is the hot path - ZERO ALLOCATIONS allowed.
    pub fn tick(&mut self) {
        // 1. Process all pending network events
        while let Ok(event) = self.event_rx.try_recv() {
            self.handle_event(event);
        }

        // 2. Update world state
        self.state.update();

        // 3. Generate and broadcast snapshot
        let snapshot = self.state.generate_snapshot(self.current_tick() as u32);
        self.broadcast_snapshot(&snapshot);

        // 4. Increment tick
        self.tick.fetch_add(1, Ordering::Relaxed);
    }

    /// Handles a network event.
    fn handle_event(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::PacketReceived { addr, data, len } => {
                self.handle_packet(addr, &data[..len]);
            }
            NetworkEvent::ClientConnected(addr) => {
                if let Some(id) = self.state.add_client(addr) {
                    self.client_count.fetch_add(1, Ordering::Relaxed);
                    tracing::info!("Client connected: {} (id: {})", addr, id.0);
                }
            }
            NetworkEvent::ClientDisconnected(id) => {
                self.state.remove_client(id);
                self.client_count.fetch_sub(1, Ordering::Relaxed);
                tracing::info!("Client disconnected: {}", id.0);
            }
        }
    }

    /// Handles a received packet.
    fn handle_packet(&mut self, addr: SocketAddr, data: &[u8]) {
        use crate::protocol::{PacketDeserializer, Packet};
        
        let mut deserializer = PacketDeserializer::new(data);
        
        if let Some(packet) = deserializer.deserialize() {
            match packet {
                Packet::Input(header, input) => {
                    self.handle_input(addr, &header, &input);
                }
                Packet::Connect(_) => {
                    self.handle_connect(addr);
                }
                Packet::Disconnect(_) => {
                    if let Some(id) = self.state.find_client_by_addr(addr) {
                        self.state.remove_client(id);
                        self.client_count.fetch_sub(1, Ordering::Relaxed);
                    }
                }
                Packet::Heartbeat(header) => {
                    if let Some(client) = self.state.find_client_by_addr_mut(addr) {
                        client.update_ack(header.ack, header.ack_bits);
                    }
                }
                _ => {
                    // Server doesn't handle other packet types from clients
                }
            }
        }
    }

    /// Handles player input.
    fn handle_input(&mut self, addr: SocketAddr, _header: &PacketHeader, input: &PlayerInput) {
        if let Some(client) = self.state.find_client_by_addr_mut(addr) {
            client.add_input(*input);
        }
    }

    /// Handles connection request.
    fn handle_connect(&mut self, addr: SocketAddr) {
        if self.state.find_client_by_addr(addr).is_some() {
            // Already connected
            return;
        }

        if let Some(id) = self.state.add_client(addr) {
            self.client_count.fetch_add(1, Ordering::Relaxed);
            
            // Send connect ack
            let mut serializer = crate::protocol::PacketSerializer::new();
            let header = PacketHeader::new(0, 0, 0);
            if serializer.serialize_connect_ack(&header, id.0) {
                let mut data = [0u8; MAX_PACKET_SIZE];
                data[..serializer.len()].copy_from_slice(serializer.as_slice());
                
                let _ = self.command_tx.try_send(NetworkCommand::Send {
                    addr,
                    data,
                    len: serializer.len(),
                });
            }
        }
    }

    /// Broadcasts a snapshot to all clients.
    fn broadcast_snapshot(&self, snapshot: &WorldSnapshot) {
        let mut serializer = crate::protocol::PacketSerializer::new();
        let header = PacketHeader::new(snapshot.tick as u16, 0, 0);
        
        if serializer.serialize_snapshot(&header, snapshot) {
            let mut data = [0u8; MAX_PACKET_SIZE];
            data[..serializer.len()].copy_from_slice(serializer.as_slice());
            
            let _ = self.command_tx.try_send(NetworkCommand::Broadcast {
                data,
                len: serializer.len(),
            });
        }
    }

    /// Sends a command to the I/O thread.
    #[inline]
    pub fn send_command(&self, command: NetworkCommand) -> bool {
        self.command_tx.try_send(command).is_ok()
    }

    /// Shuts down the server.
    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
        let _ = self.command_tx.try_send(NetworkCommand::Shutdown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let config = ServerConfig::default();
        let server = InfernoServer::new(config);
        
        assert!(!server.is_running());
        assert_eq!(server.current_tick(), 0);
        assert_eq!(server.client_count(), 0);
    }

    #[test]
    fn test_server_config() {
        let config = ServerConfig {
            tick_rate: 120,
            max_clients: 100,
            port: 8888,
            bind_address: "127.0.0.1:8888".parse().unwrap(),
        };
        
        assert_eq!(config.tick_rate, 120);
        assert_eq!(config.max_clients, 100);
        assert_eq!(config.port, 8888);
    }
}
