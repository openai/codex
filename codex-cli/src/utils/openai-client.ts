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

  // Azure OpenAI: construct dedicated client instance.
  if (config.provider?.toLowerCase() === "azure") {
    const apiKey = getApiKey(config.provider);
    const baseURL = getBaseUrl(config.provider);

    // For Azure OpenAI with Responses API, we need to use the api-key header
    // instead of the Bearer token format
    const azureHeaders = { ...headers };
    if (apiKey) {
      azureHeaders["api-key"] = apiKey;
    }

    // Ensure the baseURL includes /openai/v1 for Responses API
    // If the baseURL already ends with /openai or /openai/v1, use it as-is
    // Otherwise, append /openai/v1
    let effectiveBaseURL = baseURL;
    if (effectiveBaseURL && !effectiveBaseURL.match(/\/openai(\/v1)?$/)) {
      effectiveBaseURL = effectiveBaseURL.replace(/\/$/, "") + "/openai/v1";
    }

    return new AzureOpenAI({
      apiKey: apiKey || "dummy", // AzureOpenAI client requires a value even if using header auth
      baseURL: effectiveBaseURL,
      apiVersion: AZURE_OPENAI_API_VERSION,
      timeout: OPENAI_TIMEOUT_MS,
      defaultHeaders: azureHeaders,
    });
  }

  return new OpenAI({
    apiKey: getApiKey(config.provider),
    baseURL: getBaseUrl(config.provider),
    timeout: OPENAI_TIMEOUT_MS,
    defaultHeaders: headers,
  });
}
