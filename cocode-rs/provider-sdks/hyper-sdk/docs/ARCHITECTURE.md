# hyper-sdk Architecture

This document describes the internal architecture of hyper-sdk, a unified multi-provider AI model SDK.

## Core Trait Hierarchy

### Provider Trait

The `Provider` trait is the entry point for each AI provider integration:

```rust
#[async_trait]
pub trait Provider: Send + Sync + Debug {
    fn name(&self) -> &str;
    fn model(&self, model_id: &str) -> Result<Arc<dyn Model>, HyperError>;
    async fn list_models(&self) -> Result<Vec<ModelInfo>, HyperError>;
}
```

Each provider (OpenAI, Anthropic, Gemini, etc.) implements this trait to:
- Provide its name for identification
- Create model instances for specific model IDs
- List available models with their capabilities

### Model Trait

The `Model` trait defines the generation interface:

```rust
#[async_trait]
pub trait Model: Send + Sync + Debug {
    fn model_id(&self) -> &str;
    fn provider(&self) -> &str;
    fn capabilities(&self) -> &[Capability];

    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, HyperError>;
    async fn stream(&self, request: GenerateRequest) -> Result<StreamResponse, HyperError>;
    async fn embed(&self, request: EmbedRequest) -> Result<EmbedResponse, HyperError>;
}
```

## Three-Level Streaming Architecture

hyper-sdk provides three levels of streaming abstraction:

### Level 1: Event Iterator

Raw stream events via `StreamResponse`:

```rust
let mut stream = model.stream(request).await?;
while let Some(result) = stream.next_event().await {
    match result? {
        StreamEvent::TextDelta { delta, .. } => { /* handle */ }
        StreamEvent::ResponseDone { .. } => break,
        _ => {}
    }
}
```

### Level 2: Processor with Callbacks

Event-driven processing via `StreamProcessor`:

```rust
stream.into_processor()
    .on_text(|delta| async move { /* handle */ })
    .on_thinking(|delta| async move { /* handle */ })
    .await?;
```

### Level 3: Snapshot-based (Crush-like)

Accumulated state pattern via `StreamSnapshot`:

```rust
stream.into_processor()
    .on_update(|snapshot| async move {
        // snapshot.text contains accumulated text
        // Update same record in database
    })
    .await?;
```

The snapshot accumulates all content incrementally, enabling "update same message" patterns ideal for real-time UI.

**Memory Note**: `StreamSnapshot.text` grows unbounded during streaming. This is intentional because streaming responses are bounded by `max_tokens` and the full response is needed for the final `GenerateResponse`. Memory pressure should be managed at the application level.

## Hook System Design

### Hook Types

1. **RequestHook**: Intercepts before sending to provider
2. **ResponseHook**: Intercepts after receiving response
3. **StreamHook**: Intercepts stream events

### Hook Context

`HookContext` provides request metadata:

```rust
pub struct HookContext {
    pub provider: String,
    pub model_id: String,
    pub request_id: String,
    pub metadata: HashMap<String, Value>,
}
```

### Built-in Hooks

- `ResponseIdHook`: Tracks `previous_response_id` for conversation continuity
- `UsageTrackingHook`: Accumulates token usage across requests
- `CrossProviderSanitizationHook`: Auto-strips thinking signatures when switching providers

## Cross-Provider Message Sanitization

When messages from one provider are sent to another, thinking signatures must be stripped to avoid verification errors. This happens automatically via:

1. `Message.sanitize_for_target(provider, model)`: Strips signatures if source differs from target
2. `GenerateRequest.sanitize_for_target()`: Applies to all messages
3. Each `Model::generate/stream` calls sanitization automatically

Provider metadata is tracked via `Message.source_provider` and `Message.source_model`.

## Provider Options Type System

Provider-specific options use a trait object pattern:

```rust
pub type ProviderOptions = Box<dyn ProviderOptionsData>;

pub trait ProviderOptionsData: Send + Sync + Debug + Any {
    fn as_any(&self) -> &dyn Any;
    fn provider_name(&self) -> &str;
}
```

Each provider has its own options type:
- `OpenAIOptions`: reasoning_effort, previous_response_id, seed
- `AnthropicOptions`: thinking_budget_tokens, metadata
- `GeminiOptions`: thinking_level, grounding
- `VolcengineOptions`: thinking_budget_tokens, caching_enabled
- `ZaiOptions`: thinking_budget_tokens, do_sample

Downcasting is done via `downcast_options::<T>()`:

```rust
if let Some(opts) = downcast_options::<OpenAIOptions>(&request.provider_options) {
    // Use OpenAI-specific options
}
```

**Serialization Note**: `provider_options` is marked `#[serde(skip)]` in `GenerateRequest` because provider-specific options are not portable across providers and would not roundtrip correctly.

## Error Handling Strategy

### Unified Error Type

`HyperError` provides a unified error type across all providers:

```rust
pub enum HyperError {
    ProviderNotFound(String),
    ModelNotFound(String),
    UnsupportedCapability(Capability),
    AuthenticationFailed(String),
    RateLimitExceeded(String),
    ContextWindowExceeded(String),
    QuotaExceeded(String),           // NOT retryable
    NetworkError(String),             // Retryable
    ParseError(String),
    StreamError(String),
    Retryable { message, delay },     // Explicit retry with delay
    // ...
}
```

### Error Chain Preservation

Errors like `NetworkError` and `ParseError` store stringified messages rather than wrapping source errors. This is intentional:

1. **Provider Independence**: Each SDK has different error types
2. **API Stability**: Avoids exposing internal dependencies
3. **Serialization**: String errors serialize cleanly

The `From` implementations preserve context via `Display` output.

### Retryability

```rust
impl HyperError {
    pub fn is_retryable(&self) -> bool {
        matches!(self,
            HyperError::Retryable { .. } |
            HyperError::RateLimitExceeded(_) |
            HyperError::NetworkError(_)
        )
    }

    pub fn retry_delay(&self) -> Option<Duration> {
        // Only Retryable variant has delay
    }
}
```

**Important**: `QuotaExceeded` is NOT retryable (requires billing change), unlike `RateLimitExceeded` (temporary).

## Provider Implementation Pattern

Each provider follows a consistent pattern:

```
providers/{name}.rs
├── {Name}Config        # Configuration struct
├── {Name}Provider      # Provider implementation
├── {Name}ProviderBuilder  # Builder pattern
├── {Name}Model         # Model implementation (private)
└── Conversion helpers  # convert_*_to_{sdk}(), map_{name}_error()
```

### Common Conversion Patterns

Each provider implements similar conversion functions:

1. `convert_content_to_{sdk}()`: `ContentBlock[]` → SDK content type
2. `convert_tool_to_{sdk}()`: `ToolDefinition` → SDK tool type
3. `convert_tool_choice_to_{sdk}()`: `ToolChoice` → SDK tool choice
4. `convert_{sdk}_response()`: SDK response → `GenerateResponse`
5. `map_{sdk}_error()`: SDK error → `HyperError`

These are not extracted to a common module because each SDK has different types that cannot share a unified interface without adding significant abstraction overhead.

## Module Organization

```
src/
├── lib.rs              # Re-exports
├── provider.rs         # Provider trait
├── model.rs            # Model trait
├── messages.rs         # Message types, ContentBlock, ProviderMetadata
├── request.rs          # GenerateRequest with builder pattern
├── response.rs         # GenerateResponse, FinishReason, TokenUsage
├── error.rs            # HyperError enum with retryability
├── capability.rs       # Capability enum, ModelInfo
├── tools.rs            # ToolDefinition, ToolCall, ToolChoice
├── registry.rs         # Global ProviderRegistry (deprecated)
├── client.rs           # HyperClient (recommended entry point)
├── conversation.rs     # ConversationContext for multi-turn
├── session.rs          # SessionConfig
├── embedding.rs        # Embedding support
├── call_id.rs          # Call ID generation/enhancement
├── providers/          # Provider implementations
│   ├── mod.rs
│   ├── openai.rs
│   ├── anthropic.rs
│   ├── gemini.rs
│   ├── volcengine.rs
│   ├── zai.rs
│   └── openai_compat.rs
├── options/            # Provider-specific options
│   ├── mod.rs
│   ├── openai.rs
│   ├── anthropic.rs
│   ├── gemini.rs
│   ├── volcengine.rs
│   └── zai.rs
├── hooks/              # Hook system
│   ├── mod.rs
│   ├── request.rs
│   ├── response.rs
│   └── stream.rs
└── stream/             # Streaming infrastructure
    ├── mod.rs
    ├── event.rs        # StreamEvent enum
    ├── response.rs     # StreamResponse
    ├── processor.rs    # StreamProcessor
    └── snapshot.rs     # StreamSnapshot for Crush-like pattern
```

## Testing

### Unit Tests

Each module contains unit tests for its functionality.

### Live Integration Tests

Located in `tests/live.rs`, these test against real provider APIs.

**Note**: Live tests must be run with `--test-threads=1` because:
1. Tests share API rate limits
2. Provider APIs may have per-key concurrent request limits
3. Prevents flaky tests from concurrent requests hitting rate limits

```bash
cargo test -p hyper-sdk --test live -- --test-threads=1
```

## Design Decisions

### Why String Errors Instead of Wrapped Sources

See `src/error.rs` module documentation.

### Why Provider Options Are Not Serializable

Provider-specific options (`OpenAIOptions`, etc.) are not serializable via `GenerateRequest` because:
1. Options are provider-specific and not portable
2. Deserialization would require knowing the target provider type
3. Options may contain non-serializable types (closures, etc.)

### Why No Shared Provider Utilities Module

Each provider SDK has different types (message, content block, tool, etc.). Extracting common utilities would require:
1. Generic traits for each SDK type
2. Significant boilerplate for type conversions
3. Loss of type safety at provider boundaries

The current approach keeps provider implementations self-contained and type-safe at the cost of some structural duplication.
