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
  askFollowupQuestionResponse,
  attemptCompletionResponse,
  listCodeDefinitionNamesResponse,
  browserActionResponse,
  mcpToolResponse,
  mcpResourceResponse,
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
  askFollowupQuestionResponse: string;
  attemptCompletionResponse: string;
  listCodeDefinitionNamesResponse: string;
  browserActionResponse: string;
  useMcpToolResponse: string;
  accessMcpResourceResponse: string;
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
  const askFollowupResponse = askFollowupQuestionResponse();
  const attemptCompletionResp = attemptCompletionResponse();
  const listCodeDefinitionResp = listCodeDefinitionNamesResponse();
  const browserActionResp = browserActionResponse();
  const useMcpToolResp = mcpToolResponse();
  const accessMcpResourceResp = mcpResourceResponse();

  return {
    systemPrompt,
    newTaskResponse,
    condenseResponse,
    planModeResponse: planResponse,
    mcpDocumentationResponse: mcpResponse,
    askFollowupQuestionResponse: askFollowupResponse,
    attemptCompletionResponse: attemptCompletionResp,
    listCodeDefinitionNamesResponse: listCodeDefinitionResp,
    browserActionResponse: browserActionResp,
    useMcpToolResponse: useMcpToolResp,
    accessMcpResourceResponse: accessMcpResourceResp,
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
  model?: string,
): string {
  // Convert provider to lowercase for case-insensitive comparison
  const providerLower = provider.toLowerCase();

  // Provider-specific adaptations
  switch (providerLower) {
    case "anthropic":
      // Claude/Anthropic-specific adaptations
      return adaptPromptForClaude(prompt, model);

    case "gemini":
      // Gemini-specific adaptations
      return adaptPromptForGemini(prompt, model);

    case "openai":
    default:
      // Default for OpenAI and other providers
      return adaptPromptForOpenAI(prompt, model);
  }
}

/**
 * Adapts the system prompt for Claude models
 *
 * Claude models have specific preferences for system prompt formatting
 */
function adaptPromptForClaude(prompt: string, _model?: string): string {
  // Claude generally works well with standard formatting,
  // but may benefit from some specific adjustments

  // Make sure the TOOL USE section is extremely clear for Claude
  // Original replacement was causing issues: the test expects this exact string
  let adapted = prompt.replace(
    "TOOL USE",
    "TOOL USE - CRITICALLY IMPORTANT INSTRUCTIONS FOR TOOL USAGE",
  );

  // Claude sometimes needs explicit boundaries and confirmation instructions
  adapted +=
    "\n\n====\n\nREMEMBER: When using tools, WAIT for confirmation after each tool use. Never assume the success of a tool without explicit confirmation of the result.";

  return adapted;
}

/**
 * Adapts the system prompt for Gemini models
 *
 * Gemini models may need specific formatting considerations
 */
function adaptPromptForGemini(prompt: string, _model?: string): string {
  // Gemini models might need simplified instructions in some cases
  // and more explicit step-by-step guidance

  // Add an explicit numbered steps section for tool usage
  let adapted = prompt.replace(
    "TOOL USE",
    "TOOL USE - FOLLOW THESE STEPS PRECISELY",
  );

  // Add more explicit guidance at the end
  adapted +=
    "\n\n====\n\nIMPORTANT GUIDELINES FOR GEMINI MODELS:\n1. Use ONE tool at a time\n2. Wait for the user's confirmation after each tool use\n3. Follow the exact XML format for tool use\n4. Proceed step-by-step";

  return adapted;
}

/**
 * Adapts the system prompt for OpenAI models
 *
 * OpenAI models like GPT-4 work well with the standard format but may benefit
 * from some optimizations
 */
function adaptPromptForOpenAI(prompt: string, _model?: string): string {
  // OpenAI models generally work well with the standard format
  // but we can add some optimizations

  // Instead of replacing the original text which breaks tests expecting the original text,
  // we'll append the OpenAI specific section
  let adapted = prompt;

  // Add an additional final reminder about waiting for confirmation
  adapted +=
    "\n\n====\n\nFINAL REMINDER: Always wait for explicit user confirmation after each tool use before proceeding to the next step.";

  return adapted;
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
