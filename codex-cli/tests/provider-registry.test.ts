/**
 * Tests for the provider registry
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { ProviderRegistry, initializeProviderRegistry } from '../src/utils/providers';
import { DEFAULT_PROVIDER_ID } from '../src/utils/provider-config';

describe('Provider Registry', () => {
  beforeEach(() => {
    // Clear the registry before each test
    ProviderRegistry.clearProviders();
  });

  describe('initialization', () => {
    beforeEach(() => {
      // Save the original environment variable
      this.originalEnv = process.env.CODEX_DEFAULT_PROVIDER;
      
      // Clear environment variable before each test
      delete process.env.CODEX_DEFAULT_PROVIDER;
    });
    
    afterEach(() => {
      // Restore the original environment variable
      if (this.originalEnv) {
        process.env.CODEX_DEFAULT_PROVIDER = this.originalEnv;
      } else {
        delete process.env.CODEX_DEFAULT_PROVIDER;
      }
    });
    
    it('should successfully register all providers', () => {
      // Initialize the registry
      initializeProviderRegistry();
      
      // Check that providers are registered
      expect(ProviderRegistry.hasProvider('openai')).toBe(true);
      expect(ProviderRegistry.hasProvider('claude')).toBe(true);
      
      // Check that we have at least two providers
      expect(ProviderRegistry.getAllProviders().length).toBeGreaterThanOrEqual(2);
    });
    
    it('should use default provider when environment variable is not set', () => {
      // Make sure env var is not set
      delete process.env.CODEX_DEFAULT_PROVIDER;
      
      // Initialize the registry
      initializeProviderRegistry();
      
      // Check default provider is the one from provider-config.js
      expect(ProviderRegistry.getDefaultProviderId()).toBe(DEFAULT_PROVIDER_ID);
    });
    
    it('should set the default provider from environment when available', () => {
      // Set environment variable
      process.env.CODEX_DEFAULT_PROVIDER = 'claude';
      
      // Initialize the registry
      initializeProviderRegistry();
      
      // Check default provider
      expect(ProviderRegistry.getDefaultProviderId()).toBe('claude');
    });
  });

  describe('getProviderForModel', () => {
    beforeEach(() => {
      // Initialize with providers for testing
      initializeProviderRegistry();
    });
    
    it('should return OpenAI provider for OpenAI models', () => {
      const openaiModels = ['gpt-4', 'o4-mini', 'gpt-3.5-turbo', 'o3'];
      
      for (const model of openaiModels) {
        const provider = ProviderRegistry.getProviderForModel(model);
        expect(provider.id).toBe('openai');
      }
    });
    
    it('should return Claude provider for Claude models', () => {
      const claudeModels = [
        'claude-3-opus-20240229',
        'claude-3-sonnet-20240229',
        'claude-3-haiku-20240307',
        'claude-2',
      ];
      
      for (const model of claudeModels) {
        const provider = ProviderRegistry.getProviderForModel(model);
        expect(provider.id).toBe('claude');
      }
    });
    
    it('should return default provider for unknown models', () => {
      // Save original env var
      const originalEnv = process.env.CODEX_DEFAULT_PROVIDER;
      
      try {
        // Clear environment variable to ensure we use the default from provider-config
        delete process.env.CODEX_DEFAULT_PROVIDER;
        
        // Re-initialize with the correct default
        initializeProviderRegistry();
        
        const provider = ProviderRegistry.getProviderForModel('unknown-model');
        expect(provider.id).toBe(DEFAULT_PROVIDER_ID);
      } finally {
        // Restore original env var
        if (originalEnv) {
          process.env.CODEX_DEFAULT_PROVIDER = originalEnv;
        } else {
          delete process.env.CODEX_DEFAULT_PROVIDER;
        }
      }
    });
  });

  describe('provider management', () => {
    it('should allow getting all registered providers', () => {
      // Register mock providers
      ProviderRegistry.register({ id: 'test1', name: 'Test 1' } as any);
      ProviderRegistry.register({ id: 'test2', name: 'Test 2' } as any);
      
      const providers = ProviderRegistry.getAllProviders();
      expect(providers.length).toBe(2);
      expect(providers.map(p => p.id).sort()).toEqual(['test1', 'test2']);
    });
    
    it('should return undefined for non-existent providers', () => {
      expect(ProviderRegistry.getProviderById('non-existent')).toBeUndefined();
    });
    
    it('should throw an error when setting a non-existent provider as default', () => {
      expect(() => {
        ProviderRegistry.setDefaultProviderId('non-existent');
      }).toThrow();
    });
  });
});
