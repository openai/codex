#!/usr/bin/env node
// Unified entry point for the Codex CLI.
/*
 * Behavior
 * =========
 *   1. By default we import the JavaScript implementation located in
 *      dist/cli.js (exactly what the original entry point did).
 *
 *   2. Developers can opt-in to a pre-compiled Rust binary by setting the
 *      environment variable CODEX_RUST to a truthy value (`1`, `true`, etc.).
 *      When that variable is present we resolve the correct binary for the
 *      current platform / architecture and execute it via child_process.
 *
 *      At the moment the npm package only bundles Linux binaries that were
 *      added when the release was staged with
 *
 *        pnpm stage-release --native
 *
 *      On unsupported systems (or if the binary is missing) we fall back to
 *      the JS implementation so that the CLI remains functional everywhere.
 */

import { spawnSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath, pathToFileURL } from 'url';

// Determine whether the user explicitly wants the Rust CLI.

const wantsNative = (() => {
  if (!process.env.CODEX_RUST) {return false;}
  const val = process.env.CODEX_RUST.toLowerCase();
  return ['1', 'true', 'yes'].includes(val);
})();

// Try native binary first (only when requested).

if (wantsNative) {
  const platform = process.platform; // 'linux', 'darwin', etc.
  const arch = process.arch;         // 'x64', 'arm64', etc.

  let targetTriple;
  if (platform === 'linux') {
    if (arch === 'x64')   {targetTriple = 'x86_64-unknown-linux-musl';}
    if (arch === 'arm64') {targetTriple = 'aarch64-unknown-linux-gnu';}
  } else if (platform === 'darwin') {
    if (arch === 'x64')   {targetTriple = 'x86_64-apple-darwin';}
    if (arch === 'arm64') {targetTriple = 'aarch64-apple-darwin';}
  } else {
    throw new Error(`Unsupported platform: ${platform} (${arch})`);
  }

  if (targetTriple) {
    // __dirname equivalent in ESM
    const __filename = fileURLToPath(import.meta.url);
    const __dirname  = path.dirname(__filename);

    const binaryPath = path.join(__dirname, '..', 'bin', `codex-${targetTriple}`);

    if (fs.existsSync(binaryPath)) {
      const result = spawnSync(binaryPath, process.argv.slice(2), {
        stdio: 'inherit',
      });

      const exitCode = typeof result.status === 'number' ? result.status : 0;
      process.exit(exitCode);
    } else {
      console.warn(`[codex-cli] Native binary not found at ${binaryPath}. Falling back to JS implementation...`);
    }
  } else {
    console.warn('[codex-cli] Platform not yet supported by native binary. Falling back to JS implementation...');
  }
}

// Fallback: execute the original JavaScript CLI.

// Determine this script's directory
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Resolve the path to the compiled CLI bundle
const cliPath = path.resolve(__dirname, '../dist/cli.js');
const cliUrl = pathToFileURL(cliPath).href;

// Load and execute the CLI
(async () => {
  try {
    await import(cliUrl);
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    // eslint-disable-next-line no-undef
    process.exit(1);
  }
})();
