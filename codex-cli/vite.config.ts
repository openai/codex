import { defineConfig } from 'vite';
import path from 'path';

// Provide a stub Vite config in the CLI package to avoid resolving a parent-level vite.config.js
export default defineConfig({
  resolve: {
    alias: {
      'src': path.resolve(__dirname, 'src'),
    },
  },
});