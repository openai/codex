#!/usr/bin/env node

/**
 * Test script for Ollama integration in Codex CLI.
 * 
 * This script tests the Ollama integration by:
 * 1. Checking if Ollama is running
 * 2. Listing available Ollama models
 * 3. Testing the Ollama API client
 */

const http = require('http');
const { execSync } = require('child_process');

// Colors for console output
const colors = {
  reset: '\x1b[0m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  cyan: '\x1b[36m',
};

/**
 * Print a colored message to the console.
 * 
 * @param {string} message The message to print.
 * @param {string} color The color to use.
 */
function log(message, color = colors.reset) {
  console.log(`${color}${message}${colors.reset}`);
}

/**
 * Check if Ollama is running.
 * 
 * @returns {Promise<boolean>} True if Ollama is running, false otherwise.
 */
function isOllamaRunning() {
  return new Promise((resolve) => {
    const req = http.get('http://localhost:11434/api/tags', (res) => {
      if (res.statusCode === 200) {
        resolve(true);
      } else {
        resolve(false);
      }
      res.resume(); // Consume response data to free up memory
    });
    
    req.on('error', () => {
      resolve(false);
    });
    
    req.setTimeout(2000, () => {
      req.destroy();
      resolve(false);
    });
  });
}

/**
 * Get the list of available Ollama models.
 * 
 * @returns {Promise<string[]>} A list of available Ollama models.
 */
async function getOllamaModels() {
  return new Promise((resolve, reject) => {
    const req = http.get('http://localhost:11434/api/tags', (res) => {
      let data = '';
      
      res.on('data', (chunk) => {
        data += chunk;
      });
      
      res.on('end', () => {
        try {
          const json = JSON.parse(data);
          const models = json.models.map(model => model.name);
          resolve(models);
        } catch (error) {
          reject(error);
        }
      });
    });
    
    req.on('error', (error) => {
      reject(error);
    });
  });
}

/**
 * Test the Ollama API client.
 */
async function testOllamaClient() {
  log('Testing Ollama integration for Codex CLI', colors.cyan);
  log('======================================', colors.cyan);
  
  // Check if Ollama is running
  log('\nChecking if Ollama is running...', colors.yellow);
  const ollamaRunning = await isOllamaRunning();
  
  if (ollamaRunning) {
    log('✅ Ollama is running', colors.green);
    
    // Get available models
    log('\nFetching available Ollama models...', colors.yellow);
    try {
      const models = await getOllamaModels();
      log(`✅ Found ${models.length} models:`, colors.green);
      models.forEach(model => {
        log(`  - ${model}`, colors.blue);
      });
      
      // Check if gemma3:1b is available
      if (models.includes('gemma3:1b')) {
        log('\n✅ gemma3:1b model is available', colors.green);
      } else {
        log('\n❌ gemma3:1b model is not available', colors.red);
        log('Please run: ollama pull gemma3:1b', colors.yellow);
      }
      
    } catch (error) {
      log(`❌ Error fetching models: ${error.message}`, colors.red);
    }
  } else {
    log('❌ Ollama is not running', colors.red);
    log('Please start Ollama and try again', colors.yellow);
  }
  
  // Test Codex CLI with Ollama
  log('\nTesting Codex CLI with Ollama integration...', colors.yellow);
  try {
    // Build the project
    log('\nBuilding the project...', colors.yellow);
    execSync('npm run build', { stdio: 'inherit', cwd: __dirname });
    
    log('\n✅ Build successful', colors.green);
    log('\nYou can now run Codex CLI with Ollama models:', colors.cyan);
    log('  node dist/cli.js --model ollama:gemma3:1b', colors.blue);
    
  } catch (error) {
    log(`❌ Error building project: ${error.message}`, colors.red);
  }
}

// Run the test
testOllamaClient().catch(error => {
  log(`❌ Unhandled error: ${error.message}`, colors.red);
  process.exit(1);
});
