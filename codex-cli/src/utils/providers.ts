export interface ProviderInfo {
  name: string;
  baseURL: string;
  envKey: string;
  customHeaders?: Record<string, string>;
}

export const providers: Record<string, ProviderInfo> = {
  openai: {
    name: "OpenAI",
    baseURL: "https://api.openai.com/v1",
    envKey: "OPENAI_API_KEY",
  },
  openrouter: {
    name: "OpenRouter",
    baseURL: "https://openrouter.ai/api/v1",
    envKey: "OPENROUTER_API_KEY",
  },
  azure: {
    name: "AzureOpenAI",
    baseURL: "https://YOUR_PROJECT_NAME.openai.azure.com/openai",
    envKey: "AZURE_OPENAI_API_KEY",
  },
  gemini: {
    name: "Gemini",
    baseURL: "https://generativelanguage.googleapis.com/v1beta/openai",
    envKey: "GEMINI_API_KEY",
  },
  ollama: {
    name: "Ollama",
    baseURL: "http://localhost:11434/v1",
    envKey: "OLLAMA_API_KEY",
  },
  mistral: {
    name: "Mistral",
    baseURL: "https://api.mistral.ai/v1",
    envKey: "MISTRAL_API_KEY",
  },
  deepseek: {
    name: "DeepSeek",
    baseURL: "https://api.deepseek.com",
    envKey: "DEEPSEEK_API_KEY",
  },
  xai: {
    name: "xAI",
    baseURL: "https://api.x.ai/v1",
    envKey: "XAI_API_KEY",
  },
  groq: {
    name: "Groq",
    baseURL: "https://api.groq.com/openai/v1",
    envKey: "GROQ_API_KEY",
  },
  arceeai: {
    name: "ArceeAI",
    baseURL: "https://conductor.arcee.ai/v1",
    envKey: "ARCEEAI_API_KEY",
  },
};

/**
 * Parse custom headers from provider-specific environment variable.
 * Format: "key: value\nkey2: value2"
 */
export function parseCustomHeadersFromEnv(
  providerName: string,
): Record<string, string> {
  // Check for a PROVIDER-specific custom headers: e.g. OPENAI_CUSTOM_HEADERS or GROQ_CUSTOM_HEADERS.
  const envHeaders =
    process.env[`${providerName.toUpperCase()}_CUSTOM_HEADERS`];
  if (!envHeaders) {
    return {};
  }

  return Object.fromEntries(
    envHeaders
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line && line.includes(":"))
      .map((line) => {
        const [key, ...rest] = line.split(":");
        return [key?.trim(), rest.join(":").trim()];
      })
      .filter(([key]) => key),
  );
}

/**
 * Get merged custom headers from provider config and provider-specific environment variable.
 * Environment variable headers take precedence over provider config headers.
 */
export function getCustomHeaders(
  provider: ProviderInfo,
  providerName: string = "openai",
): Record<string, string> {
  return {
    ...provider.customHeaders,
    ...parseCustomHeadersFromEnv(providerName),
  };
}
