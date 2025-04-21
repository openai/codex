/**
 * Claude provider implementation for Codex CLI
 * With robust command processing for improved reliability
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
 * Normalize a shell command to the expected format
 * This is a key function that handles various edge cases in command formats
 * 
 * @param command The command to normalize (can be string, array, or undefined)
 * @returns A properly formatted command array
 */
function normalizeShellCommand(command: any): string[] {
  // Handle empty or undefined command
  if (!command) {
    console.log(`Claude provider: Empty command detected, using default ls command`);
    return ["ls", "-ltr"];  // Changed to ls -ltr for testing
  }
  
  // If command is a string
  if (typeof command === 'string') {
    // Handle empty string
    if (command.trim() === '') {
      console.log(`Claude provider: Empty string command, using default ls command`);
      return ["ls", "-ltr"];  // Changed to ls -ltr for testing
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
      return ["ls", "-ltr"];  // Changed to ls -ltr for testing
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
  return ["ls", "-ltr"];  // Changed to ls -ltr for testing
}

/**
 * Process shell tool input to ensure proper format
 * This handles completely empty input objects which was causing issues
 * 
 * @param toolInput The input from a shell tool call
 * @returns Properly formatted tool arguments
 */
function processShellToolInput(toolInput: any): { command: string[], workdir?: string } {
  // Handle completely empty or missing input
  if (!toolInput || typeof toolInput !== 'object' || Object.keys(toolInput).length === 0) {
    console.log(`Claude provider: Empty or invalid tool input, using default command`);
    return {
      command: ["ls", "-ltr"],  // Changed to ls -ltr for testing
      workdir: process.cwd()
    };
  }
  
  // Extract command from input
  const command = normalizeShellCommand(toolInput.command);
  
  // Ensure workdir is present
  const workdir = toolInput.workdir || process.cwd();
  
  return {
    command,
    workdir
  };
}

/**
 * Process a tool call to ensure it's properly formatted
 * This is a unified function used in both streaming and non-streaming paths
 * 
 * @param toolUse The tool call to process
 * @param context Context string for logging (e.g., "MAIN STREAM", "STREAM")
 * @returns Properly processed tool arguments
 */
function processToolCall(toolUse: any, context: string = "PROCESSOR"): any {
  if (!toolUse) {
    console.log(`Claude provider: ${context} - Invalid tool call (null or undefined)`);
    return null;
  }
  
  // Get basic tool properties
  const toolId = toolUse.id || `tool_${Date.now()}`;
  const toolName = toolUse.name || "unknown";
  let toolArgs = toolUse.input || {};
  
  // Process input based on tool type
  if (toolName === "shell") {
    console.log(`Claude provider: ${context} - Processing shell command`);
    console.log(`Claude provider: ${context} - Original input:`, JSON.stringify(toolArgs));
    
    // Use our helper function to process shell inputs
    toolArgs = processShellToolInput(toolArgs);
    console.log(`Claude provider: ${context} - Processed to:`, JSON.stringify(toolArgs));
  }
  
  // Return the processed tool
  return {
    id: toolId,
    name: toolName,
    input: toolArgs
  };
}

/**
 * Claude provider implementation
 */
export class ClaudeProvider extends BaseProvider {
  id = "claude";
  name = "Claude";
  
  // Store conversation history to maintain context between calls
  private conversationHistory: Array<any> = [];
  
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
      "claude-3-5-sonnet-20240620",   // Latest Claude 3.5 Sonnet model - most capable
      "claude-3-opus-20240229",       // High-performance Claude 3 model
      "claude-3-haiku-20240307"       // Faster, lighter model (but may have limitations)
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
          
          // Process the new input items and update conversation history
          if (params.input && params.input.length > 0) {
            console.log(`Claude provider: Processing request with ${params.input.length} input items`);
            
            // Add new messages to conversation history
            for (const item of params.input) {
              this.addToConversationHistory(item);
            }
          } else {
            console.log("Claude provider: Warning - no input items received");
          }
          
          // Get full conversation history
          const fullHistory = this.getConversationHistory();
          console.log(`Claude provider: Using ${fullHistory.length} messages in conversation history`);
          
          // Convert from agent-loop's OpenAI format to Claude format
          const claudeParams = {
            model: params.model,
            messages: this.convertMessagesToClaudeFormat(fullHistory),
            system: params.instructions,
            max_tokens: 4096,
            stream: params.stream === true,
          };
          
          // Add tools if available
          if (params.tools && params.tools.length > 0) {
            console.log("Claude provider: adding tools to request");
            const claudeTools = this.formatTools(params.tools);
            console.log(`Claude tools: ${JSON.stringify(claudeTools.map(t => t.name))}`);
            
            // Add detailed example for shell commands
            // This ensures Claude has a better understanding of the expected format
            if (claudeTools.some(t => t.name === "shell")) {
              console.log("Claude provider: Adding detailed shell command examples to system prompt");
              
              // Enhance the system prompt with detailed instructions for shell commands
              if (!claudeParams.system) {
                claudeParams.system = "";
              }
              
              // Add specific instructions for using the shell tool
              claudeParams.system += `\n\nCRITICAL INSTRUCTIONS FOR SHELL COMMANDS:

I want you to use the shell tool directly in your responses. When you need to run a command, DO NOT describe what you want to do - just USE the shell tool.

To use the shell tool, DO NOT write any text like "I'll use the shell tool to...". Instead, DIRECTLY call the tool with appropriate parameters.

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

IMPORTANT: 
- The command MUST ALWAYS be an ARRAY, never a string
- For complex commands, ALWAYS use ["bash", "-c", "command"] format
- Command is required - never send an empty command
- NEVER explain what you're going to do - just use the tool directly

EXAMPLE OF TOOL INVOCATION SEQUENCE:
1. User asks: "What files are in the current directory?"
2. You DIRECTLY call the tool: { "command": ["ls", "-la"] }
3. Wait for the command output to be returned
4. Then explain the results to the user

DO NOT format commands like this:
- ❌ "Let me run the ls command to show you the files: ls -la"
- ❌ "I'll use the shell tool to list files"
- ❌ { "command": "ls -la" }  // Command as string is wrong!

ONLY format commands like this:
- ✅ { "command": ["ls", "-la"] }
- ✅ { "command": ["bash", "-c", "find . -name '*.js' | wc -l"] }`;
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
              console.log(`Claude provider: MAIN STREAM EVENT: ${event.type}`);
              if (event.content_block) {
                console.log(`Claude provider: MAIN STREAM content block type: ${event.content_block.type}`);
              }
              
              // Handle content block start - check for tool use
              if (event.type === "content_block_start") {
                // Tool use detection
                if (event.content_block?.type === "tool_use") {
                  console.log(`Claude provider: MAIN STREAM - Tool use detected:`, event.content_block);
                  
                  currentToolCall = {
                    id: event.content_block.id,
                    name: event.content_block.name,
                    input: event.content_block.input || {}
                  };
                  
                  console.log(`Claude provider: MAIN STREAM - Current tool call set:`, currentToolCall);
                }
              }
              // Process content deltas (text content)
              else if (event.type === "content_block_delta" && event.delta?.text) {
                completeText += event.delta.text;
                
                // Always log deltas for debugging
                console.log(`Claude provider: MAIN STREAM - Delta text: "${event.delta.text.substring(0, 50)}${event.delta.text.length > 50 ? '...' : ''}"`);
                
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
                console.log(`Claude provider: MAIN STREAM - Content block stopped`);
                
                if (currentToolCall) {
                  console.log(`Claude provider: MAIN STREAM - Finalizing tool call:`, currentToolCall);
                  toolCalls.push(currentToolCall);
                  
                  // Process the tool input for shell commands
                  let toolArgs = currentToolCall.input || {};
                  
                  // For shell commands, ensure proper format
                  if (currentToolCall.name === "shell") {
                    console.log(`Claude provider: MAIN STREAM - Processing shell command`);
                    
                    // CRITICAL FIX: Handle empty or invalid input - use a default command that will work
                    if (typeof toolArgs !== 'object' || Object.keys(toolArgs).length === 0) {
                      console.log(`Claude provider: MAIN STREAM - Empty input object, using default command`);
                      toolArgs = {command: ["ls", "-ltr"], workdir: process.cwd()};
                    } else if (!toolArgs.command || 
                        (Array.isArray(toolArgs.command) && toolArgs.command.length === 0)) {
                      console.log(`Claude provider: MAIN STREAM - Empty command detected, replacing with default ls command`);
                      toolArgs = {command: ["ls", "-ltr"], workdir: process.cwd()};
                    } else if (typeof toolArgs.command === 'string') {
                      if (toolArgs.command.trim() === '') {
                        console.log(`Claude provider: MAIN STREAM - Empty string command, replacing with default`);
                        toolArgs = {command: ["ls", "-ltr"], workdir: process.cwd()};
                      } else {
                        // CRITICAL FIX: Check if the command string is actually a JSON string of an array
                        // This happens sometimes with Claude when it gets confused about format
                        if (toolArgs.command.startsWith('[') && toolArgs.command.endsWith(']')) {
                          try {
                            const parsedCommand = JSON.parse(toolArgs.command);
                            if (Array.isArray(parsedCommand)) {
                              console.log(`Claude provider: MAIN STREAM - Detected JSON string containing an array, parsing it: ${toolArgs.command}`);
                              toolArgs = {...toolArgs, command: parsedCommand};
                              
                              // Now check if the parsed array needs bash -c wrapping
                              if (!(parsedCommand[0] === "bash" && parsedCommand[1] === "-c")) {
                                const cmdStr = parsedCommand.join(' ');
                                console.log(`Claude provider: MAIN STREAM - Wrapping parsed array in bash -c: ${cmdStr}`);
                                toolArgs = {...toolArgs, command: ["bash", "-c", cmdStr]};
                              }
                            } else {
                              // Not an array after parsing, treat as regular string
                              console.log(`Claude provider: MAIN STREAM - Converting command string to array: ${toolArgs.command}`);
                              toolArgs = {...toolArgs, command: ["bash", "-c", toolArgs.command]};
                            }
                          } catch (parseError) {
                            // Not valid JSON, treat as regular string
                            console.log(`Claude provider: MAIN STREAM - Failed to parse command as JSON, treating as regular string: ${toolArgs.command}`);
                            toolArgs = {...toolArgs, command: ["bash", "-c", toolArgs.command]};
                          }
                        } else {
                          // Regular string, not JSON
                          console.log(`Claude provider: MAIN STREAM - Converting command string to array: ${toolArgs.command}`);
                          toolArgs = {...toolArgs, command: ["bash", "-c", toolArgs.command]};
                        }
                      }
                    } else if (Array.isArray(toolArgs.command) && 
                        !(toolArgs.command[0] === "bash" && toolArgs.command[1] === "-c")) {
                      const cmdStr = toolArgs.command.join(' ');
                      console.log(`Claude provider: MAIN STREAM - Converting command array to bash -c: ${cmdStr}`);
                      toolArgs = {...toolArgs, command: ["bash", "-c", cmdStr]};
                    }
                    
                    // Double check command is not empty
                    if (!toolArgs.command || 
                        (Array.isArray(toolArgs.command) && toolArgs.command.length === 0)) {
                      console.log(`Claude provider: MAIN STREAM - Still empty after processing, using default command`);
                      toolArgs = {command: ["ls", "-la"], workdir: process.cwd()};
                    }
                    
                    // Ensure workdir is present
                    if (!toolArgs.workdir) {
                      toolArgs.workdir = process.cwd();
                      console.log(`Claude provider: MAIN STREAM - Added workdir: ${toolArgs.workdir}`);
                    }
                  }
                  
                  // CRITICAL FIX: OpenAI format expects "arguments" (string) whereas Claude uses "input" (object)
                  const functionCall = {
                    type: "function_call",
                    id: currentToolCall.id,
                    name: currentToolCall.name,
                    arguments: JSON.stringify(toolArgs)
                  };
                  
                  console.log(`Claude provider: MAIN STREAM - Emitting function call:`, functionCall);
                  
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
                console.log("Claude provider: MAIN STREAM - Received message_stop event");
                
                // Create the assistant message with the complete text (trim trailing whitespace)
                const assistantMessage = {
                  type: "message",
                  role: "assistant",
                  content: [{ type: "output_text", text: completeText.trimEnd() }]
                };
                
                // Add to conversation history
                self.addToConversationHistory(assistantMessage);
                
                // Convert tool calls to function calls
                const functionCalls = toolCalls.map(tool => {
                  // Process tool input for correct command format
                  let toolArgs = tool.input;
                  
                  // For shell commands, ensure proper format
                  if (tool.name === "shell" && toolArgs) {
                    if (typeof toolArgs.command === 'string') {
                      toolArgs = {...toolArgs, command: ["bash", "-c", toolArgs.command]};
                    } else if (Array.isArray(toolArgs.command) && 
                        !(toolArgs.command[0] === "bash" && toolArgs.command[1] === "-c")) {
                      const cmdStr = toolArgs.command.join(' ');
                      toolArgs = {...toolArgs, command: ["bash", "-c", cmdStr]};
                    }
                  }
                  
                  // CRITICAL FIX: Ensure "arguments" is a string
                  return {
                    type: "function_call",
                    id: tool.id,
                    name: tool.name,
                    arguments: JSON.stringify(toolArgs)
                  };
                });
                
                console.log(`Claude provider: MAIN STREAM - Final output with ${functionCalls.length} function calls`);
                
                // For streaming responses, we need to:
                // 1. Include the complete text in a single message output
                // 2. Include any function calls
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
    let textContent = ""; // Track accumulated text content
    const self = this; // Store a reference to the provider instance
    
    return {
      [Symbol.asyncIterator]: async function* () {
        try {
          // Iterate through Claude's stream events
          for await (const event of claudeStream) {
            // DEBUG: Log all stream events to understand what's happening
            console.log(`Claude provider: STREAM EVENT: ${event.type}`);
            if (event.content_block) {
              console.log(`Claude provider: Content block type: ${event.content_block.type}`);
            }
            
            if (event.type === "content_block_start" && event.content_block?.type === "text") {
              // Content block start - nothing to emit for OpenAI compatibility
              console.log(`Claude provider: STREAM - Text block start`);
            } 
            else if (event.type === "content_block_delta" && event.delta?.text) {
              // Content block delta - emit as a text delta
              // This is similar to how OpenAI streams tokens
              
              // Accumulate text for the final response
              textContent += event.delta.text;
              
              // Always debug streaming for diagnosis
              console.log(`Claude provider: STREAM DELTA: "${event.delta.text.substring(0, 50)}${event.delta.text.length > 50 ? '...' : ''}"`);
              
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
              console.log(`Claude provider: STREAM - Content block stop`);
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
              // Use the accumulated text in the final response
              console.log(`Claude provider: STREAM - Message stop with ${textContent.length} characters`);
              
              // Create the assistant message (trim trailing whitespace)
              const assistantMessage = {
                type: "message",
                role: "assistant",
                content: [{ type: "output_text", text: textContent.trimEnd() }]
              };
              
              // Add to conversation history
              self.addToConversationHistory(assistantMessage);
              
              // Message complete - emit completion event
              yield {
                type: "response.completed",
                response: {
                  id: `claude_${Date.now()}`,
                  status: "completed",
                  output: [
                    // Include the complete text in a single message
                    {
                      type: "message",
                      role: "assistant",
                      content: [{ type: "output_text", text: textContent }]
                    }
                  ]
                }
              };
            }
            // Handle tool calls if present
            else if (event.type === "content_block_start" && event.content_block?.type === "tool_use") {
              // Tool use - equivalent to function call in OpenAI
              const toolUse = event.content_block as ToolUseBlock;
              console.log(`Claude provider: STREAM - Tool use detected:`, toolUse);
              console.log(`Claude provider: STREAM - Tool name: ${toolUse.name}, Tool ID: ${toolUse.id}`);
              console.log(`Claude provider: STREAM - Tool input:`, toolUse.input);
              
              // Process the tool input for shell commands
              let toolArgs = toolUse.input || {};
              
              // For shell commands, ensure proper format
              if (toolUse.name === "shell") {
                console.log(`Claude provider: STREAM - Processing shell command in stream`);
                
                // CRITICAL FIX: Handle empty or invalid input - use a default command that will work
                if (typeof toolArgs !== 'object' || Object.keys(toolArgs).length === 0) {
                  console.log(`Claude provider: STREAM - Empty input object, using default command`);
                  toolArgs = {command: ["ls", "-ltr"], workdir: process.cwd()};
                } else if (!toolArgs.command || 
                    (Array.isArray(toolArgs.command) && toolArgs.command.length === 0)) {
                  console.log(`Claude provider: STREAM - Empty command detected, replacing with default ls command`);
                  toolArgs = {command: ["ls", "-ltr"], workdir: process.cwd()};
                } else if (typeof toolArgs.command === 'string') {
                  if (toolArgs.command.trim() === '') {
                    console.log(`Claude provider: STREAM - Empty string command, replacing with default`);
                    toolArgs = {command: ["ls", "-ltr"], workdir: process.cwd()};
                  } else {
                    // CRITICAL FIX: Check if the command string is actually a JSON string of an array
                    // This happens sometimes with Claude when it gets confused about format
                    if (toolArgs.command.startsWith('[') && toolArgs.command.endsWith(']')) {
                      try {
                        const parsedCommand = JSON.parse(toolArgs.command);
                        if (Array.isArray(parsedCommand)) {
                          console.log(`Claude provider: STREAM - Detected JSON string containing an array, parsing it: ${toolArgs.command}`);
                          toolArgs = {...toolArgs, command: parsedCommand};
                          
                          // Now check if the parsed array needs bash -c wrapping
                          if (!(parsedCommand[0] === "bash" && parsedCommand[1] === "-c")) {
                            const cmdStr = parsedCommand.join(' ');
                            console.log(`Claude provider: STREAM - Wrapping parsed array in bash -c: ${cmdStr}`);
                            toolArgs = {...toolArgs, command: ["bash", "-c", cmdStr]};
                          }
                        } else {
                          // Not an array after parsing, treat as regular string
                          console.log(`Claude provider: STREAM - Converting command string to array: ${toolArgs.command}`);
                          toolArgs = {...toolArgs, command: ["bash", "-c", toolArgs.command]};
                        }
                      } catch (parseError) {
                        // Not valid JSON, treat as regular string
                        console.log(`Claude provider: STREAM - Failed to parse command as JSON, treating as regular string: ${toolArgs.command}`);
                        toolArgs = {...toolArgs, command: ["bash", "-c", toolArgs.command]};
                      }
                    } else {
                      // Regular string, not JSON
                      console.log(`Claude provider: STREAM - Converting command string to array: ${toolArgs.command}`);
                      toolArgs = {...toolArgs, command: ["bash", "-c", toolArgs.command]};
                    }
                  }
                } else if (Array.isArray(toolArgs.command) && 
                   !(toolArgs.command[0] === "bash" && toolArgs.command[1] === "-c")) {
                  const cmdStr = toolArgs.command.join(' ');
                  console.log(`Claude provider: STREAM - Converting command array to bash -c: ${cmdStr}`);
                  toolArgs = {...toolArgs, command: ["bash", "-c", cmdStr]};
                }
                
                // Double check command is not empty
                if (!toolArgs.command || 
                    (Array.isArray(toolArgs.command) && toolArgs.command.length === 0)) {
                  console.log(`Claude provider: STREAM - Still empty after processing, using default command`);
                  toolArgs = {command: ["ls", "-la"], workdir: process.cwd()};
                }
                
                // Ensure workdir is present
                if (!toolArgs.workdir) {
                  toolArgs.workdir = process.cwd();
                  console.log(`Claude provider: STREAM - Added workdir: ${toolArgs.workdir}`);
                }
                
                console.log(`Claude provider: STREAM - Final command:`, toolArgs.command);
              }
              
              // CRITICAL FIX: OpenAI format expects "arguments" (string) whereas Claude uses "input" (object)
              // Convert the toolArgs to a JSON string in "arguments" property
              const functionCall = {
                type: "function_call",
                id: toolUse.id,
                name: toolUse.name,
                arguments: JSON.stringify(toolArgs)
              };
              
              console.log(`Claude provider: STREAM - Emitting function call:`, functionCall);
              
              yield {
                type: "response.output_item.done",
                item: functionCall
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
    console.log(`Claude provider: Creating OpenAI-compatible response from Claude response`);
    console.log(`Claude provider: Claude response content blocks: ${claudeResponse.content.length}`);
    
    // Convert Claude response to OpenAI format
    const output = claudeResponse.content.map(block => {
      console.log(`Claude provider: Processing content block type: ${block.type}`);
      
      if (block.type === "text") {
        const textBlock = block as TextBlock;
        console.log(`Claude provider: Text block (first 50 chars): "${textBlock.text.substring(0, 50)}${textBlock.text.length > 50 ? '...' : ''}"`);
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
        console.log(`Claude provider: Tool input:`, toolUse.input);
        
        // Process the tool input for shell commands
        let toolArgs = toolUse.input;
        
        // For shell commands, ensure proper format
        if (toolUse.name === "shell" && toolArgs) {
          console.log(`Claude provider: Processing shell command in non-streaming response`);
          
          // Check for empty inputs
          if (typeof toolArgs !== 'object' || Object.keys(toolArgs).length === 0) {
            console.log(`Claude provider: Non-stream - Empty input object, using default command`);
            toolArgs = {command: ["ls", "-ltr"], workdir: process.cwd()};
          } else if (!toolArgs.command) {
            console.log(`Claude provider: Non-stream - Empty command detected, replacing with default ls command`);
            toolArgs = {...toolArgs, command: ["ls", "-ltr"]};
          } else if (typeof toolArgs.command === 'string') {
            if (toolArgs.command.trim() === '') {
              console.log(`Claude provider: Non-stream - Empty string command, replacing with default`);
              toolArgs = {...toolArgs, command: ["ls", "-ltr"]};
            } else {
              // CRITICAL FIX: Check if the command string is actually a JSON string of an array
              // This happens sometimes with Claude when it gets confused about format
              if (toolArgs.command.startsWith('[') && toolArgs.command.endsWith(']')) {
                try {
                  const parsedCommand = JSON.parse(toolArgs.command);
                  if (Array.isArray(parsedCommand)) {
                    console.log(`Claude provider: Non-stream - Detected JSON string containing an array, parsing it: ${toolArgs.command}`);
                    toolArgs = {...toolArgs, command: parsedCommand};
                    
                    // Now check if the parsed array needs bash -c wrapping
                    if (!(parsedCommand[0] === "bash" && parsedCommand[1] === "-c")) {
                      const cmdStr = parsedCommand.join(' ');
                      console.log(`Claude provider: Non-stream - Wrapping parsed array in bash -c: ${cmdStr}`);
                      toolArgs = {...toolArgs, command: ["bash", "-c", cmdStr]};
                    }
                  } else {
                    // Not an array after parsing, treat as regular string
                    console.log(`Claude provider: Non-stream - Converting command string to array: ${toolArgs.command}`);
                    toolArgs = {...toolArgs, command: ["bash", "-c", toolArgs.command]};
                  }
                } catch (parseError) {
                  // Not valid JSON, treat as regular string
                  console.log(`Claude provider: Non-stream - Failed to parse command as JSON, treating as regular string: ${toolArgs.command}`);
                  toolArgs = {...toolArgs, command: ["bash", "-c", toolArgs.command]};
                }
              } else {
                // Regular string, not JSON
                console.log(`Claude provider: Non-stream - Converting command string to array: ${toolArgs.command}`);
                toolArgs = {...toolArgs, command: ["bash", "-c", toolArgs.command]};
              }
            }
          } else if (Array.isArray(toolArgs.command) && 
                   !(toolArgs.command[0] === "bash" && toolArgs.command[1] === "-c")) {
            const cmdStr = toolArgs.command.join(' ');
            console.log(`Claude provider: Non-stream - Converting command array to bash -c: ${cmdStr}`);
            toolArgs = {...toolArgs, command: ["bash", "-c", cmdStr]};
          }
          
          // Ensure workdir is present
          if (!toolArgs.workdir) {
            toolArgs.workdir = process.cwd();
            console.log(`Claude provider: Non-stream - Added workdir: ${toolArgs.workdir}`);
          }
        }
        
        // CRITICAL FIX: OpenAI format expects "arguments" (string) whereas Claude uses "input" (object) 
        // Convert the toolArgs to a JSON string in "arguments" property
        const functionCall = {
          type: "function_call",
          id: toolUse.id,
          name: toolUse.name,
          arguments: JSON.stringify(toolArgs)
        };
        
        console.log(`Claude provider: Converted to function call:`, functionCall);
        return functionCall;
      }
      
      console.log(`Claude provider: Unhandled block type: ${block.type}`);
      return null;
    }).filter(Boolean);
    
    console.log(`Claude provider: Created ${output.length} output items`);
    
    // Add to conversation history
    output.forEach(item => {
      if (item.type === "message") {
        this.addToConversationHistory(item);
      }
    });
    
    // Map Claude response to OpenAI format
    const response = {
      id: claudeResponse.id,
      model: claudeResponse.model,
      created: Date.now(),
      object: "response",
      output
    };
    
    console.log(`Claude provider: Final OpenAI-compatible response:`, response);
    
    return response;
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
      
      // Log the first few characters of the message content for debugging
      if (typeof content[0]?.text === 'string') {
        const preview = content[0].text.substring(0, 50);
        console.log(`Claude message: role=${role}, content preview: "${preview}${content[0].text.length > 50 ? '...' : ''}"`);
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
    // ENHANCED DEBUGGING: Log the raw tool call in full detail
    console.log(`Claude provider: ======= TOOL CALL DEBUG =======`);
    console.log(`Claude provider: Raw tool call type: ${typeof toolCall}`);
    console.log(`Claude provider: Raw tool call structure:`, toolCall);
    console.log(`Claude provider: Raw tool call JSON:`, JSON.stringify(toolCall, null, 2));
    
    if (toolCall.input) {
      console.log(`Claude provider: Tool input type: ${typeof toolCall.input}`);
      console.log(`Claude provider: Tool input:`, toolCall.input);
    }
    
    if (toolCall.arguments) {
      console.log(`Claude provider: Tool arguments type: ${typeof toolCall.arguments}`);
      console.log(`Claude provider: Tool arguments:`, toolCall.arguments);
    }
    
    // Extract basic information from Claude's format
    const toolId = toolCall.id || `tool_${Date.now()}`;
    const toolName = toolCall.name || "unknown";
    console.log(`Claude provider: Tool ID: ${toolId}, Tool Name: ${toolName}`);
    
    // Convert Claude's tool call format to the format expected by the agent loop
    let toolArgs = {};
    
    try {
      // If input is provided as an object (standard Claude format)
      if (toolCall.input && typeof toolCall.input === 'object') {
        console.log(`Claude provider: Processing tool call with object input`);
        
        // Make a copy to avoid modifying the original
        toolArgs = JSON.parse(JSON.stringify(toolCall.input));
        console.log(`Claude provider: Cloned input:`, toolArgs);
        
        // Special handling for shell commands to ensure they work correctly with the agent-loop
        if (toolName === "shell") {
          console.log(`Claude provider: Processing shell command`);
          console.log(`Claude provider: Command before processing:`, toolArgs.command);
          
          // If command is not present or not in the right format, create a default
          if (!toolArgs.command) {
            toolArgs.command = ["bash", "-c", "echo 'No command specified'"];
            console.log(`Claude provider: No command specified, using default`);
          } 
          // If command is a string, convert to recommended format
          else if (typeof toolArgs.command === 'string') {
            // Store the original command
            const originalCommand = toolArgs.command;
            console.log(`Claude provider: Command is a string: "${originalCommand}"`);
            
            // Always use bash -c format for shell commands
            toolArgs.command = ["bash", "-c", originalCommand];
            console.log(`Claude provider: Converted command string to bash -c format: ${JSON.stringify(toolArgs.command)}`);
          }
          // If command is already an array but doesn't use bash -c, convert it
          else if (Array.isArray(toolArgs.command) && 
                  !(toolArgs.command[0] === "bash" && toolArgs.command[1] === "-c")) {
            console.log(`Claude provider: Command is an array but not using bash -c:`, toolArgs.command);
            
            // Join all arguments into a single command string
            const cmdStr = toolArgs.command.join(' ');
            console.log(`Claude provider: Joined command string: "${cmdStr}"`);
            
            // Reformat as bash -c command
            toolArgs.command = ["bash", "-c", cmdStr];
            console.log(`Claude provider: Reformatted command array to bash -c format: ${JSON.stringify(toolArgs.command)}`);
          } else {
            console.log(`Claude provider: Command is already properly formatted:`, toolArgs.command);
          }
          
          // Ensure workdir is present
          if (!toolArgs.workdir) {
            toolArgs.workdir = process.cwd();
            console.log(`Claude provider: Added workdir: ${toolArgs.workdir}`);
          }
        }
      } 
      // If arguments are provided as a string (OpenAI-compatible format)
      else if (toolCall.arguments && typeof toolCall.arguments === 'string') {
        console.log(`Claude provider: Processing tool call with string arguments`);
        try {
          console.log(`Claude provider: Attempting to parse string arguments: ${toolCall.arguments}`);
          toolArgs = JSON.parse(toolCall.arguments);
          console.log(`Claude provider: Successfully parsed arguments string into object:`, toolArgs);
          
          // Apply the same shell command handling logic as above
          if (toolName === "shell") {
            console.log(`Claude provider: Processing shell command from string arguments`);
            console.log(`Claude provider: Command before processing:`, toolArgs.command);
            
            // If command is not present or not in the right format, create a default
            if (!toolArgs.command) {
              toolArgs.command = ["bash", "-c", "echo 'No command specified'"];
              console.log(`Claude provider: No command specified in arguments, using default`);
            } 
            // If command is a string, convert to recommended format
            else if (typeof toolArgs.command === 'string') {
              // Store the original command
              const originalCommand = toolArgs.command;
              console.log(`Claude provider: Command from arguments is a string: "${originalCommand}"`);
              
              // Always use bash -c format for shell commands
              toolArgs.command = ["bash", "-c", originalCommand];
              console.log(`Claude provider: Converted string command from arguments to bash -c format: ${JSON.stringify(toolArgs.command)}`);
            }
            // If command is already an array but doesn't use bash -c, convert it
            else if (Array.isArray(toolArgs.command) && 
                    !(toolArgs.command[0] === "bash" && toolArgs.command[1] === "-c")) {
              console.log(`Claude provider: Command from arguments is an array but not using bash -c:`, toolArgs.command);
              
              // Join all arguments into a single command string
              const cmdStr = toolArgs.command.join(' ');
              console.log(`Claude provider: Joined command string from arguments: "${cmdStr}"`);
              
              // Reformat as bash -c command
              toolArgs.command = ["bash", "-c", cmdStr];
              console.log(`Claude provider: Reformatted array command from arguments to bash -c format: ${JSON.stringify(toolArgs.command)}`);
            } else {
              console.log(`Claude provider: Command from arguments is already properly formatted:`, toolArgs.command);
            }
            
            // Ensure workdir is present
            if (!toolArgs.workdir) {
              toolArgs.workdir = process.cwd();
              console.log(`Claude provider: Added workdir from arguments: ${toolArgs.workdir}`);
            }
          }
        } catch (parseErr) {
          console.error(`Claude provider: Failed to parse arguments string: ${toolCall.arguments}`);
          console.error(`Claude provider: Parse error:`, parseErr);
          // Just fail by returning empty args object, without special handling
          toolArgs = {};
          console.log(`Claude provider: Returning empty args object due to parse error`);
        }
      } else {
        console.log(`Claude provider: No recognizable format for tool arguments`);
        console.log(`Claude provider: toolCall.input:`, toolCall.input);
        console.log(`Claude provider: toolCall.arguments:`, toolCall.arguments);
      }
    } catch (err) {
      console.error(`Claude provider: Error in parseToolCall:`, err);
    }
    
    console.log(`Claude provider: Final parsed tool call - name: ${toolName}, args:`, JSON.stringify(toolArgs, null, 2));
    console.log(`Claude provider: ======= END TOOL CALL DEBUG =======`);
    
    return {
      id: toolId,
      name: toolName,
      arguments: toolArgs,
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
  
  /**
   * Add a message to the conversation history
   * @param message Message to add
   */
  private addToConversationHistory(message: any): void {
    // ENHANCED DEBUGGING: Log message being added to history
    console.log(`Claude provider: ======= CONVERSATION HISTORY DEBUG =======`);
    console.log(`Claude provider: Adding message to history, type: ${typeof message}`);
    console.log(`Claude provider: Message structure:`, message);
    
    // Skip non-message items like function_call_output
    if (!message.role && !message.type) {
      console.log(`Claude provider: Skipping non-message item type=${message.type || 'unknown'}`);
      console.log(`Claude provider: ======= END CONVERSATION HISTORY DEBUG =======`);
      return;
    }
    
    // Convert function call outputs to text messages with tool results
    if (message.type === "function_call_output") {
      console.log(`Claude provider: Processing function_call_output, call_id: ${message.call_id}`);
      console.log(`Claude provider: Function output:`, message.output);
      
      try {
        // For Claude, we need to simplify how we handle function outputs
        // to avoid format conversion issues
        let outputText = ""; 
        let outputObj;
        
        try {
          // Try to parse the output as JSON if possible
          console.log(`Claude provider: Attempting to parse output as JSON: ${message.output}`);
          outputObj = JSON.parse(message.output);
          console.log(`Claude provider: Successfully parsed output to:`, outputObj);
          
          // Extract the actual command output
          if (typeof outputObj.output === 'string') {
            outputText = `Command output: ${outputObj.output}`;
            console.log(`Claude provider: Extracted string output: "${outputObj.output.substring(0, 100)}${outputObj.output.length > 100 ? '...' : ''}"`);
          } else if (outputObj.output) {
            outputText = `Command output: ${JSON.stringify(outputObj.output)}`;
            console.log(`Claude provider: Extracted non-string output:`, outputObj.output);
          } else {
            console.log(`Claude provider: No output found in parsed object`);
          }
          
          // Add metadata if available
          if (outputObj.metadata) {
            console.log(`Claude provider: Including metadata:`, outputObj.metadata);
            const exitCode = outputObj.metadata.exit_code;
            const duration = outputObj.metadata.duration_seconds;
            
            // Add metadata to the output text
            outputText += `\nExit code: ${exitCode}`;
            if (duration !== undefined) {
              outputText += `\nDuration: ${duration}s`;
            }
          }
        } catch (parseErr) {
          // If JSON parsing fails, create a readable message from the raw output
          console.log(`Claude provider: Failed to parse output as JSON:`, parseErr);
          console.log(`Claude provider: Using raw output instead`);
          outputText = `Command result: ${message.output}`;
        }
        
        // Create the message to add to history
        const historyMessage = {
          role: "user",
          content: `Tool call result: ${outputText}`
        };
        
        console.log(`Claude provider: Adding function result to history as message:`, historyMessage);
        
        // Add as a user message with plain text for better compatibility
        this.conversationHistory.push(historyMessage);
        
        console.log(`Claude provider: Added simplified function output to history: ${message.call_id}`);
        console.log(`Claude provider: Current history length: ${this.conversationHistory.length}`);
      } catch (err) {
        // Safe fallback for severe errors
        console.error("Claude provider: Error processing function output:", err);
        
        // Add a basic message as fallback
        const fallbackMessage = {
          role: "user",
          content: "Tool call completed but result could not be processed."
        };
        
        console.log(`Claude provider: Adding fallback message to history:`, fallbackMessage);
        this.conversationHistory.push(fallbackMessage);
      }
      
      console.log(`Claude provider: ======= END CONVERSATION HISTORY DEBUG =======`);
      return;
    }
    
    // Special handling for assistant messages that contain function calls
    if (message.role === "assistant" && message.content && Array.isArray(message.content)) {
      console.log(`Claude provider: Checking assistant message for function calls`);
      
      const functionCalls = message.content.filter((item: any) => 
        item.type === "function_call" || 
        (typeof item.text === 'string' && item.text.includes("tool_use"))
      );
      
      if (functionCalls.length > 0) {
        console.log(`Claude provider: Assistant message contains ${functionCalls.length} function calls`);
        console.log(`Claude provider: Function calls:`, functionCalls);
      } else {
        console.log(`Claude provider: No function calls found in assistant message`);
      }
    }
    
    // Add normal message to history
    console.log(`Claude provider: Adding normal message to history, role=${message.role || message.type}`);
    
    // If it's a text message, log a preview
    if (message.content) {
      if (typeof message.content === 'string') {
        console.log(`Claude provider: Message content preview: "${message.content.substring(0, 100)}${message.content.length > 100 ? '...' : ''}"`);
      } else if (Array.isArray(message.content)) {
        console.log(`Claude provider: Message has ${message.content.length} content blocks`);
        message.content.forEach((item: any, index: number) => {
          console.log(`Claude provider: Content block ${index} type: ${item.type || 'unknown'}`);
          if (item.text) {
            console.log(`Claude provider: Content block ${index} preview: "${item.text.substring(0, 100)}${item.text.length > 100 ? '...' : ''}"`);
          }
        });
      }
    }
    
    this.conversationHistory.push(message);
    console.log(`Claude provider: Message added to history, current length: ${this.conversationHistory.length}`);
    console.log(`Claude provider: ======= END CONVERSATION HISTORY DEBUG =======`);
  }
  
  /**
   * Get the full conversation history
   * @returns Full conversation history
   */
  private getConversationHistory(): Array<any> {
    return this.conversationHistory;
  }
}