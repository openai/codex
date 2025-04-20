# Claude Provider Implementation Details

## Overview

This document outlines the specific implementation details for integrating Anthropic's Claude models into Codex CLI through the provider abstraction layer. The Claude implementation will follow the LLMProvider interface defined in the provider-interface-design.md document.

## Claude Provider Implementation

### Basic Provider Structure

```typescript
// src/utils/providers/claude-provider.ts
import Anthropic from "@anthropic-ai/sdk";
import { BaseProvider, LLMProvider, CompletionParams, CompletionResult, ModelDefaults, ParsedToolCall } from "./provider-interface";
import { AppConfig } from "../config";

export class ClaudeProvider extends BaseProvider implements LLMProvider {
  id = "claude";
  name = "Anthropic Claude";
  
  // Implementation of LLMProvider interface methods...
}
```

### Model Management

```typescript
async getModels(): Promise<string[]> {
  // Claude does not currently have an API endpoint to list models
  // Return a static list of supported models
  return [
    "claude-3-opus-20240229",
    "claude-3-sonnet-20240229",
    "claude-3-haiku-20240307",
    "claude-3-5-sonnet-20240620"
  ];
}

async isModelSupported(model: string): Promise<boolean> {
  const models = await this.getModels();
  return models.includes(model);
}

getModelDefaults(model: string): ModelDefaults {
  // Base defaults for all Claude models
  const baseDefaults: ModelDefaults = {
    timeoutMs: 180000, // 3 minutes (longer than OpenAI default)
    temperature: 0.7,
    supportsToolCalls: true,
    supportsStreaming: true,
    contextWindowSize: 100000, // 100k tokens for most models
  };
  
  // Model-specific overrides
  switch (model) {
    case "claude-3-opus-20240229":
      return {
        ...baseDefaults,
        contextWindowSize: 200000, // 200k tokens
        timeoutMs: 300000, // 5 minutes for large contexts
      };
    case "claude-3-5-sonnet-20240620":
      return {
        ...baseDefaults,
        contextWindowSize: 200000, // 200k tokens
      };
    default:
      return baseDefaults;
  }
}
```

### Client Creation

```typescript
createClient(config: AppConfig): Anthropic {
  // Extract Claude-specific config
  const claudeConfig = config.providerConfigs?.claude || {};
  
  // Get API key from config or environment variable
  const apiKey = claudeConfig.apiKey || 
                 process.env.CLAUDE_API_KEY || 
                 process.env.ANTHROPIC_API_KEY || 
                 "";
  
  if (!apiKey) {
    throw new Error("Claude API key not found. Please set CLAUDE_API_KEY environment variable or configure it in the Codex config.");
  }
  
  // Create Anthropic client
  return new Anthropic({
    apiKey,
    baseURL: claudeConfig.baseUrl || process.env.CLAUDE_BASE_URL,
    timeout: claudeConfig.timeoutMs || parseInt(process.env.CLAUDE_TIMEOUT_MS || "180000", 10),
  });
}
```

### Running Completions

```typescript
async runCompletion(params: CompletionParams): Promise<CompletionResult> {
  const client = this.createClient(params.config);
  
  try {
    if (params.stream) {
      return await this.runStreamingCompletion(client, params);
    } else {
      return await this.runNonStreamingCompletion(client, params);
    }
  } catch (error) {
    // Handle errors and convert to standard format
    return {
      id: `error-${Date.now()}`,
      status: "error",
      output: [],
      errorDetails: {
        message: this.formatErrorMessage(error),
        code: this.extractErrorCode(error),
        type: this.isRateLimitError(error) ? "rate_limit" : "api_error",
      }
    };
  }
}

private async runNonStreamingCompletion(
  client: Anthropic, 
  params: CompletionParams
): Promise<CompletionResult> {
  // Convert params to Claude format
  const claudeParams = this.convertToClaudeFormat(params);
  
  // Make API call
  const response = await client.messages.create(claudeParams);
  
  // Convert response to standard format
  return {
    id: response.id,
    status: "completed",
    output: this.convertClaudeOutputToStandardFormat(response),
  };
}

private async runStreamingCompletion(
  client: Anthropic, 
  params: CompletionParams
): Promise<CompletionResult> {
  // Convert params to Claude format
  const claudeParams = this.convertToClaudeFormat(params);
  
  // Add streaming parameter
  claudeParams.stream = true;
  
  // Make streaming API call
  const stream = await client.messages.create(claudeParams);
  
  // Process stream events
  // This implementation will need to handle the stream events and
  // convert them to the standard completion result format
  
  // Example placeholder implementation
  let responseId = "";
  const outputItems: ResponseItem[] = [];
  
  try {
    for await (const chunk of stream) {
      responseId = chunk.id || responseId;
      
      // Process chunk content and add to output items
      if (chunk.type === "content_block_delta" && chunk.delta.text) {
        // Add text to output
      } else if (chunk.type === "tool_use") {
        // Handle tool use
      }
    }
  } catch (error) {
    // Handle streaming errors
  }
  
  return {
    id: responseId,
    status: "completed",
    output: outputItems,
  };
}
```

### Format Conversion Methods

```typescript
private convertToClaudeFormat(params: CompletionParams): any {
  // Convert standard parameters to Claude-specific format
  const claudeParams: any = {
    model: params.model,
    max_tokens: params.maxTokens || 4096,
    temperature: params.temperature,
    system: this.extractSystemMessage(params.messages),
    messages: this.convertMessagesToClaudeFormat(params.messages),
  };
  
  // Add tools if provided
  if (params.tools && params.tools.length > 0) {
    claudeParams.tools = this.formatTools(params.tools);
  }
  
  return claudeParams;
}

private extractSystemMessage(messages: Message[]): string {
  // Find system message and extract its content
  const systemMessage = messages.find(m => m.role === "system");
  if (systemMessage && typeof systemMessage.content === "string") {
    return systemMessage.content;
  }
  return "";
}

private convertMessagesToClaudeFormat(messages: Message[]): any[] {
  // Convert standard messages to Claude format
  // Skip system messages as they're handled separately
  return messages
    .filter(m => m.role !== "system")
    .map(m => {
      if (m.role === "tool") {
        // Convert tool messages to tool_result format
        return {
          role: "assistant",
          content: "",
          tool_results: [{
            tool_use_id: m.toolOutputs?.[0]?.tool_call_id,
            content: m.toolOutputs?.[0]?.output || "",
          }],
        };
      } else {
        // Convert user/assistant messages
        return {
          role: m.role === "user" ? "user" : "assistant",
          content: this.formatMessageContent(m.content),
        };
      }
    });
}

formatTools(tools: Tool[]): any[] {
  // Convert standard tools to Claude format
  return tools.map(tool => ({
    name: tool.name,
    description: tool.description || "",
    input_schema: tool.parameters,
  }));
}

parseToolCall(rawToolCall: any): ParsedToolCall {
  // Convert Claude tool call to standard format
  return {
    id: rawToolCall.id,
    name: rawToolCall.name,
    arguments: JSON.parse(rawToolCall.input || "{}"),
  };
}
```

### Error Handling

```typescript
isRateLimitError(error: any): boolean {
  // Check if error is a rate limit error
  return (
    error?.status === 429 ||
    error?.error?.type === "rate_limit_error" ||
    /rate limit/i.test(error?.message || "") ||
    /too many requests/i.test(error?.message || "")
  );
}

formatErrorMessage(error: any): string {
  // Extract and format error message for display
  if (error?.error?.message) {
    return `Claude API Error: ${error.error.message}`;
  } else if (error?.message) {
    return `Claude API Error: ${error.message}`;
  } else {
    return "Unknown error occurred when calling Claude API";
  }
}

private extractErrorCode(error: any): string | undefined {
  // Extract error code for classification
  return error?.error?.type || error?.type || undefined;
}
```

## Streaming Implementation Details

Claude's streaming implementation differs from OpenAI's in several ways. The following section outlines the approach for handling Claude's streaming events and mapping them to Codex's expected format.

### Event Types and Handling

Claude's stream emits different event types including:

1. `message_start`: Beginning of the message
2. `content_block_start`: Beginning of a content block
3. `content_block_delta`: Content increments
4. `content_block_stop`: End of a content block
5. `message_delta`: Message metadata updates
6. `message_stop`: End of the message
7. `tool_use`: Tool usage events

Example implementation:

```typescript
private async processStreamEvents(stream: any): Promise<{
  responseId: string;
  output: ResponseItem[];
}> {
  let responseId = "";
  const output: ResponseItem[] = [];
  let currentText = "";
  
  try {
    for await (const chunk of stream) {
      // Capture response ID
      if (chunk.message?.id) {
        responseId = chunk.message.id;
      }
      
      // Process based on event type
      switch (chunk.type) {
        case "content_block_delta":
          if (chunk.delta.text) {
            currentText += chunk.delta.text;
            // Create or update text output item
          }
          break;
          
        case "content_block_stop":
          // Finalize text block
          if (currentText) {
            output.push({
              type: "message",
              role: "assistant",
              content: [
                {
                  type: "input_text",
                  text: currentText,
                },
              ],
            });
            currentText = "";
          }
          break;
          
        case "tool_use":
          // Handle tool use events
          const toolCall = this.parseToolCall(chunk.tool_use);
          output.push({
            type: "function_call",
            id: toolCall.id,
            function: {
              name: toolCall.name,
              arguments: JSON.stringify(toolCall.arguments),
            },
          });
          break;
      }
    }
  } catch (error) {
    // Handle streaming errors
    console.error("Error processing Claude stream:", error);
  }
  
  return { responseId, output };
}
```

## Tool Calling Implementation

The implementation needs to handle differences between OpenAI's function calling and Claude's tool use:

1. **Format Conversion**: Map between Codex's tool definition format and Claude's tool schema
2. **Tool Call Parsing**: Convert Claude's tool_use events to Codex's expected function call format
3. **Tool Result Handling**: Map tool results back to Claude's expected format

```typescript
// Example of handling tool call in streaming context
private handleToolUse(toolUse: any): ResponseItem {
  return {
    type: "function_call",
    id: toolUse.id,
    function: {
      name: toolUse.name,
      arguments: JSON.stringify(toolUse.input || {}),
    },
  };
}

// Example of sending tool output back to Claude
private formatToolOutputForClaude(toolOutput: ToolOutput): any {
  return {
    tool_results: [{
      tool_use_id: toolOutput.tool_call_id,
      content: toolOutput.output,
    }],
  };
}
```

## Integration with Package.json

To support the Claude provider, the following dependencies need to be added to package.json:

```json
{
  "dependencies": {
    "@anthropic-ai/sdk": "^0.8.1"
  }
}
```

## Environment Variables

The Claude provider implementation will respect the following environment variables:

- `CLAUDE_API_KEY` or `ANTHROPIC_API_KEY`: API key for accessing Claude
- `CLAUDE_BASE_URL`: Optional custom endpoint URL
- `CLAUDE_TIMEOUT_MS`: Optional timeout value (defaults to 180000ms)

## Configuration File Integration

The Codex configuration file will need to be extended to support Claude-specific settings:

```yaml
# ~/.codex/config.yaml
defaultProvider: "openai"  # or "claude"

providerConfigs:
  openai:
    apiKey: "sk-..."  # Optional, can use environment variable
    
  claude:
    apiKey: "sk-ant-..."  # Optional, can use environment variable
    timeoutMs: 180000
```