/**
 * Tests for config.ts provider integration with provider-config.ts
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { join } from 'path';
import { mkdtempSync, rmSync, writeFileSync, mkdirSync } from 'fs';
import { tmpdir } from 'os';
import { loadConfig, saveConfig, CONFIG_DIR } from '../src/utils/config.js';
import { DEFAULT_PROVIDER_ID } from '../src/utils/provider-config.js';

describe('Config Provider Integration Tests', () => {
  let tempDir: string;
  let configDir: string;
  let configPath: string;
  let instructionsPath: string;

  beforeEach(() => {
    // Create a temporary directory for testing
    tempDir = mkdtempSync(join(tmpdir(), 'config-test-'));
    configDir = join(tempDir, '.codex');
    configPath = join(configDir, 'config.json');
    instructionsPath = join(configDir, 'instructions.md');
    
    // Create the config directory
    mkdirSync(configDir, { recursive: true });
    
    // Mock environment variables
    vi.stubEnv('OPENAI_API_KEY', 'test-openai-key');
    vi.stubEnv('CLAUDE_API_KEY', 'test-claude-key');
  });

  afterEach(() => {
    // Clean up temporary directory
    rmSync(tempDir, { recursive: true, force: true });
    
    // Clear environment variable mocks
    vi.unstubAllEnvs();
  });

  it('should load empty config with default values', () => {
    const config = loadConfig(configPath, instructionsPath);
    
    expect(config.defaultProvider).toBe(DEFAULT_PROVIDER_ID);
    expect(config.providers).toBeDefined();
    expect(Object.keys(config.providers).length).toBeGreaterThan(0);
  });

  it('should load environment variables into provider configs', () => {
    const config = loadConfig(configPath, instructionsPath);
    
    expect(config.providers.openai.apiKey).toBe('test-openai-key');
    expect(config.providers.claude.apiKey).toBe('test-claude-key');
  });

  it('should save and load provider configurations', () => {
    // Create a config object with provider settings
    const initialConfig = loadConfig(configPath, instructionsPath);
    initialConfig.providers.openai.defaultModel = 'gpt-4o';
    initialConfig.providers.claude.defaultModel = 'claude-3-opus';
    initialConfig.defaultProvider = 'claude';
    
    // Save the config
    saveConfig(initialConfig, configPath, instructionsPath);
    
    // Load the config again
    const loadedConfig = loadConfig(configPath, instructionsPath);
    
    // Verify provider config was saved and loaded correctly
    expect(loadedConfig.defaultProvider).toBe('claude');
    expect(loadedConfig.providers.openai.defaultModel).toBe('gpt-4o');
    expect(loadedConfig.providers.claude.defaultModel).toBe('claude-3-opus');
    
    // Environment variables should still be loaded
    expect(loadedConfig.providers.openai.apiKey).toBe('test-openai-key');
    expect(loadedConfig.providers.claude.apiKey).toBe('test-claude-key');
  });

  it('should use the provider ID from config as default', () => {
    // Create a config with a specific default provider
    const configContent = JSON.stringify({
      defaultProvider: 'claude',
      model: 'claude-3-sonnet'
    });
    
    writeFileSync(configPath, configContent);
    
    const config = loadConfig(configPath, instructionsPath);
    
    expect(config.defaultProvider).toBe('claude');
    expect(config.model).toBe('claude-3-sonnet');
  });

  it('should handle custom provider configurations', () => {
    // Create a config with custom provider settings
    const configContent = JSON.stringify({
      defaultProvider: 'openai',
      model: 'o4-mini',
      providers: {
        openai: {
          baseUrl: 'https://custom-openai-api.example.com',
          defaultModel: 'o4-mini',
          timeoutMs: 60000
        },
        claude: {
          baseUrl: 'https://custom-claude-api.example.com',
          defaultModel: 'claude-3-haiku',
          timeoutMs: 30000
        }
      }
    });
    
    writeFileSync(configPath, configContent);
    
    const config = loadConfig(configPath, instructionsPath);
    
    // Custom settings should be preserved
    expect(config.providers.openai.baseUrl).toBe('https://custom-openai-api.example.com');
    expect(config.providers.openai.defaultModel).toBe('o4-mini');
    expect(config.providers.openai.timeoutMs).toBe(60000);
    
    expect(config.providers.claude.baseUrl).toBe('https://custom-claude-api.example.com');
    expect(config.providers.claude.defaultModel).toBe('claude-3-haiku');
    expect(config.providers.claude.timeoutMs).toBe(30000);
    
    // Environment variables should take precedence for API keys
    expect(config.providers.openai.apiKey).toBe('test-openai-key');
    expect(config.providers.claude.apiKey).toBe('test-claude-key');
  });
});