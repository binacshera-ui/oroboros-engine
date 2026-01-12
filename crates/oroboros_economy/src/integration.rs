//! # Integration API for Squad Veridia
//!
//! **THE BANK** - Nothing is created without our approval.
//!
//! This module exposes the public API that other squads call:
//! - Unit 4 (Inferno) calls us for block breaks, combat damage, trades
//! - Unit 1 (Core) reads our inventory state
//! - Unit 2 (Neon) subscribes to our events for VFX
//!
//! ## The Golden Path: Block Break
//!
//! ```text
//! Unit 4 (Server) ──> on_block_break() ──> Unit 3 (Veridia)
//!                                              │
//!                     ┌────────────────────────┼────────────────────────┐
//!                     │                        │                        │
//!                     ▼                        ▼                        ▼
//!              Calculate Loot           Write to WAL            Update Inventory
//!              (Crypto RNG)             (Batched)               (Unit 1 Memory)
//!                     │                        │                        │
//!                     └────────────────────────┼────────────────────────┘
//!                                              │
//!                                              ▼
//!                                    Return BlockBreakResult
//!                                              │
//!                     ┌────────────────────────┼────────────────────────┐
//!                     │                        │                        │
//!                     ▼                        ▼                        ▼
//!              Unit 4: Broadcast       Unit 2: VFX             Unit 1: Commit
//! ```
//!
//! ## Performance Budget
//!
//! Total time from input to VFX: **< 50ms**
//! Our share (Veridia processing): **< 5ms**

use crate::error::EconomyResult;
use crate::inventory::{Inventory, ItemId};
use crate::loot::{BlockchainSalt, LootCalculator, Rarity};
use crate::wal_batched::{BatchedWal, BatchedWalConfig};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

// ============================================================================
// Public Types for Cross-Unit Communication
// ============================================================================

/// Entity ID (matches Unit 1's entity system).
pub type EntityId = u64;

/// Block ID in the world.
pub type BlockId = u32;

/// Result of breaking a block.
#[derive(Clone, Debug)]
pub struct BlockBreakResult {
    /// Whether the break was successful.
    pub success: bool,
    /// Items dropped (if any).
    pub drops: Vec<ItemDrop>,
    /// Time taken for economy processing (microseconds).
    pub processing_time_us: u64,
    /// WAL sequence number (for crash recovery).
    pub wal_lsn: Option<u64>,
}

/// A single item drop.
#[derive(Clone, Debug)]
pub struct ItemDrop {
    /// Item type ID.
    pub item_id: ItemId,
    /// Quantity dropped.
    pub quantity: u32,
    /// Rarity tier (for VFX selection).
    pub rarity: DropRarity,
}

/// Rarity tier for VFX (simplified for Unit 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DropRarity {
    /// Common - gray sparkles.
    Common = 0,
    /// Uncommon - green glow.
    Uncommon = 1,
    /// Rare - blue flash.
    Rare = 2,
    /// Epic - purple burst.
    Epic = 3,
    /// Legendary - orange explosion.
    Legendary = 4,
    /// Mythic - red supernova.
    Mythic = 5,
}

impl From<Rarity> for DropRarity {
    fn from(r: Rarity) -> Self {
        match r {
            Rarity::Common => Self::Common,
            Rarity::Uncommon => Self::Uncommon,
            Rarity::Rare => Self::Rare,
            Rarity::Epic => Self::Epic,
            Rarity::Legendary => Self::Legendary,
            Rarity::Mythic => Self::Mythic,
        }
    }
}

/// Result of a crafting operation.
#[derive(Clone, Debug)]
pub struct CraftResult {
    /// Whether crafting succeeded.
    pub success: bool,
    /// Items consumed.
    pub consumed: Vec<(ItemId, u32)>,
    /// Items produced.
    pub produced: Vec<(ItemId, u32)>,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Economic event for Unit 2 VFX system.
#[derive(Clone, Debug)]
pub enum EconomyEvent {
    /// Block was broken, items dropped.
    BlockBroken {
        /// Position in world (for particle spawn).
        position: [f32; 3],
        /// Block type that was broken.
        block_id: BlockId,
        /// Items that dropped.
        drops: Vec<ItemDrop>,
    },
    /// Item was crafted.
    ItemCrafted {
        /// Recipe that was used.
        recipe_id: u32,
        /// Output items.
        outputs: Vec<(ItemId, u32)>,
    },
    /// Rare drop occurred (special VFX).
    RareDrop {
        /// Position in world.
        position: [f32; 3],
        /// The rare item.
        item_id: ItemId,
        /// Rarity level.
        rarity: DropRarity,
    },
}

// ============================================================================
// The Bank - Main Integration Interface
// ============================================================================

/// The Bank - Squad Veridia's integration interface.
///
/// This is the single point of contact for other units.
/// All economic operations go through here.
///
/// ## Thread Safety
///
/// `TheBank` is `Send + Sync` and can be shared across threads.
/// Internal synchronization is handled via the batched WAL.
///
/// ## Usage (by Unit 4)
///
/// ```rust,ignore
/// // On server startup
/// let bank = TheBank::init("data/economy.wal")?;
///
/// // When player breaks a block
/// let result = bank.on_block_break(
///     player_entity,
///     block_id,
///     block_position,
///     player_level,
///     tool_tier,
/// )?;
///
/// // Broadcast result to clients
/// if !result.drops.is_empty() {
///     network.broadcast(Event::BlockBroken { drops: result.drops });
/// }
/// ```
pub struct TheBank {
    /// Loot calculator with crypto RNG.
    loot: parking_lot::RwLock<LootCalculator>,
    /// Batched WAL for durability.
    wal: Arc<BatchedWal>,
    /// Inventories by entity ID.
    inventories: parking_lot::RwLock<HashMap<EntityId, Inventory>>,
    /// Max stack sizes by item ID.
    max_stacks: HashMap<ItemId, u32>,
    /// Event buffer for Unit 2 (VFX).
    event_buffer: parking_lot::Mutex<Vec<EconomyEvent>>,
    /// Current blockchain salt.
    blockchain_salt: parking_lot::RwLock<BlockchainSalt>,
}

impl TheBank {
    /// Initializes The Bank.
    ///
    /// # Arguments
    ///
    /// * `wal_path` - Path to the WAL file
    /// * `server_secret` - 32-byte secret for crypto RNG (from secure storage)
    ///
    /// # Errors
    ///
    /// Returns error if WAL cannot be opened.
    pub fn init(wal_path: impl AsRef<Path>, server_secret: &[u8; 32]) -> EconomyResult<Self> {
        let wal = BatchedWal::open(wal_path, BatchedWalConfig::production())?;

        Ok(Self {
            loot: parking_lot::RwLock::new(LootCalculator::with_secret(server_secret)),
            wal: Arc::new(wal),
            inventories: parking_lot::RwLock::new(HashMap::new()),
            max_stacks: Self::default_max_stacks(),
            event_buffer: parking_lot::Mutex::new(Vec::with_capacity(1000)),
            blockchain_salt: parking_lot::RwLock::new(BlockchainSalt::default()),
        })
    }

    /// Default max stack sizes.
    fn default_max_stacks() -> HashMap<ItemId, u32> {
        let mut stacks = HashMap::new();
        // Common materials
        stacks.insert(1, 64);   // Stone
        stacks.insert(2, 64);   // Wood
        stacks.insert(3, 64);   // Iron Ore
        stacks.insert(4, 64);   // Gold Ore
        stacks.insert(5, 64);   // Diamond
        // Tools (non-stackable)
        stacks.insert(100, 1);  // Wooden Pickaxe
        stacks.insert(101, 1);  // Stone Pickaxe
        stacks.insert(102, 1);  // Iron Pickaxe
        stacks.insert(103, 1);  // Diamond Pickaxe
        stacks
    }

    // ========================================================================
    // API for Unit 4 (Inferno) - Primary Writer
    // ========================================================================

    /// Called when a player breaks a block.
    ///
    /// **THE GOLDEN PATH ENTRY POINT**
    ///
    /// This is the main function Unit 4 calls after validating a block break.
    ///
    /// # Arguments
    ///
    /// * `entity_id` - The player entity that broke the block
    /// * `block_id` - The block type that was broken
    /// * `position` - World position of the block (for VFX)
    /// * `player_level` - Player's current level (0-255)
    /// * `tool_tier` - Tool tier used (0-255)
    ///
    /// # Returns
    ///
    /// `BlockBreakResult` with drops and processing time.
    ///
    /// # Performance
    ///
    /// Target: **< 1ms** (excluding WAL commit wait)
    pub fn on_block_break(
        &self,
        entity_id: EntityId,
        block_id: BlockId,
        position: [f32; 3],
        player_level: u8,
        tool_tier: u8,
    ) -> EconomyResult<BlockBreakResult> {
        let start = Instant::now();

        // Get current blockchain entropy
        let salt = *self.blockchain_salt.read();
        let weather_seed = salt.low as u32;
        let entropy = salt.high as u32;

        // Calculate loot (crypto RNG for rare items)
        let drop_result = {
            let mut loot = self.loot.write();
            loot.calculate_drop_secure(block_id, player_level, tool_tier, weather_seed, entropy)
        };

        let mut drops = Vec::new();
        let mut wal_lsn = None;

        if let Some(item_id) = drop_result.item_id {
            let quantity = drop_result.quantity;
            let rarity = DropRarity::from(drop_result.rarity);

            // Add to inventory
            let max_stack = *self.max_stacks.get(&item_id).unwrap_or(&64);
            {
                let mut inventories = self.inventories.write();
                let inventory = inventories.entry(entity_id).or_insert_with(Inventory::new);
                inventory.add(item_id, quantity, max_stack)?;
            }

            // Write to WAL (async, non-blocking)
            let handle = self.wal.log_loot_drop(entity_id, block_id, item_id, quantity)?;
            wal_lsn = Some(handle.lsn);

            // Build drop info
            let drop = ItemDrop {
                item_id,
                quantity,
                rarity,
            };
            drops.push(drop.clone());

            // Queue event for Unit 2 VFX
            {
                let mut events = self.event_buffer.lock();
                events.push(EconomyEvent::BlockBroken {
                    position,
                    block_id,
                    drops: vec![drop.clone()],
                });

                // Special event for rare+ drops
                if rarity as u8 >= DropRarity::Rare as u8 {
                    events.push(EconomyEvent::RareDrop {
                        position,
                        item_id,
                        rarity,
                    });
                }
            }
        }

        let processing_time_us = start.elapsed().as_micros() as u64;

        Ok(BlockBreakResult {
            success: true,
            drops,
            processing_time_us,
            wal_lsn,
        })
    }

    /// Updates the blockchain salt (call every block).
    ///
    /// Unit 4 should call this when a new blockchain block is received.
    pub fn update_blockchain_salt(&self, salt: BlockchainSalt) {
        *self.blockchain_salt.write() = salt;
        self.loot.write().update_blockchain_salt(salt);
    }

    /// Updates the server tick (call every game tick).
    pub fn update_server_tick(&self, tick: u64) {
        self.loot.write().update_server_tick(tick);
    }

    // ========================================================================
    // API for Unit 1 (Core) - Memory Owner
    // ========================================================================

    /// Gets a player's inventory for reading.
    ///
    /// Unit 1 can use this to sync inventory state to the ECS.
    #[must_use]
    pub fn get_inventory(&self, entity_id: EntityId) -> Option<Inventory> {
        self.inventories.read().get(&entity_id).cloned()
    }

    /// Gets item count for a specific item.
    #[must_use]
    pub fn get_item_count(&self, entity_id: EntityId, item_id: ItemId) -> u32 {
        self.inventories
            .read()
            .get(&entity_id)
            .map(|inv| inv.count_item(item_id))
            .unwrap_or(0)
    }

    // ========================================================================
    // API for Unit 2 (Neon) - VFX Consumer
    // ========================================================================

    /// Drains all pending economy events for VFX.
    ///
    /// Unit 2 should call this every frame to get events for particle effects.
    pub fn drain_events(&self) -> Vec<EconomyEvent> {
        let mut events = self.event_buffer.lock();
        std::mem::take(&mut *events)
    }

    /// Peeks at pending event count (for debug UI).
    #[must_use]
    pub fn pending_event_count(&self) -> usize {
        self.event_buffer.lock().len()
    }

    // ========================================================================
    // Maintenance
    // ========================================================================

    /// Flushes all pending WAL writes.
    ///
    /// Call this before shutdown or checkpoint.
    pub fn flush(&self) -> EconomyResult<()> {
        self.wal.flush()
    }

    /// Returns WAL statistics.
    #[must_use]
    pub fn wal_stats(&self) -> crate::wal_batched::WalStats {
        self.wal.stats()
    }
}

// Thread safety is guaranteed by:
// - parking_lot::RwLock for loot, inventories, blockchain_salt
// - parking_lot::Mutex for event_buffer
// - Arc<BatchedWal> for WAL
// - HashMap<ItemId, u32> is Send + Sync (read-only after init)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loot::{LootEntry, LootTable, Rarity};

    fn temp_wal_path() -> std::path::PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("test_bank_{id}.wal"))
    }

    fn create_test_loot_table() -> LootTable {
        LootTable {
            block_id: 1,
            block_rarity: Rarity::Common,
            entries: vec![
                LootEntry {
                    item_id: 1,
                    weight: 90,
                    min_quantity: 1,
                    max_quantity: 3,
                    rarity: Rarity::Common,
                    min_level: 0,
                    min_pickaxe_tier: 0,
                },
                LootEntry {
                    item_id: 5, // Diamond
                    weight: 10,
                    min_quantity: 1,
                    max_quantity: 1,
                    rarity: Rarity::Rare,
                    min_level: 0,
                    min_pickaxe_tier: 0,
                },
            ],
            total_weight: 0,
        }
    }

    #[test]
    fn test_golden_path_block_break() {
        let path = temp_wal_path();
        let secret = [42u8; 32];

        let bank = TheBank::init(&path, &secret).unwrap();

        // Register loot table
        {
            let mut loot = bank.loot.write();
            loot.register_table(create_test_loot_table());
        }

        // Simulate block break
        let result = bank
            .on_block_break(
                1,            // entity_id
                1,            // block_id
                [0.0, 0.0, 0.0], // position
                50,           // player_level
                3,            // tool_tier
            )
            .unwrap();

        assert!(result.success);
        println!("Block break took {} µs", result.processing_time_us);
        println!("Drops: {:?}", result.drops);

        // Check events were queued for Unit 2
        let events = bank.drain_events();
        println!("Events for VFX: {:?}", events);

        // Cleanup
        drop(bank);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_block_break_under_1ms() {
        let path = temp_wal_path();
        let secret = [42u8; 32];

        let bank = TheBank::init(&path, &secret).unwrap();

        // Register loot table
        {
            let mut loot = bank.loot.write();
            loot.register_table(create_test_loot_table());
        }

        // Warm up
        for _ in 0..100 {
            let _ = bank.on_block_break(1, 1, [0.0, 0.0, 0.0], 50, 3);
        }

        // Measure
        let mut total_us = 0u64;
        let iterations = 1000;

        for _ in 0..iterations {
            let result = bank
                .on_block_break(1, 1, [0.0, 0.0, 0.0], 50, 3)
                .unwrap();
            total_us += result.processing_time_us;
        }

        let avg_us = total_us / iterations;
        println!("Average block break: {} µs", avg_us);

        // Target: < 1000µs (1ms)
        #[cfg(not(debug_assertions))]
        assert!(
            avg_us < 1000,
            "Block break should be < 1ms, got {} µs",
            avg_us
        );

        // Cleanup
        drop(bank);
        std::fs::remove_file(&path).ok();
    }
}
