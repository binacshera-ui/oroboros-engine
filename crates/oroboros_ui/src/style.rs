//! Styling system for the Bloomberg terminal aesthetic.
//!
//! Dark backgrounds, neon accents, monospace fonts, dense information display.

/// RGBA color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red component (0-1).
    pub r: f32,
    /// Green component (0-1).
    pub g: f32,
    /// Blue component (0-1).
    pub b: f32,
    /// Alpha component (0-1).
    pub a: f32,
}

impl Color {
    /// Transparent black.
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);
    /// Solid black.
    pub const BLACK: Self = Self::rgba(0.0, 0.0, 0.0, 1.0);
    /// Solid white.
    pub const WHITE: Self = Self::rgba(1.0, 1.0, 1.0, 1.0);
    /// Neon green (terminal style).
    pub const NEON_GREEN: Self = Self::rgba(0.2, 1.0, 0.3, 1.0);
    /// Neon cyan.
    pub const NEON_CYAN: Self = Self::rgba(0.2, 0.9, 1.0, 1.0);
    /// Neon pink.
    pub const NEON_PINK: Self = Self::rgba(1.0, 0.2, 0.6, 1.0);
    /// Profit green.
    pub const PROFIT: Self = Self::rgba(0.2, 0.9, 0.3, 1.0);
    /// Loss red.
    pub const LOSS: Self = Self::rgba(0.9, 0.2, 0.2, 1.0);
    /// Warning orange.
    pub const WARNING: Self = Self::rgba(1.0, 0.6, 0.1, 1.0);
    
    /// Creates a color from RGBA values (0-1).
    #[must_use]
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    
    /// Creates a color from RGB values (0-1) with full alpha.
    #[must_use]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::rgba(r, g, b, 1.0)
    }
    
    /// Creates a color from hex value (0xRRGGBB or 0xRRGGBBAA).
    #[must_use]
    pub const fn hex(hex: u32) -> Self {
        let r = ((hex >> 24) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let b = ((hex >> 8) & 0xFF) as f32 / 255.0;
        let a = (hex & 0xFF) as f32 / 255.0;
        Self::rgba(r, g, b, a)
    }
    
    /// Returns a new color with different alpha.
    #[must_use]
    pub const fn with_alpha(self, a: f32) -> Self {
        Self::rgba(self.r, self.g, self.b, a)
    }
    
    /// Linearly interpolates between two colors.
    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self::rgba(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
            self.a + (other.a - self.a) * t,
        )
    }
    
    /// Converts to array format.
    #[must_use]
    pub const fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::WHITE
    }
}

/// Style for a widget.
#[derive(Debug, Clone)]
pub struct Style {
    /// Background color.
    pub background: Color,
    /// Border color.
    pub border: Color,
    /// Text color.
    pub text: Color,
    /// Accent color.
    pub accent: Color,
    /// Border width.
    pub border_width: f32,
    /// Corner radius.
    pub corner_radius: f32,
    /// Padding.
    pub padding: f32,
    /// Font size.
    pub font_size: f32,
    /// Use monospace font.
    pub monospace: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: Color::rgba(0.05, 0.05, 0.08, 0.95),
            border: Color::rgba(0.2, 0.3, 0.2, 0.8),
            text: Color::rgba(0.9, 0.9, 0.9, 1.0),
            accent: Color::NEON_GREEN,
            border_width: 1.0,
            corner_radius: 4.0,
            padding: 8.0,
            font_size: 14.0,
            monospace: true,
        }
    }
}

/// Complete theme for the UI.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Primary background color.
    pub background: Color,
    /// Surface color (cards, panels).
    pub surface: Color,
    /// Primary accent color.
    pub primary: Color,
    /// Secondary accent color.
    pub secondary: Color,
    /// Tertiary accent color.
    pub tertiary: Color,
    /// Text color.
    pub text: Color,
    /// Muted text color.
    pub text_muted: Color,
    /// Border color.
    pub border: Color,
    /// Profit indicator color.
    pub profit: Color,
    /// Loss indicator color.
    pub loss: Color,
    /// Warning color.
    pub warning: Color,
    /// Error color.
    pub error: Color,
}

impl Theme {
    /// Bloomberg terminal dark theme.
    pub const BLOOMBERG: Self = Self {
        background: Color::rgba(0.02, 0.02, 0.03, 1.0),
        surface: Color::rgba(0.05, 0.05, 0.08, 0.95),
        primary: Color::NEON_GREEN,
        secondary: Color::NEON_CYAN,
        tertiary: Color::NEON_PINK,
        text: Color::rgba(0.9, 0.9, 0.9, 1.0),
        text_muted: Color::rgba(0.5, 0.5, 0.5, 1.0),
        border: Color::rgba(0.15, 0.2, 0.15, 0.8),
        profit: Color::PROFIT,
        loss: Color::LOSS,
        warning: Color::WARNING,
        error: Color::rgba(0.9, 0.2, 0.2, 1.0),
    };
    
    /// Neon Prime cyberpunk theme.
    pub const NEON_PRIME: Self = Self {
        background: Color::rgba(0.01, 0.01, 0.02, 1.0),
        surface: Color::rgba(0.03, 0.03, 0.05, 0.9),
        primary: Color::NEON_CYAN,
        secondary: Color::NEON_PINK,
        tertiary: Color::rgba(0.6, 0.2, 1.0, 1.0), // Purple
        text: Color::rgba(0.85, 0.9, 0.95, 1.0),
        text_muted: Color::rgba(0.4, 0.45, 0.5, 1.0),
        border: Color::rgba(0.1, 0.2, 0.2, 0.6),
        profit: Color::PROFIT,
        loss: Color::LOSS,
        warning: Color::rgba(1.0, 0.7, 0.0, 1.0),
        error: Color::rgba(1.0, 0.1, 0.2, 1.0),
    };
}

impl Default for Theme {
    fn default() -> Self {
        Self::BLOOMBERG
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_color_lerp() {
        let black = Color::BLACK;
        let white = Color::WHITE;
        let mid = black.lerp(white, 0.5);
        
        assert!((mid.r - 0.5).abs() < 0.01);
        assert!((mid.g - 0.5).abs() < 0.01);
        assert!((mid.b - 0.5).abs() < 0.01);
    }
    
    #[test]
    fn test_color_hex() {
        let color = Color::hex(0xFF0000FF);
        assert!((color.r - 1.0).abs() < 0.01);
        assert!((color.g - 0.0).abs() < 0.01);
        assert!((color.b - 0.0).abs() < 0.01);
    }
}
