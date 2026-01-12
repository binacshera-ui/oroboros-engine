// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/utils/cryptography/MessageHashUtils.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/**
 * @title TheVault (The Bridge)
 * @author OROBOROS Core Kernel Team
 * @notice The heart of the OROBOROS economy - bridges ETH between blockchain and game.
 * @dev This contract is the ONLY entry/exit point for real money in the game.
 * 
 * SECURITY MODEL (PARANOID LEVEL):
 * ================================
 * 1. DEFAULT_ADMIN_ROLE: CEO's Cold Wallet (Ledger/Trezor, air-gapped).
 *    - Can withdraw accumulated fees.
 *    - Can pause the contract in emergency.
 *    - Can grant/revoke SIGNER_ROLE.
 * 
 * 2. SIGNER_ROLE: Game Server's Hot Wallet.
 *    - CANNOT withdraw fees.
 *    - CANNOT grant roles.
 *    - CAN sign withdrawal approvals for players.
 * 
 * FLOW:
 * =====
 * DEPOSIT: Player sends ETH -> Contract emits event -> Server credits in-game balance.
 * WITHDRAW: Player requests withdrawal -> Server signs approval -> Player submits signature
 *           -> Contract verifies -> Deducts fee -> Sends ETH to player.
 * 
 * INVARIANT: 
 * sum(player_withdrawals) + accumulated_fees + contract_balance == sum(deposits)
 */
contract TheVault is AccessControl, ReentrancyGuard {
    using ECDSA for bytes32;
    using MessageHashUtils for bytes32;

    /// @notice Role identifier for the CEO's wallet (fee withdrawal, admin).
    bytes32 public constant CEO_ROLE = DEFAULT_ADMIN_ROLE;

    /// @notice Role identifier for the Game Server's hot wallet (signing withdrawals).
    bytes32 public constant SIGNER_ROLE = keccak256("SIGNER_ROLE");

    /// @notice Withdrawal fee in basis points (100 = 1%, 200 = 2%).
    uint256 public withdrawalFeeBps = 200; // 2% default

    /// @notice Accumulated fees available for CEO withdrawal.
    uint256 public accumulatedFees;

    /// @notice Minimum deposit amount (prevents dust attacks).
    uint256 public constant MIN_DEPOSIT = 0.001 ether;

    /// @notice Maximum single withdrawal (circuit breaker).
    uint256 public maxWithdrawal = 10 ether;

    /// @notice Nonce tracking to prevent replay attacks.
    mapping(address => uint256) public nonces;

    /// @notice Pause flag for emergency.
    bool public paused;

    // =========== EVENTS ===========

    /// @notice Emitted when ETH is deposited.
    event Deposited(
        address indexed player,
        uint256 amount,
        uint256 timestamp
    );

    /// @notice Emitted when ETH is withdrawn.
    event Withdrawn(
        address indexed player,
        uint256 grossAmount,
        uint256 fee,
        uint256 netAmount,
        uint256 nonce
    );

    /// @notice Emitted when fees are claimed by CEO.
    event FeesClaimed(
        address indexed ceo,
        uint256 amount
    );

    /// @notice Emitted when withdrawal fee is updated.
    event FeeUpdated(uint256 oldFeeBps, uint256 newFeeBps);

    /// @notice Emitted when contract is paused/unpaused.
    event PauseToggled(bool paused);

    // =========== ERRORS ===========

    error ContractPaused();
    error DepositTooSmall();
    error WithdrawalTooLarge();
    error InvalidSignature();
    error InvalidNonce();
    error InsufficientContractBalance();
    error TransferFailed();
    error NoFeesToClaim();
    error InvalidFee();

    // =========== MODIFIERS ===========

    modifier whenNotPaused() {
        if (paused) revert ContractPaused();
        _;
    }

    // =========== CONSTRUCTOR ===========

    /**
     * @notice Initializes the vault with CEO and Server addresses.
     * @param ceoWallet The CEO's cold wallet address.
     * @param serverWallet The Game Server's hot wallet address.
     */
    constructor(address ceoWallet, address serverWallet) {
        require(ceoWallet != address(0), "CEO wallet cannot be zero");
        require(serverWallet != address(0), "Server wallet cannot be zero");
        
        _grantRole(CEO_ROLE, ceoWallet);
        _grantRole(SIGNER_ROLE, serverWallet);
    }

    // =========== DEPOSIT ===========

    /**
     * @notice Deposits ETH to receive in-game credits.
     * @dev Emits a Deposited event for the server to process.
     */
    function depositETH() external payable whenNotPaused {
        if (msg.value < MIN_DEPOSIT) revert DepositTooSmall();

        emit Deposited(msg.sender, msg.value, block.timestamp);
    }

    /**
     * @notice Allows direct ETH transfers as deposits.
     */
    receive() external payable {
        if (msg.value >= MIN_DEPOSIT) {
            emit Deposited(msg.sender, msg.value, block.timestamp);
        }
    }

    // =========== WITHDRAW ===========

    /**
     * @notice Withdraws ETH using a server-signed approval.
     * @dev The signature proves the server authorized this withdrawal.
     * @param amount The gross amount to withdraw.
     * @param signature The server's signature over (player, amount, nonce, chainId, contract).
     */
    function withdrawETH(
        uint256 amount,
        bytes calldata signature
    ) external nonReentrant whenNotPaused {
        if (amount > maxWithdrawal) revert WithdrawalTooLarge();

        // Get and increment nonce (prevents replay)
        uint256 nonce = nonces[msg.sender]++;

        // Construct the message hash
        bytes32 messageHash = keccak256(
            abi.encodePacked(
                msg.sender,
                amount,
                nonce,
                block.chainid,
                address(this)
            )
        );

        // Convert to Ethereum Signed Message Hash
        bytes32 ethSignedHash = messageHash.toEthSignedMessageHash();

        // Recover the signer
        address signer = ethSignedHash.recover(signature);

        // Verify the signer has SIGNER_ROLE
        if (!hasRole(SIGNER_ROLE, signer)) revert InvalidSignature();

        // Calculate fee
        uint256 fee = (amount * withdrawalFeeBps) / 10000;
        uint256 netAmount = amount - fee;

        // Check contract balance
        if (address(this).balance < amount) revert InsufficientContractBalance();

        // Accumulate fee for CEO
        accumulatedFees += fee;

        // Transfer to player
        (bool success, ) = msg.sender.call{value: netAmount}("");
        if (!success) revert TransferFailed();

        emit Withdrawn(msg.sender, amount, fee, netAmount, nonce);
    }

    // =========== CEO FUNCTIONS ===========

    /**
     * @notice Withdraws accumulated fees to the CEO's wallet.
     * @dev Only callable by addresses with CEO_ROLE.
     */
    function claimFees() external onlyRole(CEO_ROLE) nonReentrant {
        uint256 fees = accumulatedFees;
        if (fees == 0) revert NoFeesToClaim();

        accumulatedFees = 0;

        (bool success, ) = msg.sender.call{value: fees}("");
        if (!success) revert TransferFailed();

        emit FeesClaimed(msg.sender, fees);
    }

    /**
     * @notice Updates the withdrawal fee percentage.
     * @dev Only callable by CEO. Max fee is 10% (1000 bps).
     * @param newFeeBps The new fee in basis points.
     */
    function setWithdrawalFee(uint256 newFeeBps) external onlyRole(CEO_ROLE) {
        if (newFeeBps > 1000) revert InvalidFee(); // Max 10%
        
        emit FeeUpdated(withdrawalFeeBps, newFeeBps);
        withdrawalFeeBps = newFeeBps;
    }

    /**
     * @notice Updates the maximum withdrawal amount.
     * @dev Circuit breaker to limit damage from compromised server.
     * @param newMax The new maximum withdrawal in wei.
     */
    function setMaxWithdrawal(uint256 newMax) external onlyRole(CEO_ROLE) {
        maxWithdrawal = newMax;
    }

    /**
     * @notice Pauses or unpauses the contract.
     * @dev Only callable by CEO. Emergency stop mechanism.
     */
    function togglePause() external onlyRole(CEO_ROLE) {
        paused = !paused;
        emit PauseToggled(paused);
    }

    // =========== VIEW FUNCTIONS ===========

    /**
     * @notice Returns the current nonce for a player.
     * @param player The player's address.
     * @return The current nonce.
     */
    function getNonce(address player) external view returns (uint256) {
        return nonces[player];
    }

    /**
     * @notice Returns the contract's ETH balance minus accumulated fees.
     * @return The withdrawable balance.
     */
    function availableBalance() external view returns (uint256) {
        return address(this).balance - accumulatedFees;
    }

    /**
     * @notice Generates the message hash for off-chain signing.
     * @param player The player's address.
     * @param amount The withdrawal amount.
     * @param nonce The player's current nonce.
     * @return The message hash to sign.
     */
    function getWithdrawalHash(
        address player,
        uint256 amount,
        uint256 nonce
    ) external view returns (bytes32) {
        return keccak256(
            abi.encodePacked(
                player,
                amount,
                nonce,
                block.chainid,
                address(this)
            )
        );
    }
}
