# Claude Integration for Codex CLI

## Overview
Add support for Anthropic's Claude models to Codex CLI as an alternative to OpenAI models, using a provider-agnostic approach that maintains feature parity.

## Current Architecture
Codex currently uses OpenAI's API exclusively:

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

## Design for Claude Integration

### 1. Provider Abstraction Layer
Create a provider-agnostic interface for LLM interactions:

```typescript
// src/utils/providers/provider-interface.ts
export interface LLMProvider {
  id: string;
  name: string;
  getModels(): Promise<string[]>;
  isModelSupported(model: string): Promise<boolean>;
  createClient(config: AppConfig): any;
  runCompletion(params: CompletionParams): Promise<CompletionResult>;
  getModelDefaults(model: string): ModelDefaults;
}

export interface CompletionParams {
  model: string;
  messages: Message[];
  tools?: Tool[];
  temperature?: number;
  stream?: boolean;
  // Other common parameters
}

export interface CompletionResult {
  // Common response structure
}
```

### 2. Implement Provider-Specific Adapters

#### OpenAI Provider (existing functionality)
```typescript
// src/utils/providers/openai-provider.ts
import OpenAI from "openai";

export class OpenAIProvider implements LLMProvider {
  id = "openai";
  name = "OpenAI";
  
  async getModels(): Promise<string[]> {
    // Existing implementation from model-utils.ts
  }
  
  async isModelSupported(model: string): Promise<boolean> {
    // Existing implementation
  }
  
  createClient(config: AppConfig) {
    return new OpenAI({
      apiKey: config.openaiApiKey,
      baseURL: config.openaiBaseUrl,
      timeout: config.openaiTimeoutMs,
    });
  }
  
  async runCompletion(params: CompletionParams): Promise<CompletionResult> {
    // Existing implementation from agent-loop.ts
  }
  
  getModelDefaults(model: string): ModelDefaults {
    // Return appropriate defaults for OpenAI models
  }
}
```

#### Claude Provider (new)
```typescript
// src/utils/providers/claude-provider.ts
import Anthropic from "@anthropic-ai/sdk";

export class ClaudeProvider implements LLMProvider {
  id = "claude";
  name = "Anthropic Claude";
  
  async getModels(): Promise<string[]> {
    // Return available Claude models
    return [
      "claude-3-5-sonnet-20240620",
      "claude-3-opus-20240229", 
      "claude-3-sonnet-20240229",
      "claude-3-haiku-20240307"
    ];
  }
  
  async isModelSupported(model: string): Promise<boolean> {
    const models = await this.getModels();
    return models.includes(model);
  }
  
  createClient(config: AppConfig) {
    return new Anthropic({
      apiKey: config.claudeApiKey,
      baseURL: config.claudeBaseUrl || undefined,
    });
  }
  
  async runCompletion(params: CompletionParams): Promise<CompletionResult> {
    // Implement Claude Messages API equivalent to OpenAI chat completions
    // Map between Codex format and Claude format
    
    // PRIORITY IMPROVEMENTS:
    // 1. Handle 400 errors gracefully with better error messages
    // 2. Implement proper backoff/retry mechanism for rate limits
    // 3. Optimize token usage through better prompt engineering
    // 4. Fix streaming implementation to reduce latency
    // 5. Add specific error handling for Claude-specific error types
  }
  
  getModelDefaults(model: string): ModelDefaults {
    // Return appropriate defaults for Claude models
    // Include higher timeout values for Claude models
    return {
      timeoutMs: 180000, // 3 minutes instead of default
      // Other Claude-specific optimizations
    };
  }
}
```

### 3. Provider Registration and Selection

```typescript
// src/utils/providers/index.ts
import { OpenAIProvider } from "./openai-provider";
import { ClaudeProvider } from "./claude-provider";

const providers: Record<string, LLMProvider> = {
  openai: new OpenAIProvider(),
  claude: new ClaudeProvider(),
};

export function getProvider(model: string): LLMProvider {
  // Detect provider from model name
  if (model.startsWith("claude")) {
    return providers.claude;
  }
  return providers.openai; // Default
}

export function getAvailableProviders(): LLMProvider[] {
  return Object.values(providers);
}
```

### 4. Configuration Updates

```typescript
// src/utils/config.ts
export interface AppConfig {
  // Existing OpenAI config
  openaiApiKey: string;
  openaiBaseUrl: string;
  openaiTimeoutMs: number;
  
  // New Claude config
  claudeApiKey: string;
  claudeBaseUrl?: string;
  claudeTimeoutMs?: number; // Claude-specific timeout
  
  // Provider selection (optional, can be derived from model)
  preferredProvider?: string;
}
```

Environment variables:
- `CLAUDE_API_KEY` - API key for Claude
- `CLAUDE_BASE_URL` - Optional custom endpoint
- `CLAUDE_TIMEOUT_MS` - Custom timeout for Claude (defaults to 180000ms)

### 5. Agent Loop Refactoring

Update `agent-loop.ts` to use the provider abstraction:

```typescript
// src/utils/agent/agent-loop.ts
import { getProvider } from "../providers";

export class AgentLoop {
  private provider: LLMProvider;
  private client: any;
  
  constructor(params: AgentLoopParams) {
    // Get provider for the selected model
    this.provider = getProvider(params.model);
    
    // Create appropriate client
    this.client = this.provider.createClient(params.config);
    
    // Rest of initialization
  }
  
  async run(input: string) {
    // Use provider.runCompletion instead of direct OpenAI calls
    
    // PRIORITY IMPROVEMENTS:
    // 1. Better error handling for Claude's specific error patterns
    // 2. Implement proper retry mechanisms with exponential backoff
    // 3. Add timeout handling specific to Claude's longer processing times
    // 4. Add proper cancellation handling for Claude requests
  }
}
```

### 6. Tool/Function Calling Mappings

Create mappings between OpenAI and Claude function/tool calling formats:

```typescript
// src/utils/providers/tool-mappings.ts
export function mapOpenAIToolsToClaudeTools(tools) {
  // Convert OpenAI tool format to Claude tool format
}

export function mapClaudeToolCallToOpenAIFormat(toolCall) {
  // Convert Claude tool call format to OpenAI format
}
```

### 7. Model Selection UI Updates

Update model selection overlay to show providers:

```tsx
// src/components/model-overlay.tsx
const MODEL_GROUPS = {
  "OpenAI": ["gpt-4", "gpt-3.5-turbo", "o4-mini", "o3"],
  "Claude": ["claude-3-opus-20240229", "claude-3-sonnet-20240229", "claude-3-haiku-20240307"],
};
```

## Implementation Strategy

1. **Phase 1 - Provider Abstraction**:
   - Refactor existing OpenAI code into provider pattern
   - Ensure all tests pass with this abstraction

2. **Phase 2 - Claude Integration**:
   - Add Anthropic SDK dependency
   - Implement Claude provider adapter
   - Add configuration options
   - FOCUS ON ERROR HANDLING, PERFORMANCE, AND RELIABILITY

3. **Phase 3 - UI and Documentation**:
   - Update model selection UI
   - Update documentation
   - Add examples for Claude-specific features

## Technical Considerations

### Dependencies
- Add `@anthropic-ai/sdk` for Claude API access
- Ensure type compatibility between providers

### Critical Performance Fixes
1. **400 Error Handling**: Improve detection and messaging for Claude API rejections
2. **Rate Limiting**: Implement Claude-specific rate limit handling with proper retries
3. **Streaming Optimization**: Fix streaming implementation to reduce latency
4. **Token Optimization**: Adjust context handling to optimize for Claude's token usage
5. **Timeout Handling**: Increase default timeouts for Claude models and add better timeout monitoring

### API Compatibility
The primary differences requiring adaptation:
1. Authentication mechanism (identical - API key in header)
2. Function/tool calling format differences
3. Response formatting differences
4. Streaming protocol differences
5. Error handling and rate limiting patterns

## Testing Plan
1. Unit tests for Claude provider adapter
2. Integration tests with mock API responses
3. End-to-end tests with actual API calls
4. Stress tests for:
   - Rate limit handling
   - Error recovery
   - Long-running operations
   - Large context handling

## Success Criteria
1. Users can seamlessly use Claude models in Codex
2. All existing functionality works with either provider
3. Claude integration matches or exceeds OpenAI reliability
4. Error handling is consistent and informative
5. Performance is comparable between providers