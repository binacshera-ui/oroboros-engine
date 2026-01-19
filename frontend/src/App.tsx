/**
 * GLITCH WARS - Enterprise Dashboard
 * React + Bevy/WASM Integration
 * 
 * Bloomberg/Binance Terminal Aesthetic
 */

import { useEffect } from 'react';
import { Header } from './components/Header';
import { Sidebar } from './components/Sidebar';
import { StatusBar } from './components/StatusBar';
import { NotificationFeed } from './components/NotificationFeed';
import { GameView } from './components/GameView';
import { useGameStore } from './store/gameStore';

function App() {
  const { updateNetworkStats, updatePlayerStats } = useGameStore();

  // Demo: Simulate some network activity
  useEffect(() => {
    // Simulate FPS counter
    let frameCount = 0;
    let lastTime = performance.now();
    
    const measureFPS = () => {
      frameCount++;
      const now = performance.now();
      
      if (now - lastTime >= 1000) {
        updateNetworkStats({ fps: frameCount });
        frameCount = 0;
        lastTime = now;
      }
      
      requestAnimationFrame(measureFPS);
    };
    
    requestAnimationFrame(measureFPS);

    // Simulate energy regeneration
    const energyInterval = setInterval(() => {
      const state = useGameStore.getState();
      if (state.player.energy < 100) {
        updatePlayerStats({ energy: Math.min(100, state.player.energy + 0.5) });
      }
    }, 500);

    return () => {
      clearInterval(energyInterval);
    };
  }, []);

  return (
    <div className="relative w-full h-full bg-terminal-darker overflow-hidden">
      {/* CRT Scanline Effect */}
      <div className="crt-overlay" />
      
      {/* Game Canvas (Background) */}
      <GameView />
      
      {/* UI Overlay (z-index layers) */}
      <Header />
      <Sidebar />
      <StatusBar />
      <NotificationFeed />
      
      {/* Keyboard hints */}
      <div className="fixed bottom-20 left-4 z-40 text-xs font-mono text-gray-600 space-y-1 pointer-events-none">
        <div><span className="text-gray-500">[WASD]</span> MOVE</div>
        <div><span className="text-gray-500">[SPACE]</span> JUMP</div>
        <div><span className="text-gray-500">[MOUSE]</span> LOOK</div>
        <div><span className="text-gray-500">[CLICK]</span> INTERACT</div>
      </div>
      </div>
  );
}

export default App;
