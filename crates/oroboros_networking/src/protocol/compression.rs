//! # Delta Compression and Bit Packing
//!
//! Efficient compression for reducing bandwidth.
//!
//! ## Techniques
//!
//! 1. **Delta Compression**: Only send what changed since last snapshot
//! 2. **Bit Packing**: Pack values into minimum required bits
//! 3. **Quantization**: Reduce precision where acceptable

use super::packets::{EntityState, WorldSnapshot, DeltaSnapshot};

/// Threshold for position change to be considered "changed" (squared distance).
const POSITION_CHANGE_THRESHOLD_SQ: f32 = 0.01; // 0.1 units squared

/// Threshold for velocity change.
const VELOCITY_CHANGE_THRESHOLD_SQ: f32 = 0.001;

/// Delta compressor for world snapshots.
///
/// Compares two snapshots and produces a delta containing only changes.
pub struct DeltaCompressor {
    /// Previous snapshot for comparison.
    previous: WorldSnapshot,
    /// Whether we have a valid previous snapshot.
    has_previous: bool,
}

impl DeltaCompressor {
    /// Creates a new compressor.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            previous: WorldSnapshot::empty(0),
            has_previous: false,
        }
    }

    /// Resets the compressor state.
    pub fn reset(&mut self) {
        self.has_previous = false;
    }

    /// Compresses a snapshot relative to the previous one.
    ///
    /// Returns None if a full snapshot should be sent instead.
    pub fn compress(&mut self, current: &WorldSnapshot) -> Option<DeltaSnapshot> {
        if !self.has_previous {
            // First snapshot - must send full
            self.previous = *current;
            self.has_previous = true;
            return None;
        }

        let mut delta = DeltaSnapshot::empty(current.tick, self.previous.tick);
        
        // Find changed and new entities
        for i in 0..current.entity_count as usize {
            let current_entity = &current.entities[i];
            
            // Find this entity in previous snapshot
            let prev_entity = self.find_entity(current_entity.entity_id);
            
            match prev_entity {
                Some(prev) => {
                    // Check if changed
                    if Self::entity_changed(prev, current_entity) {
                        if delta.changed_count as usize >= DeltaSnapshot::MAX_CHANGES {
                            // Too many changes - send full snapshot
                            self.previous = *current;
                            return None;
                        }
                        delta.changed[delta.changed_count as usize] = *current_entity;
                        delta.changed_count += 1;
                    }
                }
                None => {
                    // New entity
                    if delta.changed_count as usize >= DeltaSnapshot::MAX_CHANGES {
                        self.previous = *current;
                        return None;
                    }
                    delta.changed[delta.changed_count as usize] = *current_entity;
                    delta.changed_count += 1;
                }
            }
        }

        // Find removed entities
        for i in 0..self.previous.entity_count as usize {
            let prev_entity = &self.previous.entities[i];
            
            let still_exists = (0..current.entity_count as usize)
                .any(|j| current.entities[j].entity_id == prev_entity.entity_id);
            
            if !still_exists {
                if delta.removed_count as usize >= DeltaSnapshot::MAX_REMOVED {
                    self.previous = *current;
                    return None;
                }
                delta.removed[delta.removed_count as usize] = prev_entity.entity_id;
                delta.removed_count += 1;
            }
        }

        // If delta is larger than would be saved, send full snapshot
        let delta_size = delta.changed_count as usize * std::mem::size_of::<EntityState>()
            + delta.removed_count as usize * 4
            + 12; // header
        
        let full_size = current.entity_count as usize * std::mem::size_of::<EntityState>() + 24;
        
        if delta_size >= full_size {
            self.previous = *current;
            return None;
        }

        self.previous = *current;
        Some(delta)
    }

    /// Finds an entity in the previous snapshot.
    fn find_entity(&self, entity_id: u32) -> Option<&EntityState> {
        (0..self.previous.entity_count as usize)
            .map(|i| &self.previous.entities[i])
            .find(|e| e.entity_id == entity_id)
    }

    /// Checks if an entity has changed significantly.
    fn entity_changed(prev: &EntityState, current: &EntityState) -> bool {
        // Position change
        let dx = current.pos_x - prev.pos_x;
        let dy = current.pos_y - prev.pos_y;
        let dz = current.pos_z - prev.pos_z;
        let pos_change = dx * dx + dy * dy + dz * dz;
        
        if pos_change > POSITION_CHANGE_THRESHOLD_SQ {
            return true;
        }

        // Velocity change
        let dvx = current.vel_x - prev.vel_x;
        let dvy = current.vel_y - prev.vel_y;
        let dvz = current.vel_z - prev.vel_z;
        let vel_change = dvx * dvx + dvy * dvy + dvz * dvz;
        
        if vel_change > VELOCITY_CHANGE_THRESHOLD_SQ {
            return true;
        }

        // Health/rotation/flags change
        if current.health != prev.health
            || current.rotation != prev.rotation
            || current.flags != prev.flags
        {
            return true;
        }

        false
    }
}

impl Default for DeltaCompressor {
    fn default() -> Self {
        Self::new()
    }
}

/// Bit packer for maximum compression.
///
/// Packs values using only the bits required.
pub struct BitPacker {
    buffer: [u8; 1200],
    bit_position: usize,
}

impl BitPacker {
    /// Creates a new bit packer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buffer: [0u8; 1200],
            bit_position: 0,
        }
    }

    /// Resets the packer.
    pub fn reset(&mut self) {
        self.bit_position = 0;
        self.buffer = [0u8; 1200];
    }

    /// Returns the number of bytes written (rounded up).
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        (self.bit_position + 7) / 8
    }

    /// Returns the packed data.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer[..self.byte_len()]
    }

    /// Writes bits to the buffer.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to write
    /// * `bits` - Number of bits to write (1-32)
    pub fn write_bits(&mut self, value: u32, bits: u8) -> bool {
        debug_assert!(bits <= 32 && bits > 0);
        
        let required_bytes = (self.bit_position + bits as usize + 7) / 8;
        if required_bytes > self.buffer.len() {
            return false;
        }

        let mask = if bits == 32 { u32::MAX } else { (1u32 << bits) - 1 };
        let value = value & mask;

        for i in 0..bits as usize {
            let bit = (value >> i) & 1;
            let byte_idx = self.bit_position / 8;
            let bit_idx = self.bit_position % 8;
            
            if bit == 1 {
                self.buffer[byte_idx] |= 1 << bit_idx;
            }
            
            self.bit_position += 1;
        }

        true
    }

    /// Writes a boolean (1 bit).
    #[inline]
    pub fn write_bool(&mut self, value: bool) -> bool {
        self.write_bits(u32::from(value), 1)
    }

    /// Writes a quantized float.
    ///
    /// # Arguments
    ///
    /// * `value` - Float value
    /// * `min` - Minimum expected value
    /// * `max` - Maximum expected value
    /// * `bits` - Number of bits to use
    pub fn write_quantized_float(&mut self, value: f32, min: f32, max: f32, bits: u8) -> bool {
        let range = max - min;
        if range <= 0.0 {
            return self.write_bits(0, bits);
        }

        let normalized = ((value - min) / range).clamp(0.0, 1.0);
        let max_int = (1u32 << bits) - 1;
        let quantized = (normalized * max_int as f32) as u32;
        
        self.write_bits(quantized, bits)
    }
}

impl Default for BitPacker {
    fn default() -> Self {
        Self::new()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_compression() {
        let mut compressor = DeltaCompressor::new();
        
        // First snapshot - should return None (send full)
        let mut snap1 = WorldSnapshot::empty(1);
        snap1.add_entity(EntityState {
            entity_id: 1,
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            ..Default::default()
        });
        
        assert!(compressor.compress(&snap1).is_none());

        // Second snapshot with small change - should return delta
        let mut snap2 = WorldSnapshot::empty(2);
        snap2.add_entity(EntityState {
            entity_id: 1,
            pos_x: 1.0, // Moved
            pos_y: 0.0,
            pos_z: 0.0,
            ..Default::default()
        });
        
        let delta = compressor.compress(&snap2).unwrap();
        assert_eq!(delta.changed_count, 1);
        assert_eq!(delta.removed_count, 0);
    }

    #[test]
    fn test_bit_packer() {
        let mut packer = BitPacker::new();
        
        packer.write_bits(0b101, 3);
        packer.write_bits(0b1111, 4);
        packer.write_bool(true);
        
        // Verify bytes written
        assert!(packer.byte_len() == 1);
    }

    #[test]
    fn test_quantized_float() {
        let mut packer = BitPacker::new();
        
        // Position: -100 to 100, 16 bits (~0.003 precision)
        assert!(packer.write_quantized_float(42.5, -100.0, 100.0, 16));
        
        // Verify 2 bytes were written (16 bits)
        assert_eq!(packer.byte_len(), 2);
    }
}
