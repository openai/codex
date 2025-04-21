/**
 * OpenAI provider implementation for Codex CLI
 * This refactors the existing OpenAI-specific code into the provider abstraction
 */

import OpenAI, { 
  APIConnectionTimeoutError, 
  APIError 
} from "openai";

import { 
  CompletionParams,
  ModelDefaults,
  NormalizedStreamEvent,
  ParsedToolCall,
  Tool 
} from "./provider-interface.js";
import { BaseProvider } from "./base-provider.js";

import type { 
  ResponseFunctionToolCall,
  ResponseItem
} from "openai/resources/responses/responses.mjs";

import type { 
  Reasoning 
} from "openai/resources.mjs";

import type { 
  AppConfig 
} from "../config.js";

import type { ProviderConfig } from "../provider-config.js";

import { 
  ORIGIN, 
  CLI_VERSION 
} from "../session.js";

/**
 * OpenAI provider implementation
 */
export class OpenAIProvider extends BaseProvider {
  id = "openai";
  name = "OpenAI";
  
  /**
   * Get available models from OpenAI
   * @returns Promise resolving to an array of model identifiers
   */
  async getModels(): Promise<string[]> {
    // If the user has not configured an API key, return recommended models
    const apiKey = this.getApiKey();
    if (!apiKey) {
      return this.getRecommendedModels();
    }
    
    try {
      const openai = this.createClient({
        providers: {
          openai: { apiKey }
        }
      } as AppConfig);
      
      const list = await openai.models.list();
      
      const models: Array<string> = [];
      for await (const model of list as AsyncIterable<{ id?: string }>) {
        if (model && typeof model.id === "string") {
          models.push(model.id);
        }
      }
      
      return models.sort();
    } catch {
      return this.getRecommendedModels();
    }
  }
  
  /**
   * Get recommended models for OpenAI
   * @returns Array of recommended model identifiers
   */
  private getRecommendedModels(): string[] {
    return ["o4-mini", "o3", "gpt-4", "gpt-3.5-turbo"];
  }
  
  /**
   * Create an OpenAI client
   * @param config Application configuration
   * @returns OpenAI client instance
   */
  createClient(config: AppConfig): OpenAI {
    // Get provider config from AppConfig
    const providerConfig = config.providers?.openai || {};
    
    // Get API key from provider config or environment
    const apiKey = providerConfig.apiKey || process.env.OPENAI_API_KEY;
    if (!apiKey) {
      throw new Error("OpenAI API key not found. Please set OPENAI_API_KEY environment variable or configure it in the Codex config.");
    }
    
    // Get base URL and timeout from provider config
    const baseURL = providerConfig.baseUrl || undefined;
    const timeout = providerConfig.timeoutMs;
    
    // Get session information
    const sessionId = config.sessionId;
    
    // Create OpenAI client
    return new OpenAI({
      apiKey,
      baseURL,
      defaultHeaders: {
        originator: ORIGIN,
        version: CLI_VERSION,
        session_id: sessionId,
      },
      ...(timeout !== undefined ? { timeout } : {}),
    });
  }
  
  /**
   * Execute a completion request
   * @param params Completion parameters
   * @returns Promise resolving to a stream of completion events
   */
  async runCompletion(params: CompletionParams): Promise<any> {
    const client = this.createClient(params.config);
    
    // Convert messages to instructions and input items
    const { instructions, input } = this.prepareCompletionParams(params);
    
    // Prepare tools
    const tools = this.formatTools(params.tools || []);
    
    // Prepare reasoning settings
    let reasoning: Reasoning | undefined;
    if (params.reasoning) {
      reasoning = { 
        effort: params.reasoning.effort || "high" 
      };
      
      if (params.reasoning.summary !== undefined) {
        // @ts-expect-error - Summary is valid but not in types yet
        reasoning.summary = params.reasoning.summary;
      }
    } else if (params.model.startsWith("o")) {
      // Default reasoning for 'o' models
      reasoning = { effort: "high" };
      
      if (params.model === "o3" || params.model === "o4-mini") {
        // @ts-expect-error - Summary is valid but not in types yet
        reasoning.summary = "auto";
      }
    }
    
    // Create stream
    return client.responses.create({
      model: params.model,
      instructions,
      previous_response_id: params.previousResponseId || undefined,
      input,
      stream: params.stream !== false, // Default to true
      parallel_tool_calls: params.parallelToolCalls || false,
      reasoning,
      tools,
    });
  }
  
  /**
   * Prepare completion parameters for OpenAI
   * @param params Completion parameters
   * @returns Prepared instructions and input
   */
  private prepareCompletionParams(params: CompletionParams): { 
    instructions: string, 
    input: any[] 
  } {
    // Extract system message as instructions
    let instructions = "";
    const input: any[] = [];
    
    // Process messages
    for (const message of params.messages) {
      if (message.role === "system") {
        // Use system message as instructions
        if (typeof message.content === "string") {
          instructions = message.content;
        }
      } else {
        // Add non-system messages to input
        input.push(this.convertMessageToInputItem(message));
      }
    }
    
    return { instructions, input };
  }
  
  /**
   * Convert a message to an input item for OpenAI
   * @param message Message to convert
   * @returns Input item for OpenAI
   */
  private convertMessageToInputItem(message: any): any {
    // For now, assuming the message is already in OpenAI format
    // This will need to be expanded as we support more complex message types
    return message;
  }
  
  /**
   * Get model defaults for OpenAI
   * @param model Model identifier
   * @returns Model defaults
   */
  getModelDefaults(model: string): ModelDefaults {
    // Base defaults for all OpenAI models
    const baseDefaults: ModelDefaults = {
      timeoutMs: 60000, // 1 minute
      temperature: 0.7,
      supportsToolCalls: true,
      supportsStreaming: true,
      contextWindowSize: 16000, // Default for older models
    };
    
    // Model-specific overrides
    switch (model) {
      case "gpt-4":
      case "gpt-4-1106-preview":
      case "gpt-4-0613":
        return {
          ...baseDefaults,
          contextWindowSize: 8000,
        };
      case "gpt-4-32k":
      case "gpt-4-32k-0613":
        return {
          ...baseDefaults,
          contextWindowSize: 32000,
        };
      case "gpt-3.5-turbo":
      case "gpt-3.5-turbo-1106":
      case "gpt-3.5-turbo-0613":
        return {
          ...baseDefaults,
          contextWindowSize: 16000,
        };
      case "o4-mini":
        return {
          ...baseDefaults,
          contextWindowSize: 128000,
        };
      case "o3":
        return {
          ...baseDefaults,
          contextWindowSize: 64000,
        };
      default:
        // Default for any new models
        return baseDefaults;
    }
  }
  
  /**
   * Parse a tool call from OpenAI format to common format
   * @param toolCall OpenAI tool call
   * @returns Normalized tool call
   */
  parseToolCall(toolCall: ResponseFunctionToolCall): ParsedToolCall {
    // The OpenAI "function_call" item may have either `call_id` (responses
    // endpoint) or `id` (chat endpoint). Prefer `call_id` if present but fall
    // back to `id` to remain compatible.
    const isChatStyle = 
      // The chat endpoint nests function details under a `function` key.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (toolCall as any).function != null;
    
    const name: string | undefined = isChatStyle
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ? (toolCall as any).function?.name
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      : (toolCall as any).name;
    
    const rawArguments: string | undefined = isChatStyle
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ? (toolCall as any).function?.arguments
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      : (toolCall as any).arguments;
    
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const callId: string = (toolCall as any).call_id ?? (toolCall as any).id;
    
    // Parse arguments
    let args = {};
    try {
      args = JSON.parse(rawArguments || "{}");
    } catch {
      console.error(`Failed to parse tool call arguments: ${rawArguments}`);
    }
    
    return {
      id: callId,
      name: name || "",
      arguments: args,
    };
  }
  
  /**
   * Format tools into OpenAI format
   * @param tools Array of tools in common format
   * @returns Tools in OpenAI format
   */
  formatTools(tools: Tool[]): any[] {
    // Tools are already in OpenAI format, just return them
    return tools;
  }
  
  /**
   * Normalize a stream event from OpenAI format to common format
   * @param event OpenAI stream event
   * @returns Normalized event
   */
  normalizeStreamEvent(event: any): NormalizedStreamEvent {
    if (event.type === "response.output_item.done") {
      const item = event.item;
      
      if (item.type === "function_call") {
        return {
          type: "tool_call",
          content: item,
          responseId: event.response_id,
          originalEvent: event,
        };
      } else {
        return {
          type: "text",
          content: item,
          responseId: event.response_id,
          originalEvent: event,
        };
      }
    } else if (event.type === "response.completed") {
      return {
        type: "completion",
        content: event.response,
        responseId: event.response.id,
        originalEvent: event,
      };
    } else {
      return {
        type: "text",
        content: event,
        responseId: event.response_id,
        originalEvent: event,
      };
    }
  }
  
  /**
   * Check if an error is a rate limit error
   * @param error Error to check
   * @returns True if it's a rate limit error
   */
  isRateLimitError(error: any): boolean {
    return (
      error?.status === 429 ||
      error?.code === "rate_limit_exceeded" ||
      error?.type === "rate_limit_exceeded" ||
      /rate limit/i.test(error?.message || "")
    );
  }
  
  /**
   * Check if an error is a timeout error
   * @param error Error to check
   * @returns True if it's a timeout error
   */
  isTimeoutError(error: any): boolean {
    return (
      error instanceof APIConnectionTimeoutError ||
      error?.code === "ETIMEDOUT" ||
      error?.code === "ESOCKETTIMEDOUT" ||
      /timeout/i.test(error?.message || "")
    );
  }
  
  /**
   * Check if an error is a connection error
   * @param error Error to check
   * @returns True if it's a connection error
   */
  isConnectionError(error: any): boolean {
    // Check for APIConnectionError
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const ApiConnErrCtor = (OpenAI as any).APIConnectionError as  
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      | (new (...args: any) => Error)
      | undefined;
      
    const isConnectionError = ApiConnErrCtor
      ? error instanceof ApiConnErrCtor
      : false;
    
    return (
      isConnectionError ||
      error?.code === "ECONNRESET" ||
      error?.code === "ECONNREFUSED" ||
      error?.code === "ENOTFOUND" ||
      error?.code === "EPIPE" ||
      error?.cause?.code === "ECONNRESET" ||
      error?.cause?.code === "ECONNREFUSED" ||
      error?.cause?.code === "ENOTFOUND" ||
      error?.cause?.code === "EPIPE" ||
      /network/i.test(error?.message || "") ||
      /socket/i.test(error?.message || "") ||
      /connection/i.test(error?.message || "")
    );
  }
  
  /**
   * Check if an error is a context length error
   * @param error Error to check
   * @returns True if it's a context length error
   */
  isContextLengthError(error: any): boolean {
    return (
      error?.param === "max_tokens" ||
      (typeof error?.message === "string" &&
        /max_tokens is too large/i.test(error?.message)) ||
      /context length exceeded/i.test(error?.message || "")
    );
  }
  
  /**
   * Check if an error is an invalid request error
   * @param error Error to check
   * @returns True if it's an invalid request error
   */
  isInvalidRequestError(error: any): boolean {
    return (
      (typeof error?.status === "number" &&
        error.status >= 400 &&
        error.status < 500 &&
        error.status !== 429) ||
      error?.code === "invalid_request_error" ||
      error?.type === "invalid_request_error"
    );
  }
  
  /**
   * Format an error message for user display
   * @param error Error to format
   * @returns User-friendly error message
   */
  formatErrorMessage(error: any): string {
    // Handle known error types
    if (this.isRateLimitError(error)) {
      return `OpenAI rate limit exceeded. Please try again later.`;
    }
    
    if (this.isTimeoutError(error)) {
      return `Request to OpenAI timed out. Please try again.`;
    }
    
    if (this.isContextLengthError(error)) {
      return `The current request exceeds the maximum context length supported by the chosen model. Please shorten the conversation or switch to a model with a larger context window.`;
    }
    
    if (this.isConnectionError(error)) {
      return `Network error while contacting OpenAI. Please check your connection and try again.`;
    }
    
    // Extract status, code, type, and message for detailed error info
    const reqId = error?.request_id ?? error?.requestId;
    const status = error?.status ?? error?.httpStatus ?? error?.statusCode;
    const code = error?.code || "unknown";
    const type = error?.type || "unknown";
    const message = error?.message || "unknown";
    
    // Format detailed error message
    const details = [
      `Status: ${status || "unknown"}`,
      `Code: ${code}`,
      `Type: ${type}`,
      `Message: ${message}`,
    ].join(", ");
    
    // Create user-friendly message with request ID if available
    return `OpenAI error${reqId ? ` (request ID: ${reqId})` : ""}: ${details}`;
  }
  
  /**
   * Get the suggested wait time for rate limit errors
   * @param error Rate limit error
   * @returns Suggested wait time in milliseconds
   */
  getRetryAfterMs(error: any): number {
    const RATE_LIMIT_RETRY_WAIT_MS = parseInt(
      process.env["OPENAI_RATE_LIMIT_RETRY_WAIT_MS"] || "2500",
      10
    );
    
    // Parse retry-after from headers
    const retryAfter = error?.headers?.["retry-after"];
    if (retryAfter && !isNaN(parseInt(retryAfter, 10))) {
      return parseInt(retryAfter, 10) * 1000;
    }
    
    // Parse suggested retry time from error message
    const msg = error?.message ?? "";
    const m = /(?:retry|try) again in ([\d.]+)s/i.exec(msg);
    if (m && m[1]) {
      const suggested = parseFloat(m[1]) * 1000;
      if (!isNaN(suggested)) {
        return suggested;
      }
    }
    
    // Default to configured retry wait time
    return RATE_LIMIT_RETRY_WAIT_MS;
  }
  
  /**
   * Get the API key from config or environment
   * @param config Optional config object
   * @returns API key or undefined
   */
  private getApiKey(config?: AppConfig): string | undefined {
    if (config?.providers?.openai?.apiKey) {
      return config.providers.openai.apiKey;
    }
    return process.env["OPENAI_API_KEY"];
  }
}