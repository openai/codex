/**
 * Provider exports for multi-provider support
 */

// Export interfaces and types
export * from "./provider-interface.js";

// Export base provider
export * from "./base-provider.js";

// Export provider registry
export * from "./provider-registry.js";

// Export OpenAI provider
export * from "./openai-provider.js";

// Import providers
import { ProviderRegistry } from "./provider-registry.js";
import { OpenAIProvider } from "./openai-provider.js";

// Register providers
ProviderRegistry.register(new OpenAIProvider());

// Convenience method to get provider for a model
export const getProviderForModel = ProviderRegistry.getProviderForModel.bind(ProviderRegistry);

// Convenience method to get the default provider
export const getDefaultProvider = ProviderRegistry.getDefaultProvider.bind(ProviderRegistry);