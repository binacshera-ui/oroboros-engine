//! # Packet Serialization
//!
//! Zero-allocation serialization for network packets.
//!
//! ## Design
//!
//! - Uses pre-allocated buffers (no heap allocations in hot path)
//! - Bit-packing for maximum compression
//! - Direct memory copies where safe (Pod types)

use bytemuck::{bytes_of, Pod};
use super::packets::*;

/// Sequence number type alias.
pub type SequenceNumber = u16;

/// Acknowledgment bitfield type alias.
pub type AckBitfield = u32;

/// Maximum packet buffer size.
pub const MAX_BUFFER_SIZE: usize = 1200;

/// Packet serializer - writes packets to a pre-allocated buffer.
///
/// This struct is designed to be reused across multiple serializations
/// to avoid allocations.
pub struct PacketSerializer {
    buffer: [u8; MAX_BUFFER_SIZE],
    position: usize,
}

impl PacketSerializer {
    /// Creates a new serializer with a fresh buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buffer: [0u8; MAX_BUFFER_SIZE],
            position: 0,
        }
    }

    /// Resets the serializer for reuse.
    #[inline]
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// Returns the number of bytes written.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.position
    }

    /// Returns true if no bytes have been written.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.position == 0
    }

    /// Returns a slice of the written data.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer[..self.position]
    }

    /// Writes a single byte.
    #[inline]
    pub fn write_u8(&mut self, value: u8) -> bool {
        if self.position >= MAX_BUFFER_SIZE {
            return false;
        }
        self.buffer[self.position] = value;
        self.position += 1;
        true
    }

    /// Writes a u16 in little-endian format.
    #[inline]
    pub fn write_u16(&mut self, value: u16) -> bool {
        if self.position + 2 > MAX_BUFFER_SIZE {
            return false;
        }
        self.buffer[self.position..self.position + 2].copy_from_slice(&value.to_le_bytes());
        self.position += 2;
        true
    }

    /// Writes a u32 in little-endian format.
    #[inline]
    pub fn write_u32(&mut self, value: u32) -> bool {
        if self.position + 4 > MAX_BUFFER_SIZE {
            return false;
        }
        self.buffer[self.position..self.position + 4].copy_from_slice(&value.to_le_bytes());
        self.position += 4;
        true
    }

    /// Writes a f32 in little-endian format.
    #[inline]
    pub fn write_f32(&mut self, value: f32) -> bool {
        if self.position + 4 > MAX_BUFFER_SIZE {
            return false;
        }
        self.buffer[self.position..self.position + 4].copy_from_slice(&value.to_le_bytes());
        self.position += 4;
        true
    }

    /// Writes a Pod type directly.
    #[inline]
    pub fn write_pod<T: Pod>(&mut self, value: &T) -> bool {
        let bytes = bytes_of(value);
        if self.position + bytes.len() > MAX_BUFFER_SIZE {
            return false;
        }
        self.buffer[self.position..self.position + bytes.len()].copy_from_slice(bytes);
        self.position += bytes.len();
        true
    }

    /// Writes a packet header.
    #[inline]
    pub fn write_header(&mut self, header: &PacketHeader) -> bool {
        self.write_pod(header)
    }

    /// Serializes a complete input packet.
    pub fn serialize_input(&mut self, header: &PacketHeader, input: &PlayerInput) -> bool {
        self.reset();
        self.write_u8(PacketType::Input as u8)
            && self.write_header(header)
            && self.write_pod(input)
    }

    /// Serializes a world snapshot packet.
    pub fn serialize_snapshot(&mut self, header: &PacketHeader, snapshot: &WorldSnapshot) -> bool {
        self.reset();
        
        if !self.write_u8(PacketType::Snapshot as u8) {
            return false;
        }
        if !self.write_header(header) {
            return false;
        }
        if !self.write_u32(snapshot.tick) {
            return false;
        }
        if !self.write_u16(snapshot.entity_count) {
            return false;
        }
        if !self.write_pod(&snapshot.dragon) {
            return false;
        }
        
        for i in 0..snapshot.entity_count as usize {
            if !self.write_pod(&snapshot.entities[i]) {
                return false;
            }
        }
        
        true
    }

    /// Serializes a dragon broadcast packet.
    pub fn serialize_dragon(&mut self, header: &PacketHeader, dragon: &DragonState) -> bool {
        self.reset();
        self.write_u8(PacketType::DragonBroadcast as u8)
            && self.write_header(header)
            && self.write_pod(dragon)
    }

    /// Serializes a hit report packet.
    pub fn serialize_hit(&mut self, header: &PacketHeader, hit: &HitReport) -> bool {
        self.reset();
        self.write_u8(PacketType::HitConfirm as u8)
            && self.write_header(header)
            && self.write_pod(hit)
    }

    /// Serializes a connect packet.
    pub fn serialize_connect(&mut self, header: &PacketHeader) -> bool {
        self.reset();
        self.write_u8(PacketType::Connect as u8)
            && self.write_header(header)
    }

    /// Serializes a connect ack packet.
    pub fn serialize_connect_ack(&mut self, header: &PacketHeader, client_id: u32) -> bool {
        self.reset();
        self.write_u8(PacketType::ConnectAck as u8)
            && self.write_header(header)
            && self.write_u32(client_id)
    }

    /// Serializes a heartbeat packet.
    pub fn serialize_heartbeat(&mut self, header: &PacketHeader) -> bool {
        self.reset();
        self.write_u8(PacketType::Heartbeat as u8)
            && self.write_header(header)
    }

    /// Serializes a disconnect packet.
    pub fn serialize_disconnect(&mut self, header: &PacketHeader) -> bool {
        self.reset();
        self.write_u8(PacketType::Disconnect as u8)
            && self.write_header(header)
    }
}

impl Default for PacketSerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Packet deserializer - reads packets from a buffer.
pub struct PacketDeserializer<'a> {
    buffer: &'a [u8],
    position: usize,
}

impl<'a> PacketDeserializer<'a> {
    /// Creates a new deserializer from a buffer.
    #[must_use]
    pub const fn new(buffer: &'a [u8]) -> Self {
        Self { buffer, position: 0 }
    }

    /// Returns the number of bytes remaining.
    #[inline]
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.buffer.len().saturating_sub(self.position)
    }

    /// Reads a single byte.
    #[inline]
    pub fn read_u8(&mut self) -> Option<u8> {
        if self.position >= self.buffer.len() {
            return None;
        }
        let value = self.buffer[self.position];
        self.position += 1;
        Some(value)
    }

    /// Reads a u16 in little-endian format.
    #[inline]
    pub fn read_u16(&mut self) -> Option<u16> {
        if self.position + 2 > self.buffer.len() {
            return None;
        }
        let value = u16::from_le_bytes([
            self.buffer[self.position],
            self.buffer[self.position + 1],
        ]);
        self.position += 2;
        Some(value)
    }

    /// Reads a u32 in little-endian format.
    #[inline]
    pub fn read_u32(&mut self) -> Option<u32> {
        if self.position + 4 > self.buffer.len() {
            return None;
        }
        let value = u32::from_le_bytes([
            self.buffer[self.position],
            self.buffer[self.position + 1],
            self.buffer[self.position + 2],
            self.buffer[self.position + 3],
        ]);
        self.position += 4;
        Some(value)
    }

    /// Reads a f32 in little-endian format.
    #[inline]
    pub fn read_f32(&mut self) -> Option<f32> {
        self.read_u32().map(f32::from_bits)
    }

    /// Reads a Pod type directly.
    #[inline]
    pub fn read_pod<T: Pod + Copy>(&mut self) -> Option<T> {
        let size = std::mem::size_of::<T>();
        if self.position + size > self.buffer.len() {
            return None;
        }
        let slice = &self.buffer[self.position..self.position + size];
        self.position += size;
        // Use try_pod_read_unaligned for safety
        bytemuck::try_pod_read_unaligned(slice).ok()
    }

    /// Reads a packet header.
    #[inline]
    pub fn read_header(&mut self) -> Option<PacketHeader> {
        self.read_pod()
    }

    /// Deserializes a packet from the buffer.
    pub fn deserialize(&mut self) -> Option<Packet> {
        let packet_type_byte = self.read_u8()?;
        let header = self.read_header()?;

        match packet_type_byte {
            x if x == PacketType::Input as u8 => {
                let input = self.read_pod::<PlayerInput>()?;
                Some(Packet::Input(header, input))
            }
            x if x == PacketType::Snapshot as u8 => {
                let tick = self.read_u32()?;
                let entity_count = self.read_u16()?;
                let dragon = self.read_pod::<DragonState>()?;
                
                let mut snapshot = WorldSnapshot::empty(tick);
                snapshot.dragon = dragon;
                
                for _ in 0..entity_count.min(WorldSnapshot::MAX_ENTITIES as u16) {
                    let entity = self.read_pod::<EntityState>()?;
                    snapshot.add_entity(entity);
                }
                
                Some(Packet::Snapshot(header, snapshot))
            }
            x if x == PacketType::DragonBroadcast as u8 => {
                let dragon = self.read_pod::<DragonState>()?;
                Some(Packet::Dragon(header, dragon))
            }
            x if x == PacketType::HitConfirm as u8 => {
                let hit = self.read_pod::<HitReport>()?;
                Some(Packet::Hit(header, hit))
            }
            x if x == PacketType::Connect as u8 => {
                Some(Packet::Connect(header))
            }
            x if x == PacketType::ConnectAck as u8 => {
                let client_id = self.read_u32()?;
                Some(Packet::ConnectAck(header, client_id))
            }
            x if x == PacketType::Heartbeat as u8 => {
                Some(Packet::Heartbeat(header))
            }
            x if x == PacketType::Disconnect as u8 => {
                Some(Packet::Disconnect(header))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_input() {
        let header = PacketHeader::new(1, 0, 0);
        let input = PlayerInput {
            tick: 100,
            input_sequence: 1,
            move_x: 127,
            move_y: 0,
            move_z: -128,
            flags: PlayerInput::FLAG_JUMP | PlayerInput::FLAG_SPRINT,
            aim_yaw: 1800,
            aim_pitch: -450,
            action: PlayerInput::ACTION_SHOOT,
            _padding: [0; 3],
            timestamp: 12345,
        };

        let mut serializer = PacketSerializer::new();
        assert!(serializer.serialize_input(&header, &input));

        let mut deserializer = PacketDeserializer::new(serializer.as_slice());
        let packet = deserializer.deserialize().unwrap();

        if let Packet::Input(h, i) = packet {
            assert_eq!(h.sequence, 1);
            assert_eq!(i.tick, 100);
            assert_eq!(i.move_x, 127);
            assert_eq!(i.move_z, -128);
            assert!(i.is_jumping());
            assert!(i.is_sprinting());
            assert!(i.is_shooting());
        } else {
            panic!("Expected Input packet");
        }
    }

    #[test]
    fn test_serialize_deserialize_snapshot() {
        let header = PacketHeader::new(1, 0, 0);
        let mut snapshot = WorldSnapshot::empty(42);
        snapshot.dragon = DragonState::new(42, DragonState::STATE_STALK);
        
        for i in 0..5 {
            let entity = EntityState {
                entity_id: i,
                pos_x: i as f32 * 10.0,
                pos_y: 100.0,
                pos_z: i as f32 * 5.0,
                ..Default::default()
            };
            snapshot.add_entity(entity);
        }

        let mut serializer = PacketSerializer::new();
        assert!(serializer.serialize_snapshot(&header, &snapshot));

        let mut deserializer = PacketDeserializer::new(serializer.as_slice());
        let packet = deserializer.deserialize().unwrap();

        if let Packet::Snapshot(_, s) = packet {
            assert_eq!(s.tick, 42);
            assert_eq!(s.entity_count, 5);
            assert_eq!(s.dragon.state, DragonState::STATE_STALK);
            assert_eq!(s.entities[0].entity_id, 0);
            assert_eq!(s.entities[4].pos_x, 40.0);
        } else {
            panic!("Expected Snapshot packet");
        }
    }

    #[test]
    fn test_packet_size_under_mtu() {
        let mut serializer = PacketSerializer::new();
        let header = PacketHeader::new(0, 0, 0);
        let mut snapshot = WorldSnapshot::empty(0);
        
        // Fill to max
        for i in 0..WorldSnapshot::MAX_ENTITIES {
            let entity = EntityState {
                entity_id: i as u32,
                ..Default::default()
            };
            snapshot.add_entity(entity);
        }

        assert!(serializer.serialize_snapshot(&header, &snapshot));
        
        // Must be under MTU
        assert!(serializer.len() <= 1200, "Packet too large: {} bytes", serializer.len());
    }
}
