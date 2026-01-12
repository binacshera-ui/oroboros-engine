//! GPU buffer management for instanced rendering.
//!
//! Pre-allocates large GPU buffers to avoid runtime allocations.

use super::instance_data::{InstanceData, DrawIndexedIndirectCommand};

/// Maximum instances per frame.
/// 1 million quads should handle even the densest scenes.
pub const MAX_INSTANCES: usize = 1_000_000;

/// Pre-allocated buffer for instance data.
///
/// This buffer is double-buffered to allow CPU writes while GPU reads.
pub struct InstanceBuffer {
    /// CPU-side staging buffer (double buffered).
    staging: [Vec<InstanceData>; 2],
    
    /// Current write buffer index.
    write_index: usize,
    
    /// Number of instances in current frame.
    instance_count: usize,
    
    /// Indirect draw command.
    indirect_command: DrawIndexedIndirectCommand,
}

impl InstanceBuffer {
    /// Creates a new instance buffer with pre-allocated capacity.
    ///
    /// # Note
    /// This allocates significant memory. Call once during initialization.
    #[must_use]
    pub fn new() -> Self {
        Self {
            staging: [
                Vec::with_capacity(MAX_INSTANCES),
                Vec::with_capacity(MAX_INSTANCES),
            ],
            write_index: 0,
            instance_count: 0,
            indirect_command: DrawIndexedIndirectCommand::for_quads(0),
        }
    }
    
    /// Begins a new frame, clearing the write buffer.
    pub fn begin_frame(&mut self) {
        self.write_index = 1 - self.write_index;
        self.staging[self.write_index].clear();
        self.instance_count = 0;
    }
    
    /// Adds an instance to the buffer.
    ///
    /// Returns false if the buffer is full.
    #[inline]
    pub fn push(&mut self, instance: InstanceData) -> bool {
        if self.instance_count >= MAX_INSTANCES {
            return false;
        }
        
        self.staging[self.write_index].push(instance);
        self.instance_count += 1;
        true
    }
    
    /// Adds multiple instances to the buffer.
    ///
    /// Returns the number of instances actually added.
    pub fn push_batch(&mut self, instances: &[InstanceData]) -> usize {
        let available = MAX_INSTANCES - self.instance_count;
        let to_add = instances.len().min(available);
        
        self.staging[self.write_index].extend_from_slice(&instances[..to_add]);
        self.instance_count += to_add;
        
        to_add
    }
    
    /// Finishes the frame and returns the data to upload.
    pub fn end_frame(&mut self) -> (&[InstanceData], DrawIndexedIndirectCommand) {
        self.indirect_command.instance_count = self.instance_count as u32;
        (&self.staging[self.write_index], self.indirect_command)
    }
    
    /// Returns the current instance count.
    #[must_use]
    pub const fn instance_count(&self) -> usize {
        self.instance_count
    }
    
    /// Returns the read buffer (for GPU upload while writing to other buffer).
    #[must_use]
    pub fn read_buffer(&self) -> &[InstanceData] {
        &self.staging[1 - self.write_index]
    }
    
    /// Returns the write buffer as bytes for GPU upload.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.staging[self.write_index])
    }
}

impl Default for InstanceBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_double_buffering() {
        let mut buffer = InstanceBuffer::new();
        
        // First frame
        buffer.begin_frame();
        buffer.push(InstanceData::default());
        assert_eq!(buffer.instance_count(), 1);
        
        // Second frame should start fresh
        buffer.begin_frame();
        assert_eq!(buffer.instance_count(), 0);
    }
    
    #[test]
    fn test_batch_push() {
        let mut buffer = InstanceBuffer::new();
        buffer.begin_frame();
        
        let instances = vec![InstanceData::default(); 100];
        let added = buffer.push_batch(&instances);
        
        assert_eq!(added, 100);
        assert_eq!(buffer.instance_count(), 100);
    }
}
