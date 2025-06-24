import type { AppConfig } from "./config.js";

import {
  getBaseUrl,
  getApiKey,
  AZURE_OPENAI_API_VERSION,
  OPENAI_TIMEOUT_MS,
  OPENAI_ORGANIZATION,
  OPENAI_PROJECT,
  parseAzureAdditionalHeaders,
  getAzureFoundryBaseUrl,
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
): OpenAI | AzureOpenAI {  const headers: Record<string, string> = {};
  
  if (config.provider?.toLowerCase() === "azure") {
    const azureAdditionalHeaders = parseAzureAdditionalHeaders();
    Object.assign(headers, azureAdditionalHeaders);
  } else {
    if (OPENAI_ORGANIZATION) {
      headers["OpenAI-Organization"] = OPENAI_ORGANIZATION;
    }
    if (OPENAI_PROJECT) {
      headers["OpenAI-Project"] = OPENAI_PROJECT;
    }
  }

  if (config.provider?.toLowerCase() === "azure") {
    const azureEndpoint = getAzureFoundryBaseUrl();
    const hasEmbeddedApiVersion = azureEndpoint && azureEndpoint.includes("api-version=");
    
    const originalEnvValue = process.env["OPENAI_API_VERSION"];
    if (hasEmbeddedApiVersion && !originalEnvValue) {
      process.env["OPENAI_API_VERSION"] = "dummy-value-ignored-by-url";
    }
    
    const azureConfig: ConstructorParameters<typeof AzureOpenAI>[0] = {
      apiKey: getApiKey(config.provider),
      baseURL: getBaseUrl(config.provider),
      timeout: OPENAI_TIMEOUT_MS,
      defaultHeaders: headers,
    };

    const extractedApiVersion = process.env['AZURE_EXTRACTED_API_VERSION'];
    if (extractedApiVersion) {
      azureConfig.apiVersion = extractedApiVersion;
    } else if (!hasEmbeddedApiVersion) {
      azureConfig.apiVersion = AZURE_OPENAI_API_VERSION;
    }
    
    try {
      const client = new AzureOpenAI(azureConfig);
      
      if (hasEmbeddedApiVersion && !originalEnvValue) {
        delete process.env["OPENAI_API_VERSION"];
      }
      
      return client;
    } catch (error) {
      if (hasEmbeddedApiVersion && !originalEnvValue) {
        delete process.env["OPENAI_API_VERSION"];
      }
      throw error;
    }
  }

  return new OpenAI({
    apiKey: getApiKey(config.provider),
    baseURL: getBaseUrl(config.provider),
    timeout: OPENAI_TIMEOUT_MS,
    defaultHeaders: headers,
  });
}
