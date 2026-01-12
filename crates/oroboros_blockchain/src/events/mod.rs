//! # Blockchain Events
//!
//! Event types and parsing for blockchain notifications.
//! Optimized for minimal latency and zero-copy where possible.

use alloy_primitives::{Address, U256};
use super::contracts::PolymorphicNFT;

/// All possible blockchain events the game cares about.
#[derive(Clone, Debug)]
pub enum BlockchainEvent {
    /// An NFT's state has changed.
    NFTStateChanged(NFTStateChange),
    /// An NFT was transferred to a new owner.
    NFTTransfer(NFTTransfer),
    /// A new block was mined (for sync purposes).
    NewBlock(u64),
}

/// NFT state change event data.
#[derive(Clone, Copy, Debug)]
pub struct NFTStateChange {
    /// The token that changed.
    pub token_id: U256,
    /// The owner at time of change.
    pub owner: Address,
    /// New evolution stage.
    pub evolution_stage: u8,
    /// New experience value.
    pub experience: u32,
    /// New strength stat.
    pub strength: u16,
    /// New agility stat.
    pub agility: u16,
    /// New intelligence stat.
    pub intelligence: u16,
    /// New visual DNA.
    pub visual_dna: [u8; 32],
    /// Block number where this occurred.
    pub block_number: u64,
    /// Transaction index within block.
    pub tx_index: u32,
    /// Log index within transaction.
    pub log_index: u32,
}

impl NFTStateChange {
    /// Converts this event into an NFT state struct.
    #[inline]
    #[must_use]
    pub fn to_nft_state(&self) -> PolymorphicNFT {
        PolymorphicNFT::from_chain_data(
            self.token_id,
            self.owner,
            self.evolution_stage,
            self.experience,
            self.strength,
            self.agility,
            self.intelligence,
            self.visual_dna,
        )
    }
}

/// NFT transfer event data.
#[derive(Clone, Copy, Debug)]
pub struct NFTTransfer {
    /// The token that was transferred.
    pub token_id: U256,
    /// Previous owner.
    pub from: Address,
    /// New owner.
    pub to: Address,
    /// Block number where this occurred.
    pub block_number: u64,
}

/// Event parser for raw log data.
///
/// This is optimized for speed - we parse directly from bytes
/// without intermediate allocations.
pub struct EventParser;

impl EventParser {
    /// Parses a StateChanged event from raw log data.
    ///
    /// # Arguments
    ///
    /// * `topics` - The indexed event topics
    /// * `data` - The non-indexed event data
    /// * `block_number` - Block where event occurred
    /// * `tx_index` - Transaction index
    /// * `log_index` - Log index
    ///
    /// # Returns
    ///
    /// Parsed event or None if parsing failed.
    #[must_use]
    pub fn parse_state_changed(
        topics: &[[u8; 32]],
        data: &[u8],
        block_number: u64,
        tx_index: u32,
        log_index: u32,
    ) -> Option<NFTStateChange> {
        // Validate topic count (event sig + tokenId + owner = 3)
        if topics.len() < 3 || data.len() < 96 {
            return None;
        }

        // Parse indexed parameters from topics
        let token_id = U256::from_be_slice(&topics[1]);
        let owner = Address::from_slice(&topics[2][12..32]);

        // Parse non-indexed parameters from data
        // Layout: evolutionStage(32) | experience(32) | strength(32) |
        //         agility(32) | intelligence(32) | visualDna(32)
        let evolution_stage = data[31]; // Last byte of first 32-byte word
        let experience = u32::from_be_bytes([data[60], data[61], data[62], data[63]]);
        let strength = u16::from_be_bytes([data[94], data[95]]);

        // Continue parsing if we have enough data
        let (agility, intelligence, visual_dna) = if data.len() >= 192 {
            let agility = u16::from_be_bytes([data[126], data[127]]);
            let intelligence = u16::from_be_bytes([data[158], data[159]]);
            let mut visual_dna = [0u8; 32];
            visual_dna.copy_from_slice(&data[160..192]);
            (agility, intelligence, visual_dna)
        } else {
            (0, 0, [0u8; 32])
        };

        Some(NFTStateChange {
            token_id,
            owner,
            evolution_stage,
            experience,
            strength,
            agility,
            intelligence,
            visual_dna,
            block_number,
            tx_index,
            log_index,
        })
    }

    /// Parses a Transfer event from raw log data.
    #[must_use]
    pub fn parse_transfer(
        topics: &[[u8; 32]],
        block_number: u64,
    ) -> Option<NFTTransfer> {
        // Transfer has 4 topics: event sig, from, to, tokenId
        if topics.len() < 4 {
            return None;
        }

        let from = Address::from_slice(&topics[1][12..32]);
        let to = Address::from_slice(&topics[2][12..32]);
        let token_id = U256::from_be_slice(&topics[3]);

        Some(NFTTransfer {
            token_id,
            from,
            to,
            block_number,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_parser_transfer() {
        let topics = [
            [0u8; 32], // Event signature
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1], // from
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2], // to
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42], // tokenId
        ];

        let transfer = EventParser::parse_transfer(&topics, 12345).unwrap();

        assert_eq!(transfer.token_id, U256::from(42));
        assert_eq!(transfer.block_number, 12345);
    }
}
