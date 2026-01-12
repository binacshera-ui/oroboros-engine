//! # Cross-Unit Integration Layer
//!
//! OPERATION COLD FUSION - Unit 4's integration module.
//!
//! This module defines the interfaces and event system for communication
//! between all four units:
//!
//! - **Unit 1 (Core)**: Memory Owner - ECS, Double Buffer
//! - **Unit 2 (Neon)**: Reader - Rendering, Particles, UI  
//! - **Unit 3 (Veridia)**: Auditor - Economy, WAL, Loot
//! - **Unit 4 (Inferno)**: Primary Writer - Network, Physics, Game Loop
//!
//! ## Data Flow Architecture
//!
//! ```text
//! Player Input → Unit 4 (Predict) → Unit 4 Server (Validate)
//!                                        ↓
//!                                  Unit 3 (Economy)
//!                                        ↓
//!                                  Unit 1 (Memory)
//!                                        ↓
//!                                  Unit 4 (Broadcast)
//!                                        ↓
//!                                  Unit 2 (Render)
//! ```

pub mod events;
pub mod traits;
pub mod game_loop;
pub mod actions;

pub use events::*;
pub use traits::*;
pub use game_loop::*;
pub use actions::*;
