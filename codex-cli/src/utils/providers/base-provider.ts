/**
 * Base provider implementation with common functionality
 */

import type { AppConfig } from "../config.js";
import {
  LLMProvider,
  CompletionParams,
  ModelDefaults,
  ParsedToolCall,
  Tool,
  NormalizedStreamEvent,
  ProviderErrorType,
  StandardizedError,
} from "./provider-interface.js";

/**
 * Abstract base class implementing common functionality for all providers
 */
export abstract class BaseProvider implements LLMProvider {
  /** Unique provider identifier */
  abstract id: string;
  
  /** Display name for the provider */
  abstract name: string;
  
  /**
   * Get available models from this provider
   */
  abstract getModels(): Promise<string[]>;
  
  /**
   * Create a client instance for this provider
   */
  abstract createClient(config: AppConfig): any;
  
  /**
   * Execute a completion request
   */
  abstract runCompletion(params: CompletionParams): Promise<any>;
  
  /**
   * Get default configuration for a specific model
   */
  abstract getModelDefaults(model: string): ModelDefaults;
  
  /**
   * Parse a tool call from provider-specific format to common format
   */
  abstract parseToolCall(rawToolCall: any): ParsedToolCall;
  
  /**
   * Format tools into provider-specific format
   */
  abstract formatTools(tools: Tool[]): any;
  
  /**
   * Normalize a stream event from provider-specific format to common format
   */
  abstract normalizeStreamEvent(event: any): NormalizedStreamEvent;
  
  /**
   * Check if a model is supported by this provider
   * Default implementation that can be overridden
   */
  async isModelSupported(model: string): Promise<boolean> {
    try {
      const models = await this.getModels();
      return models.includes(model);
    } catch (error) {
      console.error(`Error checking if model ${model} is supported:`, error);
      return false;
    }
  }
  
  /**
   * Check if an error is a rate limit error for this provider
   * Default implementation that should be overridden by specific providers
   */
  isRateLimitError(error: any): boolean {
    // Default implementation looks for common patterns
    return (
      error?.status === 429 ||
      error?.statusCode === 429 ||
      error?.code === "rate_limit_exceeded" ||
      error?.type === "rate_limit_exceeded" ||
      /rate limit/i.test(error?.message || "") ||
      /too many requests/i.test(error?.message || "")
    );
  }
  
  /**
   * Check if an error is a timeout error for this provider
   * Default implementation that should be overridden by specific providers
   */
  isTimeoutError(error: any): boolean {
    // Default implementation looks for common patterns
    return (
      error?.name === "AbortError" ||
      error?.code === "ETIMEDOUT" ||
      error?.code === "ESOCKETTIMEDOUT" ||
      /timeout/i.test(error?.message || "") ||
      /timed out/i.test(error?.message || "")
    );
  }
  
  /**
   * Check if an error is a connection error for this provider
   * Default implementation that should be overridden by specific providers
   */
  isConnectionError(error: any): boolean {
    // Default implementation looks for common patterns
    return (
      error?.code === "ECONNRESET" ||
      error?.code === "ECONNREFUSED" ||
      error?.code === "ENOTFOUND" ||
      error?.code === "EPIPE" ||
      error?.code === "ENETUNREACH" ||
      /network/i.test(error?.message || "") ||
      /connection/i.test(error?.message || "")
    );
  }
  
  /**
   * Check if an error is a context length error for this provider
   * Default implementation that should be overridden by specific providers
   */
  isContextLengthError(error: any): boolean {
    // Default implementation looks for common patterns
    return (
      error?.code === "context_length_exceeded" ||
      /maximum context length/i.test(error?.message || "") ||
      /too many tokens/i.test(error?.message || "") ||
      /context window is full/i.test(error?.message || "")
    );
  }
  
  /**
   * Check if an error is an invalid request error for this provider
   * Default implementation that should be overridden by specific providers
   */
  isInvalidRequestError(error: any): boolean {
    // Default implementation looks for common patterns
    return (
      (error?.status >= 400 && error?.status < 500 && error?.status !== 429) ||
      error?.type === "invalid_request_error" ||
      error?.code === "invalid_request_error"
    );
  }
  
  /**
   * Format an error message for user display
   * Default implementation that should be overridden by specific providers
   */
  formatErrorMessage(error: any): string {
    if (error?.message) {
      return `API Error: ${error.message}`;
    } else if (error?.error?.message) {
      return `API Error: ${error.error.message}`;
    } else {
      return "Unknown API error occurred";
    }
  }
  
  /**
   * Get the recommended wait time for rate limit errors
   * Default implementation that should be overridden by specific providers
   */
  getRetryAfterMs(error: any): number {
    // Check for retry-after header (in seconds)
    const retryAfter = error?.headers?.["retry-after"] || 
                       error?.response?.headers?.["retry-after"];
    
    if (retryAfter && !isNaN(parseInt(retryAfter, 10))) {
      return parseInt(retryAfter, 10) * 1000;
    }
    
    // Check for retry-after in error message
    const message = error?.message || "";
    const match = /(?:retry|try) again in ([\d.]+)s/i.exec(message);
    if (match && match[1] && !isNaN(parseFloat(match[1]))) {
      return parseFloat(match[1]) * 1000;
    }
    
    // Default to 2.5 seconds
    return 2500;
  }
  
  /**
   * Standardize an error from provider-specific format
   * @param error Provider-specific error
   * @returns Standardized error
   */
  standardizeError(error: any): StandardizedError {
    if (this.isRateLimitError(error)) {
      return {
        type: ProviderErrorType.RATE_LIMIT,
        message: this.formatErrorMessage(error),
        originalError: error,
        retryable: true,
        suggestedWaitMs: this.getRetryAfterMs(error),
      };
    } else if (this.isTimeoutError(error)) {
      return {
        type: ProviderErrorType.TIMEOUT,
        message: this.formatErrorMessage(error),
        originalError: error,
        retryable: true,
      };
    } else if (this.isConnectionError(error)) {
      return {
        type: ProviderErrorType.NETWORK,
        message: this.formatErrorMessage(error),
        originalError: error,
        retryable: true,
      };
    } else if (this.isContextLengthError(error)) {
      return {
        type: ProviderErrorType.CONTEXT_LENGTH,
        message: this.formatErrorMessage(error),
        originalError: error,
        retryable: false,
      };
    } else if (this.isInvalidRequestError(error)) {
      return {
        type: ProviderErrorType.INVALID_REQUEST,
        message: this.formatErrorMessage(error),
        originalError: error,
        retryable: false,
      };
    } else if (error?.status >= 500) {
      return {
        type: ProviderErrorType.SERVER,
        message: this.formatErrorMessage(error),
        originalError: error,
        retryable: true,
      };
    } else if (error?.status === 401 || error?.status === 403) {
      return {
        type: ProviderErrorType.AUTHENTICATION,
        message: this.formatErrorMessage(error),
        originalError: error,
        retryable: false,
      };
    } else {
      return {
        type: ProviderErrorType.UNKNOWN,
        message: this.formatErrorMessage(error),
        originalError: error,
        retryable: false,
      };
    }
  }
}