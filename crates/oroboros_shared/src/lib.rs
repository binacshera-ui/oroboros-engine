//! # OROBOROS Shared
//!
//! Common types used by both client and server.
//!
//! ## CRITICAL RULE
//!
//! This crate must NEVER depend on:
//! - `wgpu`
//! - `raw-window-handle`
//! - Any GPU or window-related crate
//!
//! If you need graphics types, put them in `oroboros_rendering`.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod constants;
pub mod math;
pub mod protocol;
pub mod events;

pub use constants::{SERVER_IP, SERVER_PORT, SERVER_ADDR, SERVER_BIND, TICK_RATE, MAX_CLIENTS};
pub use math::{Vec3, Vec2, Quaternion, Transform};
pub use protocol::{PacketType, EntityUpdate, BlockUpdate, ItemDrop, DamageEvent};
pub use events::{SharedEvent, EventType};
