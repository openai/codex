#!/usr/bin/env node

/**
 * LLM Bridge Script - Orchestrates conversation between Claude Code and Codex CLI
 */

import fs from 'fs';
import path from 'path';

const BRIDGE_DIR = '/mnt/c/Users/chris/codex/llm_bridge';
const FILES = {
  claudeToCodex: path.join(BRIDGE_DIR, 'claude_to_codex.txt'),
  codexToClaude: path.join(BRIDGE_DIR, 'codex_to_claude.txt'),
  conversationLog: path.join(BRIDGE_DIR, 'conversation_log.txt'),
  turnControl: path.join(BRIDGE_DIR, 'turn_control.txt'),
  status: path.join(BRIDGE_DIR, 'bridge_status.txt')
};

class LLMBridge {
  constructor() {
    this.isRunning = false;
    this.conversationTurn = 1;
    this.lastClaudeMessage = '';
    this.lastCodexMessage = '';
    
    this.setupFiles();
  }

  setupFiles() {
    // Ensure bridge directory exists
    if (!fs.existsSync(BRIDGE_DIR)) {
      fs.mkdirSync(BRIDGE_DIR, { recursive: true });
    }

    // Initialize files
    Object.values(FILES).forEach(file => {
      if (!fs.existsSync(file)) {
        fs.writeFileSync(file, '');
      }
    });

    // Set initial turn (Claude starts)
    fs.writeFileSync(FILES.turnControl, 'CLAUDE');
    fs.writeFileSync(FILES.status, 'WAITING_FOR_CLAUDE');
    
    this.log('Bridge initialized. Waiting for Claude to start...');
  }

  log(message) {
    const timestamp = new Date().toISOString();
    const logEntry = `[${timestamp}] BRIDGE: ${message}\n`;
    
    console.log(logEntry.trim());
    fs.appendFileSync(FILES.conversationLog, logEntry);
  }

  logConversation(speaker, message) {
    const timestamp = new Date().toISOString();
    const logEntry = `[${timestamp}] ${speaker}: ${message}\n`;
    
    fs.appendFileSync(FILES.conversationLog, logEntry);
    console.log(`${speaker}: ${message.substring(0, 100)}${message.length > 100 ? '...' : ''}`);
  }

  getCurrentTurn() {
    try {
      return fs.readFileSync(FILES.turnControl, 'utf8').trim();
    } catch {
      return 'CLAUDE';
    }
  }

  setTurn(who) {
    fs.writeFileSync(FILES.turnControl, who);
    fs.writeFileSync(FILES.status, `WAITING_FOR_${who}`);
  }

  readMessage(file) {
    try {
      const content = fs.readFileSync(file, 'utf8').trim();
      return content || null;
    } catch {
      return null;
    }
  }

  clearMessage(file) {
    fs.writeFileSync(file, '');
  }

  async processClaudeMessage() {
    const message = this.readMessage(FILES.claudeToCodex);
    if (!message || message === this.lastClaudeMessage) {
      return false;
    }

    this.lastClaudeMessage = message;
    this.logConversation('CLAUDE', message);
    
    // Signal that Claude's message is ready for Codex
    this.setTurn('CODEX');
    this.log(`Turn ${this.conversationTurn}: Claude → Codex`);
    
    return true;
  }

  async processCodexMessage() {
    const message = this.readMessage(FILES.codexToClaude);
    if (!message || message === this.lastCodexMessage) {
      return false;
    }

    this.lastCodexMessage = message;
    this.logConversation('CODEX', message);
    
    // Clear the Codex message and signal Claude's turn
    this.setTurn('CLAUDE');
    this.conversationTurn++;
    this.log(`Turn ${this.conversationTurn}: Codex → Claude`);
    
    return true;
  }

  async start() {
    this.isRunning = true;
    this.log('Bridge started! Monitoring conversation...');

    while (this.isRunning) {
      const currentTurn = this.getCurrentTurn();

      if (currentTurn === 'CLAUDE') {
        await this.processClaudeMessage();
      } else if (currentTurn === 'CODEX') {
        await this.processCodexMessage();
      }

      // Small delay to prevent excessive CPU usage
      await new Promise(resolve => setTimeout(resolve, 500));
    }
  }

  stop() {
    this.isRunning = false;
    this.log('Bridge stopped.');
  }
}

// Handle graceful shutdown
const bridge = new LLMBridge();

process.on('SIGINT', () => {
  console.log('\nShutting down bridge...');
  bridge.stop();
  process.exit(0);
});

process.on('SIGTERM', () => {
  bridge.stop();
  process.exit(0);
});

// Start the bridge
bridge.start().catch(console.error);