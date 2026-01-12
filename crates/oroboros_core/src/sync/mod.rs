//! # Synchronization Primitives for Multi-threaded ECS
//!
//! ARCHITECT'S ORDER: No locks. No race conditions. No compromises.
//!
//! ## The Problem
//!
//! ```text
//! Thread 1 (Logic/Inferno):  WRITE to entities
//! Thread 2 (Render/Neon):    READ from entities
//!
//! Without synchronization: RACE CONDITION → CRASH
//! With Mutex:              LOCK CONTENTION → 0 FPS
//! ```
//!
//! ## The Solution: Double Buffering
//!
//! ```text
//! Frame N:
//!   Logic writes to Buffer A
//!   Render reads from Buffer B (last frame's state)
//!
//! Frame N+1:
//!   SWAP (atomic pointer exchange)
//!   Logic writes to Buffer B
//!   Render reads from Buffer A
//! ```
//!
//! Zero locks. Zero contention. Full parallelism.

mod double_buffer;

pub use double_buffer::{
    DoubleBufferedWorld,
    WorldWriteHandle,
    WorldReadHandle,
    FrameSync,
};
