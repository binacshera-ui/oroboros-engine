// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title PolymorphicNFT
 * @author OROBOROS Team
 * @notice NFT that evolves based on in-game actions
 * @dev Minimal gas, maximum flexibility. State changes trigger events for Rust listener.
 *
 * ARCHITECT'S REQUIREMENTS:
 * - Events must be parseable in < 1ms
 * - State must be minimal but complete
 * - Only game server can update (TRUST NO ONE)
 */

import "@openzeppelin/contracts/token/ERC721/ERC721.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract PolymorphicNFT is ERC721, Ownable {
    // =========================================================================
    // State Structure - Packed for gas efficiency
    // =========================================================================
    
    /// @notice NFT state - packed into 2 slots (64 bytes)
    struct NFTState {
        uint8 evolutionStage;      // 0-255 evolution stages
        uint32 experience;          // Total XP earned
        uint16 strength;            // STR stat
        uint16 agility;             // AGI stat
        uint16 intelligence;        // INT stat
        bytes32 visualDna;          // Procedural generation seed
        uint64 lastUpdateBlock;     // Last state change block
    }
    
    // =========================================================================
    // Storage
    // =========================================================================
    
    /// @notice Token ID counter
    uint256 private _nextTokenId;
    
    /// @notice NFT states by token ID
    mapping(uint256 => NFTState) public states;
    
    /// @notice Authorized game servers that can update state
    mapping(address => bool) public authorizedServers;
    
    // =========================================================================
    // Events - Indexed for fast filtering in Rust
    // =========================================================================
    
    /// @notice Emitted when NFT state changes
    /// @dev Indexed fields allow fast log filtering
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
    
    /// @notice Emitted when server authorization changes
    event ServerAuthorizationChanged(
        address indexed server,
        bool authorized
    );
    
    // =========================================================================
    // Errors - Custom errors are cheaper than strings
    // =========================================================================
    
    error NotAuthorizedServer();
    error TokenDoesNotExist();
    error InvalidEvolution();
    
    // =========================================================================
    // Modifiers
    // =========================================================================
    
    modifier onlyAuthorizedServer() {
        if (!authorizedServers[msg.sender]) {
            revert NotAuthorizedServer();
        }
        _;
    }
    
    modifier tokenExists(uint256 tokenId) {
        if (!_exists(tokenId)) {
            revert TokenDoesNotExist();
        }
        _;
    }
    
    // =========================================================================
    // Constructor
    // =========================================================================
    
    constructor() ERC721("OROBOROS Polymorphic", "POLY") Ownable(msg.sender) {
        // Owner is initially an authorized server
        authorizedServers[msg.sender] = true;
    }
    
    // =========================================================================
    // Admin Functions
    // =========================================================================
    
    /// @notice Add or remove authorized game server
    /// @param server Address of the game server
    /// @param authorized Whether to authorize or deauthorize
    function setServerAuthorization(
        address server,
        bool authorized
    ) external onlyOwner {
        authorizedServers[server] = authorized;
        emit ServerAuthorizationChanged(server, authorized);
    }
    
    // =========================================================================
    // Minting
    // =========================================================================
    
    /// @notice Mint a new NFT with initial stats
    /// @param to Recipient address
    /// @param visualDna Visual generation seed
    /// @return tokenId The minted token ID
    function mint(
        address to,
        bytes32 visualDna
    ) external onlyAuthorizedServer returns (uint256) {
        uint256 tokenId = _nextTokenId++;
        _safeMint(to, tokenId);
        
        // Initialize state
        states[tokenId] = NFTState({
            evolutionStage: 0,
            experience: 0,
            strength: 10,      // Base stats
            agility: 10,
            intelligence: 10,
            visualDna: visualDna,
            lastUpdateBlock: uint64(block.number)
        });
        
        emit StateChanged(
            tokenId,
            to,
            0,
            0,
            10,
            10,
            10,
            visualDna
        );
        
        return tokenId;
    }
    
    // =========================================================================
    // State Updates (Game Server Only)
    // =========================================================================
    
    /// @notice Update NFT state from game server
    /// @dev This is THE critical function - must be gas efficient
    /// @param tokenId Token to update
    /// @param evolutionStage New evolution stage
    /// @param experience New experience value
    /// @param strength New STR stat
    /// @param agility New AGI stat
    /// @param intelligence New INT stat
    /// @param visualDna New visual DNA (can change on evolution)
    function updateState(
        uint256 tokenId,
        uint8 evolutionStage,
        uint32 experience,
        uint16 strength,
        uint16 agility,
        uint16 intelligence,
        bytes32 visualDna
    ) external onlyAuthorizedServer tokenExists(tokenId) {
        NFTState storage state = states[tokenId];
        
        // Validate evolution (can only go up)
        if (evolutionStage < state.evolutionStage) {
            revert InvalidEvolution();
        }
        
        // Update state
        state.evolutionStage = evolutionStage;
        state.experience = experience;
        state.strength = strength;
        state.agility = agility;
        state.intelligence = intelligence;
        state.visualDna = visualDna;
        state.lastUpdateBlock = uint64(block.number);
        
        emit StateChanged(
            tokenId,
            ownerOf(tokenId),
            evolutionStage,
            experience,
            strength,
            agility,
            intelligence,
            visualDna
        );
    }
    
    /// @notice Batch update multiple NFTs (gas efficient for mass updates)
    /// @param tokenIds Array of token IDs
    /// @param newExperience Array of new experience values
    function batchAddExperience(
        uint256[] calldata tokenIds,
        uint32[] calldata newExperience
    ) external onlyAuthorizedServer {
        require(tokenIds.length == newExperience.length, "Length mismatch");
        
        for (uint256 i = 0; i < tokenIds.length; i++) {
            if (_exists(tokenIds[i])) {
                NFTState storage state = states[tokenIds[i]];
                state.experience = newExperience[i];
                state.lastUpdateBlock = uint64(block.number);
                
                emit StateChanged(
                    tokenIds[i],
                    ownerOf(tokenIds[i]),
                    state.evolutionStage,
                    newExperience[i],
                    state.strength,
                    state.agility,
                    state.intelligence,
                    state.visualDna
                );
            }
        }
    }
    
    // =========================================================================
    // View Functions
    // =========================================================================
    
    /// @notice Get full NFT state
    /// @param tokenId Token to query
    /// @return evolutionStage Current evolution stage
    /// @return experience Total experience
    /// @return strength STR stat
    /// @return agility AGI stat
    /// @return intelligence INT stat
    /// @return visualDna Visual generation seed
    function getState(uint256 tokenId) 
        external 
        view 
        tokenExists(tokenId)
        returns (
            uint8 evolutionStage,
            uint32 experience,
            uint16 strength,
            uint16 agility,
            uint16 intelligence,
            bytes32 visualDna
        ) 
    {
        NFTState storage state = states[tokenId];
        return (
            state.evolutionStage,
            state.experience,
            state.strength,
            state.agility,
            state.intelligence,
            state.visualDna
        );
    }
    
    /// @notice Calculate power level of an NFT
    /// @param tokenId Token to query
    /// @return power Combined power level
    function getPowerLevel(uint256 tokenId) 
        external 
        view 
        tokenExists(tokenId)
        returns (uint256) 
    {
        NFTState storage state = states[tokenId];
        uint256 base = uint256(state.strength) + 
                       uint256(state.agility) + 
                       uint256(state.intelligence);
        return base * (uint256(state.evolutionStage) + 1);
    }
    
    /// @notice Get total supply
    /// @return Total minted tokens
    function totalSupply() external view returns (uint256) {
        return _nextTokenId;
    }
    
    // =========================================================================
    // Internal Helpers
    // =========================================================================
    
    function _exists(uint256 tokenId) internal view returns (bool) {
        return tokenId < _nextTokenId;
    }
}
