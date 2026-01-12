//! # Replay System
//!
//! Records and plays back game sessions for analysis.
//!
//! ## Modules
//!
//! - **Full replay**: Complete recording (debugging, huge storage)
//! - **Compressed replay**: Keyframe + Delta + Suspicious events only (production)
//!
//! ## Storage Calculations
//!
//! Without compression: 500 players * 60 FPS * 50 bytes = 5.4 GB/hour
//! With compression: ~50 MB/hour (100x reduction)
//!
//! ## File Format
//!
//! ```text
//! ┌────────────────────────────────────────────────────┐
//! │ Header (64 bytes)                                  │
//! ├────────────────────────────────────────────────────┤
//! │ Magic (4) │ Version (4) │ Tick Rate (4) │ ...     │
//! ├────────────────────────────────────────────────────┤
//! │ Frame 0 (variable)                                 │
//! ├────────────────────────────────────────────────────┤
//! │ Frame 1 (variable)                                 │
//! ├────────────────────────────────────────────────────┤
//! │ ...                                                │
//! └────────────────────────────────────────────────────┘
//! ```

pub mod compressed;

pub use compressed::{
    CompressedReplayRecorder, CompressedReplayConfig, RecordingMode,
    CompressionStats, SuspiciousEvent, SuspiciousEventType, SuspiciousEventData,
};

use bytemuck::{Pod, Zeroable};
use oroboros_core::Position;
use oroboros_networking::protocol::{PlayerInput, EntityState, DragonState};
use std::io::{self, Read, Write};

/// Magic number for replay files.
pub const REPLAY_MAGIC: u32 = 0x4F524F42; // "OROB"

/// Current replay format version.
pub const REPLAY_VERSION: u32 = 1;

/// Maximum inputs per frame.
const MAX_INPUTS_PER_FRAME: usize = 64;

/// Maximum entities per frame.
const MAX_ENTITIES_PER_FRAME: usize = 64;

/// Replay file header.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ReplayHeader {
    /// Magic number for file identification.
    pub magic: u32,
    /// Format version.
    pub version: u32,
    /// Server tick rate.
    pub tick_rate: u32,
    /// Total number of frames.
    pub frame_count: u32,
    /// Duration in ticks.
    pub duration_ticks: u32,
    /// Start timestamp (Unix epoch seconds).
    pub start_timestamp: u64,
    /// World ID (Inferno = 3).
    pub world_id: u32,
    /// Number of players.
    pub player_count: u32,
    /// Reserved for future use.
    pub reserved: [u8; 24],
}

impl ReplayHeader {
    /// Size of header in bytes.
    pub const SIZE: usize = 64;

    /// Creates a new header.
    #[must_use]
    pub fn new(tick_rate: u32, world_id: u32) -> Self {
        Self {
            magic: REPLAY_MAGIC,
            version: REPLAY_VERSION,
            tick_rate,
            frame_count: 0,
            duration_ticks: 0,
            start_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            world_id,
            player_count: 0,
            reserved: [0; 24],
        }
    }

    /// Serializes the header to bytes.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..4].copy_from_slice(&self.magic.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.version.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.tick_rate.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.frame_count.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.duration_ticks.to_le_bytes());
        bytes[20..28].copy_from_slice(&self.start_timestamp.to_le_bytes());
        bytes[28..32].copy_from_slice(&self.world_id.to_le_bytes());
        bytes[32..36].copy_from_slice(&self.player_count.to_le_bytes());
        bytes[36..60].copy_from_slice(&self.reserved);
        bytes
    }

    /// Deserializes header from bytes.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        Self {
            magic: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            version: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            tick_rate: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            frame_count: u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
            duration_ticks: u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
            start_timestamp: u64::from_le_bytes([
                bytes[20], bytes[21], bytes[22], bytes[23],
                bytes[24], bytes[25], bytes[26], bytes[27],
            ]),
            world_id: u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]),
            player_count: u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]),
            reserved: bytes[36..60].try_into().unwrap_or([0; 24]),
        }
    }

    /// Validates the header.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.magic == REPLAY_MAGIC && self.version <= REPLAY_VERSION
    }
}

impl Default for ReplayHeader {
    fn default() -> Self {
        Self::new(60, 3)
    }
}

/// Input record in a frame.
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct InputRecord {
    /// Player/connection ID.
    pub player_id: u32,
    /// The input.
    pub input: PlayerInput,
}

/// A single frame of replay data.
#[derive(Clone, Debug)]
pub struct ReplayFrame {
    /// Server tick number.
    pub tick: u32,
    /// Dragon state at this tick.
    pub dragon: DragonState,
    /// All player inputs this tick.
    pub inputs: Vec<InputRecord>,
    /// All entity states this tick.
    pub entities: Vec<EntityState>,
}

impl ReplayFrame {
    /// Creates a new empty frame.
    #[must_use]
    pub fn new(tick: u32) -> Self {
        Self {
            tick,
            dragon: DragonState::new(tick, 0),
            inputs: Vec::with_capacity(MAX_INPUTS_PER_FRAME),
            entities: Vec::with_capacity(MAX_ENTITIES_PER_FRAME),
        }
    }

    /// Adds an input record.
    pub fn add_input(&mut self, player_id: u32, input: PlayerInput) {
        if self.inputs.len() < MAX_INPUTS_PER_FRAME {
            self.inputs.push(InputRecord { player_id, input });
        }
    }

    /// Adds an entity state.
    pub fn add_entity(&mut self, entity: EntityState) {
        if self.entities.len() < MAX_ENTITIES_PER_FRAME {
            self.entities.push(entity);
        }
    }

    /// Serializes the frame to bytes.
    pub fn serialize(&self, buffer: &mut Vec<u8>) {
        // Tick
        buffer.extend_from_slice(&self.tick.to_le_bytes());
        
        // Dragon state
        buffer.extend_from_slice(bytemuck::bytes_of(&self.dragon));
        
        // Input count and inputs
        buffer.extend_from_slice(&(self.inputs.len() as u16).to_le_bytes());
        for input in &self.inputs {
            buffer.extend_from_slice(bytemuck::bytes_of(input));
        }
        
        // Entity count and entities
        buffer.extend_from_slice(&(self.entities.len() as u16).to_le_bytes());
        for entity in &self.entities {
            buffer.extend_from_slice(bytemuck::bytes_of(entity));
        }
    }

    /// Deserializes a frame from bytes.
    pub fn deserialize(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 4 + 16 + 2 + 2 {
            return None;
        }

        let mut pos = 0;

        // Tick
        let tick = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
        pos += 4;

        // Dragon state
        if pos + 16 > data.len() {
            return None;
        }
        let dragon: DragonState = *bytemuck::from_bytes(&data[pos..pos+16]);
        pos += 16;

        // Inputs
        if pos + 2 > data.len() {
            return None;
        }
        let input_count = u16::from_le_bytes([data[pos], data[pos+1]]) as usize;
        pos += 2;

        let input_size = std::mem::size_of::<InputRecord>();
        if pos + input_count * input_size > data.len() {
            return None;
        }

        let mut inputs = Vec::with_capacity(input_count);
        for _ in 0..input_count {
            let record: InputRecord = *bytemuck::from_bytes(&data[pos..pos+input_size]);
            inputs.push(record);
            pos += input_size;
        }

        // Entities
        if pos + 2 > data.len() {
            return None;
        }
        let entity_count = u16::from_le_bytes([data[pos], data[pos+1]]) as usize;
        pos += 2;

        let entity_size = std::mem::size_of::<EntityState>();
        if pos + entity_count * entity_size > data.len() {
            return None;
        }

        let mut entities = Vec::with_capacity(entity_count);
        for _ in 0..entity_count {
            let entity: EntityState = *bytemuck::from_bytes(&data[pos..pos+entity_size]);
            entities.push(entity);
            pos += entity_size;
        }

        Some((Self { tick, dragon, inputs, entities }, pos))
    }
}

/// Replay recorder - captures game state.
pub struct ReplayRecorder {
    /// Header information.
    header: ReplayHeader,
    /// Buffered frames.
    frames: Vec<ReplayFrame>,
    /// Current frame being built.
    current_frame: Option<ReplayFrame>,
    /// Whether recording is active.
    recording: bool,
}

impl ReplayRecorder {
    /// Creates a new recorder.
    #[must_use]
    pub fn new(tick_rate: u32) -> Self {
        Self {
            header: ReplayHeader::new(tick_rate, 3), // Inferno = world 3
            frames: Vec::with_capacity(60 * 60), // 1 minute at 60Hz
            current_frame: None,
            recording: false,
        }
    }

    /// Starts recording.
    pub fn start(&mut self) {
        self.recording = true;
        self.frames.clear();
        self.header.frame_count = 0;
        self.header.duration_ticks = 0;
        self.header.start_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }

    /// Stops recording.
    pub fn stop(&mut self) {
        self.recording = false;
        // Finalize current frame
        if let Some(frame) = self.current_frame.take() {
            self.frames.push(frame);
        }
        self.header.frame_count = self.frames.len() as u32;
        if let Some(last) = self.frames.last() {
            self.header.duration_ticks = last.tick;
        }
    }

    /// Returns whether recording is active.
    #[must_use]
    pub const fn is_recording(&self) -> bool {
        self.recording
    }

    /// Begins a new frame.
    pub fn begin_frame(&mut self, tick: u32) {
        if !self.recording {
            return;
        }

        // Save previous frame
        if let Some(frame) = self.current_frame.take() {
            self.frames.push(frame);
        }

        self.current_frame = Some(ReplayFrame::new(tick));
    }

    /// Records a player input.
    pub fn record_input(&mut self, player_id: u32, input: PlayerInput) {
        if let Some(ref mut frame) = self.current_frame {
            frame.add_input(player_id, input);
        }
    }

    /// Records an entity state.
    pub fn record_entity(&mut self, entity: EntityState) {
        if let Some(ref mut frame) = self.current_frame {
            frame.add_entity(entity);
        }
    }

    /// Records dragon state.
    pub fn record_dragon(&mut self, dragon: DragonState) {
        if let Some(ref mut frame) = self.current_frame {
            frame.dragon = dragon;
        }
    }

    /// Sets the player count.
    pub fn set_player_count(&mut self, count: u32) {
        self.header.player_count = count;
    }

    /// Writes the replay to a writer.
    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        // Write header
        writer.write_all(&self.header.to_bytes())?;

        // Write frames
        let mut frame_buffer = Vec::with_capacity(4096);
        for frame in &self.frames {
            frame_buffer.clear();
            frame.serialize(&mut frame_buffer);
            writer.write_all(&frame_buffer)?;
        }

        Ok(())
    }

    /// Returns the number of recorded frames.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Returns the frames.
    #[must_use]
    pub fn frames(&self) -> &[ReplayFrame] {
        &self.frames
    }
}

impl Default for ReplayRecorder {
    fn default() -> Self {
        Self::new(60)
    }
}

/// Replay player - plays back recorded games.
pub struct ReplayPlayer {
    /// Header information.
    header: ReplayHeader,
    /// All frames.
    frames: Vec<ReplayFrame>,
    /// Current playback position.
    position: usize,
    /// Playback speed (1.0 = normal).
    speed: f32,
    /// Whether playback is paused.
    paused: bool,
}

impl ReplayPlayer {
    /// Loads a replay from a reader.
    pub fn load<R: Read>(reader: &mut R) -> io::Result<Self> {
        // Read header
        let mut header_bytes = [0u8; ReplayHeader::SIZE];
        reader.read_exact(&mut header_bytes)?;
        let header = ReplayHeader::from_bytes(&header_bytes);

        if !header.is_valid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid replay file",
            ));
        }

        // Read all frames
        let mut frames = Vec::with_capacity(header.frame_count as usize);
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;

        let mut pos = 0;
        while pos < data.len() {
            if let Some((frame, consumed)) = ReplayFrame::deserialize(&data[pos..]) {
                frames.push(frame);
                pos += consumed;
            } else {
                break;
            }
        }

        Ok(Self {
            header,
            frames,
            position: 0,
            speed: 1.0,
            paused: false,
        })
    }

    /// Returns the header.
    #[must_use]
    pub const fn header(&self) -> &ReplayHeader {
        &self.header
    }

    /// Returns the current frame.
    #[must_use]
    pub fn current_frame(&self) -> Option<&ReplayFrame> {
        self.frames.get(self.position)
    }

    /// Advances to the next frame.
    pub fn next_frame(&mut self) -> Option<&ReplayFrame> {
        if self.paused {
            return self.current_frame();
        }

        if self.position < self.frames.len() {
            self.position += 1;
        }
        self.current_frame()
    }

    /// Goes to a specific tick.
    pub fn seek_to_tick(&mut self, tick: u32) {
        self.position = self.frames
            .iter()
            .position(|f| f.tick >= tick)
            .unwrap_or(self.frames.len().saturating_sub(1));
    }

    /// Pauses playback.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resumes playback.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Resets to the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// Sets playback speed.
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.max(0.1).min(10.0);
    }

    /// Returns whether playback is finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.position >= self.frames.len()
    }

    /// Returns the total number of frames.
    #[must_use]
    pub fn total_frames(&self) -> usize {
        self.frames.len()
    }

    /// Returns the current position.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    /// Gets entity position at current frame.
    #[must_use]
    pub fn get_entity_position(&self, entity_id: u32) -> Option<Position> {
        let frame = self.current_frame()?;
        frame.entities
            .iter()
            .find(|e| e.entity_id == entity_id)
            .map(|e| e.position())
    }

    /// Gets all inputs for a player in the current frame.
    #[must_use]
    pub fn get_player_inputs(&self, player_id: u32) -> Vec<&PlayerInput> {
        self.current_frame()
            .map(|frame| {
                frame.inputs
                    .iter()
                    .filter(|r| r.player_id == player_id)
                    .map(|r| &r.input)
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_validation() {
        let header = ReplayHeader::new(60, 3);
        assert!(header.is_valid());
        
        let mut invalid = header;
        invalid.magic = 0;
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_frame_serialization() {
        let mut frame = ReplayFrame::new(42);
        frame.add_input(1, PlayerInput::new(42, 1));
        frame.add_entity(EntityState {
            entity_id: 1,
            pos_x: 10.0,
            pos_y: 20.0,
            pos_z: 30.0,
            ..Default::default()
        });

        let mut buffer = Vec::new();
        frame.serialize(&mut buffer);

        let (deserialized, _) = ReplayFrame::deserialize(&buffer).unwrap();
        assert_eq!(deserialized.tick, 42);
        assert_eq!(deserialized.inputs.len(), 1);
        assert_eq!(deserialized.entities.len(), 1);
        assert_eq!(deserialized.entities[0].pos_x, 10.0);
    }

    #[test]
    fn test_recorder() {
        let mut recorder = ReplayRecorder::new(60);
        recorder.start();

        for tick in 0..10 {
            recorder.begin_frame(tick);
            recorder.record_input(1, PlayerInput::new(tick, tick));
            recorder.record_entity(EntityState {
                entity_id: 1,
                pos_x: tick as f32,
                ..Default::default()
            });
        }

        recorder.stop();

        assert_eq!(recorder.frame_count(), 10);
    }

    #[test]
    fn test_write_and_load() {
        let mut recorder = ReplayRecorder::new(60);
        recorder.start();

        for tick in 0..5 {
            recorder.begin_frame(tick);
            recorder.record_input(1, PlayerInput::new(tick, tick));
        }

        recorder.stop();

        // Write to buffer
        let mut buffer = Vec::new();
        recorder.write(&mut buffer).unwrap();

        // Load back
        let mut cursor = std::io::Cursor::new(buffer);
        let player = ReplayPlayer::load(&mut cursor).unwrap();

        assert_eq!(player.total_frames(), 5);
        assert_eq!(player.header().tick_rate, 60);
    }
}
