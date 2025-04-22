import * as process from "process";

export const providers: Record<
  string,
  {
    name: string;
    baseURL: string;
    envKey: string;
    envBaseURLKey: string;
  }
> = {
  openai: {
    name: "OpenAI",
    baseURL: "https://api.openai.com/v1",
    envKey: "OPENAI_API_KEY",
    envBaseURLKey: "OPENAI_BASE_URL",
  },
  openrouter: {
    name: "OpenRouter",
    baseURL: "https://openrouter.ai/api/v1",
    envKey: "OPENROUTER_API_KEY",
    envBaseURLKey: "OPENROUTER_BASE_URL",
  },
  gemini: {
    name: "Gemini",
    baseURL: "https://generativelanguage.googleapis.com/v1beta/openai",
    envKey: "GEMINI_API_KEY",
    envBaseURLKey: "GEMINI_BASE_URL",
  },
  ollama: {
    name: "Ollama",
    baseURL: "http://localhost:11434/v1",
    envKey: "OLLAMA_API_KEY",
    envBaseURLKey: "GEMINI_BASE_URL",
  },
  mistral: {
    name: "Mistral",
    baseURL: "https://api.mistral.ai/v1",
    envKey: "MISTRAL_API_KEY",
    envBaseURLKey: "MISTRAL_BASE_URL",
  },
  deepseek: {
    name: "DeepSeek",
    baseURL: "https://api.deepseek.com",
    envKey: "DEEPSEEK_API_KEY",
    envBaseURLKey: "DEEPSEEK_BASE_URL",
  },
  xai: {
    name: "xAI",
    baseURL: "https://api.x.ai/v1",
    envKey: "XAI_API_KEY",
    envBaseURLKey: "XAI_BASE_URL",
  },
  groq: {
    name: "Groq",
    baseURL: "https://api.groq.com/openai/v1",
    envKey: "GROQ_API_KEY",
    envBaseURLKey: "GROQ_BASE_URL",
  },
};
