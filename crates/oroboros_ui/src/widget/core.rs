//! Core widget types and traits.

use crate::layout::Rect;
use crate::render::RenderCommand;
use crate::input::InputState;

/// Unique identifier for a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId(pub u64);

impl WidgetId {
    /// Creates a new widget ID.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Returns the raw ID value.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Widget state flags (bitfield for efficiency).
#[derive(Debug, Clone, Copy, Default)]
pub struct WidgetFlags(u32);

impl WidgetFlags {
    /// Widget is visible.
    pub const VISIBLE: u32 = 1 << 0;
    /// Widget is enabled (can receive input).
    pub const ENABLED: u32 = 1 << 1;
    /// Widget is focused.
    pub const FOCUSED: u32 = 1 << 2;
    /// Widget is hovered.
    pub const HOVERED: u32 = 1 << 3;
    /// Widget is pressed.
    pub const PRESSED: u32 = 1 << 4;
    /// Widget needs layout recalculation.
    pub const DIRTY_LAYOUT: u32 = 1 << 5;
    /// Widget needs redraw.
    pub const DIRTY_RENDER: u32 = 1 << 6;
    
    /// Default flags for a new widget.
    pub const DEFAULT: Self = Self(Self::VISIBLE | Self::ENABLED | Self::DIRTY_LAYOUT | Self::DIRTY_RENDER);
    
    /// Creates new flags with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self::DEFAULT
    }
    
    /// Returns true if the flag is set.
    #[inline]
    #[must_use]
    pub const fn has(self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }
    
    /// Sets a flag.
    #[inline]
    pub fn set(&mut self, flag: u32) {
        self.0 |= flag;
    }
    
    /// Clears a flag.
    #[inline]
    pub fn clear(&mut self, flag: u32) {
        self.0 &= !flag;
    }
    
    /// Toggles a flag.
    #[inline]
    pub fn toggle(&mut self, flag: u32) {
        self.0 ^= flag;
    }
}

/// Common widget state.
#[derive(Debug, Clone)]
pub struct WidgetState {
    /// Widget identifier.
    pub id: WidgetId,
    /// Bounding rectangle (set after layout).
    pub rect: Rect,
    /// State flags.
    pub flags: WidgetFlags,
    /// Z-index for layering.
    pub z_index: i32,
    /// Parent widget ID (None for root).
    pub parent: Option<WidgetId>,
}

impl WidgetState {
    /// Creates a new widget state.
    #[must_use]
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            rect: Rect::ZERO,
            flags: WidgetFlags::DEFAULT,
            z_index: 0,
            parent: None,
        }
    }
    
    /// Returns true if the widget is visible.
    #[inline]
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.flags.has(WidgetFlags::VISIBLE)
    }
    
    /// Returns true if the widget is hovered.
    #[inline]
    #[must_use]
    pub fn is_hovered(&self) -> bool {
        self.flags.has(WidgetFlags::HOVERED)
    }
    
    /// Returns true if the widget is pressed.
    #[inline]
    #[must_use]
    pub fn is_pressed(&self) -> bool {
        self.flags.has(WidgetFlags::PRESSED)
    }
    
    /// Marks the widget as needing redraw.
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.flags.set(WidgetFlags::DIRTY_RENDER);
    }
}

/// Response from widget update.
#[derive(Debug, Clone, Copy, Default)]
pub struct WidgetResponse {
    /// Widget was clicked.
    pub clicked: bool,
    /// Widget was double-clicked.
    pub double_clicked: bool,
    /// Widget gained focus.
    pub focused: bool,
    /// Widget lost focus.
    pub unfocused: bool,
    /// Widget was hovered (just entered).
    pub hovered: bool,
    /// Widget was unhovered (just left).
    pub unhovered: bool,
    /// Widget value changed.
    pub changed: bool,
}

/// Base trait for all widgets.
pub trait Widget {
    /// Returns the widget's state.
    fn state(&self) -> &WidgetState;
    
    /// Returns mutable access to the widget's state.
    fn state_mut(&mut self) -> &mut WidgetState;
    
    /// Handles input and updates widget state.
    ///
    /// This is called EVERY frame, even without input events.
    /// MANDATE: Must complete in <1μs for instant tooltip response.
    fn update(&mut self, input: &InputState, dt: f32) -> WidgetResponse;
    
    /// Generates render commands for this widget.
    ///
    /// Called only when DIRTY_RENDER is set.
    fn render(&self, commands: &mut Vec<RenderCommand>);
    
    /// Returns the minimum size of this widget.
    fn min_size(&self) -> (f32, f32) {
        (0.0, 0.0)
    }
    
    /// Returns the preferred size of this widget.
    fn preferred_size(&self) -> (f32, f32) {
        self.min_size()
    }
}

/// Widget with tooltip support.
pub trait TooltipWidget: Widget {
    /// Returns the tooltip text for this widget.
    ///
    /// Called on EVERY frame when hovered - must be instant.
    /// Return None for no tooltip.
    fn tooltip_text(&self) -> Option<&str>;
}
