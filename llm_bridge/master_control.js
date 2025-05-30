#!/usr/bin/env node

/**
 * Master Control Interface - Mixture-of-Idiots Method
 * Human-operated interface that can route commands to Claude or Codex
 * Or let them continue their AI-to-AI conversation
 */

import fs from 'fs';
import readline from 'readline';
import MixtureConfig from './mixture_config.js';

class MasterControl {
  constructor() {
    this.config = new MixtureConfig();
    this.files = this.config.getFiles();
    this.rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout
    });
    
    this.isRunning = false;
    this.conversationMode = 'AI_TO_AI'; // AI_TO_AI, DIRECTING_CLAUDE, DIRECTING_CODEX
    this.turnCount = 0;
    
    this.setupFiles();
    this.setupSignalHandling();
  }

  setupFiles() {
    // Ensure all message files exist
    Object.values(this.files).forEach(file => {
      if (!fs.existsSync(file)) {
        if (file.endsWith('.json')) {
          fs.writeFileSync(file, '{}');
        } else {
          fs.writeFileSync(file, '');
        }
      }
    });

    this.updateSystemStatus({
      mode: this.conversationMode,
      turnCount: this.turnCount,
      lastActivity: new Date().toISOString(),
      humanActive: true
    });
  }

  setupSignalHandling() {
    process.on('SIGINT', () => {
      console.log('\n👋 Shutting down Master Control...');
      this.shutdown();
    });
  }

  updateSystemStatus(status) {
    const currentStatus = this.getSystemStatus();
    const newStatus = { ...currentStatus, ...status };
    fs.writeFileSync(this.files.systemStatus, JSON.stringify(newStatus, null, 2));
  }

  getSystemStatus() {
    try {
      return JSON.parse(fs.readFileSync(this.files.systemStatus, 'utf8'));
    } catch {
      return {};
    }
  }

  log(message, type = 'info') {
    const timestamp = new Date().toISOString();
    const logEntry = `[${timestamp}] MASTER(${type.toUpperCase()}): ${message}\n`;
    
    console.log(`🎮 ${message}`);
    fs.appendFileSync(this.files.conversationLog, logEntry);
  }

  logConversation(speaker, message, context = '') {
    const timestamp = new Date().toISOString();
    const contextStr = context ? ` (${context})` : '';
    const logEntry = `[${timestamp}] ${speaker}${contextStr}: ${message}\n`;
    
    fs.appendFileSync(this.files.conversationLog, logEntry);
  }

  parseCommand(input) {
    const trimmed = input.trim();
    
    if (trimmed.startsWith('/claude ')) {
      return {
        type: 'DIRECT_CLAUDE',
        message: trimmed.substring(8).trim(),
        originalInput: trimmed
      };
    }
    
    if (trimmed.startsWith('/codex ')) {
      return {
        type: 'DIRECT_CODEX',
        message: trimmed.substring(7).trim(),
        originalInput: trimmed
      };
    }
    
    if (trimmed === '/status') {
      return { type: 'STATUS', originalInput: trimmed };
    }
    
    if (trimmed === '/help') {
      return { type: 'HELP', originalInput: trimmed };
    }
    
    if (trimmed === '/quit' || trimmed === '/exit') {
      return { type: 'QUIT', originalInput: trimmed };
    }
    
    if (trimmed === '/continue') {
      return { type: 'CONTINUE_AI', originalInput: trimmed };
    }
    
    if (trimmed === '/pause') {
      return { type: 'PAUSE_AI', originalInput: trimmed };
    }
    
    // Regular message - continue AI conversation
    return {
      type: 'CONTINUE_CONVERSATION',
      message: trimmed,
      originalInput: trimmed
    };
  }

  sendToSystem(command) {
    const message = {
      timestamp: new Date().toISOString(),
      command: command,
      turnCount: ++this.turnCount
    };
    
    fs.writeFileSync(this.files.masterToSystem, JSON.stringify(message, null, 2));
    this.logConversation('HUMAN_COMMAND', command.originalInput, command.type);
  }

  checkSystemResponse() {
    try {
      const response = fs.readFileSync(this.files.systemToMaster, 'utf8').trim();
      if (response) {
        const data = JSON.parse(response);
        console.log(`\n📋 System Response: ${data.message}`);
        
        // Clear the response after reading
        fs.writeFileSync(this.files.systemToMaster, '');
        return data;
      }
    } catch {
      // No response yet
    }
    return null;
  }

  showStatus() {
    const status = this.getSystemStatus();
    console.log('\n📊 MIXTURE-OF-IDIOTS STATUS:');
    console.log('═'.repeat(50));
    console.log(`Mode: ${status.mode || 'Unknown'}`);
    console.log(`Turn Count: ${status.turnCount || 0}`);
    console.log(`Last Activity: ${status.lastActivity || 'Never'}`);
    console.log(`Auto Continue: ${this.config.get('AUTO_CONTINUE_CONVERSATION')}`);
    console.log(`Max Turns: ${this.config.get('MAX_CONVERSATION_TURNS')}`);
    console.log('═'.repeat(50));
  }

  showHelp() {
    console.log('\n🤖 MIXTURE-OF-IDIOTS COMMANDS:');
    console.log('═'.repeat(50));
    console.log('/claude <message>   → Send message directly to Claude Code');
    console.log('/codex <message>    → Send message directly to Codex CLI');
    console.log('<regular message>   → Continue AI-to-AI conversation');
    console.log('/continue           → Resume AI-to-AI conversation');
    console.log('/pause              → Pause AI conversation');
    console.log('/status             → Show system status');
    console.log('/help               → Show this help');
    console.log('/quit               → Shutdown system');
    console.log('═'.repeat(50));
    console.log('💡 TIP: Type normal messages to let AIs continue talking!');
  }

  async promptUser() {
    return new Promise((resolve) => {
      this.rl.question('\n🎮 Master Control> ', (answer) => {
        resolve(answer);
      });
    });
  }

  async start() {
    console.log('🚀 MIXTURE-OF-IDIOTS MASTER CONTROL STARTED!');
    console.log('🧠 Control two AI coding assistants with intelligence routing');
    
    this.showHelp();
    this.isRunning = true;
    
    while (this.isRunning) {
      try {
        // Check for system responses
        this.checkSystemResponse();
        
        // Get user input
        const input = await this.promptUser();
        const command = this.parseCommand(input);
        
        switch (command.type) {
          case 'DIRECT_CLAUDE':
            this.log(`Routing command to Claude: "${command.message}"`);
            this.conversationMode = 'DIRECTING_CLAUDE';
            this.sendToSystem(command);
            break;
            
          case 'DIRECT_CODEX':
            this.log(`Routing command to Codex: "${command.message}"`);
            this.conversationMode = 'DIRECTING_CODEX';
            this.sendToSystem(command);
            break;
            
          case 'CONTINUE_CONVERSATION':
            this.log('Continuing AI-to-AI conversation');
            this.conversationMode = 'AI_TO_AI';
            this.sendToSystem(command);
            break;
            
          case 'STATUS':
            this.showStatus();
            break;
            
          case 'HELP':
            this.showHelp();
            break;
            
          case 'CONTINUE_AI':
            this.log('Resuming AI conversation');
            this.conversationMode = 'AI_TO_AI';
            this.sendToSystem(command);
            break;
            
          case 'PAUSE_AI':
            this.log('Pausing AI conversation');
            this.conversationMode = 'PAUSED';
            this.sendToSystem(command);
            break;
            
          case 'QUIT':
            this.isRunning = false;
            break;
            
          default:
            console.log('❓ Unknown command. Type /help for assistance.');
        }
        
        this.updateSystemStatus({
          mode: this.conversationMode,
          turnCount: this.turnCount,
          lastActivity: new Date().toISOString()
        });
        
      } catch (error) {
        console.error('❌ Error:', error.message);
      }
    }
    
    this.shutdown();
  }

  shutdown() {
    this.log('Master Control shutting down');
    this.updateSystemStatus({
      mode: 'SHUTDOWN',
      lastActivity: new Date().toISOString(),
      humanActive: false
    });
    
    this.rl.close();
    process.exit(0);
  }
}

const masterControl = new MasterControl();
masterControl.start().catch(console.error);