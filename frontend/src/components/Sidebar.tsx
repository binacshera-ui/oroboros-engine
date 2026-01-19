/**
 * GLITCH WARS - Sidebar Component
 * Inventory, Contracts, Stats
 */

import { useGameStore } from '../store/gameStore';

export function Sidebar() {
  const { sidebarOpen, inventory, player, toggleSidebar } = useGameStore();

  if (!sidebarOpen) return null;

  const rarityColors = {
    common: 'text-gray-400 border-gray-500',
    rare: 'text-terminal-blue border-terminal-blue',
    epic: 'text-terminal-purple border-terminal-purple',
    legendary: 'text-terminal-yellow border-terminal-yellow',
  };

  return (
    <>
      {/* Backdrop */}
      <div 
        className="fixed inset-0 bg-black/60 z-40"
        onClick={toggleSidebar}
      />
      
      {/* Sidebar Panel */}
      <aside className="fixed top-14 left-0 bottom-0 w-72 z-50 glass-panel-dark border-r border-white/10 overflow-hidden flex flex-col">
        
        {/* Player Stats */}
        <div className="p-4 border-b border-white/10">
          <h2 className="text-xs uppercase tracking-widest text-gray-500 mb-3">OPERATOR STATS</h2>
          
          <div className="space-y-3">
            {/* Health */}
            <div>
              <div className="flex justify-between text-xs mb-1">
                <span className="text-gray-400">HEALTH</span>
                <span className="text-terminal-red">{player.health}%</span>
              </div>
              <div className="h-2 bg-white/5 rounded overflow-hidden">
                <div 
                  className="h-full bg-gradient-to-r from-terminal-red to-red-400 transition-all"
                  style={{ width: `${player.health}%` }}
                />
              </div>
            </div>
            
            {/* Energy */}
            <div>
              <div className="flex justify-between text-xs mb-1">
                <span className="text-gray-400">ENERGY</span>
                <span className="text-terminal-green">{Math.round(player.energy)}%</span>
              </div>
              <div className="h-2 bg-white/5 rounded overflow-hidden">
                <div 
                  className="h-full bg-gradient-to-r from-terminal-green to-green-400 transition-all"
                  style={{ width: `${player.energy}%` }}
                />
              </div>
            </div>
            
            {/* Risk */}
            <div>
              <div className="flex justify-between text-xs mb-1">
                <span className="text-gray-400">RISK EXPOSURE</span>
                <span className="text-terminal-yellow">${player.risk.toLocaleString()}</span>
              </div>
              <div className="h-2 bg-white/5 rounded overflow-hidden">
                <div 
                  className="h-full bg-gradient-to-r from-terminal-yellow to-yellow-400 transition-all"
                  style={{ width: `${Math.min(player.risk / 100, 100)}%` }}
                />
              </div>
            </div>
          </div>
        </div>

        {/* Inventory */}
        <div className="flex-1 p-4 overflow-y-auto">
          <h2 className="text-xs uppercase tracking-widest text-gray-500 mb-3">INVENTORY</h2>
          
          {inventory.length === 0 ? (
            <div className="text-center py-8 text-gray-600">
              <div className="text-4xl mb-2">📦</div>
              <p className="text-xs">No items collected</p>
            </div>
          ) : (
            <div className="grid grid-cols-4 gap-2">
              {inventory.map(item => (
                <div 
                  key={item.id}
                  className={`aspect-square bg-white/5 border rounded-lg flex flex-col items-center justify-center cursor-pointer hover:bg-white/10 transition-colors ${rarityColors[item.rarity]}`}
                  title={item.name}
                >
                  <span className="text-lg">💎</span>
                  {item.quantity > 1 && (
                    <span className="text-[10px] font-mono">{item.quantity}</span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Active Contracts */}
        <div className="p-4 border-t border-white/10">
          <h2 className="text-xs uppercase tracking-widest text-gray-500 mb-3">ACTIVE CONTRACTS</h2>
          
          <div className="space-y-2">
            <div className="p-3 bg-terminal-green/10 border border-terminal-green/30 rounded-lg">
              <div className="flex justify-between items-center">
                <span className="text-xs text-terminal-green">MINING OP</span>
                <span className="text-xs text-gray-500">+$10/block</span>
              </div>
              <div className="mt-2 text-xs text-gray-400">
                Extract resources from the grid
              </div>
            </div>
            
            <div className="p-3 bg-white/5 border border-white/10 rounded-lg opacity-50">
              <div className="flex justify-between items-center">
                <span className="text-xs text-gray-400">PVP BOUNTY</span>
                <span className="text-xs text-gray-500">LOCKED</span>
              </div>
              <div className="mt-2 text-xs text-gray-500">
                Requires Level 5
              </div>
            </div>
          </div>
        </div>
      </aside>
    </>
  );
}

export default Sidebar;
