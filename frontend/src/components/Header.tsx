/**
 * GLITCH WARS - Header Component
 * Logo + Wallet Connect + Network Status
 */

import { useGameStore } from '../store/gameStore';

export function Header() {
  const { wallet, network, toggleSidebar, connectWallet, disconnectWallet, addNotification } = useGameStore();

  const handleConnectWallet = () => {
    // Mock wallet connection
    const mockAddress = '0x' + Array(40).fill(0).map(() => 
      Math.floor(Math.random() * 16).toString(16)
    ).join('');
    
    connectWallet(mockAddress);
    addNotification('WALLET CONNECTED', 'success');
  };

  const formatAddress = (addr: string) => {
    return `${addr.slice(0, 6)}...${addr.slice(-4)}`;
  };

  return (
    <header className="fixed top-0 left-0 right-0 z-50 h-14 flex items-center justify-between px-4 glass-panel-dark border-b border-white/10">
      {/* Left - Logo & Menu */}
      <div className="flex items-center gap-4">
        <button 
          onClick={toggleSidebar}
          className="p-2 hover:bg-white/10 rounded-lg transition-colors"
          aria-label="Toggle menu"
        >
          <svg className="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
          </svg>
        </button>
        
        <div className="flex items-center gap-2">
          <div className="w-8 h-8 bg-gradient-to-br from-terminal-green to-terminal-blue rounded-lg flex items-center justify-center">
            <span className="text-xs font-bold text-black">GW</span>
          </div>
          <h1 className="font-display text-lg font-bold tracking-widest neon-text-green hidden sm:block">
            GLITCH WARS
          </h1>
        </div>
      </div>

      {/* Center - Network Status */}
      <div className="hidden md:flex items-center gap-6">
        <div className="flex items-center gap-2">
          <span className={`w-2 h-2 rounded-full ${network.connected ? 'bg-terminal-green animate-pulse' : 'bg-terminal-red'}`} />
          <span className="text-xs text-gray-400 uppercase tracking-wider">
            {network.connected ? 'LIVE' : 'OFFLINE'}
          </span>
        </div>
        
        <div className="text-xs font-mono">
          <span className="text-gray-500">PLAYERS:</span>
          <span className="text-terminal-blue ml-1">{network.playerCount}</span>
        </div>
        
        <div className="text-xs font-mono">
          <span className="text-gray-500">PING:</span>
          <span className={`ml-1 ${network.ping < 50 ? 'text-terminal-green' : network.ping < 100 ? 'text-terminal-yellow' : 'text-terminal-red'}`}>
            {network.ping}ms
          </span>
        </div>
        
        <div className="text-xs font-mono">
          <span className="text-gray-500">FPS:</span>
          <span className={`ml-1 ${network.fps >= 55 ? 'text-terminal-green' : network.fps >= 30 ? 'text-terminal-yellow' : 'text-terminal-red'}`}>
            {network.fps}
          </span>
        </div>
      </div>

      {/* Right - Wallet */}
      <div className="flex items-center gap-3">
        {wallet.isConnected ? (
          <>
            <div className="hidden sm:flex flex-col items-end">
              <span className="text-xs text-gray-500">WALLET</span>
              <span className="text-sm font-mono text-terminal-green">
                {formatAddress(wallet.address!)}
              </span>
            </div>
            <div className="hidden sm:flex flex-col items-end">
              <span className="text-xs text-gray-500">BALANCE</span>
              <span className="text-sm font-mono text-terminal-yellow">
                ${wallet.balance.toLocaleString()}
              </span>
            </div>
            <button
              onClick={disconnectWallet}
              className="btn-danger text-xs py-1.5"
            >
              DISCONNECT
            </button>
          </>
        ) : (
          <button
            onClick={handleConnectWallet}
            className="btn-primary"
          >
            CONNECT WALLET
          </button>
        )}
      </div>
    </header>
  );
}

export default Header;
