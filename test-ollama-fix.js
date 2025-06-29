#!/usr/bin/env node

import { spawn } from 'child_process';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Test command that should trigger a tool call
const testPrompt = "List the files in the current directory";

console.log('Testing Ollama with function calling fix...\n');
console.log(`Prompt: "${testPrompt}"\n`);

// Set up environment
const env = {
  ...process.env,
  OLLAMA_API_KEY: 'dummy',
};

// Path to the local codex
const codexPath = path.join(__dirname, 'codex-cli/bin/codex.js');

// Spawn the codex process
const codex = spawn('node', [
  codexPath,
  testPrompt,
  '--provider', 'ollama',
  '--model', 'qwen2.5-coder:32b-128k',
  '--approval', 'auto',  // Auto-approve for testing
], {
  env,
  stdio: ['pipe', 'pipe', 'pipe'],
});

// Capture output
let output = '';
let errorOutput = '';

codex.stdout.on('data', (data) => {
  output += data.toString();
  process.stdout.write(data);
});

codex.stderr.on('data', (data) => {
  errorOutput += data.toString();
  process.stderr.write(data);
});

// Handle exit
codex.on('close', (code) => {
  console.log(`\nProcess exited with code ${code}`);
  
  // Check if a shell command was executed
  if (output.includes('shell') || output.includes('ls') || output.includes('dir')) {
    console.log('\n✅ Success! Tool call appears to be working.');
  } else if (errorOutput.includes('Raw mode is not supported')) {
    console.log('\n⚠️  Terminal mode issue encountered. This is expected in non-interactive mode.');
  } else {
    console.log('\n❌ No tool call detected in output.');
  }
});

// Send input after a delay to handle any prompts
setTimeout(() => {
  codex.stdin.write('\n');
  codex.stdin.end();
}, 1000);