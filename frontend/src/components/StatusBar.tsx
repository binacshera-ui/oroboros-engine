/**
 * GLITCH WARS - Status Bar Component
 * Bottom HUD with Energy, Health, and Stats
 */

import { useGameStore } from '../store/gameStore';

export function StatusBar() {
  const { player, wallet, network } = useGameStore();

  return (
    <footer className="fixed bottom-0 left-0 right-0 z-40 h-16 glass-panel-dark border-t border-white/10">
      <div className="h-full flex items-center justify-between px-4">
        
        {/* Left - Energy Bar */}
        <div className="flex items-center gap-4">
          <div className="w-64">
            <div className="flex justify-between text-xs mb-1">
              <span className="text-gray-400 uppercase tracking-wider">ENERGY</span>
              <span className="text-terminal-green font-mono">{Math.round(player.energy)}%</span>
            </div>
            <div className="h-3 bg-white/5 rounded-full overflow-hidden border border-white/10">
              <div 
                className="h-full bg-gradient-to-r from-terminal-green/80 to-terminal-green rounded-full transition-all duration-300"
                style={{ 
                  width: `${player.energy}%`,
                  boxShadow: player.energy > 20 ? '0 0 10px rgba(0, 255, 136, 0.5)' : 'none'
                }}
              />
            </div>
          </div>
          
          {/* Health indicator */}
          <div className="flex items-center gap-2 px-4 py-2 bg-terminal-red/10 border border-terminal-red/30 rounded-lg">
            <span className="text-terminal-red">❤</span>
            <span className="text-sm font-mono text-terminal-red">{player.health}</span>
          </div>
        </div>

        {/* Center - Risk/Loot Display */}
        <div className="flex items-center gap-8">
          <div className="text-center">
            <div className="text-xs text-gray-500 uppercase tracking-wider mb-1">RISK AMOUNT</div>
            <div className="text-2xl font-mono font-bold text-terminal-red">
              ${player.risk.toLocaleString()}
            </div>
          </div>
          
          <div className="w-px h-8 bg-white/10" />
          
          <div className="text-center">
            <div className="text-xs text-gray-500 uppercase tracking-wider mb-1">LOOT COLLECTED</div>
            <div className="text-2xl font-mono font-bold text-terminal-yellow">
              ${player.loot.toLocaleString()}
            </div>
          </div>
          
          {wallet.isConnected && (
            <>
              <div className="w-px h-8 bg-white/10" />
              <div className="text-center">
                <div className="text-xs text-gray-500 uppercase tracking-wider mb-1">TOTAL BALANCE</div>
                <div className="text-2xl font-mono font-bold text-terminal-green">
                  ${wallet.balance.toLocaleString()}
                </div>
              </div>
            </>
          )}
        </div>

        {/* Right - Network Stats */}
        <div className="flex items-center gap-4 text-xs font-mono">
          <div className="flex flex-col items-end">
            <span className="text-gray-500">TICK</span>
            <span className="text-terminal-blue">{network.serverTick}</span>
          </div>
          <div className="flex flex-col items-end">
            <span className="text-gray-500">LATENCY</span>
            <span className={network.ping < 50 ? 'text-terminal-green' : network.ping < 100 ? 'text-terminal-yellow' : 'text-terminal-red'}>
              {network.ping}ms
            </span>
          </div>
          <div className="flex flex-col items-end">
            <span className="text-gray-500">RENDER</span>
            <span className={network.fps >= 55 ? 'text-terminal-green' : 'text-terminal-yellow'}>
              {network.fps} FPS
            </span>
          </div>
        </div>
      </div>
    </footer>
  );
}

export default StatusBar;
