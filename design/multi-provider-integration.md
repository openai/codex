# Multi-Provider LLM Integration for Codex CLI

## Overview
Refactor Codex CLI to support multiple LLM providers beyond OpenAI, creating a provider-agnostic architecture that allows seamless switching between different model providers (OpenAI, Anthropic Claude, etc.).

## Current Architecture
Codex currently has OpenAI-specific implementation:

1. **Model Selection**: 
   - Uses OpenAI-specific model names in `model-utils.ts`
   - Fetches available models from OpenAI API
   - Recommended models are hardcoded (`o4-mini`, `o3`)

2. **API Client**:
   - Direct dependency on `openai` npm package
   - OpenAI client initialization in `agent-loop.ts`
   - OpenAI-specific response types and parsing

3. **Configuration**:
   - `OPENAI_API_KEY` environment variable
   - `OPENAI_BASE_URL` for API endpoint
   - Model selection through command line args/config

## Design for Multi-Provider Support

### 1. Provider Abstraction Layer
Create a provider-agnostic interface for LLM interactions:

```typescript
// src/utils/llm/provider-interface.ts
export interface LLMProvider {
  id: string;
  name: string;
  getModels(): Promise<string[]>;
  isModelSupported(model: string): Promise<boolean>;
  createClient(config: AppConfig): any;
  runCompletion(params: CompletionParams): Promise<CompletionResult>;
  getModelDefaults(model: string): ModelDefaults;
  parseToolCall(rawToolCall: any): ParsedToolCall;
}

export interface CompletionParams {
  model: string;
  messages: Message[];
  tools?: Tool[];
  temperature?: number;
  stream?: boolean;
  // Other common parameters
}
```

### 2. Provider Implementation Base Class

```typescript
// src/utils/llm/base-provider.ts
export abstract class BaseProvider implements LLMProvider {
  abstract id: string;
  abstract name: string;
  
  abstract getModels(): Promise<string[]>;
  abstract createClient(config: AppConfig): any;
  abstract runCompletion(params: CompletionParams): Promise<CompletionResult>;
  
  // Common functionality shared by all providers
  async isModelSupported(model: string): Promise<boolean> {
    const models = await this.getModels();
    return models.includes(model);
  }
}
```

### 3. Provider Registry and Factory

```typescript
// src/utils/llm/provider-registry.ts
export class ProviderRegistry {
  private static providers: Map<string, LLMProvider> = new Map();
  
  static register(provider: LLMProvider) {
    this.providers.set(provider.id, provider);
  }
  
  static getProvider(id: string): LLMProvider | undefined {
    return this.providers.get(id);
  }
  
  static getProviderForModel(model: string): LLMProvider {
    // Detect provider from model name pattern
    if (model.startsWith("claude")) {
      return this.getProvider("anthropic") || this.getDefaultProvider();
    }
    // Add more provider detection as needed
    
    return this.getDefaultProvider();
  }
  
  static getDefaultProvider(): LLMProvider {
    return this.getProvider("openai") || 
      Array.from(this.providers.values())[0];
  }
  
  static getAllProviders(): LLMProvider[] {
    return Array.from(this.providers.values());
  }
}
```

### 4. Individual Provider Implementations

#### OpenAI Provider
```typescript
// src/utils/llm/providers/openai-provider.ts
export class OpenAIProvider extends BaseProvider {
  id = "openai";
  name = "OpenAI";
  
  async getModels(): Promise<string[]> {
    // Implementation
  }
  
  createClient(config: AppConfig) {
    return new OpenAI({
      apiKey: config.openaiApiKey,
      baseURL: config.openaiBaseUrl,
      timeout: config.openaiTimeoutMs,
    });
  }
  
  async runCompletion(params: CompletionParams): Promise<CompletionResult> {
    // Implementation
  }
}

// Register the provider
ProviderRegistry.register(new OpenAIProvider());
```

#### Claude Provider
```typescript
// src/utils/llm/providers/claude-provider.ts
export class ClaudeProvider extends BaseProvider {
  id = "anthropic";
  name = "Anthropic Claude";
  
  async getModels(): Promise<string[]> {
    return [
      "claude-3-5-sonnet-20240620",
      "claude-3-opus-20240229", 
      "claude-3-sonnet-20240229",
      "claude-3-haiku-20240307"
    ];
  }
  
  createClient(config: AppConfig) {
    return new Anthropic({
      apiKey: config.anthropicApiKey,
      baseURL: config.anthropicBaseUrl || undefined,
    });
  }
  
  async runCompletion(params: CompletionParams): Promise<CompletionResult> {
    // Implementation
  }
}

// Register the provider
ProviderRegistry.register(new ClaudeProvider());
```

### 5. Extended Configuration

```typescript
// src/utils/config.ts
export interface LLMProviderConfig {
  apiKey?: string;
  baseUrl?: string;
  timeoutMs?: number;
  defaultModel?: string;
  additionalOptions?: Record<string, any>;
}

export interface AppConfig {
  // General LLM config
  defaultProvider: string;
  
  // Provider-specific configurations
  providerConfigs: Record<string, LLMProviderConfig>;
}
```

### 6. Environment Variable Handling

```typescript
// src/utils/config-loader.ts
export function loadEnvConfig(): AppConfig {
  // Load from environment variables with consistent pattern
  
  // General config
  const defaultProvider = process.env.CODEX_DEFAULT_PROVIDER || "openai";
  
  // OpenAI
  const openaiConfig: LLMProviderConfig = {
    apiKey: process.env.OPENAI_API_KEY,
    baseUrl: process.env.OPENAI_BASE_URL,
    timeoutMs: parseInt(process.env.OPENAI_TIMEOUT_MS || "60000"),
    defaultModel: process.env.OPENAI_DEFAULT_MODEL || "o4-mini",
  };
  
  // Anthropic
  const anthropicConfig: LLMProviderConfig = {
    apiKey: process.env.ANTHROPIC_API_KEY || process.env.CLAUDE_API_KEY,
    baseUrl: process.env.ANTHROPIC_BASE_URL || process.env.CLAUDE_BASE_URL,
    timeoutMs: parseInt(process.env.ANTHROPIC_TIMEOUT_MS || "120000"),
    defaultModel: process.env.ANTHROPIC_DEFAULT_MODEL || "claude-3-sonnet-20240229",
  };
  
  return {
    defaultProvider,
    providerConfigs: {
      openai: openaiConfig,
      anthropic: anthropicConfig,
      // Additional providers...
    }
  };
}
```

### 7. Updated Agent Loop

```typescript
// src/utils/agent/agent-loop.ts
import { ProviderRegistry } from "../llm/provider-registry";

export class AgentLoop {
  private provider: LLMProvider;
  private client: any;
  
  constructor(params: AgentLoopParams) {
    // Get provider based on model or explicit provider selection
    this.provider = params.provider || 
      ProviderRegistry.getProviderForModel(params.model);
    
    // Create client for selected provider
    this.client = this.provider.createClient(params.config);
    
    // Rest of initialization
  }
  
  // Rest of implementation using provider abstraction
}
```

### 8. Model Selection UI Updates

```tsx
// src/components/model-overlay.tsx
function ModelSelector() {
  const providers = ProviderRegistry.getAllProviders();
  
  // Group models by provider
  const modelGroups = useMemo(() => {
    const groups: Record<string, string[]> = {};
    
    for (const provider of providers) {
      groups[provider.name] = provider.getModels();
    }
    
    return groups;
  }, [providers]);
  
  // Render provider selection and model selection
}
```

## Extension Points for Future Providers

The design allows for easy addition of new providers:

1. **Provider-Specific Tool Mapping**:
   ```typescript
   // For each provider, implement transformations
   interface ToolMapper {
     toProviderFormat(tools: Tool[]): any;
     fromProviderFormat(result: any): ParsedToolCall[];
   }
   ```

2. **Streaming Protocol Adapters**:
   ```typescript
   // Adapter for provider-specific streaming
   interface StreamAdapter {
     adaptStream(nativeStream: any): ReadableStream<CompletionChunk>;
   }
   ```

3. **Error Handling**:
   ```typescript
   // Provider-specific error handling
   interface ErrorHandler {
     mapError(error: any): StandardError;
     isRetryable(error: any): boolean;
   }
   ```

## Implementation Roadmap

### Phase 1: Abstraction Layer
1. Create provider interface and base classes
2. Refactor OpenAI implementation to use the new abstraction
3. Ensure all tests pass with the refactored implementation

### Phase 2: Claude Implementation
1. Add Anthropic SDK dependency
2. Implement Claude provider adapter
3. Add configuration options for Claude
4. Test basic functionality

### Phase 3: UI and Configuration
1. Update model selection UI for multi-provider support
2. Enhance configuration handling
3. Add provider-specific settings

### Phase 4: Additional Providers (Future)
1. Template for adding new providers
2. Documentation for provider implementation

## Technical Considerations

### API Differences
Different providers have variations in:
1. Authentication methods
2. Rate limiting and error reporting
3. Tool/function calling formats
4. Response formats and capabilities
5. Streaming implementation

### Testing Strategy
1. Unit tests for each provider adapter
2. Mock API responses for testing
3. Integration tests for provider switching
4. Test matrix of features across providers

## Success Criteria
1. Users can seamlessly switch between different model providers
2. All core Codex functionality works with any provider
3. Configuration is intuitive and consistent
4. Adding new providers is straightforward
5. Performance impact of abstraction layer is minimal