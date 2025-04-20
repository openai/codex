/**
 * Provider registry for managing LLM providers
 */

import { LLMProvider } from "./provider-interface.js";

/**
 * Registry for managing and accessing LLM providers
 */
export class ProviderRegistry {
  /** Map of registered providers by ID */
  private static providers: Map<string, LLMProvider> = new Map();
  
  /** Default provider ID */
  private static defaultProviderId: string = "openai";
  
  /**
   * Register a provider with the registry
   * @param provider Provider to register
   */
  static register(provider: LLMProvider): void {
    this.providers.set(provider.id, provider);
  }
  
  /**
   * Get a provider by ID
   * @param id Provider ID
   * @returns The provider, or undefined if not found
   */
  static getProviderById(id: string): LLMProvider | undefined {
    return this.providers.get(id);
  }
  
  /**
   * Set the default provider ID
   * @param id Provider ID to use as default
   */
  static setDefaultProviderId(id: string): void {
    if (this.providers.has(id)) {
      this.defaultProviderId = id;
    } else {
      throw new Error(`Cannot set default provider: provider with ID "${id}" not registered`);
    }
  }
  
  /**
   * Get the default provider ID
   * @returns The current default provider ID
   */
  static getDefaultProviderId(): string {
    return this.defaultProviderId;
  }
  
  /**
   * Get a provider for a specific model
   * @param model Model identifier
   * @returns The appropriate provider for the model
   */
  static getProviderForModel(model: string): LLMProvider {
    // Detect provider from model name pattern
    if (model.startsWith("claude")) {
      return this.getProviderById("claude") || this.getDefaultProvider();
    }
    
    // Detect OpenAI models
    if (
      model.startsWith("gpt-") ||
      model.startsWith("o") ||
      model.startsWith("text-") ||
      model === "gpt-4" ||
      model === "gpt-3.5-turbo"
    ) {
      return this.getProviderById("openai") || this.getDefaultProvider();
    }
    
    // Default to the default provider
    return this.getDefaultProvider();
  }
  
  /**
   * Get the default provider
   * @returns Default provider, or first available provider
   * @throws Error if no providers are registered
   */
  static getDefaultProvider(): LLMProvider {
    const defaultProvider = this.getProviderById(this.defaultProviderId);
    if (defaultProvider) {
      return defaultProvider;
    }
    
    // Fallback to first available provider
    const providers = Array.from(this.providers.values());
    if (providers.length > 0) {
      return providers[0];
    }
    
    throw new Error("No LLM providers registered");
  }
  
  /**
   * Get all registered providers
   * @returns Array of all providers
   */
  static getAllProviders(): LLMProvider[] {
    return Array.from(this.providers.values());
  }
  
  /**
   * Check if a provider is registered
   * @param id Provider ID
   * @returns True if the provider is registered
   */
  static hasProvider(id: string): boolean {
    return this.providers.has(id);
  }
  
  /**
   * Clear all registered providers
   * Primarily used for testing
   */
  static clearProviders(): void {
    this.providers.clear();
    this.defaultProviderId = "openai";
  }
}