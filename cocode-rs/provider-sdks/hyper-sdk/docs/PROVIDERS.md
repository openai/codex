# Provider Capabilities Matrix

This document describes the capabilities and configuration options for each provider supported by hyper-sdk.

## Supported Providers

| Provider | Text | Vision | Streaming | Tools | Embedding | Extended Thinking |
|----------|------|--------|-----------|-------|-----------|-------------------|
| OpenAI | Yes | Yes | Yes | Yes | Yes | No |
| Anthropic | Yes | Yes | Yes | Yes | No | Yes |
| Gemini | Yes | Yes | Yes | Yes | Yes | Yes (thinking_level) |
| Volcengine | Yes | No | No | Yes | No | Yes |
| Z.AI | Yes | No | No | Yes | No | Yes |

## Provider-Specific Options

### OpenAI (`OpenAIOptions`)

```rust
use hyper_sdk::OpenAIOptions;

let opts = OpenAIOptions::new()
    .reasoning_effort(ReasoningEffort::High)  // For o-series models
    .with_response_format(format)             // Structured output
    .with_seed(42);                           // Deterministic sampling
```

**Configuration Fields:**
- `reasoning_effort`: Low/Medium/High for o-series reasoning models
- `previous_response_id`: Enable conversation continuity via response chaining
- `response_format`: JSON schema for structured output
- `seed`: Deterministic sampling seed

**Models:**
- GPT-4o, GPT-4o-mini
- GPT-4-turbo, GPT-4
- o1, o1-mini, o1-pro (reasoning models)
- text-embedding-3-small, text-embedding-3-large

### Anthropic (`AnthropicOptions`)

```rust
use hyper_sdk::AnthropicOptions;

let opts = AnthropicOptions::new()
    .thinking_budget_tokens(8192)  // Extended thinking budget
    .cache_control(true);          // Enable prompt caching
```

**Configuration Fields:**
- `thinking_budget_tokens`: Token budget for extended thinking
- `cache_control`: Enable prompt caching for repeated requests
- `metadata`: Request metadata for tracking

**Models:**
- Claude 3.5 Sonnet (claude-3-5-sonnet-20241022)
- Claude 3.5 Haiku (claude-3-5-haiku-20241022)
- Claude 3 Opus (claude-3-opus-20240229)

**Extended Thinking Notes:**
- Requires model with thinking capability (Claude 3.5+)
- Budget must be specified via `thinking_budget_tokens`
- Thinking content available via `response.thinking()`

### Gemini (`GeminiOptions`)

```rust
use hyper_sdk::GeminiOptions;

let opts = GeminiOptions::new()
    .thinking_level(ThinkingLevel::High)  // Thinking intensity
    .with_grounding(true)                  // Google Search grounding
    .stop_sequences(vec!["END".into()]);   // Custom stop sequences
```

**Configuration Fields:**
- `thinking_level`: None/Low/Medium/High for thinking intensity
- `grounding`: Enable Google Search grounding
- `safety_settings`: Content safety configuration
- `stop_sequences`: Custom stop sequences

**Models:**
- Gemini 2.0 Flash (gemini-2.0-flash-exp)
- Gemini 1.5 Pro (gemini-1.5-pro)
- Gemini 1.5 Flash (gemini-1.5-flash)
- text-embedding-004

**Thinking Notes:**
- Thinking level controls amount of reasoning
- Available on Gemini 2.0+ models

### Volcengine (`VolcengineOptions`)

```rust
use hyper_sdk::VolcengineOptions;

let opts = VolcengineOptions::new()
    .thinking_budget_tokens(4096)
    .reasoning_effort(ReasoningEffort::Medium)
    .caching_enabled(true);
```

**Configuration Fields:**
- `thinking_budget_tokens`: Extended thinking budget
- `previous_response_id`: Conversation continuity
- `caching_enabled`: Enable response caching
- `reasoning_effort`: Low/Medium/High

**Models:**
- Doubao-pro-32k, Doubao-pro-128k
- Doubao-lite-32k, Doubao-lite-128k

### Z.AI (`ZaiOptions`)

```rust
use hyper_sdk::ZaiOptions;

let opts = ZaiOptions::new()
    .thinking_budget_tokens(4096)
    .do_sample(true)
    .request_id("custom-id".into());
```

**Configuration Fields:**
- `thinking_budget_tokens`: Extended thinking budget
- `do_sample`: Enable sampling (vs greedy)
- `request_id`: Custom request ID for tracking
- `user_id`: User identifier

**Models:**
- Z1-preview (extended thinking)
- Z1-preview-lite

## Error Handling by Provider

| Error Type | OpenAI | Anthropic | Gemini | Volcengine | Z.AI |
|------------|--------|-----------|--------|------------|------|
| Rate Limit | 429 + retry-after | 529 + retry-after | 429 | 429 | 429 |
| Quota Exceeded | 429 (insufficient_quota) | Message pattern | 429 | 429 | 429 |
| Auth Error | 401 | 401 | 401 | 401 | 401 |
| Context Window | 400 | 400 | 400 | 400 | 400 |

### Error Handling Best Practices

```rust
use hyper_sdk::{HyperError, retry::{RetryConfig, RetryExecutor}};

// Use built-in retry for transient errors
let config = RetryConfig::default()
    .with_max_attempts(3)
    .with_respect_retry_after(true);

let executor = RetryExecutor::new(config);
let result = executor.execute(|| async {
    model.generate(request.clone()).await
}).await;

// Check error type for specific handling
match result {
    Err(HyperError::QuotaExceeded(_)) => {
        // Requires billing change, don't retry
    }
    Err(HyperError::ContextWindowExceeded(_)) => {
        // Reduce input size
    }
    Err(e) if e.is_retryable() => {
        // Can retry with backoff
    }
    _ => {}
}
```

## Streaming Event Support

| Event | OpenAI | Anthropic | Gemini |
|-------|--------|-----------|--------|
| ResponseCreated | Yes | Yes | Yes |
| TextDelta | Yes | Yes | Yes |
| TextDone | Yes | Yes | Yes |
| ThinkingDelta | No | Yes | Yes |
| ThinkingDone | No | Yes | Yes |
| ToolCallStart | Yes | Yes | Yes |
| ToolCallDelta | Yes | Yes | No* |
| ToolCallDone | Yes | Yes | Yes |
| ResponseDone | Yes | Yes | Yes |

*Gemini sends complete tool calls in a single event rather than streaming deltas.

**Note:** Volcengine and Z.AI do not currently support streaming. Use `generate()` for these providers.

## Usage Examples

### Basic Generation

```rust
use hyper_sdk::{OpenAIProvider, Provider, GenerateRequest, Message};

let provider = OpenAIProvider::from_env()?;
let model = provider.model("gpt-4o")?;

let response = model.generate(
    GenerateRequest::new(vec![Message::user("Hello!")])
).await?;

println!("{}", response.text());
```

### Streaming with Processor

```rust
let mut stream = model.stream(request).await?;

let response = stream.into_processor()
    .on_update(|snapshot| async move {
        println!("Progress: {} chars", snapshot.text.len());
        Ok(())
    })
    .await?;
```

### Extended Thinking (Anthropic)

```rust
use hyper_sdk::{AnthropicProvider, AnthropicOptions, GenerateRequest, Message};

let provider = AnthropicProvider::from_env()?;
let model = provider.model("claude-3-5-sonnet-20241022")?;

let request = GenerateRequest::new(vec![
    Message::user("Solve this complex problem...")
]).with_options(
    AnthropicOptions::new()
        .thinking_budget_tokens(8192)
        .boxed()
);

let response = model.generate(request).await?;

if let Some(thinking) = response.thinking() {
    println!("Reasoning: {}", thinking);
}
println!("Answer: {}", response.text());
```

### Tool Calling

```rust
use hyper_sdk::{GenerateRequest, Message, ToolDefinition, ToolChoice};

let request = GenerateRequest::new(vec![
    Message::user("What's the weather in NYC?")
])
.tools(vec![
    ToolDefinition::full(
        "get_weather",
        "Get current weather for a location",
        serde_json::json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            },
            "required": ["location"]
        })
    )
])
.tool_choice(ToolChoice::Auto);

let response = model.generate(request).await?;

for tool_call in response.tool_calls() {
    println!("Call {} with {}", tool_call.name, tool_call.arguments);
}
```

### Cross-Provider Compatibility

```rust
use hyper_sdk::hooks::CrossProviderSanitizationHook;

// When switching between providers, use sanitization hook
let hook_chain = HookChain::new()
    .add_request_hook(CrossProviderSanitizationHook);

// This ensures thinking signatures and other provider-specific
// data is properly handled when messages cross provider boundaries
```

## Environment Variables

Each provider reads credentials from environment variables:

| Provider | API Key Variable | Base URL Variable (optional) |
|----------|-----------------|------------------------------|
| OpenAI | `OPENAI_API_KEY` | `OPENAI_BASE_URL` |
| Anthropic | `ANTHROPIC_API_KEY` | `ANTHROPIC_BASE_URL` |
| Gemini | `GOOGLE_API_KEY` | `GOOGLE_BASE_URL` |
| Volcengine | `VOLCENGINE_API_KEY` | `VOLCENGINE_BASE_URL` |
| Z.AI | `ZAI_API_KEY` | `ZAI_BASE_URL` |

## Provider Selection

```rust
use hyper_sdk::{HyperClient, providers::any_from_env};

// Recommended: Use HyperClient for multi-provider setup
let client = HyperClient::from_env()?;
let model = client.model("openai", "gpt-4o")?;

// Or auto-detect first available provider
let provider = any_from_env()?;
```
