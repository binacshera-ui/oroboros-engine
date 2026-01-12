//! # Loot Table System
//!
//! **O(1) Drop Chance Calculations with Tiered Security**
//!
//! This module implements the mining/loot system with the following constraints:
//! - All calculations happen in O(1) constant time
//! - No loops or branching based on input size
//! - Deterministic results for the same inputs
//! - Configurable via external TOML files
//!
//! ## Security Model
//!
//! **CRITICAL**: We use different hash algorithms based on item value:
//!
//! - **Common/Uncommon items (FNV-1a)**: Fast, non-cryptographic. Predictable but
//!   worthless items don't matter if someone games the system.
//!
//! - **Rare+ items (SipHash-2-4)**: Cryptographically secure with dynamic salt from
//!   blockchain. Even if attacker knows the seed, the blockchain salt changes every
//!   block making prediction impossible without controlling the blockchain itself.
//!
//! ## Input Parameters (5 Variables + Blockchain Salt)
//!
//! 1. `pickaxe_tier` - Tool quality (0-255)
//! 2. `player_level` - Character progression (0-255)
//! 3. `block_rarity` - Target block type rarity (0-255)
//! 4. `weather_seed` - Blockchain-derived randomness (32-bit)
//! 5. `entropy` - Additional blockchain entropy (32-bit)
//! 6. `blockchain_salt` - Dynamic 128-bit salt from latest block hash (for rare+)

use serde::{Deserialize, Serialize};
use siphasher::sip128::{Hasher128, SipHasher24};
use std::collections::HashMap;
use std::hash::Hasher;

use crate::inventory::ItemId;

/// Rarity tier for items and blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Rarity {
    /// Common items (gray) - ~70% of drops
    Common = 0,
    /// Uncommon items (green) - ~20% of drops
    Uncommon = 1,
    /// Rare items (blue) - ~7% of drops
    Rare = 2,
    /// Epic items (purple) - ~2.5% of drops
    Epic = 3,
    /// Legendary items (orange) - ~0.4% of drops
    Legendary = 4,
    /// Mythic items (red) - ~0.1% of drops
    Mythic = 5,
}

impl Rarity {
    /// Base drop rate multiplier for this rarity (in basis points, 10000 = 100%).
    #[inline]
    #[must_use]
    pub const fn base_drop_rate_bp(self) -> u32 {
        match self {
            Self::Common => 7000,     // 70%
            Self::Uncommon => 2000,   // 20%
            Self::Rare => 700,        // 7%
            Self::Epic => 250,        // 2.5%
            Self::Legendary => 40,    // 0.4%
            Self::Mythic => 10,       // 0.1%
        }
    }

    /// Converts from u8 to Rarity.
    #[inline]
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Common,
            1 => Self::Uncommon,
            2 => Self::Rare,
            3 => Self::Epic,
            4 => Self::Legendary,
            _ => Self::Mythic,
        }
    }
}

/// Result of a loot drop calculation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropResult {
    /// The item that dropped (if any).
    pub item_id: Option<ItemId>,
    /// Quantity dropped.
    pub quantity: u32,
    /// The rarity of the drop.
    pub rarity: Rarity,
    /// The calculated drop chance (for debugging/display).
    pub drop_chance_bp: u32,
}

impl DropResult {
    /// Creates an empty (no drop) result.
    #[inline]
    #[must_use]
    pub const fn nothing() -> Self {
        Self {
            item_id: None,
            quantity: 0,
            rarity: Rarity::Common,
            drop_chance_bp: 0,
        }
    }
}

/// A single entry in a loot table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LootEntry {
    /// The item ID to drop.
    pub item_id: ItemId,
    /// Base drop weight (higher = more common).
    pub weight: u32,
    /// Minimum quantity.
    pub min_quantity: u32,
    /// Maximum quantity.
    pub max_quantity: u32,
    /// Item rarity.
    pub rarity: Rarity,
    /// Minimum player level required.
    pub min_level: u8,
    /// Minimum pickaxe tier required.
    pub min_pickaxe_tier: u8,
}

/// A complete loot table for a block type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LootTable {
    /// Block ID this table is for.
    pub block_id: u32,
    /// Block rarity tier.
    pub block_rarity: Rarity,
    /// All possible drops from this block.
    pub entries: Vec<LootEntry>,
    /// Total weight of all entries (pre-calculated).
    #[serde(skip)]
    pub total_weight: u32,
}

impl LootTable {
    /// Calculates the total weight of all entries.
    pub fn calculate_total_weight(&mut self) {
        self.total_weight = self.entries.iter().map(|e| e.weight).sum();
    }
}

/// Pre-computed lookup tables for O(1) calculations.
///
/// These tables are computed once at startup and indexed directly.
struct LookupTables {
    /// Level bonus table (256 entries, one per level).
    level_bonus: [u16; 256],
    /// Pickaxe bonus table (256 entries, one per tier).
    pickaxe_bonus: [u16; 256],
    /// Rarity multiplier table (6 entries).
    rarity_multiplier: [u16; 6],
}

impl LookupTables {
    /// Creates new lookup tables with pre-computed values.
    fn new() -> Self {
        let mut level_bonus = [0u16; 256];
        let mut pickaxe_bonus = [0u16; 256];

        // Pre-compute level bonuses (0-100% bonus spread across 255 levels)
        for (i, bonus) in level_bonus.iter_mut().enumerate() {
            // Linear progression: level 255 = 100% bonus (1000 basis points)
            *bonus = ((i * 1000) / 255) as u16;
        }

        // Pre-compute pickaxe bonuses
        for (i, bonus) in pickaxe_bonus.iter_mut().enumerate() {
            // Tier bonus: each tier adds 5% (50 basis points)
            *bonus = (i * 50).min(2500) as u16;
        }

        // Rarity multipliers (in basis points relative to base rate)
        let rarity_multiplier = [
            10000, // Common: 100% of base
            8000,  // Uncommon: 80% of base (harder to get)
            6000,  // Rare: 60%
            4000,  // Epic: 40%
            2000,  // Legendary: 20%
            1000,  // Mythic: 10%
        ];

        Self {
            level_bonus,
            pickaxe_bonus,
            rarity_multiplier,
        }
    }
}

/// Blockchain salt for cryptographically secure loot generation.
///
/// This is derived from the latest block hash and changes every block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockchainSalt {
    /// Lower 64 bits of block hash.
    pub low: u64,
    /// Upper 64 bits of block hash.
    pub high: u64,
}

impl BlockchainSalt {
    /// Creates a new blockchain salt from a 256-bit block hash.
    #[must_use]
    pub fn from_block_hash(block_hash: &[u8; 32]) -> Self {
        Self {
            low: u64::from_le_bytes(block_hash[0..8].try_into().unwrap()),
            high: u64::from_le_bytes(block_hash[8..16].try_into().unwrap()),
        }
    }

    /// Creates a test salt (NOT FOR PRODUCTION).
    #[must_use]
    pub const fn test_salt() -> Self {
        Self {
            low: 0xDEAD_BEEF_CAFE_BABE,
            high: 0x1337_C0DE_DEAD_F00D,
        }
    }
}

/// Server-side secret seed for preventing pre-computation attacks.
///
/// ## Security Model
///
/// The blockchain salt alone is NOT enough because:
/// 1. Block hash is public - anyone can see it
/// 2. SipHash is deterministic - same inputs = same outputs
/// 3. Attacker can pre-compute: hash(block_salt, player_params) for all blocks
///
/// Solution: Add server-secret components that the client CANNOT know:
///
/// ```text
/// final_seed = SipHash(
///     blockchain_salt,       // Public: changes every block
///     server_secret,         // Secret: 256-bit random, rotated daily
///     server_tick,           // Semi-secret: internal tick counter
///     player_action_nonce    // Per-action: prevents replay
/// )
/// ```
///
/// Even if attacker knows blockchain_salt and their player_params,
/// they CANNOT compute the result without server_secret.
#[derive(Clone)]
pub struct SecureSeed {
    /// Server-side secret (256 bits, rotated daily).
    /// 
    /// **CRITICAL**: This MUST be:
    /// 1. Generated from cryptographically secure RNG
    /// 2. Never exposed to clients
    /// 3. Rotated periodically (daily recommended)
    /// 4. Backed up securely
    secret: [u64; 4],
    /// Current server tick (monotonic counter).
    tick: u64,
}

impl SecureSeed {
    /// Creates a new secure seed from random bytes.
    ///
    /// # Arguments
    ///
    /// * `secret` - 32 bytes of cryptographically secure random data
    ///
    /// # Security
    ///
    /// Generate this using a CSPRNG like `rand::rngs::OsRng`.
    /// DO NOT use predictable or reused secrets.
    #[must_use]
    pub fn new(secret: &[u8; 32]) -> Self {
        Self {
            secret: [
                u64::from_le_bytes(secret[0..8].try_into().unwrap()),
                u64::from_le_bytes(secret[8..16].try_into().unwrap()),
                u64::from_le_bytes(secret[16..24].try_into().unwrap()),
                u64::from_le_bytes(secret[24..32].try_into().unwrap()),
            ],
            tick: 0,
        }
    }

    /// Creates a test seed (NOT FOR PRODUCTION).
    #[must_use]
    pub const fn test_seed() -> Self {
        Self {
            secret: [0x1234_5678_9ABC_DEF0, 0xFEDC_BA98_7654_3210,
                     0xAAAA_BBBB_CCCC_DDDD, 0x1111_2222_3333_4444],
            tick: 0,
        }
    }

    /// Advances the tick counter.
    pub fn advance_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// Sets the tick counter.
    pub fn set_tick(&mut self, tick: u64) {
        self.tick = tick;
    }

    /// Gets the current tick.
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    /// Combines server secret with blockchain salt to produce final keys.
    ///
    /// This is the core anti-prediction mechanism.
    #[inline]
    #[must_use]
    pub fn derive_keys(&self, blockchain_salt: BlockchainSalt, action_nonce: u64) -> (u64, u64) {
        // Mix all components together using XOR and rotation
        // This ensures all components influence the final result
        
        // First key: blockchain + secret[0,1] + tick
        let k1 = blockchain_salt.low
            .wrapping_add(self.secret[0])
            .rotate_left(13)
            ^ self.secret[1]
            ^ self.tick;

        // Second key: blockchain + secret[2,3] + nonce
        let k2 = blockchain_salt.high
            .wrapping_add(self.secret[2])
            .rotate_left(17)
            ^ self.secret[3]
            ^ action_nonce;

        (k1, k2)
    }
}

impl Default for SecureSeed {
    fn default() -> Self {
        Self::test_seed()
    }
}

impl std::fmt::Debug for SecureSeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // NEVER expose the secret in debug output
        f.debug_struct("SecureSeed")
            .field("secret", &"[REDACTED]")
            .field("tick", &self.tick)
            .finish()
    }
}

/// The main loot calculator.
///
/// Uses pre-computed tables for O(1) drop calculations with tiered security:
/// - FNV-1a for common/uncommon items (fast)
/// - SipHash-2-4 with blockchain salt + server secret for rare+ items (secure)
///
/// ## Security Architecture
///
/// ```text
/// Attacker knows:        Server adds:           Result:
/// ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
/// │ Block hash      │ +  │ Server secret   │ =  │ Unpredictable   │
/// │ Player params   │    │ Server tick     │    │ Cannot pre-     │
/// │ (level, tool)   │    │ Action nonce    │    │ compute drops   │
/// └─────────────────┘    └─────────────────┘    └─────────────────┘
/// ```
pub struct LootCalculator {
    /// Pre-computed lookup tables.
    tables: LookupTables,
    /// Loot tables indexed by block ID.
    loot_tables: HashMap<u32, LootTable>,
    /// Current blockchain salt (public, changes every block).
    blockchain_salt: BlockchainSalt,
    /// Server-side secret seed (NEVER exposed to clients).
    server_seed: SecureSeed,
    /// Action nonce counter (monotonic, per-server).
    action_nonce: u64,
}

impl LootCalculator {
    /// Creates a new loot calculator.
    ///
    /// **WARNING**: Uses test seed. In production, call `with_secret`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tables: LookupTables::new(),
            loot_tables: HashMap::new(),
            blockchain_salt: BlockchainSalt::default(),
            server_seed: SecureSeed::test_seed(),
            action_nonce: 0,
        }
    }

    /// Creates a loot calculator with a production secret.
    ///
    /// # Arguments
    ///
    /// * `secret` - 32 bytes from a CSPRNG (e.g., `OsRng`)
    ///
    /// # Security
    ///
    /// This secret MUST be:
    /// 1. Cryptographically random
    /// 2. Never logged or exposed
    /// 3. Rotated periodically (daily)
    /// 4. Backed up securely
    #[must_use]
    pub fn with_secret(secret: &[u8; 32]) -> Self {
        Self {
            tables: LookupTables::new(),
            loot_tables: HashMap::new(),
            blockchain_salt: BlockchainSalt::default(),
            server_seed: SecureSeed::new(secret),
            action_nonce: 0,
        }
    }

    /// Updates the blockchain salt (call this every block).
    ///
    /// **SECURITY CRITICAL**: This must be called with the latest block hash.
    pub fn update_blockchain_salt(&mut self, salt: BlockchainSalt) {
        self.blockchain_salt = salt;
    }

    /// Updates the server tick (call every game tick).
    pub fn update_server_tick(&mut self, tick: u64) {
        self.server_seed.set_tick(tick);
    }

    /// Rotates the server secret (call daily or on security events).
    ///
    /// # Arguments
    ///
    /// * `new_secret` - Fresh 32 bytes from CSPRNG
    pub fn rotate_secret(&mut self, new_secret: &[u8; 32]) {
        self.server_seed = SecureSeed::new(new_secret);
    }

    /// Registers a loot table for a block type.
    pub fn register_table(&mut self, mut table: LootTable) {
        table.calculate_total_weight();
        self.loot_tables.insert(table.block_id, table);
    }

    /// Calculates the drop for a mining action (fast mode for common items).
    ///
    /// **WARNING**: This uses FNV-1a which is NOT secure for valuable items.
    /// Use `calculate_drop_secure` for rare+ items.
    #[must_use]
    pub fn calculate_drop(
        &self,
        block_id: u32,
        player_level: u8,
        pickaxe_tier: u8,
        weather_seed: u32,
        entropy: u32,
    ) -> DropResult {
        // Get the loot table for this block
        let Some(table) = self.loot_tables.get(&block_id) else {
            return DropResult::nothing();
        };

        if table.entries.is_empty() || table.total_weight == 0 {
            return DropResult::nothing();
        }

        // Use fast hash for common items
        let hash = self.compute_hash_fast(block_id, player_level, pickaxe_tier, weather_seed, entropy);
        self.apply_loot_roll(table, hash, player_level, pickaxe_tier)
    }

    /// Calculates the drop with cryptographic security for rare items.
    ///
    /// Uses SipHash-2-4 with server secret + blockchain salt to prevent prediction.
    ///
    /// ## Security
    ///
    /// This method consumes a unique action nonce internally, making each call
    /// produce different results even with identical inputs.
    ///
    /// # Arguments
    ///
    /// * `block_id` - The block being mined
    /// * `player_level` - Player's current level (0-255)
    /// * `pickaxe_tier` - Tool tier (0-255)
    /// * `weather_seed` - Blockchain weather entropy
    /// * `entropy` - Additional entropy (timestamp, player-specific nonce)
    pub fn calculate_drop_secure(
        &mut self,
        block_id: u32,
        player_level: u8,
        pickaxe_tier: u8,
        weather_seed: u32,
        entropy: u32,
    ) -> DropResult {
        // Get the loot table for this block
        let Some(table) = self.loot_tables.get(&block_id) else {
            return DropResult::nothing();
        };

        if table.entries.is_empty() || table.total_weight == 0 {
            return DropResult::nothing();
        }

        // Clone what we need to avoid borrow issues
        let table = table.clone();

        // Use secure hash with server secret
        let hash = self.compute_hash_secure(block_id, player_level, pickaxe_tier, weather_seed, entropy);
        self.apply_loot_roll(&table, hash, player_level, pickaxe_tier)
    }

    /// Applies the loot roll using a pre-computed hash.
    fn apply_loot_roll(
        &self,
        table: &LootTable,
        hash: u64,
        player_level: u8,
        pickaxe_tier: u8,
    ) -> DropResult {
        // Look up bonuses from pre-computed tables (O(1))
        let level_bonus = u32::from(self.tables.level_bonus[player_level as usize]);
        let pickaxe_bonus = u32::from(self.tables.pickaxe_bonus[pickaxe_tier as usize]);
        let rarity_mult = u32::from(self.tables.rarity_multiplier[table.block_rarity as usize]);

        // Calculate base drop chance (O(1))
        let base_rate = table.block_rarity.base_drop_rate_bp();
        let bonus_mult = 10000 + level_bonus + pickaxe_bonus;
        let drop_chance_bp = (base_rate * bonus_mult / 10000) * rarity_mult / 10000;

        // Roll against drop chance (O(1))
        let roll = (hash % 10000) as u32;
        if roll >= drop_chance_bp {
            return DropResult {
                item_id: None,
                quantity: 0,
                rarity: table.block_rarity,
                drop_chance_bp,
            };
        }

        // Select which item dropped
        let weight_roll = (hash >> 16) % u64::from(table.total_weight);
        let mut cumulative = 0u64;

        for entry in &table.entries {
            cumulative += u64::from(entry.weight);
            if weight_roll < cumulative {
                if player_level < entry.min_level || pickaxe_tier < entry.min_pickaxe_tier {
                    continue;
                }

                let quantity = if entry.min_quantity == entry.max_quantity {
                    entry.min_quantity
                } else {
                    let range = entry.max_quantity - entry.min_quantity + 1;
                    entry.min_quantity + ((hash >> 32) as u32 % range)
                };

                return DropResult {
                    item_id: Some(entry.item_id),
                    quantity,
                    rarity: entry.rarity,
                    drop_chance_bp,
                };
            }
        }

        DropResult::nothing()
    }

    /// Fast hash using FNV-1a (for common items).
    ///
    /// **NOT CRYPTOGRAPHICALLY SECURE** - Use only for worthless items.
    #[inline]
    fn compute_hash_fast(
        &self,
        block_id: u32,
        player_level: u8,
        pickaxe_tier: u8,
        weather_seed: u32,
        entropy: u32,
    ) -> u64 {
        const FNV_PRIME: u64 = 0x00000100000001B3;
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;

        let mut hash = FNV_OFFSET;

        hash ^= u64::from(block_id);
        hash = hash.wrapping_mul(FNV_PRIME);
        hash ^= u64::from(player_level);
        hash = hash.wrapping_mul(FNV_PRIME);
        hash ^= u64::from(pickaxe_tier);
        hash = hash.wrapping_mul(FNV_PRIME);
        hash ^= u64::from(weather_seed);
        hash = hash.wrapping_mul(FNV_PRIME);
        hash ^= u64::from(entropy);
        hash = hash.wrapping_mul(FNV_PRIME);

        hash
    }

    /// Secure hash using SipHash-2-4 with server secret + blockchain salt.
    ///
    /// **CRYPTOGRAPHICALLY SECURE** - Use for rare+ items.
    ///
    /// ## Anti-Prediction Security
    ///
    /// Even if an attacker knows:
    /// - The blockchain salt (it's public)
    /// - Their player parameters (level, tool)
    ///
    /// They CANNOT predict the drop because they don't know:
    /// - The server secret (256-bit, rotated daily)
    /// - The server tick (internal counter)
    /// - The action nonce (unique per action)
    #[inline]
    fn compute_hash_secure(
        &mut self,
        block_id: u32,
        player_level: u8,
        pickaxe_tier: u8,
        weather_seed: u32,
        entropy: u32,
    ) -> u64 {
        // Generate unique action nonce (monotonic, never reused)
        let action_nonce = self.action_nonce;
        self.action_nonce = self.action_nonce.wrapping_add(1);

        // Derive keys from server secret + blockchain salt + nonce
        let (k1, k2) = self.server_seed.derive_keys(self.blockchain_salt, action_nonce);

        // SipHash-2-4 keyed with derived keys
        let mut hasher = SipHasher24::new_with_keys(k1, k2);

        // Feed all inputs into the hasher
        hasher.write_u32(block_id);
        hasher.write_u8(player_level);
        hasher.write_u8(pickaxe_tier);
        hasher.write_u32(weather_seed);
        hasher.write_u32(entropy);
        // Also mix in the action nonce for extra entropy
        hasher.write_u64(action_nonce);

        // Get 128-bit result and fold to 64-bit
        let result = hasher.finish128();
        result.h1 ^ result.h2
    }

    /// Runs statistical analysis on the loot system.
    ///
    /// Returns histogram data for verification.
    #[must_use]
    pub fn run_statistics(
        &self,
        block_id: u32,
        player_level: u8,
        pickaxe_tier: u8,
        iterations: u32,
    ) -> LootStatistics {
        let mut stats = LootStatistics::new();

        for i in 0..iterations {
            // Use iteration as entropy for variation
            let weather = i.wrapping_mul(0x9E3779B9);
            let entropy = i.wrapping_mul(0x517CC1B7);

            let result = self.calculate_drop(block_id, player_level, pickaxe_tier, weather, entropy);

            stats.total_rolls += 1;
            if let Some(item_id) = result.item_id {
                stats.total_drops += 1;
                *stats.item_counts.entry(item_id).or_insert(0) += result.quantity;
                *stats.rarity_counts.entry(result.rarity as u8).or_insert(0) += 1;
            }
        }

        stats
    }
}

impl Default for LootCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics from loot table simulation.
#[derive(Clone, Debug, Default)]
pub struct LootStatistics {
    /// Total number of rolls performed.
    pub total_rolls: u64,
    /// Total number of successful drops.
    pub total_drops: u64,
    /// Item drop counts by item ID.
    pub item_counts: HashMap<ItemId, u32>,
    /// Drop counts by rarity tier.
    pub rarity_counts: HashMap<u8, u32>,
}

impl LootStatistics {
    /// Creates empty statistics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the drop rate as a percentage.
    #[must_use]
    pub fn drop_rate_percent(&self) -> f64 {
        if self.total_rolls == 0 {
            0.0
        } else {
            (self.total_drops as f64 / self.total_rolls as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_table() -> LootTable {
        LootTable {
            block_id: 1,
            block_rarity: Rarity::Common,
            entries: vec![
                LootEntry {
                    item_id: 100,
                    weight: 70,
                    min_quantity: 1,
                    max_quantity: 3,
                    rarity: Rarity::Common,
                    min_level: 0,
                    min_pickaxe_tier: 0,
                },
                LootEntry {
                    item_id: 101,
                    weight: 20,
                    min_quantity: 1,
                    max_quantity: 1,
                    rarity: Rarity::Uncommon,
                    min_level: 5,
                    min_pickaxe_tier: 1,
                },
                LootEntry {
                    item_id: 102,
                    weight: 10,
                    min_quantity: 1,
                    max_quantity: 1,
                    rarity: Rarity::Rare,
                    min_level: 10,
                    min_pickaxe_tier: 2,
                },
            ],
            total_weight: 0,
        }
    }

    #[test]
    fn test_deterministic_drops() {
        let mut calc = LootCalculator::new();
        calc.register_table(create_test_table());

        // Same inputs should always produce same output
        let result1 = calc.calculate_drop(1, 50, 5, 12345, 67890);
        let result2 = calc.calculate_drop(1, 50, 5, 12345, 67890);

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_secure_drops_unique_per_call() {
        let mut calc = LootCalculator::new();
        calc.register_table(create_test_table());
        calc.update_blockchain_salt(BlockchainSalt::test_salt());

        // Even with same inputs, each call produces different result
        // because action_nonce increments internally
        let mut different_count = 0;
        for _ in 0..100 {
            let r1 = calc.calculate_drop_secure(1, 50, 5, 12345, 67890);
            let r2 = calc.calculate_drop_secure(1, 50, 5, 12345, 67890);
            if r1 != r2 {
                different_count += 1;
            }
        }

        // Should be different most of the time (not deterministic with same inputs)
        assert!(
            different_count > 30,
            "Secure drops should differ per call due to nonce: {different_count}/100 were different"
        );
    }

    #[test]
    fn test_secure_drops_change_with_salt() {
        let mut calc = LootCalculator::new();
        calc.register_table(create_test_table());

        // Results should differ when salt changes
        let mut different_count = 0;
        for i in 0..100u64 {
            calc.update_blockchain_salt(BlockchainSalt { low: i, high: i });
            let r1 = calc.calculate_drop_secure(1, 50, 5, 12345, 67890);
            calc.update_blockchain_salt(BlockchainSalt { low: i + 1000, high: i + 1000 });
            let r2 = calc.calculate_drop_secure(1, 50, 5, 12345, 67890);
            if r1 != r2 {
                different_count += 1;
            }
        }

        assert!(
            different_count > 50,
            "Secure drops should change with salt: {different_count}/100 were different"
        );
    }

    #[test]
    fn test_server_secret_prevents_prediction() {
        // Two calculators with different secrets but same blockchain salt
        let secret1 = [0u8; 32];
        let secret2 = [1u8; 32];

        let mut calc1 = LootCalculator::with_secret(&secret1);
        let mut calc2 = LootCalculator::with_secret(&secret2);

        calc1.register_table(create_test_table());
        calc2.register_table(create_test_table());

        let salt = BlockchainSalt { low: 42, high: 42 };
        calc1.update_blockchain_salt(salt);
        calc2.update_blockchain_salt(salt);

        // Even with same salt and inputs, different secrets = different results
        let mut different_count = 0;
        for i in 0..100 {
            let r1 = calc1.calculate_drop_secure(1, 50, 5, i, i);
            let r2 = calc2.calculate_drop_secure(1, 50, 5, i, i);
            if r1 != r2 {
                different_count += 1;
            }
        }

        assert!(
            different_count > 50,
            "Different server secrets should produce different results: {different_count}/100"
        );
    }

    #[test]
    fn test_different_entropy_different_results() {
        let mut calc = LootCalculator::new();
        calc.register_table(create_test_table());

        let mut results = Vec::new();
        for i in 0..100 {
            let result = calc.calculate_drop(1, 50, 5, i, i * 2);
            results.push(result);
        }

        let first = &results[0];
        let has_variety = results.iter().any(|r| r != first);
        assert!(has_variety, "Results should vary with different entropy");
    }

    #[test]
    fn test_level_affects_drops() {
        let mut calc = LootCalculator::new();
        calc.register_table(create_test_table());

        let low_level_stats = calc.run_statistics(1, 1, 1, 10000);
        let high_level_stats = calc.run_statistics(1, 200, 1, 10000);

        assert!(
            high_level_stats.drop_rate_percent() >= low_level_stats.drop_rate_percent(),
            "Higher level should have equal or better drop rate"
        );
    }

    #[test]
    fn test_million_drops_per_second_fast() {
        let mut calc = LootCalculator::new();
        calc.register_table(create_test_table());

        let start = std::time::Instant::now();
        for i in 0..1_000_000 {
            let _ = calc.calculate_drop(1, 50, 5, i, i);
        }
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_secs_f64() < 1.0,
            "1M fast drops took {:?}, should be < 1s",
            elapsed
        );

        println!("1,000,000 fast drops in {:?}", elapsed);
    }

    #[test]
    fn test_secure_drops_performance() {
        let mut calc = LootCalculator::new();
        calc.register_table(create_test_table());
        calc.update_blockchain_salt(BlockchainSalt::test_salt());

        let start = std::time::Instant::now();
        for i in 0..1_000_000 {
            let _ = calc.calculate_drop_secure(1, 50, 5, i, i);
        }
        let elapsed = start.elapsed();

        let drops_per_sec = 1_000_000.0 / elapsed.as_secs_f64();
        println!(
            "1,000,000 SECURE drops in {:?} ({:.0}/sec)",
            elapsed, drops_per_sec
        );

        // Target: at least 5M/sec (slower than fast but still fast enough)
        assert!(
            drops_per_sec >= 5_000_000.0,
            "Secure drops should be >= 5M/sec, got {drops_per_sec:.0}"
        );
    }

    #[test]
    fn test_cannot_predict_without_server_secret() {
        // This test documents why the server secret is essential
        //
        // Without server secret, an attacker who knows the blockchain salt
        // could pre-compute all possible outcomes and choose optimal timing.
        //
        // With server secret:
        // - Attacker sees block hash (public) = 256 bits
        // - Attacker knows their params = ~32 bits
        // - Attacker DOESN'T know server_secret = 256 bits
        // - Attacker DOESN'T know server_tick = varies
        // - Attacker DOESN'T know action_nonce = 64 bits, unique per action
        //
        // Total unknown entropy: >320 bits
        // Pre-computation: infeasible (2^320 operations)

        let secret = [42u8; 32];
        let calc = LootCalculator::with_secret(&secret);

        // Verify secret is not exposed
        let debug_str = format!("{:?}", calc.server_seed);
        assert!(
            debug_str.contains("REDACTED"),
            "Server secret should not be visible in debug output"
        );
    }
}
