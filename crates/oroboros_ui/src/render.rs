//! UI rendering system.
//!
//! Generates batched render commands for efficient GPU submission.

use crate::layout::Rect;
use crate::style::Color;

/// A render command for the UI.
#[derive(Debug, Clone)]
pub enum RenderCommand {
    /// Filled rectangle.
    Rect {
        /// Bounds.
        bounds: Rect,
        /// Fill color.
        color: Color,
        /// Corner radius.
        corner_radius: f32,
    },
    /// Rectangle outline.
    RectOutline {
        /// Bounds.
        bounds: Rect,
        /// Stroke color.
        color: Color,
        /// Line width.
        width: f32,
        /// Corner radius.
        corner_radius: f32,
    },
    /// Text.
    Text {
        /// Text content.
        text: String,
        /// X position.
        x: f32,
        /// Y position.
        y: f32,
        /// Text color.
        color: Color,
        /// Font size.
        font_size: f32,
        /// Use monospace font.
        monospace: bool,
    },
    /// Icon from atlas.
    Icon {
        /// Bounds.
        bounds: Rect,
        /// Icon ID in atlas.
        icon_id: u32,
        /// Tint color.
        color: Color,
    },
    /// Textured quad.
    Texture {
        /// Bounds.
        bounds: Rect,
        /// Texture ID.
        texture_id: u32,
        /// UV coordinates (u0, v0, u1, v1).
        uv: [f32; 4],
        /// Tint color.
        color: Color,
    },
    /// Scissor rect (clip children).
    PushClip {
        /// Clip bounds.
        bounds: Rect,
    },
    /// Pop scissor rect.
    PopClip,
}

/// A batch of render commands with the same state.
#[derive(Debug, Clone)]
pub struct UIBatch {
    /// Commands in this batch.
    pub commands: Vec<RenderCommand>,
    /// Clip rect (if any).
    pub clip: Option<Rect>,
    /// Z-index for sorting.
    pub z_index: i32,
}

impl UIBatch {
    /// Creates a new empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: Vec::with_capacity(256),
            clip: None,
            z_index: 0,
        }
    }
}

impl Default for UIBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// UI renderer that collects and batches commands.
pub struct UIRenderer {
    /// All commands from the frame.
    commands: Vec<RenderCommand>,
    /// Clip stack.
    clip_stack: Vec<Rect>,
    /// Final batches for rendering.
    batches: Vec<UIBatch>,
}

impl UIRenderer {
    /// Creates a new UI renderer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: Vec::with_capacity(4096),
            clip_stack: Vec::with_capacity(16),
            batches: Vec::with_capacity(64),
        }
    }
    
    /// Begins a new frame.
    pub fn begin_frame(&mut self) {
        self.commands.clear();
        self.clip_stack.clear();
        self.batches.clear();
    }
    
    /// Adds a render command.
    pub fn push(&mut self, command: RenderCommand) {
        self.commands.push(command);
    }
    
    /// Adds multiple render commands.
    pub fn extend(&mut self, commands: impl IntoIterator<Item = RenderCommand>) {
        self.commands.extend(commands);
    }
    
    /// Pushes a clip rect.
    pub fn push_clip(&mut self, bounds: Rect) {
        // Intersect with current clip if any
        let actual_clip = if let Some(current) = self.clip_stack.last() {
            current.intersection(&bounds).unwrap_or(Rect::ZERO)
        } else {
            bounds
        };
        
        self.clip_stack.push(actual_clip);
        self.commands.push(RenderCommand::PushClip { bounds: actual_clip });
    }
    
    /// Pops the current clip rect.
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
        self.commands.push(RenderCommand::PopClip);
    }
    
    /// Returns the current clip rect.
    #[must_use]
    pub fn current_clip(&self) -> Option<Rect> {
        self.clip_stack.last().copied()
    }
    
    /// Ends the frame and returns batches for rendering.
    pub fn end_frame(&mut self) -> &[UIBatch] {
        // For now, just create a single batch with all commands
        // In production, we'd sort and batch by texture/state
        let batch = UIBatch {
            commands: std::mem::take(&mut self.commands),
            clip: None,
            z_index: 0,
        };
        
        self.batches.clear();
        self.batches.push(batch);
        
        &self.batches
    }
    
    /// Returns the total command count.
    #[must_use]
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }
}

impl Default for UIRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Vertex for UI rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UIVertex {
    /// Position (x, y).
    pub position: [f32; 2],
    /// UV coordinates.
    pub uv: [f32; 2],
    /// Color (RGBA).
    pub color: [f32; 4],
}

impl UIVertex {
    /// Creates a new vertex.
    #[must_use]
    pub const fn new(x: f32, y: f32, u: f32, v: f32, color: [f32; 4]) -> Self {
        Self {
            position: [x, y],
            uv: [u, v],
            color,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_renderer_frame() {
        let mut renderer = UIRenderer::new();
        
        renderer.begin_frame();
        renderer.push(RenderCommand::Rect {
            bounds: Rect::new(0.0, 0.0, 100.0, 50.0),
            color: Color::WHITE,
            corner_radius: 0.0,
        });
        
        let batches = renderer.end_frame();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].commands.len(), 1);
    }
    
    #[test]
    fn test_clip_stack() {
        let mut renderer = UIRenderer::new();
        renderer.begin_frame();
        
        renderer.push_clip(Rect::new(0.0, 0.0, 100.0, 100.0));
        assert!(renderer.current_clip().is_some());
        
        renderer.pop_clip();
        assert!(renderer.current_clip().is_none());
    }
}
