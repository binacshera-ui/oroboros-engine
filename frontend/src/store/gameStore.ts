/**
 * GLITCH WARS - Global Game State (Zustand)
 * 
 * Manages all UI state that bridges between React and WASM/Rust
 */

import { create } from 'zustand';

// ============================================================================
// TYPES
// ============================================================================

export interface WalletState {
  address: string | null;
  balance: number;
  isConnected: boolean;
}

export interface PlayerStats {
  health: number;
  energy: number;
  risk: number;
  loot: number;
}

export interface NetworkStats {
  connected: boolean;
  playerCount: number;
  ping: number;
  fps: number;
  serverTick: number;
}

export interface Notification {
  id: string;
  message: string;
  type: 'info' | 'success' | 'warning' | 'danger';
  timestamp: number;
}

export interface InventoryItem {
  id: string;
  name: string;
  quantity: number;
  rarity: 'common' | 'rare' | 'epic' | 'legendary';
}

export interface GameState {
  // Loading
  isLoading: boolean;
  loadingProgress: number;
  loadingMessage: string;
  
  // Wallet
  wallet: WalletState;
  
  // Player
  player: PlayerStats;
  
  // Network
  network: NetworkStats;
  
  // UI
  notifications: Notification[];
  sidebarOpen: boolean;
  inventory: InventoryItem[];
  
  // Actions
  setLoading: (loading: boolean, message?: string) => void;
  setLoadingProgress: (progress: number) => void;
  
  connectWallet: (address: string) => void;
  disconnectWallet: () => void;
  updateBalance: (balance: number) => void;
  
  updatePlayerStats: (stats: Partial<PlayerStats>) => void;
  updateNetworkStats: (stats: Partial<NetworkStats>) => void;
  
  addNotification: (message: string, type: Notification['type']) => void;
  removeNotification: (id: string) => void;
  
  toggleSidebar: () => void;
  addInventoryItem: (item: InventoryItem) => void;
}

// ============================================================================
// STORE
// ============================================================================

export const useGameStore = create<GameState>((set, get) => ({
  // Initial state
  isLoading: true,
  loadingProgress: 0,
  loadingMessage: 'Initializing...',
  
  wallet: {
    address: null,
    balance: 0,
    isConnected: false,
  },
  
  player: {
    health: 100,
    energy: 100,
    risk: 0,
    loot: 0,
  },
  
  network: {
    connected: false,
    playerCount: 0,
    ping: 0,
    fps: 60,
    serverTick: 0,
  },
  
  notifications: [],
  sidebarOpen: false,
  inventory: [],
  
  // Actions
  setLoading: (loading, message) => set({ 
    isLoading: loading, 
    loadingMessage: message || (loading ? 'Loading...' : '') 
  }),
  
  setLoadingProgress: (progress) => set({ loadingProgress: progress }),
  
  connectWallet: (address) => set({ 
    wallet: { address, balance: 0, isConnected: true } 
  }),
  
  disconnectWallet: () => set({ 
    wallet: { address: null, balance: 0, isConnected: false } 
  }),
  
  updateBalance: (balance) => set(state => ({ 
    wallet: { ...state.wallet, balance } 
  })),
  
  updatePlayerStats: (stats) => set(state => ({ 
    player: { ...state.player, ...stats } 
  })),
  
  updateNetworkStats: (stats) => set(state => ({ 
    network: { ...state.network, ...stats } 
  })),
  
  addNotification: (message, type) => {
    const id = `notif-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
    const notification: Notification = { id, message, type, timestamp: Date.now() };
    
    set(state => ({
      notifications: [notification, ...state.notifications].slice(0, 5)
    }));
    
    // Auto-remove after 5 seconds
    setTimeout(() => {
      get().removeNotification(id);
    }, 5000);
  },
  
  removeNotification: (id) => set(state => ({
    notifications: state.notifications.filter(n => n.id !== id)
  })),
  
  toggleSidebar: () => set(state => ({ sidebarOpen: !state.sidebarOpen })),
  
  addInventoryItem: (item) => set(state => {
    const existing = state.inventory.find(i => i.id === item.id);
    if (existing) {
      return {
        inventory: state.inventory.map(i => 
          i.id === item.id ? { ...i, quantity: i.quantity + item.quantity } : i
        )
      };
    }
    return { inventory: [...state.inventory, item] };
  }),
}));

// ============================================================================
// GLOBAL BRIDGE - Functions callable from WASM/Rust
// ============================================================================

// Expose to window for Rust/WASM calls
if (typeof window !== 'undefined') {
  const store = useGameStore.getState;
  
  (window as any).updateBalance = (val: number) => {
    store().updateBalance(val);
  };
  
  (window as any).updateRisk = (val: number) => {
    store().updatePlayerStats({ risk: val });
  };
  
  (window as any).updateEnergy = (val: number) => {
    store().updatePlayerStats({ energy: val });
  };
  
  (window as any).updateLoot = (val: number) => {
    store().updatePlayerStats({ loot: val });
  };
  
  (window as any).updateServerStatus = (connected: boolean, playerCount: number, ping: number) => {
    store().updateNetworkStats({ connected, playerCount, ping });
  };
  
  (window as any).addNotification = (message: string, type: 'info' | 'success' | 'warning' | 'danger') => {
    store().addNotification(message, type);
  };
  
  (window as any).setGameLoaded = () => {
    store().setLoading(false);
    store().addNotification('SYSTEM ONLINE', 'success');
  };
  
  (window as any).updateFPS = (fps: number) => {
    store().updateNetworkStats({ fps });
  };
}

export default useGameStore;
