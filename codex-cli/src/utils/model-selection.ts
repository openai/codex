import type { AppConfig } from "./config";

/**
 * Get the appropriate model for the selected provider
 *
 * @param config The application configuration
 * @param cliFlags The CLI flags containing model and provider information
 * @returns The model to use
 */
export function getModelForProvider(
  config: AppConfig,
  cliFlags: { model?: string; provider?: string },
): string {
  // CLI model flag takes precedence
  if (cliFlags.model) {
    return cliFlags.model;
  }

  // If provider is specified and there's a default model for it in providerDefaultModels, use that
  if (cliFlags.provider && config.providerDefaultModels?.[cliFlags.provider]) {
    return config.providerDefaultModels[cliFlags.provider] as string;
  }

  // If provider is specified and there's a provider config with defaultModel, use that
  if (
    cliFlags.provider &&
    config.providers?.[cliFlags.provider]?.defaultModel
  ) {
    return config.providers[cliFlags.provider]?.defaultModel as string;
  }

  // Fall back to global default model
  return config.model;
}
