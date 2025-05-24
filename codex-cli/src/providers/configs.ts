import { AuthType, type ProviderConfig } from "./types.js";

/**
 * Extended provider configuration
 */
export interface ExtendedProviderConfig extends ProviderConfig {
  /** Which adapter to use */
  adapter?: "standard" | "azure" | "vertex";
}

/**
 * Default provider configurations
 */
export const providerConfigs: Record<string, ExtendedProviderConfig> = {
  openai: {
    id: "openai",
    name: "OpenAI",
    baseURL: "https://api.openai.com/v1",
    envKey: "OPENAI_API_KEY",
    authType: AuthType.API_KEY,
    adapter: "standard",
  },
  openrouter: {
    id: "openrouter",
    name: "OpenRouter",
    baseURL: "https://openrouter.ai/api/v1",
    envKey: "OPENROUTER_API_KEY",
    authType: AuthType.API_KEY,
    adapter: "standard",
  },
  azure: {
    id: "azure",
    name: "AzureOpenAI",
    baseURL: "", // Must be provided via AZURE_BASE_URL env var or config
    envKey: "AZURE_OPENAI_API_KEY",
    authType: AuthType.API_KEY,
    adapter: "azure",
  },
  gemini: {
    id: "gemini",
    name: "Gemini",
    baseURL: "https://generativelanguage.googleapis.com/v1beta/openai",
    envKey: "GEMINI_API_KEY",
    authType: AuthType.API_KEY,
    adapter: "standard",
  },
  vertex: {
    id: "vertex",
    name: "Vertex AI",
    baseURL: "dynamic", // Will be constructed by adapter
    envKey: "GOOGLE_APPLICATION_CREDENTIALS",
    authType: AuthType.OAUTH,
    adapter: "vertex",
  },
  ollama: {
    id: "ollama",
    name: "Ollama",
    baseURL: "http://localhost:11434/v1",
    envKey: "OLLAMA_API_KEY",
    authType: AuthType.NONE,
    adapter: "standard",
  },
  mistral: {
    id: "mistral",
    name: "Mistral",
    baseURL: "https://api.mistral.ai/v1",
    envKey: "MISTRAL_API_KEY",
    authType: AuthType.API_KEY,
    adapter: "standard",
  },
  deepseek: {
    id: "deepseek",
    name: "DeepSeek",
    baseURL: "https://api.deepseek.com",
    envKey: "DEEPSEEK_API_KEY",
    authType: AuthType.API_KEY,
    adapter: "standard",
  },
  xai: {
    id: "xai",
    name: "xAI",
    baseURL: "https://api.x.ai/v1",
    envKey: "XAI_API_KEY",
    authType: AuthType.API_KEY,
    adapter: "standard",
  },
  groq: {
    id: "groq",
    name: "Groq",
    baseURL: "https://api.groq.com/openai/v1",
    envKey: "GROQ_API_KEY",
    authType: AuthType.API_KEY,
    adapter: "standard",
  },
  arceeai: {
    id: "arceeai",
    name: "ArceeAI",
    baseURL: "https://conductor.arcee.ai/v1",
    envKey: "ARCEEAI_API_KEY",
    authType: AuthType.API_KEY,
    adapter: "standard",
  },
};
