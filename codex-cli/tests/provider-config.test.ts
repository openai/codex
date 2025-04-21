import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { 
  loadProviderConfig, 
  loadAllProviderConfigs,
  getProviderIdForModel,
  getDefaultModelForProvider,
  DEFAULT_PROVIDER_MODELS
} from '../src/utils/provider-config';

describe('Provider Configuration System', () => {
  // Clear environment variables before each test
  beforeEach(() => {
    vi.resetModules();
    
    // Clear all provider-related environment variables
    vi.stubEnv('OPENAI_API_KEY', '');
    vi.stubEnv('OPENAI_BASE_URL', '');
    vi.stubEnv('OPENAI_TIMEOUT_MS', '');
    
    vi.stubEnv('CLAUDE_API_KEY', '');
    vi.stubEnv('ANTHROPIC_API_KEY', '');
    vi.stubEnv('CLAUDE_BASE_URL', '');
    vi.stubEnv('CLAUDE_TIMEOUT_MS', '');
    
    vi.stubEnv('CODEX_DEFAULT_PROVIDER', '');
  });
  
  afterEach(() => {
    vi.unstubAllEnvs();
  });
  
  describe('loadProviderConfig', () => {
    it('should load configuration from environment variables', () => {
      // Set environment variables
      vi.stubEnv('OPENAI_API_KEY', 'test-api-key');
      vi.stubEnv('OPENAI_BASE_URL', 'https://test-api.openai.com');
      vi.stubEnv('OPENAI_TIMEOUT_MS', '60000');
      
      const config = loadProviderConfig('openai');
      
      expect(config.apiKey).toBe('test-api-key');
      expect(config.baseUrl).toBe('https://test-api.openai.com');
      expect(config.timeoutMs).toBe(60000);
    });
    
    it('should handle multiple environment variable options', () => {
      // Set environment variables for the second option
      vi.stubEnv('ANTHROPIC_API_KEY', 'test-anthropic-key');
      vi.stubEnv('CLAUDE_BASE_URL', 'https://test-api.anthropic.com');
      
      const config = loadProviderConfig('claude');
      
      // Should pick up API key from ANTHROPIC_API_KEY
      expect(config.apiKey).toBe('test-anthropic-key');
      // Should pick up base URL from CLAUDE_BASE_URL
      expect(config.baseUrl).toBe('https://test-api.anthropic.com');
    });
    
    it('should prioritize environment variables over stored config', () => {
      // Set environment variables
      vi.stubEnv('OPENAI_API_KEY', 'env-api-key');
      
      // Create stored config
      const storedConfig = {
        apiKey: 'stored-api-key',
        baseUrl: 'https://stored-api.openai.com',
      };
      
      const config = loadProviderConfig('openai', storedConfig);
      
      // API key should come from environment
      expect(config.apiKey).toBe('env-api-key');
      // Base URL should come from stored config
      expect(config.baseUrl).toBe('https://stored-api.openai.com');
    });
    
    it('should handle invalid timeoutMs values', () => {
      // Set invalid timeout value
      vi.stubEnv('OPENAI_TIMEOUT_MS', 'not-a-number');
      
      const config = loadProviderConfig('openai');
      
      // Should be undefined when parsing fails
      expect(config.timeoutMs).toBeUndefined();
    });
    
    it('should handle unknown provider IDs', () => {
      const config = loadProviderConfig('unknown-provider');
      
      // Should return an empty config object
      expect(config).toEqual({});
    });
  });
  
  describe('loadAllProviderConfigs', () => {
    it('should load configurations for all providers', () => {
      // Set environment variables
      vi.stubEnv('OPENAI_API_KEY', 'openai-key');
      vi.stubEnv('CLAUDE_API_KEY', 'claude-key');
      
      const configs = loadAllProviderConfigs();
      
      expect(configs.openai.apiKey).toBe('openai-key');
      expect(configs.claude.apiKey).toBe('claude-key');
    });
    
    it('should merge with stored provider configs', () => {
      // Set environment variables
      vi.stubEnv('OPENAI_API_KEY', 'env-openai-key');
      
      // Create stored configs
      const storedConfigs = {
        openai: {
          baseUrl: 'https://stored.openai.com',
        },
        claude: {
          apiKey: 'stored-claude-key',
          baseUrl: 'https://stored.anthropic.com',
        },
      };
      
      const configs = loadAllProviderConfigs(storedConfigs);
      
      // OpenAI config should be merged
      expect(configs.openai.apiKey).toBe('env-openai-key');
      expect(configs.openai.baseUrl).toBe('https://stored.openai.com');
      
      // Claude config should come from stored since no env vars
      expect(configs.claude.apiKey).toBe('stored-claude-key');
      expect(configs.claude.baseUrl).toBe('https://stored.anthropic.com');
    });
    
    it('should include providers from both sources', () => {
      // Set environment variables for known provider
      vi.stubEnv('OPENAI_API_KEY', 'openai-key');
      
      // Create stored config with custom provider
      const storedConfigs = {
        'custom-provider': {
          apiKey: 'custom-key',
          baseUrl: 'https://custom-api.example.com',
        },
      };
      
      const configs = loadAllProviderConfigs(storedConfigs);
      
      // Both providers should be included
      expect(configs.openai.apiKey).toBe('openai-key');
      expect(configs['custom-provider'].apiKey).toBe('custom-key');
    });
  });
  
  describe('getProviderIdForModel', () => {
    it('should identify OpenAI models', () => {
      expect(getProviderIdForModel('gpt-4')).toBe('openai');
      expect(getProviderIdForModel('o4-mini')).toBe('openai');
      expect(getProviderIdForModel('gpt-3.5-turbo')).toBe('openai');
      expect(getProviderIdForModel('text-davinci-003')).toBe('openai');
    });
    
    it('should identify Claude models', () => {
      expect(getProviderIdForModel('claude-3-opus-20240229')).toBe('claude');
      expect(getProviderIdForModel('claude-3-sonnet-20240229')).toBe('claude');
      expect(getProviderIdForModel('claude-2')).toBe('claude');
    });
    
    it('should default to OpenAI for unknown models', () => {
      expect(getProviderIdForModel('unknown-model')).toBe('openai');
      expect(getProviderIdForModel('')).toBe('openai');
    });
  });
  
  describe('getDefaultModelForProvider', () => {
    it('should return the default model for known providers', () => {
      expect(getDefaultModelForProvider('openai')).toBe(DEFAULT_PROVIDER_MODELS.openai);
      expect(getDefaultModelForProvider('claude')).toBe(DEFAULT_PROVIDER_MODELS.claude);
    });
    
    it('should fallback to OpenAI default for unknown providers', () => {
      expect(getDefaultModelForProvider('unknown-provider')).toBe(DEFAULT_PROVIDER_MODELS.openai);
    });
  });
});