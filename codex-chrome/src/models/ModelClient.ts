/**
 * Model Client base interface and types for codex-chrome
 * Based on contract tests and codex-rs model client implementation
 */

/**
 * Request configuration for completion API calls
 */
export interface CompletionRequest {
  /** The model to use for completion */
  model: string;
  /** Array of messages in the conversation */
  messages: Message[];
  /** Sampling temperature between 0 and 2 */
  temperature?: number;
  /** Maximum number of tokens to generate */
  maxTokens?: number;
  /** Tools available to the model */
  tools?: ToolDefinition[];
  /** Whether to stream the response */
  stream?: boolean;
}

/**
 * Response from completion API calls
 */
export interface CompletionResponse {
  /** Unique identifier for the completion */
  id: string;
  /** Model used for the completion */
  model: string;
  /** Array of completion choices */
  choices: Choice[];
  /** Token usage information */
  usage: Usage;
}

/**
 * Message in a conversation
 */
export interface Message {
  /** Role of the message sender */
  role: 'system' | 'user' | 'assistant' | 'tool';
  /** Content of the message (null for tool calls without content) */
  content: string | null;
  /** Tool calls made by the assistant */
  toolCalls?: ToolCall[];
  /** ID of the tool call this message responds to */
  toolCallId?: string;
}

/**
 * A single completion choice
 */
export interface Choice {
  /** Index of this choice */
  index: number;
  /** Message for this choice */
  message: Message;
  /** Reason the completion finished */
  finishReason: 'stop' | 'length' | 'tool_calls' | 'content_filter';
}

/**
 * Token usage statistics
 */
export interface Usage {
  /** Number of tokens in the prompt */
  promptTokens: number;
  /** Number of tokens in the completion */
  completionTokens: number;
  /** Total number of tokens used */
  totalTokens: number;
}

/**
 * Tool definition for function calling
 */
export interface ToolDefinition {
  /** Type of tool (currently only function supported) */
  type: 'function';
  /** Function definition */
  function: {
    /** Name of the function */
    name: string;
    /** Description of what the function does */
    description: string;
    /** JSON schema for the function parameters */
    parameters: any;
  };
}

/**
 * Tool call made by the model
 */
export interface ToolCall {
  /** Unique identifier for the tool call */
  id: string;
  /** Type of tool call */
  type: 'function';
  /** Function call details */
  function: {
    /** Name of the function to call */
    name: string;
    /** JSON string of arguments */
    arguments: string;
  };
}

/**
 * Streaming chunk from completion API
 */
export interface StreamChunk {
  /** Delta containing new content */
  delta?: {
    /** New content to append */
    content?: string;
    /** Tool calls being made */
    toolCalls?: ToolCall[];
  };
  /** Reason the stream finished (only in final chunk) */
  finishReason?: string;
}

/**
 * Configuration for retry behavior
 */
export interface RetryConfig {
  /** Maximum number of retry attempts */
  maxRetries: number;
  /** Base delay in milliseconds */
  baseDelay: number;
  /** Maximum delay in milliseconds */
  maxDelay: number;
  /** Multiplier for exponential backoff */
  backoffMultiplier: number;
}

/**
 * Error thrown by model clients
 */
export class ModelClientError extends Error {
  constructor(
    message: string,
    public readonly statusCode?: number,
    public readonly provider?: string,
    public readonly retryable: boolean = false
  ) {
    super(message);
    this.name = 'ModelClientError';
  }
}

/**
 * Abstract base class for model clients
 */
export abstract class ModelClient {
  protected retryConfig: RetryConfig;

  constructor(retryConfig?: Partial<RetryConfig>) {
    this.retryConfig = {
      maxRetries: 3,
      baseDelay: 1000,
      maxDelay: 30000,
      backoffMultiplier: 2,
      ...retryConfig,
    };
  }

  /**
   * Complete a chat conversation
   * @param request The completion request
   * @returns Promise resolving to the completion response
   */
  abstract complete(request: CompletionRequest): Promise<CompletionResponse>;

  /**
   * Stream a chat conversation
   * @param request The completion request
   * @returns Async generator yielding stream chunks
   */
  abstract stream(request: CompletionRequest): AsyncGenerator<StreamChunk>;

  /**
   * Count tokens in a text string for a given model
   * @param text The text to count tokens for
   * @param model The model to use for counting
   * @returns Number of tokens
   */
  abstract countTokens(text: string, model: string): number;

  /**
   * Get the provider name for this client
   */
  abstract getProvider(): string;

  /**
   * Stream completion with events
   * @param request The completion request
   * @returns Async generator yielding stream events
   */
  abstract streamCompletion(request: CompletionRequest): AsyncGenerator<any>;

  /**
   * Get current model identifier
   */
  abstract getModel(): string;

  /**
   * Set the model to use
   */
  abstract setModel(model: string): void;

  /**
   * Get context window size for current model
   */
  abstract getContextWindow(): number | undefined;

  /**
   * Get reasoning effort configuration
   */
  abstract getReasoningEffort(): any;

  /**
   * Set reasoning effort configuration
   */
  abstract setReasoningEffort(effort: any): void;

  /**
   * Get reasoning summary configuration
   */
  abstract getReasoningSummary(): any;

  /**
   * Set reasoning summary configuration
   */
  abstract setReasoningSummary(summary: any): void;

  /**
   * Validate a request before sending
   * @param request The request to validate
   * @throws ModelClientError if validation fails
   */
  protected validateRequest(request: CompletionRequest): void {
    if (!request.model?.trim()) {
      throw new ModelClientError('Model is required');
    }

    if (!request.messages || request.messages.length === 0) {
      throw new ModelClientError('At least one message is required');
    }

    // Validate temperature
    if (request.temperature !== undefined && (request.temperature < 0 || request.temperature > 2)) {
      throw new ModelClientError('Temperature must be between 0 and 2');
    }

    // Validate maxTokens
    if (request.maxTokens !== undefined && request.maxTokens <= 0) {
      throw new ModelClientError('maxTokens must be positive');
    }

    // Validate messages
    for (const message of request.messages) {
      if (!['system', 'user', 'assistant', 'tool'].includes(message.role)) {
        throw new ModelClientError(`Invalid message role: ${message.role}`);
      }

      if (message.role === 'tool' && !message.toolCallId) {
        throw new ModelClientError('Tool messages must have a toolCallId');
      }
    }
  }

  /**
   * Execute a function with retry logic
   * @param fn The function to execute
   * @param retryableErrors Function to determine if an error is retryable
   * @returns Promise resolving to the function result
   */
  protected async withRetry<T>(
    fn: () => Promise<T>,
    retryableErrors: (error: any) => boolean = () => false
  ): Promise<T> {
    let lastError: any;

    for (let attempt = 0; attempt <= this.retryConfig.maxRetries; attempt++) {
      try {
        return await fn();
      } catch (error) {
        lastError = error;

        // Don't retry on the last attempt
        if (attempt === this.retryConfig.maxRetries) {
          break;
        }

        // Only retry if the error is retryable
        if (!retryableErrors(error)) {
          break;
        }

        // Calculate delay with exponential backoff and jitter
        const delay = Math.min(
          this.retryConfig.baseDelay * Math.pow(this.retryConfig.backoffMultiplier, attempt),
          this.retryConfig.maxDelay
        );

        // Add jitter to prevent thundering herd
        const jitteredDelay = delay + Math.random() * 1000;

        await new Promise(resolve => setTimeout(resolve, jitteredDelay));
      }
    }

    throw lastError;
  }

  /**
   * Check if an HTTP error is retryable
   * @param statusCode The HTTP status code
   * @returns True if the error is retryable
   */
  protected isRetryableHttpError(statusCode: number): boolean {
    return statusCode >= 500 || statusCode === 429 || statusCode === 408;
  }
}