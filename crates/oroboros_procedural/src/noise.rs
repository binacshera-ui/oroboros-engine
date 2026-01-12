//! # Simplex Noise Implementation
//!
//! High-performance, deterministic noise generation.
//!
//! ## Why Simplex over Perlin?
//!
//! - Fewer directional artifacts
//! - Better gradient distribution
//! - O(n) complexity vs O(2^n) for Perlin
//! - More visually pleasing results
//!
//! ## Determinism Guarantee
//!
//! Given the same `WorldSeed`, this implementation will produce
//! **exactly** the same values on any platform, any time.

/// World seed for deterministic generation.
///
/// All procedural generation derives from this seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorldSeed(u64);

impl WorldSeed {
    /// Creates a new world seed.
    #[inline]
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Returns the raw seed value.
    #[inline]
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Derives a sub-seed for a specific purpose (e.g., biome generation).
    ///
    /// Uses a hash function to create independent streams from one seed.
    #[inline]
    #[must_use]
    pub const fn derive(self, purpose: u64) -> Self {
        // FNV-1a hash mixing
        let mut hash = self.0;
        hash ^= purpose;
        hash = hash.wrapping_mul(0x517cc1b727220a95);
        hash ^= hash >> 32;
        Self(hash)
    }
}

impl Default for WorldSeed {
    fn default() -> Self {
        Self(0xDEAD_BEEF_CAFE_BABE)
    }
}

/// Pre-computed permutation table for noise.
///
/// This is computed once from the seed and reused.
struct PermutationTable {
    /// 512-entry permutation table (256 entries, doubled for overflow handling).
    perm: [u8; 512],
    /// Gradient table (12 gradients for 2D simplex).
    grad: [[i8; 2]; 12],
}

impl PermutationTable {
    /// Creates a new permutation table from a seed.
    fn new(seed: WorldSeed) -> Self {
        let mut perm = [0u8; 512];

        // Initialize with identity permutation
        for i in 0..256 {
            perm[i] = i as u8;
        }

        // Fisher-Yates shuffle with deterministic RNG
        let mut rng_state = seed.value();
        for i in (1..256).rev() {
            // Simple xorshift64 for deterministic shuffling
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;

            let j = (rng_state as usize) % (i + 1);
            perm.swap(i, j);
        }

        // Double the table to avoid index wrapping
        for i in 0..256 {
            perm[256 + i] = perm[i];
        }

        // 12 gradient vectors for 2D simplex
        // These point to vertices of a regular 12-gon
        let grad = [
            [1, 0], [1, 1], [0, 1], [-1, 1],
            [-1, 0], [-1, -1], [0, -1], [1, -1],
            [1, 0], [0, 1], [-1, 0], [0, -1],
        ];

        Self { perm, grad }
    }

    /// Gets a permutation value (with automatic wrapping).
    #[inline]
    fn get(&self, index: usize) -> u8 {
        self.perm[index & 511]
    }

    /// Gets a gradient for a given hash.
    #[inline]
    fn gradient(&self, hash: u8) -> [i8; 2] {
        self.grad[(hash % 12) as usize]
    }
}

/// 2D Simplex noise generator.
///
/// Produces smooth, continuous noise values in the range [-1, 1].
///
/// # Performance
///
/// - O(1) per sample
/// - No allocations
/// - Cache-friendly access patterns
///
/// # Example
///
/// ```rust,ignore
/// let noise = SimplexNoise::new(WorldSeed::new(42));
///
/// // Sample noise at coordinates
/// let value = noise.sample(100.5, 200.3);
/// assert!(value >= -1.0 && value <= 1.0);
///
/// // Generate octaved noise for terrain
/// let terrain = noise.octaved(x, y, 6, 0.5, 2.0);
/// ```
pub struct SimplexNoise {
    /// The permutation table.
    perm_table: PermutationTable,
}

impl SimplexNoise {
    /// Skewing factor for 2D simplex grid.
    const F2: f64 = 0.366025403784439; // (sqrt(3) - 1) / 2
    /// Unskewing factor for 2D simplex grid.
    const G2: f64 = 0.211324865405187; // (3 - sqrt(3)) / 6

    /// Creates a new simplex noise generator from a seed.
    #[must_use]
    pub fn new(seed: WorldSeed) -> Self {
        Self {
            perm_table: PermutationTable::new(seed),
        }
    }

    /// Samples 2D simplex noise at the given coordinates.
    ///
    /// # Returns
    ///
    /// A value in the range [-1, 1].
    #[must_use]
    pub fn sample(&self, x: f64, y: f64) -> f64 {
        // Skew input coordinates to simplex grid
        let skew = (x + y) * Self::F2;
        let i = fast_floor(x + skew);
        let j = fast_floor(y + skew);

        // Unskew to get first corner in simplex
        let unskew = (i + j) as f64 * Self::G2;
        let x0 = x - (i as f64 - unskew);
        let y0 = y - (j as f64 - unskew);

        // Determine which simplex we're in (upper or lower triangle)
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

        // Offsets for second and third corners
        let x1 = x0 - i1 as f64 + Self::G2;
        let y1 = y0 - j1 as f64 + Self::G2;
        let x2 = x0 - 1.0 + 2.0 * Self::G2;
        let y2 = y0 - 1.0 + 2.0 * Self::G2;

        // Hash coordinates to get gradient indices
        let ii = (i & 255) as usize;
        let jj = (j & 255) as usize;

        let gi0 = self.perm_table.get(ii + self.perm_table.get(jj) as usize);
        let gi1 = self.perm_table.get(ii + i1 + self.perm_table.get(jj + j1) as usize);
        let gi2 = self.perm_table.get(ii + 1 + self.perm_table.get(jj + 1) as usize);

        // Calculate contribution from three corners
        let n0 = self.contribution(x0, y0, gi0);
        let n1 = self.contribution(x1, y1, gi1);
        let n2 = self.contribution(x2, y2, gi2);

        // Scale to [-1, 1] range
        // The magic number 70.0 normalizes the output
        70.0 * (n0 + n1 + n2)
    }

    /// Calculates the contribution from one corner of the simplex.
    #[inline]
    fn contribution(&self, x: f64, y: f64, gradient_index: u8) -> f64 {
        let t = 0.5 - x * x - y * y;
        if t < 0.0 {
            0.0
        } else {
            let grad = self.perm_table.gradient(gradient_index);
            let t2 = t * t;
            t2 * t2 * (x * f64::from(grad[0]) + y * f64::from(grad[1]))
        }
    }

    /// Generates octaved (fractal) noise.
    ///
    /// Combines multiple layers of noise at different frequencies
    /// to create more natural-looking terrain.
    ///
    /// # Arguments
    ///
    /// * `x`, `y` - Coordinates
    /// * `octaves` - Number of noise layers (typically 4-8)
    /// * `persistence` - Amplitude decay per octave (typically 0.5)
    /// * `lacunarity` - Frequency increase per octave (typically 2.0)
    ///
    /// # Returns
    ///
    /// A value roughly in the range [-1, 1].
    #[must_use]
    pub fn octaved(
        &self,
        x: f64,
        y: f64,
        octaves: u32,
        persistence: f64,
        lacunarity: f64,
    ) -> f64 {
        let mut total = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = 1.0;
        let mut max_amplitude = 0.0;

        for _ in 0..octaves {
            total += self.sample(x * frequency, y * frequency) * amplitude;
            max_amplitude += amplitude;
            amplitude *= persistence;
            frequency *= lacunarity;
        }

        // Normalize to [-1, 1]
        total / max_amplitude
    }

    /// Generates ridged noise (good for mountains).
    ///
    /// Creates sharp ridges by taking the absolute value and inverting.
    #[must_use]
    pub fn ridged(&self, x: f64, y: f64, octaves: u32, persistence: f64, lacunarity: f64) -> f64 {
        let mut total = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = 1.0;
        let mut max_amplitude = 0.0;

        for _ in 0..octaves {
            // Ridge formula: 1 - |noise|
            let noise = self.sample(x * frequency, y * frequency);
            let ridge = 1.0 - noise.abs();
            total += ridge * ridge * amplitude;
            max_amplitude += amplitude;
            amplitude *= persistence;
            frequency *= lacunarity;
        }

        total / max_amplitude
    }

    /// Samples noise and maps to integer range [0, max).
    ///
    /// Useful for selecting discrete values like block types.
    #[must_use]
    pub fn sample_discrete(&self, x: f64, y: f64, max: u32) -> u32 {
        let noise = (self.sample(x, y) + 1.0) * 0.5; // Map to [0, 1]
        let scaled = noise * f64::from(max);
        (scaled as u32).min(max - 1)
    }
}

/// Fast floor function.
///
/// Faster than `f64::floor()` for our use case.
#[inline]
fn fast_floor(x: f64) -> i32 {
    let xi = x as i32;
    if x < xi as f64 { xi - 1 } else { xi }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determinism() {
        let seed = WorldSeed::new(12345);
        let noise1 = SimplexNoise::new(seed);
        let noise2 = SimplexNoise::new(seed);

        // Same seed should produce identical results
        for i in 0..100 {
            let x = i as f64 * 0.1;
            let y = i as f64 * 0.17;
            assert_eq!(
                noise1.sample(x, y),
                noise2.sample(x, y),
                "Noise should be deterministic"
            );
        }
    }

    #[test]
    fn test_different_seeds_different_results() {
        let noise1 = SimplexNoise::new(WorldSeed::new(1));
        let noise2 = SimplexNoise::new(WorldSeed::new(2));

        let v1 = noise1.sample(100.0, 100.0);
        let v2 = noise2.sample(100.0, 100.0);

        assert_ne!(v1, v2, "Different seeds should produce different results");
    }

    #[test]
    fn test_range() {
        let noise = SimplexNoise::new(WorldSeed::new(42));

        // Sample many points and verify range
        for i in 0..10000 {
            let x = (i as f64 * 0.1) - 500.0;
            let y = (i as f64 * 0.13) - 650.0;
            let value = noise.sample(x, y);

            assert!(
                value >= -1.0 && value <= 1.0,
                "Value {value} out of range at ({x}, {y})"
            );
        }
    }

    #[test]
    fn test_continuity() {
        let noise = SimplexNoise::new(WorldSeed::new(42));

        // Sample adjacent points - should be similar
        let x = 100.0;
        let y = 100.0;
        let delta = 0.001;

        let v1 = noise.sample(x, y);
        let v2 = noise.sample(x + delta, y);
        let v3 = noise.sample(x, y + delta);

        let diff1 = (v1 - v2).abs();
        let diff2 = (v1 - v3).abs();

        // Adjacent samples should be very similar
        assert!(diff1 < 0.01, "Noise should be continuous: diff = {diff1}");
        assert!(diff2 < 0.01, "Noise should be continuous: diff = {diff2}");
    }

    #[test]
    fn test_octaved_noise() {
        let noise = SimplexNoise::new(WorldSeed::new(42));

        let value = noise.octaved(100.0, 100.0, 6, 0.5, 2.0);

        // Octaved noise should still be in reasonable range
        assert!(
            value >= -1.5 && value <= 1.5,
            "Octaved value {value} out of expected range"
        );
    }

    #[test]
    fn test_seed_derivation() {
        let base = WorldSeed::new(42);
        let derived1 = base.derive(1);
        let derived2 = base.derive(2);
        let derived1_again = base.derive(1);

        assert_ne!(derived1, derived2, "Different purposes should give different seeds");
        assert_eq!(derived1, derived1_again, "Same purpose should give same seed");
        assert_ne!(derived1, base, "Derived seed should differ from base");
    }

    #[test]
    fn test_discrete_sampling() {
        let noise = SimplexNoise::new(WorldSeed::new(42));

        // Sample discrete values
        for i in 0..1000 {
            let x = i as f64 * 0.5;
            let y = i as f64 * 0.7;
            let value = noise.sample_discrete(x, y, 10);
            assert!(value < 10, "Discrete value should be in range [0, 10)");
        }
    }

    #[test]
    fn test_performance_million_samples() {
        let noise = SimplexNoise::new(WorldSeed::new(42));

        let start = std::time::Instant::now();
        for i in 0..1_000_000 {
            let x = (i % 10000) as f64 * 0.01;
            let y = (i / 10000) as f64 * 0.01;
            let _ = noise.sample(x, y);
        }
        let elapsed = start.elapsed();

        println!("1M noise samples in {:?}", elapsed);
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "1M samples should complete in <1s, took {:?}",
            elapsed
        );
    }
}
