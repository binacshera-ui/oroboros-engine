//! Core Adapter - Connects to oroboros_core's DoubleBufferedWorld
//!
//! This provides the REAL WorldReader implementation for Unit 1's data.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use oroboros_core::DoubleBufferedWorld;
//! use oroboros_rendering::integration::CoreWorldReader;
//!
//! let db_world = DoubleBufferedWorld::new(1_000_000, 100_000);
//! let read_handle = db_world.read_handle();
//!
//! // Create the adapter
//! let reader = CoreWorldReader::from_read_handle(&read_handle);
//!
//! // Use with render loop
//! let frame = render_loop.frame(&reader, camera, view_proj, dt);
//! ```

use super::render_bridge::WorldReader;
use oroboros_core::sync::WorldReadHandle;

/// Adapter that implements WorldReader for oroboros_core's WorldReadHandle
///
/// This is the bridge between Unit 2's rendering and Unit 1's ECS.
pub struct CoreWorldReader<'a> {
    /// Reference to the read handle from DoubleBufferedWorld
    handle: &'a WorldReadHandle,
}

impl<'a> CoreWorldReader<'a> {
    /// Creates a new CoreWorldReader from a WorldReadHandle
    #[must_use]
    pub fn new(handle: &'a WorldReadHandle) -> Self {
        Self { handle }
    }
}

impl<'a> WorldReader for CoreWorldReader<'a> {
    fn buffer_index(&self) -> u8 {
        self.handle.buffer_index() as u8
    }

    fn for_each_position_velocity<F>(&self, mut f: F)
    where
        F: FnMut(u64, [f32; 3], [f32; 3]),
    {
        // Access the archetype world through the read handle
        let world = &**self.handle;

        // Iterate over the PV (Position+Velocity) table
        let pv_table = &world.pv_table;

        for idx in 0..pv_table.len() {
            if let (Some(pos), Some(vel)) = (
                pv_table.get_position(idx),
                pv_table.get_velocity(idx),
            ) {
                f(
                    idx as u64,
                    [pos.x, pos.y, pos.z],
                    [vel.x, vel.y, vel.z],
                );
            }
        }
    }

    fn for_each_position<F>(&self, mut f: F)
    where
        F: FnMut(u64, [f32; 3]),
    {
        // Access the archetype world
        let world = &**self.handle;

        // Iterate over P-only (Position only) table for static entities
        let p_table = &world.p_table;

        for idx in 0..p_table.len() {
            if let Some(pos) = p_table.get_position(idx) {
                f(idx as u64, [pos.x, pos.y, pos.z]);
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use oroboros_core::sync::DoubleBufferedWorld;
    use oroboros_core::{Position, Velocity};

    #[test]
    fn test_core_adapter() {
        // Create a double-buffered world
        let db_world = DoubleBufferedWorld::new(1000, 1000);

        // Write some entities
        {
            let mut write = db_world.write_handle();
            let _ = write.spawn_pv(
                Position::new(1.0, 2.0, 3.0),
                Velocity::new(0.1, 0.2, 0.3),
            );
            let _ = write.spawn_pv(
                Position::new(4.0, 5.0, 6.0),
                Velocity::new(0.4, 0.5, 0.6),
            );
        }

        // Swap buffers to make data readable
        db_world.swap_buffers();

        // Read through the adapter
        let read_handle = db_world.read_handle();
        let reader = CoreWorldReader::new(&read_handle);

        let mut entities = Vec::new();
        reader.for_each_position_velocity(|id, pos, vel| {
            entities.push((id, pos, vel));
        });

        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].1, [1.0, 2.0, 3.0]);
        assert_eq!(entities[1].1, [4.0, 5.0, 6.0]);
    }
}
