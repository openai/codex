#!/usr/bin/env node

/**
 * Smart Bridge - Mixture-of-Idiots Method
 * Intelligent routing between Master Control, Claude, and Codex
 * Handles command routing and AI-to-AI conversation management
 */

import fs from 'fs';
import MixtureConfig from './mixture_config.js';

class SmartBridge {
  constructor() {
    this.config = new MixtureConfig();
    this.files = this.config.getFiles();
    this.isRunning = false;
    
    this.state = {
      currentSpeaker: 'CLAUDE', // CLAUDE, CODEX, HUMAN
      conversationMode: 'AI_TO_AI', // AI_TO_AI, DIRECTING_CLAUDE, DIRECTING_CODEX, PAUSED
      turnCount: 0,
      lastMasterCommand: null,
      lastClaudeMessage: '',
      lastCodexMessage: ''
    };
    
    this.setupFiles();
  }

  setupFiles() {
    Object.values(this.files).forEach(file => {
      if (!fs.existsSync(file)) {
        if (file.endsWith('.json')) {
          fs.writeFileSync(file, '{}');
        } else {
          fs.writeFileSync(file, '');
        }
      }
    });
    
    this.log('Smart Bridge initialized');
  }

  log(message, type = 'info') {
    const timestamp = new Date().toISOString();
    const logEntry = `[${timestamp}] BRIDGE(${type.toUpperCase()}): ${message}\n`;
    
    console.log(`🌉 ${message}`);
    fs.appendFileSync(this.files.conversationLog, logEntry);
  }

  logConversation(speaker, message, context = '') {
    const timestamp = new Date().toISOString();
    const contextStr = context ? ` (${context})` : '';
    const logEntry = `[${timestamp}] ${speaker}${contextStr}: ${message}\n`;
    
    fs.appendFileSync(this.files.conversationLog, logEntry);
    
    // Also log to console with truncation
    const preview = message.length > 100 ? message.substring(0, 100) + '...' : message;
    console.log(`💬 ${speaker}: ${preview}`);
  }

  sendResponseToMaster(message) {
    const response = {
      timestamp: new Date().toISOString(),
      message: message,
      state: this.state
    };
    
    fs.writeFileSync(this.files.systemToMaster, JSON.stringify(response, null, 2));
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

  checkMasterCommand() {
    try {
      const content = this.readMessage(this.files.masterToSystem);
      if (content && content !== this.state.lastMasterCommand) {
        const command = JSON.parse(content);
        this.state.lastMasterCommand = content;
        this.clearMessage(this.files.masterToSystem);
        return command;
      }
    } catch (error) {
      this.log(`Error reading master command: ${error.message}`, 'error');
    }
    return null;
  }

  processMasterCommand(command) {
    this.log(`Processing master command: ${command.command.type}`);
    
    switch (command.command.type) {
      case 'DIRECT_CLAUDE':
        this.sendToClaudeDirectly(command.command.message);
        this.state.conversationMode = 'DIRECTING_CLAUDE';
        this.sendResponseToMaster('Command sent to Claude');
        break;
        
      case 'DIRECT_CODEX':
        this.sendToCodexDirectly(command.command.message);
        this.state.conversationMode = 'DIRECTING_CODEX';
        this.sendResponseToMaster('Command sent to Codex');
        break;
        
      case 'CONTINUE_CONVERSATION':
        this.continueAIConversation(command.command.message);
        this.state.conversationMode = 'AI_TO_AI';
        this.sendResponseToMaster('AI conversation continuing');
        break;
        
      case 'CONTINUE_AI':
        this.state.conversationMode = 'AI_TO_AI';
        this.sendResponseToMaster('AI conversation resumed');
        break;
        
      case 'PAUSE_AI':
        this.state.conversationMode = 'PAUSED';
        this.sendResponseToMaster('AI conversation paused');
        break;
        
      default:
        this.sendResponseToMaster(`Unknown command: ${command.command.type}`);
    }
    
    this.state.turnCount = command.turnCount;
  }

  sendToClaudeDirectly(message) {
    fs.writeFileSync(this.files.codexToClaude, message);
    this.logConversation('HUMAN→CLAUDE', message, 'direct command');
    this.state.currentSpeaker = 'CLAUDE';
  }

  sendToCodexDirectly(message) {
    fs.writeFileSync(this.files.claudeToCodex, message);
    this.logConversation('HUMAN→CODEX', message, 'direct command');
    this.state.currentSpeaker = 'CODEX';
  }

  continueAIConversation(humanMessage) {
    // If human provided a message, inject it into the conversation
    if (humanMessage) {
      // Send to whoever should speak next
      if (this.state.currentSpeaker === 'CLAUDE') {
        this.sendToClaudeDirectly(humanMessage);
      } else {
        this.sendToCodexDirectly(humanMessage);
      }
    } else {
      // Just continue the AI conversation
      this.log('Continuing AI-to-AI conversation');
    }
  }

  checkClaudeMessage() {
    const message = this.readMessage(this.files.claudeToCodex);
    if (message && message !== this.state.lastClaudeMessage) {
      this.state.lastClaudeMessage = message;
      this.logConversation('CLAUDE→CODEX', message);
      this.state.currentSpeaker = 'CODEX';
      
      // In AI_TO_AI mode, automatically forward to Codex
      if (this.state.conversationMode === 'AI_TO_AI') {
        // Message is already in the right file for Codex to pick up
        this.log('Forwarded Claude message to Codex');
      }
      
      return true;
    }
    return false;
  }

  checkCodexMessage() {
    const message = this.readMessage(this.files.codexToClaude);
    if (message && message !== this.state.lastCodexMessage) {
      this.state.lastCodexMessage = message;
      this.logConversation('CODEX→CLAUDE', message);
      this.state.currentSpeaker = 'CLAUDE';
      
      // In AI_TO_AI mode, automatically forward to Claude
      if (this.state.conversationMode === 'AI_TO_AI') {
        // Message is already in the right file for Claude to pick up
        this.log('Forwarded Codex message to Claude');
      }
      
      return true;
    }
    return false;
  }

  updateSystemStatus() {
    const status = {
      mode: this.state.conversationMode,
      turnCount: this.state.turnCount,
      currentSpeaker: this.state.currentSpeaker,
      lastActivity: new Date().toISOString(),
      bridgeActive: true
    };
    
    try {
      fs.writeFileSync(this.files.systemStatus, JSON.stringify(status, null, 2));
    } catch (error) {
      this.log(`Error updating status: ${error.message}`, 'error');
    }
  }

  async start() {
    this.isRunning = true;
    this.log('Smart Bridge started - monitoring all channels');
    
    // Initialize first conversation
    const initialMessage = "Hello! I'm Claude Code connected through the Mixture-of-Idiots bridge system. I can see you're Codex CLI. A human operator can direct either of us with /claude or /codex commands, or let us continue talking. What would you like to work on together?";
    this.sendToCodexDirectly(initialMessage);
    
    while (this.isRunning) {
      try {
        // Check for master commands (highest priority)
        const masterCommand = this.checkMasterCommand();
        if (masterCommand) {
          this.processMasterCommand(masterCommand);
        }
        
        // Check for AI messages (if in appropriate mode)
        if (this.state.conversationMode === 'AI_TO_AI' || 
            this.state.conversationMode === 'DIRECTING_CLAUDE' ||
            this.state.conversationMode === 'DIRECTING_CODEX') {
          
          this.checkClaudeMessage();
          this.checkCodexMessage();
        }
        
        // Update system status
        this.updateSystemStatus();
        
        // Prevent CPU spinning
        await new Promise(resolve => setTimeout(resolve, 500));
        
      } catch (error) {
        this.log(`Bridge error: ${error.message}`, 'error');
      }
    }
  }

  stop() {
    this.isRunning = false;
    this.log('Smart Bridge stopped');
    
    // Update final status
    const status = {
      mode: 'SHUTDOWN',
      lastActivity: new Date().toISOString(),
      bridgeActive: false
    };
    fs.writeFileSync(this.files.systemStatus, JSON.stringify(status, null, 2));
  }
}

// Handle graceful shutdown
const bridge = new SmartBridge();

process.on('SIGINT', () => {
  console.log('\n🌉 Shutting down Smart Bridge...');
  bridge.stop();
  process.exit(0);
});

bridge.start().catch(console.error);