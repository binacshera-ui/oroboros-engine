//! Render Bridge - Connects Unit 2 to Unit 1's Double Buffer
//!
//! This is the ONLY way Unit 2 accesses world data.
//! We read from Buffer B while Unit 4 writes to Buffer A.


/// Configuration for the render bridge
#[derive(Debug, Clone)]
pub struct RenderBridgeConfig {
    /// Maximum entities to render per frame
    pub max_entities: usize,
    /// Maximum chunks to render per frame
    pub max_chunks: usize,
    /// View distance in chunks
    pub view_distance: u32,
    /// Enable debug overlays
    pub debug_mode: bool,
}

impl Default for RenderBridgeConfig {
    fn default() -> Self {
        Self {
            max_entities: 100_000,
            max_chunks: 4096,
            view_distance: 16,
            debug_mode: false,
        }
    }
}

/// Statistics from render bridge operations
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderBridgeStats {
    /// Entities read from ECS
    pub entities_read: u32,
    /// Chunks read from world
    pub chunks_read: u32,
    /// Time spent reading (microseconds)
    pub read_time_us: u32,
    /// Frame number from Core
    pub frame_number: u64,
    /// Buffer index we're reading from
    pub buffer_index: u8,
}

/// The bridge between rendering and Core's double buffer
///
/// IMPORTANT: This does NOT own the world data.
/// It borrows a ReadHandle from Unit 1's DoubleBufferedWorld.
///
/// ## Usage
///
/// ```rust,ignore
/// // Unit 1 creates the world
/// let db_world = DoubleBufferedWorld::new(1_000_000, 100_000);
///
/// // Unit 2 creates the bridge
/// let bridge = RenderBridge::new(RenderBridgeConfig::default());
///
/// // Each frame, Unit 2 gets a read handle and extracts render data
/// let read_handle = db_world.read_handle();
/// let frame_data = bridge.extract_frame_data(&read_handle, camera);
/// ```
pub struct RenderBridge {
    /// Configuration
    config: RenderBridgeConfig,
    /// Statistics from last frame
    stats: RenderBridgeStats,
    /// Pre-allocated buffer for entity transforms
    transform_buffer: Vec<EntityTransform>,
    /// Pre-allocated buffer for chunk render data
    chunk_buffer: Vec<ChunkRenderData>,
}

/// Transform data extracted from ECS for rendering
#[derive(Debug, Clone, Copy, Default)]
pub struct EntityTransform {
    /// Entity ID (for debugging/UI)
    pub entity_id: u64,
    /// World position
    pub position: [f32; 3],
    /// Velocity (for motion blur, interpolation)
    pub velocity: [f32; 3],
    /// Entity type (for model selection)
    pub entity_type: u16,
    /// Flags (visible, highlighted, etc)
    pub flags: u16,
}

/// Chunk data extracted for rendering
#[derive(Debug, Clone, Copy, Default)]
pub struct ChunkRenderData {
    /// Chunk coordinates
    pub coord: [i32; 3],
    /// Mesh offset in instance buffer
    pub mesh_offset: u32,
    /// Number of instances
    pub instance_count: u32,
    /// LOD level (0 = highest detail)
    pub lod: u8,
    /// Flags (dirty, visible, etc)
    pub flags: u8,
}

/// Complete frame data for rendering
#[derive(Debug)]
pub struct FrameData {
    /// Frame number (from Core)
    pub frame_number: u64,
    /// Delta time since last frame
    pub delta_time: f32,
    /// Camera position
    pub camera_pos: [f32; 3],
    /// Camera view-projection matrix
    pub view_proj: [[f32; 4]; 4],
    /// Entity transforms to render
    pub entities: Vec<EntityTransform>,
    /// Chunks to render
    pub chunks: Vec<ChunkRenderData>,
    /// Statistics
    pub stats: RenderBridgeStats,
}

impl RenderBridge {
    /// Creates a new render bridge
    #[must_use]
    pub fn new(config: RenderBridgeConfig) -> Self {
        Self {
            transform_buffer: Vec::with_capacity(config.max_entities),
            chunk_buffer: Vec::with_capacity(config.max_chunks),
            stats: RenderBridgeStats::default(),
            config,
        }
    }

    /// Extracts render data from Unit 1's read buffer
    ///
    /// This is the MAIN INTERFACE between Unit 2 and Unit 1.
    ///
    /// # Type Parameters
    ///
    /// * `W` - Any type that provides archetype iteration
    ///
    /// # Arguments
    ///
    /// * `world` - Read handle from DoubleBufferedWorld
    /// * `camera_pos` - Camera position for culling
    /// * `view_proj` - View-projection matrix for frustum culling
    /// * `delta_time` - Time since last frame
    /// * `frame_number` - Current frame number
    pub fn extract_frame_data<W: WorldReader>(
        &mut self,
        world: &W,
        camera_pos: [f32; 3],
        view_proj: [[f32; 4]; 4],
        delta_time: f32,
        frame_number: u64,
    ) -> FrameData {
        let start = std::time::Instant::now();

        // Clear buffers (no allocation, just reset len)
        self.transform_buffer.clear();
        self.chunk_buffer.clear();

        // Extract entities from ECS
        let entity_count = self.extract_entities(world, camera_pos);

        // Extract visible chunks
        let chunk_count = self.extract_chunks(world, camera_pos, &view_proj);

        let elapsed = start.elapsed();

        // Update stats
        self.stats = RenderBridgeStats {
            entities_read: entity_count,
            chunks_read: chunk_count,
            read_time_us: elapsed.as_micros() as u32,
            frame_number,
            buffer_index: world.buffer_index(),
        };

        // Return frame data (moves the buffers, no copy)
        FrameData {
            frame_number,
            delta_time,
            camera_pos,
            view_proj,
            entities: std::mem::take(&mut self.transform_buffer),
            chunks: std::mem::take(&mut self.chunk_buffer),
            stats: self.stats,
        }
    }

    /// Extracts entity transforms from ECS
    fn extract_entities<W: WorldReader>(&mut self, world: &W, camera_pos: [f32; 3]) -> u32 {
        let mut count = 0u32;
        let max = self.config.max_entities;
        let view_dist_sq = (self.config.view_distance as f32 * 32.0).powi(2);

        // Iterate over Position+Velocity entities
        world.for_each_position_velocity(|entity_id, pos, vel| {
            if count as usize >= max {
                return;
            }

            // Distance culling
            let dx = pos[0] - camera_pos[0];
            let dy = pos[1] - camera_pos[1];
            let dz = pos[2] - camera_pos[2];
            let dist_sq = dx * dx + dy * dy + dz * dz;

            if dist_sq <= view_dist_sq {
                self.transform_buffer.push(EntityTransform {
                    entity_id,
                    position: pos,
                    velocity: vel,
                    entity_type: 0, // TODO: get from component
                    flags: 1, // visible
                });
                count += 1;
            }
        });

        count
    }

    /// Extracts visible chunks
    fn extract_chunks<W: WorldReader>(
        &mut self,
        _world: &W,
        _camera_pos: [f32; 3],
        _view_proj: &[[f32; 4]; 4],
    ) -> u32 {
        // TODO: Integrate with VoxelWorld chunk iteration
        // For now, return 0 - this will be connected to actual chunk data
        0
    }

    /// Returns statistics from the last extract operation
    #[must_use]
    pub fn stats(&self) -> RenderBridgeStats {
        self.stats
    }

    /// Returns the current configuration
    #[must_use]
    pub fn config(&self) -> &RenderBridgeConfig {
        &self.config
    }
}

/// Trait for reading world data
///
/// This abstracts over the actual world implementation,
/// allowing Unit 2 to work with any type that provides entity iteration.
pub trait WorldReader {
    /// Returns the buffer index being read
    fn buffer_index(&self) -> u8;

    /// Iterates over all entities with Position and Velocity
    fn for_each_position_velocity<F>(&self, f: F)
    where
        F: FnMut(u64, [f32; 3], [f32; 3]);

    /// Iterates over all entities with just Position
    fn for_each_position<F>(&self, f: F)
    where
        F: FnMut(u64, [f32; 3]);
}

/// Mock world reader for testing (without Core dependency)
pub struct MockWorldReader {
    /// Entities in the mock world: (id, position, velocity)
    pub entities: Vec<(u64, [f32; 3], [f32; 3])>,
}

impl WorldReader for MockWorldReader {
    fn buffer_index(&self) -> u8 {
        0
    }

    fn for_each_position_velocity<F>(&self, mut f: F)
    where
        F: FnMut(u64, [f32; 3], [f32; 3]),
    {
        for (id, pos, vel) in &self.entities {
            f(*id, *pos, *vel);
        }
    }

    fn for_each_position<F>(&self, mut f: F)
    where
        F: FnMut(u64, [f32; 3]),
    {
        for (id, pos, _) in &self.entities {
            f(*id, *pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_entities() {
        let mut bridge = RenderBridge::new(RenderBridgeConfig::default());

        let mock_world = MockWorldReader {
            entities: vec![
                (1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
                (2, [10.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
                (3, [1000.0, 0.0, 0.0], [0.0, 0.0, 1.0]), // Far away, should be culled
            ],
        };

        let frame = bridge.extract_frame_data(
            &mock_world,
            [0.0, 0.0, 0.0],
            [[1.0, 0.0, 0.0, 0.0]; 4],
            0.016,
            1,
        );

        assert_eq!(frame.entities.len(), 2); // Third entity culled
        assert_eq!(frame.stats.entities_read, 2);
    }
}
