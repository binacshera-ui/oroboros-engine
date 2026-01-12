//! # Network & Blockchain Constants
//!
//! Production configuration for OROBOROS network and economy.
//!
//! **CRITICAL:** These values are baked into the client binary.
//! Changes require a client rebuild.

use alloy_primitives::{address, Address};

// =============================================================================
// NETWORK CONFIGURATION
// =============================================================================

/// Server public IP address (Hetzner German Datacenter)
pub const SERVER_IP: &str = "162.55.2.222";

/// Server port for game traffic
pub const SERVER_PORT: u16 = 7777;

/// Full server address for client connections
pub const SERVER_ADDR: &str = "162.55.2.222:7777";

/// Server bind address (accepts connections from all interfaces)
pub const SERVER_BIND: &str = "0.0.0.0:7777";

/// Tick rate (updates per second)
pub const TICK_RATE: u32 = 60;

/// Maximum clients per server
pub const MAX_CLIENTS: usize = 500;

/// Maximum packet size (MTU-safe)
pub const MAX_PACKET_SIZE: usize = 1200;

// =============================================================================
// BLOCKCHAIN CONFIGURATION - BASE MAINNET
// =============================================================================

/// Hot Wallet - Server/Deployer
/// 
/// This wallet is used by the game server for:
/// - Signing withdrawal approvals for players
/// - Minting/burning game items (ERC-1155)
/// - Deploying contracts
/// 
/// **SECURITY:** This wallet has SIGNER_ROLE in TheVault and MINTER_ROLE in OrobosItems.
/// It CANNOT withdraw accumulated fees.
pub const HOT_WALLET: Address = address!("13a66f39406b8bea69ad0d8be910e1ecaccf6382");

/// Cold Wallet - CEO/Owner
/// 
/// This wallet is the ultimate authority:
/// - Receives accumulated fees from TheVault
/// - Can pause contracts in emergency
/// - Controls all admin functions (DEFAULT_ADMIN_ROLE)
/// 
/// **SECURITY:** This should be a hardware wallet (Ledger/Trezor).
/// Keep it air-gapped and offline when not in use.
pub const COLD_WALLET: Address = address!("32B34AC473822Bc70A24404de36b8D0F9da06eDc");

/// Base Mainnet Chain ID
pub const BASE_CHAIN_ID: u64 = 8453;

/// Base Mainnet RPC URL (public endpoint)
pub const BASE_RPC_URL: &str = "https://mainnet.base.org";

// =============================================================================
// CONTRACT ADDRESSES (Set after deployment)
// =============================================================================

/// TheVault contract address (set after deployment)
/// 
/// **NOTE:** Update this after running DeployEconomy.s.sol
pub const VAULT_ADDRESS: Option<Address> = None;

/// OrobosToken ($PULSE) contract address (set after deployment)
/// 
/// **NOTE:** Update this after running DeployEconomy.s.sol
pub const TOKEN_ADDRESS: Option<Address> = None;

/// OrobosItems contract address (set after deployment)
/// 
/// **NOTE:** Update this after running DeployEconomy.s.sol
pub const ITEMS_ADDRESS: Option<Address> = None;
