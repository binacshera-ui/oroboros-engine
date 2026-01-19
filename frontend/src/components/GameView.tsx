/**
 * GLITCH WARS - Game View Component
 * Handles WASM loading and canvas rendering
 */

import { useEffect, useRef, useState } from 'react';
import { useGameStore } from '../store/gameStore';

export function GameView() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const { setLoading, setLoadingProgress, addNotification, isLoading, loadingProgress, loadingMessage } = useGameStore();
  const initStarted = useRef(false);

  useEffect(() => {
    if (initStarted.current) return;
    initStarted.current = true;

    const loadGame = async () => {
      try {
        setLoading(true, 'Loading WASM module...');
        setLoadingProgress(10);

        // Create script element to load WASM JS
        const script = document.createElement('script');
        script.type = 'module';
        
        // Inline module that imports and runs the WASM
        // NOTE: Bevy WASM uses exceptions for control flow, so init() never truly resolves
        // Instead, it throws an exception to return control to the JS event loop
        script.textContent = `
          import init from '/wasm/glitch_wars.js';
          
          // Don't await - Bevy's init throws an exception to return control to event loop
          init({ module_or_path: '/wasm/glitch_wars_bg.wasm' }).then(() => {
            console.log('[WASM] Game initialized successfully');
            if (window.setGameLoaded) window.setGameLoaded();
          }).catch(e => {
            // Check if this is the expected control flow exception
            if (e.message && e.message.includes('control flow')) {
              console.log('[WASM] Bevy running (control flow exception is normal)');
              if (window.setGameLoaded) window.setGameLoaded();
            } else {
              console.error('[WASM] Failed to initialize:', e);
            }
          });
          
          console.log('[WASM] Init started (async)');
        `;
        
        setLoadingProgress(50);
        setLoading(true, 'Starting game engine...');
        
        document.head.appendChild(script);
        
        setLoadingProgress(80);
        
        // Wait for game to signal ready or timeout
        const waitForGame = new Promise<void>((resolve) => {
          const timeout = setTimeout(() => {
            resolve();
          }, 10000);
          
          // Check if game started (canvas created)
          const checkCanvas = setInterval(() => {
            const canvas = document.querySelector('canvas');
            if (canvas) {
              clearInterval(checkCanvas);
              clearTimeout(timeout);
              resolve();
            }
          }, 100);
        });
        
        await waitForGame;
        
        setLoadingProgress(100);
        setLoading(false);
        addNotification('SYSTEM INITIALIZED', 'success');
        
      } catch (err) {
        console.error('Failed to load game:', err);
        setError(err instanceof Error ? err.message : 'Failed to load game');
        setLoading(false);
        addNotification('SYSTEM ERROR', 'danger');
      }
    };

    loadGame();
  }, []);

  // Loading screen
  if (isLoading || error) {
    return (
      <div className="absolute inset-0 flex flex-col items-center justify-center bg-[#050508] z-30">
        {/* Background grid effect */}
        <div 
          className="absolute inset-0 opacity-10"
          style={{
            backgroundImage: `
              linear-gradient(rgba(0,255,136,0.1) 1px, transparent 1px),
              linear-gradient(90deg, rgba(0,255,136,0.1) 1px, transparent 1px)
            `,
            backgroundSize: '50px 50px',
          }}
        />
        
        {/* Logo */}
        <div className="relative z-10 text-center">
          <h1 className="font-[Orbitron] text-5xl font-bold tracking-widest neon-text-green mb-2">
            GLITCH WARS
          </h1>
          <p className="text-gray-500 text-sm tracking-widest uppercase">
            THE SIMULATION
          </p>
        </div>

        {error ? (
          <div className="mt-12 text-center">
            <div className="text-[#ff3366] text-6xl mb-4">⚠</div>
            <p className="text-[#ff3366] font-mono">{error}</p>
            <button 
              onClick={() => window.location.reload()}
              className="mt-6 btn-primary"
            >
              RETRY CONNECTION
            </button>
          </div>
        ) : (
          <div className="mt-12 w-80">
            {/* Progress bar */}
            <div className="h-1 bg-white/10 rounded-full overflow-hidden mb-4">
              <div 
                className="h-full bg-[#00ff88] transition-all duration-300"
                style={{ width: `${loadingProgress}%` }}
              />
            </div>
            
            {/* Status text */}
            <div className="flex justify-between items-center text-xs font-mono">
              <span className="text-[#00ff88] animate-pulse">
                {loadingMessage}
              </span>
              <span className="text-gray-500">
                {loadingProgress}%
              </span>
            </div>
            
            {/* ASCII loading animation */}
            <div className="mt-8 text-center font-mono text-[#00ff88] text-xs">
              <div className="animate-pulse">
                {'>'} INITIALIZING NEURAL NETWORK...
              </div>
            </div>
          </div>
        )}

        {/* Version */}
        <div className="absolute bottom-4 left-4 text-xs text-gray-600 font-mono">
          v0.1.0-alpha | React + Bevy
        </div>
      </div>
    );
  }

  return (
    <div 
      id="game-canvas-container" 
      ref={containerRef}
      className="absolute inset-0"
    />
  );
}

export default GameView;
