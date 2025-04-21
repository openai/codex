/**
 * Claude provider implementation for Codex CLI
 */

import { BaseProvider } from "./base-provider.js";
import {
  CompletionParams,
  ModelDefaults,
  NormalizedStreamEvent,
  ParsedToolCall,
  Tool
} from "./provider-interface.js";

import type { AppConfig } from "../config.js";
import type { ProviderConfig } from "../provider-config.js";

import { ORIGIN, CLI_VERSION } from "../session.js";

// Import Anthropic's SDK
// Note: A real implementation would need the actual Anthropic SDK
//       For this PR, we'll create a placeholder implementation

// Placeholder for Anthropic's SDK - in production code, this would be the real SDK import
class Anthropic {
  apiKey: string;
  baseURL?: string;
  timeout?: number;
  defaultHeaders?: Record<string, string>;
  
  // Add responses property to mirror OpenAI's API structure
  responses: {
    create: (params: any) => Promise<any>;
  };
  
  // Add messages property for Claude's native API
  messages: {
    create: (params: any) => Promise<any>;
    stream: (params: any) => any;
  };

  constructor(options: { 
    apiKey: string, 
    baseURL?: string, 
    timeout?: number,
    defaultHeaders?: Record<string, string>
  }) {
    this.apiKey = options.apiKey;
    this.baseURL = options.baseURL;
    this.timeout = options.timeout;
    this.defaultHeaders = options.defaultHeaders;
    
    // Initialize the messages API
    this.messages = {
      create: async (params: any) => {
        // This would be a real API call in production code
        return { id: "msg_123", content: [] };
      },
      stream: async (params: any) => {
        // Mock streaming interface
        return {
          [Symbol.asyncIterator]: async function* () {
            yield { type: "content_block_start", content_block: { type: "text" } };
            yield { type: "content_block_delta", delta: { text: "Response from Claude" } };
            yield { type: "content_block_stop" };
            yield { type: "message_stop" };
          }
        };
      }
    };
    
    // Create a responses property that maps to messages to be compatible with OpenAI
    this.responses = {
      create: (params: any) => {
        console.log("Claude provider: mapping responses.create to messages.create");
        
        // For streaming requests, we need to return an async iterable
        if (params.stream) {
          console.log("Claude provider: creating streaming response");
          
          // Return an object that implements the AsyncIterable interface
          return {
            [Symbol.asyncIterator]: async function* () {
              // Mock response events that match what OpenAI's client expects
              // These are specific to OpenAI's client and would need to be adapted 
              // from Anthropic's actual streaming response format in a real implementation
              
              // First yield response item with text
              // Print to console directly so we can see it works
              console.log("assistant: Hello! This is a response from the Claude provider mock implementation. In a real implementation, this would come from the Anthropic API.");
              
              yield { 
                type: "response.output_item.done", 
                item: {
                  type: "message",
                  role: "assistant",
                  content: [
                    {
                      type: "output_text",
                      text: "Hello! This is a response from the Claude provider mock implementation. In a real implementation, this would come from the Anthropic API."
                    }
                  ]
                }
              };
              
              // Then yield completion event
              yield { 
                type: "response.completed", 
                response: {
                  id: "resp_" + Date.now(),
                  status: "completed",
                  output: [
                    {
                      type: "message",
                      role: "assistant",
                      content: [
                        {
                          type: "output_text",
                          text: "Hello! This is a response from the Claude provider mock implementation. In a real implementation, this would come from the Anthropic API."
                        }
                      ]
                    }
                  ]
                }
              };
            },
            controller: {
              abort: () => console.log("Claude provider: aborting stream")
            }
          };
        }
        
        // For non-streaming requests
        return this.messages.create({
          model: params.model,
          messages: params.input || [],
          system: params.instructions,
          stream: false,
          tools: params.tools,
        });
      }
    };
  }

  // Mock API for demonstration purposes
  // In a real implementation, these would interact with Anthropic's API
  async listModels() {
    return {
      data: [
        { id: "claude-3-opus-20240229" },
        { id: "claude-3-sonnet-20240229" },
        { id: "claude-3-haiku-20240307" },
      ]
    };
  }
}

/**
 * Claude provider implementation
 */
export class ClaudeProvider extends BaseProvider {
  id = "claude";
  name = "Claude";
  
  /**
   * Get available models from Claude/Anthropic
   * @returns Promise resolving to an array of model identifiers
   */
  async getModels(): Promise<string[]> {
    // If the user has not configured an API key, return recommended models
    const apiKey = this.getApiKey();
    if (!apiKey) {
      return this.getRecommendedModels();
    }
    
    try {
      const client = this.createClient({
        providers: {
          claude: { apiKey }
        }
      } as AppConfig);
      
      const response = await client.listModels();
      
      return response.data.map(model => model.id).sort();
    } catch {
      return this.getRecommendedModels();
    }
  }
  
  /**
   * Get recommended models for Claude
   * @returns Array of recommended model identifiers
   */
  private getRecommendedModels(): string[] {
    return [
      "claude-3-opus-20240229",
      "claude-3-sonnet-20240229",
      "claude-3-haiku-20240307"
    ];
  }
  
  /**
   * Create an Anthropic/Claude client
   * @param config Application configuration
   * @returns Anthropic client instance
   */
  createClient(config: AppConfig): Anthropic {
    // Get provider config from AppConfig
    const providerConfig = config.providers?.claude || {};
    
    // Get API key from provider config or environment
    const apiKey = providerConfig.apiKey || 
                  process.env.CLAUDE_API_KEY || 
                  process.env.ANTHROPIC_API_KEY;
                  
    if (!apiKey) {
      throw new Error("Claude API key not found. Please set CLAUDE_API_KEY or ANTHROPIC_API_KEY environment variable or configure it in the Codex config.");
    }
    
    // Get base URL and timeout from provider config
    const baseURL = providerConfig.baseUrl || undefined;
    const timeout = providerConfig.timeoutMs;
    
    // Get session information
    const sessionId = config.sessionId;
    
    // Create Claude client
    return new Anthropic({
      apiKey,
      baseURL,
      timeout,
      defaultHeaders: {
        originator: ORIGIN,
        version: CLI_VERSION,
        session_id: sessionId || "",
      }
    });
  }
  
  /**
   * Execute a completion request
   * @param params Completion parameters
   * @returns Promise resolving to a stream of completion events
   */
  async runCompletion(params: CompletionParams): Promise<any> {
    const client = this.createClient(params.config);
    
    // Convert generic messages to Claude format
    const messages = this.convertMessagesToClaudeFormat(params.messages);
    
    // Convert tools to Claude format
    const tools = this.formatTools(params.tools || []);
    
    // Create request parameters
    const requestParams = {
      model: params.model,
      messages,
      system: this.extractSystemMessage(params.messages),
      temperature: params.temperature || 0.7,
      max_tokens: params.maxTokens,
      stream: params.stream !== false, // Default to true
      tools: tools.length > 0 ? tools : undefined,
    };
    
    // Stream the response
    if (params.stream) {
      return client.messages.stream(requestParams);
    } else {
      return client.messages.create(requestParams);
    }
  }
  
  /**
   * Extract system message content from messages array
   * @param messages Array of messages
   * @returns System message content or undefined
   */
  private extractSystemMessage(messages: any[]): string | undefined {
    for (const message of messages) {
      if (message.role === "system" && typeof message.content === "string") {
        return message.content;
      }
    }
    return undefined;
  }
  
  /**
   * Convert generic messages to Claude format
   * @param messages Generic message array
   * @returns Claude-formatted messages
   */
  private convertMessagesToClaudeFormat(messages: any[]): any[] {
    // Filter out system messages (handled separately in Claude)
    const nonSystemMessages = messages.filter(msg => msg.role !== "system");
    
    // Convert to Claude format
    return nonSystemMessages.map(message => {
      // Simple conversion - in a real implementation, this would be more
      // sophisticated to handle different content types and formats
      return {
        role: message.role === "assistant" ? "assistant" : "user",
        content: message.content
      };
    });
  }
  
  /**
   * Get model defaults for Claude
   * @param model Model identifier
   * @returns Model defaults
   */
  getModelDefaults(model: string): ModelDefaults {
    // Base defaults for all Claude models
    const baseDefaults: ModelDefaults = {
      timeoutMs: 60000, // 1 minute
      temperature: 0.7,
      supportsToolCalls: true,
      supportsStreaming: true,
      contextWindowSize: 100000, // Default
    };
    
    // Model-specific overrides
    switch (model) {
      case "claude-3-opus-20240229":
        return {
          ...baseDefaults,
          contextWindowSize: 200000,
        };
      case "claude-3-sonnet-20240229":
        return {
          ...baseDefaults,
          contextWindowSize: 180000,
        };
      case "claude-3-haiku-20240307":
        return {
          ...baseDefaults,
          contextWindowSize: 150000,
        };
      default:
        return baseDefaults;
    }
  }
  
  /**
   * Parse a tool call from Claude format to common format
   * @param toolCall Claude tool call
   * @returns Normalized tool call
   */
  parseToolCall(toolCall: any): ParsedToolCall {
    // This would be implemented based on Claude's tool call format
    // For now, returning a placeholder implementation
    return {
      id: toolCall.id || "unknown",
      name: toolCall.name || "unknown",
      arguments: toolCall.input || {},
    };
  }
  
  /**
   * Format tools into Claude format
   * @param tools Array of tools in common format
   * @returns Tools in Claude format
   */
  formatTools(tools: Tool[]): any[] {
    // Convert generic tools to Claude format
    // This is a simplified implementation
    return tools.map(tool => ({
      name: tool.name,
      description: tool.description || "",
      input_schema: {
        type: "object",
        properties: tool.parameters,
        required: [],
      }
    }));
  }
  
  /**
   * Normalize a stream event from Claude format to common format
   * @param event Claude stream event
   * @returns Normalized event
   */
  normalizeStreamEvent(event: any): NormalizedStreamEvent {
    // Convert Claude-specific events to common format
    // This would be implemented based on Claude's streaming format
    
    if (event.type === "content_block_delta") {
      return {
        type: "text",
        content: event.delta.text,
        originalEvent: event,
      };
    } else if (event.type === "message_stop") {
      return {
        type: "completion",
        content: event,
        originalEvent: event,
      };
    } else {
      return {
        type: "text",
        content: event,
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
    return (
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
      error?.type === "context_length_exceeded" ||
      /context length exceeded/i.test(error?.message || "") ||
      /token limit/i.test(error?.message || "")
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
      error?.type === "invalid_request"
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
      return `Claude rate limit exceeded. Please try again later.`;
    }
    
    if (this.isTimeoutError(error)) {
      return `Request to Claude timed out. Please try again.`;
    }
    
    if (this.isContextLengthError(error)) {
      return `The current request exceeds the maximum context length supported by the chosen Claude model. Please shorten the conversation or switch to a model with a larger context window.`;
    }
    
    if (this.isConnectionError(error)) {
      return `Network error while contacting Claude. Please check your connection and try again.`;
    }
    
    // Extract status and message for detailed error info
    const status = error?.status || "unknown";
    const message = error?.message || "unknown";
    
    // Format detailed error message
    return `Claude error: Status ${status}, Message: ${message}`;
  }
  
  /**
   * Get the suggested wait time for rate limit errors
   * @param error Rate limit error
   * @returns Recommended wait time in milliseconds
   */
  getRetryAfterMs(error: any): number {
    // Default retry time
    const DEFAULT_RETRY_MS = 5000;
    
    // Parse retry-after from headers
    const retryAfter = error?.headers?.["retry-after"];
    if (retryAfter && !isNaN(parseInt(retryAfter, 10))) {
      return parseInt(retryAfter, 10) * 1000;
    }
    
    return DEFAULT_RETRY_MS;
  }
  
  /**
   * Get the API key from config or environment
   * @param config Optional config object
   * @returns API key or undefined
   */
  private getApiKey(config?: AppConfig): string | undefined {
    if (config?.providers?.claude?.apiKey) {
      return config.providers.claude.apiKey;
    }
    return process.env["CLAUDE_API_KEY"] || process.env["ANTHROPIC_API_KEY"];
  }
}