//! # Client-Side Prediction
//!
//! Predict local player movement for responsive gameplay.
//!
//! ## How It Works
//!
//! 1. Client sends input to server
//! 2. Client immediately predicts the result locally
//! 3. Server processes input and sends authoritative state
//! 4. Client reconciles prediction with server state
//!
//! ```text
//! Input:      [1] [2] [3] [4] [5]
//!              │   │   │   │   │
//! Prediction: [P1][P2][P3][P4][P5]
//!              │
//! Server Ack: [S1]────────────────
//!              │
//! Reconcile:  Compare P1 with S1
//!             If different: replay [2,3,4,5] from S1
//! ```

use oroboros_core::{Position, Velocity};
use crate::protocol::PlayerInput;

/// Size of the input buffer.
const INPUT_BUFFER_SIZE: usize = 64;

/// Input stored for prediction replay.
#[derive(Clone, Copy, Debug, Default)]
pub struct StoredInput {
    /// Sequence number.
    pub sequence: u32,
    /// The input.
    pub input: PlayerInput,
    /// Whether this has been acknowledged.
    pub acknowledged: bool,
}

/// Buffer of unacknowledged inputs.
pub struct InputBuffer {
    /// Ring buffer of inputs.
    inputs: [StoredInput; INPUT_BUFFER_SIZE],
    /// Write index.
    write_index: usize,
    /// Number of valid inputs.
    count: usize,
    /// Oldest unacknowledged sequence.
    oldest_unacked: u32,
}

impl InputBuffer {
    /// Creates a new input buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            inputs: [StoredInput {
                sequence: 0,
                input: PlayerInput::new(0, 0),
                acknowledged: false,
            }; INPUT_BUFFER_SIZE],
            write_index: 0,
            count: 0,
            oldest_unacked: 0,
        }
    }

    /// Adds an input to the buffer.
    pub fn add(&mut self, sequence: u32, input: PlayerInput) {
        self.inputs[self.write_index] = StoredInput {
            sequence,
            input,
            acknowledged: false,
        };
        self.write_index = (self.write_index + 1) % INPUT_BUFFER_SIZE;
        self.count = (self.count + 1).min(INPUT_BUFFER_SIZE);
    }

    /// Acknowledges all inputs up to the given sequence.
    pub fn acknowledge(&mut self, up_to_sequence: u32) {
        for i in 0..self.count {
            let idx = (self.write_index + INPUT_BUFFER_SIZE - 1 - i) % INPUT_BUFFER_SIZE;
            if self.inputs[idx].sequence <= up_to_sequence {
                self.inputs[idx].acknowledged = true;
            }
        }
        self.oldest_unacked = up_to_sequence + 1;
    }

    /// Returns unacknowledged inputs after the given sequence.
    pub fn unacked_after(&self, sequence: u32) -> impl Iterator<Item = &StoredInput> {
        (0..self.count)
            .map(move |i| {
                let idx = (self.write_index + INPUT_BUFFER_SIZE - self.count + i) % INPUT_BUFFER_SIZE;
                &self.inputs[idx]
            })
            .filter(move |s| !s.acknowledged && s.sequence > sequence)
    }

    /// Clears all inputs.
    pub fn clear(&mut self) {
        self.count = 0;
        self.write_index = 0;
    }
}

impl Default for InputBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of reconciliation.
#[derive(Clone, Copy, Debug)]
pub enum ReconciliationResult {
    /// No correction needed - prediction was accurate.
    NoCorrection,
    /// Small correction applied via smoothing.
    SmallCorrection {
        /// Error magnitude.
        error: f32,
    },
    /// Large correction - snapped to server position.
    Snap {
        /// Error magnitude.
        error: f32,
    },
}

/// Prediction buffer for client-side prediction.
pub struct PredictionBuffer {
    /// Input buffer.
    inputs: InputBuffer,
    /// Current predicted position.
    predicted_position: Position,
    /// Current predicted velocity.
    predicted_velocity: Velocity,
    /// Last acknowledged tick.
    last_acked_tick: u32,
    /// Smoothing factor for corrections (0-1).
    smoothing: f32,
    /// Error threshold for snapping.
    snap_threshold: f32,
}

impl PredictionBuffer {
    /// Creates a new prediction buffer.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let _ = capacity; // Using constant buffer size
        
        Self {
            inputs: InputBuffer::new(),
            predicted_position: Position::default(),
            predicted_velocity: Velocity::default(),
            last_acked_tick: 0,
            smoothing: 0.1, // 10% correction per frame
            snap_threshold: 1.0, // Snap if error > 1 unit
        }
    }

    /// Adds an input and updates prediction.
    pub fn add_input(&mut self, sequence: u32, input: PlayerInput) {
        // Predict new position
        let new_pos = self.predict_movement(&input);
        
        // Store for replay
        self.inputs.add(sequence, input);
        
        self.predicted_position = new_pos;
    }

    /// Predicts movement from input.
    fn predict_movement(&self, input: &PlayerInput) -> Position {
        const DT: f32 = 1.0 / 60.0;
        const GRAVITY: f32 = -20.0;
        
        let speed = if input.is_sprinting() { 10.0 } else { 5.0 };
        
        // Apply input
        let vel_x = input.move_x as f32 / 127.0 * speed;
        let vel_z = input.move_z as f32 / 127.0 * speed;
        let mut vel_y = self.predicted_velocity.y;
        
        // Jump
        if input.is_jumping() && self.predicted_position.y <= 0.1 {
            vel_y = 8.0;
        }
        
        // Apply gravity
        if self.predicted_position.y > 0.0 {
            vel_y += GRAVITY * DT;
        }
        
        // Update position
        let mut new_pos = self.predicted_position;
        new_pos.x += vel_x * DT;
        new_pos.y += vel_y * DT;
        new_pos.z += vel_z * DT;
        
        // Ground collision
        if new_pos.y < 0.0 {
            new_pos.y = 0.0;
        }
        
        new_pos
    }

    /// Reconciles prediction with server state.
    pub fn reconcile(&mut self, server_tick: u32, server_position: Position) -> ReconciliationResult {
        // Calculate error
        let error = calculate_error(self.predicted_position, server_position);
        
        if error < 0.01 {
            // Prediction was accurate
            self.inputs.acknowledge(server_tick);
            self.last_acked_tick = server_tick;
            return ReconciliationResult::NoCorrection;
        }
        
        if error > self.snap_threshold {
            // Large error - snap to server
            self.predicted_position = server_position;
            self.inputs.acknowledge(server_tick);
            self.last_acked_tick = server_tick;
            
            // Replay unacknowledged inputs
            self.replay_inputs(server_tick);
            
            return ReconciliationResult::Snap { error };
        }
        
        // Small error - smooth correction
        self.predicted_position = Position::new(
            lerp(self.predicted_position.x, server_position.x, self.smoothing),
            lerp(self.predicted_position.y, server_position.y, self.smoothing),
            lerp(self.predicted_position.z, server_position.z, self.smoothing),
        );
        
        self.inputs.acknowledge(server_tick);
        self.last_acked_tick = server_tick;
        
        ReconciliationResult::SmallCorrection { error }
    }

    /// Replays unacknowledged inputs from the server position.
    fn replay_inputs(&mut self, from_tick: u32) {
        // Collect inputs to replay
        let inputs_to_replay: Vec<PlayerInput> = self.inputs
            .unacked_after(from_tick)
            .map(|s| s.input)
            .collect();
        
        // Replay each input
        for input in inputs_to_replay {
            self.predicted_position = self.predict_movement(&input);
        }
    }

    /// Returns the current predicted position.
    #[must_use]
    pub fn predicted_position(&self) -> Option<Position> {
        Some(self.predicted_position)
    }

    /// Returns the current predicted velocity.
    #[must_use]
    pub const fn predicted_velocity(&self) -> &Velocity {
        &self.predicted_velocity
    }

    /// Sets the smoothing factor (0-1).
    pub fn set_smoothing(&mut self, smoothing: f32) {
        self.smoothing = smoothing.clamp(0.0, 1.0);
    }

    /// Sets the snap threshold.
    pub fn set_snap_threshold(&mut self, threshold: f32) {
        self.snap_threshold = threshold.max(0.0);
    }

    /// Resets prediction state.
    pub fn reset(&mut self, position: Position) {
        self.predicted_position = position;
        self.predicted_velocity = Velocity::default();
        self.inputs.clear();
        self.last_acked_tick = 0;
    }
}

impl Default for PredictionBuffer {
    fn default() -> Self {
        Self::new(64)
    }
}

/// Calculates position error.
fn calculate_error(predicted: Position, actual: Position) -> f32 {
    let dx = predicted.x - actual.x;
    let dy = predicted.y - actual.y;
    let dz = predicted.z - actual.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Linear interpolation.
#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_buffer() {
        let mut buffer = InputBuffer::new();
        
        for i in 0..10 {
            buffer.add(
                i,
                PlayerInput::new(i, i),
            );
        }
        
        assert_eq!(buffer.count, 10);
        
        // Acknowledge up to sequence 5
        buffer.acknowledge(5);
        
        // Count unacked after 5
        let unacked: Vec<_> = buffer.unacked_after(5).collect();
        assert_eq!(unacked.len(), 4); // 6, 7, 8, 9
    }

    #[test]
    fn test_prediction() {
        let mut pred = PredictionBuffer::new(64);
        pred.reset(Position::new(0.0, 0.0, 0.0));
        
        // Add movement input
        let mut input = PlayerInput::new(0, 0);
        input.move_x = 127; // Full right
        
        pred.add_input(0, input);
        
        // Should have moved
        let pos = pred.predicted_position().unwrap();
        assert!(pos.x > 0.0);
    }

    #[test]
    fn test_reconciliation_no_error() {
        let mut pred = PredictionBuffer::new(64);
        pred.reset(Position::new(10.0, 0.0, 10.0));
        
        // Server confirms same position
        let result = pred.reconcile(1, Position::new(10.0, 0.0, 10.0));
        
        assert!(matches!(result, ReconciliationResult::NoCorrection));
    }

    #[test]
    fn test_reconciliation_snap() {
        let mut pred = PredictionBuffer::new(64);
        pred.reset(Position::new(0.0, 0.0, 0.0));
        
        // Server says we're way off
        let result = pred.reconcile(1, Position::new(100.0, 0.0, 100.0));
        
        assert!(matches!(result, ReconciliationResult::Snap { .. }));
        
        // Position should be snapped
        let pos = pred.predicted_position().unwrap();
        assert!((pos.x - 100.0).abs() < 0.01);
    }
}
