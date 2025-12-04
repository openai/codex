import esbuild from 'esbuild';
import fs from 'fs/promises';
import path from 'path';

const format = process.env.CJS ? 'cjs' : 'esm';
const debug = process.argv.includes('--debug');
const outfile = `${debug ? 'debug/' : ''}tree-sitter.${format === 'esm' ? 'js' : 'cjs'}`;

// Copy source files to lib directory - we'll map the wasm's sourecmap to these files.
async function copySourceFiles() {
  const sourceDir = '../src';
  const files = await fs.readdir(sourceDir);

  for (const file of files) {
    if (file.endsWith('.c') || file.endsWith('.h')) {
      await fs.copyFile(
        path.join(sourceDir, file),
        path.join('lib', file)
      );
    }
  }
}

async function processWasmSourceMap(inputPath, outputPath) {
  const mapContent = await fs.readFile(inputPath, 'utf8');
  const sourceMap = JSON.parse(mapContent);

  // Filter out emscripten files and normalize paths
  sourceMap.sources = sourceMap.sources
    .filter(source => {
      // Keep only tree-sitter source files
      return source.includes('../../src/') || source === 'tree-sitter.c';
    })
    .map(source => {
      if (source.includes('../../src/')) {
        return source.replace('../../src/', 'lib/');
      }
      return source;
    });

  await fs.writeFile(outputPath, JSON.stringify(sourceMap, null, 2));
}


async function build() {
  await esbuild.build({
    entryPoints: ['src/index.ts'],
    bundle: true,
    platform: 'node',
    format,
    outfile,
    sourcemap: true,
    sourcesContent: true,
    keepNames: true,
    external: ['fs/*', 'fs/promises'],
    resolveExtensions: ['.ts', '.js', format === 'esm' ? '.mjs' : '.cjs'],
  });

  // Copy the WASM files to the appropriate spot, as esbuild doesn't "bundle" WASM files
  const outputWasmName = `${debug ? 'debug/' : ''}tree-sitter.wasm`;
  await fs.copyFile('lib/tree-sitter.wasm', outputWasmName);

  await copySourceFiles();
  await processWasmSourceMap('lib/tree-sitter.wasm.map', `${outputWasmName}.map`);
}

build().catch(console.error);
