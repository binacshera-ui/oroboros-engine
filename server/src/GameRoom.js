/**
 * ╔══════════════════════════════════════════════════════════════════════════╗
 * ║                        GLITCH WARS - GAME ROOM                           ║
 * ╠══════════════════════════════════════════════════════════════════════════╣
 * ║  Individual game room with its own physics loop, players, and map state  ║
 * ║  Each room runs at 30 TPS independently                                  ║
 * ╚══════════════════════════════════════════════════════════════════════════╝
 */

const { MapStorage, BLOCK_TYPE, BLOCK_VALUE, MINEABLE_BLOCKS } = require('./MapStorage');

// ============================================================================
// ROOM TIERS CONFIGURATION
// ============================================================================

const ROOM_TIER = {
    FREE: 'FREE',
    PREMIUM: 'PREMIUM',
    WHALE: 'WHALE',
};

const TIER_CONFIG = {
    [ROOM_TIER.FREE]: {
        name: 'Scavenger Zone',
        maxPlayers: 50,
        mapSize: { x: 128, z: 128 },
        lootMultiplier: 1.0,
        goldSpawnRate: 0.01,
        description: 'Entry-level zone. Low risk, low reward.',
    },
    [ROOM_TIER.PREMIUM]: {
        name: 'Operator Arena',
        maxPlayers: 30,
        mapSize: { x: 256, z: 256 },
        lootMultiplier: 2.5,
        goldSpawnRate: 0.03,
        description: 'Mid-tier zone. Better loot, more competition.',
    },
    [ROOM_TIER.WHALE]: {
        name: 'Whale Depths',
        maxPlayers: 20,
        mapSize: { x: 512, z: 512 },
        lootMultiplier: 5.0,
        goldSpawnRate: 0.08,
        description: 'Elite zone. Massive rewards, dangerous terrain.',
    },
};

// ============================================================================
// PHYSICS / VALIDATION CONSTANTS
// ============================================================================

const PHYSICS = {
    TICK_RATE: 30,
    TICK_INTERVAL: 1000 / 30,
    MAX_SPEED: 15.0,
    MAX_TELEPORT: 10.0,
    MINING_RANGE: 5.0,
    MINING_COOLDOWN: 250, // ms between mines
};

// ============================================================================
// GAME ROOM CLASS
// ============================================================================

class GameRoom {
    /**
     * @param {string} roomId - Unique room identifier
     * @param {string} tier - Room tier (FREE, PREMIUM, WHALE)
     */
    constructor(roomId, tier = ROOM_TIER.FREE) {
        this.roomId = roomId;
        this.tier = tier;
        this.config = TIER_CONFIG[tier];
        
        // Players in this room
        this.players = new Map(); // Map<playerId, Player>
        
        // Map state (persistent voxel changes)
        this.mapStorage = new MapStorage(`${roomId}_${tier.toLowerCase()}`);
        
        // Game loop
        this.tickCount = 0;
        this.lastTickTime = Date.now();
        this.isRunning = false;
        this.tickInterval = null;
        
        // Stats
        this.createdAt = Date.now();
        this.totalPlayersJoined = 0;
        
        console.log(`[ROOM] Created ${this.roomId} (${tier}) - ${this.config.name}`);
    }

    // ========================================================================
    // LIFECYCLE
    // ========================================================================

    /**
     * Start the room's game loop
     */
    start() {
        if (this.isRunning) return;
        
        this.isRunning = true;
        this.tickInterval = setInterval(() => this.tick(), PHYSICS.TICK_INTERVAL);
        
        console.log(`[ROOM] ${this.roomId} started at ${PHYSICS.TICK_RATE} TPS`);
    }

    /**
     * Stop the room's game loop
     */
    stop() {
        if (!this.isRunning) return;
        
        this.isRunning = false;
        clearInterval(this.tickInterval);
        this.mapStorage.destroy();
        
        console.log(`[ROOM] ${this.roomId} stopped`);
    }

    /**
     * Check if room should be cleaned up (no players for 5 min)
     */
    shouldCleanup() {
        return this.players.size === 0 && 
               Date.now() - this.lastTickTime > 5 * 60 * 1000;
    }

    // ========================================================================
    // PLAYER MANAGEMENT
    // ========================================================================

    /**
     * Add a player to this room
     * @param {Player} player 
     */
    addPlayer(player) {
        if (this.players.size >= this.config.maxPlayers) {
            return { success: false, reason: 'ROOM_FULL' };
        }
        
        // Set spawn position
        player.x = 0;
        player.y = 6;
        player.z = 0;
        player.roomId = this.roomId;
        
        this.players.set(player.id, player);
        this.totalPlayersJoined++;
        
        // Send room join confirmation
        player.send({
            type: 'ROOM_JOINED',
            roomId: this.roomId,
            tier: this.tier,
            config: {
                name: this.config.name,
                description: this.config.description,
                lootMultiplier: this.config.lootMultiplier,
                mapSize: this.config.mapSize,
            },
            spawn: { x: player.x, y: player.y, z: player.z },
            playerCount: this.players.size,
            mapChanges: this.mapStorage.getChangesForSync(),
        });
        
        // Notify other players
        this.broadcast({
            type: 'PLAYER_JOINED',
            playerId: player.id,
            tier: player.tier,
            playerCount: this.players.size,
        }, player.id);
        
        console.log(`[ROOM] ${player.shortId} joined ${this.roomId} (${this.players.size}/${this.config.maxPlayers})`);
        
        return { success: true };
    }

    /**
     * Remove a player from this room
     * @param {string} playerId 
     */
    removePlayer(playerId) {
        const player = this.players.get(playerId);
        if (!player) return;
        
        this.players.delete(playerId);
        player.roomId = null;
        
        // Notify other players
        this.broadcast({
            type: 'PLAYER_LEFT',
            playerId: playerId,
            playerCount: this.players.size,
        });
        
        console.log(`[ROOM] ${player.shortId} left ${this.roomId} (${this.players.size}/${this.config.maxPlayers})`);
    }

    /**
     * Get a player by ID
     * @param {string} playerId 
     * @returns {Player|undefined}
     */
    getPlayer(playerId) {
        return this.players.get(playerId);
    }

    // ========================================================================
    // GAME LOOP (30 TPS)
    // ========================================================================

    /**
     * Main game tick - runs 30 times per second
     */
    tick() {
        const now = Date.now();
        const deltaTime = (now - this.lastTickTime) / 1000;
        this.lastTickTime = now;
        this.tickCount++;

        // Skip if no players (save CPU)
        if (this.players.size === 0) return;

        // ====================================================================
        // PHASE 1: Process Player Inputs
        // ====================================================================
        
        this.players.forEach((player) => {
            if (!player.inputBuffer) return;
            this.processPlayerInput(player, deltaTime);
        });

        // ====================================================================
        // PHASE 2: Broadcast World State
        // ====================================================================
        
        const playerList = [];
        this.players.forEach((player) => {
            playerList.push(player.serialize());
        });

        const stateMessage = {
            type: 'STATE',
            tick: this.tickCount,
            time: now,
            roomId: this.roomId,
            players: playerList,
        };

        this.broadcast(stateMessage);
    }

    /**
     * Process input from a single player with validation
     * @param {Player} player 
     * @param {number} deltaTime 
     */
    processPlayerInput(player, deltaTime) {
        const input = player.inputBuffer;
        const oldPos = { x: player.x, y: player.y, z: player.z };
        const newPos = { x: input.x, y: input.y, z: input.z };

        // Calculate distance moved
        const dx = newPos.x - oldPos.x;
        const dy = newPos.y - oldPos.y;
        const dz = newPos.z - oldPos.z;
        const dist = Math.sqrt(dx * dx + dy * dy + dz * dz);

        const maxAllowedDist = PHYSICS.MAX_SPEED * deltaTime * 2;

        // Anti-cheat: Check for teleport
        if (dist > PHYSICS.MAX_TELEPORT) {
            player.addViolation('TELEPORT');
            player.send({
                type: 'POSITION_CORRECTION',
                x: player.x,
                y: player.y,
                z: player.z,
                reason: 'TELEPORT_DETECTED',
            });
        }
        // Anti-cheat: Check for speed hack
        else if (dist > maxAllowedDist) {
            const ratio = maxAllowedDist / dist;
            player.updatePosition(
                oldPos.x + dx * ratio,
                oldPos.y + dy * ratio,
                oldPos.z + dz * ratio,
                input.yaw
            );
        }
        // Valid movement
        else {
            player.updatePosition(newPos.x, newPos.y, newPos.z, input.yaw);
        }

        // Clear input buffer
        player.inputBuffer = null;
    }

    // ========================================================================
    // BLOCK MINING (Economy)
    // ========================================================================

    /**
     * Handle block mining request from player
     * @param {Player} player 
     * @param {number} blockX 
     * @param {number} blockY 
     * @param {number} blockZ 
     * @param {number} originalBlockType - Block type from client (validated)
     * @returns {Object} Result
     */
    handleMineBlock(player, blockX, blockY, blockZ, originalBlockType) {
        const now = Date.now();

        // Cooldown check
        if (now - player.lastMineTime < PHYSICS.MINING_COOLDOWN) {
            return { success: false, reason: 'COOLDOWN' };
        }

        // Range check
        if (!player.canReachBlock(blockX, blockY, blockZ)) {
            player.addViolation('MINE_RANGE');
            return { success: false, reason: 'OUT_OF_RANGE' };
        }

        // Check if block is mineable
        if (!MINEABLE_BLOCKS.has(originalBlockType)) {
            return { success: false, reason: 'NOT_MINEABLE' };
        }

        // Mine the block
        const result = this.mapStorage.mineBlock(blockX, blockY, blockZ, originalBlockType);

        if (!result.success) {
            return result;
        }

        // Update player state
        player.lastMineTime = now;
        
        // Calculate reward with tier multiplier
        const reward = Math.floor(result.value * this.config.lootMultiplier);
        const newBalance = player.addEarnings(reward, result.wasGold ? 'GOLD' : 'BLOCK');

        // Notify player
        player.send({
            type: 'MINE_SUCCESS',
            blockX, blockY, blockZ,
            reward,
            newBalance,
            wasGold: result.wasGold,
        });

        // Broadcast to room
        this.broadcast({
            type: 'BLOCK_MINED',
            playerId: player.id,
            blockX, blockY, blockZ,
            newType: BLOCK_TYPE.AIR,
        }, player.id);

        return { success: true, reward, newBalance };
    }

    // ========================================================================
    // NETWORKING
    // ========================================================================

    /**
     * Broadcast message to all players in room
     * @param {Object} message 
     * @param {string} excludeId - Player ID to exclude
     */
    broadcast(message, excludeId = null) {
        const data = JSON.stringify(message);
        
        this.players.forEach((player, id) => {
            if (id !== excludeId && player.isConnected) {
                player.socket.send(data);
            }
        });
    }

    /**
     * Send message to specific player
     * @param {string} playerId 
     * @param {Object} message 
     */
    sendToPlayer(playerId, message) {
        const player = this.players.get(playerId);
        if (player) {
            player.send(message);
        }
    }

    // ========================================================================
    // UTILITIES
    // ========================================================================

    /**
     * Get room status for monitoring
     */
    getStatus() {
        return {
            roomId: this.roomId,
            tier: this.tier,
            name: this.config.name,
            players: this.players.size,
            maxPlayers: this.config.maxPlayers,
            tickCount: this.tickCount,
            isRunning: this.isRunning,
            mapChanges: this.mapStorage.blockChanges.size,
            stats: {
                totalPlayersJoined: this.totalPlayersJoined,
                totalBlocksMined: this.mapStorage.totalBlocksMined,
                totalGoldMined: this.mapStorage.totalGoldMined,
            },
        };
    }
}

// ============================================================================
// EXPORTS
// ============================================================================

module.exports = {
    GameRoom,
    ROOM_TIER,
    TIER_CONFIG,
    PHYSICS,
};
