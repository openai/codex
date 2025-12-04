import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    coverage: {
      include: [
        'tree-sitter.js',
      ],
      exclude: [
        'test/**',
        'dist/**',
        'lib/**',
        'wasm/**'
      ],
    },
  }
})
