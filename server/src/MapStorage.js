/**
 * ╔══════════════════════════════════════════════════════════════════════════╗
 * ║                      GLITCH WARS - MAP STORAGE                           ║
 * ╠══════════════════════════════════════════════════════════════════════════╣
 * ║  Server-side voxel map state with persistence                            ║
 * ║  Stores block changes and saves to disk periodically                     ║
 * ╚══════════════════════════════════════════════════════════════════════════╝
 */

const fs = require('fs');
const path = require('path');

// ============================================================================
// BLOCK TYPES (Must match Rust definitions)
// ============================================================================

const BLOCK_TYPE = {
    AIR: 0,
    CONCRETE_FLOOR: 1,
    CONCRETE_WALL: 2,
    HAZARD_NEON: 3,
    EXTRACTION_BEAM: 4,
    BEDROCK: 5,
    GOLD_LOOT: 6,
    METAL_BRIDGE: 7,
};

// Block values (for economy)
const BLOCK_VALUE = {
    [BLOCK_TYPE.GOLD_LOOT]: 50,
    [BLOCK_TYPE.CONCRETE_FLOOR]: 1,
    [BLOCK_TYPE.CONCRETE_WALL]: 2,
    [BLOCK_TYPE.METAL_BRIDGE]: 5,
};

// Mineable blocks
const MINEABLE_BLOCKS = new Set([
    BLOCK_TYPE.GOLD_LOOT,
    BLOCK_TYPE.CONCRETE_FLOOR,
    BLOCK_TYPE.CONCRETE_WALL,
    BLOCK_TYPE.METAL_BRIDGE,
]);

// ============================================================================
// MAP STORAGE CLASS
// ============================================================================

class MapStorage {
    /**
     * @param {string} mapId - Unique map identifier
     * @param {Object} options - Configuration options
     */
    constructor(mapId, options = {}) {
        this.mapId = mapId;
        this.dataDir = options.dataDir || path.join(__dirname, '..', 'data');
        this.saveInterval = options.saveInterval || 10000; // 10 seconds
        
        // Block changes: Map<"x,y,z", blockId>
        // Only stores CHANGED blocks, not the full procedural map
        this.blockChanges = new Map();
        
        // Loot spawns: Map<"x,y,z", {type, spawnTime, value}>
        this.lootSpawns = new Map();
        
        // Stats
        this.totalBlocksMined = 0;
        this.totalGoldMined = 0;
        this.lastSaveTime = Date.now();
        this.isDirty = false;
        
        // Ensure data directory exists
        if (!fs.existsSync(this.dataDir)) {
            fs.mkdirSync(this.dataDir, { recursive: true });
        }
        
        // Load existing state
        this.load();
        
        // Start auto-save timer
        this.saveTimer = setInterval(() => this.save(), this.saveInterval);
    }

    // ========================================================================
    // BLOCK OPERATIONS
    // ========================================================================

    /**
     * Get block key from coordinates
     * @param {number} x 
     * @param {number} y 
     * @param {number} z 
     * @returns {string}
     */
    static blockKey(x, y, z) {
        return `${Math.floor(x)},${Math.floor(y)},${Math.floor(z)}`;
    }

    /**
     * Parse block key to coordinates
     * @param {string} key 
     * @returns {{x: number, y: number, z: number}}
     */
    static parseKey(key) {
        const [x, y, z] = key.split(',').map(Number);
        return { x, y, z };
    }

    /**
     * Check if a block has been modified from its original state
     * @param {number} x 
     * @param {number} y 
     * @param {number} z 
     * @returns {boolean}
     */
    isModified(x, y, z) {
        return this.blockChanges.has(MapStorage.blockKey(x, y, z));
    }

    /**
     * Get current block type at position
     * If modified, returns the new type; otherwise null (use procedural)
     * @param {number} x 
     * @param {number} y 
     * @param {number} z 
     * @returns {number|null}
     */
    getBlock(x, y, z) {
        const key = MapStorage.blockKey(x, y, z);
        return this.blockChanges.has(key) ? this.blockChanges.get(key) : null;
    }

    /**
     * Set block type at position
     * @param {number} x 
     * @param {number} y 
     * @param {number} z 
     * @param {number} blockType 
     */
    setBlock(x, y, z, blockType) {
        const key = MapStorage.blockKey(x, y, z);
        this.blockChanges.set(key, blockType);
        this.isDirty = true;
    }

    /**
     * Remove a block (set to AIR)
     * @param {number} x 
     * @param {number} y 
     * @param {number} z 
     * @returns {{success: boolean, value: number, wasGold: boolean}}
     */
    mineBlock(x, y, z, originalBlockType) {
        const key = MapStorage.blockKey(x, y, z);
        
        // Check if block was already mined
        if (this.blockChanges.has(key) && this.blockChanges.get(key) === BLOCK_TYPE.AIR) {
            return { success: false, value: 0, wasGold: false, reason: 'ALREADY_MINED' };
        }
        
        // Check if block is mineable
        if (!MINEABLE_BLOCKS.has(originalBlockType)) {
            return { success: false, value: 0, wasGold: false, reason: 'NOT_MINEABLE' };
        }
        
        // Mine the block
        this.blockChanges.set(key, BLOCK_TYPE.AIR);
        this.isDirty = true;
        this.totalBlocksMined++;
        
        const value = BLOCK_VALUE[originalBlockType] || 1;
        const wasGold = originalBlockType === BLOCK_TYPE.GOLD_LOOT;
        
        if (wasGold) {
            this.totalGoldMined++;
        }
        
        return { success: true, value, wasGold, reason: 'OK' };
    }

    // ========================================================================
    // LOOT SPAWNS
    // ========================================================================

    /**
     * Register a loot spawn point
     * @param {number} x 
     * @param {number} y 
     * @param {number} z 
     * @param {number} type 
     * @param {number} value 
     */
    registerLoot(x, y, z, type, value) {
        const key = MapStorage.blockKey(x, y, z);
        this.lootSpawns.set(key, {
            type,
            value,
            spawnTime: Date.now(),
            collected: false,
        });
    }

    /**
     * Check if loot at position is available
     * @param {number} x 
     * @param {number} y 
     * @param {number} z 
     * @returns {Object|null}
     */
    getLoot(x, y, z) {
        const key = MapStorage.blockKey(x, y, z);
        const loot = this.lootSpawns.get(key);
        
        if (!loot || loot.collected) {
            return null;
        }
        
        return loot;
    }

    /**
     * Collect loot at position
     * @param {number} x 
     * @param {number} y 
     * @param {number} z 
     * @returns {{success: boolean, value: number}}
     */
    collectLoot(x, y, z) {
        const key = MapStorage.blockKey(x, y, z);
        const loot = this.lootSpawns.get(key);
        
        if (!loot || loot.collected) {
            return { success: false, value: 0 };
        }
        
        loot.collected = true;
        this.isDirty = true;
        
        return { success: true, value: loot.value };
    }

    // ========================================================================
    // SERIALIZATION / PERSISTENCE
    // ========================================================================

    /**
     * Get file path for this map
     * @returns {string}
     */
    get filePath() {
        return path.join(this.dataDir, `${this.mapId}.json`);
    }

    /**
     * Serialize state for saving
     * @returns {Object}
     */
    toJSON() {
        return {
            mapId: this.mapId,
            version: 1,
            savedAt: new Date().toISOString(),
            stats: {
                totalBlocksMined: this.totalBlocksMined,
                totalGoldMined: this.totalGoldMined,
                changedBlocks: this.blockChanges.size,
            },
            // Convert Map to array of [key, value] pairs
            blockChanges: Array.from(this.blockChanges.entries()),
            lootSpawns: Array.from(this.lootSpawns.entries()),
        };
    }

    /**
     * Load state from JSON
     * @param {Object} data 
     */
    fromJSON(data) {
        if (data.version !== 1) {
            console.warn(`[MAP] Unknown version ${data.version}, skipping load`);
            return;
        }
        
        this.totalBlocksMined = data.stats?.totalBlocksMined || 0;
        this.totalGoldMined = data.stats?.totalGoldMined || 0;
        
        // Restore Maps
        this.blockChanges = new Map(data.blockChanges || []);
        this.lootSpawns = new Map(data.lootSpawns || []);
        
        console.log(`[MAP] Loaded ${this.mapId}: ${this.blockChanges.size} block changes`);
    }

    /**
     * Save state to disk
     */
    save() {
        if (!this.isDirty) {
            return;
        }
        
        try {
            const data = JSON.stringify(this.toJSON(), null, 2);
            fs.writeFileSync(this.filePath, data, 'utf8');
            
            this.isDirty = false;
            this.lastSaveTime = Date.now();
            
            console.log(`[MAP] Saved ${this.mapId} (${this.blockChanges.size} changes)`);
        } catch (err) {
            console.error(`[MAP] Failed to save ${this.mapId}:`, err.message);
        }
    }

    /**
     * Load state from disk
     */
    load() {
        try {
            if (!fs.existsSync(this.filePath)) {
                console.log(`[MAP] No save file for ${this.mapId}, starting fresh`);
                return;
            }
            
            const data = JSON.parse(fs.readFileSync(this.filePath, 'utf8'));
            this.fromJSON(data);
            
        } catch (err) {
            console.error(`[MAP] Failed to load ${this.mapId}:`, err.message);
        }
    }

    /**
     * Get map changes for client sync
     * @returns {Array<{x: number, y: number, z: number, type: number}>}
     */
    getChangesForSync() {
        const changes = [];
        
        for (const [key, type] of this.blockChanges) {
            const { x, y, z } = MapStorage.parseKey(key);
            changes.push({ x, y, z, type });
        }
        
        return changes;
    }

    /**
     * Cleanup on shutdown
     */
    destroy() {
        clearInterval(this.saveTimer);
        this.save(); // Final save
    }
}

// ============================================================================
// EXPORTS
// ============================================================================

module.exports = {
    MapStorage,
    BLOCK_TYPE,
    BLOCK_VALUE,
    MINEABLE_BLOCKS,
};
