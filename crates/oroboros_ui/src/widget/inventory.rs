//! Inventory widget - the core trading interface.
//!
//! Displays items with instant tooltips showing value, stats, and market data.

use super::{Widget, WidgetId, WidgetState, WidgetResponse, TooltipWidget};
use crate::layout::Rect;
use crate::render::RenderCommand;
use crate::input::InputState;
use crate::style::Color;
use crate::animation::{Animation, Easing};

/// An item in the inventory.
#[derive(Debug, Clone)]
pub struct InventoryItem {
    /// Item ID.
    pub id: u64,
    /// Display name.
    pub name: String,
    /// Material/icon ID for rendering.
    pub icon_id: u32,
    /// Stack count.
    pub quantity: u32,
    /// Current market value.
    pub value: f64,
    /// Value change (positive = profit, negative = loss).
    pub value_change: f64,
    /// Rarity tier (0-5).
    pub rarity: u8,
}

impl InventoryItem {
    /// Creates a new inventory item.
    #[must_use]
    pub fn new(id: u64, name: impl Into<String>, icon_id: u32) -> Self {
        Self {
            id,
            name: name.into(),
            icon_id,
            quantity: 1,
            value: 0.0,
            value_change: 0.0,
            rarity: 0,
        }
    }
    
    /// Returns the color based on rarity.
    #[must_use]
    pub fn rarity_color(&self) -> Color {
        match self.rarity {
            0 => Color::rgba(0.6, 0.6, 0.6, 1.0),    // Common - gray
            1 => Color::rgba(0.2, 0.8, 0.2, 1.0),    // Uncommon - green
            2 => Color::rgba(0.2, 0.4, 1.0, 1.0),    // Rare - blue
            3 => Color::rgba(0.8, 0.2, 0.8, 1.0),    // Epic - purple
            4 => Color::rgba(1.0, 0.6, 0.0, 1.0),    // Legendary - orange
            _ => Color::rgba(1.0, 0.2, 0.2, 1.0),    // Mythic - red
        }
    }
    
    /// Formats the tooltip text.
    #[must_use]
    pub fn tooltip(&self) -> String {
        let change_indicator = if self.value_change > 0.0 {
            "▲"
        } else if self.value_change < 0.0 {
            "▼"
        } else {
            "─"
        };
        
        format!(
            "{}\n\
             ─────────────────\n\
             Qty: {:>12}\n\
             Val: {:>10.2} ORO\n\
             Chg: {} {:>+8.2}%\n\
             ─────────────────\n\
             [CLICK TO TRADE]",
            self.name,
            self.quantity,
            self.value,
            change_indicator,
            self.value_change * 100.0
        )
    }
}

/// Single inventory slot widget.
pub struct InventorySlot {
    /// Widget state.
    state: WidgetState,
    /// Item in this slot (if any).
    item: Option<InventoryItem>,
    /// Slot size.
    size: f32,
    /// Hover animation.
    hover_anim: Animation,
    /// Selected state.
    selected: bool,
}

impl InventorySlot {
    /// Standard slot size.
    pub const DEFAULT_SIZE: f32 = 48.0;
    
    /// Creates a new inventory slot.
    #[must_use]
    pub fn new(id: WidgetId) -> Self {
        Self {
            state: WidgetState::new(id),
            item: None,
            size: Self::DEFAULT_SIZE,
            hover_anim: Animation::new(0.0, Easing::ExponentialOut),
            selected: false,
        }
    }
    
    /// Sets the item in this slot.
    pub fn set_item(&mut self, item: Option<InventoryItem>) {
        self.item = item;
        self.state.mark_dirty();
    }
    
    /// Returns the item in this slot.
    #[must_use]
    pub fn item(&self) -> Option<&InventoryItem> {
        self.item.as_ref()
    }
    
    /// Sets the selected state.
    pub fn set_selected(&mut self, selected: bool) {
        if self.selected != selected {
            self.selected = selected;
            self.state.mark_dirty();
        }
    }
}

impl Widget for InventorySlot {
    fn state(&self) -> &WidgetState {
        &self.state
    }
    
    fn state_mut(&mut self) -> &mut WidgetState {
        &mut self.state
    }
    
    fn update(&mut self, input: &InputState, dt: f32) -> WidgetResponse {
        let mut response = WidgetResponse::default();
        
        let was_hovered = self.state.is_hovered();
        let is_hovered = self.state.rect.contains(input.mouse_x, input.mouse_y);
        
        // Update hover state
        if is_hovered != was_hovered {
            if is_hovered {
                self.state.flags.set(super::WidgetFlags::HOVERED);
                response.hovered = true;
            } else {
                self.state.flags.clear(super::WidgetFlags::HOVERED);
                response.unhovered = true;
            }
            self.state.mark_dirty();
        }
        
        // Update hover animation - FAST exponential
        let target = if is_hovered { 1.0 } else { 0.0 };
        self.hover_anim.set_target(target);
        self.hover_anim.update(dt * 8.0); // 8x speed for snappy response
        
        // Handle clicks
        if is_hovered && input.mouse_clicked(crate::input::MouseButton::Left) {
            response.clicked = true;
        }
        
        response
    }
    
    fn render(&self, commands: &mut Vec<RenderCommand>) {
        let rect = self.state.rect;
        let hover_t = self.hover_anim.value();
        
        // Background
        let bg_color = if self.selected {
            Color::rgba(0.2, 0.4, 0.3, 0.9)
        } else {
            Color::rgba(0.08, 0.08, 0.12, 0.9).lerp(
                Color::rgba(0.12, 0.12, 0.18, 0.95),
                hover_t,
            )
        };
        
        commands.push(RenderCommand::Rect {
            bounds: rect,
            color: bg_color,
            corner_radius: 4.0,
        });
        
        // Border
        let border_color = if self.selected {
            Color::rgba(0.2, 1.0, 0.4, 1.0) // Green for selected
        } else if let Some(item) = &self.item {
            item.rarity_color().with_alpha(0.5 + hover_t * 0.5)
        } else {
            Color::rgba(0.3, 0.3, 0.3, 0.5 + hover_t * 0.3)
        };
        
        commands.push(RenderCommand::RectOutline {
            bounds: rect,
            color: border_color,
            width: 1.0 + hover_t,
            corner_radius: 4.0,
        });
        
        // Item icon
        if let Some(item) = &self.item {
            let icon_padding = 4.0;
            let icon_rect = Rect::new(
                rect.x + icon_padding,
                rect.y + icon_padding,
                rect.width - icon_padding * 2.0,
                rect.height - icon_padding * 2.0,
            );
            
            commands.push(RenderCommand::Icon {
                bounds: icon_rect,
                icon_id: item.icon_id,
                color: Color::WHITE,
            });
            
            // Quantity badge (if > 1)
            if item.quantity > 1 {
                let badge_text = if item.quantity > 999 {
                    format!("{}K", item.quantity / 1000)
                } else {
                    item.quantity.to_string()
                };
                
                commands.push(RenderCommand::Text {
                    text: badge_text,
                    x: rect.x + rect.width - 4.0,
                    y: rect.y + rect.height - 4.0,
                    color: Color::WHITE,
                    font_size: 10.0,
                    monospace: true,
                });
            }
            
            // Value change indicator
            if item.value_change.abs() > 0.001 {
                let indicator_color = if item.value_change > 0.0 {
                    Color::rgba(0.2, 1.0, 0.3, 1.0) // Green for profit
                } else {
                    Color::rgba(1.0, 0.2, 0.2, 1.0) // Red for loss
                };
                
                let indicator = if item.value_change > 0.0 { "▲" } else { "▼" };
                
                commands.push(RenderCommand::Text {
                    text: indicator.to_string(),
                    x: rect.x + 2.0,
                    y: rect.y + 2.0,
                    color: indicator_color,
                    font_size: 8.0,
                    monospace: true,
                });
            }
        }
    }
    
    fn min_size(&self) -> (f32, f32) {
        (self.size, self.size)
    }
    
    fn preferred_size(&self) -> (f32, f32) {
        (self.size, self.size)
    }
}

impl TooltipWidget for InventorySlot {
    fn tooltip_text(&self) -> Option<&str> {
        // Note: In production, we'd cache this string
        // For now, returning None and handling tooltip separately
        None
    }
}

/// Complete inventory grid widget.
pub struct InventoryWidget {
    /// Widget state.
    state: WidgetState,
    /// Grid of slots.
    slots: Vec<InventorySlot>,
    /// Number of columns.
    columns: usize,
    /// Currently selected slot index.
    selected: Option<usize>,
    /// Open/close animation.
    open_anim: Animation,
    /// Is the inventory open?
    is_open: bool,
}

impl InventoryWidget {
    /// Creates a new inventory widget.
    #[must_use]
    pub fn new(id: WidgetId, rows: usize, columns: usize) -> Self {
        let slot_count = rows * columns;
        let mut slots = Vec::with_capacity(slot_count);
        
        for i in 0..slot_count {
            let slot_id = WidgetId::new(id.raw() * 1000 + i as u64);
            slots.push(InventorySlot::new(slot_id));
        }
        
        Self {
            state: WidgetState::new(id),
            slots,
            columns,
            selected: None,
            open_anim: Animation::new(0.0, Easing::ExponentialOut),
            is_open: false,
        }
    }
    
    /// Opens the inventory.
    pub fn open(&mut self) {
        self.is_open = true;
        self.open_anim.set_target(1.0);
        self.state.mark_dirty();
    }
    
    /// Closes the inventory.
    pub fn close(&mut self) {
        self.is_open = false;
        self.open_anim.set_target(0.0);
        self.state.mark_dirty();
    }
    
    /// Toggles the inventory.
    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }
    
    /// Sets an item in a slot.
    pub fn set_item(&mut self, slot_index: usize, item: Option<InventoryItem>) {
        if let Some(slot) = self.slots.get_mut(slot_index) {
            slot.set_item(item);
        }
    }
    
    /// Gets the hovered item's tooltip.
    #[must_use]
    pub fn hovered_tooltip(&self) -> Option<String> {
        self.slots.iter()
            .find(|s| s.state.is_hovered())
            .and_then(|s| s.item.as_ref())
            .map(|item| item.tooltip())
    }
    
    /// Calculates layout for all slots.
    pub fn layout(&mut self, x: f32, y: f32) {
        let padding = 8.0;
        let slot_size = InventorySlot::DEFAULT_SIZE;
        let gap = 4.0;
        
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let col = i % self.columns;
            let row = i / self.columns;
            
            let slot_x = x + padding + col as f32 * (slot_size + gap);
            let slot_y = y + padding + row as f32 * (slot_size + gap);
            
            slot.state_mut().rect = Rect::new(slot_x, slot_y, slot_size, slot_size);
        }
        
        let rows = (self.slots.len() + self.columns - 1) / self.columns;
        let width = padding * 2.0 + self.columns as f32 * (slot_size + gap) - gap;
        let height = padding * 2.0 + rows as f32 * (slot_size + gap) - gap;
        
        self.state.rect = Rect::new(x, y, width, height);
    }
}

impl Widget for InventoryWidget {
    fn state(&self) -> &WidgetState {
        &self.state
    }
    
    fn state_mut(&mut self) -> &mut WidgetState {
        &mut self.state
    }
    
    fn update(&mut self, input: &InputState, dt: f32) -> WidgetResponse {
        let mut response = WidgetResponse::default();
        
        // Update open animation - SHARP exponential
        self.open_anim.update(dt * 10.0); // Very fast
        
        if !self.is_open && self.open_anim.value() < 0.01 {
            return response;
        }
        
        // Update all slots and collect which one was clicked
        let mut clicked_index: Option<usize> = None;
        
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let slot_response = slot.update(input, dt);
            
            if slot_response.clicked {
                clicked_index = Some(i);
            }
        }
        
        // Handle selection change after iteration
        if let Some(new_selection) = clicked_index {
            // Deselect previous
            if let Some(prev) = self.selected {
                if prev < self.slots.len() {
                    self.slots[prev].set_selected(false);
                }
            }
            
            // Select new
            self.selected = Some(new_selection);
            if new_selection < self.slots.len() {
                self.slots[new_selection].set_selected(true);
            }
            response.changed = true;
        }
        
        response
    }
    
    fn render(&self, commands: &mut Vec<RenderCommand>) {
        let open_t = self.open_anim.value();
        
        if open_t < 0.01 {
            return;
        }
        
        // Background panel
        let rect = self.state.rect;
        let animated_rect = Rect::new(
            rect.x,
            rect.y - (1.0 - open_t) * 20.0, // Slide in from above
            rect.width,
            rect.height * open_t, // Scale height
        );
        
        commands.push(RenderCommand::Rect {
            bounds: animated_rect,
            color: Color::rgba(0.03, 0.03, 0.05, 0.95 * open_t),
            corner_radius: 8.0,
        });
        
        // Border (neon green)
        commands.push(RenderCommand::RectOutline {
            bounds: animated_rect,
            color: Color::rgba(0.2, 0.8, 0.3, 0.8 * open_t),
            width: 1.0,
            corner_radius: 8.0,
        });
        
        // Header
        commands.push(RenderCommand::Text {
            text: "INVENTORY".to_string(),
            x: rect.x + 8.0,
            y: rect.y + 4.0,
            color: Color::rgba(0.5, 0.8, 0.5, open_t),
            font_size: 12.0,
            monospace: true,
        });
        
        // Render slots
        if open_t > 0.5 {
            for slot in &self.slots {
                slot.render(commands);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_inventory_creation() {
        let inv = InventoryWidget::new(WidgetId::new(1), 4, 8);
        assert_eq!(inv.slots.len(), 32);
    }
    
    #[test]
    fn test_item_tooltip() {
        let mut item = InventoryItem::new(1, "Plasma Sword", 42);
        item.value = 1234.56;
        item.value_change = 0.05;
        item.quantity = 3;
        
        let tooltip = item.tooltip();
        assert!(tooltip.contains("Plasma Sword"));
        assert!(tooltip.contains("1234.56"));
    }
}
