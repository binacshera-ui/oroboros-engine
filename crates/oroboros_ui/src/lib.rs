//! # OROBOROS UI System
//!
//! Zero-lag Bloomberg-style interface designed for:
//! - Instant tooltip response (Frame 1)
//! - Sharp exponential animations
//! - Military-grade information density
//! - Monospace font rendering
//!
//! ## Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────┐
//! │                     UI PIPELINE                         │
//! ├────────────────────────────────────────────────────────┤
//! │  Input Events → Widget Tree → Layout → Render Commands │
//! │       ↓              ↓            ↓           ↓        │
//! │  Hit Testing    State Update   Batching   GPU Submit   │
//! └────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Design Philosophy
//!
//! This is NOT a game UI. This is a **financial terminal**.
//! - Information density over aesthetics
//! - Instant feedback over smooth animations
//! - Precision over hand-holding

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::perf)]

pub mod widget;
pub mod layout;
pub mod animation;
pub mod render;
pub mod input;
pub mod style;

pub use widget::{Widget, WidgetId, WidgetTree};
pub use layout::{Layout, Rect, Constraints};
pub use animation::{Animation, Easing};
pub use render::{UIRenderer, RenderCommand, UIBatch};
pub use input::{InputState, MouseButton, Key};
pub use style::{Style, Theme, Color};
