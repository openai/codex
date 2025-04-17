import { OPENAI_API_KEY } from "./config";
import { OLLAMA_DEFAULT_MODEL } from "./ollama-client";
import OpenAI from "openai";

const MODEL_LIST_TIMEOUT_MS = 2_000; // 2 seconds
export const RECOMMENDED_MODELS: Array<string> = ["o4-mini", "o3"];
export const RECOMMENDED_OLLAMA_MODELS: Array<string> = ["llama2", "codellama", "mistral"];

export type ModelProvider = "openai" | "ollama";

export function getModelProvider(model: string): ModelProvider {
  if (RECOMMENDED_MODELS.includes(model)) {
    return "openai";
  }
  return "ollama";
}

let modelsPromise: Promise<Array<string>> | null = null;

export async function getAvailableModels(): Promise<Array<string>> {
  if (!modelsPromise) {
    modelsPromise = (async () => {
      try {
        // For OpenAI models
        const openaiModels = RECOMMENDED_MODELS;

        // For Ollama models
        const ollamaModels = RECOMMENDED_OLLAMA_MODELS;

        return [...openaiModels, ...ollamaModels];
      } catch (error) {
        console.error("Failed to fetch available models:", error);
        return [];
      }
    })();
  }
  return modelsPromise;
}

export async function preloadModels(): Promise<void> {
  // For OpenAI, we don't need to preload anything
  if (!OPENAI_API_KEY) {
    console.warn("No OpenAI API key found, skipping OpenAI model preload");
  }

  // For Ollama, we could potentially check if models are downloaded
  // but for now we'll just assume they are available
  await getAvailableModels();
}

/**
 * Verify that the provided model identifier is supported.
 * For Ollama models, we assume all models are supported.
 * For OpenAI models, we check against the available models list.
 */
export async function isModelSupportedForResponses(
  model: string | undefined | null,
): Promise<boolean> {
  if (typeof model !== "string" || model.trim() === "") {
    return true;
  }

  // All Ollama models are supported
  if (getModelProvider(model) === "ollama") {
    return true;
  }

  // For OpenAI models, first check recommended models
  if (RECOMMENDED_MODELS.includes(model)) {
    return true;
  }

  try {
    const models = await Promise.race<Array<string>>([
      getAvailableModels(),
      new Promise<Array<string>>((resolve) =>
        setTimeout(() => resolve([]), MODEL_LIST_TIMEOUT_MS),
      ),
    ]);

    // If the timeout fired we get an empty list → treat as supported to avoid
    // false negatives.
    if (models.length === 0) {
      return true;
    }

    return models.includes(model.trim());
  } catch {
    // Network or library failure → don't block start‑up.
    return true;
  }
}
