/**
 * Ollama API client for Codex CLI.
 * 
 * This module provides functions to interact with the Ollama API, including
 * listing available models and making chat completion requests.
 */

import type { ResponseItem } from "openai/resources/responses/responses.mjs";
import { OLLAMA_API_URL } from "./config.js";

/**
 * Represents a model available in Ollama.
 */
export type OllamaModel = {
  name: string;
  modified_at: string;
  size: number;
  digest: string;
  details: {
    format: string;
    family: string;
    families: string[];
    parameter_size: string;
    quantization_level: string;
  };
};

/**
 * Represents a message in the Ollama chat API.
 */
export type OllamaMessage = {
  role: "user" | "assistant" | "system";
  content: string;
};

/**
 * Represents a chat completion request to the Ollama API.
 */
export type OllamaChatCompletionRequest = {
  model: string;
  messages: OllamaMessage[];
  stream?: boolean;
  options?: Record<string, unknown>;
};

/**
 * Represents a chat completion response from the Ollama API.
 */
export type OllamaChatCompletionResponse = {
  model: string;
  created_at: string;
  message: OllamaMessage;
  done: boolean;
};

/**
 * Fetches the list of available models from the Ollama API.
 * 
 * @returns A promise that resolves to an array of model names.
 */
export async function fetchOllamaModels(): Promise<string[]> {
  try {
    const response = await fetch(`${OLLAMA_API_URL}/api/tags`);
    
    if (!response.ok) {
      throw new Error(`Failed to fetch Ollama models: ${response.statusText}`);
    }
    
    const data = await response.json() as { models: OllamaModel[] };
    return data.models.map(model => model.name);
  } catch (error) {
    console.error("Error fetching Ollama models:", error);
    return [];
  }
}

/**
 * Converts an Ollama model name to a format compatible with Codex.
 * 
 * @param modelName The original Ollama model name.
 * @returns The prefixed model name (e.g., "ollama:gemma3:1b").
 */
export function formatOllamaModelName(modelName: string): string {
  return `ollama:${modelName}`;
}

/**
 * Extracts the original Ollama model name from a prefixed model name.
 * 
 * @param prefixedModelName The prefixed model name (e.g., "ollama:gemma3:1b").
 * @returns The original Ollama model name or null if the input is not an Ollama model.
 */
export function extractOllamaModelName(prefixedModelName: string): string | null {
  if (!prefixedModelName.startsWith("ollama:")) {
    return null;
  }
  
  return prefixedModelName.substring(7); // Remove "ollama:" prefix
}

/**
 * Checks if a model name refers to an Ollama model.
 * 
 * @param modelName The model name to check.
 * @returns True if the model name refers to an Ollama model, false otherwise.
 */
export function isOllamaModel(modelName: string): boolean {
  return modelName.startsWith("ollama:");
}

/**
 * Converts OpenAI-style response items to Ollama messages.
 * 
 * @param items The OpenAI-style response items.
 * @returns An array of Ollama messages.
 */
export function convertResponseItemsToOllamaMessages(items: ResponseItem[]): OllamaMessage[] {
  const messages: OllamaMessage[] = [];
  
  for (const item of items) {
    if (item.type === "message") {
      let content = "";
      
      for (const contentItem of item.content) {
        if (contentItem.type === "input_text" || contentItem.type === "output_text") {
          content += contentItem.text;
        }
      }
      
      if (content) {
        messages.push({
          role: item.role as "user" | "assistant" | "system",
          content,
        });
      }
    } else if (item.type === "function_call_output") {
      // For function call outputs, we add them as system messages
      messages.push({
        role: "system",
        content: item.output,
      });
    }
  }
  
  return messages;
}

/**
 * Makes a chat completion request to the Ollama API.
 * 
 * @param modelName The name of the Ollama model to use.
 * @param messages The messages to send to the model.
 * @param stream Whether to stream the response.
 * @returns A promise that resolves to the chat completion response.
 */
export async function ollamaChatCompletion(
  modelName: string,
  messages: OllamaMessage[],
  stream = false,
): Promise<OllamaChatCompletionResponse | ReadableStream> {
  const actualModelName = extractOllamaModelName(modelName) || modelName;
  
  const requestBody: OllamaChatCompletionRequest = {
    model: actualModelName,
    messages,
    stream,
  };
  
  const response = await fetch(`${OLLAMA_API_URL}/api/chat`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(requestBody),
  });
  
  if (!response.ok) {
    throw new Error(`Ollama API error: ${response.statusText}`);
  }
  
  if (stream) {
    return response.body as ReadableStream;
  }
  
  return await response.json() as OllamaChatCompletionResponse;
}
