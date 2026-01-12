//! # Network Protocol
//!
//! Binary packet definitions with bit-packing for minimal bandwidth.
//!
//! ## Packet Structure
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │ Header (8 bytes)                                             │
//! ├──────────────────────────────────────────────────────────────┤
//! │ Sequence (2) │ Ack (2) │ AckBits (4)                        │
//! ├──────────────────────────────────────────────────────────────┤
//! │ Payload (variable, max 1192 bytes)                           │
//! └──────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Design Philosophy
//!
//! - Every bit counts - we pay per byte in bandwidth
//! - Fixed-size structures where possible for zero-copy
//! - Delta compression for world state
//! - Reliable delivery for critical packets only

mod packets;
mod serialization;
mod compression;

pub use packets::{
    Packet, PacketType, PacketHeader, PlayerInput, WorldSnapshot, 
    DeltaSnapshot, EntityState, DragonState, HitReport, ShotFired,
};
pub use serialization::{
    SequenceNumber, AckBitfield, PacketSerializer, PacketDeserializer,
};
pub use compression::{DeltaCompressor, BitPacker};
