//! # Snapshot System
//!
//! Server-side snapshot generation and client-side interpolation.
//!
//! ## Snapshot Interpolation
//!
//! The client maintains a buffer of recent snapshots and interpolates
//! between them to render smooth movement, even with packet loss.
//!
//! ```text
//! Server Ticks:    [1] [2] [3] [4] [5] [6]
//!                   │   │   │   │   │   │
//! Network Delay:    ~~~~~~~~~~~~~~~~~~~~~
//!                             │   │
//! Client Buffer:            [3] [4] (waiting for 5, 6)
//!                             │
//! Render Time:              ▼
//!                    Interpolate between 3 and 4
//! ```

use crate::protocol::{WorldSnapshot, EntityState};

/// Snapshot buffer for interpolation.
///
/// Stores recent snapshots in a ring buffer.
pub struct SnapshotBuffer {
    /// Ring buffer of snapshots.
    snapshots: Vec<WorldSnapshot>,
    /// Write index.
    write_index: usize,
    /// Number of valid snapshots.
    count: usize,
    /// Interpolation delay in ticks.
    interp_delay: u32,
}

impl SnapshotBuffer {
    /// Creates a new snapshot buffer.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            snapshots: vec![WorldSnapshot::default(); capacity],
            write_index: 0,
            count: 0,
            interp_delay: 2, // 2 ticks behind for smooth interpolation
        }
    }

    /// Adds a snapshot to the buffer.
    pub fn add_snapshot(&mut self, snapshot: WorldSnapshot) {
        self.snapshots[self.write_index] = snapshot;
        self.write_index = (self.write_index + 1) % self.snapshots.len();
        self.count = (self.count + 1).min(self.snapshots.len());
    }

    /// Gets a snapshot by tick number.
    #[must_use]
    pub fn get_snapshot(&self, tick: u32) -> Option<&WorldSnapshot> {
        for i in 0..self.count {
            let idx = (self.write_index + self.snapshots.len() - 1 - i) % self.snapshots.len();
            if self.snapshots[idx].tick == tick {
                return Some(&self.snapshots[idx]);
            }
        }
        None
    }

    /// Gets the latest snapshot.
    #[must_use]
    pub fn latest(&self) -> Option<&WorldSnapshot> {
        if self.count == 0 {
            return None;
        }
        let idx = if self.write_index == 0 {
            self.snapshots.len() - 1
        } else {
            self.write_index - 1
        };
        Some(&self.snapshots[idx])
    }

    /// Finds two snapshots to interpolate between.
    fn find_interp_snapshots(&self, render_tick: f64) -> Option<(&WorldSnapshot, &WorldSnapshot, f64)> {
        if self.count < 2 {
            return None;
        }

        // Find snapshots bracketing render_tick
        let mut prev: Option<&WorldSnapshot> = None;
        let mut next: Option<&WorldSnapshot> = None;

        for i in 0..self.count {
            let idx = (self.write_index + self.snapshots.len() - 1 - i) % self.snapshots.len();
            let snap = &self.snapshots[idx];
            
            if (snap.tick as f64) <= render_tick {
                prev = Some(snap);
                break;
            }
            next = Some(snap);
        }

        // If we didn't find a "next", use the latest as both
        let prev = prev?;
        let next = next.unwrap_or(prev);

        if prev.tick == next.tick {
            return Some((prev, prev, 0.0));
        }

        let t = (render_tick - prev.tick as f64) / (next.tick - prev.tick) as f64;
        Some((prev, next, t.clamp(0.0, 1.0)))
    }

    /// Interpolates between snapshots for smooth rendering.
    #[must_use]
    pub fn interpolate(&self, render_time: f64) -> Option<WorldSnapshot> {
        let (prev, next, t) = self.find_interp_snapshots(render_time)?;
        
        if t == 0.0 || prev.tick == next.tick {
            return Some(*prev);
        }

        let mut result = WorldSnapshot::empty(prev.tick);
        result.dragon = prev.dragon; // Dragon state is discrete, not interpolated
        
        // Interpolate each entity
        for i in 0..prev.entity_count as usize {
            let prev_entity = &prev.entities[i];
            
            // Find matching entity in next snapshot
            let next_entity = (0..next.entity_count as usize)
                .map(|j| &next.entities[j])
                .find(|e| e.entity_id == prev_entity.entity_id);
            
            let interp_entity = if let Some(next_e) = next_entity {
                interpolate_entity(prev_entity, next_e, t as f32)
            } else {
                *prev_entity
            };
            
            result.add_entity(interp_entity);
        }
        
        Some(result)
    }

    /// Sets the interpolation delay in ticks.
    pub fn set_interp_delay(&mut self, ticks: u32) {
        self.interp_delay = ticks;
    }

    /// Returns the interpolation delay.
    #[must_use]
    pub const fn interp_delay(&self) -> u32 {
        self.interp_delay
    }

    /// Returns the number of buffered snapshots.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Clears all snapshots.
    pub fn clear(&mut self) {
        self.count = 0;
        self.write_index = 0;
    }
}

/// Interpolates between two entity states.
fn interpolate_entity(prev: &EntityState, next: &EntityState, t: f32) -> EntityState {
    EntityState {
        entity_id: prev.entity_id,
        pos_x: lerp(prev.pos_x, next.pos_x, t),
        pos_y: lerp(prev.pos_y, next.pos_y, t),
        pos_z: lerp(prev.pos_z, next.pos_z, t),
        vel_x: lerp(prev.vel_x, next.vel_x, t),
        vel_y: lerp(prev.vel_y, next.vel_y, t),
        vel_z: lerp(prev.vel_z, next.vel_z, t),
        rotation: lerp_i16(prev.rotation, next.rotation, t),
        health: prev.health, // Don't interpolate health
        flags: prev.flags,
    }
}

/// Linear interpolation.
#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation for i16.
#[inline]
fn lerp_i16(a: i16, b: i16, t: f32) -> i16 {
    (a as f32 + (b - a) as f32 * t) as i16
}

/// State of interpolation for rendering.
#[derive(Clone, Copy, Debug, Default)]
pub struct InterpolationState {
    /// Previous snapshot tick.
    pub prev_tick: u32,
    /// Next snapshot tick.
    pub next_tick: u32,
    /// Interpolation factor (0-1).
    pub t: f32,
    /// Render time.
    pub render_time: f64,
}

/// Compressor for snapshots.
pub struct SnapshotCompressor {
    /// Previous snapshot for delta calculation.
    previous: Option<WorldSnapshot>,
}

impl SnapshotCompressor {
    /// Creates a new compressor.
    #[must_use]
    pub const fn new() -> Self {
        Self { previous: None }
    }

    /// Compresses a snapshot.
    ///
    /// Returns true if delta compression was used.
    pub fn compress(&mut self, snapshot: &WorldSnapshot) -> (Vec<u8>, bool) {
        // For now, just return the raw snapshot
        // Delta compression is handled in the protocol layer
        self.previous = Some(*snapshot);
        (Vec::new(), false)
    }

    /// Resets the compressor.
    pub fn reset(&mut self) {
        self.previous = None;
    }
}

impl Default for SnapshotCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_buffer() {
        let mut buffer = SnapshotBuffer::new(4);
        
        for i in 1..=5 {
            let mut snap = WorldSnapshot::empty(i);
            snap.add_entity(EntityState {
                entity_id: 1,
                pos_x: i as f32 * 10.0,
                ..Default::default()
            });
            buffer.add_snapshot(snap);
        }
        
        // Should have 4 snapshots (buffer size)
        assert_eq!(buffer.count(), 4);
        
        // Latest should be tick 5
        assert_eq!(buffer.latest().unwrap().tick, 5);
        
        // Tick 2 should be found
        assert!(buffer.get_snapshot(2).is_some());
        
        // Tick 1 should be gone (evicted)
        assert!(buffer.get_snapshot(1).is_none());
    }

    #[test]
    fn test_interpolation() {
        let mut buffer = SnapshotBuffer::new(4);
        
        // Add two snapshots
        let mut snap1 = WorldSnapshot::empty(1);
        snap1.add_entity(EntityState {
            entity_id: 1,
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            ..Default::default()
        });
        buffer.add_snapshot(snap1);
        
        let mut snap2 = WorldSnapshot::empty(2);
        snap2.add_entity(EntityState {
            entity_id: 1,
            pos_x: 10.0,
            pos_y: 0.0,
            pos_z: 0.0,
            ..Default::default()
        });
        buffer.add_snapshot(snap2);
        
        // Interpolate at t=1.5 (halfway between tick 1 and 2)
        let interp = buffer.interpolate(1.5).unwrap();
        
        // Position should be interpolated
        let entity = &interp.entities[0];
        assert!((entity.pos_x - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < f32::EPSILON);
        assert!((lerp(0.0, 10.0, 0.0) - 0.0).abs() < f32::EPSILON);
        assert!((lerp(0.0, 10.0, 1.0) - 10.0).abs() < f32::EPSILON);
    }
}
