//! Instant tooltip system.
//!
//! ARCHITECT'S MANDATE: Tooltips appear on Frame 1.
//! Not Frame 2. Not "after a delay". FRAME ONE.

use crate::layout::Rect;
use crate::render::RenderCommand;
use crate::style::Color;

/// Configuration for tooltip rendering.
#[derive(Debug, Clone, Copy)]
pub struct TooltipConfig {
    /// Background color.
    pub background: Color,
    /// Border color.
    pub border: Color,
    /// Text color.
    pub text: Color,
    /// Padding inside tooltip.
    pub padding: f32,
    /// Border width.
    pub border_width: f32,
    /// Offset from cursor.
    pub cursor_offset: (f32, f32),
    /// Maximum width before wrapping.
    pub max_width: f32,
}

impl Default for TooltipConfig {
    fn default() -> Self {
        Self {
            background: Color::rgba(0.05, 0.05, 0.08, 0.95),
            border: Color::rgba(0.2, 0.8, 0.4, 1.0), // Neon green border
            text: Color::rgba(0.9, 0.9, 0.9, 1.0),
            padding: 8.0,
            border_width: 1.0,
            cursor_offset: (12.0, 12.0),
            max_width: 300.0,
        }
    }
}

/// A tooltip to be displayed.
#[derive(Debug, Clone)]
pub struct Tooltip {
    /// Text content.
    pub text: String,
    /// Position (screen coordinates).
    pub position: (f32, f32),
    /// Calculated bounds after layout.
    pub bounds: Rect,
    /// Configuration.
    pub config: TooltipConfig,
}

impl Tooltip {
    /// Creates a new tooltip.
    #[must_use]
    pub fn new(text: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            text: text.into(),
            position: (x, y),
            bounds: Rect::ZERO,
            config: TooltipConfig::default(),
        }
    }
    
    /// Sets custom configuration.
    #[must_use]
    pub fn with_config(mut self, config: TooltipConfig) -> Self {
        self.config = config;
        self
    }
    
    /// Calculates the tooltip bounds.
    pub fn layout(&mut self, screen_width: f32, screen_height: f32) {
        // Estimate text size (monospace: ~8px per character, 16px line height)
        let char_width = 8.0;
        let line_height = 16.0;
        
        let text_width = (self.text.len() as f32 * char_width).min(self.config.max_width);
        let lines = (self.text.len() as f32 * char_width / self.config.max_width).ceil().max(1.0);
        let text_height = lines * line_height;
        
        let width = text_width + self.config.padding * 2.0;
        let height = text_height + self.config.padding * 2.0;
        
        // Position near cursor, but keep on screen
        let mut x = self.position.0 + self.config.cursor_offset.0;
        let mut y = self.position.1 + self.config.cursor_offset.1;
        
        // Clamp to screen bounds
        if x + width > screen_width {
            x = self.position.0 - width - self.config.cursor_offset.0;
        }
        if y + height > screen_height {
            y = self.position.1 - height - self.config.cursor_offset.1;
        }
        
        x = x.max(0.0);
        y = y.max(0.0);
        
        self.bounds = Rect::new(x, y, width, height);
    }
    
    /// Generates render commands for this tooltip.
    pub fn render(&self, commands: &mut Vec<RenderCommand>) {
        // Background
        commands.push(RenderCommand::Rect {
            bounds: self.bounds,
            color: self.config.background,
            corner_radius: 2.0,
        });
        
        // Border
        commands.push(RenderCommand::RectOutline {
            bounds: self.bounds,
            color: self.config.border,
            width: self.config.border_width,
            corner_radius: 2.0,
        });
        
        // Text
        let text_x = self.bounds.x + self.config.padding;
        let text_y = self.bounds.y + self.config.padding;
        
        commands.push(RenderCommand::Text {
            text: self.text.clone(),
            x: text_x,
            y: text_y,
            color: self.config.text,
            font_size: 14.0,
            monospace: true,
        });
    }
}

/// Manages active tooltips.
///
/// CRITICAL: No delays, no animations for showing.
/// Tooltip appears IMMEDIATELY when hovered.
pub struct TooltipManager {
    /// Currently active tooltip (if any).
    current: Option<Tooltip>,
    /// Screen dimensions for clamping.
    screen_size: (f32, f32),
    /// Default configuration.
    config: TooltipConfig,
}

impl TooltipManager {
    /// Creates a new tooltip manager.
    #[must_use]
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            current: None,
            screen_size: (screen_width, screen_height),
            config: TooltipConfig::default(),
        }
    }
    
    /// Updates screen size.
    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.screen_size = (width, height);
    }
    
    /// Shows a tooltip immediately.
    ///
    /// MANDATE: This must be called and processed in the SAME FRAME
    /// as the hover event. No delays.
    pub fn show(&mut self, text: &str, cursor_x: f32, cursor_y: f32) {
        let mut tooltip = Tooltip::new(text, cursor_x, cursor_y);
        tooltip.config = self.config;
        tooltip.layout(self.screen_size.0, self.screen_size.1);
        self.current = Some(tooltip);
    }
    
    /// Hides the current tooltip.
    pub fn hide(&mut self) {
        self.current = None;
    }
    
    /// Returns true if a tooltip is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.current.is_some()
    }
    
    /// Returns the current tooltip.
    #[must_use]
    pub fn current(&self) -> Option<&Tooltip> {
        self.current.as_ref()
    }
    
    /// Generates render commands for the active tooltip.
    pub fn render(&self, commands: &mut Vec<RenderCommand>) {
        if let Some(tooltip) = &self.current {
            tooltip.render(commands);
        }
    }
}

impl Default for TooltipManager {
    fn default() -> Self {
        Self::new(1920.0, 1080.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tooltip_instant_show() {
        let mut manager = TooltipManager::new(1920.0, 1080.0);
        
        // Show must be instant - no frame delay
        manager.show("Test tooltip", 100.0, 100.0);
        
        assert!(manager.is_active());
        assert!(manager.current().is_some());
    }
    
    #[test]
    fn test_tooltip_screen_clamping() {
        let mut tooltip = Tooltip::new("Test", 1900.0, 1060.0);
        tooltip.layout(1920.0, 1080.0);
        
        // Should be clamped to stay on screen
        assert!(tooltip.bounds.x + tooltip.bounds.width <= 1920.0);
        assert!(tooltip.bounds.y + tooltip.bounds.height <= 1080.0);
    }
}
