//! # OROBOROS Economy System
//!
//! Pure Rust economic logic for the OROBOROS game engine.
//!
//! ## Design Principles
//!
//! 1. **Zero floating point** - All monetary calculations use fixed-point (u64 with implicit decimals)
//! 2. **O(1) loot calculations** - No loops during mining/combat
//! 3. **Transactional crafting** - All-or-nothing item transformations
//! 4. **External configuration** - All balance data in TOML files
//!
//! ## Thread Safety
//!
//! The economy system is designed to be called from the authoritative server.
//! Client-side calculations are untrusted and ignored.
//!
//! ## Example
//!
//! ```rust,ignore
//! use oroboros_economy::{LootTable, CraftingGraph, FixedPoint};
//!
//! // Load loot tables from config
//! let loot = LootTable::from_toml("data/schemas/economy/loot.toml")?;
//!
//! // Calculate drop in O(1) time
//! let drop = loot.calculate_drop(
//!     player_level,
//!     pickaxe_tier,
//!     block_rarity,
//!     weather_seed,
//!     blockchain_entropy,
//! );
//! ```

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::perf)]

pub mod crafting;
pub mod error;
pub mod fixed_point;
pub mod inventory;
pub mod loot;
pub mod systems;
pub mod wal;
pub mod wal_batched;

pub use crafting::{CraftingGraph, Recipe, RecipeId};
pub use error::EconomyError;
pub use fixed_point::{FixedPoint, FixedPoint18};
pub use inventory::{Inventory, Item, ItemId, ItemStack};
pub use loot::{BlockchainSalt, DropResult, LootCalculator, LootTable, Rarity, SecureSeed};
pub use systems::{EconomySystem, TransactionResult};
pub use wal::{WalOperation, WriteAheadLog};
pub use wal_batched::{BatchedWal, BatchedWalConfig, WalHandle, WalStats};
pub mod integration;

pub use integration::{
    BlockBreakResult, CraftResult, DropRarity, EconomyEvent, ItemDrop, TheBank,
};
