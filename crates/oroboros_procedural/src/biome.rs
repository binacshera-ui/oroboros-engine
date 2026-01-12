//! # Biome Classification
//!
//! Determines terrain type from noise values.
//!
//! Uses a climate model based on:
//! - Temperature (from latitude and elevation)
//! - Humidity (from a separate noise channel)
//! - Elevation (from terrain noise)

use crate::noise::{SimplexNoise, WorldSeed};

/// Biome types in the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Biome {
    /// Deep ocean (elevation < -0.5)
    DeepOcean = 0,
    /// Shallow ocean (elevation < -0.2)
    Ocean = 1,
    /// Beach/coastline
    Beach = 2,
    /// Plains/grassland
    Plains = 3,
    /// Forest
    Forest = 4,
    /// Dense jungle
    Jungle = 5,
    /// Arid desert
    Desert = 6,
    /// Cold tundra
    Tundra = 7,
    /// Snowy taiga forest
    Taiga = 8,
    /// High mountains
    Mountains = 9,
    /// Snowy peaks
    SnowyPeaks = 10,
    /// Swamp/wetland
    Swamp = 11,
    /// Savanna grassland
    Savanna = 12,
    /// Volcanic/badlands
    Badlands = 13,
}

impl Biome {
    /// Returns the block ID for the surface of this biome.
    #[must_use]
    pub const fn surface_block(self) -> u32 {
        match self {
            Self::DeepOcean | Self::Ocean => 10, // Water
            Self::Beach => 11,                   // Sand
            Self::Plains | Self::Savanna => 1,   // Grass
            Self::Forest => 1,                   // Grass
            Self::Jungle => 12,                  // Jungle grass
            Self::Desert => 11,                  // Sand
            Self::Tundra => 13,                  // Frozen dirt
            Self::Taiga => 13,                   // Frozen dirt
            Self::Mountains => 2,                // Stone
            Self::SnowyPeaks => 14,              // Snow
            Self::Swamp => 15,                   // Mud
            Self::Badlands => 16,                // Red sand
        }
    }

    /// Returns whether this biome can have trees.
    #[must_use]
    pub const fn has_trees(self) -> bool {
        matches!(
            self,
            Self::Forest | Self::Jungle | Self::Taiga | Self::Swamp
        )
    }

    /// Returns the average tree density (0-100).
    #[must_use]
    pub const fn tree_density(self) -> u8 {
        match self {
            Self::Forest => 50,
            Self::Jungle => 80,
            Self::Taiga => 40,
            Self::Swamp => 30,
            Self::Plains => 5,
            Self::Savanna => 10,
            _ => 0,
        }
    }

    /// Converts from u8.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::DeepOcean,
            1 => Self::Ocean,
            2 => Self::Beach,
            3 => Self::Plains,
            4 => Self::Forest,
            5 => Self::Jungle,
            6 => Self::Desert,
            7 => Self::Tundra,
            8 => Self::Taiga,
            9 => Self::Mountains,
            10 => Self::SnowyPeaks,
            11 => Self::Swamp,
            12 => Self::Savanna,
            _ => Self::Badlands,
        }
    }
}

/// Biome classifier that determines biome from world coordinates.
///
/// Uses multiple noise channels to simulate climate.
pub struct BiomeClassifier {
    /// Temperature noise
    temperature_noise: SimplexNoise,
    /// Humidity noise
    humidity_noise: SimplexNoise,
    /// Elevation noise (also used for terrain height)
    elevation_noise: SimplexNoise,
    /// Detail noise for variation (reserved for future use)
    #[allow(dead_code)]
    detail_noise: SimplexNoise,
}

impl BiomeClassifier {
    /// Scale for temperature noise (larger = more gradual changes).
    const TEMPERATURE_SCALE: f64 = 0.002;
    /// Scale for humidity noise.
    const HUMIDITY_SCALE: f64 = 0.003;
    /// Scale for elevation noise (reduced 50% for larger landmasses).
    const ELEVATION_SCALE: f64 = 0.0025;

    /// Creates a new biome classifier from a world seed.
    #[must_use]
    pub fn new(seed: WorldSeed) -> Self {
        Self {
            temperature_noise: SimplexNoise::new(seed.derive(1)),
            humidity_noise: SimplexNoise::new(seed.derive(2)),
            elevation_noise: SimplexNoise::new(seed.derive(3)),
            detail_noise: SimplexNoise::new(seed.derive(4)),
        }
    }

    /// Classifies the biome at world coordinates.
    ///
    /// # Arguments
    ///
    /// * `x` - World X coordinate (block position)
    /// * `y` - World Y coordinate (block position)
    ///
    /// # Returns
    ///
    /// The biome at this location.
    #[must_use]
    pub fn classify(&self, x: f64, y: f64) -> Biome {
        // Get climate values
        let elevation = self.get_elevation(x, y);
        let temperature = self.get_temperature(x, y, elevation);
        let humidity = self.get_humidity(x, y);

        // Classify based on climate
        self.classify_from_climate(elevation, temperature, humidity)
    }

    /// Gets the elevation at world coordinates.
    ///
    /// # Returns
    ///
    /// Value in range [-1, 1] where:
    /// - < -0.5: Deep water
    /// - < -0.2: Water
    /// - < 0.0: Low land
    /// - < 0.5: Normal land
    /// - >= 0.5: Mountains
    #[must_use]
    pub fn get_elevation(&self, x: f64, y: f64) -> f64 {
        // Use octaved noise for natural terrain (reduced octaves for smoother terrain)
        let base = self.elevation_noise.octaved(
            x * Self::ELEVATION_SCALE,
            y * Self::ELEVATION_SCALE,
            4, // Reduced from 6 for smoother terrain
            0.5,
            2.0,
        );

        // Add some ridged noise for mountains (only in high areas)
        let ridged = self.elevation_noise.ridged(
            x * Self::ELEVATION_SCALE * 1.5,
            y * Self::ELEVATION_SCALE * 1.5,
            3, // Reduced from 4
            0.5,
            2.0,
        );

        // Blend: mostly base with mountain features only in high areas
        let raw = base * 0.8 + ridged * 0.2;
        
        // Apply terrain curve: flatten valleys, keep mountains steep
        Self::apply_terrain_curve(raw)
    }

    /// Applies a curve function to terrain elevation.
    ///
    /// - Flattens valleys (elevation < -0.1) for walkable ground
    /// - Creates gentle plains in mid-range (-0.1 to 0.3)
    /// - Keeps mountains steep (elevation > 0.3)
    #[inline]
    fn apply_terrain_curve(elevation: f64) -> f64 {
        if elevation < -0.3 {
            // Deep water - keep as is
            elevation
        } else if elevation < -0.1 {
            // Shallow water/beach transition - gentle slope
            -0.3 + (elevation + 0.3) * 0.5
        } else if elevation < 0.3 {
            // Plains/valleys - flatten significantly for walking/building
            // Map [-0.1, 0.3] to [-0.2, 0.1] (compressed range)
            let t = (elevation + 0.1) / 0.4; // Normalize to [0, 1]
            -0.2 + t * 0.3
        } else if elevation < 0.5 {
            // Hills - moderate slope
            // Map [0.3, 0.5] to [0.1, 0.4]
            let t = (elevation - 0.3) / 0.2;
            0.1 + t * 0.3
        } else {
            // Mountains - steep, amplified
            // Map [0.5, 1.0] to [0.4, 1.0]
            let t = (elevation - 0.5) / 0.5;
            0.4 + t * 0.6
        }
    }

    /// Gets the temperature at world coordinates.
    ///
    /// Temperature decreases with:
    /// - Distance from equator (y = 0)
    /// - Elevation (higher = colder)
    #[must_use]
    pub fn get_temperature(&self, x: f64, y: f64, elevation: f64) -> f64 {
        // Base temperature from noise
        let base = self.temperature_noise.sample(
            x * Self::TEMPERATURE_SCALE,
            y * Self::TEMPERATURE_SCALE,
        );

        // Latitude effect (farther from y=0 = colder)
        // Map world Y to latitude factor
        let latitude_factor = (y.abs() * 0.0001).min(1.0);

        // Elevation effect (higher = colder)
        let elevation_factor = elevation.max(0.0) * 0.5;

        // Combine factors
        let temperature = base - latitude_factor * 0.5 - elevation_factor;

        temperature.clamp(-1.0, 1.0)
    }

    /// Gets the humidity at world coordinates.
    #[must_use]
    pub fn get_humidity(&self, x: f64, y: f64) -> f64 {
        self.humidity_noise.octaved(
            x * Self::HUMIDITY_SCALE,
            y * Self::HUMIDITY_SCALE,
            4,
            0.5,
            2.0,
        )
    }

    /// Classifies biome from climate values.
    fn classify_from_climate(&self, elevation: f64, temperature: f64, humidity: f64) -> Biome {
        // Water biomes
        if elevation < -0.5 {
            return Biome::DeepOcean;
        }
        if elevation < -0.2 {
            return Biome::Ocean;
        }
        if elevation < -0.1 {
            return Biome::Beach;
        }

        // Mountain biomes
        if elevation > 0.7 {
            if temperature < -0.2 {
                return Biome::SnowyPeaks;
            }
            return Biome::Mountains;
        }

        // Land biomes based on temperature and humidity
        match (temperature, humidity) {
            // Cold biomes
            (t, _) if t < -0.5 => Biome::Tundra,
            (t, h) if t < -0.2 && h > 0.0 => Biome::Taiga,
            (t, _) if t < -0.2 => Biome::Tundra,

            // Hot biomes
            (t, h) if t > 0.5 && h < -0.3 => Biome::Desert,
            (t, h) if t > 0.5 && h > 0.5 => Biome::Jungle,
            (t, h) if t > 0.3 && h < 0.0 => Biome::Savanna,
            (t, _) if t > 0.6 => Biome::Badlands,

            // Temperate biomes
            (_, h) if h > 0.5 && elevation < 0.1 => Biome::Swamp,
            (_, h) if h > 0.2 => Biome::Forest,
            (_, h) if h < -0.2 => Biome::Plains,

            // Default
            _ => Biome::Plains,
        }
    }

    /// Gets the terrain height at world coordinates.
    ///
    /// This is the actual Y level of the terrain surface.
    ///
    /// # Arguments
    ///
    /// * `x` - World X coordinate
    /// * `z` - World Z coordinate (horizontal)
    /// * `sea_level` - The Y level of the sea
    /// * `max_height` - Maximum terrain height
    ///
    /// # Returns
    ///
    /// The Y level of the terrain at this X,Z position.
    #[must_use]
    pub fn get_terrain_height(&self, x: f64, z: f64, sea_level: i32, max_height: i32) -> i32 {
        let elevation = self.get_elevation(x, z);

        // Map elevation [-1, 1] to height
        let height_range = max_height - sea_level;
        let height_offset = ((elevation + 1.0) * 0.5 * f64::from(height_range)) as i32;

        sea_level + height_offset - (height_range / 2)
    }

    /// Fast terrain height for bulk generation (uses simple noise).
    ///
    /// Optimized for generating large maps quickly.
    /// Uses single noise sample for maximum speed during export.
    /// The result is still deterministic and visually coherent.
    #[inline]
    #[must_use]
    pub fn get_terrain_height_fast(&self, x: f64, z: f64, sea_level: i32, max_height: i32) -> i32 {
        // Ultra-fast version: single noise sample only
        let elevation = self.elevation_noise.sample(
            x * Self::ELEVATION_SCALE,
            z * Self::ELEVATION_SCALE,
        );

        let height_range = max_height - sea_level;
        let height_offset = ((elevation + 1.0) * 0.5 * f64::from(height_range)) as i32;

        sea_level + height_offset - (height_range / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biome_determinism() {
        let classifier1 = BiomeClassifier::new(WorldSeed::new(42));
        let classifier2 = BiomeClassifier::new(WorldSeed::new(42));

        for i in 0..100 {
            let x = i as f64 * 100.0;
            let y = i as f64 * 73.0;

            assert_eq!(
                classifier1.classify(x, y),
                classifier2.classify(x, y),
                "Biome classification should be deterministic"
            );
        }
    }

    #[test]
    fn test_elevation_range() {
        let classifier = BiomeClassifier::new(WorldSeed::new(42));

        for i in 0..1000 {
            let x = (i as f64 - 500.0) * 10.0;
            let y = (i as f64 * 0.7 - 350.0) * 10.0;

            let elevation = classifier.get_elevation(x, y);
            assert!(
                elevation >= -1.5 && elevation <= 1.5,
                "Elevation {elevation} out of range"
            );
        }
    }

    #[test]
    fn test_all_biomes_reachable() {
        let classifier = BiomeClassifier::new(WorldSeed::new(12345));
        let mut found_biomes = std::collections::HashSet::new();

        // Sample a large area to find various biomes
        for x in (-1000..1000).step_by(50) {
            for y in (-1000..1000).step_by(50) {
                let biome = classifier.classify(x as f64, y as f64);
                found_biomes.insert(biome);
            }
        }

        // Should find at least 5 different biomes in a 2000x2000 area
        assert!(
            found_biomes.len() >= 5,
            "Should find multiple biomes, found: {found_biomes:?}"
        );
    }

    #[test]
    fn test_ocean_at_low_elevation() {
        let classifier = BiomeClassifier::new(WorldSeed::new(42));

        // Manually check that low elevation gives water biomes
        let biome = classifier.classify_from_climate(-0.6, 0.0, 0.0);
        assert_eq!(biome, Biome::DeepOcean);

        let biome = classifier.classify_from_climate(-0.3, 0.0, 0.0);
        assert_eq!(biome, Biome::Ocean);
    }

    #[test]
    fn test_terrain_height() {
        let classifier = BiomeClassifier::new(WorldSeed::new(42));

        let sea_level = 64;
        let max_height = 256;

        for i in 0..100 {
            let x = i as f64 * 10.0;
            let z = i as f64 * 7.0;

            let height = classifier.get_terrain_height(x, z, sea_level, max_height);

            assert!(
                height >= 0 && height <= max_height,
                "Height {height} out of range"
            );
        }
    }
}
