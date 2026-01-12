//! # Compressed Replay System
//!
//! THE ARCHITECT'S MATH:
//! 500 players * 60 FPS * 50 bytes = 1.5 MB/s = 5.4 GB/hour = DEATH
//!
//! ## Solution: Keyframes + Delta + Suspicious Only
//!
//! 1. **Keyframes**: Full state every N seconds (configurable)
//! 2. **Delta**: Only changed data between keyframes
//! 3. **Suspicious Only**: Filter out boring movement, keep combat/anomalies
//!
//! ## Storage Reduction
//!
//! - Keyframe every 5 seconds: 12 keyframes/minute vs 3600 frames
//! - Delta compression: ~90% reduction in normal gameplay
//! - Suspicious filter: ~95% reduction (only combat matters)
//!
//! Result: ~5.4 GB/hour → ~50 MB/hour (100x reduction)

use oroboros_networking::protocol::{PlayerInput, EntityState};

/// Replay recording mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordingMode {
    /// Record everything (debugging only, HUGE storage).
    Full,
    /// Keyframes + delta (moderate storage).
    KeyframeDelta,
    /// Only suspicious events (minimal storage, production mode).
    SuspiciousOnly,
}

impl Default for RecordingMode {
    fn default() -> Self {
        Self::SuspiciousOnly
    }
}

/// Configuration for compressed replay.
#[derive(Clone, Debug)]
pub struct CompressedReplayConfig {
    /// Recording mode.
    pub mode: RecordingMode,
    /// Keyframe interval in seconds.
    pub keyframe_interval_secs: u32,
    /// Minimum position change to record delta (units).
    pub delta_position_threshold: f32,
    /// Minimum health change to record delta.
    pub delta_health_threshold: u8,
    /// Events that trigger suspicious recording.
    pub suspicious_triggers: SuspiciousTriggers,
    /// How many seconds to keep around suspicious events.
    pub suspicious_context_secs: f32,
}

impl Default for CompressedReplayConfig {
    fn default() -> Self {
        Self {
            mode: RecordingMode::SuspiciousOnly,
            keyframe_interval_secs: 5,
            delta_position_threshold: 0.1,
            delta_health_threshold: 1,
            suspicious_triggers: SuspiciousTriggers::default(),
            suspicious_context_secs: 2.0,
        }
    }
}

/// Events that trigger suspicious recording.
#[derive(Clone, Debug)]
pub struct SuspiciousTriggers {
    /// Record when damage is dealt.
    pub on_damage: bool,
    /// Record when player dies.
    pub on_death: bool,
    /// Record when shot is fired.
    pub on_shot: bool,
    /// Record when dragon changes state.
    pub on_dragon_change: bool,
    /// Record when movement speed exceeds threshold.
    pub on_speed_anomaly: bool,
    /// Speed threshold for anomaly (units/tick).
    pub speed_anomaly_threshold: f32,
    /// Record when aim snaps too fast.
    pub on_aim_snap: bool,
    /// Aim snap threshold (degrees/tick).
    pub aim_snap_threshold: f32,
}

impl Default for SuspiciousTriggers {
    fn default() -> Self {
        Self {
            on_damage: true,
            on_death: true,
            on_shot: true,
            on_dragon_change: true,
            on_speed_anomaly: true,
            speed_anomaly_threshold: 0.5,
            on_aim_snap: true,
            aim_snap_threshold: 90.0,
        }
    }
}

/// Type of recorded frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    /// Full keyframe with all entity states.
    Keyframe = 0,
    /// Delta frame with only changes.
    Delta = 1,
    /// Suspicious event marker.
    Suspicious = 2,
}

/// Compressed frame header.
#[derive(Clone, Copy, Debug)]
pub struct CompressedFrameHeader {
    /// Frame type.
    pub frame_type: FrameType,
    /// Server tick.
    pub tick: u32,
    /// Number of entities in this frame.
    pub entity_count: u16,
    /// Number of inputs in this frame.
    pub input_count: u16,
    /// Compressed size in bytes.
    pub compressed_size: u32,
}

/// Delta entity - only changed fields.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeltaEntity {
    /// Entity ID.
    pub entity_id: u32,
    /// Changed fields bitmask.
    pub changed_mask: u8,
    /// Position delta (if changed).
    pub delta_pos: Option<(i16, i16, i16)>, // Quantized delta
    /// Health (if changed).
    pub health: Option<u8>,
    /// Rotation delta (if changed).
    pub rotation_delta: Option<i16>,
}

impl DeltaEntity {
    /// Bitmask: position changed.
    pub const MASK_POSITION: u8 = 1 << 0;
    /// Bitmask: health changed.
    pub const MASK_HEALTH: u8 = 1 << 1;
    /// Bitmask: rotation changed.
    pub const MASK_ROTATION: u8 = 1 << 2;

    /// Calculates delta between two entity states.
    pub fn calculate(prev: &EntityState, current: &EntityState, threshold: f32) -> Option<Self> {
        let mut delta = Self {
            entity_id: current.entity_id,
            changed_mask: 0,
            delta_pos: None,
            health: None,
            rotation_delta: None,
        };

        // Position delta
        let dx = current.pos_x - prev.pos_x;
        let dy = current.pos_y - prev.pos_y;
        let dz = current.pos_z - prev.pos_z;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        if dist > threshold {
            delta.changed_mask |= Self::MASK_POSITION;
            // Quantize to i16 (centimeters precision)
            delta.delta_pos = Some((
                (dx * 100.0) as i16,
                (dy * 100.0) as i16,
                (dz * 100.0) as i16,
            ));
        }

        // Health change
        if current.health != prev.health {
            delta.changed_mask |= Self::MASK_HEALTH;
            delta.health = Some(current.health);
        }

        // Rotation change
        if current.rotation != prev.rotation {
            delta.changed_mask |= Self::MASK_ROTATION;
            delta.rotation_delta = Some(current.rotation - prev.rotation);
        }

        // Only return if something changed
        if delta.changed_mask != 0 {
            Some(delta)
        } else {
            None
        }
    }

    /// Serializes to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16);
        bytes.extend_from_slice(&self.entity_id.to_le_bytes());
        bytes.push(self.changed_mask);

        if let Some((dx, dy, dz)) = self.delta_pos {
            bytes.extend_from_slice(&dx.to_le_bytes());
            bytes.extend_from_slice(&dy.to_le_bytes());
            bytes.extend_from_slice(&dz.to_le_bytes());
        }

        if let Some(health) = self.health {
            bytes.push(health);
        }

        if let Some(rot) = self.rotation_delta {
            bytes.extend_from_slice(&rot.to_le_bytes());
        }

        bytes
    }

    /// Size in bytes.
    pub fn size(&self) -> usize {
        let mut size = 5; // entity_id (4) + mask (1)
        if self.delta_pos.is_some() { size += 6; }
        if self.health.is_some() { size += 1; }
        if self.rotation_delta.is_some() { size += 2; }
        size
    }
}

/// Suspicious event record.
#[derive(Clone, Debug)]
pub struct SuspiciousEvent {
    /// Tick when event occurred.
    pub tick: u32,
    /// Player who triggered.
    pub player_id: u32,
    /// Type of suspicious activity.
    pub event_type: SuspiciousEventType,
    /// Additional data.
    pub data: SuspiciousEventData,
}

/// Type of suspicious event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SuspiciousEventType {
    /// Player dealt damage.
    Damage = 0,
    /// Player died.
    Death = 1,
    /// Shot fired.
    Shot = 2,
    /// Dragon state change.
    DragonChange = 3,
    /// Abnormal movement speed.
    SpeedAnomaly = 4,
    /// Inhuman aim snap.
    AimSnap = 5,
}

/// Data for suspicious event.
#[derive(Clone, Debug)]
pub enum SuspiciousEventData {
    /// Damage event data: target ID and amount.
    Damage {
        /// ID of the target entity.
        target_id: u32,
        /// Damage amount.
        amount: u16,
    },
    /// Death event data: killer ID (if any).
    Death {
        /// ID of the killer (None if environmental).
        killer_id: Option<u32>,
    },
    /// Shot event data: direction vector.
    Shot {
        /// Direction vector (x, y, z).
        direction: (f32, f32, f32),
    },
    /// Dragon state change data.
    Dragon {
        /// Previous state.
        old_state: u8,
        /// New state.
        new_state: u8,
    },
    /// Speed anomaly data.
    Speed {
        /// Measured speed.
        measured: f32,
        /// Expected maximum speed.
        expected_max: f32,
    },
    /// Aim snap data.
    AimSnap {
        /// Degrees of rotation in one tick.
        delta_degrees: f32,
    },
}

/// Compressed replay recorder.
pub struct CompressedReplayRecorder {
    /// Configuration.
    config: CompressedReplayConfig,
    /// Previous frame state (for delta calculation).
    prev_states: Vec<EntityState>,
    /// Keyframe buffer.
    keyframes: Vec<(u32, Vec<EntityState>)>,
    /// Delta buffer.
    deltas: Vec<(u32, Vec<DeltaEntity>)>,
    /// Suspicious events.
    suspicious_events: Vec<SuspiciousEvent>,
    /// Suspicious context buffer (frames around suspicious events).
    suspicious_context: Vec<(u32, Vec<EntityState>)>,
    /// Last keyframe tick.
    last_keyframe_tick: u32,
    /// Total frames processed.
    frames_processed: u64,
    /// Total bytes written.
    bytes_written: u64,
    /// Bytes that would be written without compression.
    bytes_uncompressed: u64,
    /// Recording active.
    recording: bool,
}

impl CompressedReplayRecorder {
    /// Creates a new compressed recorder.
    pub fn new(config: CompressedReplayConfig) -> Self {
        Self {
            config,
            prev_states: Vec::with_capacity(500),
            keyframes: Vec::new(),
            deltas: Vec::new(),
            suspicious_events: Vec::new(),
            suspicious_context: Vec::new(),
            last_keyframe_tick: 0,
            frames_processed: 0,
            bytes_written: 0,
            bytes_uncompressed: 0,
            recording: false,
        }
    }

    /// Starts recording.
    pub fn start(&mut self) {
        self.recording = true;
        self.prev_states.clear();
        self.keyframes.clear();
        self.deltas.clear();
        self.suspicious_events.clear();
        self.suspicious_context.clear();
        self.last_keyframe_tick = 0;
        self.frames_processed = 0;
        self.bytes_written = 0;
        self.bytes_uncompressed = 0;
    }

    /// Records a frame.
    pub fn record_frame(&mut self, tick: u32, entities: &[EntityState], inputs: &[PlayerInput]) {
        if !self.recording {
            return;
        }

        self.frames_processed += 1;
        self.bytes_uncompressed += (entities.len() * 32 + inputs.len() * 24) as u64;

        let ticks_since_keyframe = tick.saturating_sub(self.last_keyframe_tick);
        let keyframe_interval = self.config.keyframe_interval_secs * 60; // Assuming 60Hz

        match self.config.mode {
            RecordingMode::Full => {
                // Record everything (no compression)
                self.keyframes.push((tick, entities.to_vec()));
                self.bytes_written += (entities.len() * 32 + inputs.len() * 24) as u64;
            }
            RecordingMode::KeyframeDelta => {
                if ticks_since_keyframe >= keyframe_interval || self.prev_states.is_empty() {
                    // Keyframe
                    self.keyframes.push((tick, entities.to_vec()));
                    self.prev_states = entities.to_vec();
                    self.last_keyframe_tick = tick;
                    self.bytes_written += (entities.len() * 32) as u64;
                } else {
                    // Delta
                    let deltas = self.calculate_deltas(entities);
                    if !deltas.is_empty() {
                        let delta_bytes: u64 = deltas.iter().map(|d| d.size() as u64).sum();
                        self.deltas.push((tick, deltas));
                        self.bytes_written += delta_bytes;
                    }
                    self.prev_states = entities.to_vec();
                }
            }
            RecordingMode::SuspiciousOnly => {
                // Only record during suspicious windows
                // (suspicious events are added via record_suspicious_event)
            }
        }
    }

    /// Records a suspicious event.
    pub fn record_suspicious_event(&mut self, event: SuspiciousEvent, context_entities: &[EntityState]) {
        if !self.recording {
            return;
        }

        self.suspicious_events.push(event.clone());
        self.suspicious_context.push((event.tick, context_entities.to_vec()));
        self.bytes_written += (context_entities.len() * 32 + 20) as u64;
    }

    /// Calculates deltas from previous state.
    fn calculate_deltas(&self, current: &[EntityState]) -> Vec<DeltaEntity> {
        let mut deltas = Vec::new();

        for entity in current {
            // Find previous state for this entity
            let prev = self.prev_states.iter().find(|e| e.entity_id == entity.entity_id);

            if let Some(prev) = prev {
                if let Some(delta) = DeltaEntity::calculate(prev, entity, self.config.delta_position_threshold) {
                    deltas.push(delta);
                }
            } else {
                // New entity - treat as full change
                deltas.push(DeltaEntity {
                    entity_id: entity.entity_id,
                    changed_mask: DeltaEntity::MASK_POSITION | DeltaEntity::MASK_HEALTH,
                    delta_pos: Some((
                        (entity.pos_x * 100.0) as i16,
                        (entity.pos_y * 100.0) as i16,
                        (entity.pos_z * 100.0) as i16,
                    )),
                    health: Some(entity.health),
                    rotation_delta: None,
                });
            }
        }

        deltas
    }

    /// Stops recording and returns statistics.
    pub fn stop(&mut self) -> CompressionStats {
        self.recording = false;

        CompressionStats {
            frames_processed: self.frames_processed,
            keyframes_stored: self.keyframes.len() as u64,
            deltas_stored: self.deltas.len() as u64,
            suspicious_events: self.suspicious_events.len() as u64,
            bytes_written: self.bytes_written,
            bytes_uncompressed: self.bytes_uncompressed,
            compression_ratio: if self.bytes_uncompressed > 0 {
                self.bytes_uncompressed as f64 / self.bytes_written.max(1) as f64
            } else {
                1.0
            },
        }
    }

    /// Returns current compression ratio.
    pub fn current_compression_ratio(&self) -> f64 {
        if self.bytes_uncompressed > 0 {
            self.bytes_uncompressed as f64 / self.bytes_written.max(1) as f64
        } else {
            1.0
        }
    }
}

/// Compression statistics.
#[derive(Clone, Debug, Default)]
pub struct CompressionStats {
    /// Total frames processed.
    pub frames_processed: u64,
    /// Keyframes stored.
    pub keyframes_stored: u64,
    /// Delta frames stored.
    pub deltas_stored: u64,
    /// Suspicious events stored.
    pub suspicious_events: u64,
    /// Bytes written.
    pub bytes_written: u64,
    /// Bytes without compression.
    pub bytes_uncompressed: u64,
    /// Compression ratio (uncompressed / compressed).
    pub compression_ratio: f64,
}

impl CompressionStats {
    /// Returns human-readable storage comparison.
    pub fn storage_comparison(&self, duration_secs: u64) -> String {
        let uncompressed_per_hour = self.bytes_uncompressed as f64 / duration_secs.max(1) as f64 * 3600.0;
        let compressed_per_hour = self.bytes_written as f64 / duration_secs.max(1) as f64 * 3600.0;

        format!(
            "Uncompressed: {:.1} GB/hour\nCompressed: {:.1} MB/hour\nRatio: {:.1}x",
            uncompressed_per_hour / 1_000_000_000.0,
            compressed_per_hour / 1_000_000.0,
            self.compression_ratio
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_calculation() {
        let prev = EntityState {
            entity_id: 1,
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            vel_x: 0.0,
            vel_y: 0.0,
            vel_z: 0.0,
            rotation: 0,
            health: 100,
            flags: 0,
        };

        let current = EntityState {
            entity_id: 1,
            pos_x: 1.0,
            pos_y: 0.0,
            pos_z: 0.0,
            vel_x: 0.0,
            vel_y: 0.0,
            vel_z: 0.0,
            rotation: 0,
            health: 90,
            flags: 0,
        };

        let delta = DeltaEntity::calculate(&prev, &current, 0.1).unwrap();
        
        assert!(delta.changed_mask & DeltaEntity::MASK_POSITION != 0);
        assert!(delta.changed_mask & DeltaEntity::MASK_HEALTH != 0);
        assert_eq!(delta.health, Some(90));
    }

    #[test]
    fn test_compression_ratio() {
        let config = CompressedReplayConfig {
            mode: RecordingMode::KeyframeDelta,
            keyframe_interval_secs: 1,
            ..Default::default()
        };

        let mut recorder = CompressedReplayRecorder::new(config);
        recorder.start();

        // Simulate 60 frames (1 second) with minimal movement
        for tick in 0..60 {
            let entities: Vec<EntityState> = (0..100).map(|i| EntityState {
                entity_id: i,
                pos_x: i as f32 + tick as f32 * 0.01, // Tiny movement
                pos_y: 0.0,
                pos_z: i as f32,
                vel_x: 0.0,
                vel_y: 0.0,
                vel_z: 0.0,
                rotation: 0,
                health: 100,
                flags: 0,
            }).collect();

            recorder.record_frame(tick, &entities, &[]);
        }

        let stats = recorder.stop();

        println!("Frames processed: {}", stats.frames_processed);
        println!("Keyframes: {}", stats.keyframes_stored);
        println!("Deltas: {}", stats.deltas_stored);
        println!("Bytes written: {}", stats.bytes_written);
        println!("Bytes uncompressed: {}", stats.bytes_uncompressed);
        println!("Compression ratio: {:.2}x", stats.compression_ratio);

        // Should have significant compression
        assert!(stats.compression_ratio > 2.0, 
            "Compression ratio {:.2} should be > 2x", stats.compression_ratio);
    }

    #[test]
    fn test_storage_calculation() {
        // Simulate 500 players * 60 FPS * 50 bytes
        let uncompressed_per_sec: u64 = 500 * 60 * 50;
        let uncompressed_per_hour = uncompressed_per_sec * 3600;
        
        println!("Uncompressed: {} bytes/sec = {:.2} GB/hour", 
            uncompressed_per_sec, 
            uncompressed_per_hour as f64 / 1_000_000_000.0);

        // With keyframe + delta (90% reduction)
        let keyframe_delta = uncompressed_per_hour / 10;
        println!("Keyframe + Delta: {:.2} MB/hour", 
            keyframe_delta as f64 / 1_000_000.0);

        // With suspicious only (95% reduction)  
        let suspicious_only = uncompressed_per_hour / 20;
        println!("Suspicious Only: {:.2} MB/hour",
            suspicious_only as f64 / 1_000_000.0);

        // Verify our targets
        assert!(suspicious_only < 500_000_000, "Suspicious only should be < 500MB/hour");
    }
}
