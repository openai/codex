# hyper-sdk

Unified multi-provider AI model SDK for Rust.

## Features

- **Provider Agnostic**: Single API across all providers
- **Multi-Provider Conversations**: Seamlessly switch between providers mid-conversation
- **Native Streaming**: Three-level streaming API with Crush-like processor
- **Hook System**: Request/Response/Stream interception for extensibility
- **Type Safe**: Strongly typed request/response types with cross-provider message conversion

## Supported Providers

| Provider | Models | Features |
|----------|--------|----------|
| OpenAI | gpt-4o, gpt-4o-mini, o1, o3-mini | Vision, Tools, Thinking |
| Anthropic | claude-sonnet-4, claude-opus-4 | Thinking signatures, Cache |
| Google Gemini | gemini-2.5-pro, gemini-1.5-pro | Vision, Thinking |
| Volcengine Ark | Doubao series | Reasoning effort |
| Z.AI | GLM-4 series | Chinese language |
| OpenAI-Compatible | Any | Custom endpoints |

## Quick Start

```rust
use hyper_sdk::{GenerateRequest, Message, OpenAIProvider, Provider};

#[tokio::main]
async fn main() -> hyper_sdk::Result<()> {
    // Create provider from environment
    let provider = OpenAIProvider::from_env()?;
    let model = provider.model("gpt-4o")?;

    // Simple generation
    let request = GenerateRequest::from_text("What is the capital of France?")
        .temperature(0.7)
        .max_tokens(1000);

    let response = model.generate(request).await?;
    println!("{}", response.text());

    Ok(())
}
```

## Cross-Provider Conversations

hyper-sdk automatically handles cross-provider message conversion:

```rust
use hyper_sdk::{
    AnthropicProvider, OpenAIProvider, Provider,
    ConversationContext, GenerateRequest, Message,
};

#[tokio::main]
async fn main() -> hyper_sdk::Result<()> {
    let openai = OpenAIProvider::from_env()?;
    let anthropic = AnthropicProvider::from_env()?;

    let mut conversation = ConversationContext::new()
        .with_provider_info("openai", "gpt-4o");

    // First turn with OpenAI
    let openai_model = openai.model("gpt-4o")?;
    let response = conversation.generate(
        openai_model.as_ref(),
        GenerateRequest::from_text("Hello!"),
    ).await?;

    // Switch to Anthropic - thinking signatures automatically sanitized
    conversation.switch_provider("anthropic", "claude-sonnet-4-20250514");
    let anthropic_model = anthropic.model("claude-sonnet-4-20250514")?;
    let response = conversation.generate(
        anthropic_model.as_ref(),
        GenerateRequest::from_text("Continue our conversation"),
    ).await?;

    Ok(())
}
```

## Streaming API

### Level 1: Event Iterator

```rust
let mut stream = model.stream(request).await?;
while let Some(result) = stream.next_event().await {
    match result? {
        StreamEvent::TextDelta { delta, .. } => print!("{}", delta),
        StreamEvent::ResponseDone { .. } => break,
        _ => {}
    }
}
```

### Level 2: Processor with Callbacks

```rust
let response = model.stream(request).await?
    .into_processor()
    .on_text(|delta| async move {
        print!("{}", delta);
        Ok(())
    })
    .await?;
```

### Level 3: Crush-like Processor (Recommended)

```rust
// Update same message pattern - ideal for real-time UI updates
let msg_id = db.insert_message(conv_id, Role::Assistant).await?;

let response = model.stream(request).await?
    .into_processor()
    .on_update(|snapshot| async move {
        // UPDATE same message (not INSERT)
        db.update_message(msg_id, &snapshot.text).await?;
        pubsub.publish(format!("message:{}", msg_id), "updated").await;
        Ok(())
    })
    .await?;
```

## Hook System

### Request Hooks

```rust
use hyper_sdk::hooks::{RequestHook, HookContext};
use async_trait::async_trait;

#[derive(Debug)]
struct LoggingHook;

#[async_trait]
impl RequestHook for LoggingHook {
    async fn on_request(
        &self,
        request: &mut GenerateRequest,
        context: &mut HookContext,
    ) -> Result<(), HyperError> {
        println!("Sending request to {} / {}", context.provider, context.model_id);
        Ok(())
    }

    fn name(&self) -> &str { "logging" }
}
```

### Built-in Hooks

- `ResponseIdHook` - Tracks `previous_response_id` for conversation continuity
- `LoggingHook` - Logs requests and responses
- `UsageTrackingHook` - Accumulates token usage across requests
- `CrossProviderSanitizationHook` - Auto-sanitizes cross-provider messages

## Configuration

### Environment Variables

| Variable | Provider | Description |
|----------|----------|-------------|
| `OPENAI_API_KEY` | OpenAI | API key |
| `OPENAI_BASE_URL` | OpenAI | Custom endpoint (optional) |
| `ANTHROPIC_API_KEY` | Anthropic | API key |
| `GOOGLE_API_KEY` | Gemini | API key |

### Provider-specific Options

```rust
use hyper_sdk::options::{OpenAIOptions, AnthropicOptions};

// OpenAI with reasoning effort
let request = GenerateRequest::from_text("Complex problem")
    .provider_options(Box::new(OpenAIOptions {
        reasoning_effort: Some(ReasoningEffort::High),
        ..Default::default()
    }));

// Anthropic with thinking budget
let request = GenerateRequest::from_text("Think step by step")
    .provider_options(Box::new(AnthropicOptions {
        thinking_budget_tokens: Some(10000),
        ..Default::default()
    }));
```

## Error Handling

```rust
use hyper_sdk::HyperError;

match model.generate(request).await {
    Ok(response) => println!("{}", response.text()),
    Err(HyperError::ContextWindowExceeded(msg)) => {
        eprintln!("Context too long: {}", msg);
    }
    Err(HyperError::RateLimitExceeded(msg)) => {
        eprintln!("Rate limited: {}", msg);
    }
    Err(HyperError::AuthenticationFailed(msg)) => {
        eprintln!("Auth failed: {}", msg);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Live Integration Tests

hyper-sdk includes live integration tests that run against real provider APIs.

### Configuration

1. Copy `.env.test.example` to `.env.test`
2. Fill in your API credentials for the providers you want to test

### Running Tests

```bash
# Run all configured provider tests
cargo test -p hyper-sdk --test live -- --test-threads=1

# Run tests for a specific provider
cargo test -p hyper-sdk --test live openai -- --test-threads=1
cargo test -p hyper-sdk --test live anthropic -- --test-threads=1

# Run specific test category
cargo test -p hyper-sdk --test live test_basic -- --test-threads=1
cargo test -p hyper-sdk --test live test_tools -- --test-threads=1
cargo test -p hyper-sdk --test live test_streaming -- --test-threads=1

# Run specific provider + feature
cargo test -p hyper-sdk --test live test_basic_openai -- --test-threads=1
```

**Why `--test-threads=1`?** Live tests must run sequentially because:
1. Tests share provider API rate limits under a single API key
2. Provider APIs may have per-key concurrent request limits
3. Running tests in parallel can trigger rate limiting, causing flaky failures

### Test Categories

| Category | Tests |
|----------|-------|
| basic | Text generation, token usage, multi-turn |
| tools | Tool calling, complete tool flow |
| vision | Image understanding |
| streaming | Stream events, streaming with tools |

## Architecture

```
hyper-sdk/src/
├── lib.rs              # Re-exports
├── provider.rs         # Provider trait
├── model.rs            # Model trait
├── messages.rs         # Role, Message, ContentBlock, ProviderMetadata
├── request.rs          # GenerateRequest
├── response.rs         # GenerateResponse, FinishReason, TokenUsage
├── error.rs            # HyperError enum
├── capability.rs       # Capability enum, ModelInfo
├── tools.rs            # ToolDefinition, ToolCall, ToolChoice
├── registry.rs         # Global ProviderRegistry
├── conversation.rs     # ConversationContext (multi-turn)
├── session.rs          # SessionConfig
├── embedding.rs        # EmbedRequest/Response
├── object.rs           # Structured output
├── providers/          # Provider implementations
│   ├── openai.rs
│   ├── anthropic.rs
│   ├── gemini.rs
│   ├── volcengine.rs
│   ├── zai.rs
│   └── openai_compat.rs
├── options/            # Provider-specific options
├── hooks/              # Request/Response/Stream hooks
└── stream/             # Streaming infrastructure
```

## License

Apache 2.0
