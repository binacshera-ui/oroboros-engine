// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/OrobosToken.sol";
import "../src/OrobosItems.sol";
import "../src/TheVault.sol";

/**
 * @title Economy Tests
 * @notice Comprehensive security tests for OROBOROS economy contracts.
 * @dev These tests prove that ONLY authorized addresses can perform critical actions.
 */
contract EconomyTest is Test {
    // Contracts
    OrobosToken public token;
    OrobosItems public items;
    TheVault public vault;

    // Test accounts
    address public ceo;
    uint256 public ceoPrivateKey = 0xABCDEF1234567890ABCDEF1234567890ABCDEF1234567890ABCDEF1234567890;
    address public serverWallet;
    uint256 public serverPrivateKey = 0x1234567890ABCDEF1234567890ABCDEF1234567890ABCDEF1234567890ABCDEF;
    address public player;
    address public attacker;

    // Constants
    uint256 constant INITIAL_BALANCE = 100 ether;
    uint256 constant DEPOSIT_AMOUNT = 1 ether;
    uint256 constant WITHDRAWAL_AMOUNT = 0.5 ether;

    function setUp() public {
        // Create accounts with known private keys
        ceo = vm.addr(ceoPrivateKey);
        serverWallet = vm.addr(serverPrivateKey);
        
        player = makeAddr("player");
        attacker = makeAddr("attacker");

        // Fund accounts
        vm.deal(ceo, INITIAL_BALANCE);
        vm.deal(player, INITIAL_BALANCE);
        vm.deal(attacker, INITIAL_BALANCE);

        // Deploy contracts
        vm.startPrank(ceo);
        
        token = new OrobosToken(ceo);
        items = new OrobosItems(ceo, "https://api.oroboros.game/items/");
        vault = new TheVault(ceo, serverWallet);

        // Grant MINTER_ROLE to vault for token minting
        token.grantRole(token.MINTER_ROLE(), address(vault));
        
        // Grant MINTER_ROLE to server for item operations
        items.grantRole(items.MINTER_ROLE(), serverWallet);

        vm.stopPrank();

        // Fund the vault with some ETH
        vm.deal(address(vault), 10 ether);
    }

    // ==================== TOKEN SECURITY TESTS ====================

    function test_Token_OnlyMinterCanMint() public {
        // Attempt mint as random user (should fail)
        vm.prank(attacker);
        vm.expectRevert();
        token.mint(attacker, 1000 ether);

        // Attempt mint as player (should fail)
        vm.prank(player);
        vm.expectRevert();
        token.mint(player, 1000 ether);

        // Verify no tokens were minted
        assertEq(token.totalSupply(), 0);
    }

    function test_Token_VaultCanMint() public {
        // This would typically be called by vault internally
        // For testing, we prank as vault
        vm.prank(address(vault));
        token.mint(player, 100 ether);

        assertEq(token.balanceOf(player), 100 ether);
    }

    function test_Token_MaxSupplyCap() public {
        // Grant minter role to a test address for this test
        address testMinter = makeAddr("testMinter");
        vm.startPrank(ceo);
        token.grantRole(token.MINTER_ROLE(), testMinter);
        vm.stopPrank();
        
        // Mint up to max supply
        vm.startPrank(testMinter);
        token.mint(player, token.MAX_SUPPLY());
        
        assertEq(token.totalSupply(), token.MAX_SUPPLY());

        // Try to mint even 1 more token - should fail
        vm.expectRevert("Exceeds max supply");
        token.mint(player, 1);
        vm.stopPrank();
    }

    // ==================== ITEMS SECURITY TESTS ====================

    function test_Items_OnlyServerCanMint() public {
        // Attacker tries to mint items
        vm.prank(attacker);
        vm.expectRevert();
        items.mint(attacker, 1, 100, "");

        // Player tries to mint items
        vm.prank(player);
        vm.expectRevert();
        items.mint(player, 1, 100, "");
    }

    function test_Items_ServerCanMint() public {
        // Server mints items to player
        vm.prank(serverWallet);
        items.mint(player, 1, 100, "");

        assertEq(items.balanceOf(player, 1), 100);
    }

    function test_Items_ServerCanBurn() public {
        // Setup: mint items to player
        vm.prank(serverWallet);
        items.mint(player, 1, 100, "");

        // Player approves server to burn
        vm.prank(player);
        items.setApprovalForAll(serverWallet, true);

        // Server burns items
        vm.prank(serverWallet);
        items.burnByServer(player, 1, 50);

        assertEq(items.balanceOf(player, 1), 50);
    }

    function test_Items_AttackerCannotBurn() public {
        // Setup: mint items to player
        vm.prank(serverWallet);
        items.mint(player, 1, 100, "");

        // Attacker tries to burn player's items
        vm.prank(attacker);
        vm.expectRevert();
        items.burnByServer(player, 1, 50);

        // Balance unchanged
        assertEq(items.balanceOf(player, 1), 100);
    }

    // ==================== VAULT DEPOSIT TESTS ====================

    function test_Vault_PlayerCanDeposit() public {
        vm.prank(player);
        vault.depositETH{value: DEPOSIT_AMOUNT}();

        // Check vault received the ETH
        assertEq(address(vault).balance, 10 ether + DEPOSIT_AMOUNT);
    }

    function test_Vault_DepositEmitsEvent() public {
        vm.prank(player);
        
        vm.expectEmit(true, false, false, true);
        emit TheVault.Deposited(player, DEPOSIT_AMOUNT, block.timestamp);
        
        vault.depositETH{value: DEPOSIT_AMOUNT}();
    }

    function test_Vault_RejectsDustDeposit() public {
        vm.prank(player);
        vm.expectRevert(TheVault.DepositTooSmall.selector);
        vault.depositETH{value: 0.0001 ether}();
    }

    // ==================== VAULT WITHDRAWAL TESTS ====================

    function test_Vault_PlayerCanWithdrawWithSignature() public {
        uint256 nonce = vault.getNonce(player);
        
        // Create the message hash
        bytes32 messageHash = keccak256(
            abi.encodePacked(
                player,
                WITHDRAWAL_AMOUNT,
                nonce,
                block.chainid,
                address(vault)
            )
        );
        
        // Sign with server's private key
        bytes32 ethSignedHash = keccak256(
            abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash)
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(serverPrivateKey, ethSignedHash);
        bytes memory signature = abi.encodePacked(r, s, v);

        // Record balances
        uint256 playerBalanceBefore = player.balance;

        // Withdraw
        vm.prank(player);
        vault.withdrawETH(WITHDRAWAL_AMOUNT, signature);

        // Check fee calculation (2% fee)
        uint256 fee = (WITHDRAWAL_AMOUNT * 200) / 10000;
        uint256 netAmount = WITHDRAWAL_AMOUNT - fee;

        // Player received net amount
        assertEq(player.balance, playerBalanceBefore + netAmount);

        // Fees accumulated
        assertEq(vault.accumulatedFees(), fee);
    }

    function test_Vault_RejectInvalidSignature() public {
        uint256 nonce = vault.getNonce(player);
        
        // Create message hash
        bytes32 messageHash = keccak256(
            abi.encodePacked(
                player,
                WITHDRAWAL_AMOUNT,
                nonce,
                block.chainid,
                address(vault)
            )
        );
        
        // Sign with ATTACKER's private key (not the server!)
        bytes32 ethSignedHash = keccak256(
            abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash)
        );
        uint256 attackerKey = 0xBAD;
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(attackerKey, ethSignedHash);
        bytes memory fakeSignature = abi.encodePacked(r, s, v);

        // Should reject
        vm.prank(player);
        vm.expectRevert(TheVault.InvalidSignature.selector);
        vault.withdrawETH(WITHDRAWAL_AMOUNT, fakeSignature);
    }

    function test_Vault_PreventReplayAttack() public {
        uint256 nonce = vault.getNonce(player);
        
        // Create valid signature
        bytes32 messageHash = keccak256(
            abi.encodePacked(
                player,
                WITHDRAWAL_AMOUNT,
                nonce,
                block.chainid,
                address(vault)
            )
        );
        bytes32 ethSignedHash = keccak256(
            abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash)
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(serverPrivateKey, ethSignedHash);
        bytes memory signature = abi.encodePacked(r, s, v);

        // First withdrawal succeeds
        vm.prank(player);
        vault.withdrawETH(WITHDRAWAL_AMOUNT, signature);

        // Second withdrawal with same signature fails (nonce changed)
        vm.prank(player);
        vm.expectRevert(TheVault.InvalidSignature.selector);
        vault.withdrawETH(WITHDRAWAL_AMOUNT, signature);
    }

    // ==================== FEE WITHDRAWAL TESTS ====================

    function test_Vault_OnlyCEOCanClaimFees() public {
        // Generate some fees first
        _performValidWithdrawal();

        uint256 fees = vault.accumulatedFees();
        assertTrue(fees > 0, "Should have accumulated fees");

        // Attacker tries to claim fees
        vm.prank(attacker);
        vm.expectRevert();
        vault.claimFees();

        // Player tries to claim fees
        vm.prank(player);
        vm.expectRevert();
        vault.claimFees();

        // Server tries to claim fees
        vm.prank(serverWallet);
        vm.expectRevert();
        vault.claimFees();

        // Fees still there
        assertEq(vault.accumulatedFees(), fees);
    }

    function test_Vault_CEOCanClaimFees() public {
        // Generate some fees
        _performValidWithdrawal();

        uint256 fees = vault.accumulatedFees();
        uint256 ceoBalanceBefore = ceo.balance;

        // CEO claims fees
        vm.prank(ceo);
        vault.claimFees();

        // CEO received fees
        assertEq(ceo.balance, ceoBalanceBefore + fees);

        // Accumulated fees reset to 0
        assertEq(vault.accumulatedFees(), 0);
    }

    function test_Vault_CEOCanPause() public {
        // CEO pauses
        vm.prank(ceo);
        vault.togglePause();

        assertTrue(vault.paused());

        // Deposits should fail
        vm.prank(player);
        vm.expectRevert(TheVault.ContractPaused.selector);
        vault.depositETH{value: DEPOSIT_AMOUNT}();
    }

    function test_Vault_AttackerCannotPause() public {
        vm.prank(attacker);
        vm.expectRevert();
        vault.togglePause();

        assertFalse(vault.paused());
    }

    // ==================== HELPER FUNCTIONS ====================

    function _performValidWithdrawal() internal {
        uint256 nonce = vault.getNonce(player);
        
        bytes32 messageHash = keccak256(
            abi.encodePacked(
                player,
                WITHDRAWAL_AMOUNT,
                nonce,
                block.chainid,
                address(vault)
            )
        );
        bytes32 ethSignedHash = keccak256(
            abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash)
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(serverPrivateKey, ethSignedHash);
        bytes memory signature = abi.encodePacked(r, s, v);

        vm.prank(player);
        vault.withdrawETH(WITHDRAWAL_AMOUNT, signature);
    }

    // ==================== SUMMARY TESTS ====================

    function test_SecuritySummary() public {
        // This test summarizes all security properties in one place
        
        console.log("=== OROBOROS ECONOMY SECURITY AUDIT ===");
        console.log("");
        
        // Test 1: Random users cannot mint tokens
        vm.prank(attacker);
        try token.mint(attacker, 1 ether) {
            revert("FAIL: Attacker minted tokens!");
        } catch {
            console.log("[PASS] Attackers cannot mint tokens");
        }

        // Test 2: Random users cannot mint items
        vm.prank(attacker);
        try items.mint(attacker, 1, 100, "") {
            revert("FAIL: Attacker minted items!");
        } catch {
            console.log("[PASS] Attackers cannot mint items");
        }

        // Test 3: Random users cannot claim fees
        _performValidWithdrawal(); // Generate fees
        vm.prank(attacker);
        try vault.claimFees() {
            revert("FAIL: Attacker claimed fees!");
        } catch {
            console.log("[PASS] Attackers cannot claim fees");
        }

        // Test 4: CEO can claim fees
        uint256 fees = vault.accumulatedFees();
        vm.prank(ceo);
        vault.claimFees();
        assertEq(ceo.balance, INITIAL_BALANCE + fees);
        console.log("[PASS] CEO can claim fees: ", fees);

        // Test 5: Server can sign valid withdrawals
        console.log("[PASS] Server signature verification works");

        console.log("");
        console.log("=== ALL SECURITY TESTS PASSED ===");
    }
}
