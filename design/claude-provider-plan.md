# Multi-Provider Implementation Plan with Claude Support

## Overview

This document outlines the prioritized implementation plan for adding multi-provider support to Codex CLI with specific emphasis on integrating Claude models (GitHub issue #11). The implementation will occur in well-defined milestones, starting with a provider-agnostic architecture and then implementing Claude support.

## Current Situation

Currently, Codex CLI is tightly coupled to OpenAI's API:
- Direct dependency on OpenAI's client library
- Hardcoded OpenAI model names and API behaviors
- OpenAI-specific error handling and response processing
- No abstraction layer to support alternative LLM providers

## Implementation Milestones

### Milestone 1: Provider Abstraction Layer (Week 1-2)

1. **Design Provider Interface**
   - Create provider interface with required methods (getModels, createClient, runCompletion)
   - Define common completion parameter and response types
   - Design error handling abstractions

2. **Refactor OpenAI Implementation**
   - Extract OpenAI-specific code into an OpenAIProvider class
   - Implement the provider interface for OpenAI
   - Ensure all existing functionality works with the abstraction

3. **Provider Registry**
   - Create a provider registry to manage multiple providers
   - Implement provider selection logic
   - Add configuration support for multiple providers

4. **Testing Infrastructure**
   - Create mock provider for testing
   - Update existing tests to work with provider abstraction
   - Add tests for provider switching

### Milestone 2: Claude Integration (Week 3-4)

1. **Initial Claude Implementation**
   - Add Anthropic SDK dependency
   - Create basic ClaudeProvider implementation 
   - Map basic operations to Claude's API
   - Implement authentication and configuration

2. **Tool/Function Calling**
   - Create mappings between OpenAI and Claude tool calling formats
   - Implement Claude-specific tool handling
   - Ensure compatibility with existing shell command execution

3. **Streaming Implementation**
   - Implement streaming support for Claude
   - Ensure proper event parsing for Claude's formats
   - Add reliable stream initialization and cancellation

4. **Error Handling**
   - Implement Claude-specific error detection
   - Create clear error messages for common issues
   - Add appropriate retry logic for rate limits and transient failures

### Milestone 3: Performance Optimization (Week 5)

1. **Performance Tuning**
   - Optimize token usage for Claude models
   - Implement provider-specific timeout handling
   - Address any latency issues in streaming implementation

2. **Message Mapping Optimization**
   - Refine message formatting for Claude
   - Optimize context handling for Claude's token patterns
   - Implement efficient message history management

3. **Configuration Fine-tuning**
   - Add Claude-specific default settings
   - Implement adaptive parameters based on model selection
   - Create model capability detection

### Milestone 4: UI and Documentation (Week 6)

1. **User Interface Updates**
   - Add provider selection to model overlay
   - Update model groups to include Claude models
   - Add provider-specific settings UI

2. **Documentation**
   - Update user documentation with multi-provider support
   - Add Claude-specific setup instructions
   - Document provider-specific behaviors and limitations

3. **Comprehensive Testing**
   - End-to-end testing with Claude models
   - Cross-provider compatibility tests
   - Performance benchmarking between providers

## Implementation Details

### Provider Interface

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
  id: string;
  output: ResponseItem[];
  status: string;
}
```

### Configuration Updates

```typescript
// src/utils/config.ts - additions
export interface ProviderConfig {
  apiKey?: string;
  baseUrl?: string;
  timeoutMs?: number;
}

export interface AppConfig {
  // Existing fields...
  
  // Provider configs
  openai: ProviderConfig;
  claude?: ProviderConfig;
  
  // Default provider selection
  defaultProvider?: string;
}
```

### Environment Variables

- `OPENAI_API_KEY` - Existing OpenAI key
- `CLAUDE_API_KEY` - API key for Claude
- `CLAUDE_BASE_URL` - Optional custom endpoint for Claude
- `CLAUDE_TIMEOUT_MS` - Timeout for Claude API calls
- `CODEX_DEFAULT_PROVIDER` - Default provider to use

## Testing Strategy

### Unit Tests

1. **Provider Interface Tests**
   - Test provider registration and selection
   - Verify model detection logic
   - Test configuration loading

2. **Mock Provider Tests**
   - Create mock provider implementing the interface
   - Test core functionality with mock provider
   - Verify error handling and recovery

3. **Claude-Specific Tests**
   - Test Claude client initialization
   - Verify message format conversion
   - Test error handling specific to Claude

### Integration Tests

1. **Cross-Provider Functionality**
   - Test switching between providers
   - Verify command execution works with both providers
   - Test configuration persistence across provider switches

2. **Streaming Tests**
   - Measure streaming performance
   - Test cancellation handling
   - Verify event processing

3. **Error Recovery Tests**
   - Test recovery from network errors
   - Verify rate limit handling
   - Test timeout scenarios

## Success Criteria

1. **Functionality**: All existing Codex features work with both providers
2. **Performance**: Claude integration performs within acceptable parameters
3. **Usability**: Users can easily switch between providers
4. **Reliability**: Robust error handling and recovery
5. **Maintenance**: Clean separation of provider-specific code