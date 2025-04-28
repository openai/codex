/**
 * Prompt loader for Codex CLI
 *
 * This module handles loading appropriate prompts for different model providers and contexts.
 */

import {
  newTaskToolResponse,
  condenseToolResponse,
  planModeResponse,
  mcpDocumentationResponse,
} from "./commands";
import { generateSystemPrompt, addUserInstructions } from "./system";

/**
 * Options for loading prompts
 */
export interface PromptLoaderOptions {
  cwd: string;
  provider?: string;
  model?: string;
  supportsBrowserUse?: boolean;
  userInstructions?: string;
  mcpServers?: Array<string>;
}

/**
 * Prompt collection for the different prompt types
 */
export interface PromptCollection {
  systemPrompt: string;
  newTaskResponse: string;
  condenseResponse: string;
  planModeResponse: string;
  mcpDocumentationResponse: string;
}

/**
 * Loads prompts for the given provider and model
 */
export function loadPrompts(options: PromptLoaderOptions): PromptCollection {
  const { cwd, supportsBrowserUse = true, userInstructions = "" } = options;

  // Generate base system prompt
  const baseSystemPrompt = generateSystemPrompt({
    cwd,
    supportsBrowserUse,
    browserSettings: { viewport: { width: 900, height: 600 } },
  });

  // Add user instructions if provided
  const systemPrompt = userInstructions
    ? addUserInstructions(baseSystemPrompt, userInstructions)
    : baseSystemPrompt;

  // Prepare all prompt responses
  const newTaskResponse = newTaskToolResponse();
  const condenseResponse = condenseToolResponse();
  const planResponse = planModeResponse();
  const mcpResponse = mcpDocumentationResponse();

  return {
    systemPrompt,
    newTaskResponse,
    condenseResponse,
    planModeResponse: planResponse,
    mcpDocumentationResponse: mcpResponse,
  };
}

/**
 * Adapts the system prompt for a specific model provider
 *
 * This ensures that prompts are optimized for each model's capabilities and quirks
 */
export function adaptPromptForProvider(
  prompt: string,
  provider: string,
  _model?: string,
): string {
  // Convert provider to lowercase for case-insensitive comparison
  const providerLower = provider.toLowerCase();

  // Provider-specific adaptations
  switch (providerLower) {
    case "anthropic":
      // Claude/Anthropic-specific adaptations
      // Claude models may need different formatting for certain instructions
      return prompt;

    case "gemini":
      // Gemini-specific adaptations
      return prompt;

    case "openai":
    default:
      // Default for OpenAI and other providers
      return prompt;
  }
}

/**
 * Returns the appropriate provider for a given model
 */
export function getProviderForModel(model: string): string {
  const modelLower = model.toLowerCase();

  if (modelLower.includes("claude")) {
    return "anthropic";
  }

  if (modelLower.includes("gemini")) {
    return "gemini";
  }

  return "openai";
}
