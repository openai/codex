/**
 * Provider registry initialization and management
 */

import { ClaudeProvider } from "./claude-provider.js";
import { OpenAIProvider } from "./openai-provider.js";
import { ProviderRegistry } from "./provider-registry.js";
import { DEFAULT_PROVIDER_ID } from "../provider-config.js";

/**
 * Initialize the provider registry
 */
export function initializeProviderRegistry(): void {
  // Clear any existing providers
  ProviderRegistry.clearProviders();
  
  // Register providers
  ProviderRegistry.register(new OpenAIProvider());
  ProviderRegistry.register(new ClaudeProvider());
  
  // Set default provider from environment or config
  const envDefault = process.env.CODEX_DEFAULT_PROVIDER;
  
  // If environment variable is set and provider exists, use it
  if (envDefault && ProviderRegistry.hasProvider(envDefault)) {
    ProviderRegistry.setDefaultProviderId(envDefault);
  } else {
    // Otherwise use the default from provider-config
    ProviderRegistry.setDefaultProviderId(DEFAULT_PROVIDER_ID);
  }
}

/**
 * Expose the provider registry
 */
export { ProviderRegistry } from "./provider-registry.js";

/**
 * Export provider constructors
 */
export { OpenAIProvider } from "./openai-provider.js";
export { ClaudeProvider } from "./claude-provider.js";
/**
 * Export provider types
 */
export type { LLMProvider, CompletionParams } from "./provider-interface.js";
