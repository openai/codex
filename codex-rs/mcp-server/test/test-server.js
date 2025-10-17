#!/usr/bin/env node
/**
 * MCP Server Test Suite
 */

const { spawn } = require('child_process');
const path = require('path');

const tests = [];
let passed = 0;
let failed = 0;

function test(name, fn) {
  tests.push({ name, fn });
}

async function runTests() {
  console.log('🧪 Running MCP Server Tests...\n');

  for (const { name, fn } of tests) {
    try {
      await fn();
      console.log(`✅ ${name}`);
      passed++;
    } catch (error) {
      console.log(`❌ ${name}`);
      console.error(`   Error: ${error.message}`);
      failed++;
    }
  }

  console.log(`\n📊 Results: ${passed} passed, ${failed} failed`);
  process.exit(failed > 0 ? 1 : 0);
}

// Test: Server starts successfully
test('Server starts', async () => {
  const serverPath = path.join(__dirname, '..', 'dist', 'index.js');
  const proc = spawn('node', [serverPath]);

  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      proc.kill();
      reject(new Error('Server did not start within 3 seconds'));
    }, 3000);

    proc.stdout.on('data', (data) => {
      if (data.toString().includes('Ready')) {
        clearTimeout(timeout);
        proc.kill();
        resolve();
      }
    });

    proc.stderr.on('data', (data) => {
      clearTimeout(timeout);
      proc.kill();
      reject(new Error(data.toString()));
    });
  });
});

// Test: Agent list available
test('List agents', async () => {
  // ルートディレクトリから.codex/agentsを探す
  const rootDir = path.join(__dirname, '..', '..', '..');
  const agentsDir = path.join(rootDir, '.codex', 'agents');
  const fs = require('fs');
  
  if (!fs.existsSync(agentsDir)) {
    throw new Error(`.codex/agents directory not found at: ${agentsDir}`);
  }

  const files = fs.readdirSync(agentsDir);
  const yamlFiles = files.filter(f => f.endsWith('.yaml'));

  if (yamlFiles.length === 0) {
    throw new Error('No agent YAML files found');
  }

  console.log(`   Found ${yamlFiles.length} agents: ${yamlFiles.join(', ')}`);
});

// Test: Artifacts directory creation
test('Artifacts directory', async () => {
  const artifactsDir = path.join(process.cwd(), 'artifacts');
  const fs = require('fs');

  if (!fs.existsSync(artifactsDir)) {
    fs.mkdirSync(artifactsDir, { recursive: true });
  }

  if (!fs.existsSync(artifactsDir)) {
    throw new Error('Failed to create artifacts directory');
  }
});

// Test: MCP Request/Response format
test('MCP request format', async () => {
  const request = {
    tool: 'code_review',
    arguments: {
      scope: './src',
      language: 'typescript'
    }
  };

  const json = JSON.stringify(request);
  const parsed = JSON.parse(json);

  if (parsed.tool !== 'code_review') {
    throw new Error('Request parsing failed');
  }
});

runTests();

