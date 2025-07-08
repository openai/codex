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

// ---------------------------------------------------------------------------
// Deep-research model helper
// ---------------------------------------------------------------------------
// Deep-research model variants (model names that contain the substring
// "deep-research") require that each Chat Completions request includes at
// least one of the special built-in tools `web_search_preview` or `mcp`.
// Forgetting to supply the tool triggers a 400 response from the API.
//
// We transparently patch the instantiated client so that *any* call to
// `client.chat.completions.create()` automatically injects a minimal
// `web_search_preview` tool when talking to a deep-research model and when the
// caller hasn’t already provided one of the required tools.  This centralises
// the fix so individual call-sites don’t need to remember the rule.

function patchForDeepResearchModels(client: OpenAI | AzureOpenAI) {
  // Bail quickly if the shape we expect isn’t present (older SDK?)
  type ChatCompletionsLike = {
    // We don’t expose the concrete SDK type here to avoid a hard dependency on
    // the SDK’s internal typings. `unknown` is used instead of `any` to satisfy
    // the `@typescript-eslint/no-explicit-any` rule while still allowing the
    // dynamic behaviour we need.
    create: (_params: unknown, _options?: unknown) => Promise<unknown>;
  };

  const chatCompletions = (
    client as { chat?: { completions?: ChatCompletionsLike } }
  ).chat?.completions;

  if (!chatCompletions?.create) {
    return;
  }

  const originalCreate = chatCompletions.create.bind(chatCompletions);

  // Monkey-patch the `create` method so that we can transparently inject the
  // required tool when a deep-research model is requested.
  chatCompletions.create = (rawParams: unknown, options?: unknown) => {
    let patchedParams = rawParams as Record<string, unknown>;

    try {
      if (
        patchedParams &&
        typeof patchedParams === "object" &&
        typeof patchedParams.model === "string" &&
        patchedParams.model.includes("deep-research")
      ) {
        const originalTools = Array.isArray(patchedParams.tools)
          ? [...patchedParams.tools]
          : [];

        const hasRequiredTool = originalTools.some(
          (t) =>
            (t as { type?: string }).type === "web_search_preview" ||
            (t as { type?: string }).type === "mcp",
        );

        const tools = hasRequiredTool
          ? originalTools
          : [...originalTools, { type: "web_search_preview" }];

        if (
          tools.length !==
          (patchedParams.tools as Array<unknown> | undefined)?.length
        ) {
          patchedParams = { ...patchedParams, tools };
        }
      }
    } catch {
      /* best-effort shim – swallow any unexpected errors */
    }

    // Delegate to the original SDK implementation.
    return originalCreate(patchedParams, options);
  };
}

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

  const providerIsAzure = config.provider?.toLowerCase() === "azure";

  const client = providerIsAzure
    ? new AzureOpenAI({
        apiKey: getApiKey(config.provider),
        baseURL: getBaseUrl(config.provider),
        apiVersion: AZURE_OPENAI_API_VERSION,
        timeout: OPENAI_TIMEOUT_MS,
        defaultHeaders: headers,
      })
    : new OpenAI({
        apiKey: getApiKey(config.provider),
        baseURL: getBaseUrl(config.provider),
        timeout: OPENAI_TIMEOUT_MS,
        defaultHeaders: headers,
      });

  // Apply deep-research tool injection shim once per client instance.
  patchForDeepResearchModels(client);

  return client;
}
