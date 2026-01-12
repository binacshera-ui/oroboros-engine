//! Widget system for UI components.
//!
//! Widgets are the building blocks of the Bloomberg interface.

mod core;
mod inventory;
mod tooltip;
mod tree;

pub use core::{Widget, WidgetId, WidgetState, WidgetFlags, WidgetResponse, TooltipWidget};
pub use inventory::{InventoryWidget, InventoryItem, InventorySlot};
pub use tooltip::{Tooltip, TooltipManager, TooltipConfig};
pub use tree::WidgetTree;
