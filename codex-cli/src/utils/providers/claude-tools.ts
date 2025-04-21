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
  console.log(`Claude tools: Shell commands are not implemented in claude provider. Command received:`, command);
  return ["echo", "Shell commands are not implemented in claude provider"];
}

/**
 * Process shell tool input to ensure proper format
 * 
 * @param toolInput The input from a shell tool call
 * @returns Properly formatted tool arguments
 */
export function processShellToolInput(toolInput: any): { command: string[], workdir?: string } {
  console.log(`Claude tools: Shell commands are not implemented in claude provider. Tool input received:`, toolInput);
  return {
    command: ["echo", "Shell commands are not implemented in claude provider"],
    workdir: process.cwd()
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
    console.log(`Claude tools: Shell commands are not implemented in claude provider. Tool use object:`, toolUse);
    toolArgs = {
      command: ["echo", "Shell commands are not implemented in claude provider"],
      workdir: process.cwd()
    };
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
    console.log(`Claude tools: Shell commands are not implemented in claude provider. Tool call received:`, toolCall);
    toolArgs = {
      command: ["echo", "Shell commands are not implemented in claude provider"],
      workdir: process.cwd()
    };
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
    console.log(`Claude tools: Tool use detected in stream event, name: ${toolUse.name}`);
    let toolArgs = toolUse.input;
    
    // Special handling for shell tool
    if (toolUse.name === "shell") {
      console.log(`Claude tools: Shell commands are not implemented in claude provider. Tool input received in stream:`, toolUse.input);
      toolArgs = {
        command: ["echo", "Shell commands are not implemented in claude provider"],
        workdir: process.cwd()
      };
    }
    
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
  console.log('Claude tools: Default tools requested, but shell commands are not implemented in claude provider');
  return [];
}

/**
 * Create the system prompt enhancement for shell commands
 * 
 * @returns String containing detailed shell command instructions
 */
export function createShellCommandInstructions(): string {
  console.log('Claude tools: Shell command instructions requested, but shell commands are not implemented in claude provider');
  return `
NOTICE: Shell commands are not implemented in the Claude provider.

Any attempt to use the shell tool will result in a message stating that shell commands are not supported.
`;
}