# Provider Interface Design for Multi-Provider Support

## Overview

This document outlines the detailed design for the provider abstraction layer that will enable Codex CLI to support multiple LLM providers. The provider interface serves as the foundation for all LLM interactions, allowing the application to work seamlessly with different model providers.

## Core Interface Design

### `LLMProvider` Interface

```typescript
export interface LLMProvider {
  // Provider identification
  id: string;
  name: string;
  
  // Model management
  getModels(): Promise<string[]>;
  isModelSupported(model: string): Promise<boolean>;
  
  // Client operations
  createClient(config: AppConfig): any;
  runCompletion(params: CompletionParams): Promise<CompletionResult>;
  
  // Configuration
  getModelDefaults(model: string): ModelDefaults;
  
  // Error handling
  isRateLimitError(error: any): boolean;
  formatErrorMessage(error: any): string;
  
  // Tool calls
  parseToolCall(rawToolCall: any): ParsedToolCall;
  formatTools(tools: Tool[]): any;
}
```

### Provider Registry

```typescript
export class ProviderRegistry {
  private static providers: Map<string, LLMProvider> = new Map();
  
  static register(provider: LLMProvider): void;
  static getProviderById(id: string): LLMProvider | undefined;
  static getProviderForModel(model: string): LLMProvider;
  static getDefaultProvider(): LLMProvider;
  static getAllProviders(): LLMProvider[];
}
```

## Data Structures

### Completion Parameters

```typescript
export interface CompletionParams {
  model: string;
  messages: Message[];
  tools?: Tool[];
  temperature?: number;
  stream?: boolean;
  previousResponseId?: string;
  maxTokens?: number;
  parallelToolCalls?: boolean;
  reasoning?: ReasoningSettings;
}
```

### Completion Result

```typescript
export interface CompletionResult {
  id: string;
  output: ResponseItem[];
  status: "completed" | "incomplete" | "error";
  errorDetails?: {
    message: string;
    code?: string;
    type?: string;
  };
}
```

### Message Structure

```typescript
export interface Message {
  role: "system" | "user" | "assistant" | "function" | "tool";
  content: string | MessageContent[];
  name?: string;
  toolOutputs?: ToolOutput[];
}

export interface MessageContent {
  type: "text" | "image_url";
  text?: string;
  image_url?: {
    url: string;
    detail?: "low" | "high" | "auto";
  };
}
```

### Tool Definitions

```typescript
export interface Tool {
  type: "function";
  name: string;
  description?: string;
  parameters: object;
  strict?: boolean;
}

export interface ParsedToolCall {
  id: string;
  name: string;
  arguments: any;
}

export interface ToolOutput {
  tool_call_id: string;
  output: string;
}
```

### Model Defaults

```typescript
export interface ModelDefaults {
  timeoutMs: number;
  temperature?: number;
  maxTokens?: number;
  supportsToolCalls: boolean;
  supportsStreaming: boolean;
  contextWindowSize: number;
}
```

## Implementation Strategy

1. **Base Provider Class**:
   Create an abstract base class that implements common functionality across providers.

   ```typescript
   export abstract class BaseProvider implements LLMProvider {
     // Abstract methods that must be implemented by concrete providers
     abstract id: string;
     abstract name: string;
     abstract getModels(): Promise<string[]>;
     abstract createClient(config: AppConfig): any;
     abstract runCompletion(params: CompletionParams): Promise<CompletionResult>;
     
     // Methods with default implementations that can be overridden
     async isModelSupported(model: string): Promise<boolean> {
       const models = await this.getModels();
       return models.includes(model);
     }
     
     // Other default implementations...
   }
   ```

2. **Provider-Specific Implementations**:
   Create concrete implementations for each supported provider.

   ```typescript
   export class OpenAIProvider extends BaseProvider {
     id = "openai";
     name = "OpenAI";
     
     // Implementation of required methods...
   }
   
   export class ClaudeProvider extends BaseProvider {
     id = "claude";
     name = "Anthropic Claude";
     
     // Implementation of required methods...
   }
   ```

## Error Handling Strategy

1. **Standardized Error Types**:
   Define a common set of error types that all providers map their specific errors to.

   ```typescript
   export enum ProviderErrorType {
     AUTHENTICATION = "authentication_error",
     RATE_LIMIT = "rate_limit_error",
     CONTEXT_LENGTH = "context_length_error",
     INVALID_REQUEST = "invalid_request_error",
     SERVER = "server_error",
     NETWORK = "network_error",
     TIMEOUT = "timeout_error",
     UNKNOWN = "unknown_error",
   }
   
   export interface StandardizedError {
     type: ProviderErrorType;
     message: string;
     originalError: any;
     retryable: boolean;
     suggestedWaitMs?: number;
   }
   ```

2. **Provider-Specific Error Mapping**:
   Each provider implements logic to map their API's error formats to the standardized types.

## Configuration Integration

1. **Extended AppConfig Interface**:
   Modify the existing AppConfig to include provider-specific configurations.

   ```typescript
   export interface AppConfig {
     // Existing fields...
     
     // Provider-specific configs
     providerConfigs: {
       openai?: {
         apiKey?: string;
         baseUrl?: string;
         timeoutMs?: number;
       };
       claude?: {
         apiKey?: string;
         baseUrl?: string;
         timeoutMs?: number;
       };
     };
     
     defaultProvider?: string;
   }
   ```

2. **Environment Variable Mapping**:
   Map environment variables to the appropriate provider configurations.

## Testing Guidelines

1. **Mock Provider Implementation**:
   Create a MockProvider implementation for testing that doesn't require real API calls.

   ```typescript
   export class MockProvider extends BaseProvider {
     id = "mock";
     name = "Mock Provider";
     
     // Simulated implementations that return predictable results
     async getModels(): Promise<string[]> {
       return ["mock-model-1", "mock-model-2"];
     }
     
     // Other mock implementations...
   }
   ```

2. **Provider Interface Unit Tests**:
   Create comprehensive unit tests to verify all aspects of the provider interface.

3. **Integration Testing Guidelines**:
   Develop testing strategies for verifying cross-provider functionality.

## Security Considerations

1. **API Key Management**:
   Ensure secure handling of API keys for all providers.

2. **Request/Response Sanitization**:
   Implement proper sanitization of inputs and outputs across all providers.

3. **Rate Limit Handling**:
   Ensure proper handling of rate limits to prevent abuse or excessive costs.

## Performance Considerations

1. **Streaming Optimization**:
   Ensure efficient handling of streaming responses for all providers.

2. **Token Optimization**:
   Implement provider-specific optimizations for token usage.

3. **Caching Strategy**:
   Define caching strategies for model lists and other reusable data.

## Integration with Agent Loop

The integration with the existing agent loop will require:

1. **Provider Selection Logic**:
   Update the agent loop to select the appropriate provider based on the model name.

2. **Provider-Agnostic Stream Handling**:
   Modify the stream handling to work with all provider streaming formats.

3. **Unified Error Handling**:
   Implement a provider-agnostic error handling approach.

```typescript
// Simplified example of agent loop integration
export class AgentLoop {
  private provider: LLMProvider;
  private client: any;
  
  constructor(params: AgentLoopParams) {
    // Get provider for the selected model
    this.provider = ProviderRegistry.getProviderForModel(params.model);
    
    // Create appropriate client
    this.client = this.provider.createClient(params.config);
    
    // Rest of initialization
  }
  
  async run(input: string) {
    try {
      // Use provider.runCompletion instead of direct OpenAI calls
      const result = await this.provider.runCompletion({
        model: this.model,
        messages: this.constructMessages(input),
        tools: this.tools,
        stream: true,
        // Other parameters...
      });
      
      // Process result
    } catch (error) {
      // Handle errors in a provider-agnostic way
      const errorMessage = this.provider.formatErrorMessage(error);
      // Display error to user
    }
  }
}
```