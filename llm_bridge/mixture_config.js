#!/usr/bin/env node

/**
 * Mixture-of-Idiots Configuration Manager
 * Loads .env file and manages system configuration
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

class MixtureConfig {
  constructor() {
    this.config = this.loadConfig();
    this.validateConfig();
  }

  loadConfig() {
    const envPath = path.join(__dirname, '.env');
    const config = {
      // Defaults
      OPENAI_API_KEY: '',
      CLAUDE_MODEL: 'claude-3-sonnet',
      CODEX_MODEL: 'o1-mini',
      AUTO_CONTINUE_CONVERSATION: true,
      LOG_LEVEL: 'info',
      MAX_CONVERSATION_TURNS: 50,
      BRIDGE_DIR: __dirname
    };

    // Load from .env file if it exists
    if (fs.existsSync(envPath)) {
      const envContent = fs.readFileSync(envPath, 'utf8');
      const lines = envContent.split('\n');
      
      for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed && !trimmed.startsWith('#')) {
          const [key, ...valueParts] = trimmed.split('=');
          if (key && valueParts.length > 0) {
            const value = valueParts.join('=').trim();
            
            // Convert string boolean values
            if (value.toLowerCase() === 'true') {
              config[key.trim()] = true;
            } else if (value.toLowerCase() === 'false') {
              config[key.trim()] = false;
            } else if (!isNaN(value)) {
              config[key.trim()] = parseInt(value);
            } else {
              config[key.trim()] = value;
            }
          }
        }
      }
    }

    // Override with environment variables
    Object.keys(config).forEach(key => {
      if (process.env[key]) {
        config[key] = process.env[key];
      }
    });

    return config;
  }

  validateConfig() {
    if (!this.config.OPENAI_API_KEY) {
      console.error('❌ ERROR: OPENAI_API_KEY not found!');
      console.error('Please create a .env file with your API key:');
      console.error('echo "OPENAI_API_KEY=your_key_here" > .env');
      process.exit(1);
    }

    if (!this.config.OPENAI_API_KEY.startsWith('sk-')) {
      console.error('❌ ERROR: Invalid OpenAI API key format');
      console.error('API key should start with "sk-"');
      process.exit(1);
    }

    console.log('✅ Configuration loaded successfully');
    console.log(`   API Key: ${this.config.OPENAI_API_KEY.substring(0, 10)}...`);
    console.log(`   Codex Model: ${this.config.CODEX_MODEL}`);
    console.log(`   Auto Continue: ${this.config.AUTO_CONTINUE_CONVERSATION}`);
  }

  get(key) {
    return this.config[key];
  }

  getFiles() {
    return {
      masterToSystem: path.join(this.config.BRIDGE_DIR, 'master_to_system.txt'),
      systemToMaster: path.join(this.config.BRIDGE_DIR, 'system_to_master.txt'),
      claudeToCodex: path.join(this.config.BRIDGE_DIR, 'claude_to_codex.txt'),
      codexToClaude: path.join(this.config.BRIDGE_DIR, 'codex_to_claude.txt'),
      conversationLog: path.join(this.config.BRIDGE_DIR, 'mixture_conversation.log'),
      systemStatus: path.join(this.config.BRIDGE_DIR, 'system_status.json'),
      currentContext: path.join(this.config.BRIDGE_DIR, 'current_context.txt')
    };
  }

  getAll() {
    return { ...this.config };
  }
}

export default MixtureConfig;