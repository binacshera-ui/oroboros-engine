//! # Visual Interpolation Module
//!
//! THE ARCHITECT'S DECREE:
//! "4 corrections per second with Hard Snap = motion sickness"
//! "The character must GLIDE to the correct position over 100ms, not teleport"
//!
//! This module provides smooth visual correction for client-side prediction errors.
//!
//! ## Architecture:
//! - **Logical Position**: Where the entity IS according to physics (for collision, hit detection)
//! - **Visual Position**: Where the entity APPEARS on screen (for rendering)
//! - When correction occurs, visual position smoothly interpolates to logical position
//!
//! ## Usage:
//! ```ignore
//! let mut interp = VisualInterpolator::new(100.0); // 100ms blend time
//!
//! // Each tick:
//! interp.update(dt);
//! let visual_pos = interp.get_visual_position(logical_pos);
//!
//! // When reconciliation happens:
//! interp.start_correction(old_logical_pos, new_logical_pos);
//! ```

use oroboros_core::Position;

/// Interpolation mode for corrections.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InterpolationMode {
    /// No interpolation - instant snap (BAD for player experience)
    HardSnap,
    /// Linear interpolation over time (OK)
    Linear,
    /// Smooth ease-out interpolation (GOOD - starts fast, slows down)
    EaseOut,
    /// Very smooth S-curve (BEST - natural feel)
    SmoothStep,
}

/// Visual interpolator for smooth correction blending.
#[derive(Clone, Debug)]
pub struct VisualInterpolator {
    /// How long corrections take to blend (milliseconds).
    pub blend_time_ms: f32,
    /// Current interpolation progress (0.0 = start, 1.0 = complete).
    progress: f32,
    /// Starting position for current correction.
    correction_start: Option<Position>,
    /// Target offset to apply (difference between old and new logical position).
    correction_offset: Position,
    /// Interpolation mode.
    mode: InterpolationMode,
    /// Statistics: total corrections processed.
    pub total_corrections: u32,
    /// Statistics: current visual offset magnitude.
    pub current_offset_magnitude: f32,
}

impl VisualInterpolator {
    /// Creates a new interpolator with specified blend time.
    ///
    /// # Arguments
    /// * `blend_time_ms` - How long corrections take to blend visually (recommended: 100-150ms)
    pub fn new(blend_time_ms: f32) -> Self {
        Self {
            blend_time_ms,
            progress: 1.0, // Start complete (no correction in progress)
            correction_start: None,
            correction_offset: Position::new(0.0, 0.0, 0.0),
            mode: InterpolationMode::EaseOut,
            total_corrections: 0,
            current_offset_magnitude: 0.0,
        }
    }

    /// Creates an interpolator with smooth step mode (best visual quality).
    pub fn smooth(blend_time_ms: f32) -> Self {
        let mut interp = Self::new(blend_time_ms);
        interp.mode = InterpolationMode::SmoothStep;
        interp
    }

    /// Sets the interpolation mode.
    pub fn set_mode(&mut self, mode: InterpolationMode) {
        self.mode = mode;
    }

    /// Called when a reconciliation correction occurs.
    ///
    /// This starts a new visual blend from the old position to the new corrected position.
    ///
    /// # Arguments
    /// * `old_logical` - Position before correction
    /// * `new_logical` - Position after correction (from server reconciliation)
    pub fn start_correction(&mut self, old_logical: Position, new_logical: Position) {
        // Calculate the offset we need to blend away
        self.correction_offset = Position::new(
            old_logical.x - new_logical.x,
            old_logical.y - new_logical.y,
            old_logical.z - new_logical.z,
        );

        // Only start correction if offset is significant
        let magnitude = (self.correction_offset.x.powi(2)
            + self.correction_offset.y.powi(2)
            + self.correction_offset.z.powi(2))
        .sqrt();

        if magnitude > 0.001 {
            self.correction_start = Some(old_logical);
            self.progress = 0.0;
            self.total_corrections += 1;
            self.current_offset_magnitude = magnitude;
        }
    }

    /// Updates interpolation progress. Call once per frame.
    ///
    /// # Arguments
    /// * `dt_ms` - Delta time in milliseconds since last update
    pub fn update(&mut self, dt_ms: f32) {
        if self.progress < 1.0 {
            self.progress += dt_ms / self.blend_time_ms;
            if self.progress >= 1.0 {
                self.progress = 1.0;
                self.correction_offset = Position::new(0.0, 0.0, 0.0);
                self.current_offset_magnitude = 0.0;
            }
        }
    }

    /// Returns the visual position for rendering.
    ///
    /// This adds a diminishing offset to the logical position during correction blending.
    ///
    /// # Arguments
    /// * `logical_pos` - Current logical/physics position
    pub fn get_visual_position(&self, logical_pos: Position) -> Position {
        if self.progress >= 1.0 {
            return logical_pos;
        }

        // Calculate blend factor based on mode
        let t = match self.mode {
            InterpolationMode::HardSnap => 1.0,
            InterpolationMode::Linear => self.progress,
            InterpolationMode::EaseOut => {
                // Fast start, slow end: 1 - (1-t)^2
                let inv = 1.0 - self.progress;
                1.0 - inv * inv
            }
            InterpolationMode::SmoothStep => {
                // S-curve: 3t^2 - 2t^3
                let t = self.progress;
                t * t * (3.0 - 2.0 * t)
            }
        };

        // Remaining offset = original offset * (1 - blend_factor)
        let remaining = 1.0 - t;

        Position::new(
            logical_pos.x + self.correction_offset.x * remaining,
            logical_pos.y + self.correction_offset.y * remaining,
            logical_pos.z + self.correction_offset.z * remaining,
        )
    }

    /// Returns true if a correction is currently being interpolated.
    pub fn is_correcting(&self) -> bool {
        self.progress < 1.0
    }

    /// Returns the current interpolation progress (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Cancels any in-progress correction (snaps to logical position).
    pub fn cancel_correction(&mut self) {
        self.progress = 1.0;
        self.correction_offset = Position::new(0.0, 0.0, 0.0);
        self.current_offset_magnitude = 0.0;
    }
}

/// Snapshot interpolator for smooth entity rendering between server updates.
///
/// This handles interpolation between server snapshots (separate from correction blending).
/// The client renders entities between their last two known server positions.
#[derive(Clone, Debug)]
pub struct SnapshotInterpolator {
    /// Previous server position.
    prev_pos: Position,
    /// Current server position.
    curr_pos: Position,
    /// Time between snapshots (milliseconds).
    snapshot_interval_ms: f32,
    /// Time since last snapshot (milliseconds).
    time_since_snapshot: f32,
}

impl SnapshotInterpolator {
    /// Creates a new snapshot interpolator.
    ///
    /// # Arguments
    /// * `initial_pos` - Initial position
    /// * `snapshot_interval_ms` - Expected time between server snapshots (e.g., 16.67 for 60Hz)
    pub fn new(initial_pos: Position, snapshot_interval_ms: f32) -> Self {
        Self {
            prev_pos: initial_pos,
            curr_pos: initial_pos,
            snapshot_interval_ms,
            time_since_snapshot: 0.0,
        }
    }

    /// Called when a new server snapshot arrives.
    pub fn push_snapshot(&mut self, new_pos: Position) {
        self.prev_pos = self.curr_pos;
        self.curr_pos = new_pos;
        self.time_since_snapshot = 0.0;
    }

    /// Updates time. Call once per frame.
    pub fn update(&mut self, dt_ms: f32) {
        self.time_since_snapshot += dt_ms;
    }

    /// Returns the interpolated position for rendering.
    pub fn get_interpolated_position(&self) -> Position {
        let t = (self.time_since_snapshot / self.snapshot_interval_ms).clamp(0.0, 1.0);

        Position::new(
            self.prev_pos.x + (self.curr_pos.x - self.prev_pos.x) * t,
            self.prev_pos.y + (self.curr_pos.y - self.prev_pos.y) * t,
            self.prev_pos.z + (self.curr_pos.z - self.prev_pos.z) * t,
        )
    }
}

/// Combined interpolation state for a player entity.
///
/// This combines both correction blending and snapshot interpolation
/// for the smoothest possible visual experience.
#[derive(Clone, Debug)]
pub struct PlayerVisualState {
    /// Correction blending for reconciliation.
    pub correction: VisualInterpolator,
    /// Snapshot interpolation for other players (not self).
    pub snapshot: Option<SnapshotInterpolator>,
    /// Last known logical position.
    pub logical_position: Position,
}

impl PlayerVisualState {
    /// Creates a new player visual state.
    pub fn new(initial_pos: Position, correction_blend_ms: f32) -> Self {
        Self {
            correction: VisualInterpolator::smooth(correction_blend_ms),
            snapshot: None,
            logical_position: initial_pos,
        }
    }

    /// Creates state for a remote player (with snapshot interpolation).
    pub fn new_remote(initial_pos: Position, snapshot_interval_ms: f32) -> Self {
        Self {
            correction: VisualInterpolator::new(0.0), // No correction for remote players
            snapshot: Some(SnapshotInterpolator::new(initial_pos, snapshot_interval_ms)),
            logical_position: initial_pos,
        }
    }

    /// Updates interpolation state. Call once per frame.
    pub fn update(&mut self, dt_ms: f32) {
        self.correction.update(dt_ms);
        if let Some(ref mut snapshot) = self.snapshot {
            snapshot.update(dt_ms);
        }
    }

    /// Called when reconciliation corrects the position.
    pub fn on_correction(&mut self, old_pos: Position, new_pos: Position) {
        self.correction.start_correction(old_pos, new_pos);
        self.logical_position = new_pos;
    }

    /// Called when a server snapshot arrives (for remote players).
    pub fn on_snapshot(&mut self, server_pos: Position) {
        if let Some(ref mut snapshot) = self.snapshot {
            snapshot.push_snapshot(server_pos);
        }
        self.logical_position = server_pos;
    }

    /// Returns the position to render on screen.
    pub fn get_render_position(&self) -> Position {
        if let Some(ref snapshot) = self.snapshot {
            // Remote player: use snapshot interpolation
            snapshot.get_interpolated_position()
        } else {
            // Local player: use correction blending
            self.correction.get_visual_position(self.logical_position)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hard_snap() {
        let mut interp = VisualInterpolator::new(100.0);
        interp.set_mode(InterpolationMode::HardSnap);

        let old_pos = Position::new(0.0, 0.0, 0.0);
        let new_pos = Position::new(10.0, 0.0, 0.0);

        interp.start_correction(old_pos, new_pos);

        // Even at progress 0, hard snap should return new position
        let visual = interp.get_visual_position(new_pos);
        assert!((visual.x - new_pos.x).abs() < 0.001);
    }

    #[test]
    fn test_linear_interpolation() {
        let mut interp = VisualInterpolator::new(100.0);
        interp.set_mode(InterpolationMode::Linear);

        let old_pos = Position::new(0.0, 0.0, 0.0);
        let new_pos = Position::new(10.0, 0.0, 0.0);

        interp.start_correction(old_pos, new_pos);

        // At start, visual should be at old position
        let visual = interp.get_visual_position(new_pos);
        assert!((visual.x - 0.0).abs() < 0.001);

        // After 50ms, should be halfway
        interp.update(50.0);
        let visual = interp.get_visual_position(new_pos);
        assert!((visual.x - 5.0).abs() < 0.1);

        // After 100ms, should be at new position
        interp.update(50.0);
        let visual = interp.get_visual_position(new_pos);
        assert!((visual.x - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_ease_out() {
        let mut interp = VisualInterpolator::new(100.0);
        interp.set_mode(InterpolationMode::EaseOut);

        let old_pos = Position::new(0.0, 0.0, 0.0);
        let new_pos = Position::new(10.0, 0.0, 0.0);

        interp.start_correction(old_pos, new_pos);

        // After 50ms with ease-out, should be MORE than halfway (fast start)
        interp.update(50.0);
        let visual = interp.get_visual_position(new_pos);
        assert!(visual.x > 5.0, "Ease-out should be past halfway at 50%");
    }

    #[test]
    fn test_smoothstep() {
        let mut interp = VisualInterpolator::smooth(100.0);

        let old_pos = Position::new(0.0, 0.0, 0.0);
        let new_pos = Position::new(10.0, 0.0, 0.0);

        interp.start_correction(old_pos, new_pos);

        // At 50%, smoothstep should also be at 50% (inflection point)
        interp.update(50.0);
        let visual = interp.get_visual_position(new_pos);
        assert!((visual.x - 5.0).abs() < 0.1);
    }
}
