// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";

/**
 * @title OrobosToken ($PULSE)
 * @author OROBOROS Core Kernel Team
 * @notice The primary in-game currency for the OROBOROS metaverse.
 * @dev ERC-20 with AccessControl for secure minting.
 * 
 * SECURITY MODEL:
 * - Only addresses with MINTER_ROLE can mint new tokens.
 * - MINTER_ROLE should ONLY be granted to TheVault contract.
 * - No public minting functions exist.
 * - Tokens can be burned by any holder (deflationary mechanism).
 */
contract OrobosToken is ERC20, ERC20Burnable, AccessControl {
    /// @notice Role identifier for addresses allowed to mint tokens.
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");

    /// @notice Maximum supply cap (1 billion tokens with 18 decimals).
    uint256 public constant MAX_SUPPLY = 1_000_000_000 * 10**18;

    /**
     * @notice Initializes the token with the deployer as admin.
     * @param admin The address that will receive DEFAULT_ADMIN_ROLE.
     */
    constructor(address admin) ERC20("OROBOROS PULSE", "PULSE") {
        require(admin != address(0), "Admin cannot be zero address");
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
    }

    /**
     * @notice Mints new tokens to a specified address.
     * @dev Only callable by addresses with MINTER_ROLE.
     * @param to The recipient address.
     * @param amount The amount of tokens to mint.
     */
    function mint(address to, uint256 amount) external onlyRole(MINTER_ROLE) {
        require(totalSupply() + amount <= MAX_SUPPLY, "Exceeds max supply");
        _mint(to, amount);
    }

    /**
     * @notice Returns the number of decimals used for token amounts.
     * @return uint8 The number of decimals (18).
     */
    function decimals() public pure override returns (uint8) {
        return 18;
    }
}
