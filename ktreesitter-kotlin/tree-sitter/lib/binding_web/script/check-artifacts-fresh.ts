import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));

const inputFiles = [
  '../lib/tree-sitter.c',
  '../src/constants.ts',
  '../src/index.ts',
  '../src/language.ts',
  '../src/lookahead_iterator.ts',
  '../src/marshal.ts',
  '../src/node.ts',
  '../src/parser.ts',
  '../src/query.ts',
  '../src/tree.ts',
  '../src/tree_cursor.ts',
  '../lib/exports.txt',
  '../lib/imports.js',
  '../lib/prefix.js',
  ...listFiles('../../include/tree_sitter'),
  ...listFiles('../../src'),
];

const outputFiles = ['../tree-sitter.js', '../tree-sitter.wasm'];
const outputMtime = Math.min(...outputFiles.map(getMtime));

for (const inputFile of inputFiles) {
  if (getMtime(inputFile) > outputMtime) {
    console.log(`File '${inputFile}' has changed. Re-run 'npm run build:wasm'.`);
    process.exit(1);
  }
}

function listFiles(dir: string): string[] {
  return fs
    .readdirSync(path.resolve(scriptDir, dir))
    .filter(p => !p.startsWith('.'))
    .map(p => path.join(dir, p));
}

function getMtime(p: string): number {
  return fs.statSync(path.resolve(scriptDir, p)).mtime.getTime();
}
