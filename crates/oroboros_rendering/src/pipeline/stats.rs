//! Rendering statistics.

/// Statistics from a render frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderStats {
    /// Number of draw calls.
    pub draw_calls: u32,
    /// Number of instances (quads) rendered.
    pub instances: u32,
    /// Number of chunks rendered.
    pub chunks_rendered: u32,
    /// Number of chunks culled by frustum.
    pub chunks_culled: u32,
    /// Number of active neon lights.
    pub neon_lights: u32,
    /// GPU time in milliseconds.
    pub gpu_time_ms: f32,
    /// Frame time in milliseconds.
    pub frame_time_ms: f32,
}

impl RenderStats {
    /// Returns FPS calculated from frame time.
    #[must_use]
    pub fn fps(&self) -> f32 {
        if self.frame_time_ms > 0.0 {
            1000.0 / self.frame_time_ms
        } else {
            0.0
        }
    }
    
    /// Returns true if meeting the 120 FPS target.
    #[must_use]
    pub fn meets_target(&self) -> bool {
        self.fps() >= 120.0
    }
    
    /// Returns true if draw calls are under budget.
    #[must_use]
    pub fn draw_calls_ok(&self) -> bool {
        self.draw_calls < 1000
    }
}
