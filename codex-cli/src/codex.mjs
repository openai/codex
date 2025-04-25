#!/usr/bin/env node
import { dirname } from 'path';
import { fileURLToPath } from 'url';

// polyfill __dirname
const __dirname = dirname(fileURLToPath(import.meta.url));
Object.defineProperty(globalThis, '__dirname', { value: __dirname });

// now load the real CLI
import './cli.js';
