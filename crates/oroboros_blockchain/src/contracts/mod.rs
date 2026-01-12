//! # Contract Definitions
//!
//! Solidity contract ABIs and type definitions generated from the contracts.

// The sol! macro generates code that we can't document, so allow missing_docs
#![allow(missing_docs)]

use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;

// Define the Polymorphic NFT contract interface using alloy's sol! macro
sol! {
    /// The Polymorphic NFT contract - core asset that changes based on player actions.
    ///
    /// This NFT's appearance and stats are stored on-chain and can be modified
    /// through gameplay, creating a living, evolving asset.
    #[derive(Debug)]
    interface IPolymorphicNFT {
        /// Emitted when an NFT's state changes.
        event StateChanged(
            uint256 indexed tokenId,
            address indexed owner,
            uint8 evolutionStage,
            uint32 experience,
            uint16 strength,
            uint16 agility,
            uint16 intelligence,
            bytes32 visualDna
        );

        /// Emitted when an NFT is transferred.
        event Transfer(
            address indexed from,
            address indexed to,
            uint256 indexed tokenId
        );

        /// Gets the current state of an NFT.
        function getState(uint256 tokenId) external view returns (
            uint8 evolutionStage,
            uint32 experience,
            uint16 strength,
            uint16 agility,
            uint16 intelligence,
            bytes32 visualDna
        );

        /// Updates NFT state (only callable by game server).
        function updateState(
            uint256 tokenId,
            uint8 evolutionStage,
            uint32 experience,
            uint16 strength,
            uint16 agility,
            uint16 intelligence,
            bytes32 visualDna
        ) external;

        /// Gets the owner of an NFT.
        function ownerOf(uint256 tokenId) external view returns (address);
    }
}

/// Rust representation of the Polymorphic NFT state.
///
/// This struct mirrors the on-chain state but is optimized for
/// fast access in the game engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PolymorphicNFT {
    /// The unique token ID.
    pub token_id: U256,
    /// Current owner address.
    pub owner: Address,
    /// Evolution stage (0-255).
    pub evolution_stage: u8,
    /// Total experience points.
    pub experience: u32,
    /// Strength stat.
    pub strength: u16,
    /// Agility stat.
    pub agility: u16,
    /// Intelligence stat.
    pub intelligence: u16,
    /// Visual DNA hash for procedural generation.
    pub visual_dna: [u8; 32],
}

impl PolymorphicNFT {
    /// Creates a new NFT state from on-chain data.
    #[must_use]
    pub fn from_chain_data(
        token_id: U256,
        owner: Address,
        evolution_stage: u8,
        experience: u32,
        strength: u16,
        agility: u16,
        intelligence: u16,
        visual_dna: [u8; 32],
    ) -> Self {
        Self {
            token_id,
            owner,
            evolution_stage,
            experience,
            strength,
            agility,
            intelligence,
            visual_dna,
        }
    }

    /// Calculates the total power level of this NFT.
    #[inline]
    #[must_use]
    pub fn power_level(&self) -> u32 {
        let base = u32::from(self.strength)
            + u32::from(self.agility)
            + u32::from(self.intelligence);
        base * u32::from(self.evolution_stage.saturating_add(1))
    }

    /// Checks if this NFT can evolve (has enough experience).
    #[inline]
    #[must_use]
    pub fn can_evolve(&self) -> bool {
        let threshold = match self.evolution_stage {
            0 => 100,
            1 => 500,
            2 => 2000,
            3 => 10000,
            _ => u32::MAX,
        };
        self.experience >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nft_power_level() {
        let nft = PolymorphicNFT {
            token_id: U256::from(1),
            owner: Address::ZERO,
            evolution_stage: 2,
            experience: 1000,
            strength: 100,
            agility: 50,
            intelligence: 50,
            visual_dna: [0u8; 32],
        };

        // (100 + 50 + 50) * (2 + 1) = 200 * 3 = 600
        assert_eq!(nft.power_level(), 600);
    }

    #[test]
    fn test_nft_evolution() {
        let mut nft = PolymorphicNFT::default();
        nft.evolution_stage = 0;
        nft.experience = 99;
        assert!(!nft.can_evolve());

        nft.experience = 100;
        assert!(nft.can_evolve());
    }
}
