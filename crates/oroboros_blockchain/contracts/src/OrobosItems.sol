// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC1155/ERC1155.sol";
import "@openzeppelin/contracts/token/ERC1155/extensions/ERC1155Burnable.sol";
import "@openzeppelin/contracts/token/ERC1155/extensions/ERC1155Supply.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";

/**
 * @title OrobosItems (Game Assets)
 * @author OROBOROS Core Kernel Team
 * @notice ERC-1155 Multi-Token standard for all game items.
 * @dev Supports polymorphic assets (swords, resources, NFTs) with quantity tracking.
 * 
 * SECURITY MODEL:
 * - Only addresses with MINTER_ROLE can mint items (Game Server hot wallet).
 * - Only addresses with MINTER_ROLE can burn items (when player deposits to game).
 * - Item metadata URI follows the {id}.json pattern for polymorphic rendering.
 * 
 * ITEM ID RANGES:
 * - 0-999: Resources (Wood, Iron, Plasma)
 * - 1000-9999: Equipment (Weapons, Armor)
 * - 10000-99999: Consumables (Potions, Scrolls)
 * - 100000+: Unique NFTs (Dragon Souls, Land Deeds)
 */
contract OrobosItems is ERC1155, ERC1155Burnable, ERC1155Supply, AccessControl {
    /// @notice Role identifier for addresses allowed to mint/burn items.
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");

    /// @notice Mapping from item ID to its name (for off-chain reference).
    mapping(uint256 => string) private _itemNames;

    /// @notice Emitted when a new item type is registered.
    event ItemRegistered(uint256 indexed itemId, string name);

    /// @notice Emitted when items are minted (player withdraws from game).
    event ItemsMinted(address indexed to, uint256 indexed itemId, uint256 amount);

    /// @notice Emitted when items are burned (player deposits to game).
    event ItemsBurned(address indexed from, uint256 indexed itemId, uint256 amount);

    /**
     * @notice Initializes the contract with base URI and admin.
     * @param admin The address that will receive DEFAULT_ADMIN_ROLE.
     * @param baseUri The base URI for token metadata (e.g., "https://api.oroboros.game/items/").
     */
    constructor(address admin, string memory baseUri) ERC1155(baseUri) {
        require(admin != address(0), "Admin cannot be zero address");
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
    }

    /**
     * @notice Mints items to a player's wallet.
     * @dev Only callable by the Game Server (MINTER_ROLE).
     * @param to The recipient address.
     * @param id The item type ID.
     * @param amount The quantity to mint.
     * @param data Additional data (unused, for ERC1155 compatibility).
     */
    function mint(
        address to,
        uint256 id,
        uint256 amount,
        bytes memory data
    ) external onlyRole(MINTER_ROLE) {
        _mint(to, id, amount, data);
        emit ItemsMinted(to, id, amount);
    }

    /**
     * @notice Mints multiple item types in a single transaction.
     * @dev Only callable by the Game Server (MINTER_ROLE).
     * @param to The recipient address.
     * @param ids Array of item type IDs.
     * @param amounts Array of quantities to mint.
     * @param data Additional data.
     */
    function mintBatch(
        address to,
        uint256[] memory ids,
        uint256[] memory amounts,
        bytes memory data
    ) external onlyRole(MINTER_ROLE) {
        _mintBatch(to, ids, amounts, data);
        for (uint256 i = 0; i < ids.length; i++) {
            emit ItemsMinted(to, ids[i], amounts[i]);
        }
    }

    /**
     * @notice Burns items from a player's wallet (when depositing to game).
     * @dev Only callable by the Game Server (MINTER_ROLE).
     * @param from The address to burn from (must have approved the server).
     * @param id The item type ID.
     * @param amount The quantity to burn.
     */
    function burnByServer(
        address from,
        uint256 id,
        uint256 amount
    ) external onlyRole(MINTER_ROLE) {
        _burn(from, id, amount);
        emit ItemsBurned(from, id, amount);
    }

    /**
     * @notice Registers a new item type with a name.
     * @dev Only callable by admin.
     * @param itemId The item type ID.
     * @param name The human-readable name.
     */
    function registerItem(uint256 itemId, string calldata name) external onlyRole(DEFAULT_ADMIN_ROLE) {
        _itemNames[itemId] = name;
        emit ItemRegistered(itemId, name);
    }

    /**
     * @notice Returns the name of an item type.
     * @param itemId The item type ID.
     * @return The item name.
     */
    function itemName(uint256 itemId) external view returns (string memory) {
        return _itemNames[itemId];
    }

    /**
     * @notice Updates the base URI for all tokens.
     * @dev Only callable by admin.
     * @param newUri The new base URI.
     */
    function setURI(string memory newUri) external onlyRole(DEFAULT_ADMIN_ROLE) {
        _setURI(newUri);
    }

    // Required overrides for Solidity
    function supportsInterface(bytes4 interfaceId) 
        public 
        view 
        override(ERC1155, AccessControl) 
        returns (bool) 
    {
        return super.supportsInterface(interfaceId);
    }

    function _update(
        address from,
        address to,
        uint256[] memory ids,
        uint256[] memory values
    ) internal override(ERC1155, ERC1155Supply) {
        super._update(from, to, ids, values);
    }
}
