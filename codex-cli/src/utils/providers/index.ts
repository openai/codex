/**
 * Provider exports for multi-provider support
 */

// Export interfaces and types
export * from "./provider-interface.js";

// Export base provider
export * from "./base-provider.js";

// Export provider registry
export * from "./provider-registry.js";

// Convenience method to get provider for a model
import { ProviderRegistry } from "./provider-registry.js";
export const getProviderForModel = ProviderRegistry.getProviderForModel.bind(ProviderRegistry);

// Convenience method to get the default provider
export const getDefaultProvider = ProviderRegistry.getDefaultProvider.bind(ProviderRegistry);