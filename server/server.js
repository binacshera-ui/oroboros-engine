/**
 * GLITCH WARS - Multiplayer WebSocket Server
 * 
 * Features:
 * - Real-time position synchronization (30 updates/sec)
 * - Basic anti-cheat (speed limit validation)
 * - Player session management
 * - Broadcast to all connected clients
 * 
 * Protocol:
 * - Client sends: { type: "position", x, y, z, yaw }
 * - Server broadcasts: { type: "players", players: [...] }
 */

const WebSocket = require('ws');

// ============================================================
// CONFIGURATION
// ============================================================

const PORT = process.env.PORT || 3000;
const TICK_RATE = 30; // Broadcasts per second
const TICK_INTERVAL = 1000 / TICK_RATE;

// Anti-cheat: Maximum allowed speed (units per second)
const MAX_SPEED = 20.0; // Generous limit to allow fast movement
const MAX_TELEPORT_DISTANCE = 50.0; // Max distance per update

// ============================================================
// SERVER STATE
// ============================================================

/** @type {Map<string, PlayerState>} */
const players = new Map();

/** @type {Map<WebSocket, string>} */
const socketToId = new Map();

let nextPlayerId = 1;

/**
 * @typedef {Object} PlayerState
 * @property {string} id - Unique player ID
 * @property {number} x - X position
 * @property {number} y - Y position
 * @property {number} z - Z position
 * @property {number} yaw - Rotation (radians)
 * @property {number} lastUpdate - Timestamp of last update
 * @property {number} balance - Player's balance
 * @property {number} risk - Current risk exposure
 * @property {number} loot - Collected loot count
 */

// ============================================================
// UTILITY FUNCTIONS
// ============================================================

/**
 * Generate a unique player ID
 * @returns {string}
 */
function generatePlayerId() {
    return `player_${nextPlayerId++}_${Date.now().toString(36)}`;
}

/**
 * Calculate distance between two 3D points
 * @param {number} x1 
 * @param {number} y1 
 * @param {number} z1 
 * @param {number} x2 
 * @param {number} y2 
 * @param {number} z2 
 * @returns {number}
 */
function distance3D(x1, y1, z1, x2, y2, z2) {
    const dx = x2 - x1;
    const dy = y2 - y1;
    const dz = z2 - z1;
    return Math.sqrt(dx * dx + dy * dy + dz * dz);
}

/**
 * Validate player movement (anti-cheat)
 * @param {PlayerState} oldState 
 * @param {Object} newPos 
 * @param {number} deltaTime - Time since last update (ms)
 * @returns {boolean}
 */
function validateMovement(oldState, newPos, deltaTime) {
    if (!oldState) return true; // First position update
    
    const dist = distance3D(
        oldState.x, oldState.y, oldState.z,
        newPos.x, newPos.y, newPos.z
    );
    
    // Check for teleport (instant large distance)
    if (dist > MAX_TELEPORT_DISTANCE) {
        console.log(`[ANTI-CHEAT] Teleport detected: ${dist.toFixed(2)} units`);
        return false;
    }
    
    // Check speed (distance over time)
    const deltaSeconds = deltaTime / 1000;
    if (deltaSeconds > 0) {
        const speed = dist / deltaSeconds;
        if (speed > MAX_SPEED) {
            console.log(`[ANTI-CHEAT] Speed hack detected: ${speed.toFixed(2)} units/sec`);
            return false;
        }
    }
    
    return true;
}

/**
 * Create a compact player state for broadcasting
 * @param {PlayerState} player 
 * @returns {Object}
 */
function compactPlayerState(player) {
    return {
        id: player.id,
        x: parseFloat(player.x.toFixed(3)),
        y: parseFloat(player.y.toFixed(3)),
        z: parseFloat(player.z.toFixed(3)),
        yaw: parseFloat(player.yaw.toFixed(3)),
        loot: player.loot
    };
}

// ============================================================
// WEBSOCKET SERVER
// ============================================================

const wss = new WebSocket.Server({ 
    host: '0.0.0.0', // Bind to all interfaces for external access
    port: PORT,
    perMessageDeflate: false // Disable compression for lower latency
});

console.log(`
╔═══════════════════════════════════════════════════════════╗
║                                                           ║
║       ██████╗ ██╗     ██╗████████╗ ██████╗██╗  ██╗        ║
║      ██╔════╝ ██║     ██║╚══██╔══╝██╔════╝██║  ██║        ║
║      ██║  ███╗██║     ██║   ██║   ██║     ███████║        ║
║      ██║   ██║██║     ██║   ██║   ██║     ██╔══██║        ║
║      ╚██████╔╝███████╗██║   ██║   ╚██████╗██║  ██║        ║
║       ╚═════╝ ╚══════╝╚═╝   ╚═╝    ╚═════╝╚═╝  ╚═╝        ║
║                                                           ║
║                    W A R S   S E R V E R                  ║
║                                                           ║
╠═══════════════════════════════════════════════════════════╣
║  Port: ${PORT.toString().padEnd(52)}║
║  Tick Rate: ${TICK_RATE} updates/sec                               ║
║  Anti-Cheat: ENABLED (Max Speed: ${MAX_SPEED} u/s)                ║
╚═══════════════════════════════════════════════════════════╝
`);

// ============================================================
// CONNECTION HANDLER
// ============================================================

wss.on('connection', (ws, req) => {
    const playerId = generatePlayerId();
    const clientIp = req.socket.remoteAddress;
    
    console.log(`[CONNECT] Player ${playerId} connected from ${clientIp}`);
    
    // Initialize player state
    const player = {
        id: playerId,
        x: 0,
        y: 5, // Spawn above floor
        z: 0,
        yaw: 0,
        lastUpdate: Date.now(),
        balance: 0,
        risk: 0,
        loot: 0
    };
    
    players.set(playerId, player);
    socketToId.set(ws, playerId);
    
    // Send welcome message with player ID
    ws.send(JSON.stringify({
        type: 'welcome',
        id: playerId,
        playerCount: players.size
    }));
    
    // Notify all players of new connection
    broadcast({
        type: 'player_joined',
        id: playerId,
        playerCount: players.size
    });
    
    // ============================================================
    // MESSAGE HANDLER
    // ============================================================
    
    ws.on('message', (data) => {
        try {
            const msg = JSON.parse(data.toString());
            const currentPlayer = players.get(playerId);
            
            if (!currentPlayer) return;
            
            switch (msg.type) {
                case 'position': {
                    // Validate movement
                    const now = Date.now();
                    const deltaTime = now - currentPlayer.lastUpdate;
                    
                    const newPos = {
                        x: msg.x || 0,
                        y: msg.y || 0,
                        z: msg.z || 0
                    };
                    
                    if (validateMovement(currentPlayer, newPos, deltaTime)) {
                        // Update position
                        currentPlayer.x = newPos.x;
                        currentPlayer.y = newPos.y;
                        currentPlayer.z = newPos.z;
                        currentPlayer.yaw = msg.yaw || 0;
                        currentPlayer.lastUpdate = now;
                    } else {
                        // Reject invalid position - send correction
                        ws.send(JSON.stringify({
                            type: 'position_rejected',
                            x: currentPlayer.x,
                            y: currentPlayer.y,
                            z: currentPlayer.z,
                            reason: 'anti_cheat'
                        }));
                    }
                    break;
                }
                
                case 'loot_pickup': {
                    // Validate loot pickup (simple increment for now)
                    currentPlayer.loot++;
                    currentPlayer.risk += 50; // Each loot adds risk
                    
                    // Notify all players
                    broadcast({
                        type: 'loot_event',
                        playerId: playerId,
                        lootCount: currentPlayer.loot
                    });
                    break;
                }
                
                case 'chat': {
                    // Broadcast chat message
                    broadcast({
                        type: 'chat',
                        playerId: playerId,
                        message: (msg.message || '').substring(0, 200) // Limit length
                    });
                    break;
                }
                
                case 'ping': {
                    // Respond with pong for latency measurement
                    ws.send(JSON.stringify({
                        type: 'pong',
                        timestamp: msg.timestamp,
                        serverTime: Date.now()
                    }));
                    break;
                }
                
                default:
                    console.log(`[UNKNOWN] Message type: ${msg.type}`);
            }
        } catch (err) {
            console.error(`[ERROR] Failed to parse message from ${playerId}:`, err.message);
        }
    });
    
    // ============================================================
    // DISCONNECT HANDLER
    // ============================================================
    
    ws.on('close', () => {
        console.log(`[DISCONNECT] Player ${playerId} disconnected`);
        
        // Remove player
        players.delete(playerId);
        socketToId.delete(ws);
        
        // Notify remaining players
        broadcast({
            type: 'player_left',
            id: playerId,
            playerCount: players.size
        });
    });
    
    ws.on('error', (err) => {
        console.error(`[ERROR] WebSocket error for ${playerId}:`, err.message);
    });
});

// ============================================================
// BROADCAST FUNCTION
// ============================================================

/**
 * Send message to all connected clients
 * @param {Object} message 
 */
function broadcast(message) {
    const data = JSON.stringify(message);
    wss.clients.forEach((client) => {
        if (client.readyState === WebSocket.OPEN) {
            client.send(data);
        }
    });
}

/**
 * Broadcast message to all clients except one
 * @param {Object} message 
 * @param {string} excludeId 
 */
function broadcastExcept(message, excludeId) {
    const data = JSON.stringify(message);
    wss.clients.forEach((client) => {
        const clientId = socketToId.get(client);
        if (client.readyState === WebSocket.OPEN && clientId !== excludeId) {
            client.send(data);
        }
    });
}

// ============================================================
// GAME TICK - Broadcast player positions
// ============================================================

setInterval(() => {
    if (players.size === 0) return;
    
    // Compile all player positions
    const playerStates = [];
    players.forEach((player) => {
        playerStates.push(compactPlayerState(player));
    });
    
    // Broadcast to all clients
    broadcast({
        type: 'players',
        count: playerStates.length,
        players: playerStates,
        serverTime: Date.now()
    });
    
}, TICK_INTERVAL);

// ============================================================
// SERVER STATS - Log every 30 seconds
// ============================================================

setInterval(() => {
    console.log(`[STATS] Players: ${players.size} | Connections: ${wss.clients.size}`);
}, 30000);

// ============================================================
// GRACEFUL SHUTDOWN
// ============================================================

process.on('SIGINT', () => {
    console.log('\n[SERVER] Shutting down...');
    
    // Notify all clients
    broadcast({
        type: 'server_shutdown',
        message: 'Server is shutting down'
    });
    
    // Close all connections
    wss.close(() => {
        console.log('[SERVER] All connections closed');
        process.exit(0);
    });
    
    // Force exit after 5 seconds
    setTimeout(() => {
        console.log('[SERVER] Force exit');
        process.exit(1);
    }, 5000);
});

console.log(`[SERVER] Ready! Waiting for players...`);
