import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  
  server: {
    port: 5173,
    host: '0.0.0.0',
    // Proxy WebSocket connections to game server
    proxy: {
      '/ws': {
        target: 'ws://localhost:3000',
        ws: true,
      },
    },
  },
  
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    // Ensure WASM files are properly handled
    rollupOptions: {
      output: {
        manualChunks: undefined,
      },
    },
  },
  
  // Handle WASM files
  optimizeDeps: {
    exclude: ['@anthropic-ai/sdk'],
  },
  
  // Allow loading WASM from public folder
  publicDir: 'public',
})
