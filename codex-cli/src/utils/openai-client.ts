import type { AppConfig } from "./config.js";

import {
  getBaseUrl,
  getApiKey,
  AZURE_OPENAI_API_VERSION,
  AZURE_OPENAI_DEPLOYMENT,
  OPENAI_TIMEOUT_MS,
  OPENAI_ORGANIZATION,
  OPENAI_PROJECT,
  HTTPS_PROXY_URL,
} from "./config.js";
import {
  DefaultAzureCredential,
  getBearerTokenProvider,
} from "@azure/identity";
import { HttpsProxyAgent } from "https-proxy-agent";
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
  headers: Record<string, string> = {},
): OpenAI | AzureOpenAI {
  const defaultHeaders: Record<string, string> = {
    ...(OPENAI_ORGANIZATION
      ? { "OpenAI-Organization": OPENAI_ORGANIZATION }
      : {}),
    ...(OPENAI_PROJECT ? { "OpenAI-Project": OPENAI_PROJECT } : {}),
    ...headers,
  };

  const apiKey = getApiKey(config.provider);
  const httpAgent = HTTPS_PROXY_URL
    ? new HttpsProxyAgent(HTTPS_PROXY_URL)
    : undefined;
  const baseURL = getBaseUrl(config.provider);
  const timeout = OPENAI_TIMEOUT_MS;

  if (config.provider?.toLowerCase() === "azure") {
    if (apiKey === undefined) {
      const credential = new DefaultAzureCredential();
      const azureADTokenProvider = getBearerTokenProvider(
        credential,
        "https://cognitiveservices.azure.com/.default",
      );
      return new AzureOpenAI({
        azureADTokenProvider,
        baseURL,
        timeout,
        defaultHeaders,
        httpAgent,
        deployment: AZURE_OPENAI_DEPLOYMENT,
        apiVersion: AZURE_OPENAI_API_VERSION,
      });
    }

    return new AzureOpenAI({
      apiKey,
      baseURL,
      timeout,
      defaultHeaders,
      httpAgent,
      apiVersion: AZURE_OPENAI_API_VERSION,
    });
  }

  return new OpenAI({
    apiKey,
    baseURL,
    timeout,
    defaultHeaders,
  });
}
