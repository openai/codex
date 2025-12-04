import { createBundle } from 'dts-buddy';

await createBundle({
  project: 'tsconfig.json',
  output: 'web-tree-sitter.d.ts',
  modules: {
    'web-tree-sitter': 'src/index.ts'
  },
  compilerOptions: {
    stripInternal: true,
  },
});
