//! # OROBOROS Security - The Black Box
//!
//! Anti-cheat and replay system for server-side validation.
//!
//! ## Features
//!
//! - **Replay Recording**: Binary stream of all player inputs
//! - **Deterministic Playback**: Perfect recreation of any fight
//! - **Anti-Cheat Detection**: Aimbot, speedhack, position tampering
//! - **Hitbox Verification**: Compare visual vs actual hit detection
//!
//! ## Architecture
//!
//! ```text
//! LIVE GAME                        REPLAY SYSTEM
//!     │                                │
//!     │─── Player Input ──────────────►│ Record
//!     │─── Server State ─────────────►│
//!     │                                │
//!     │                                ▼
//!     │                         ┌──────────────┐
//!     │                         │ Binary File  │
//!     │                         │ (Compressed) │
//!     │                         └──────────────┘
//!     │                                │
//!     │◄─── Playback ─────────────────┤ Analyze
//!     │                                │
//! ```

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::perf)]

pub mod replay;
pub mod anti_cheat;
pub mod validation;

pub use replay::{ReplayRecorder, ReplayPlayer, ReplayHeader, ReplayFrame};
pub use anti_cheat::{CheatDetector, CheatReport, CheatType};
pub use validation::{HitboxValidator, ValidationResult};
