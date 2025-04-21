/**
 * Claude provider implementation for Codex CLI
 * 
 * This provider implements the LLMProvider interface for the Claude/Anthropic AI models.
 * 
 * Features:
 * - Supports both plain chat and tool-calling modes via a config option
 * - Tools are disabled by default (pure chat mode) for stability
 * - Enable tools via config.providers.claude.enableToolCalls = true or DISABLE_CLAUDE_TOOLS=false
 * - When tools are disabled, bypasses all the complex prompt engineering and JSON parsing
 * - Simple to maintain until Anthropic releases native function/tool calling API
 */

import { BaseProvider } from "./base-provider.js";
import type {
  CompletionParams,
  ModelDefaults,
  NormalizedStreamEvent,
  ParsedToolCall,
  Tool,
  ToolOutput
} from "./provider-interface.js";

import type { AppConfig } from "../config.js";
import type { ProviderConfig } from "../provider-config.js";

// Import Anthropic's official SDK
import Anthropic from "@anthropic-ai/sdk";

// Import Anthropic types
import type {
  MessageCreateParams,
  MessageParam,
  Message,
  TextBlock,
  ToolUseBlock,
  ContentBlockDeltaEvent,
  ContentBlockStartEvent,
  ContentBlockStopEvent,
  MessageStopEvent
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
      
      // Sort models for consistent display
      return response.data.map(model => model.id).sort();
    } catch {
      // Fallback to recommended models if API call fails
      return this.getRecommendedModels();
    }
  }

  /**
   * Get recommended models for Claude
   * @returns Array of recommended model identifiers
   */
  private getRecommendedModels(): string[] {
    return [
      "claude-3-5-sonnet-20240620",   // Latest Claude 3.5 Sonnet model - most capable
      "claude-3-opus-20240229",       // High-performance Claude 3 model
      "claude-3-haiku-20240307"       // Faster, lighter model
    ];
  }

  /**
   * Create an Anthropic/Claude client
   * @param config Application configuration
   * @returns Anthropic client instance with wrapper for OpenAI compatibility
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
    const timeout = providerConfig.timeoutMs || 180000; // 3 minute default
    
    // Create the Anthropic client instance
    const anthropicClient = new Anthropic({
      apiKey,
      baseURL,
      maxRetries: 3,
      timeout,
    });
    
    // Create a wrapper that adds the 'responses' property expected by agent-loop.ts
    const clientWrapper = {
      // Pass through the original Anthropic client
      ...anthropicClient,
      
      // Add the responses.create method expected by agent-loop.ts
      responses: {
        create: async (params: any) => {
          // Convert request to Claude format
          const claudeParams = this.convertRequestToClaudeFormat(params);
          
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
   * Convert request parameters from OpenAI format to Claude format
   * @param params OpenAI-style request parameters
   * @returns Claude-compatible request parameters
   */
  private convertRequestToClaudeFormat(params: any): MessageCreateParams {
    console.log("Claude provider: Converting request to Claude format");
    
    // Extract system message if present
    const systemPrompt = this.extractSystemMessage(params.input);
    
    // Convert messages to Claude format (excluding system messages)
    const messages = this.convertMessagesToClaudeFormat(params.input || []);
    
    // Create Claude request parameters
    const claudeParams: MessageCreateParams = {
      model: params.model,
      messages,
      system: systemPrompt || params.instructions,
      max_tokens: params.max_tokens || 4096,
      stream: params.stream === true,
    };
    
    // Add temperature if specified
    if (params.temperature !== undefined) {
      claudeParams.temperature = params.temperature;
    }
    
    // Check if tool calls are enabled via config
    const enableTools = Boolean(params.config.providers?.claude?.enableToolCalls);
    console.log(`Claude provider: Tool calls ${enableTools ? 'enabled' : 'disabled'}`);
    
    // Add tools if available and enabled
    if (enableTools && params.tools && params.tools.length > 0) {
      console.log("Claude provider: adding tools to request");
      const claudeTools = this.formatTools(params.tools);
      
      // If shell tool is included, enhance system prompt with detailed instructions
      if (claudeTools.some(t => t.name === "shell")) {
        console.log("Claude provider: Adding shell command instructions to system prompt");
        
        // Ensure system prompt exists
        if (!claudeParams.system) {
          claudeParams.system = "";
        }
        
        // Add shell command instructions
        claudeParams.system += this.createShellCommandInstructions();
      }
      
      // @ts-ignore - Type safety for tools
      claudeParams.tools = claudeTools;
    } else if (!enableTools) {
      // Strip out any tool bits when tools are disabled
      delete claudeParams.tools;
      // No shell command instructions added to the prompt
      console.log("Claude provider: Tools disabled, using plain chat interface");
    }
    
    return claudeParams;
  }

  /**
   * Extract system message from input messages
   * @param messages Array of input messages
   * @returns System message content or undefined
   */
  private extractSystemMessage(messages: any[]): string | undefined {
    if (!messages || !Array.isArray(messages)) {
      return undefined;
    }
    
    for (const message of messages) {
      if (message.role === "system" && typeof message.content === "string") {
        return message.content;
      }
    }
    
    return undefined;
  }

  /**
   * Convert messages to Claude format
   * @param messages OpenAI-style messages
   * @returns Messages in Claude format
   */
  private convertMessagesToClaudeFormat(messages: any[]): MessageParam[] {
    if (!messages || !Array.isArray(messages)) {
      return [];
    }
    
    // Filter out system messages (handled separately in Claude)
    const nonSystemMessages = messages.filter(msg => msg.role !== "system");
    
    console.log(`Claude provider: Converting ${nonSystemMessages.length} messages to Claude format`);
    
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
            // Trim trailing whitespace for assistant messages (Claude requirement)
            const text = role === "assistant" ? item.text.trimEnd() : item.text;
            return { type: "text", text };
          }
          
          // Handle image content
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
      // If content is a string (simple format)
      else if (typeof message.content === "string") {
        // Trim trailing whitespace for assistant messages (Claude requirement)
        const text = role === "assistant" ? message.content.trimEnd() : message.content;
        content = [{ type: "text", text }];
      }
      // Handle tool call results and other message formats
      else if (message.type === "function_call_output") {
        // Format function call outputs as simple text
        const functionName = message.name || "function";
        const outputText = `Tool output from ${functionName}: ${message.output || "No output"}`;
        content = [{ type: "text", text: outputText }];
      }
      // For other formats, try to convert to string
      else {
        content = [{ type: "text", text: JSON.stringify(message.content) }];
      }
      
      // Verify content is properly formatted
      if (!content || content.length === 0) {
        // If somehow content is empty, add a placeholder
        content = [{ type: "text", text: "No content" }];
      }
      
      // Make sure each text item has a valid text property
      content = content.map(item => {
        if (item.type === "text" && !item.text) {
          return { ...item, text: "Empty message" };
        }
        return item;
      });
      
      return { role, content };
    });
  }

  /**
   * Create a streaming response compatible with OpenAI's interface
   * @param client Anthropic client
   * @param params Claude API parameters
   * @returns Stream response compatible with OpenAI format
   */
  private async createStreamingResponse(client: Anthropic, params: MessageCreateParams): Promise<any> {
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
              console.log(`Claude provider: Stream event type: ${event.type}`);
              
              // Handle content block start - check for tool use
              if (event.type === "content_block_start") {
                const startEvent = event as ContentBlockStartEvent;
                
                console.log(`Claude provider: Content block start event, type: ${startEvent.content_block?.type}`);
                console.log(`Claude provider: Content block details:`, JSON.stringify(startEvent.content_block));
                
                // Tool use detection
                if (startEvent.content_block?.type === "tool_use") {
                  console.log(`Claude provider: Tool use detected:`, startEvent.content_block);
                  
                  // Create a default shell command if empty input detected
                  let toolInput = startEvent.content_block.input || {};
                  if (startEvent.content_block.name === "shell") {
                    // For shell tools, ensure there's a valid command
                    if (!toolInput || Object.keys(toolInput).length === 0) {
                      console.log(`Claude provider: Empty shell tool input detected, adding default command`);
                      toolInput = {
                        command: ["ls", "-la"],
                        workdir: process.cwd()
                      };
                    } else if (!toolInput.command) {
                      console.log(`Claude provider: Shell tool input missing command, adding default command`);
                      toolInput = {
                        ...toolInput,
                        command: ["ls", "-la"]
                      };
                    }
                  }
                  
                  currentToolCall = {
                    id: startEvent.content_block.id,
                    name: startEvent.content_block.name,
                    input: toolInput
                  };
                  
                  // Debug the input structure
                  console.log(`Claude provider: Tool input type: ${typeof toolInput}`);
                  console.log(`Claude provider: Tool input:`, JSON.stringify(toolInput));
                }
              }
              // Process content deltas (text content)
              else if (event.type === "content_block_delta" && event.delta?.text) {
                const deltaEvent = event as ContentBlockDeltaEvent;
                completeText += deltaEvent.delta.text;
                
                // Emit in OpenAI format
                yield {
                  type: "response.output_item.delta",
                  delta: { 
                    content: [{ type: "output_text", text: deltaEvent.delta.text }] 
                  },
                  item: {
                    type: "message",
                    role: "assistant",
                    content: [{ type: "output_text", text: deltaEvent.delta.text }]
                  }
                };
              }
              // Process content block stop - finalize tool call if needed
              else if (event.type === "content_block_stop") {
                const stopEvent = event as ContentBlockStopEvent;
                console.log(`Claude provider: Content block stop event - has content_block: ${!!stopEvent.content_block}`);
                if (stopEvent.content_block) {
                  console.log(`Claude provider: Content block type: ${stopEvent.content_block.type}`);
                }
                
                // Handle tool call completion
                if (currentToolCall) {
                  console.log(`Claude provider: Finalizing tool call with current tool:`, currentToolCall);
                  toolCalls.push(currentToolCall);
                  
                  // Process the tool call and ensure proper format
                  const parsedToolCall = self.parseToolCall(currentToolCall);
                  
                  // Convert to OpenAI function call format
                  const functionCall = {
                    type: "function_call",
                    id: parsedToolCall.id,
                    name: parsedToolCall.name,
                    arguments: JSON.stringify(parsedToolCall.arguments)
                  };
                  
                  console.log(`Claude provider: Emitting function call:`, functionCall);
                  
                  // Emit function call in OpenAI format
                  yield {
                    type: "response.output_item.done",
                    item: functionCall
                  };
                  
                  currentToolCall = null;
                }
              }
              // When message is complete, emit final events
              else if (event.type === "message_stop") {
                console.log("Claude provider: Received message_stop event");
                
                // Convert tool calls to function calls for the final output
                const functionCalls = toolCalls.map(tool => {
                  const parsedTool = self.parseToolCall(tool);
                  return {
                    type: "function_call",
                    id: parsedTool.id,
                    name: parsedTool.name,
                    arguments: JSON.stringify(parsedTool.arguments)
                  };
                });
                
                console.log(`Claude provider: Final output with ${functionCalls.length} function calls`);
                
                // Emit the final completion event
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
                        content: [{ type: "output_text", text: completeText.trimEnd() }]
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
    console.log(`Claude provider: Claude response content blocks: ${claudeResponse.content.length}`);
    
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
        console.log(`Claude provider: Tool use block detected, name: ${toolUse.name}, id: ${toolUse.id}`);
        
        // Process the tool call and ensure proper format
        const parsedToolCall = this.parseToolCall(toolUse);
        
        // Convert to OpenAI function call format
        return {
          type: "function_call",
          id: parsedToolCall.id,
          name: parsedToolCall.name,
          arguments: JSON.stringify(parsedToolCall.arguments)
        };
      }
      
      console.log(`Claude provider: Unhandled block type: ${block.type}`);
      return null;
    }).filter(Boolean);
    
    console.log(`Claude provider: Created ${output.length} output items`);
    
    // Map Claude response to OpenAI format
    return {
      id: claudeResponse.id,
      model: claudeResponse.model,
      created: Date.now(),
      object: "response",
      output
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
   * Execute a completion request
   * @param params Completion parameters
   * @returns Promise resolving to a stream of completion events
   */
  async runCompletion(params: CompletionParams): Promise<any> {
    const client = this.createClient(params.config);
    
    console.log(`Claude provider: Running completion with model "${params.model}"`);
    
    // Convert generic messages to Claude format
    const claudeMessages = this.convertMessagesToClaudeFormat(params.messages);
    
    // Extract system message
    const systemPrompt = this.extractSystemMessage(params.messages);
    
    // Create Anthropic-specific request parameters
    const requestParams: MessageCreateParams = {
      model: params.model,
      messages: claudeMessages,
      system: systemPrompt,
      temperature: params.temperature || 0.7,
      max_tokens: params.maxTokens || 4096,
      stream: params.stream !== false, // Default to true
    };
    
    // Check if tool calls are enabled via config
    const enableTools = Boolean(params.config.providers?.claude?.enableToolCalls);
    console.log(`Claude provider: Tool calls ${enableTools ? 'enabled' : 'disabled'}`);
    
    // Add tools if available and enabled
    if (enableTools && params.tools && params.tools.length > 0) {
      // Convert tools to Claude format
      const claudeTools = this.formatTools(params.tools);
      
      // If shell tool is included, enhance system prompt with detailed instructions
      if (claudeTools.some(t => t.name === "shell")) {
        console.log("Claude provider: Adding shell command instructions to system prompt");
        
        // Ensure system prompt exists
        if (!requestParams.system) {
          requestParams.system = "";
        }
        
        // Add shell command instructions
        requestParams.system += this.createShellCommandInstructions();
      }
      
      // @ts-ignore - Type safety for tools
      requestParams.tools = claudeTools;
    } else if (!enableTools) {
      // Strip out any tool bits when tools are disabled
      delete requestParams.tools;
      // No shell command instructions added to the prompt
      console.log("Claude provider: Tools disabled, using plain chat interface");
    }
    
    try {
      // When tools are disabled and not streaming, use a simpler flow
      if (!enableTools && !params.stream) {
        console.log("Claude provider: Using simple non-streaming response for tools-disabled mode");
        const response = await client.messages.create(requestParams);
        return this.createOpenAICompatibleResponse(response);
      }
      // For streaming or when tools are enabled, use the full streaming adapter
      else if (params.stream) {
        // Create an adapter that makes Claude's streaming API compatible with OpenAI's format
        return await this.createStreamingResponse(client, requestParams);
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
    // Log the raw tool call
    console.log(`Claude provider: Parsing tool call: ${JSON.stringify(toolCall)}`);
    
    // Extract basic information from Claude's format
    const toolId = toolCall.id || `tool_${Date.now()}`;
    const toolName = toolCall.name || "unknown";
    
    // Get the raw input from Claude's format
    let toolArgs = toolCall.input || {};
    
    // Special handling for shell commands to ensure they work correctly
    if (toolName === "shell") {
      console.log(`Claude provider: Processing shell command`);
      
      // Process shell command to ensure it's in the correct format
      toolArgs = this.processShellToolInput(toolArgs);
    }
    
    console.log(`Claude provider: Parsed tool call: ${toolName}, args: ${JSON.stringify(toolArgs)}`);
    
    return {
      id: toolId,
      name: toolName,
      arguments: toolArgs,
    };
  }

  /**
   * Process shell tool input to ensure proper format
   * @param toolInput The shell tool input
   * @returns Properly formatted tool arguments
   */
  private processShellToolInput(toolInput: any): { command: string[], workdir?: string } {
    // Handle completely empty or missing input
    if (!toolInput || typeof toolInput !== 'object') {
      console.log(`Claude provider: Empty or invalid tool input, using default command`);
      return {
        command: ["ls", "-la"],
        workdir: process.cwd()
      };
    }
    
    // Extract command from input
    let command: string[] = this.normalizeShellCommand(toolInput.command);
    
    // Ensure workdir is present
    const workdir = toolInput.workdir || process.cwd();
    
    return {
      command,
      workdir
    };
  }

  /**
   * Normalize a shell command to the expected format
   * @param command The command to normalize (can be string, array, or undefined)
   * @returns A properly formatted command array
   */
  private normalizeShellCommand(command: any): string[] {
    // Handle empty or undefined command
    if (!command) {
      console.log(`Claude provider: Empty command detected, using default ls command`);
      return ["ls", "-la"];
    }
    
    // If command is a string
    if (typeof command === 'string') {
      // Handle empty string
      if (command.trim() === '') {
        console.log(`Claude provider: Empty string command, using default ls command`);
        return ["ls", "-la"];
      }
      
      // Check if the command string is actually a JSON string of an array
      if (command.startsWith('[') && command.endsWith(']')) {
        try {
          const parsedCommand = JSON.parse(command);
          if (Array.isArray(parsedCommand)) {
            console.log(`Claude provider: Detected JSON string containing an array, parsing it: ${command}`);
            
            // Now check if the parsed array needs bash -c wrapping
            if (!(parsedCommand[0] === "bash" && parsedCommand[1] === "-c")) {
              const cmdStr = parsedCommand.join(' ');
              console.log(`Claude provider: Wrapping parsed array in bash -c: ${cmdStr}`);
              return ["bash", "-c", cmdStr];
            }
            
            return parsedCommand;
          }
        } catch (parseError) {
          // Not valid JSON, treat as regular string
          console.log(`Claude provider: Failed to parse command as JSON, using bash -c: ${command}`);
        }
      }
      
      // For all other strings, wrap in bash -c
      console.log(`Claude provider: Converting command string to bash -c: ${command}`);
      return ["bash", "-c", command];
    }
    
    // If command is an array
    if (Array.isArray(command)) {
      // Handle empty array
      if (command.length === 0) {
        console.log(`Claude provider: Empty command array, using default ls command`);
        return ["ls", "-la"];
      }
      
      // If not already in bash -c format and contains shell special characters
      // or seems to need shell features, wrap it
      if (!(command[0] === "bash" && command[1] === "-c")) {
        const cmdStr = command.join(' ');
        
        // Check if command contains shell special characters
        const needsBashC = cmdStr.includes('|') || 
                         cmdStr.includes('>') || 
                         cmdStr.includes('<') || 
                         cmdStr.includes('*') || 
                         cmdStr.includes('?') || 
                         cmdStr.includes('$') ||
                         cmdStr.includes('&&') ||
                         cmdStr.includes('||');
        
        if (needsBashC) {
          console.log(`Claude provider: Converting command array to bash -c: ${cmdStr}`);
          return ["bash", "-c", cmdStr];
        }
      }
      
      // Return the array as is
      return command;
    }
    
    // For any other type, return default command
    console.log(`Claude provider: Unknown command type (${typeof command}), using default ls command`);
    return ["ls", "-la"];
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
    
    // Check if tool calls are enabled via config
    const enableTools = Boolean(this.getEnabledToolsConfig());
    
    // Return empty tools array when tools are disabled
    if (!enableTools) {
      console.log("Claude provider: Tools disabled, returning empty tools array");
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
   * Get tool calls enablement status from config or environment variable
   * @returns True if tool calls are enabled, false otherwise
   */
  private getEnabledToolsConfig(): boolean {
    // First check environment variable
    if (process.env.DISABLE_CLAUDE_TOOLS !== undefined) {
      return process.env.DISABLE_CLAUDE_TOOLS !== "true";
    }
    
    // Fall back to configuration (if passed via client)
    const lastConfig = this.lastConfig;
    if (lastConfig?.providers?.claude?.enableToolCalls !== undefined) {
      return Boolean(lastConfig.providers.claude.enableToolCalls);
    }
    
    // Default to false - tools disabled by default for stability
    return false;
  }
  
  // Track the last config object that was passed to createClient
  private lastConfig?: AppConfig;
  
  /**
   * When creating a client, store the config for later reference
   */
  createClient(config: AppConfig): any {
    // Store config for later reference
    this.lastConfig = config;
    
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
    const timeout = providerConfig.timeoutMs || 180000; // 3 minute default
    
    // Create the Anthropic client instance
    const anthropicClient = new Anthropic({
      apiKey,
      baseURL,
      maxRetries: 3,
      timeout,
    });
    
    // Create a wrapper that adds the 'responses' property expected by agent-loop.ts
    const clientWrapper = {
      // Pass through the original Anthropic client
      ...anthropicClient,
      
      // Add the responses.create method expected by agent-loop.ts
      responses: {
        create: async (params: any) => {
          // Convert request to Claude format
          const claudeParams = this.convertRequestToClaudeFormat(params);
          
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
   * Create shell command instructions for the system prompt
   * @returns Detailed shell command instructions
   */
  private createShellCommandInstructions(): string {
    return `\n\nCRITICAL INSTRUCTIONS FOR SHELL COMMANDS:

You MUST use the shell tool directly in your responses. When the user asks you to run a command, DO NOT describe what you want to do - just USE the shell tool IMMEDIATELY without any preamble.

ALWAYS format shell commands as follows:

1. For ANY command with pipes, redirects, wildcards, or shell features:
   { "command": ["bash", "-c", "your command here with pipes or redirects"] }

2. For simple commands:
   { "command": ["command", "arg1", "arg2"] }

EXAMPLES OF CORRECT USAGE:
- Calculator: { "command": ["bash", "-c", "echo '1+1' | bc"] }
- File search: { "command": ["bash", "-c", "find . -name '*.js' | grep 'test'"] }
- Directory listing: { "command": ["ls", "-la"] }
- Echo: { "command": ["echo", "hello world"] }

ABSOLUTELY REQUIRED FORMAT:
- The command MUST ALWAYS be an ARRAY, never a string
- For complex commands, ALWAYS use ["bash", "-c", "command"] format
- Command is required - never send an empty command
- ALWAYS include a proper command array: ["command", "arg1"] or ["bash", "-c", "complex command"]
- NEVER generate text before using the tool - invoke the tool directly as your first action

EXAMPLE OF CORRECT TOOL INVOCATION SEQUENCE:
1. User asks: "What files are in the current directory?"
2. You IMMEDIATELY call the tool: { "command": ["ls", "-la"] }
3. Wait for the command output to be returned
4. Then explain the results to the user

DO NOT format commands like this:
- ❌ "Let me run the ls command to show you the files: ls -la"
- ❌ "I'll use the shell tool to list files"
- ❌ { "command": "ls -la" }  // Command as string is wrong!
- ❌ { }  // Empty input is wrong!

ONLY format commands like this:
- ✅ { "command": ["ls", "-la"] }
- ✅ { "command": ["bash", "-c", "find . -name '*.js' | wc -l"] }

If the user asks to run a command like "ls" or "bc", you MUST run it through the shell tool - do not explain first, just RUN THE COMMAND IMMEDIATELY.`;
  }

  /**
   * Normalize a stream event from Claude format to common format
   * @param event Claude stream event
   * @returns Normalized event
   */
  normalizeStreamEvent(event: any): NormalizedStreamEvent {
    // Convert Claude-specific events to common format
    if (event.type === "content_block_delta") {
      return {
        type: "text",
        content: event.delta.text,
        originalEvent: event,
      };
    } else if (event.type === "content_block_start" && event.content_block?.type === "tool_use") {
      return {
        type: "tool_call",
        content: event.content_block,
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