/**
 * ╔══════════════════════════════════════════════════════════════════════════╗
 * ║                         GLITCH WARS - PLAYER CLASS                       ║
 * ╠══════════════════════════════════════════════════════════════════════════╣
 * ║  Represents a connected player with wallet, balance, and game state      ║
 * ╚══════════════════════════════════════════════════════════════════════════╝
 */

const { v4: uuidv4 } = require('uuid');

// ============================================================================
// PLAYER TIERS (Based on Staking Level)
// ============================================================================

const PLAYER_TIER = {
    SCAVENGER: 'SCAVENGER',    // < $100 staked
    OPERATOR: 'OPERATOR',       // $100-$999 staked
    WHALE: 'WHALE',             // >= $1000 staked
};

// Tier thresholds
const TIER_THRESHOLDS = {
    OPERATOR: 100,
    WHALE: 1000,
};

// ============================================================================
// PLAYER CLASS
// ============================================================================

class Player {
    /**
     * @param {WebSocket} socket - WebSocket connection
     * @param {string} remoteAddress - Client IP
     */
    constructor(socket, remoteAddress) {
        // Identity
        this.id = uuidv4();
        this.socket = socket;
        this.remoteAddress = remoteAddress;
        
        // Authentication
        this.wallet = null;
        this.isAuthenticated = false;
        this.tier = PLAYER_TIER.SCAVENGER;
        
        // Economy
        this.stakedBalance = 0;      // External staking (determines tier)
        this.gameBalance = 0;        // In-game earnings
        this.totalLoot = 0;          // Total blocks mined
        
        // Game State
        this.x = 0;
        this.y = 6;
        this.z = 0;
        this.yaw = 0;
        this.roomId = null;
        
        // Stats
        this.lastUpdate = Date.now();
        this.lastMineTime = 0;
        this.inputBuffer = null;
        
        // Anti-cheat
        this.violations = 0;
        this.lastViolationTime = 0;
    }

    // ========================================================================
    // AUTHENTICATION
    // ========================================================================

    /**
     * Authenticate player with wallet address
     * @param {string} wallet - Wallet address (0x...)
     * @param {number} stakedBalance - Amount staked (determines tier)
     */
    authenticate(wallet, stakedBalance) {
        this.wallet = wallet;
        this.stakedBalance = stakedBalance;
        this.isAuthenticated = true;
        this.tier = this.calculateTier(stakedBalance);
        
        console.log(`[PLAYER] ${this.shortId} authenticated as ${this.tier} (Staked: $${stakedBalance})`);
        
        return {
            id: this.id,
            wallet: this.wallet,
            tier: this.tier,
            stakedBalance: this.stakedBalance,
        };
    }

    /**
     * Calculate player tier based on staked balance
     * @param {number} balance - Staked balance
     * @returns {string} Tier name
     */
    calculateTier(balance) {
        if (balance >= TIER_THRESHOLDS.WHALE) {
            return PLAYER_TIER.WHALE;
        } else if (balance >= TIER_THRESHOLDS.OPERATOR) {
            return PLAYER_TIER.OPERATOR;
        }
        return PLAYER_TIER.SCAVENGER;
    }

    // ========================================================================
    // POSITION & MOVEMENT
    // ========================================================================

    /**
     * Update player position (validated by GameRoom)
     * @param {number} x 
     * @param {number} y 
     * @param {number} z 
     * @param {number} yaw 
     */
    updatePosition(x, y, z, yaw) {
        this.x = x;
        this.y = y;
        this.z = z;
        this.yaw = yaw;
        this.lastUpdate = Date.now();
    }

    /**
     * Get distance to a point in 3D space
     * @param {number} x 
     * @param {number} y 
     * @param {number} z 
     * @returns {number} Distance
     */
    distanceTo(x, y, z) {
        const dx = this.x - x;
        const dy = this.y - y;
        const dz = this.z - z;
        return Math.sqrt(dx * dx + dy * dy + dz * dz);
    }

    /**
     * Check if player is within mining range of a block
     * @param {number} blockX 
     * @param {number} blockY 
     * @param {number} blockZ 
     * @returns {boolean}
     */
    canReachBlock(blockX, blockY, blockZ) {
        const MINING_RANGE = 5.0; // Blocks
        return this.distanceTo(blockX, blockY, blockZ) <= MINING_RANGE;
    }

    // ========================================================================
    // ECONOMY
    // ========================================================================

    /**
     * Add earnings to player's game balance
     * @param {number} amount - Amount to add
     * @param {string} reason - Reason for earning
     */
    addEarnings(amount, reason) {
        this.gameBalance += amount;
        this.totalLoot++;
        
        console.log(`[ECONOMY] ${this.shortId} earned $${amount} (${reason}) | Total: $${this.gameBalance}`);
        
        return this.gameBalance;
    }

    // ========================================================================
    // ANTI-CHEAT
    // ========================================================================

    /**
     * Record a violation (speed hack, teleport, etc.)
     * @param {string} type - Violation type
     */
    addViolation(type) {
        const now = Date.now();
        
        // Reset violations if >30 seconds since last
        if (now - this.lastViolationTime > 30000) {
            this.violations = 0;
        }
        
        this.violations++;
        this.lastViolationTime = now;
        
        console.warn(`[ANTI-CHEAT] ${this.shortId} violation: ${type} (${this.violations}/5)`);
        
        // Kick after 5 violations
        if (this.violations >= 5) {
            this.kick('Too many violations');
            return true;
        }
        
        return false;
    }

    /**
     * Kick player from server
     * @param {string} reason 
     */
    kick(reason) {
        console.warn(`[KICK] ${this.shortId} kicked: ${reason}`);
        
        this.send({
            type: 'KICKED',
            reason: reason,
        });
        
        this.socket.close(1000, reason);
    }

    // ========================================================================
    // NETWORKING
    // ========================================================================

    /**
     * Send JSON message to player
     * @param {Object} message 
     */
    send(message) {
        if (this.socket.readyState === 1) { // WebSocket.OPEN
            this.socket.send(JSON.stringify(message));
        }
    }

    /**
     * Serialize player for network transmission
     * @returns {Object}
     */
    serialize() {
        return {
            id: this.id,
            x: Math.round(this.x * 1000) / 1000,
            y: Math.round(this.y * 1000) / 1000,
            z: Math.round(this.z * 1000) / 1000,
            yaw: Math.round(this.yaw * 1000) / 1000,
            tier: this.tier,
            gameBalance: this.gameBalance,
        };
    }

    // ========================================================================
    // UTILITIES
    // ========================================================================

    /**
     * Short ID for logging
     */
    get shortId() {
        return this.id.slice(0, 8);
    }

    /**
     * Is the socket still connected?
     */
    get isConnected() {
        return this.socket.readyState === 1;
    }
}

// ============================================================================
// EXPORTS
// ============================================================================

module.exports = {
    Player,
    PLAYER_TIER,
    TIER_THRESHOLDS,
};
