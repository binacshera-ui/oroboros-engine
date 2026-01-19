/**
 * ╔══════════════════════════════════════════════════════════════════════════╗
 * ║                      GLITCH WARS - ROOM MANAGER                          ║
 * ╠══════════════════════════════════════════════════════════════════════════╣
 * ║  The "Gatekeeper" - Handles authentication, tier routing, room sharding  ║
 * ╚══════════════════════════════════════════════════════════════════════════╝
 */

const { GameRoom, ROOM_TIER, TIER_CONFIG } = require('./GameRoom');
const { Player, PLAYER_TIER, TIER_THRESHOLDS } = require('./Player');

// ============================================================================
// WALLET BALANCE MOCK (Replace with actual Web3 integration)
// ============================================================================

/**
 * Mock function to get wallet staking balance
 * In production, this would query a blockchain or database
 * @param {string} wallet - Wallet address
 * @returns {Promise<number>} - Staked balance
 */
async function getWalletBalance(wallet) {
    // ========================================================================
    // QA TEST: Hardcoded balance based on wallet prefix
    // ========================================================================
    let mockBalance = 50; // Default: poor (FREE tier)
    
    if (wallet.toLowerCase().startsWith('0xwhale')) {
        mockBalance = 5000; // Rich → WHALE tier
        console.log(`[WALLET] 🐋 WHALE DETECTED: ${wallet.slice(0, 12)}... → $${mockBalance}`);
    } else if (wallet.toLowerCase().startsWith('0xnoob')) {
        mockBalance = 50; // Poor → FREE tier
        console.log(`[WALLET] 🆓 NOOB DETECTED: ${wallet.slice(0, 12)}... → $${mockBalance}`);
    } else {
        // Default: derive from wallet address
        const lastChars = wallet.slice(-4);
        mockBalance = parseInt(lastChars, 16) % 2000;
        console.log(`[WALLET] Mock balance for ${wallet.slice(0, 10)}...: $${mockBalance}`);
    }
    
    return mockBalance;
}

/**
 * Determine which room tier a player should join based on staked balance
 * @param {number} stakedBalance 
 * @returns {string} Room tier
 */
function determineRoomTier(stakedBalance) {
    if (stakedBalance >= TIER_THRESHOLDS.WHALE) {
        return ROOM_TIER.WHALE;
    } else if (stakedBalance >= TIER_THRESHOLDS.OPERATOR) {
        return ROOM_TIER.PREMIUM;
    }
    return ROOM_TIER.FREE;
}

// ============================================================================
// ROOM MANAGER CLASS
// ============================================================================

class RoomManager {
    constructor() {
        // Rooms organized by tier
        this.rooms = {
            [ROOM_TIER.FREE]: new Map(),
            [ROOM_TIER.PREMIUM]: new Map(),
            [ROOM_TIER.WHALE]: new Map(),
        };
        
        // All connected players (before room assignment)
        this.pendingPlayers = new Map();
        
        // Player to room mapping
        this.playerRooms = new Map(); // Map<playerId, roomId>
        
        // Stats
        this.totalConnections = 0;
        this.totalAuthentications = 0;
        
        // Create default rooms for each tier
        this.initializeDefaultRooms();
        
        // Cleanup timer (every 5 minutes)
        this.cleanupInterval = setInterval(() => this.cleanupEmptyRooms(), 5 * 60 * 1000);
    }

    // ========================================================================
    // INITIALIZATION
    // ========================================================================

    /**
     * Create default rooms for each tier
     */
    initializeDefaultRooms() {
        // Create one room per tier initially
        this.createRoom(ROOM_TIER.FREE, 'free-main');
        this.createRoom(ROOM_TIER.PREMIUM, 'premium-main');
        this.createRoom(ROOM_TIER.WHALE, 'whale-main');
        
        console.log('[ROOMS] Default rooms initialized');
    }

    /**
     * Create a new game room
     * @param {string} tier 
     * @param {string} roomId 
     * @returns {GameRoom}
     */
    createRoom(tier, roomId = null) {
        const id = roomId || `${tier.toLowerCase()}-${Date.now()}`;
        const room = new GameRoom(id, tier);
        
        this.rooms[tier].set(id, room);
        room.start();
        
        return room;
    }

    // ========================================================================
    // CONNECTION HANDLING
    // ========================================================================

    /**
     * Handle new WebSocket connection (pre-authentication)
     * @param {WebSocket} socket 
     * @param {Object} request - HTTP request object
     * @returns {Player}
     */
    handleConnection(socket, request) {
        const player = new Player(socket, request.socket.remoteAddress);
        
        this.pendingPlayers.set(player.id, player);
        this.totalConnections++;
        
        // Send welcome message (requires LOGIN to proceed)
        player.send({
            type: 'CONNECTED',
            message: 'Connected to GLITCH WARS. Send LOGIN to authenticate.',
            serverId: 'glitch-wars-v1',
            tiers: Object.keys(ROOM_TIER),
        });
        
        console.log(`[CONNECT] ${player.shortId} connected (pending auth)`);
        
        return player;
    }

    /**
     * Handle player disconnection
     * @param {string} playerId 
     */
    handleDisconnection(playerId) {
        // Remove from pending
        const pending = this.pendingPlayers.get(playerId);
        if (pending) {
            this.pendingPlayers.delete(playerId);
            console.log(`[DISCONNECT] ${pending.shortId} disconnected (was pending)`);
            return;
        }
        
        // Remove from room
        const roomId = this.playerRooms.get(playerId);
        if (roomId) {
            const room = this.findRoom(roomId);
            if (room) {
                room.removePlayer(playerId);
            }
            this.playerRooms.delete(playerId);
        }
    }

    // ========================================================================
    // AUTHENTICATION & ROUTING (The Gatekeeper)
    // ========================================================================

    /**
     * Handle LOGIN message - authenticate and route to appropriate room
     * @param {string} playerId 
     * @param {string} wallet - Wallet address
     * @returns {Promise<Object>} Result
     */
    async handleLogin(playerId, wallet) {
        const player = this.pendingPlayers.get(playerId);
        
        if (!player) {
            return { success: false, reason: 'PLAYER_NOT_FOUND' };
        }
        
        // Validate wallet format
        if (!wallet || !wallet.startsWith('0x') || wallet.length !== 42) {
            player.send({
                type: 'LOGIN_FAILED',
                reason: 'INVALID_WALLET',
            });
            return { success: false, reason: 'INVALID_WALLET' };
        }
        
        try {
            // Get staked balance (mock or real)
            const stakedBalance = await getWalletBalance(wallet);
            
            // Authenticate player
            const authResult = player.authenticate(wallet, stakedBalance);
            
            // Determine target tier
            const targetTier = determineRoomTier(stakedBalance);
            
            // Find or create room
            const room = this.findAvailableRoom(targetTier);
            
            // Move from pending to room
            this.pendingPlayers.delete(playerId);
            
            // Add to room
            const joinResult = room.addPlayer(player);
            
            if (!joinResult.success) {
                player.send({
                    type: 'LOGIN_FAILED',
                    reason: joinResult.reason,
                });
                return joinResult;
            }
            
            // Track player-room mapping
            this.playerRooms.set(playerId, room.roomId);
            this.totalAuthentications++;
            
            // ========================================================================
            // QA LOG: Clear assignment log
            // ========================================================================
            console.log(`╔══════════════════════════════════════════════════════════════════╗`);
            console.log(`║  PLAYER ASSIGNED                                                 ║`);
            console.log(`╠══════════════════════════════════════════════════════════════════╣`);
            console.log(`║  UUID:    ${player.id}                          ║`);
            console.log(`║  Wallet:  ${wallet.slice(0, 20)}...                          ║`);
            console.log(`║  Balance: $${stakedBalance.toString().padEnd(10)}                                       ║`);
            console.log(`║  TIER:    ${targetTier.padEnd(10)}                                       ║`);
            console.log(`║  ROOM:    ${room.roomId.padEnd(20)}                         ║`);
            console.log(`╚══════════════════════════════════════════════════════════════════╝`);
            
            return { 
                success: true, 
                playerId: player.id,
                tier: targetTier,
                roomId: room.roomId,
            };
            
        } catch (err) {
            console.error(`[AUTH] Error authenticating ${playerId}:`, err);
            player.send({
                type: 'LOGIN_FAILED',
                reason: 'SERVER_ERROR',
            });
            return { success: false, reason: 'SERVER_ERROR' };
        }
    }

    /**
     * Find an available room for a tier (or create one if needed)
     * @param {string} tier 
     * @returns {GameRoom}
     */
    findAvailableRoom(tier) {
        const tierRooms = this.rooms[tier];
        
        // Find room with space
        for (const room of tierRooms.values()) {
            if (room.players.size < room.config.maxPlayers) {
                return room;
            }
        }
        
        // All rooms full - create new one
        console.log(`[ROOMS] All ${tier} rooms full, creating new one`);
        return this.createRoom(tier);
    }

    /**
     * Find a room by ID (across all tiers)
     * @param {string} roomId 
     * @returns {GameRoom|null}
     */
    findRoom(roomId) {
        for (const tierRooms of Object.values(this.rooms)) {
            if (tierRooms.has(roomId)) {
                return tierRooms.get(roomId);
            }
        }
        return null;
    }

    /**
     * Get room for a player
     * @param {string} playerId 
     * @returns {GameRoom|null}
     */
    getPlayerRoom(playerId) {
        const roomId = this.playerRooms.get(playerId);
        return roomId ? this.findRoom(roomId) : null;
    }

    // ========================================================================
    // MESSAGE ROUTING
    // ========================================================================

    /**
     * Route a message to the appropriate handler
     * @param {string} playerId 
     * @param {Object} message 
     */
    async routeMessage(playerId, message) {
        // Handle LOGIN (pre-room)
        if (message.type === 'LOGIN') {
            return this.handleLogin(playerId, message.wallet);
        }
        
        // All other messages require room assignment
        const room = this.getPlayerRoom(playerId);
        if (!room) {
            // Player not in room yet - might be pending
            const pending = this.pendingPlayers.get(playerId);
            if (pending) {
                pending.send({
                    type: 'ERROR',
                    message: 'Must LOGIN first',
                });
            }
            return { success: false, reason: 'NOT_IN_ROOM' };
        }
        
        const player = room.getPlayer(playerId);
        if (!player) {
            return { success: false, reason: 'PLAYER_NOT_FOUND' };
        }
        
        // Route based on message type
        switch (message.type) {
            case 'INPUT':
                player.inputBuffer = {
                    x: message.x,
                    y: message.y,
                    z: message.z,
                    yaw: message.yaw,
                    timestamp: Date.now(),
                };
                return { success: true };
                
            case 'MINE_BLOCK':
                return room.handleMineBlock(
                    player,
                    message.blockX,
                    message.blockY,
                    message.blockZ,
                    message.blockType || 0
                );
                
            case 'PING':
                player.send({
                    type: 'PONG',
                    clientTime: message.timestamp,
                    serverTime: Date.now(),
                    tick: room.tickCount,
                });
                return { success: true };
                
            case 'CHAT':
                // Broadcast chat to room
                room.broadcast({
                    type: 'CHAT',
                    playerId: player.id,
                    message: (message.text || '').slice(0, 200), // Limit length
                    tier: player.tier,
                });
                return { success: true };
                
            default:
                console.warn(`[MSG] Unknown message type: ${message.type}`);
                return { success: false, reason: 'UNKNOWN_TYPE' };
        }
    }

    // ========================================================================
    // MAINTENANCE
    // ========================================================================

    /**
     * Clean up empty rooms (except default ones)
     */
    cleanupEmptyRooms() {
        const defaultRooms = ['free-main', 'premium-main', 'whale-main'];
        let cleaned = 0;
        
        for (const [tier, tierRooms] of Object.entries(this.rooms)) {
            for (const [roomId, room] of tierRooms) {
                if (!defaultRooms.includes(roomId) && room.shouldCleanup()) {
                    room.stop();
                    tierRooms.delete(roomId);
                    cleaned++;
                    console.log(`[CLEANUP] Removed empty room ${roomId}`);
                }
            }
        }
        
        if (cleaned > 0) {
            console.log(`[CLEANUP] Cleaned ${cleaned} empty rooms`);
        }
    }

    /**
     * Force save all room map states
     */
    saveAllMaps() {
        for (const tierRooms of Object.values(this.rooms)) {
            for (const room of tierRooms.values()) {
                room.mapStorage.save();
            }
        }
    }

    /**
     * Shutdown all rooms gracefully
     */
    shutdown() {
        console.log('[ROOMS] Shutting down all rooms...');
        
        clearInterval(this.cleanupInterval);
        
        // Notify all players
        for (const tierRooms of Object.values(this.rooms)) {
            for (const room of tierRooms.values()) {
                room.broadcast({
                    type: 'SERVER_SHUTDOWN',
                    message: 'Server is restarting',
                });
                room.stop();
            }
        }
        
        console.log('[ROOMS] All rooms stopped');
    }

    // ========================================================================
    // MONITORING
    // ========================================================================

    /**
     * Get overall server status
     */
    getStatus() {
        const roomStats = {};
        let totalPlayers = 0;
        let totalRooms = 0;
        
        for (const [tier, tierRooms] of Object.entries(this.rooms)) {
            roomStats[tier] = {
                rooms: tierRooms.size,
                players: 0,
            };
            
            for (const room of tierRooms.values()) {
                roomStats[tier].players += room.players.size;
                totalPlayers += room.players.size;
                totalRooms++;
            }
        }
        
        return {
            server: {
                uptime: process.uptime(),
                totalConnections: this.totalConnections,
                totalAuthentications: this.totalAuthentications,
            },
            rooms: {
                total: totalRooms,
                byTier: roomStats,
            },
            players: {
                total: totalPlayers,
                pending: this.pendingPlayers.size,
            },
        };
    }

    /**
     * Get detailed room list
     */
    getRoomList() {
        const list = [];
        
        for (const tierRooms of Object.values(this.rooms)) {
            for (const room of tierRooms.values()) {
                list.push(room.getStatus());
            }
        }
        
        return list;
    }
}

// ============================================================================
// EXPORTS
// ============================================================================

module.exports = {
    RoomManager,
    getWalletBalance,
    determineRoomTier,
};
