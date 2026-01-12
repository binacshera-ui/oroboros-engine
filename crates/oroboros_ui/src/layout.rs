//! Layout system for UI positioning.

/// A rectangle in screen coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    /// X position (left edge).
    pub x: f32,
    /// Y position (top edge).
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Rect {
    /// A zero-sized rect at the origin.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };
    
    /// Creates a new rectangle.
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }
    
    /// Creates a rectangle from position and size.
    #[must_use]
    pub const fn from_pos_size(pos: (f32, f32), size: (f32, f32)) -> Self {
        Self {
            x: pos.0,
            y: pos.1,
            width: size.0,
            height: size.1,
        }
    }
    
    /// Returns the right edge.
    #[must_use]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }
    
    /// Returns the bottom edge.
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }
    
    /// Returns the center point.
    #[must_use]
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width * 0.5, self.y + self.height * 0.5)
    }
    
    /// Returns true if the point is inside the rectangle.
    #[must_use]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
    
    /// Returns true if two rectangles intersect.
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }
    
    /// Returns the intersection of two rectangles, or None if they don't intersect.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        if !self.intersects(other) {
            return None;
        }
        
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        
        Some(Self::new(x, y, right - x, bottom - y))
    }
    
    /// Expands the rectangle by the given amount on all sides.
    #[must_use]
    pub fn expand(&self, amount: f32) -> Self {
        Self::new(
            self.x - amount,
            self.y - amount,
            self.width + amount * 2.0,
            self.height + amount * 2.0,
        )
    }
    
    /// Shrinks the rectangle by the given amount on all sides.
    #[must_use]
    pub fn shrink(&self, amount: f32) -> Self {
        self.expand(-amount)
    }
}

/// Layout constraints for widgets.
#[derive(Debug, Clone, Copy, Default)]
pub struct Constraints {
    /// Minimum width.
    pub min_width: f32,
    /// Maximum width.
    pub max_width: f32,
    /// Minimum height.
    pub min_height: f32,
    /// Maximum height.
    pub max_height: f32,
}

impl Constraints {
    /// Unconstrained (any size).
    pub const UNBOUNDED: Self = Self {
        min_width: 0.0,
        max_width: f32::INFINITY,
        min_height: 0.0,
        max_height: f32::INFINITY,
    };
    
    /// Creates tight constraints (exact size).
    #[must_use]
    pub const fn tight(width: f32, height: f32) -> Self {
        Self {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        }
    }
    
    /// Creates loose constraints (up to max size).
    #[must_use]
    pub const fn loose(max_width: f32, max_height: f32) -> Self {
        Self {
            min_width: 0.0,
            max_width,
            min_height: 0.0,
            max_height,
        }
    }
    
    /// Clamps a size to these constraints.
    #[must_use]
    pub fn clamp(&self, width: f32, height: f32) -> (f32, f32) {
        (
            width.clamp(self.min_width, self.max_width),
            height.clamp(self.min_height, self.max_height),
        )
    }
}

/// Layout direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Horizontal (left to right).
    #[default]
    Horizontal,
    /// Vertical (top to bottom).
    Vertical,
}

/// Layout alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    /// Align to start (left/top).
    #[default]
    Start,
    /// Align to center.
    Center,
    /// Align to end (right/bottom).
    End,
    /// Stretch to fill available space.
    Stretch,
}

/// Layout manager for arranging widgets.
pub struct Layout {
    /// Current layout direction.
    pub direction: Direction,
    /// Main axis alignment.
    pub main_alignment: Alignment,
    /// Cross axis alignment.
    pub cross_alignment: Alignment,
    /// Gap between elements.
    pub gap: f32,
    /// Padding around content.
    pub padding: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            direction: Direction::Horizontal,
            main_alignment: Alignment::Start,
            cross_alignment: Alignment::Start,
            gap: 4.0,
            padding: 0.0,
        }
    }
}

impl Layout {
    /// Creates a horizontal layout.
    #[must_use]
    pub fn horizontal() -> Self {
        Self {
            direction: Direction::Horizontal,
            ..Default::default()
        }
    }
    
    /// Creates a vertical layout.
    #[must_use]
    pub fn vertical() -> Self {
        Self {
            direction: Direction::Vertical,
            ..Default::default()
        }
    }
    
    /// Sets the gap between elements.
    #[must_use]
    pub const fn with_gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }
    
    /// Sets padding around content.
    #[must_use]
    pub const fn with_padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }
    
    /// Sets main axis alignment.
    #[must_use]
    pub const fn align_main(mut self, alignment: Alignment) -> Self {
        self.main_alignment = alignment;
        self
    }
    
    /// Sets cross axis alignment.
    #[must_use]
    pub const fn align_cross(mut self, alignment: Alignment) -> Self {
        self.cross_alignment = alignment;
        self
    }
    
    /// Lays out a list of sizes within the given bounds.
    ///
    /// Returns the positions for each element.
    #[must_use]
    pub fn arrange(&self, bounds: Rect, sizes: &[(f32, f32)]) -> Vec<Rect> {
        if sizes.is_empty() {
            return Vec::new();
        }
        
        let content_bounds = bounds.shrink(self.padding);
        let mut results = Vec::with_capacity(sizes.len());
        
        match self.direction {
            Direction::Horizontal => {
                let total_width: f32 = sizes.iter().map(|(w, _)| *w).sum();
                let total_gap = self.gap * (sizes.len() - 1) as f32;
                
                let start_x = match self.main_alignment {
                    Alignment::Start => content_bounds.x,
                    Alignment::Center => content_bounds.x + (content_bounds.width - total_width - total_gap) * 0.5,
                    Alignment::End => content_bounds.right() - total_width - total_gap,
                    Alignment::Stretch => content_bounds.x,
                };
                
                let mut x = start_x;
                for (w, h) in sizes {
                    let y = match self.cross_alignment {
                        Alignment::Start => content_bounds.y,
                        Alignment::Center => content_bounds.y + (content_bounds.height - h) * 0.5,
                        Alignment::End => content_bounds.bottom() - h,
                        Alignment::Stretch => content_bounds.y,
                    };
                    
                    let height = if self.cross_alignment == Alignment::Stretch {
                        content_bounds.height
                    } else {
                        *h
                    };
                    
                    results.push(Rect::new(x, y, *w, height));
                    x += w + self.gap;
                }
            }
            Direction::Vertical => {
                let total_height: f32 = sizes.iter().map(|(_, h)| *h).sum();
                let total_gap = self.gap * (sizes.len() - 1) as f32;
                
                let start_y = match self.main_alignment {
                    Alignment::Start => content_bounds.y,
                    Alignment::Center => content_bounds.y + (content_bounds.height - total_height - total_gap) * 0.5,
                    Alignment::End => content_bounds.bottom() - total_height - total_gap,
                    Alignment::Stretch => content_bounds.y,
                };
                
                let mut y = start_y;
                for (w, h) in sizes {
                    let x = match self.cross_alignment {
                        Alignment::Start => content_bounds.x,
                        Alignment::Center => content_bounds.x + (content_bounds.width - w) * 0.5,
                        Alignment::End => content_bounds.right() - w,
                        Alignment::Stretch => content_bounds.x,
                    };
                    
                    let width = if self.cross_alignment == Alignment::Stretch {
                        content_bounds.width
                    } else {
                        *w
                    };
                    
                    results.push(Rect::new(x, y, width, *h));
                    y += h + self.gap;
                }
            }
        }
        
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rect_contains() {
        let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
        
        assert!(rect.contains(50.0, 30.0));
        assert!(!rect.contains(5.0, 30.0));
        assert!(!rect.contains(50.0, 80.0));
    }
    
    #[test]
    fn test_layout_horizontal() {
        let layout = Layout::horizontal().with_gap(10.0);
        let bounds = Rect::new(0.0, 0.0, 200.0, 50.0);
        let sizes = vec![(30.0, 20.0), (40.0, 20.0), (30.0, 20.0)];
        
        let result = layout.arrange(bounds, &sizes);
        
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].x, 0.0);
        assert_eq!(result[1].x, 40.0); // 30 + 10 gap
        assert_eq!(result[2].x, 90.0); // 40 + 40 + 10 gap
    }
}
