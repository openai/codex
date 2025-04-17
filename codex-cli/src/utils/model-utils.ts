import { OPENAI_API_KEY, OLLAMA_ENABLED } from "./config.js";
import OpenAI from "openai";
import { 
  fetchOllamaModels, 
  formatOllamaModelName, 
  isOllamaModel 
} from "./ollama-client.js";

const MODEL_LIST_TIMEOUT_MS = 2_000; // 2 seconds
export const RECOMMENDED_OPENAI_MODELS: Array<string> = ["o4-mini", "o3"];
export const RECOMMENDED_OLLAMA_MODELS: Array<string> = ["gemma3:1b", "llama3:8b"];
export const RECOMMENDED_MODELS: Array<string> = [
  ...RECOMMENDED_OPENAI_MODELS,
  ...RECOMMENDED_OLLAMA_MODELS.map(formatOllamaModelName)
];

/**
 * Background model loader / cache.
 *
 * We start fetching the list of available models from OpenAI once the CLI
 * enters interactive mode.  The request is made exactly once during the
 * lifetime of the process and the results are cached for subsequent calls.
 */

let modelsPromise: Promise<Array<string>> | null = null;

// Expose a way to reset the modelsPromise for testing
if (process.env["NODE_ENV"] === "test") {
  // @ts-ignore - This is only used in tests
  global.__TEST_RESET_MODELS_PROMISE = () => {
    modelsPromise = null;
  };
}

async function fetchModels(): Promise<Array<string>> {
  const models: Array<string> = [];

  // Fetch OpenAI models if API key is available
  if (OPENAI_API_KEY) {
    try {
      const openai = new OpenAI({ apiKey: OPENAI_API_KEY });
      const list = await openai.models.list();

      for await (const model of list as AsyncIterable<{ id?: string }>) {
        if (model && typeof model.id === "string") {
          models.push(model.id);
        }
      }
    } catch (error) {
      console.error("Error fetching OpenAI models:", error);
      // Fall back to recommended models for OpenAI
      models.push(...RECOMMENDED_OPENAI_MODELS);
    }
  } else {
    // If no API key, use recommended OpenAI models
    models.push(...RECOMMENDED_OPENAI_MODELS);
  }

  // Fetch Ollama models if enabled
  if (OLLAMA_ENABLED) {
    try {
      const ollamaModels = await fetchOllamaModels();
      // Add prefix to Ollama models to distinguish them
      models.push(...ollamaModels.map(formatOllamaModelName));
    } catch (error) {
      console.error("Error fetching Ollama models:", error);
      // Fall back to recommended models for Ollama
      models.push(...RECOMMENDED_OLLAMA_MODELS.map(formatOllamaModelName));
    }
  }

  return models.sort();
}

export function preloadModels(): void {
  if (!modelsPromise) {
    // Fire‑and‑forget – callers that truly need the list should `await`
    // `getAvailableModels()` instead.
    void getAvailableModels();
  }
}

export async function getAvailableModels(): Promise<Array<string>> {
  if (!modelsPromise) {
    modelsPromise = fetchModels();
  }
  return modelsPromise;
}

/**
 * Verify that the provided model identifier is present in the set returned by
 * {@link getAvailableModels}. The list of models is fetched from the OpenAI
 * `/models` endpoint and Ollama API the first time it is required and then 
 * cached in‑process.
 */
export async function isModelSupportedForResponses(
  model: string | undefined | null,
): Promise<boolean> {
  if (
    typeof model !== "string" ||
    model.trim() === "" ||
    RECOMMENDED_MODELS.includes(model)
  ) {
    return true;
  }

  // For Ollama models, check if Ollama is enabled
  if (isOllamaModel(model) && !OLLAMA_ENABLED) {
    return false;
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
