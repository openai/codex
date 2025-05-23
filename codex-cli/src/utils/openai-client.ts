import type { AppConfig } from "./config.js";

import {
  getBaseUrl,
  getApiKey,
  AZURE_OPENAI_API_VERSION,
  OPENAI_TIMEOUT_MS,
  OPENAI_ORGANIZATION,
  OPENAI_PROJECT,
} from "./config.js";
import OpenAI, { AzureOpenAI } from "openai";

type OpenAIClientConfig = {
  provider: string;
};

/**
 * Creates an OpenAI client instance based on the provided configuration.
 * Handles both standard OpenAI and Azure OpenAI configurations.
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

  const provider = config.provider || "openai";

  try {
    // This will either return a valid API key or throw an error for non-OpenAI providers
    const apiKey = getApiKey(provider);

    if (provider.toLowerCase() === "azure") {
      return new AzureOpenAI({
        apiKey: apiKey as unknown as string,
        baseURL: getBaseUrl(provider),
        apiVersion: AZURE_OPENAI_API_VERSION,
        timeout: OPENAI_TIMEOUT_MS,
        defaultHeaders: headers,
      });
    }

    // For OpenAI and all other providers (including OpenRouter)
    return new OpenAI({
      apiKey: apiKey as unknown as string,
      baseURL: getBaseUrl(provider),
      timeout: OPENAI_TIMEOUT_MS,
      defaultHeaders: headers,
    });
  } catch (error) {
    // Special handling for OpenAI provider - we want a specific error message
    if (provider.toLowerCase() === "openai") {
      throw new Error("Missing API key for OpenAI provider");
    }
    // Re-throw the original error for other providers
    throw error;
  }
}
