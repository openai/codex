#!/usr/bin/env node

/**
 * Codex Interface - Interfaces with Codex CLI and participates in bridge conversation
 */

import fs from 'fs';
import path from 'path';
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const BRIDGE_DIR = '/mnt/c/Users/chris/codex/llm_bridge';
const CODEX_CLI_PATH = '/mnt/c/Users/chris/codex/codex-cli/dist/cli.js';

const FILES = {
  claudeToCodex: path.join(BRIDGE_DIR, 'claude_to_codex.txt'),
  codexToClaude: path.join(BRIDGE_DIR, 'codex_to_claude.txt'),
  turnControl: path.join(BRIDGE_DIR, 'turn_control.txt'),
  status: path.join(BRIDGE_DIR, 'bridge_status.txt')
};

class CodexInterface {
  constructor() {
    this.lastClaudeMessage = '';
    this.isWaiting = false;
    this.codexProcess = null;
  }

  log(message) {
    console.log(`[CODEX INTERFACE] ${message}`);
  }

  getCurrentTurn() {
    try {
      return fs.readFileSync(FILES.turnControl, 'utf8').trim();
    } catch {
      return 'UNKNOWN';
    }
  }

  sendResponseToClaude(message) {
    fs.writeFileSync(FILES.codexToClaude, message);
    this.log('Response sent to Claude');
  }

  checkForClaudeMessage() {
    try {
      const message = fs.readFileSync(FILES.claudeToCodex, 'utf8').trim();
      if (message && message !== this.lastClaudeMessage) {
        this.lastClaudeMessage = message;
        console.log('\n' + '='.repeat(60));
        console.log('CLAUDE SAYS:');
        console.log('='.repeat(60));
        console.log(message);
        console.log('='.repeat(60) + '\n');
        
        // Clear the message after reading
        fs.writeFileSync(FILES.claudeToCodex, '');
        
        return message;
      }
    } catch (error) {
      // File might not exist yet
    }
    return null;
  }

  async runCodexCommand(prompt) {
    return new Promise((resolve, reject) => {
      this.log(`Running Codex with prompt: "${prompt.substring(0, 50)}..."`);
      
      // Use quiet mode to get clean output
      const codex = spawn('node', [CODEX_CLI_PATH, '--quiet', prompt], {
        stdio: ['pipe', 'pipe', 'pipe'],
        env: { 
          ...process.env,
          OPENAI_API_KEY: process.env.OPENAI_API_KEY 
        }
      });

      let output = '';
      let errorOutput = '';

      codex.stdout.on('data', (data) => {
        output += data.toString();
      });

      codex.stderr.on('data', (data) => {
        errorOutput += data.toString();
      });

      codex.on('close', (code) => {
        if (code === 0) {
          resolve(output.trim() || 'Codex completed successfully but provided no output.');
        } else {
          reject(new Error(`Codex exited with code ${code}: ${errorOutput}`));
        }
      });

      codex.on('error', (error) => {
        reject(error);
      });

      // Set a timeout for long-running commands
      setTimeout(() => {
        codex.kill();
        reject(new Error('Codex command timed out after 60 seconds'));
      }, 60000);
    });
  }

  async waitForTurn() {
    this.isWaiting = true;
    this.log('Waiting for Claude\'s message...');
    
    while (this.isWaiting) {
      const turn = this.getCurrentTurn();
      
      if (turn === 'CODEX') {
        const claudeMessage = this.checkForClaudeMessage();
        if (claudeMessage) {
          this.isWaiting = false;
          return claudeMessage;
        }
      }
      
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
  }

  async start() {
    console.log('Codex Interface Started!');
    console.log('This interface receives messages from Claude and forwards them to Codex CLI.');
    console.log('Make sure you have OPENAI_API_KEY set in your environment.\n');

    // Check if bridge is running
    if (!fs.existsSync(FILES.turnControl)) {
      console.log('ERROR: Bridge not detected. Please start the bridge script first:');
      console.log('node bridge.js');
      process.exit(1);
    }

    // Check for API key
    if (!process.env.OPENAI_API_KEY) {
      console.log('ERROR: OPENAI_API_KEY environment variable not set.');
      console.log('Please set it with: export OPENAI_API_KEY="your_key_here"');
      process.exit(1);
    }

    while (true) {
      try {
        // Wait for Claude's message
        const claudeMessage = await this.waitForTurn();
        
        if (claudeMessage.toLowerCase().includes('quit') || claudeMessage.toLowerCase().includes('exit')) {
          this.log('Received quit signal. Goodbye!');
          break;
        }
        
        // Run Codex with Claude's message
        this.log('Processing Claude\'s message with Codex...');
        const codexResponse = await this.runCodexCommand(claudeMessage);
        
        // Send Codex's response back to Claude
        this.sendResponseToClaude(codexResponse);
        
      } catch (error) {
        this.log(`Error: ${error.message}`);
        this.sendResponseToClaude(`Error occurred: ${error.message}`);
      }
    }
  }
}

const interface_instance = new CodexInterface();
interface_instance.start().catch(console.error);