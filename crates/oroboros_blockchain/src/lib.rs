//! # OROBOROS Blockchain Bridge
//!
//! Fast, low-latency integration between Rust game state and Solidity contracts.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐    Events    ┌─────────────────┐
//! │  Solidity       │ ──────────▶  │  EventListener  │
//! │  Contract       │              │  (< 5ms E2E)    │
//! └─────────────────┘              └────────┬────────┘
//!                                           │
//!                                           ▼
//!                                  ┌─────────────────┐
//!                                  │  GameState      │
//!                                  │  (Zero-alloc)   │
//!                                  └─────────────────┘
//! ```
//!
//! ## Performance Requirements
//!
//! - Event parsing: < 1ms
//! - State update: < 1ms
//! - Total E2E: < 5ms

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::perf)]

pub mod contracts;
pub mod events;
pub mod ipc;
pub mod listener;
pub mod state;

pub use contracts::PolymorphicNFT;
pub use events::{BlockchainEvent, NFTStateChange, NFTTransfer};
pub use ipc::{IpcConfig, IpcError, IpcListener, IpcStats};
pub use listener::{EventListener, EventSimulator, ListenerConfig, ListenerStats};
pub use state::ChainSyncedState;
