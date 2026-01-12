//! # OROBOROS
//!
//! The main game crate, integrating all systems.
//!
//! ## Architecture (The Four Units)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                         OROBOROS GAME ENGINE                            │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │  ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐   │
//! │  │   UNIT 1        │     │   UNIT 2        │     │   UNIT 3        │   │
//! │  │   Core Kernel   │────>│   Neon (GFX)    │     │   Veridia       │   │
//! │  │                 │     │                 │<────│   (Economy)     │   │
//! │  │  • ECS          │     │  • Voxels       │     │  • Loot         │   │
//! │  │  • Double Buffer│     │  • Particles    │     │  • WAL          │   │
//! │  │  • Memory       │     │  • UI           │     │  • Crafting     │   │
//! │  └────────┬────────┘     └─────────────────┘     └────────┬────────┘   │
//! │           │                                               │            │
//! │           │              ┌─────────────────┐              │            │
//! │           └─────────────>│   UNIT 4        │<─────────────┘            │
//! │                          │   Inferno       │                           │
//! │                          │   (Networking)  │                           │
//! │                          │                 │                           │
//! │                          │  • UDP Protocol │                           │
//! │                          │  • Prediction   │                           │
//! │                          │  • Physics      │                           │
//! │                          └─────────────────┘                           │
//! │                                                                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Modules
//!
//! - `events`: Inter-unit event system
//! - `game_loop`: Frame orchestration and timing
//! - `integration`: Vertical slice tests

pub mod events;
pub mod game_loop;
pub mod gameplay;
pub mod integration;
pub mod physics;

// Re-export the units
pub use oroboros_core as core;
pub use oroboros_economy as economy;

#[cfg(feature = "rendering")]
pub use oroboros_rendering as rendering;

#[cfg(feature = "networking")]
pub use oroboros_networking as networking;

#[cfg(feature = "security")]
pub use oroboros_security as security;

// Re-export commonly used types
pub use events::{EventBus, EventSender, EventReceiver, EventSystem, GameEvent};
pub use game_loop::{GameLoop, GameLoopConfig, FrameStats, FrameContext, RenderContext};
