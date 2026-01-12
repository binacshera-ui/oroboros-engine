//! # Chain-Synced State
//!
//! Game state that stays synchronized with blockchain events.
//! Designed for zero-allocation updates during gameplay.

use std::collections::HashMap;
use alloy_primitives::{Address, U256};

use super::contracts::PolymorphicNFT;
use super::events::{BlockchainEvent, NFTStateChange, NFTTransfer};

/// Pre-allocated state storage synchronized with the blockchain.
///
/// This structure maintains game-relevant blockchain state with:
/// - O(1) lookups by token ID
/// - O(1) lookups by owner address
/// - Zero allocations during normal operation (after initial setup)
///
/// # Capacity Planning
///
/// The state is pre-allocated for a maximum number of NFTs.
/// Exceeding this capacity requires a resize (which DOES allocate).
pub struct ChainSyncedState {
    /// NFT states indexed by token ID.
    nfts: HashMap<U256, PolymorphicNFT>,
    /// Token IDs owned by each address (for fast lookup).
    ownership: HashMap<Address, Vec<U256>>,
    /// Last processed block number.
    last_block: u64,
    /// Total state updates processed.
    updates_processed: u64,
}

impl ChainSyncedState {
    /// Creates a new chain-synced state with pre-allocated capacity.
    ///
    /// # Arguments
    ///
    /// * `nft_capacity` - Expected maximum number of NFTs
    /// * `owner_capacity` - Expected maximum number of unique owners
    #[must_use]
    pub fn new(nft_capacity: usize, owner_capacity: usize) -> Self {
        Self {
            nfts: HashMap::with_capacity(nft_capacity),
            ownership: HashMap::with_capacity(owner_capacity),
            last_block: 0,
            updates_processed: 0,
        }
    }

    /// Returns the last processed block number.
    #[inline]
    #[must_use]
    pub const fn last_block(&self) -> u64 {
        self.last_block
    }

    /// Returns the total number of state updates processed.
    #[inline]
    #[must_use]
    pub const fn updates_processed(&self) -> u64 {
        self.updates_processed
    }

    /// Returns the number of tracked NFTs.
    #[inline]
    #[must_use]
    pub fn nft_count(&self) -> usize {
        self.nfts.len()
    }

    /// Gets an NFT by token ID.
    ///
    /// # Arguments
    ///
    /// * `token_id` - The token ID to look up
    #[inline]
    #[must_use]
    pub fn get_nft(&self, token_id: &U256) -> Option<&PolymorphicNFT> {
        self.nfts.get(token_id)
    }

    /// Gets all NFTs owned by an address.
    ///
    /// # Arguments
    ///
    /// * `owner` - The owner address
    #[must_use]
    pub fn get_nfts_by_owner(&self, owner: &Address) -> Vec<&PolymorphicNFT> {
        self.ownership
            .get(owner)
            .map(|token_ids| {
                token_ids
                    .iter()
                    .filter_map(|id| self.nfts.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Processes a blockchain event and updates state.
    ///
    /// This is the main entry point for state updates.
    /// Designed for minimal latency.
    ///
    /// # Arguments
    ///
    /// * `event` - The blockchain event to process
    pub fn process_event(&mut self, event: &BlockchainEvent) {
        match event {
            BlockchainEvent::NFTStateChanged(state_change) => {
                self.apply_state_change(state_change);
            }
            BlockchainEvent::NFTTransfer(transfer) => {
                self.apply_transfer(transfer);
            }
            BlockchainEvent::NewBlock(block_number) => {
                self.last_block = *block_number;
            }
        }
        self.updates_processed += 1;
    }

    /// Applies an NFT state change.
    fn apply_state_change(&mut self, change: &NFTStateChange) {
        let nft = change.to_nft_state();

        // Update or insert the NFT
        if let Some(existing) = self.nfts.get_mut(&change.token_id) {
            *existing = nft;
        } else {
            // New NFT - need to track ownership
            self.nfts.insert(change.token_id, nft);
            self.ownership
                .entry(change.owner)
                .or_default()
                .push(change.token_id);
        }

        self.last_block = self.last_block.max(change.block_number);
    }

    /// Applies an NFT transfer.
    fn apply_transfer(&mut self, transfer: &NFTTransfer) {
        // Update ownership in the NFT record
        if let Some(nft) = self.nfts.get_mut(&transfer.token_id) {
            let old_owner = nft.owner;
            nft.owner = transfer.to;

            // Remove from old owner's list
            if let Some(tokens) = self.ownership.get_mut(&old_owner) {
                tokens.retain(|id| *id != transfer.token_id);
            }

            // Add to new owner's list
            self.ownership
                .entry(transfer.to)
                .or_default()
                .push(transfer.token_id);
        }

        self.last_block = self.last_block.max(transfer.block_number);
    }

    /// Batch processes multiple events.
    ///
    /// More efficient than processing one at a time for large batches.
    ///
    /// # Arguments
    ///
    /// * `events` - Iterator of events to process
    pub fn process_batch<'a>(&mut self, events: impl Iterator<Item = &'a BlockchainEvent>) {
        for event in events {
            self.process_event(event);
        }
    }

    /// Clears all state (for testing/reset).
    pub fn clear(&mut self) {
        self.nfts.clear();
        self.ownership.clear();
        self.last_block = 0;
        self.updates_processed = 0;
    }
}

impl Default for ChainSyncedState {
    fn default() -> Self {
        // Reasonable defaults for a medium-sized game
        Self::new(10_000, 1_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::NFTStateChange;

    #[test]
    fn test_state_creation() {
        let state = ChainSyncedState::new(1000, 100);
        assert_eq!(state.nft_count(), 0);
        assert_eq!(state.last_block(), 0);
    }

    #[test]
    fn test_state_change_processing() {
        let mut state = ChainSyncedState::default();

        let change = NFTStateChange {
            token_id: U256::from(1),
            owner: Address::repeat_byte(1),
            evolution_stage: 2,
            experience: 500,
            strength: 100,
            agility: 50,
            intelligence: 75,
            visual_dna: [0u8; 32],
            block_number: 12345,
            tx_index: 0,
            log_index: 0,
        };

        let event = BlockchainEvent::NFTStateChanged(change);
        state.process_event(&event);

        assert_eq!(state.nft_count(), 1);
        assert_eq!(state.last_block(), 12345);

        let nft = state.get_nft(&U256::from(1)).unwrap();
        assert_eq!(nft.evolution_stage, 2);
        assert_eq!(nft.strength, 100);
    }

    #[test]
    fn test_ownership_tracking() {
        let mut state = ChainSyncedState::default();
        let owner = Address::repeat_byte(1);

        // Add NFT
        let change = NFTStateChange {
            token_id: U256::from(1),
            owner,
            evolution_stage: 0,
            experience: 0,
            strength: 0,
            agility: 0,
            intelligence: 0,
            visual_dna: [0u8; 32],
            block_number: 1,
            tx_index: 0,
            log_index: 0,
        };

        state.process_event(&BlockchainEvent::NFTStateChanged(change));

        let owned = state.get_nfts_by_owner(&owner);
        assert_eq!(owned.len(), 1);
    }
}
