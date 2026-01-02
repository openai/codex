import { defineConfig } from 'tsup';

export default defineConfig({
  entry: { 'mcp-server': 'src/index.ts' },
  format: ['cjs'],
  target: 'node18',
  clean: true,
  shims: true
});
