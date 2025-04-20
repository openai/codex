/**
 * Provider interface for multi-provider support in Codex CLI.
 * This interface defines the contract that all LLM providers must implement.
 */

import type { AppConfig } from "../config.js";
import type {
  ResponseFunctionToolCall,
  ResponseInputItem,
  ResponseItem,
} from "openai/resources/responses/responses.mjs";

/**
 * Core interface that all LLM providers must implement
 */
export interface LLMProvider {
  /** Unique identifier for the provider */
  id: string;
  
  /** Display name for the provider */
  name: string;
  
  /**
   * Get available models from this provider
   * @returns Promise resolving to an array of model identifiers
   */
  getModels(): Promise<string[]>;
  
  /**
   * Check if a model is supported by this provider
   * @param model Model identifier to check
   * @returns Promise resolving to true if the model is supported
   */
  isModelSupported(model: string): Promise<boolean>;
  
  /**
   * Create a client instance for this provider
   * @param config Application configuration
   * @returns Provider-specific client instance
   */
  createClient(config: AppConfig): any;
  
  /**
   * Execute a completion request
   * @param params Completion parameters
   * @returns Promise resolving to a stream of completion events
   */
  runCompletion(params: CompletionParams): Promise<any>;
  
  /**
   * Get default configuration for a specific model
   * @param model Model identifier
   * @returns Model-specific default settings
   */
  getModelDefaults(model: string): ModelDefaults;
  
  /**
   * Check if an error is a rate limit error for this provider
   * @param error Provider-specific error
   * @returns True if the error is a rate limit error
   */
  isRateLimitError(error: any): boolean;
  
  /**
   * Check if an error is a timeout error for this provider
   * @param error Provider-specific error
   * @returns True if the error is a timeout error
   */
  isTimeoutError(error: any): boolean;
  
  /**
   * Check if an error is a connection error for this provider
   * @param error Provider-specific error
   * @returns True if the error is a connection error
   */
  isConnectionError(error: any): boolean;
  
  /**
   * Check if an error is a context length error for this provider
   * @param error Provider-specific error
   * @returns True if the error is a context length error
   */
  isContextLengthError(error: any): boolean;
  
  /**
   * Check if an error is an invalid request error for this provider
   * @param error Provider-specific error
   * @returns True if the error is an invalid request error
   */
  isInvalidRequestError(error: any): boolean;
  
  /**
   * Format an error message for user display
   * @param error Provider-specific error
   * @returns User-friendly error message
   */
  formatErrorMessage(error: any): string;
  
  /**
   * Get the recommended wait time for rate limit errors
   * @param error Provider-specific error
   * @returns Recommended wait time in milliseconds
   */
  getRetryAfterMs(error: any): number;
  
  /**
   * Parse a tool call from provider-specific format to common format
   * @param rawToolCall Provider-specific tool call
   * @returns Normalized tool call
   */
  parseToolCall(rawToolCall: any): ParsedToolCall;
  
  /**
   * Format tools into provider-specific format
   * @param tools Array of tools in common format
   * @returns Tools in provider-specific format
   */
  formatTools(tools: Tool[]): any;
  
  /**
   * Normalize a stream event from provider-specific format to common format
   * @param event Provider-specific stream event
   * @returns Normalized event in common format
   */
  normalizeStreamEvent(event: any): NormalizedStreamEvent;
}

/**
 * Common parameters for completion requests
 */
export interface CompletionParams {
  /** Model identifier */
  model: string;
  
  /** Messages for the conversation */
  messages: Message[];
  
  /** Tools available to the model */
  tools?: Tool[];
  
  /** Temperature parameter (0.0-2.0) */
  temperature?: number;
  
  /** Whether to stream the response */
  stream?: boolean;
  
  /** Previous response ID for continuations */
  previousResponseId?: string;
  
  /** Max tokens to generate */
  maxTokens?: number;
  
  /** Whether to use parallel tool calls */
  parallelToolCalls?: boolean;
  
  /** Reasoning settings */
  reasoning?: ReasoningSettings;
  
  /** Application configuration */
  config: AppConfig;
}

/**
 * Model defaults for provider configuration
 */
export interface ModelDefaults {
  /** Default timeout for this model in milliseconds */
  timeoutMs: number;
  
  /** Default temperature for this model */
  temperature?: number;
  
  /** Maximum tokens supported by this model */
  maxTokens?: number;
  
  /** Whether this model supports tool/function calling */
  supportsToolCalls: boolean;
  
  /** Whether this model supports streaming */
  supportsStreaming: boolean;
  
  /** Maximum context window size in tokens */
  contextWindowSize: number;
}

/**
 * Message representation for providers
 */
export interface Message {
  /** Message role */
  role: "system" | "user" | "assistant" | "function" | "tool";
  
  /** Message content */
  content: string | MessageContent[];
  
  /** Optional name for the message sender */
  name?: string;
  
  /** Tool outputs (for tool messages) */
  toolOutputs?: ToolOutput[];
}

/**
 * Structure for message content items
 */
export interface MessageContent {
  /** Content type */
  type: "text" | "image_url";
  
  /** Text content */
  text?: string;
  
  /** Image URL data */
  image_url?: {
    url: string;
    detail?: "low" | "high" | "auto";
  };
}

/**
 * Tool definition for providers
 */
export interface Tool {
  /** Tool type */
  type: "function";
  
  /** Tool name */
  name: string;
  
  /** Tool description */
  description?: string;
  
  /** Parameters schema */
  parameters: object;
  
  /** Whether to use strict parameter validation */
  strict?: boolean;
}

/**
 * Normalized tool call for consistent handling
 */
export interface ParsedToolCall {
  /** Tool call ID */
  id: string;
  
  /** Tool name */
  name: string;
  
  /** Tool arguments */
  arguments: any;
}

/**
 * Structure for tool outputs
 */
export interface ToolOutput {
  /** Tool call ID this output corresponds to */
  tool_call_id: string;
  
  /** Output content */
  output: string;
}

/**
 * Reasoning settings for models that support it
 */
export interface ReasoningSettings {
  /** Reasoning effort level */
  effort?: "low" | "medium" | "high" | "auto";
  
  /** Whether to include reasoning summary */
  summary?: boolean;
}

/**
 * Normalized stream event type for consistent handling
 */
export interface NormalizedStreamEvent {
  /** Event type */
  type: "text" | "tool_call" | "completion" | "error";
  
  /** Event content */
  content?: string | object;
  
  /** Response ID */
  responseId?: string;
  
  /** Original event */
  originalEvent: any;
}

/**
 * Provider error types for standardized error handling
 */
export enum ProviderErrorType {
  AUTHENTICATION = "authentication_error",
  RATE_LIMIT = "rate_limit_error",
  CONTEXT_LENGTH = "context_length_error",
  INVALID_REQUEST = "invalid_request_error",
  SERVER = "server_error",
  NETWORK = "network_error",
  TIMEOUT = "timeout_error",
  UNKNOWN = "unknown_error",
}

/**
 * Standardized error for consistent error handling
 */
export interface StandardizedError {
  /** Error type */
  type: ProviderErrorType;
  
  /** Error message */
  message: string;
  
  /** Original error */
  originalError: any;
  
  /** Whether the error is retryable */
  retryable: boolean;
  
  /** Suggested wait time in milliseconds before retry */
  suggestedWaitMs?: number;
}