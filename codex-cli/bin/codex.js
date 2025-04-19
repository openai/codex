#!/usr/bin/env node

// Unified entry point for Codex CLI on all platforms
// Dynamically loads the compiled ESM bundle in dist/cli.js

import path from 'path';
import { fileURLToPath, pathToFileURL } from 'url';

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
