/**
 * Minimal Claude Provider Implementation
 * 
 * This is a streamlined implementation of the Claude provider, focused on 
 * correctly handling shell commands and tool calls. It is built using testable
 * primitive functions for reliability.
 */

import { BaseProvider } from "./base-provider.js";
import {
  CompletionParams,
  ModelDefaults,
  NormalizedStreamEvent,
  ParsedToolCall,
  Tool
} from "./provider-interface.js";

import Anthropic from "@anthropic-ai/sdk";
import type {
  Message,
  MessageParam,
  ToolUseBlock,
  ToolResultBlock,
  TextBlock
} from "@anthropic-ai/sdk";

import type { AppConfig } from "../config.js";
import { ORIGIN, CLI_VERSION } from "../session.js";

// Import primitives for tool processing
import {
  normalizeShellCommand,
  processShellToolInput,
  claudeToolToOpenAIFunction,
  parseClaudeToolCall,
  convertClaudeMessageToOpenAI,
  createShellCommandInstructions,
  createDefaultClaudeTools
} from "./claude-tools.js";

/**
 * Minimal Claude Provider
 */
export class ClaudeProvider extends BaseProvider {
  id = "claude";
  name = "Claude";
  
  // No longer using provider-level history management
  
  /**
   * Get available models from Claude/Anthropic
   * @returns Promise resolving to an array of model identifiers
   */
  async getModels(): Promise<string[]> {
    return [
      "claude-3-5-sonnet-20240620",  // Latest Claude 3.5 Sonnet model
      "claude-3-opus-20240229",      // High-performance Claude 3 model
      "claude-3-haiku-20240307"      // Faster, lighter model
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
    
    // Create Anthropic client
    const anthropicClient = new Anthropic({
      apiKey,
      baseURL: providerConfig.baseUrl,
      maxRetries: 3,
      timeout: providerConfig.timeoutMs || 180000,
    });
    
    // Create a wrapper that implements the expected interface for agent-loop.ts
    const clientWrapper = {
      // Pass through the original Anthropic client
      ...anthropicClient,
      
      // Add the responses property expected by agent-loop.ts
      responses: {
        create: async (params: any) => {
          console.log("Claude provider: translating request to Claude format");
          
          // Process the new input items and update conversation history
          if (params.input && params.input.length > 0) {
            console.log(`Claude provider: Processing request with ${params.input.length} input items`);
          }
          
          // Use the input directly - application now sends complete history
          console.log(`Claude provider: Converting messages to Claude format`);
          
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
            
            // Add detailed example for shell commands to system prompt
            if (claudeTools.some(t => t.name === "shell")) {
              console.log("Claude provider: Adding detailed shell command examples to system prompt");
              
              // Enhance the system prompt with detailed instructions for shell commands
              if (!claudeParams.system) {
                claudeParams.system = "";
              }
              
              // Add specific instructions for using the shell tool
              claudeParams.system += createShellCommandInstructions();
            }
            
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
            throw error;
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
      console.log("Claude provider: Creating streaming response");
      const claudeStream = await client.messages.stream(params);
      
      // Build up the complete text as we go for the final event
      let completeText = "";
      
      // Keep track of tool uses (function calls)
      const toolCalls: any[] = [];
      let currentToolCall: any = null;
      
      // Store a reference to the provider instance
      const self = this;
      
      // Create an iterable that adapts Claude's streaming format to OpenAI's
      const adaptedStream = {
        [Symbol.asyncIterator]: async function* () {
          try {
            // Process Claude streaming events
            for await (const event of claudeStream) {
              console.log(`Claude provider: STREAM EVENT: ${event.type}`);
              if (event.content_block) {
                console.log(`Claude provider: Content block type: ${event.content_block.type}`);
              }
              
              // Handle content block start - check for tool use
              if (event.type === "content_block_start") {
                // Tool use detection
                if (event.content_block?.type === "tool_use") {
                  console.log(`Claude provider: Tool use detected:`, event.content_block);
                  
                  currentToolCall = {
                    id: event.content_block.id,
                    name: event.content_block.name,
                    input: event.content_block.input || {}
                  };
                  
                  console.log(`Claude provider: Current tool call:`, currentToolCall);
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
                console.log(`Claude provider: Content block stopped`);
                
                if (currentToolCall) {
                  console.log(`Claude provider: Finalizing tool call:`, currentToolCall);
                  toolCalls.push(currentToolCall);
                  
                  // Convert Claude tool call to OpenAI function call format
                  const functionCall = claudeToolToOpenAIFunction(currentToolCall);
                  
                  if (functionCall) {
                    console.log(`Claude provider: Emitting function call:`, functionCall);
                    
                    // Emit function call in OpenAI format
                    yield {
                      type: "response.output_item.done",
                      item: functionCall
                    };
                  }
                  
                  currentToolCall = null;
                }
              }
              // When message is complete, emit final events
              else if (event.type === "message_stop") {
                console.log("Claude provider: Received message_stop event");
                
                // Create the assistant message with the complete text (trim trailing whitespace)
                const assistantMessage = {
                  type: "message",
                  role: "assistant",
                  content: [{ type: "output_text", text: completeText.trimEnd() }]
                };
                
                // Add to conversation history
                // Assistant message is now managed at the application level
                
                // Convert tool calls to function calls for final output
                const functionCalls = toolCalls.map(tool => claudeToolToOpenAIFunction(tool)).filter(Boolean);
                
                console.log(`Claude provider: Final output with ${functionCalls.length} function calls`);
                
                // For streaming responses, emit completion event
                yield {
                  type: "response.completed",
                  response: {
                    id: `claude_${Date.now()}`,
                    status: "completed",
                    output: [
                      // Include the complete text as a single message
                      {
                        type: "message",
                        role: "assistant",
                        content: [{ type: "output_text", text: completeText }]
                      },
                      // Include any function calls
                      ...functionCalls
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
   * Create an OpenAI-compatible response from Claude's response
   * @param claudeResponse Claude response
   * @returns OpenAI-compatible response
   */
  private createOpenAICompatibleResponse(claudeResponse: Message): any {
    console.log(`Claude provider: Creating OpenAI-compatible response from Claude response`);
    
    // Convert Claude response to OpenAI format
    const output = claudeResponse.content.map(block => {
      console.log(`Claude provider: Processing content block type: ${block.type}`);
      
      if (block.type === "text") {
        const textBlock = block as TextBlock;
        return {
          type: "message",
          role: "assistant",
          content: [{ type: "output_text", text: textBlock.text }]
        };
      }
      else if (block.type === "tool_use") {
        // Tool use block (function call in OpenAI terms)
        const toolUse = block as ToolUseBlock;
        console.log(`Claude provider: Tool use block detected, name: ${toolUse.name}`);
        
        // Convert to function call format
        const functionCall = claudeToolToOpenAIFunction(toolUse);
        console.log(`Claude provider: Converted to function call:`, functionCall);
        
        return functionCall;
      }
      
      return null;
    }).filter(Boolean);
    
    // Messages are now managed at the application level
    
    // Map Claude response to OpenAI format
    const response = {
      id: claudeResponse.id,
      model: claudeResponse.model,
      created: Date.now(),
      object: "response",
      output
    };
    
    return response;
  }
  
  /**
   * Execute a completion request
   * @param params Completion parameters
   * @returns Promise resolving to a stream of completion events
   */
  async runCompletion(params: CompletionParams): Promise<any> {
    const client = this.createClient(params.config);
    
    // Extract system message
    const systemPrompt = this.extractSystemMessage(params.messages);
    
    // Create Anthropic-specific request parameters
    const requestParams: any = {
      model: params.model,
      messages: this.convertMessagesToClaudeFormat(params.messages),
      system: systemPrompt,
      temperature: params.temperature || 0.7,
      max_tokens: params.maxTokens || 4096,
      stream: params.stream !== false, // Default to true
    };
    
    // Add tools if available
    if (params.tools && params.tools.length > 0) {
      const claudeTools = this.formatTools(params.tools);
      
      // Add detailed shell command instructions if needed
      if (claudeTools.some(t => t.name === "shell")) {
        if (!requestParams.system) {
          requestParams.system = "";
        }
        requestParams.system += createShellCommandInstructions();
      }
      
      requestParams.tools = claudeTools;
    }
    
    try {
      // Stream the response
      if (params.stream) {
        const stream = await client.messages.stream(requestParams);
        return this.createStreamingResponse(stream, requestParams);
      } else {
        const response = await client.messages.create(requestParams);
        return this.createOpenAICompatibleResponse(response);
      }
    } catch (error) {
      // Handle Claude-specific errors
      console.error("Claude API error:", error);
      throw error;
    }
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
      case "claude-3-5-sonnet-20240620":
        return {
          ...baseDefaults,
          contextWindowSize: 200000, // Claude 3.5 has larger context window
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
    return parseClaudeToolCall(toolCall);
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
      
      // For shell tool, enhance the description to help Claude understand how to use it
      let description = tool.description || "";
      if (tool.name === "shell") {
        description = `${description}\n\nTo use this tool, provide a command array with the commands to run.\n\nEXAMPLES:\n\n1. To run a simple command:\n   { "command": ["ls", "-la"] }\n\n2. To use shell operators (pipes, redirects):\n   { "command": ["bash", "-c", "echo hello | grep hello"] }\n\n3. For calculator operations:\n   { "command": ["bash", "-c", "echo '2+2' | bc"] }\n\nIMPORTANT: The 'command' value must be an ARRAY of strings, with each argument as a separate element.`;
      }
      
      return {
        name: tool.name,
        description,
        input_schema: {
          type: "object",
          properties: properties,
          required: requiredProps,
        }
      };
    });
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
    
    console.log(`Claude provider: Converting ${nonSystemMessages.length} messages to Claude format`);
    
    // Convert to Claude format
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
            // Trim trailing whitespace for assistant messages (Claude requirement)
            const text = role === "assistant" ? item.text.trimEnd() : item.text;
            return { type: "text", text };
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
        // Trim trailing whitespace for assistant messages (Claude requirement)
        const text = role === "assistant" ? message.content.trimEnd() : message.content;
        content = [{ type: "text", text }];
      }
      // For other formats, try to convert to string
      else {
        content = [{ type: "text", text: JSON.stringify(message.content) }];
      }
      
      return { role, content };
    });
  }
  
  /**
   * Add a message to the conversation history
   * @param message Message to add
   */
  // History management is now handled at the application level in terminal-chat.tsx
  
  /**
   * Get the full conversation history
   * @returns Full conversation history
   */
  // History is now managed at the application level
}
