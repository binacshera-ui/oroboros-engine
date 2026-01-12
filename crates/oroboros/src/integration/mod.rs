//! # Vertical Slice Integration
//!
//! THE ARCHITECT'S MISSION: Full combat loop in < 50ms RTT.
//!
//! ```text
//! ┌────────────┐      ┌────────────┐      ┌────────────┐      ┌────────────┐
//! │  CLIENT    │─UDP─>│  NETWORK   │─────>│  PHYSICS   │─────>│  ECONOMY   │
//! │  Attack!   │      │  (Unit 4)  │      │  (Hit?)    │      │  (Unit 3)  │
//! └────────────┘      └────────────┘      └────────────┘      └────────────┘
//!       ^                                        │                   │
//!       │                                        │                   │
//!       └──────────────< 50ms RTT <──────────────┴───────────────────┘
//! ```

pub mod combat;
pub mod vertical_slice;

pub use combat::{
    AttackCommand, AttackResult, CombatProcessor, HitInfo,
    DamageType, LootDrop, LootRarity, ServerEntity,
};

pub use vertical_slice::{
    VerticalSliceServer, VerticalSliceClient, VerticalSliceConfig,
    SliceMetrics, run_vertical_slice_test,
};
