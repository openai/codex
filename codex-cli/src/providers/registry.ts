import { AzureAdapter } from "./adapters/azure-adapter.js";
import { StandardAdapter } from "./adapters/standard-adapter.js";
import { VertexAdapter } from "./adapters/vertex-adapter.js";
import { ApiKeyAuthProvider } from "./auth/api-key-auth.js";
import { GoogleAuthProvider } from "./auth/google-auth.js";
import { NoAuthProvider } from "./auth/no-auth.js";
import { 
  AuthType, 
  type ProviderConfig, 
  type ProviderAdapter,
  type AuthProvider,
  type AuthProviderFactory
} from "./types.js";
import { getApiKey } from "../utils/config.js";
import { providerConfigs } from "./configs.js";

/**
 * Default auth provider factories by auth type
 */
const authFactories: Record<AuthType, AuthProviderFactory> = {
  [AuthType.API_KEY]: (config) => new ApiKeyAuthProvider(getApiKey(config.id), config.name),
  [AuthType.OAUTH]: () => new GoogleAuthProvider(), // Currently only Google OAuth
  [AuthType.NONE]: () => new NoAuthProvider(),
};

/**
 * Default adapter factories
 */
const adapterFactories = {
  standard: (config: ProviderConfig, auth: AuthProvider) => new StandardAdapter(config, auth),
  azure: (config: ProviderConfig, auth: AuthProvider) => new AzureAdapter(config, auth),
  vertex: (config: ProviderConfig, auth: AuthProvider) => new VertexAdapter(config, auth),
};

/**
 * Create a provider adapter for the given provider ID
 */
export async function createProviderAdapter(providerId: string): Promise<ProviderAdapter> {
  const config = providerConfigs[providerId.toLowerCase()];
  if (!config) {
    throw new Error(`Unknown provider: ${providerId}`);
  }

  // Create auth provider using factory
  const authFactory = authFactories[config.authType];
  if (!authFactory) {
    throw new Error(`Unknown auth type for ${config.name}: ${config.authType}`);
  }
  const authProviderOrPromise = authFactory(config);
  const authProvider = authProviderOrPromise instanceof Promise ? 
    await authProviderOrPromise : authProviderOrPromise;

  // Create adapter using factory
  const adapterKey = config.adapter || "standard";
  const adapterFactory = adapterFactories[adapterKey];
  if (!adapterFactory) {
    throw new Error(`Unknown adapter type: ${adapterKey}`);
  }

  return adapterFactory(config, authProvider);
}

/**
 * Get provider configuration (for backward compatibility)
 */
export function getProviderConfig(providerId: string): ProviderConfig | undefined {
  return providerConfigs[providerId.toLowerCase()];
}

// Re-export configs for convenience
export { providerConfigs } from "./configs.js";