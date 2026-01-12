//! # Economy Error Types
//!
//! All errors that can occur in the economy system.

use thiserror::Error;

/// Errors that can occur in the economy system.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum EconomyError {
    /// Attempted to craft with insufficient materials.
    #[error("insufficient materials: need {required} of item {item_id}, have {available}")]
    InsufficientMaterials {
        /// The item that was missing.
        item_id: u32,
        /// The amount required.
        required: u32,
        /// The amount available.
        available: u32,
    },

    /// Recipe not found in the crafting graph.
    #[error("recipe not found: {0}")]
    RecipeNotFound(u32),

    /// Detected a cycle in the crafting graph (infinite resource generation).
    #[error("cycle detected in crafting graph at recipe {0}")]
    CycleDetected(u32),

    /// Item not found in registry.
    #[error("item not found: {0}")]
    ItemNotFound(u32),

    /// Inventory is full, cannot add more items.
    #[error("inventory full: capacity {capacity}, tried to add {amount}")]
    InventoryFull {
        /// Current capacity.
        capacity: u32,
        /// Amount tried to add.
        amount: u32,
    },

    /// Transaction failed and was rolled back.
    #[error("transaction rolled back: {reason}")]
    TransactionRolledBack {
        /// Reason for rollback.
        reason: String,
    },

    /// Arithmetic overflow in fixed-point calculation.
    #[error("arithmetic overflow in economic calculation")]
    ArithmeticOverflow,

    /// Invalid configuration file.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Database lock contention.
    #[error("database busy, try again")]
    DatabaseBusy,
}

/// Result type for economy operations.
pub type EconomyResult<T> = Result<T, EconomyError>;
