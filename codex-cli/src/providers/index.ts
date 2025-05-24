/**
 * Provider abstraction layer for supporting different AI providers
 * with various authentication methods.
 */

export { 
  AuthType,
  type ProviderConfig,
  type AuthProvider,
  type ProviderAdapter,
} from "./types.js";

export { 
  createProviderAdapter,
  getProviderConfig,
  providerConfigs,
} from "./registry.js";

// Re-export for convenience
export { createOpenAIClientAsync } from "../utils/openai-client.js";