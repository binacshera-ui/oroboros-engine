//! # OROBOROS Networking - The Ghost Protocol
//!
//! Zero-latency, lock-free networking infrastructure for high-stakes PvP combat.
//!
//! ## Architecture
//!
//! This crate implements the complete networking stack for OROBOROS:
//!
//! - **Protocol**: Custom binary protocol with bit-packing (< 1200 bytes MTU)
//! - **Transport**: UDP with reliability layer for critical packets
//! - **Synchronization**: Snapshot interpolation with delta compression
//! - **Prediction**: Client-side prediction with server reconciliation
//! - **Authority**: Server is the single source of truth (Trust No One)
//!
//! ## Performance Guarantees
//!
//! - Zero heap allocations in the hot path (tick loop)
//! - Lock-free data structures for inter-thread communication
//! - Sub-millisecond packet processing latency
//! - 60Hz+ server tick rate
//!
//! ## Security Model
//!
//! ```text
//! CLIENT                           SERVER
//!   |                                 |
//!   |--- Input: "I shot at X" ------->|
//!   |                                 | <- Server validates
//!   |<-- Response: "Hit/Miss" --------|
//!   |                                 |
//! ```
//!
//! The client NEVER determines outcomes. The server ALWAYS validates.
//!
//! ## Example
//!
//! ```rust,ignore
//! use oroboros_networking::{InfernoServer, ServerConfig};
//!
//! let config = ServerConfig {
//!     tick_rate: 60,
//!     max_clients: 500,
//!     port: 7777,
//! };
//!
//! let server = InfernoServer::new(config);
//! server.run(); // Blocks, runs at 60Hz
//! ```

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::perf)]

pub mod protocol;
pub mod server;
pub mod client;
pub mod snapshot;
pub mod prediction;
pub mod simulation;
pub mod transport;
pub mod interpolation;
pub mod integration;

// Re-exports for convenience
pub use protocol::{
    Packet, PacketType, PlayerInput, WorldSnapshot, DeltaSnapshot,
    PacketHeader, SequenceNumber, AckBitfield,
};
pub use server::{InfernoServer, ServerConfig, ClientConnection, ConnectionId};
pub use client::{GameClient, ClientConfig, ClientState};
pub use snapshot::{SnapshotBuffer, InterpolationState, SnapshotCompressor};
pub use prediction::{PredictionBuffer, InputBuffer, ReconciliationResult};
pub use simulation::{BotSimulation, SimulationConfig, NetworkConditions};
pub use interpolation::{VisualInterpolator, SnapshotInterpolator, PlayerVisualState, InterpolationMode};

/// Network tick rate for Inferno (updates per second).
/// 
/// At 60Hz, each tick is ~16.67ms.
/// This provides smooth combat while being achievable on moderate hardware.
pub const INFERNO_TICK_RATE: u32 = 60;

/// Maximum Transmission Unit - packets must be smaller than this.
/// 
/// We use 1200 bytes to be safe across all networks (< 1500 MTU).
pub const MAX_PACKET_SIZE: usize = 1200;

/// Maximum number of simultaneous clients in Inferno.
pub const MAX_CLIENTS: usize = 500;

/// Server tick duration in microseconds (60Hz = 16,666 μs).
pub const TICK_DURATION_MICROS: u64 = 1_000_000 / INFERNO_TICK_RATE as u64;
