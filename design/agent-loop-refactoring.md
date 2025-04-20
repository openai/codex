# Agent Loop Refactoring for Multi-Provider Support

## Overview

This document outlines the refactoring needed in the `agent-loop.ts` file to support multiple LLM providers. The current implementation is tightly coupled with the OpenAI API, and needs to be modified to work with the new provider abstraction layer.

## Current Architecture

The current `AgentLoop` class has the following OpenAI-specific elements:

1. Direct OpenAI client instantiation
2. OpenAI-specific request construction
3. OpenAI-specific error handling
4. OpenAI-specific response parsing
5. OpenAI-specific streaming implementation

## Refactoring Strategy

### 1. Provider Selection

Replace direct OpenAI client instantiation with provider-based initialization:

```typescript
// Before:
this.oai = new OpenAI({
  apiKey: OPENAI_API_KEY,
  baseURL: OPENAI_BASE_URL,
  timeout: OPENAI_TIMEOUT_MS,
});

// After:
this.provider = ProviderRegistry.getProviderForModel(this.model);
this.client = this.provider.createClient(this.config);
```

### 2. Completion Request

Replace OpenAI-specific request with provider-agnostic approach:

```typescript
// Before:
stream = await this.oai.responses.create({
  model: this.model,
  instructions: mergedInstructions,
  previous_response_id: lastResponseId || undefined,
  input: turnInput,
  stream: true,
  parallel_tool_calls: false,
  reasoning,
  tools: [
    {
      type: "function",
      name: "shell",
      description: "Runs a shell command, and returns its output.",
      strict: false,
      parameters: {
        // ...
      },
    },
  ],
});

// After:
stream = await this.provider.runCompletion({
  model: this.model,
  instructions: mergedInstructions,
  previousResponseId: lastResponseId || undefined,
  messages: this.convertInputToMessages(turnInput),
  stream: true,
  parallelToolCalls: false,
  reasoning,
  tools: this.getTools(),
  config: this.config,
});
```

### 3. Error Handling

Update error handling to work with provider-agnostic error formats:

```typescript
// Before:
const isTimeout = error instanceof APIConnectionTimeoutError;
const ApiConnErrCtor = (OpenAI as any).APIConnectionError;
const isConnectionError = ApiConnErrCtor ? error instanceof ApiConnErrCtor : false;

// After:
const isTimeout = this.provider.isTimeoutError(error);
const isConnectionError = this.provider.isConnectionError(error);
```

### 4. Standardized Error Messages

Use provider-specific error message formatting:

```typescript
// Before:
const errorDetails = [
  `Status: ${status || "unknown"}`,
  `Code: ${errCtx.code || "unknown"}`,
  `Type: ${errCtx.type || "unknown"}`,
  `Message: ${errCtx.message || "unknown"}`,
].join(", ");

// After:
const errorDetails = this.provider.formatErrorMessage(error);
```

### 5. Response Handling

Adapt response handling to work with provider-agnostic structures:

```typescript
// Before:
for await (const event of stream) {
  if (event.type === "response.output_item.done") {
    const item = event.item;
    // Process item...
  }
  
  if (event.type === "response.completed") {
    // Process completion...
  }
}

// After:
for await (const event of stream) {
  // Process using provider-specific event dispatcher
  this.handleStreamEvent(event);
}
```

## Updated Class Structure

```typescript
export class AgentLoop {
  private provider: LLMProvider;
  private client: any;
  // Other properties...
  
  constructor(params: AgentLoopParams) {
    // Initialize
    this.model = params.model;
    this.instructions = params.instructions;
    this.approvalPolicy = params.approvalPolicy;
    this.config = params.config || this.createDefaultConfig();
    
    // Get provider for model
    this.provider = ProviderRegistry.getProviderForModel(this.model);
    
    // Get provider-specific defaults
    const modelDefaults = this.provider.getModelDefaults(this.model);
    
    // Apply model defaults to config if needed
    this.config = {
      ...this.config,
      timeoutMs: this.config.timeoutMs || modelDefaults.timeoutMs,
    };
    
    // Create client
    this.client = this.provider.createClient(this.config);
    
    // Other initialization...
  }
  
  // Run method with provider abstraction
  public async run(
    input: Array<ResponseInputItem>,
    previousResponseId: string = "",
  ): Promise<void> {
    try {
      // Set up generation and cancellation
      // ...
      
      // Prepare input
      let turnInput = [...abortOutputs, ...input];
      
      this.onLoading(true);
      
      while (turnInput.length > 0) {
        if (this.canceled || this.hardAbort.signal.aborted) {
          this.onLoading(false);
          return;
        }
        
        // Emit input items
        for (const item of turnInput) {
          this.stageItem(item as ResponseItem);
        }
        
        // Execute completion with provider
        try {
          const result = await this.provider.runCompletion({
            model: this.model,
            messages: this.convertInputToMessages(turnInput),
            tools: this.getTools(),
            stream: true,
            previousResponseId: previousResponseId || undefined,
            temperature: this.config.temperature,
            // Other parameters...
          });
          
          // Process result
          // ...
          
          // Update response ID
          lastResponseId = result.id;
          this.onLastResponseId(result.id);
          
        } catch (error) {
          // Provider-agnostic error handling
          this.handleProviderError(error);
        }
        
        turnInput = []; // Clear for next cycle
      }
      
      // Completion successful - cleanup
      // ...
      
    } catch (error) {
      // Top-level error handler
      // ...
    }
  }
  
  // Provider-agnostic error handling
  private handleProviderError(error: any): void {
    if (this.provider.isRateLimitError(error)) {
      // Handle rate limit errors
      const retryAfterMs = this.provider.getRetryAfterMs(error);
      // ...
    } else if (this.provider.isTimeoutError(error)) {
      // Handle timeout errors
      // ...
    } else if (this.provider.isInvalidRequestError(error)) {
      // Handle invalid request errors
      // ...
    } else if (this.provider.isContextLengthError(error)) {
      // Handle context length errors
      // ...
    } else {
      // Generic error handling
      const errorMessage = this.provider.formatErrorMessage(error);
      this.onItem({
        id: `error-${Date.now()}`,
        type: "message",
        role: "system",
        content: [
          {
            type: "input_text",
            text: `⚠️  ${errorMessage}`,
          },
        ],
      });
    }
    
    this.onLoading(false);
  }
  
  // Helper methods for provider abstraction
  private convertInputToMessages(input: Array<ResponseInputItem>): Array<Message> {
    // Convert Codex input format to provider message format
    // ...
  }
  
  private getTools(): Array<Tool> {
    // Get tools in provider-compatible format
    return [
      {
        type: "function",
        name: "shell",
        description: "Runs a shell command, and returns its output.",
        parameters: {
          type: "object",
          properties: {
            command: { type: "array", items: { type: "string" } },
            workdir: { type: "string" },
            timeout: { type: "number" },
          },
          required: ["command"],
        },
      },
    ];
  }
}
```

## Changes to Function Call Handling

The `handleFunctionCall` method needs to be updated to handle tool calls from different providers:

```typescript
private async handleFunctionCall(
  item: ResponseFunctionToolCall,
): Promise<Array<ResponseInputItem>> {
  // Exit early if canceled
  if (this.canceled) {
    return [];
  }
  
  // Parse provider-specific tool call format
  const parsedToolCall = this.provider.parseToolCall(item);
  
  const callId = parsedToolCall.id;
  const name = parsedToolCall.name;
  const args = parsedToolCall.arguments;
  
  // Create base output item
  const outputItem: ResponseInputItem.FunctionCallOutput = {
    type: "function_call_output",
    call_id: callId,
    output: "no function found",
  };
  
  // Process function/tool call using existing logic
  // ...
  
  return [outputItem, ...additionalItems];
}
```

## Stream Processing Updates

The stream processing logic needs to be generalized to handle different provider streaming formats:

```typescript
private async processStream(stream: any): Promise<void> {
  try {
    // Use provider-specific stream processing
    for await (const event of stream) {
      // Different providers emit different event types
      // Use provider to normalize events
      const normalizedEvent = this.provider.normalizeStreamEvent(event);
      
      switch (normalizedEvent.type) {
        case "text":
          // Handle text
          break;
          
        case "tool_call":
          // Handle tool call
          break;
          
        case "completion":
          // Handle completion
          break;
          
        // Other event types...
      }
    }
  } catch (error) {
    // Handle streaming errors
    if (error instanceof AbortError && this.canceled) {
      // Graceful cancellation
      return;
    }
    throw error;
  }
}
```

## Configuration Changes

The constructor needs to work with expanded configuration options:

```typescript
constructor({
  model,
  instructions,
  approvalPolicy,
  config,
  onItem,
  onLoading,
  getCommandConfirmation,
  onLastResponseId,
  additionalWritableRoots,
}: AgentLoopParams) {
  this.model = model;
  this.instructions = instructions;
  this.approvalPolicy = approvalPolicy;
  
  // Enhanced config handling
  this.config = config ?? this.createDefaultConfig(model);
  
  // Get provider
  this.provider = ProviderRegistry.getProviderForModel(model);
  
  // Get provider-specific defaults
  const modelDefaults = this.provider.getModelDefaults(model);
  
  // Apply model defaults to config if needed
  this.config = {
    ...this.config,
    timeoutMs: this.config.timeoutMs || modelDefaults.timeoutMs,
  };
  
  // Create provider-specific client
  this.client = this.provider.createClient(this.config);
  
  // Other initialization...
}

private createDefaultConfig(model: string): AppConfig {
  // Create minimal config with model-specific defaults
  const provider = ProviderRegistry.getProviderForModel(model);
  const modelDefaults = provider.getModelDefaults(model);
  
  return {
    model,
    instructions: this.instructions ?? "",
    timeoutMs: modelDefaults.timeoutMs,
    // Other defaults...
  };
}
```

## Testing Strategy

1. **Unit Tests**:
   - Create tests for provider-agnostic AgentLoop behavior
   - Test error handling with various provider error types
   - Test completion request construction
   - Test tool call handling

2. **Integration Tests**:
   - Test full run cycles with mock providers
   - Test streaming behavior
   - Test cancellation
   - Test recovery from errors

3. **Provider-Specific Tests**:
   - Test with actual OpenAI provider
   - Test with actual Claude provider
   - Compare results between providers

## Transition Plan

1. **Phase 1**: Extract provider interface and create OpenAI provider implementation
2. **Phase 2**: Update AgentLoop to use provider abstraction with OpenAI provider
3. **Phase 3**: Add Claude provider implementation
4. **Phase 4**: Test and optimize AgentLoop with both providers