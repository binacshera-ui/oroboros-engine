// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "../src/OrobosToken.sol";
import "../src/OrobosItems.sol";
import "../src/TheVault.sol";

/**
 * @title DeployEconomy
 * @author OROBOROS Core Kernel Team
 * @notice Production deployment script for Base Mainnet.
 * @dev Run with: forge script script/DeployEconomy.s.sol --rpc-url base --broadcast
 * 
 * WALLET CONFIGURATION:
 * =====================
 * HOT_WALLET: 0x13a66f39406b8bea69ad0d8be910e1ecaccf6382
 *   - Deployer address (executes this script)
 *   - Game Server's signing wallet
 *   - Granted SIGNER_ROLE in TheVault
 *   - Granted MINTER_ROLE in OrobosItems
 * 
 * COLD_WALLET: 0x32B34AC473822Bc70A24404de36b8D0F9da06eDc
 *   - CEO's hardware wallet (Ledger/Trezor)
 *   - Receives DEFAULT_ADMIN_ROLE in all contracts
 *   - ONLY address that can withdraw accumulated fees
 *   - Can pause contracts in emergency
 * 
 * SECURITY INVARIANTS:
 * ====================
 * 1. HOT_WALLET cannot withdraw fees (only sign player withdrawals)
 * 2. COLD_WALLET controls all admin functions
 * 3. Token MINTER_ROLE is granted ONLY to TheVault (not HOT_WALLET)
 * 4. Items MINTER_ROLE is granted to HOT_WALLET (server mints items)
 */
contract DeployEconomy is Script {
    // ============================================
    // PRODUCTION WALLET ADDRESSES - BASE MAINNET
    // ============================================
    
    /// @notice Server's hot wallet - deploys contracts, signs withdrawals
    address public constant HOT_WALLET = 0x13A66F39406b8Bea69AD0D8be910E1EcaCCF6382;
    
    /// @notice CEO's cold wallet - receives fees, admin control
    address public constant COLD_WALLET = 0x32B34AC473822Bc70A24404de36b8D0F9da06eDc;
    
    /// @notice Base URI for item metadata
    string public constant ITEMS_BASE_URI = "https://api.oroboros.game/items/";

    // Deployed contract addresses (set after deployment)
    OrobosToken public token;
    OrobosItems public items;
    TheVault public vault;

    function run() external {
        // Verify we're deploying from HOT_WALLET
        console.log("==============================================");
        console.log("  OROBOROS ECONOMY DEPLOYMENT - BASE MAINNET  ");
        console.log("==============================================");
        console.log("");
        console.log("Deployer (HOT_WALLET):", HOT_WALLET);
        console.log("Admin (COLD_WALLET):  ", COLD_WALLET);
        console.log("");

        // Start broadcasting transactions from HOT_WALLET
        vm.startBroadcast();

        // ============================================
        // STEP 1: Deploy TheVault (The Heart)
        // ============================================
        console.log("[1/5] Deploying TheVault...");
        vault = new TheVault(COLD_WALLET, HOT_WALLET);
        console.log("      TheVault deployed at:", address(vault));
        console.log("      - CEO_ROLE granted to:", COLD_WALLET);
        console.log("      - SIGNER_ROLE granted to:", HOT_WALLET);

        // ============================================
        // STEP 2: Deploy OrobosToken ($PULSE)
        // ============================================
        console.log("[2/5] Deploying OrobosToken ($PULSE)...");
        token = new OrobosToken(COLD_WALLET);
        console.log("      OrobosToken deployed at:", address(token));
        console.log("      - DEFAULT_ADMIN_ROLE granted to:", COLD_WALLET);

        // ============================================
        // STEP 3: Deploy OrobosItems (Game Assets)
        // ============================================
        console.log("[3/5] Deploying OrobosItems...");
        items = new OrobosItems(COLD_WALLET, ITEMS_BASE_URI);
        console.log("      OrobosItems deployed at:", address(items));
        console.log("      - DEFAULT_ADMIN_ROLE granted to:", COLD_WALLET);
        console.log("      - Metadata URI:", ITEMS_BASE_URI);

        // ============================================
        // STEP 4: Configure Roles (CRITICAL!)
        // ============================================
        console.log("[4/5] Configuring roles...");
        
        // NOTE: The deployer (HOT_WALLET) does NOT have admin rights on token/items
        // because we passed COLD_WALLET as admin. The COLD_WALLET must grant roles.
        // 
        // For a production deployment, the COLD_WALLET owner must:
        // 1. Call token.grantRole(MINTER_ROLE, address(vault))
        // 2. Call items.grantRole(MINTER_ROLE, HOT_WALLET)
        //
        // This script cannot do this because HOT_WALLET is not the admin.
        
        console.log("      [!] MANUAL ACTION REQUIRED:");
        console.log("          COLD_WALLET must execute from Ledger/Trezor:");
        console.log("          1. token.grantRole(MINTER_ROLE, vault_address)");
        console.log("          2. items.grantRole(MINTER_ROLE, HOT_WALLET)");

        vm.stopBroadcast();

        // ============================================
        // STEP 5: Verification Summary
        // ============================================
        console.log("");
        console.log("[5/5] DEPLOYMENT COMPLETE");
        console.log("==============================================");
        console.log("CONTRACT ADDRESSES (Save these!):");
        console.log("==============================================");
        console.log("TheVault:     ", address(vault));
        console.log("OrobosToken:  ", address(token));
        console.log("OrobosItems:  ", address(items));
        console.log("");
        console.log("VERIFICATION COMMANDS:");
        console.log("==============================================");
        console.log("forge verify-contract", address(vault), "TheVault --chain base");
        console.log("forge verify-contract", address(token), "OrobosToken --chain base");
        console.log("forge verify-contract", address(items), "OrobosItems --chain base");
        console.log("");
        console.log("[!] REMINDER: COLD_WALLET must grant roles!");
    }

    /**
     * @notice Verify deployment configuration before broadcasting.
     * @dev Run with: forge script script/DeployEconomy.s.sol --sig "verify()"
     */
    function verify() external pure {
        console.log("==============================================");
        console.log("  DEPLOYMENT VERIFICATION                     ");
        console.log("==============================================");
        console.log("");
        console.log("HOT_WALLET (Server):", HOT_WALLET);
        console.log("COLD_WALLET (CEO):  ", COLD_WALLET);
        console.log("");
        
        // Sanity checks
        require(HOT_WALLET != address(0), "HOT_WALLET is zero!");
        require(COLD_WALLET != address(0), "COLD_WALLET is zero!");
        require(HOT_WALLET != COLD_WALLET, "Wallets must be different!");
        
        console.log("[OK] All addresses are valid and different.");
        console.log("[OK] Ready for deployment.");
    }
}
