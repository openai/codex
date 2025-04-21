/**
 * Provider configuration system for multi-provider support
 * 
 * This module handles loading and managing configurations for different LLM providers
 * in a way that's extensible and not hardcoded to specific providers.
 */

import { readFileSync } from 'fs';

/**
 * Provider configuration with common settings and extensible properties
 */
export interface ProviderConfig {
  apiKey?: string;
  baseUrl?: string;
  timeoutMs?: number;
  defaultModel?: string;
  // Claude-specific configuration
  enableToolCalls?: boolean;
  // Allow additional provider-specific settings
  [key: string]: any;
}

/**
 * Environment variable mapping for provider settings
 * Maps each config property to a list of environment variable names to check
 */
export interface ProviderEnvMapping {
  apiKey: string[];      // List of env var names to check for API key
  baseUrl?: string[];    // List of env var names to check for base URL
  timeoutMs?: string[];  // List of env var names to check for timeout
  [key: string]: string[] | undefined;
}

/**
 * Registry of provider environment variable mappings
 */
export const PROVIDER_ENV_MAPPINGS: Record<string, ProviderEnvMapping> = {
  openai: {
    apiKey: ["OPENAI_API_KEY"],
    baseUrl: ["OPENAI_BASE_URL"],
    timeoutMs: ["OPENAI_TIMEOUT_MS"],
  },
  claude: {
    apiKey: ["CLAUDE_API_KEY", "ANTHROPIC_API_KEY"],
    baseUrl: ["CLAUDE_BASE_URL", "ANTHROPIC_BASE_URL"],
    timeoutMs: ["CLAUDE_TIMEOUT_MS", "ANTHROPIC_TIMEOUT_MS"],
  },
};

/**
 * Default models for each provider
 */
export const DEFAULT_PROVIDER_MODELS: Record<string, string> = {
  openai: "o4-mini",
  claude: "claude-3-5-sonnet-20240620", // Using the most capable non-deprecated Claude model
};

/**
 * Default provider to use if none is specified
 */
export const DEFAULT_PROVIDER_ID = process.env.CODEX_DEFAULT_PROVIDER || "openai";

/**
 * Default model to use for each provider if none is specified
 * This is used when a provider is selected but no model is specified
 */
export const DEFAULT_MODEL = process.env.CODEX_DEFAULT_MODEL || 
  (DEFAULT_PROVIDER_ID === "claude" ? DEFAULT_PROVIDER_MODELS.claude : DEFAULT_PROVIDER_MODELS.openai);

/**
 * Load provider configuration from environment variables and stored config
 * @param providerId Provider identifier
 * @param storedConfig Optional stored configuration to use as base
 * @returns Merged provider configuration
 */
export function loadProviderConfig(
  providerId: string,
  storedConfig?: ProviderConfig
): ProviderConfig {
  const result: ProviderConfig = { ...storedConfig };
  const envMapping = PROVIDER_ENV_MAPPINGS[providerId];
  
  if (envMapping) {
    // Check each config property against its environment variables
    for (const [key, envVars] of Object.entries(envMapping)) {
      if (!envVars) continue;
      
      // Try each environment variable in order, use first non-empty value
      for (const envVar of envVars) {
        const value = process.env[envVar];
        if (value) {
          if (key === 'timeoutMs') {
            result[key] = parseInt(value, 10) || undefined;
          } else {
            result[key] = value;
          }
          break;
        }
      }
    }
  }
  
  return result;
}

/**
 * Load all provider configurations from stored config and environment variables
 * @param storedProviders Provider configurations from stored config
 * @returns Map of provider IDs to their configurations
 */
export function loadAllProviderConfigs(
  storedProviders: Record<string, ProviderConfig> = {}
): Record<string, ProviderConfig> {
  const result: Record<string, ProviderConfig> = {};
  
  // Get the set of all provider IDs from both sources
  const providerIds = new Set([
    ...Object.keys(PROVIDER_ENV_MAPPINGS),
    ...Object.keys(storedProviders)
  ]);
  
  // Process each provider
  for (const providerId of providerIds) {
    result[providerId] = loadProviderConfig(
      providerId, 
      storedProviders[providerId]
    );
  }
  
  return result;
}

/**
 * Get the appropriate provider for a model
 * @param model Model identifier
 * @returns Provider ID that supports this model
 */
export function getProviderIdForModel(model: string): string {
  // Simple pattern matching for now, can be made more sophisticated
  if (model.startsWith("claude")) {
    return "claude";
  }
  
  // Default to OpenAI for all other models
  return "openai";
}

/**
 * Get the default model for a provider
 * @param providerId Provider identifier
 * @returns Default model for the provider
 */
export function getDefaultModelForProvider(providerId: string): string {
  return DEFAULT_PROVIDER_MODELS[providerId] || DEFAULT_PROVIDER_MODELS.openai;
}