#!/usr/bin/env node

/**
 * Claude Interface - Allows Claude Code to participate in the bridge conversation
 */

import fs from 'fs';
import path from 'path';
import readline from 'readline';

const BRIDGE_DIR = '/mnt/c/Users/chris/codex/llm_bridge';
const FILES = {
  claudeToCodex: path.join(BRIDGE_DIR, 'claude_to_codex.txt'),
  codexToClaude: path.join(BRIDGE_DIR, 'codex_to_claude.txt'),
  turnControl: path.join(BRIDGE_DIR, 'turn_control.txt'),
  status: path.join(BRIDGE_DIR, 'bridge_status.txt')
};

class ClaudeInterface {
  constructor() {
    this.rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout
    });
    this.lastCodexMessage = '';
    this.isWaiting = false;
  }

  log(message) {
    console.log(`[CLAUDE INTERFACE] ${message}`);
  }

  getCurrentTurn() {
    try {
      return fs.readFileSync(FILES.turnControl, 'utf8').trim();
    } catch {
      return 'UNKNOWN';
    }
  }

  sendMessage(message) {
    fs.writeFileSync(FILES.claudeToCodex, message);
    this.log('Message sent to Codex');
  }

  checkForCodexMessage() {
    try {
      const message = fs.readFileSync(FILES.codexToClaude, 'utf8').trim();
      if (message && message !== this.lastCodexMessage) {
        this.lastCodexMessage = message;
        console.log('\n' + '='.repeat(60));
        console.log('CODEX SAYS:');
        console.log('='.repeat(60));
        console.log(message);
        console.log('='.repeat(60) + '\n');
        
        // Clear the message after reading
        fs.writeFileSync(FILES.codexToClaude, '');
        
        return true;
      }
    } catch (error) {
      // File might not exist yet
    }
    return false;
  }

  async promptForMessage() {
    return new Promise((resolve) => {
      this.rl.question('\n[YOUR TURN] Enter message for Codex (or "quit" to exit): ', (answer) => {
        resolve(answer.trim());
      });
    });
  }

  async waitForTurn() {
    this.isWaiting = true;
    this.log('Waiting for your turn...');
    
    while (this.isWaiting) {
      const turn = this.getCurrentTurn();
      
      if (turn === 'CLAUDE') {
        this.isWaiting = false;
        return true;
      }
      
      // Check for Codex messages while waiting
      this.checkForCodexMessage();
      
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
  }

  async start() {
    console.log('Claude Interface Started!');
    console.log('This interface lets you communicate with Codex CLI through the bridge.');
    console.log('You\'ll send messages that get forwarded to Codex, and see Codex\'s responses.\n');

    // Check if bridge is running
    if (!fs.existsSync(FILES.turnControl)) {
      console.log('ERROR: Bridge not detected. Please start the bridge script first:');
      console.log('node bridge.js');
      process.exit(1);
    }

    while (true) {
      // Wait for our turn
      await this.waitForTurn();
      
      // Check for any messages from Codex first
      this.checkForCodexMessage();
      
      // Get input from user (Claude Code operator)
      const message = await this.promptForMessage();
      
      if (message.toLowerCase() === 'quit') {
        this.log('Goodbye!');
        break;
      }
      
      if (message) {
        this.sendMessage(message);
      }
    }
    
    this.rl.close();
  }
}

const interface_instance = new ClaudeInterface();
interface_instance.start().catch(console.error);