import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from "path"

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src")
    }
  },
  optimizeDeps: {
    exclude: ['class-variance-authority', 'clsx', 'tailwind-merge']
  },
  // Server configuration: host binding and proxy to Codex server
  server: {
    // Listen on all interfaces so Docker port mapping works
    host: '0.0.0.0',
    port: 5173,
    proxy: {
      // Forward prompt submissions
      '/prompt': { target: 'http://localhost:3000', changeOrigin: true },
      // Forward state fetches
      '/state': { target: 'http://localhost:3000', changeOrigin: true },
      // Forward Codex WebSocket upgrades under /codex-ws, rewrite to /ws
      '/codex-ws': {
        target: 'ws://localhost:3000',
        changeOrigin: true,
        ws: true,
        rewrite: (path) => path.replace(/^\/codex-ws/, '/ws'),
      },
      // Leave /ws for Vite HMR
    }
  }
})
