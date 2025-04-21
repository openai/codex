/**
 * Claude Provider Tools
 * 
 * Primitive functions for handling Claude API requests, responses, and tool calls.
 * These functions are designed to be:
 * 1. Pure functions with clear inputs and outputs
 * 2. Independently testable
 * 3. Composable to build the full Claude provider
 */

import type { AppConfig } from "../config.js";
import type { ParsedToolCall } from "./provider-interface.js";

/**
 * Normalize a shell command to the expected format
 * 
 * @param command The command to normalize (can be string, array, or undefined)
 * @returns A properly formatted command array
 */
export function normalizeShellCommand(command: any): string[] {
  // Handle empty or undefined command
  if (!command) {
    console.log(`Claude tools: Empty command detected, using default ls command`);
    return ["ls", "-la"];
  }
  
  // If command is a string
  if (typeof command === 'string') {
    // Handle empty string
    if (command.trim() === '') {
      console.log(`Claude tools: Empty string command, using default ls command`);
      return ["ls", "-la"];
    }
    
    // Check if the command string is actually a JSON string of an array
    if (command.startsWith('[') && command.endsWith(']')) {
      try {
        const parsedCommand = JSON.parse(command);
        if (Array.isArray(parsedCommand)) {
          console.log(`Claude tools: Detected JSON string containing an array, parsing it: ${command}`);
          
          // Now check if the parsed array needs bash -c wrapping
          if (!(parsedCommand[0] === "bash" && parsedCommand[1] === "-c")) {
            const cmdStr = parsedCommand.join(' ');
            console.log(`Claude tools: Wrapping parsed array in bash -c: ${cmdStr}`);
            return ["bash", "-c", cmdStr];
          }
          
          return parsedCommand;
        }
      } catch (parseError) {
        // Not valid JSON, treat as regular string
        console.log(`Claude tools: Failed to parse command as JSON, using bash -c: ${command}`);
      }
    }
    
    // For all other strings, wrap in bash -c
    console.log(`Claude tools: Converting command string to bash -c: ${command}`);
    return ["bash", "-c", command];
  }
  
  // If command is an array
  if (Array.isArray(command)) {
    // Handle empty array
    if (command.length === 0) {
      console.log(`Claude tools: Empty command array, using default ls command`);
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
        console.log(`Claude tools: Converting command array to bash -c: ${cmdStr}`);
        return ["bash", "-c", cmdStr];
      }
    }
    
    // Return the array as is
    return command;
  }
  
  // For any other type, return default command
  console.log(`Claude tools: Unknown command type (${typeof command}), using default ls command`);
  return ["ls", "-la"];
}

/**
 * Process shell tool input to ensure proper format
 * 
 * @param toolInput The input from a shell tool call
 * @returns Properly formatted tool arguments
 */
export function processShellToolInput(toolInput: any): { command: string[], workdir?: string } {
  // If input is a JSON string or raw string, attempt to parse or treat as direct command
  if (typeof toolInput === 'string') {
    try {
      const parsed = JSON.parse(toolInput);
      // If parsed is an array, treat as command array
      if (Array.isArray(parsed)) {
        console.log(`Claude tools: Parsed JSON array for shell command: ${toolInput}`);
        return { command: normalizeShellCommand(parsed), workdir: process.cwd() };
      }
      // If parsed is an object, use it as the new input
      if (parsed && typeof parsed === 'object') {
        toolInput = parsed;
      } else {
        // Fallback: treat string as shell command
        console.log(`Claude tools: Using raw string for shell command: ${toolInput}`);
        return { command: normalizeShellCommand(toolInput), workdir: process.cwd() };
      }
    } catch (e) {
      // Not JSON: treat as raw shell command string
      console.log(`Claude tools: Treating tool input string as shell command: ${toolInput}`);
      return { command: normalizeShellCommand(toolInput), workdir: process.cwd() };
    }
  }
  // Handle arrays of content blocks (e.g., text blocks)
  if (Array.isArray(toolInput)) {
    try {
      const text = toolInput
        .map((item: any) => (typeof item?.text === 'string' ? item.text : ''))
        .join('');
      console.log(`Claude tools: Extracted text from toolInput blocks: ${text}`);
      return processShellToolInput(text);
    } catch {
      // Fallback to default on error
      console.log(`Claude tools: Failed to extract text from array input, using default command`);
      return { command: ["ls", "-la"], workdir: process.cwd() };
    }
  }
  // Handle a single text block (from Claude) with .text property
  if (
    toolInput &&
    typeof toolInput === 'object' &&
    typeof (toolInput as any).text === 'string'
  ) {
    const txt = (toolInput as any).text;
    console.log(`Claude tools: Detected single text block for shell command: ${txt}`);
    return processShellToolInput(txt);
  }
  // Handle completely empty or missing or non-object input
  if (!toolInput || typeof toolInput !== 'object') {
    console.log(`Claude tools: Empty or invalid tool input, using default command`);
    return {
      command: ["ls", "-la"],
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
 * Convert Claude tool to OpenAI function call format
 * 
 * @param toolUse Claude tool use object
 * @returns OpenAI function call format
 */
export function claudeToolToOpenAIFunction(toolUse: any): any {
  // Validate tool use object
  if (!toolUse || !toolUse.name) {
    console.log(`Claude tools: Invalid tool use object`);
    return null;
  }
  
  const toolId = toolUse.id || `tool_${Date.now()}`;
  const toolName = toolUse.name;
  
  // Process input based on tool type
  let toolArgs = toolUse.input || {};
  
  // Special handling for shell tool
  if (toolName === "shell") {
    toolArgs = processShellToolInput(toolArgs);
  }
  
  // Convert to OpenAI format
  return {
    type: "function_call",
    id: toolId,
    name: toolName,
    arguments: JSON.stringify(toolArgs)
  };
}

/**
 * Parse a tool call from Claude format to common format
 * 
 * @param toolCall Claude tool call object
 * @returns Normalized tool call
 */
export function parseClaudeToolCall(toolCall: any): ParsedToolCall {
  // Extract basic information
  const toolId = toolCall.id || `tool_${Date.now()}`;
  const toolName = toolCall.name || "unknown";
  
  // Process input based on tool type
  let toolArgs = {};
  
  // Get the raw input from Claude's format
  const rawInput = toolCall.input || {};
  
  // Special handling for shell commands
  if (toolName === "shell") {
    toolArgs = processShellToolInput(rawInput);
  } else {
    // For other tools, just use the input as is
    toolArgs = rawInput;
  }
  
  return {
    id: toolId,
    name: toolName,
    arguments: toolArgs,
  };
}

/**
 * Convert Claude message format to OpenAI message format
 * 
 * @param claudeMessage Message in Claude format
 * @returns Message in OpenAI format
 */
export function convertClaudeMessageToOpenAI(claudeMessage: any): any {
  // Skip conversion if message is not in Claude format
  if (!claudeMessage || !claudeMessage.role || !claudeMessage.content) {
    return claudeMessage;
  }
  
  // Map roles (Claude only supports 'user' and 'assistant')
  const role = claudeMessage.role === "assistant" ? "assistant" : "user";
  
  // Handle different content types
  let content;
  
  // If content is an array (Claude's native format)
  if (Array.isArray(claudeMessage.content)) {
    content = claudeMessage.content.map(item => {
      // Convert text blocks
      if (item.type === "text") {
        return { 
          type: role === "assistant" ? "output_text" : "input_text", 
          text: item.text 
        };
      }
      
      // Convert image blocks (if present)
      if (item.type === "image") {
        return {
          type: "input_image",
          media_type: item.source.media_type || "image/jpeg",
          image_url: `data:${item.source.media_type || "image/jpeg"};base64,${item.source.data}`
        };
      }
      
      // Convert tool use blocks to function calls
      if (item.type === "tool_use") {
        return claudeToolToOpenAIFunction(item);
      }
      
      // For unknown types, return as is
      return item;
    });
  } 
  // If content is a string
  else if (typeof claudeMessage.content === "string") {
    content = [{ 
      type: role === "assistant" ? "output_text" : "input_text", 
      text: claudeMessage.content 
    }];
  }
  // For other formats, try to convert to string
  else {
    content = [{ 
      type: role === "assistant" ? "output_text" : "input_text", 
      text: JSON.stringify(claudeMessage.content) 
    }];
  }
  
  return {
    type: "message",
    role,
    content
  };
}

/**
 * Check if an error is a rate limit error from Claude API
 * 
 * @param error Error from Claude API
 * @returns True if it's a rate limit error
 */
export function isClaudeRateLimitError(error: any): boolean {
  if (error?.status === 429) {
    return true;
  }
  
  if (error?.error?.type === "rate_limit_error") {
    return true;
  }
  
  const errorMsg = error?.message || error?.error?.message || "";
  return /rate limit/i.test(errorMsg) || /too many requests/i.test(errorMsg);
}

/**
 * Check if an error is a context length error from Claude API
 * 
 * @param error Error from Claude API
 * @returns True if it's a context length error
 */
export function isClaudeContextLengthError(error: any): boolean {
  // Claude specific error type
  if (error?.error?.type === "context_length_exceeded") {
    return true;
  }
  
  // Check error status and message patterns
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
 * Format a Claude API error into a user-friendly message
 * 
 * @param error Error from Claude API
 * @returns User-friendly error message
 */
export function formatClaudeErrorMessage(error: any): string {
  // Handle known error types with Claude-specific messages
  if (isClaudeRateLimitError(error)) {
    return `Claude rate limit exceeded. Please try again in a few minutes.`;
  }
  
  if (isClaudeContextLengthError(error)) {
    return `The current request exceeds the maximum context length for Claude. Please shorten your prompt or conversation history, or switch to a Claude model with a larger context window.`;
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
 * Process a Claude stream event into a normalized format
 * 
 * @param event Claude stream event
 * @returns Normalized event in OpenAI-compatible format
 */
export function processClaudeStreamEvent(event: any): any {
  // Log event for debugging
  console.log(`Claude tools: Processing stream event: ${event.type}`);
  if (event.content_block) {
    console.log(`Claude tools: Content block type: ${event.content_block.type}`);
  }
  
  // Content block deltas (text content)
  if (event.type === "content_block_delta" && event.delta?.text) {
    console.log(`Claude tools: Delta text: "${event.delta.text.substring(0, 50)}${event.delta.text.length > 50 ? '...' : ''}"`);
    
    return {
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
  
  // Tool use blocks (function calls)
  if (event.type === "content_block_stop" && event.content_block?.type === "tool_use") {
    const toolUse = event.content_block;
    console.log(`Claude tools: Tool use detected:`, toolUse);
    
    // Process the tool input
    let toolArgs = processShellToolInput(toolUse.input);
    
    // Create function call in OpenAI format
    const functionCall = claudeToolToOpenAIFunction(toolUse);
    
    console.log(`Claude tools: Emitting function call:`, functionCall);
    
    return {
      type: "response.output_item.done",
      item: functionCall
    };
  }
  
  // Message completion
  if (event.type === "message_stop") {
    console.log(`Claude tools: Message stop event`);
    
    return {
      type: "response.completed",
      response: {
        id: `claude_${Date.now()}`,
        status: "completed",
        output: [] // Will be populated by the provider
      }
    };
  }
  
  // Default - return null for events we don't process
  return null;
}

/**
 * Create a default tools array for Claude
 * 
 * @returns Array of tools in Claude format
 */
export function createDefaultClaudeTools(): any[] {
  return [
    {
      name: "shell",
      description: "Runs a shell command, and returns its output.",
      input_schema: {
        type: "object",
        properties: {
          command: { 
            type: "array", 
            items: { type: "string" },
            description: "The command to execute as an array of strings."
          },
          workdir: {
            type: "string",
            description: "The working directory for the command."
          }
        },
        required: ["command"]
      }
    }
  ];
}

/**
 * Create the system prompt enhancement for shell commands
 * 
 * @returns String containing detailed shell command instructions
 */
export function createShellCommandInstructions(): string {
  return `
CRITICAL INSTRUCTIONS FOR SHELL COMMANDS:

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