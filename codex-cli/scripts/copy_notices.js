#!/usr/bin/env node
import { copyFileSync, existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '..', '..');
const pkgDir = resolve(__dirname, '..');

const files = [
  { src: resolve(repoRoot, 'LICENSE'), dest: resolve(pkgDir, 'LICENSE') },
  { src: resolve(repoRoot, 'NOTICE'), dest: resolve(pkgDir, 'NOTICE') },
  { src: resolve(repoRoot, 'THIRD-PARTY-NOTICES.md'), dest: resolve(pkgDir, 'THIRD-PARTY-NOTICES.md') },
  { src: resolve(repoRoot, 'codex-rs', 'THIRD-PARTY-LICENSES.txt'), dest: resolve(pkgDir, 'THIRD-PARTY-LICENSES.txt') },
];

for (const { src, dest } of files) {
  if (!existsSync(src)) {
    // Skip silently if not present; generation step may create it.
    continue;
  }
  copyFileSync(src, dest);
  console.log(`Copied ${src} -> ${dest}`);
}
