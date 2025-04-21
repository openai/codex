import { getBaseUrl, getApiKey } from "./config";
import OpenAI from "openai";

const MODEL_LIST_TIMEOUT_MS = 2_000; // 2 seconds
export const RECOMMENDED_MODELS: Array<string> = ["o4-mini", "o3"];

/**
 * Background model loader / cache.
 *
 * We start fetching the list of available models from OpenAI once the CLI
 * enters interactive mode.  The request is made exactly once during the
 * lifetime of the process and the results are cached for subsequent calls.
 */

async function fetchModels(provider: string): Promise<Array<string>> {
  // If the user has not configured an API key we cannot hit the network.
  if (!getApiKey(provider)) {
    throw new Error("No API key configured for provider: " + provider);
  }

  const baseURL = getBaseUrl(provider);
  try {
    const openai = new OpenAI({ apiKey: getApiKey(provider), baseURL });
    const list = await openai.models.list();
    const models: Array<string> = [];
    for await (const model of list as AsyncIterable<{ id?: string }>) {
      if (model && typeof model.id === "string") {
        let modelStr = model.id;
        // fix for gemini
        if (modelStr.startsWith("models/")) {
          modelStr = modelStr.replace("models/", "");
        }
        models.push(modelStr);
      }
    }

    return models.sort();
  } catch (error) {
    return [];
  }
}

export async function getAvailableModels(
  provider: string,
): Promise<Array<string>> {
  return fetchModels(provider.toLowerCase());
}

/**
 * Verify that the provided model identifier is present in the set returned by
 * {@link getAvailableModels}. The list of models is fetched from the OpenAI
 * `/models` endpoint the first time it is required and then cached in‑process.
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

  try {
    const models = await Promise.race<Array<string>>([
      getAvailableModels("openai"),
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
