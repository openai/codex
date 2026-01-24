# z-ai-sdk

Rust SDK for the Z.AI and ZhipuAI Chat/Embeddings API.

Reference: [z-ai-sdk-python](https://github.com/zai-org/z-ai-sdk-python/) @`6d9d9f84a4ffece0f169fc2ee311ce3fbff22267`

## Features

- **Chat Completions API**: Create chat completions with text, images, and tool calls
- **Embeddings API**: Generate text embeddings for semantic search
- **Extended Thinking**: Support for reasoning/thinking models (ultrathink)
- **Tool/Function Calling**: Define and use tools in conversations
- **Image Input**: Support for base64 and URL image inputs
- **Multi-region**: Support for both Z.AI and ZhipuAI endpoints
- **JWT Authentication**: Automatic JWT token generation with caching
- **Async-first**: Built on tokio for async/await support
- **Type-safe**: Full Rust type system for API requests and responses
- **Error handling**: Comprehensive error types with retry support

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
z-ai-sdk = { path = "../z-ai-sdk" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Quick Start

```rust
use z_ai_sdk::{ZaiClient, ChatCompletionsCreateParams, MessageParam};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client from ZAI_API_KEY env var
    let client = ZaiClient::from_env()?;

    // Send a message
    let completion = client.chat().completions().create(
        ChatCompletionsCreateParams::new(
            "glm-4",
            vec![MessageParam::user("Hello!")],
        )
    ).await?;

    println!("{}", completion.text());
    Ok(())
}
```

## Examples

### Basic Text Message

```rust
use z_ai_sdk::{ZaiClient, ChatCompletionsCreateParams, MessageParam};

let client = ZaiClient::from_env()?;

let completion = client.chat().completions().create(
    ChatCompletionsCreateParams::new(
        "glm-4",
        vec![
            MessageParam::system("You are a helpful assistant."),
            MessageParam::user("What is the capital of France?"),
        ],
    )
    .temperature(0.7)
    .max_tokens(1024)
).await?;

println!("Response: {}", completion.text());
```

### Image Input (Base64)

```rust
use z_ai_sdk::{ZaiClient, ChatCompletionsCreateParams, MessageParam, ContentBlock};

let client = ZaiClient::from_env()?;

// Read and encode image
let image_data = base64::encode(std::fs::read("image.png")?);

let completion = client.chat().completions().create(
    ChatCompletionsCreateParams::new(
        "glm-4v",
        vec![MessageParam::user_with_content(vec![
            ContentBlock::image_base64(&image_data, "image/png"),
            ContentBlock::text("What's in this image?"),
        ])],
    )
).await?;

println!("{}", completion.text());
```

### Tool Use

```rust
use z_ai_sdk::{ZaiClient, ChatCompletionsCreateParams, MessageParam, Tool, ToolChoice};

let client = ZaiClient::from_env()?;

let weather_tool = Tool::function(
    "get_weather",
    Some("Get the current weather for a location".to_string()),
    serde_json::json!({
        "type": "object",
        "properties": {
            "location": {
                "type": "string",
                "description": "City name"
            }
        },
        "required": ["location"]
    }),
);

let completion = client.chat().completions().create(
    ChatCompletionsCreateParams::new(
        "glm-4",
        vec![MessageParam::user("What's the weather in Paris?")],
    )
    .tools(vec![weather_tool])
    .tool_choice(ToolChoice::auto())
).await?;

// Check for tool calls
if let Some(tool_calls) = completion.tool_calls() {
    for call in tool_calls {
        println!("Tool: {} ({})", call.function.name, call.id);
        println!("Arguments: {}", call.function.arguments);
    }
}
```

### Extended Thinking (Ultrathink)

```rust
use z_ai_sdk::{ZaiClient, ChatCompletionsCreateParams, MessageParam, ThinkingConfig};

let client = ZaiClient::from_env()?;

let completion = client.chat().completions().create(
    ChatCompletionsCreateParams::new(
        "glm-4-plus",
        vec![MessageParam::user("Solve this step by step: What is 15% of 80?")],
    )
    .thinking(ThinkingConfig::enabled_with_budget(8192))
).await?;

// Get reasoning content
if let Some(reasoning) = completion.reasoning() {
    println!("Reasoning: {}", reasoning);
}

println!("Answer: {}", completion.text());
```

### Embeddings

```rust
use z_ai_sdk::{ZaiClient, EmbeddingsCreateParams};

let client = ZaiClient::from_env()?;

let response = client.embeddings().create(
    EmbeddingsCreateParams::new("embedding-3", "Hello, world!")
        .dimensions(1024)
).await?;

if let Some(embedding) = response.embedding() {
    println!("Embedding dimension: {}", embedding.len());
}
```

### ZhipuAI Region

```rust
use z_ai_sdk::{ZhipuAiClient, ChatCompletionsCreateParams, MessageParam};

// Use ZhipuAI endpoint (open.bigmodel.cn)
let client = ZhipuAiClient::from_env()?;

let completion = client.chat().completions().create(
    ChatCompletionsCreateParams::new(
        "glm-4",
        vec![MessageParam::user("Hello!")],
    )
).await?;

println!("{}", completion.text());
```

### Multi-turn Conversation

```rust
use z_ai_sdk::{ZaiClient, ChatCompletionsCreateParams, MessageParam};

let client = ZaiClient::from_env()?;

let messages = vec![
    MessageParam::system("You are a helpful assistant."),
    MessageParam::user("Hi, I'm Alice"),
    MessageParam::assistant("Hello Alice! How can I help you today?"),
    MessageParam::user("What's my name?"),
];

let completion = client.chat().completions().create(
    ChatCompletionsCreateParams::new("glm-4", messages)
).await?;

println!("{}", completion.text());
// Output: Your name is Alice!
```

### Error Handling

```rust
use z_ai_sdk::{ZaiClient, ChatCompletionsCreateParams, MessageParam, ZaiError};

let client = ZaiClient::from_env()?;

match client.chat().completions().create(
    ChatCompletionsCreateParams::new("glm-4", vec![])
).await {
    Ok(completion) => println!("{}", completion.text()),
    Err(ZaiError::BadRequest(msg)) => eprintln!("Invalid request: {}", msg),
    Err(ZaiError::RateLimited { retry_after }) => {
        eprintln!("Rate limited, retry after: {:?}", retry_after);
    }
    Err(ZaiError::Authentication(msg)) => eprintln!("Auth failed: {}", msg),
    Err(e) => eprintln!("Error: {}", e),
}
```

## Supported Models

### Chat Models
- `glm-4` (GLM-4 base model)
- `glm-4-plus` (GLM-4 Plus with extended capabilities)
- `glm-4v` (GLM-4 Vision model)
- `glm-4-flash` (Fast inference model)

### Embedding Models
- `embedding-3` (Text embedding model)

## API Reference

### Client

| Method | Description |
|--------|-------------|
| `ZaiClient::from_env()` | Create from `ZAI_API_KEY` env var |
| `ZhipuAiClient::from_env()` | Create from `ZHIPUAI_API_KEY` env var |
| `ZaiClient::new(config)` | Create with full configuration |
| `client.chat()` | Get the Chat resource |
| `client.embeddings()` | Get the Embeddings resource |

### Chat Completions

| Method | Description |
|--------|-------------|
| `chat.completions().create(params)` | Create a chat completion |

### Embeddings

| Method | Description |
|--------|-------------|
| `embeddings.create(params)` | Create embeddings |

### Types

- `MessageParam` - Input message (system/user/assistant/tool)
- `Completion` - Chat completion response
- `CompletionMessage` - Message in completion (with reasoning_content)
- `ContentBlock` - Input content (text/image_url)
- `Tool` - Tool definition
- `ThinkingConfig` - Extended thinking configuration
- `CompletionUsage` - Token usage information
- `EmbeddingsResponded` - Embeddings response

## Not Yet Implemented

- Streaming responses (SSE)
- File uploads
- Batch processing
- Fine-tuning API
- Knowledge base API

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `ZAI_API_KEY` | API key for Z.AI | Required |
| `ZHIPUAI_API_KEY` | API key for ZhipuAI | Required |
| `ZAI_BASE_URL` | Override Z.AI base URL | `https://api.z.ai/api/paas/v4` |
| `ZHIPUAI_BASE_URL` | Override ZhipuAI base URL | `https://open.bigmodel.cn/api/paas/v4` |

## License

Apache-2.0
