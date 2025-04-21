import { providerApiKeys } from "./config";
import { DEFAULT_PROVIDER_MODELS } from './provider-config';
import { ProviderRegistry, initializeProviderRegistry } from './providers/index.js';
import OpenAI from "openai";

const MODEL_LIST_TIMEOUT_MS = 2_000; // 2 seconds
export const RECOMMENDED_MODELS: Array<string> = Object.values(DEFAULT_PROVIDER_MODELS);

/**
 * Background model loader / cache.
 *
 * We start fetching the list of available models from all configured providers
 * once the CLI enters interactive mode. The request is made exactly once during the
 * lifetime of the process and the results are cached for subsequent calls.
 */

let modelsPromise: Promise<Array<string>> | null = null;

async function fetchModels(): Promise<Array<string>> {
  // Initialize provider registry if not done already
  initializeProviderRegistry();
  
  // Start with recommended models
  const allModels = [...RECOMMENDED_MODELS];
  
  try {
    // Get models from all available providers
    const providers = ProviderRegistry.getAllProviders();
    
    // For each provider, try to get their models
    for (const provider of providers) {
      try {
        const providerModels = await provider.getModels();
        
        // Add models from this provider
        for (const model of providerModels) {
          if (!allModels.includes(model)) {
            allModels.push(model);
          }
        }
      } catch (error) {
        console.error(`Error fetching models from provider ${provider.id}:`, error);
      }
    }

    return allModels.sort();
  } catch (error) {
    // On failure, return at least the recommended models
    console.error("Error fetching models:", error);
    return RECOMMENDED_MODELS;
  }
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

// Used for testing - resets the cached models promise
export function resetModelsCache(): void {
  modelsPromise = null;
}

/**
 * Verify that the provided model identifier is present in the set returned by
 * {@link getAvailableModels}. The list of models is fetched from the provider
 * API the first time it is required and then cached in‑process.
 */
export async function isModelSupportedForResponses(
  model: string | undefined | null,
): Promise<boolean> {
  // If model is not provided, is empty, or is a recommended model, 
  // consider it supported
  if (
    typeof model !== "string" ||
    model.trim() === "" ||
    RECOMMENDED_MODELS.includes(model)
  ) {
    return true;
  }

  try {
    const models = await Promise.race<Array<string>>([
      getAvailableModels(),
      new Promise<Array<string>>((resolve) =>
        setTimeout(() => resolve([]), MODEL_LIST_TIMEOUT_MS),
      ),
    ]);

    // If the timeout fired or no models returned, always consider supported 
    // to avoid false negatives and allow offline usage
    if (models.length === 0) {
      return true;
    }

    // Always include the recommended models
    if (RECOMMENDED_MODELS.includes(model.trim())) {
      return true;
    }

    return models.includes(model.trim());
  } catch (error) {
    // Network or library failure → don't block start‑up.
    // Always return true to avoid blocking the application
    console.error("Error checking model support:", error);
    return true;
  }
}
