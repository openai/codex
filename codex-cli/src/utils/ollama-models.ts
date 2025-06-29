import type { ModelInfo } from "./model-info.js";

/**
 * Known Ollama models with their context window sizes.
 * This helps provide accurate context window information for popular Ollama models.
 */
export const ollamaModelInfo: Record<string, ModelInfo> = {
  // Qwen models
  "qwen2.5-coder:32b-128k": {
    label: "Qwen 2.5 Coder 32B (128k)",
    maxContextLength: 128000,
  },
  "qwen2.5-coder:32b": {
    label: "Qwen 2.5 Coder 32B",
    maxContextLength: 32000,
  },
  "qwen2.5-coder:7b": {
    label: "Qwen 2.5 Coder 7B",
    maxContextLength: 32000,
  },
  "qwen2.5-coder:14b": {
    label: "Qwen 2.5 Coder 14B",
    maxContextLength: 32000,
  },
  
  // DeepSeek models
  "deepseek-coder-v2:16b": {
    label: "DeepSeek Coder v2 16B",
    maxContextLength: 32000,
  },
  "deepseek-coder-v2:236b": {
    label: "DeepSeek Coder v2 236B",
    maxContextLength: 128000,
  },
  
  // Llama models
  "llama3.1:8b": {
    label: "Llama 3.1 8B",
    maxContextLength: 128000,
  },
  "llama3.1:70b": {
    label: "Llama 3.1 70B",
    maxContextLength: 128000,
  },
  "llama3.1:405b": {
    label: "Llama 3.1 405B",
    maxContextLength: 128000,
  },
  "llama3:8b": {
    label: "Llama 3 8B",
    maxContextLength: 8000,
  },
  "llama3:70b": {
    label: "Llama 3 70B",
    maxContextLength: 8000,
  },
  
  // CodeLlama models
  "codellama:7b": {
    label: "Code Llama 7B",
    maxContextLength: 16000,
  },
  "codellama:13b": {
    label: "Code Llama 13B",
    maxContextLength: 16000,
  },
  "codellama:34b": {
    label: "Code Llama 34B",
    maxContextLength: 16000,
  },
  "codellama:70b": {
    label: "Code Llama 70B",
    maxContextLength: 100000,
  },
  
  // Mixtral models
  "mixtral:8x7b": {
    label: "Mixtral 8x7B",
    maxContextLength: 32000,
  },
  "mixtral:8x22b": {
    label: "Mixtral 8x22B",
    maxContextLength: 64000,
  },
  
  // Mistral models
  "mistral:7b": {
    label: "Mistral 7B",
    maxContextLength: 8000,
  },
  "mistral-nemo:12b": {
    label: "Mistral Nemo 12B",
    maxContextLength: 128000,
  },
  
  // Phi models
  "phi3:14b": {
    label: "Phi-3 14B",
    maxContextLength: 128000,
  },
  "phi3:3.8b": {
    label: "Phi-3 3.8B",
    maxContextLength: 128000,
  },
  
  // Gemma models
  "gemma2:9b": {
    label: "Gemma 2 9B",
    maxContextLength: 8192,
  },
  "gemma2:27b": {
    label: "Gemma 2 27B",
    maxContextLength: 8192,
  },
  
  // Command-R models
  "command-r:35b": {
    label: "Command-R 35B",
    maxContextLength: 128000,
  },
  "command-r-plus:104b": {
    label: "Command-R Plus 104B",
    maxContextLength: 128000,
  },
};

/**
 * Get context window size for an Ollama model.
 * Falls back to parsing the model name if not in the registry.
 */
export function getOllamaModelContextWindow(model: string): number {
  // Check exact match first
  if (model in ollamaModelInfo) {
    return ollamaModelInfo[model].maxContextLength;
  }
  
  // Check for partial matches (without tag)
  const modelBase = model.split(":")[0];
  for (const [key, info] of Object.entries(ollamaModelInfo)) {
    if (key.startsWith(modelBase + ":")) {
      return info.maxContextLength;
    }
  }
  
  return 0; // Let the fallback logic in model-utils.ts handle it
}