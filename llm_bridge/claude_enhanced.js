#!/usr/bin/env node

/**
 * Enhanced Claude Interface - Mixture-of-Idiots Method
 * Connects Claude Code to the smart bridge system
 * Handles both direct human commands and AI-to-AI conversation
 */

import fs from 'fs';
import MixtureConfig from './mixture_config.js';

class ClaudeEnhanced {
  constructor() {
    this.config = new MixtureConfig();
    this.files = this.config.getFiles();
    this.lastMessageFromCodex = '';
    this.isWaiting = false;
  }

  log(message, type = 'info') {
    const timestamp = new Date().toISOString();
    const logEntry = `[${timestamp}] CLAUDE_ENHANCED(${type.toUpperCase()}): ${message}\n`;
    
    console.log(`🔵 Claude: ${message}`);
    fs.appendFileSync(this.files.conversationLog, logEntry);
  }

  logConversation(speaker, message) {
    const timestamp = new Date().toISOString();
    const logEntry = `[${timestamp}] ${speaker}: ${message}\n`;
    
    fs.appendFileSync(this.files.conversationLog, logEntry);
  }

  checkForCodexMessage() {
    try {
      const message = fs.readFileSync(this.files.codexToClaude, 'utf8').trim();
      if (message && message !== this.lastMessageFromCodex) {
        this.lastMessageFromCodex = message;
        
        console.log('\n' + '='.repeat(80));
        console.log('🤖 CODEX SAYS:');
        console.log('='.repeat(80));
        console.log(message);
        console.log('='.repeat(80));
        
        // Clear the message after reading
        fs.writeFileSync(this.files.codexToClaude, '');
        
        return message;
      }
    } catch (error) {
      // File might not exist yet
    }
    return null;
  }

  generateClaudeResponse(codexMessage) {
    // This is where I (Claude) would generate my response
    // For this demo, I'll create contextual responses based on what Codex said
    
    const responses = this.analyzeCodexMessage(codexMessage);
    return responses.response;
  }

  analyzeCodexMessage(message) {
    // Analyze Codex's message and generate appropriate response
    const lowerMessage = message.toLowerCase();
    
    if (lowerMessage.includes('hello') || lowerMessage.includes('hi')) {
      return {
        type: 'greeting',
        response: `Hello! I'm Claude Code. I can see you're Codex CLI. I appreciate you connecting through this bridge system. I'm particularly good at reading and analyzing code, explaining complex concepts, and helping with architecture decisions. What kind of project would you like to collaborate on?`
      };
    }
    
    if (lowerMessage.includes('build') || lowerMessage.includes('create') || lowerMessage.includes('project')) {
      return {
        type: 'project_discussion',
        response: `Excellent! I love collaborative projects. I can help with code analysis, architecture planning, debugging, and documentation. I notice you can execute commands and interact with the file system directly. Should we start by exploring the current codebase structure, or do you have a specific new project in mind? I can analyze what's already here and suggest improvements or extensions.`
      };
    }
    
    if (lowerMessage.includes('code') || lowerMessage.includes('function') || lowerMessage.includes('file')) {
      return {
        type: 'code_analysis',
        response: `I see you're working with code. I can help analyze, review, and improve it. I'm particularly good at spotting patterns, suggesting optimizations, and explaining complex logic. Would you like me to examine any specific files or help design new functionality? I can also help with testing strategies and documentation.`
      };
    }
    
    if (lowerMessage.includes('error') || lowerMessage.includes('bug') || lowerMessage.includes('issue')) {
      return {
        type: 'debugging',
        response: `I'd be happy to help debug! I can analyze error messages, trace through code logic, and suggest fixes. Could you share the specific error or describe what's not working as expected? I can also help implement proper error handling and logging to prevent similar issues.`
      };
    }
    
    if (lowerMessage.includes('test') || lowerMessage.includes('testing')) {
      return {
        type: 'testing',
        response: `Testing is crucial! I can help design test strategies, write test cases, and set up testing frameworks. I notice this codebase uses Vitest for testing. Should we add more comprehensive tests, or are you working on testing a specific feature? I can also help with integration tests and test automation.`
      };
    }
    
    // Default response - continue conversation naturally
    return {
      type: 'general',
      response: `That's interesting! I can help analyze and expand on that. Based on what you've shared, I think we could approach this systematically. What specific aspect would you like to focus on first? I'm ready to dive into code analysis, architecture planning, or whatever direction you'd like to take this collaboration.`
    };
  }

  sendResponseToCodex(response) {
    fs.writeFileSync(this.files.claudeToCodex, response);
    this.log('Response sent to Codex');
    this.logConversation('CLAUDE→CODEX', response);
  }

  getSystemStatus() {
    try {
      return JSON.parse(fs.readFileSync(this.files.systemStatus, 'utf8'));
    } catch {
      return { mode: 'AI_TO_AI' };
    }
  }

  async waitAndRespond() {
    this.log('Waiting for messages from Codex...');
    
    while (true) {
      const status = this.getSystemStatus();
      
      // Check if we should be active
      if (status.mode === 'SHUTDOWN') {
        this.log('System shutdown detected, exiting');
        break;
      }
      
      if (status.mode === 'PAUSED') {
        this.log('System paused, waiting...');
        await new Promise(resolve => setTimeout(resolve, 2000));
        continue;
      }
      
      // Check for messages from Codex
      const codexMessage = this.checkForCodexMessage();
      
      if (codexMessage) {
        this.log('Processing Codex message...');
        
        // Generate my response
        const myResponse = this.generateClaudeResponse(codexMessage);
        
        // Send response back to Codex
        this.sendResponseToCodex(myResponse);
        
        this.log('Response cycle complete');
      }
      
      // Don't spin the CPU
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
  }

  async start() {
    console.log('🔵 CLAUDE ENHANCED INTERFACE STARTED!');
    console.log('Connected to Mixture-of-Idiots bridge system');
    console.log('Waiting for messages from Codex or human commands...\n');

    // Check if bridge is running
    if (!fs.existsSync(this.files.systemStatus)) {
      console.log('❌ ERROR: Bridge not detected. Please start the smart bridge first:');
      console.log('node smart_bridge.js');
      process.exit(1);
    }

    await this.waitAndRespond();
  }
}

const claudeInterface = new ClaudeEnhanced();
claudeInterface.start().catch(console.error);