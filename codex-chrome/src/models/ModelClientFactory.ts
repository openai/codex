/**
 * Model Client Factory for codex-chrome
 * Creates and manages model client instances with provider selection and caching
 */

import { ModelClient, ModelClientError, type RetryConfig } from './ModelClient';
import { OpenAIClient } from './OpenAIClient';
import { AnthropicClient } from './AnthropicClient';

/**
 * Supported model providers
 */
export type ModelProvider = 'openai' | 'anthropic';

/**
 * Configuration for model client creation
 */
export interface ModelClientConfig {
  /** Provider to use */
  provider: ModelProvider;
  /** API key for the provider */
  apiKey: string;
  /** Additional provider-specific options */
  options?: {
    /** Base URL for API requests (optional) */
    baseUrl?: string;
    /** Organization ID (OpenAI only) */
    organization?: string;
    /** API version (Anthropic only) */
    version?: string;
  };
}

/**
 * Storage keys for Chrome storage
 */
const STORAGE_KEYS = {
  OPENAI_API_KEY: 'openai_api_key',
  ANTHROPIC_API_KEY: 'anthropic_api_key',
  DEFAULT_PROVIDER: 'default_provider',
  OPENAI_ORGANIZATION: 'openai_organization',
  ANTHROPIC_VERSION: 'anthropic_version',
} as const;

/**
 * Model name to provider mapping
 */
const MODEL_PROVIDER_MAP: Record<string, ModelProvider> = {
  // OpenAI models
  'gpt-4': 'openai',
  'gpt-4-turbo': 'openai',
  'gpt-4o': 'openai',
  'gpt-3.5-turbo': 'openai',
  'gpt-3.5-turbo-16k': 'openai',

  // Anthropic models
  'claude-3-opus-20240229': 'anthropic',
  'claude-3-sonnet-20240229': 'anthropic',
  'claude-3-haiku-20240307': 'anthropic',
  'claude-3-5-sonnet-20240620': 'anthropic',
  'claude-3-5-haiku-20241022': 'anthropic',
};

/**
 * Factory for creating and managing model clients
 */
export class ModelClientFactory {
  private static instance: ModelClientFactory;
  private clientCache: Map<string, ModelClient> = new Map();

  private constructor() {}

  /**
   * Get the singleton instance of the factory
   */
  static getInstance(): ModelClientFactory {
    if (!ModelClientFactory.instance) {
      ModelClientFactory.instance = new ModelClientFactory();
    }
    return ModelClientFactory.instance;
  }

  /**
   * Create a model client for the specified model
   * @param model The model name to create a client for
   * @returns Promise resolving to a model client
   */
  async createClientForModel(model: string): Promise<ModelClient> {
    const provider = this.getProviderForModel(model);
    return this.createClient(provider);
  }

  /**
   * Create a model client for the specified provider
   * @param provider The provider to create a client for
   * @returns Promise resolving to a model client
   */
  async createClient(provider: ModelProvider): Promise<ModelClient> {
    // Check cache first
    const cached = this.clientCache.get(provider);
    if (cached) {
      return cached;
    }

    const config = await this.loadConfigForProvider(provider);
    const client = this.instantiateClient(config);

    // Cache the client instance
    this.clientCache.set(provider, client);

    return client;
  }

  /**
   * Create a client with explicit configuration
   * @param config The client configuration
   * @returns Model client instance
   */
  createClientWithConfig(config: ModelClientConfig): ModelClient {
    const cacheKey = `${config.provider}-${this.hashConfig(config)}`;

    // Check cache first
    const cached = this.clientCache.get(cacheKey);
    if (cached) {
      return cached;
    }

    const client = this.instantiateClient(config);

    // Cache the client instance
    this.clientCache.set(cacheKey, client);

    return client;
  }

  /**
   * Get the provider for a given model name
   * @param model The model name
   * @returns The provider for the model
   */
  getProviderForModel(model: string): ModelProvider {
    const provider = MODEL_PROVIDER_MAP[model];

    if (!provider) {
      // Try to infer from model name patterns
      if (model.startsWith('gpt-')) {
        return 'openai';
      } else if (model.startsWith('claude-')) {
        return 'anthropic';
      }

      throw new ModelClientError(`Unknown model: ${model}. Cannot determine provider.`);
    }

    return provider;
  }

  /**
   * Get all supported models for a provider
   * @param provider The provider
   * @returns Array of model names
   */
  getSupportedModels(provider: ModelProvider): string[] {
    return Object.entries(MODEL_PROVIDER_MAP)
      .filter(([, p]) => p === provider)
      .map(([model]) => model);
  }

  /**
   * Save API key for a provider to Chrome storage
   * @param provider The provider
   * @param apiKey The API key to save
   */
  async saveApiKey(provider: ModelProvider, apiKey: string): Promise<void> {
    const key = provider === 'openai' ? STORAGE_KEYS.OPENAI_API_KEY : STORAGE_KEYS.ANTHROPIC_API_KEY;

    await new Promise<void>((resolve, reject) => {
      chrome.storage.sync.set({ [key]: apiKey }, () => {
        if (chrome.runtime.lastError) {
          reject(new Error(chrome.runtime.lastError.message));
        } else {
          resolve();
        }
      });
    });

    // Clear cache to force recreation with new API key
    this.clearCache(provider);
  }

  /**
   * Load API key for a provider from Chrome storage
   * @param provider The provider
   * @returns Promise resolving to the API key or null if not found
   */
  async loadApiKey(provider: ModelProvider): Promise<string | null> {
    const key = provider === 'openai' ? STORAGE_KEYS.OPENAI_API_KEY : STORAGE_KEYS.ANTHROPIC_API_KEY;

    return new Promise((resolve, reject) => {
      chrome.storage.sync.get([key], (result) => {
        if (chrome.runtime.lastError) {
          reject(new Error(chrome.runtime.lastError.message));
        } else {
          resolve(result[key] || null);
        }
      });
    });
  }

  /**
   * Set the default provider
   * @param provider The provider to set as default
   */
  async setDefaultProvider(provider: ModelProvider): Promise<void> {
    await new Promise<void>((resolve, reject) => {
      chrome.storage.sync.set({ [STORAGE_KEYS.DEFAULT_PROVIDER]: provider }, () => {
        if (chrome.runtime.lastError) {
          reject(new Error(chrome.runtime.lastError.message));
        } else {
          resolve();
        }
      });
    });
  }

  /**
   * Get the default provider
   * @returns Promise resolving to the default provider
   */
  async getDefaultProvider(): Promise<ModelProvider> {
    return new Promise((resolve, reject) => {
      chrome.storage.sync.get([STORAGE_KEYS.DEFAULT_PROVIDER], (result) => {
        if (chrome.runtime.lastError) {
          reject(new Error(chrome.runtime.lastError.message));
        } else {
          resolve(result[STORAGE_KEYS.DEFAULT_PROVIDER] || 'openai');
        }
      });
    });
  }

  /**
   * Clear the client cache
   * @param provider Optional provider to clear, or all if not specified
   */
  clearCache(provider?: ModelProvider): void {
    if (provider) {
      this.clientCache.delete(provider);
    } else {
      this.clientCache.clear();
    }
  }

  /**
   * Check if a provider has a valid API key configured
   * @param provider The provider to check
   * @returns Promise resolving to true if API key exists
   */
  async hasValidApiKey(provider: ModelProvider): Promise<boolean> {
    const apiKey = await this.loadApiKey(provider);
    return apiKey !== null && apiKey.trim().length > 0;
  }

  /**
   * Get configuration status for all providers
   * @returns Promise resolving to configuration status
   */
  async getConfigurationStatus(): Promise<Record<ModelProvider, { hasApiKey: boolean; isDefault: boolean }>> {
    const [openaiHasKey, anthropicHasKey, defaultProvider] = await Promise.all([
      this.hasValidApiKey('openai'),
      this.hasValidApiKey('anthropic'),
      this.getDefaultProvider(),
    ]);

    return {
      openai: {
        hasApiKey: openaiHasKey,
        isDefault: defaultProvider === 'openai',
      },
      anthropic: {
        hasApiKey: anthropicHasKey,
        isDefault: defaultProvider === 'anthropic',
      },
    };
  }

  /**
   * Load configuration for a provider from Chrome storage
   * @param provider The provider
   * @returns Promise resolving to the client configuration
   */
  private async loadConfigForProvider(provider: ModelProvider): Promise<ModelClientConfig> {
    const apiKey = await this.loadApiKey(provider);

    if (!apiKey) {
      throw new ModelClientError(`No API key configured for provider: ${provider}`);
    }

    const config: ModelClientConfig = {
      provider,
      apiKey,
      options: {},
    };

    // Load provider-specific options
    if (provider === 'openai') {
      const organization = await this.loadFromStorage(STORAGE_KEYS.OPENAI_ORGANIZATION);
      if (organization) {
        config.options!.organization = organization;
      }
    } else if (provider === 'anthropic') {
      const version = await this.loadFromStorage(STORAGE_KEYS.ANTHROPIC_VERSION);
      if (version) {
        config.options!.version = version;
      }
    }

    return config;
  }

  /**
   * Load a value from Chrome storage
   * @param key The storage key
   * @returns Promise resolving to the value or null
   */
  private async loadFromStorage(key: string): Promise<string | null> {
    return new Promise((resolve, reject) => {
      chrome.storage.sync.get([key], (result) => {
        if (chrome.runtime.lastError) {
          reject(new Error(chrome.runtime.lastError.message));
        } else {
          resolve(result[key] || null);
        }
      });
    });
  }

  /**
   * Instantiate a client with the given configuration
   * @param config The client configuration
   * @returns Model client instance
   */
  private instantiateClient(config: ModelClientConfig): ModelClient {
    switch (config.provider) {
      case 'openai':
        return new OpenAIClient(config.apiKey, {
          baseUrl: config.options?.baseUrl,
          organization: config.options?.organization,
        });

      case 'anthropic':
        return new AnthropicClient(config.apiKey, {
          baseUrl: config.options?.baseUrl,
          version: config.options?.version,
        });

      default:
        throw new ModelClientError(`Unsupported provider: ${(config as any).provider}`);
    }
  }

  /**
   * Create a simple hash of the configuration for caching
   * @param config The configuration to hash
   * @returns Hash string
   */
  private hashConfig(config: ModelClientConfig): string {
    const str = JSON.stringify({
      provider: config.provider,
      apiKey: config.apiKey.slice(0, 10), // Only use first 10 chars for privacy
      options: config.options || {},
    });

    // Simple hash function
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = ((hash << 5) - hash) + char;
      hash = hash & hash; // Convert to 32-bit integer
    }

    return hash.toString(36);
  }
}

/**
 * Convenience function to get the factory instance
 */
export const getModelClientFactory = () => ModelClientFactory.getInstance();