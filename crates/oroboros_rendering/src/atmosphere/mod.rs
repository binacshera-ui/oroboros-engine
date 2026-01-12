//! Atmospheric effects for Neon Prime.
//!
//! This module implements the cinematic visual effects that make
//! the voxel city look like a sci-fi movie.

mod volumetric_fog;
/// Neon lighting module.
pub mod neon_lighting;
mod ssr;
mod half_res;

pub use volumetric_fog::{VolumetricFog, VolumetricFogUniforms};
pub use neon_lighting::{NeonLighting, NeonLight, NeonLightBuffer};
pub use ssr::ScreenSpaceReflections;
pub use half_res::{HalfResRenderer, HalfResConfig, HalfResDimensions, UpscaleUniforms};