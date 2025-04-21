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
import Anthropic from "@anthropic-ai/sdk";
import { LLMMock } from "./llm-mock.js";

// Import anthropic types
import type {
  Message,
  MessageParam,
  ToolUseBlock,
  ToolResultBlock,
  TextBlock
} from "@anthropic-ai/sdk";

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
  createClient(config: AppConfig): any {
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
    const timeout = providerConfig.timeoutMs || 180000; // 3 minute default for Claude
    
    // Get session information
    const sessionId = config.sessionId;
    
    // Create the real Anthropic client instance
    const anthropicClient = new Anthropic({
      apiKey,
      baseURL: baseURL,
      maxRetries: 3,
      timeout: timeout,
    });
    
    // Create a wrapper that implements the expected interface for agent-loop.ts
    // This adds the 'responses' property that matches what OpenAI client provides
    const clientWrapper = {
      // Pass through the original Anthropic client
      ...anthropicClient,
      
      // Add the responses property expected by agent-loop.ts
      responses: {
        create: async (params: any) => {
          console.log("Claude provider: translating request to Claude format");
          
          // Convert from agent-loop's OpenAI format to Claude format
          const claudeParams = {
            model: params.model,
            messages: this.convertMessagesToClaudeFormat(params.input || []),
            system: params.instructions,
            max_tokens: 4096,
            stream: params.stream === true,
          };
          
          // Add tools if available
          if (params.tools && params.tools.length > 0) {
            console.log("Claude provider: adding tools to request");
            const claudeTools = this.formatTools(params.tools);
            console.log(`Claude tools: ${JSON.stringify(claudeTools.map(t => t.name))}`);
            // @ts-ignore - Type safety for tools
            claudeParams.tools = claudeTools;
          }
          
          try {
            // Call Claude API
            if (params.stream) {
              // For streaming, return a suitable async iterator
              return await this.createStreamingResponse(anthropicClient, claudeParams);
            } else {
              // For non-streaming, get the response and convert to expected format
              const claudeResponse = await anthropicClient.messages.create(claudeParams);
              return this.createOpenAICompatibleResponse(claudeResponse);
            }
          } catch (error) {
            console.error("Claude API error:", error);
            throw this.formatClaudeError(error);
          }
        }
      }
    };
    
    return clientWrapper;
  }
  
  /**
   * Create a streaming response compatible with OpenAI's interface
   * @param client Anthropic client
   * @param params Claude API parameters
   * @returns Stream response compatible with OpenAI format
   */
  private async createStreamingResponse(client: Anthropic, params: any): Promise<any> {
    try {
      // Get the Claude streaming response
      const claudeStream = await client.messages.stream(params);
      
      // Build up the complete text as we go for the final event
      let completeText = "";
      
      // Keep track of tool uses (function calls)
      const toolCalls: any[] = [];
      let currentToolCall: any = null;
      
      // Create an iterable that adapts Claude's streaming format to OpenAI's
      const adaptedStream = {
        [Symbol.asyncIterator]: async function* () {
          try {
            // Process Claude streaming events
            for await (const event of claudeStream) {
              // Handle content block start - check for tool use
              if (event.type === "content_block_start") {
                // Tool use detection
                if (event.content_block?.type === "tool_use") {
                  currentToolCall = {
                    id: event.content_block.id,
                    name: event.content_block.name,
                    input: event.content_block.input || {}
                  };
                }
              }
              // Process content deltas (text content)
              else if (event.type === "content_block_delta" && event.delta?.text) {
                completeText += event.delta.text;
                
                // Emit in OpenAI format
                yield {
                  type: "response.output_item.delta",
                  delta: { 
                    content: [{ type: "output_text", text: event.delta.text }] 
                  },
                  item: {
                    type: "message",
                    role: "assistant",
                    content: [{ type: "output_text", text: event.delta.text }]
                  }
                };
              }
              // Process content block stop - finalize tool call if needed
              else if (event.type === "content_block_stop") {
                if (currentToolCall) {
                  toolCalls.push(currentToolCall);
                  
                  // Emit function call in OpenAI format
                  yield {
                    type: "response.output_item.done",
                    item: {
                      type: "function_call",
                      id: currentToolCall.id,
                      name: currentToolCall.name,
                      args: currentToolCall.input
                    }
                  };
                  
                  currentToolCall = null;
                }
              }
              // When message is complete, emit final events
              else if (event.type === "message_stop") {
                // Final message event
                yield {
                  type: "response.completed",
                  response: {
                    id: `claude_${Date.now()}`,
                    status: "completed",
                    output: [
                      // Include text output if any
                      ...(completeText ? [{
                        type: "message",
                        role: "assistant",
                        content: [{ 
                          type: "output_text", 
                          text: completeText 
                        }]
                      }] : []),
                      // Include function calls if any (though these would already have been emitted)
                      ...toolCalls.map(tool => ({
                        type: "function_call",
                        id: tool.id,
                        name: tool.name,
                        args: tool.input
                      }))
                    ].filter(Boolean)
                  }
                };
              }
            }
          } catch (err) {
            console.error("Error in Claude stream adapter:", err);
            throw err;
          }
        },
        // Add controller for OpenAI compatibility
        controller: {
          abort: () => claudeStream.controller?.abort?.()
        }
      };
      
      return adaptedStream;
    } catch (error) {
      console.error("Error creating Claude streaming response:", error);
      throw error;
    }
  }
  
  /**
   * Execute a completion request
   * @param params Completion parameters
   * @returns Promise resolving to a stream of completion events
   */
  async runCompletion(params: CompletionParams): Promise<any> {
    const client = this.createClient(params.config);
    
    console.log(`Claude provider: Running completion with model "${params.model}"`);
    
    // Convert generic messages to Claude format
    const claudeMessages = this.convertMessagesToClaudeFormat(params.messages);
    
    // Convert tools to Claude format
    const claudeTools = this.formatTools(params.tools || []);
    
    // Extract system message
    const systemPrompt = this.extractSystemMessage(params.messages);
    
    // Create Anthropic-specific request parameters
    const requestParams: any = {
      model: params.model,
      messages: claudeMessages,
      system: systemPrompt,
      temperature: params.temperature || 0.7,
      max_tokens: params.maxTokens || 4096,
      stream: params.stream !== false, // Default to true
    };
    
    // Add tools if available
    if (claudeTools.length > 0) {
      requestParams.tools = claudeTools;
    }
    
    try {
      // Stream the response
      if (params.stream) {
        const stream = await client.messages.stream(requestParams);
        
        // Create an adapter that makes Claude's streaming API compatible with OpenAI's format
        return this.createOpenAICompatibleStream(stream);
      } else {
        const response = await client.messages.create(requestParams);
        return this.createOpenAICompatibleResponse(response);
      }
    } catch (error) {
      // Handle Claude-specific errors
      console.error("Claude API error:", error);
      throw this.formatClaudeError(error);
    }
  }
  
  /**
   * Create an OpenAI-compatible stream from Claude's streaming API
   * @param claudeStream Claude stream
   * @returns OpenAI-compatible stream
   */
  private createOpenAICompatibleStream(claudeStream: any): any {
    // Create an async iterable that maps Claude's events to OpenAI's format
    return {
      [Symbol.asyncIterator]: async function* () {
        try {
          // Iterate through Claude's stream events
          for await (const event of claudeStream) {
            if (event.type === "content_block_start" && event.content_block?.type === "text") {
              // Content block start - nothing to emit for OpenAI compatibility
            } 
            else if (event.type === "content_block_delta" && event.delta?.text) {
              // Content block delta - emit as a text delta
              // This is similar to how OpenAI streams tokens
              yield {
                type: "response.output_item.delta",
                delta: { content: [{ type: "output_text", text: event.delta.text }] },
                item: { 
                  type: "message", 
                  role: "assistant",
                  content: [{ type: "output_text", text: event.delta.text }]
                }
              };
            }
            else if (event.type === "content_block_stop") {
              // Content block complete - emit the completed message
              yield {
                type: "response.output_item.done",
                item: {
                  type: "message",
                  role: "assistant",
                  content: [{ type: "output_text", text: "" }]
                }
              };
            }
            else if (event.type === "message_stop") {
              // Message complete - emit completion event
              yield {
                type: "response.completed",
                response: {
                  id: `claude_${Date.now()}`,
                  status: "completed",
                  output: [
                    {
                      type: "message",
                      role: "assistant",
                      content: [{ type: "output_text", text: "" }]
                    }
                  ]
                }
              };
            }
            // Handle tool calls if present
            else if (event.type === "content_block_start" && event.content_block?.type === "tool_use") {
              // Tool use - equivalent to function call in OpenAI
              const toolUse = event.content_block as ToolUseBlock;
              yield {
                type: "response.output_item.done",
                item: {
                  type: "function_call",
                  id: toolUse.id,
                  name: toolUse.name,
                  args: toolUse.input
                }
              };
            }
          }
        } catch (error) {
          console.error("Error in Claude stream adapter:", error);
          throw error;
        }
      },
      // Add controller for aborting the stream
      controller: {
        abort: () => {
          console.log("Aborting Claude stream");
          claudeStream.controller?.abort?.();
        }
      }
    };
  }
  
  /**
   * Create an OpenAI-compatible response from Claude's response
   * @param claudeResponse Claude response
   * @returns OpenAI-compatible response
   */
  private createOpenAICompatibleResponse(claudeResponse: Message): any {
    // Map Claude response to OpenAI format
    return {
      id: claudeResponse.id,
      model: claudeResponse.model,
      created: Date.now(),
      object: "response",
      output: claudeResponse.content.map(block => {
        if (block.type === "text") {
          return {
            type: "message",
            role: "assistant",
            content: [{ type: "output_text", text: (block as TextBlock).text }]
          };
        }
        else if (block.type === "tool_use") {
          // Tool use block (function call in OpenAI terms)
          const toolUse = block as ToolUseBlock;
          return {
            type: "function_call",
            id: toolUse.id,
            name: toolUse.name,
            args: toolUse.input
          };
        }
        return null;
      }).filter(Boolean)
    };
  }
  
  /**
   * Format Claude API errors to a common format
   * @param error The error from Claude API
   * @returns Formatted error
   */
  private formatClaudeError(error: any): Error {
    // Extract relevant error information
    const status = error.status || error.statusCode;
    const message = error.message || "Unknown Claude API error";
    const type = error.type || "unknown_error";
    
    // Create a formatted error with Claude-specific information
    const formattedError = new Error(`Claude API error (${status}): ${message} (${type})`);
    
    // Add original error properties
    (formattedError as any).originalError = error;
    (formattedError as any).statusCode = status;
    (formattedError as any).errorType = type;
    
    return formattedError;
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
  private convertMessagesToClaudeFormat(messages: any[]): MessageParam[] {
    // Filter out system messages (handled separately in Claude)
    const nonSystemMessages = messages.filter(msg => msg.role !== "system");
    
    // Convert to Claude format - Claude only accepts 'user' and 'assistant' roles
    return nonSystemMessages.map(message => {
      // Determine role - Claude only supports 'user' and 'assistant'
      const role = message.role === "assistant" ? "assistant" : "user";
      
      // Handle different content types
      let content;
      
      // If content is an array (e.g., OpenAI format with multiple content parts)
      if (Array.isArray(message.content)) {
        content = message.content.map(item => {
          // Handle text content
          if (item.type === "input_text" || item.type === "output_text") {
            return { type: "text", text: item.text };
          }
          
          // Handle image content (if supported)
          if (item.type === "input_image") {
            return {
              type: "image",
              source: {
                type: "base64",
                media_type: item.media_type || "image/jpeg",
                data: item.image_url?.replace(/^data:image\/[a-z]+;base64,/, "") || item.data
              }
            };
          }
          
          // Default to text for unknown types
          return { type: "text", text: JSON.stringify(item) };
        });
      } 
      // If content is a string (simpler format)
      else if (typeof message.content === "string") {
        content = [{ type: "text", text: message.content }];
      }
      // For other formats, try to convert to string
      else {
        content = [{ type: "text", text: JSON.stringify(message.content) }];
      }
      
      return { role, content };
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
    if (!tools || tools.length === 0) {
      return [];
    }
    
    // Convert generic tools to Claude format
    return tools.map(tool => {
      // Get required properties from parameters schema
      const requiredProps = tool.parameters?.required || [];
      
      // Convert parameters.properties to Claude's expected format
      const properties = tool.parameters?.properties || {};
      
      return {
        name: tool.name,
        description: tool.description || "",
        input_schema: {
          type: "object",
          properties: properties,
          required: requiredProps,
        }
      };
    });
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
    // Claude specific rate limit checks
    if (error?.status === 429) {
      return true;
    }
    
    // Check error type and status
    if (error?.error?.type === "rate_limit_error") {
      return true;
    }
    
    // Check error message
    const errorMsg = error?.message || error?.error?.message || "";
    return /rate limit/i.test(errorMsg) || /too many requests/i.test(errorMsg);
  }
  
  /**
   * Check if an error is a timeout error
   * @param error Error to check
   * @returns True if it's a timeout error
   */
  isTimeoutError(error: any): boolean {
    // Common timeout error codes
    const timeoutCodes = ["ETIMEDOUT", "ESOCKETTIMEDOUT", "ECONNABORTED"];
    if (timeoutCodes.includes(error?.code)) {
      return true;
    }
    
    // Check error message
    const errorMsg = error?.message || error?.error?.message || "";
    return /timeout/i.test(errorMsg) || /timed out/i.test(errorMsg);
  }
  
  /**
   * Check if an error is a connection error
   * @param error Error to check
   * @returns True if it's a connection error
   */
  isConnectionError(error: any): boolean {
    // Common network error codes
    const networkCodes = [
      "ECONNRESET", "ECONNREFUSED", "ENOTFOUND", "EPIPE", 
      "ENETUNREACH", "ENETRESET", "ECONNABORTED"
    ];
    
    // Check if error code is a network error
    if (networkCodes.includes(error?.code) || networkCodes.includes(error?.cause?.code)) {
      return true;
    }
    
    // Check for network-related terms in message
    const errorMsg = error?.message || error?.error?.message || "";
    return (
      /network/i.test(errorMsg) || 
      /socket/i.test(errorMsg) || 
      /connection/i.test(errorMsg) ||
      /dns/i.test(errorMsg)
    );
  }
  
  /**
   * Check if an error is a context length error
   * @param error Error to check
   * @returns True if it's a context length error
   */
  isContextLengthError(error: any): boolean {
    // Claude specific error type
    if (error?.error?.type === "context_length_exceeded") {
      return true;
    }
    
    // Check error status and message patterns specific to Claude
    if (error?.status === 400) {
      const errorMsg = error?.message || error?.error?.message || "";
      return (
        /context length exceeded/i.test(errorMsg) ||
        /token limit/i.test(errorMsg) ||
        /input is too long/i.test(errorMsg) ||
        /exceeds the max tokens/i.test(errorMsg)
      );
    }
    
    return false;
  }
  
  /**
   * Check if an error is an invalid request error
   * @param error Error to check
   * @returns True if it's an invalid request error
   */
  isInvalidRequestError(error: any): boolean {
    // 4xx errors except rate limit (429) are generally invalid requests
    if (typeof error?.status === "number" && 
        error.status >= 400 && 
        error.status < 500 && 
        error.status !== 429) {
      return true;
    }
    
    // Claude-specific error types
    const invalidTypes = ["invalid_request_error", "authentication_error", "permission_error"];
    if (invalidTypes.includes(error?.error?.type)) {
      return true;
    }
    
    return false;
  }
  
  /**
   * Format an error message for user display
   * @param error Error to format
   * @returns User-friendly error message
   */
  formatErrorMessage(error: any): string {
    // Handle known error types with Claude-specific messages
    if (this.isRateLimitError(error)) {
      return `Claude rate limit exceeded. Please try again in a few minutes.`;
    }
    
    if (this.isTimeoutError(error)) {
      return `Request to Claude timed out. Claude models may take longer to process complex requests. Please try again or consider using a faster model.`;
    }
    
    if (this.isContextLengthError(error)) {
      return `The current request exceeds the maximum context length for Claude. Please shorten your prompt or conversation history, or switch to a Claude model with a larger context window.`;
    }
    
    if (this.isConnectionError(error)) {
      return `Network error while contacting Claude API. Please check your internet connection and Anthropic service status.`;
    }
    
    if (error?.status === 401 || error?.error?.type === "authentication_error") {
      return `Claude API authentication failed. Please check your API key and ensure it has the correct permissions.`;
    }
    
    // Format generic errors with Claude-specific information
    const status = error?.status || error?.error?.status || "unknown";
    const type = error?.error?.type || "unknown_error";
    const message = error?.message || error?.error?.message || "Unknown Claude API error";
    
    return `Claude API error: [${status}] ${type} - ${message}`;
  }
  
  /**
   * Get the suggested wait time for rate limit errors
   * @param error Rate limit error
   * @returns Recommended wait time in milliseconds
   */
  getRetryAfterMs(error: any): number {
    // Default retry time for Claude
    const DEFAULT_RETRY_MS = 5000;
    
    // Try to parse retry-after header from Claude response
    const retryAfter = error?.headers?.["retry-after"] || error?.error?.headers?.["retry-after"];
    if (retryAfter && !isNaN(parseInt(retryAfter, 10))) {
      return parseInt(retryAfter, 10) * 1000;
    }
    
    // Check if there's a specific retry duration in the error message
    const errorMsg = error?.message || error?.error?.message || "";
    const retryMatch = /retry after (\d+) seconds/i.exec(errorMsg);
    if (retryMatch && retryMatch[1] && !isNaN(parseInt(retryMatch[1], 10))) {
      return parseInt(retryMatch[1], 10) * 1000;
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