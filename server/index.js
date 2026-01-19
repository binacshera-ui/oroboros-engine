/**
 * ╔══════════════════════════════════════════════════════════════════════════╗
 * ║                 GLITCH WARS - MULTI-ROOM GAME SERVER                     ║
 * ╠══════════════════════════════════════════════════════════════════════════╣
 * ║  Architecture: Multi-Room Sharding with Tier-Based Routing               ║
 * ║  Protocol: WebSocket (JSON)                                              ║
 * ║  Persistence: JSON files (upgradeable to Redis/SQLite)                   ║
 * ╚══════════════════════════════════════════════════════════════════════════╝
 * 
 * ROOM TIERS:
 *   - FREE:    Scavenger Zone     (Stake < $100)     - 1x loot
 *   - PREMIUM: Operator Arena     (Stake $100-999)   - 2.5x loot
 *   - WHALE:   Whale Depths       (Stake >= $1000)   - 5x loot
 * 
 * FLOW:
 *   1. Client connects -> Receives CONNECTED
 *   2. Client sends LOGIN { wallet: '0x...' }
 *   3. Server checks balance -> Routes to appropriate tier room
 *   4. Client receives ROOM_JOINED { tier, mapChanges, ... }
 *   5. Normal gameplay: INPUT, MINE_BLOCK, etc.
 */

const WebSocket = require('ws');
const http = require('http');
const { RoomManager } = require('./src/RoomManager');
const { ROOM_TIER, TIER_CONFIG } = require('./src/GameRoom');

// ============================================================================
// CONFIGURATION
// ============================================================================

const CONFIG = {
    PORT: process.env.PORT || 3000,
    HOST: '0.0.0.0',
};

// ============================================================================
// SERVER INITIALIZATION
// ============================================================================

// Create Room Manager (the brain)
const roomManager = new RoomManager();

// Create HTTP server
const httpServer = http.createServer((req, res) => {
    // Health check
    if (req.url === '/health') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
            status: 'ok',
            ...roomManager.getStatus(),
        }));
        return;
    }
    
    // Room list
    if (req.url === '/rooms') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
            rooms: roomManager.getRoomList(),
        }));
        return;
    }
    
    // Default
    res.writeHead(426, { 'Content-Type': 'text/plain' });
    res.end(`GLITCH WARS Game Server

Connect via WebSocket: ws://${CONFIG.HOST}:${CONFIG.PORT}

Protocol:
  1. Connect to receive CONNECTED message
  2. Send LOGIN { type: 'LOGIN', wallet: '0x...' }
  3. Receive ROOM_JOINED with your tier assignment
  4. Send INPUT/MINE_BLOCK messages to play

Endpoints:
  GET /health - Server status
  GET /rooms  - Room list
`);
});

// Create WebSocket server
const wss = new WebSocket.Server({
    server: httpServer,
    perMessageDeflate: false,
});

// ============================================================================
// CONNECTION HANDLING
// ============================================================================

wss.on('connection', (socket, request) => {
    // Create player and add to pending
    const player = roomManager.handleConnection(socket, request);
    
    // Handle messages
    socket.on('message', async (rawData) => {
        try {
            const message = JSON.parse(rawData.toString());
            await roomManager.routeMessage(player.id, message);
        } catch (err) {
            console.error(`[ERROR] Failed to process message from ${player.shortId}:`, err.message);
        }
    });
    
    // Handle disconnect
    socket.on('close', () => {
        roomManager.handleDisconnection(player.id);
    });
    
    // Handle errors
    socket.on('error', (err) => {
        console.error(`[ERROR] Socket error for ${player.shortId}:`, err.message);
    });
});

// ============================================================================
// SERVER STARTUP
// ============================================================================

httpServer.listen(CONFIG.PORT, CONFIG.HOST, () => {
    console.log(`
╔══════════════════════════════════════════════════════════════════════════════╗
║                                                                              ║
║    ██████╗ ██╗     ██╗████████╗ ██████╗██╗  ██╗    ██╗    ██╗ █████╗ ██████╗ ║
║   ██╔════╝ ██║     ██║╚══██╔══╝██╔════╝██║  ██║    ██║    ██║██╔══██╗██╔══██╗║
║   ██║  ███╗██║     ██║   ██║   ██║     ███████║    ██║ █╗ ██║███████║██████╔╝║
║   ██║   ██║██║     ██║   ██║   ██║     ██╔══██║    ██║███╗██║██╔══██║██╔══██╗║
║   ╚██████╔╝███████╗██║   ██║   ╚██████╗██║  ██║    ╚███╔███╔╝██║  ██║██║  ██║║
║    ╚═════╝ ╚══════╝╚═╝   ╚═╝    ╚═════╝╚═╝  ╚═╝     ╚══╝╚══╝ ╚═╝  ╚═╝╚═╝  ╚═╝║
║                                                                              ║
║                    M U L T I - R O O M   G A M E   S E R V E R               ║
║                                                                              ║
╠══════════════════════════════════════════════════════════════════════════════╣
║                                                                              ║
║  Host: ${CONFIG.HOST.padEnd(15)}  Port: ${String(CONFIG.PORT).padEnd(25)}            ║
║                                                                              ║
║  ROOM TIERS:                                                                 ║
║    ┌─────────────┬─────────────────┬─────────────┬───────────────┐           ║
║    │ TIER        │ NAME            │ STAKE REQ   │ LOOT MULT     │           ║
║    ├─────────────┼─────────────────┼─────────────┼───────────────┤           ║
║    │ FREE        │ Scavenger Zone  │ < $100      │ 1.0x          │           ║
║    │ PREMIUM     │ Operator Arena  │ $100-999    │ 2.5x          │           ║
║    │ WHALE       │ Whale Depths    │ >= $1000    │ 5.0x          │           ║
║    └─────────────┴─────────────────┴─────────────┴───────────────┘           ║
║                                                                              ║
║  FEATURES:                                                                   ║
║    ✓ Multi-Room Sharding (30 TPS per room)                                   ║
║    ✓ Tier-Based Routing (Wallet Balance)                                     ║
║    ✓ Server-Side Map State                                                   ║
║    ✓ Persistent Block Changes (JSON)                                         ║
║    ✓ Economy Validation (Mining Range/Cooldown)                              ║
║    ✓ Anti-Cheat (Speed/Teleport Detection)                                   ║
║                                                                              ║
║  ENDPOINTS:                                                                  ║
║    ws://${CONFIG.HOST}:${CONFIG.PORT}         - Game WebSocket                             ║
║    http://${CONFIG.HOST}:${CONFIG.PORT}/health  - Server Status                            ║
║    http://${CONFIG.HOST}:${CONFIG.PORT}/rooms   - Room List                                ║
║                                                                              ║
╚══════════════════════════════════════════════════════════════════════════════╝
`);
});

// ============================================================================
// MONITORING (Every 30 seconds)
// ============================================================================

setInterval(() => {
    const status = roomManager.getStatus();
    if (status.players.total > 0) {
        console.log(`[STATUS] Players: ${status.players.total} | Rooms: ${status.rooms.total} | Pending: ${status.players.pending}`);
    }
}, 30000);

// ============================================================================
// GRACEFUL SHUTDOWN
// ============================================================================

function shutdown() {
    console.log('\n[SERVER] Initiating graceful shutdown...');
    
    // Save all map data
    roomManager.saveAllMaps();
    
    // Shutdown all rooms
    roomManager.shutdown();
    
    // Close WebSocket server
    wss.close(() => {
        httpServer.close(() => {
            console.log('[SERVER] Shutdown complete. Goodbye!');
            process.exit(0);
        });
    });
    
    // Force exit after 10 seconds
    setTimeout(() => {
        console.log('[SERVER] Force exit');
        process.exit(1);
    }, 10000);
}

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);

// ============================================================================
// ERROR HANDLING
// ============================================================================

process.on('uncaughtException', (err) => {
    console.error('[FATAL] Uncaught exception:', err);
    shutdown();
});

process.on('unhandledRejection', (reason, promise) => {
    console.error('[FATAL] Unhandled rejection at:', promise, 'reason:', reason);
});
