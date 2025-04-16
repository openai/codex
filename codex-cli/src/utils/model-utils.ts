import { OPENAI_API_KEY, OPENROUTER_API_KEY, OPENROUTER_BASE_URL } from "./config";
import OpenAI from "openai";
import axios from "axios";

const MODEL_LIST_TIMEOUT_MS = 2_000; // 2 seconds
export const RECOMMENDED_MODELS: Array<string> = ["o4-mini", "o3"];
export const OPENROUTER_RECOMMENDED_MODELS: Array<string> = ["anthropic/claude-3-opus", "anthropic/claude-3-sonnet", "meta-llama/llama-3-70b-instruct"];

/**
 * Background model loader / cache.
 *
 * We start fetching the list of available models from OpenAI once the CLI
 * enters interactive mode.  The request is made exactly once during the
 * lifetime of the process and the results are cached for subsequent calls.
 */

let modelsPromise: Promise<Array<string>> | null = null;
let openRouterModelsPromise: Promise<Array<string>> | null = null;

async function fetchModels(): Promise<Array<string>> {
  // If the user has not configured an API key we cannot hit the network.
  if (!OPENAI_API_KEY) {
    return RECOMMENDED_MODELS;
  }

  try {
    const openai = new OpenAI({ apiKey: OPENAI_API_KEY });
    const list = await openai.models.list();

    const models: Array<string> = [];
    for await (const model of list as AsyncIterable<{ id?: string }>) {
      if (model && typeof model.id === "string") {
        models.push(model.id);
      }
    }

    return models.sort();
  } catch {
    return [];
  }
}

async function fetchOpenRouterModels(): Promise<Array<string>> {
  // If the user has not configured an API key we cannot hit the network.
  if (!OPENROUTER_API_KEY) {
    return OPENROUTER_RECOMMENDED_MODELS;
  }

  try {
    const response = await axios.get(`${OPENROUTER_BASE_URL}/models`, {
      headers: {
        Authorization: `Bearer ${OPENROUTER_API_KEY}`,
      },
      timeout: MODEL_LIST_TIMEOUT_MS,
    });

    if (response.data && Array.isArray(response.data.data)) {
      const models = response.data.data
        .filter((model: any) => model.id && typeof model.id === "string")
        .map((model: any) => model.id);
      return models.sort();
    }

    return OPENROUTER_RECOMMENDED_MODELS;
  } catch {
    return OPENROUTER_RECOMMENDED_MODELS;
  }
}

export function preloadModels(): void {
  if (!modelsPromise) {
    // Fire‑and‑forget – callers that truly need the list should `await`
    // `getAvailableModels()` instead.
    void getAvailableModels();
  }

  if (!openRouterModelsPromise) {
    // Fire‑and‑forget – callers that truly need the list should `await`
    // `getAvailableOpenRouterModels()` instead.
    void getAvailableOpenRouterModels();
  }
}

export async function getAvailableModels(): Promise<Array<string>> {
  if (!modelsPromise) {
    modelsPromise = fetchModels();
  }
  return modelsPromise;
}

export async function getAvailableOpenRouterModels(): Promise<Array<string>> {
  if (!openRouterModelsPromise) {
    openRouterModelsPromise = fetchOpenRouterModels();
  }
  return openRouterModelsPromise;
}

export async function getAllAvailableModels(useOpenRouter: boolean = false): Promise<Array<string>> {
  const openAIModels = await getAvailableModels();

  if (useOpenRouter) {
    const openRouterModels = await getAvailableOpenRouterModels();
    return [...openAIModels, ...openRouterModels];
  }

  return openAIModels;
}

/**
 * Verify that the provided model identifier is present in the set returned by
 * {@link getAvailableModels}. The list of models is fetched from the OpenAI
 * `/models` endpoint the first time it is required and then cached in‑process.
 */
export async function isModelSupportedForResponses(
  model: string | undefined | null,
  useOpenRouter: boolean = false,
): Promise<boolean> {
  if (
    typeof model !== "string" ||
    model.trim() === "" ||
    RECOMMENDED_MODELS.includes(model) ||
    (useOpenRouter && OPENROUTER_RECOMMENDED_MODELS.includes(model))
  ) {
    return true;
  }

  try {
    let models: Array<string>;

    if (useOpenRouter) {
      // Check both OpenAI and OpenRouter models
      models = await Promise.race<Array<string>>([
        getAllAvailableModels(true),
        new Promise<Array<string>>((resolve) =>
          setTimeout(() => resolve([]), MODEL_LIST_TIMEOUT_MS),
        ),
      ]);
    } else {
      // Only check OpenAI models
      models = await Promise.race<Array<string>>([
        getAvailableModels(),
        new Promise<Array<string>>((resolve) =>
          setTimeout(() => resolve([]), MODEL_LIST_TIMEOUT_MS),
        ),
      ]);
    }

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
