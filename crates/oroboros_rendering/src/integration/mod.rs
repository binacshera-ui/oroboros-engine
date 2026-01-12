//! # Rendering Integration Layer
//!
//! OPERATION COLD FUSION: Connecting Unit 2 to the organism.
//!
//! ## Unit 2's Role
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    DATA FLOW HIERARCHY                          │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │   Unit 1 (Core)     ──── Memory Owner (Double Buffer) ───┐     │
//! │   Unit 4 (Network)  ──── Primary Writer ─────────────────┤     │
//! │   Unit 3 (Economy)  ──── Auditor ────────────────────────┤     │
//! │                                                          ▼     │
//! │   Unit 2 (Render)   ◄─── READER ◄── Buffer B ◄───────────┘     │
//! │        │                                                        │
//! │        └──► GPU ──► Screen                                     │
//! │                                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Rules
//!
//! 1. We only READ from Buffer B (never write to Core's data)
//! 2. We receive EVENTS from Unit 4 (network) and Unit 3 (economy)
//! 3. Events trigger visual effects (particles, UI, screen flash)
//! 4. Target: Event → Visual in < 16ms (same frame)

pub mod render_bridge;
mod game_events;
mod render_loop;
mod core_adapter;

pub use render_bridge::{RenderBridge, RenderBridgeConfig, WorldReader};
pub use game_events::{
    GameEvent, GameEventQueue, EventCategory,
    BlockBreakEvent, ItemDropEvent, DamageEvent, DeathEvent,
};
pub use render_loop::{RenderLoop, RenderLoopConfig, FrameResult};
pub use core_adapter::CoreWorldReader;
