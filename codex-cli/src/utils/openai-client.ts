import type { AppConfig } from "./config.js";

import {
  getBaseUrl,
  getApiKey,
  AZURE_OPENAI_API_VERSION,
  OPENAI_TIMEOUT_MS,
  OPENAI_ORGANIZATION,
  OPENAI_PROJECT,
} from "./config.js";
import { createProviderAdapter } from "../providers/registry.js";
import { providerConfigs } from "../providers/configs.js";
import { AuthType } from "../providers/types.js";
import OpenAI, { AzureOpenAI } from "openai";

type OpenAIClientConfig = {
  provider: string;
};

/**
 * Creates an OpenAI client instance based on the provided configuration.
 * This is the legacy synchronous version maintained for backward compatibility.
 * For providers that require async initialization (like Vertex), this will throw an error.
 *
 * @param config The configuration containing provider information
 * @returns An instance of either OpenAI or AzureOpenAI client
 */
export function createOpenAIClient(
  config: OpenAIClientConfig | AppConfig,
): OpenAI | AzureOpenAI {
  const headers: Record<string, string> = {};
  if (OPENAI_ORGANIZATION) {
    headers["OpenAI-Organization"] = OPENAI_ORGANIZATION;
  }
  if (OPENAI_PROJECT) {
    headers["OpenAI-Project"] = OPENAI_PROJECT;
  }

  const providerId = config.provider?.toLowerCase() || "openai";

  // Check if this provider needs async initialization
  const providerConfig = providerConfigs[providerId];
  if (providerConfig && providerConfig.authType === AuthType.OAUTH) {
    throw new Error(
      `Provider '${providerId}' requires async initialization. ` +
      `Please use createOpenAIClientAsync() instead.`
    );
  }

  // Legacy implementation for backward compatibility
  if (providerId === "azure") {
    return new AzureOpenAI({
      apiKey: getApiKey(config.provider),
      baseURL: getBaseUrl(config.provider),
      apiVersion: AZURE_OPENAI_API_VERSION,
      timeout: OPENAI_TIMEOUT_MS,
      defaultHeaders: headers,
    });
  }

  return new OpenAI({
    apiKey: getApiKey(config.provider),
    baseURL: getBaseUrl(config.provider),
    timeout: OPENAI_TIMEOUT_MS,
    defaultHeaders: headers,
  });
}

/**
 * Creates an OpenAI client instance based on the provided configuration.
 * Uses the provider registry to handle different provider types and authentication methods.
 * This async version supports all providers including those requiring OAuth.
 *
 * @param config The configuration containing provider information
 * @returns An instance of OpenAI client configured for the specific provider
 */
export async function createOpenAIClientAsync(
  config: OpenAIClientConfig | AppConfig,
): Promise<OpenAI> {
  const providerId = config.provider || "openai";
  
  try {
    // Create provider adapter
    const adapter = await createProviderAdapter(providerId);
    
    // Create the client
    const client = await adapter.createClient();
    
    // Add global headers if this is OpenAI
    if (providerId === "openai" && (OPENAI_ORGANIZATION || OPENAI_PROJECT)) {
      const headers: Record<string, string> = {};
      if (OPENAI_ORGANIZATION) {
        headers["OpenAI-Organization"] = OPENAI_ORGANIZATION;
      }
      if (OPENAI_PROJECT) {
        headers["OpenAI-Project"] = OPENAI_PROJECT;
      }
      
      // Update default headers
      (client as unknown as { defaultHeaders: Record<string, string> }).defaultHeaders = {
        ...(client as unknown as { defaultHeaders: Record<string, string> }).defaultHeaders,
        ...headers,
      };
    }
    
    return client;
  } catch (error) {
    // For backward compatibility, create a basic client if provider is not found
    // This allows custom providers to still work
    if ((error as Error).message.includes("Unknown provider")) {
      return new OpenAI({
        apiKey: getApiKey(providerId),
        baseURL: getBaseUrl(providerId),
        timeout: OPENAI_TIMEOUT_MS,
        defaultHeaders: {},
      });
    }
    throw error;
  }
}
