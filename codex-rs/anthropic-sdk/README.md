# anthropic-sdk

Rust SDK for the Anthropic Claude API.

Reference: [anthropic-sdk-python](https://github.com/anthropics/anthropic-sdk-python/)

## Features

- **Messages API**: Create messages with text, images, and tool calls
- **Token Counting**: Count tokens before sending messages
- **Async-first**: Built on tokio for async/await support
- **Type-safe**: Full Rust type system for API requests and responses
- **Error handling**: Comprehensive error types with retry support

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
anthropic-sdk = { path = "../anthropic-sdk" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Quick Start

```rust
use anthropic_sdk::{Client, MessageCreateParams, MessageParam};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client from ANTHROPIC_API_KEY env var
    let client = Client::from_env()?;

    // Send a message
    let message = client.messages().create(
        MessageCreateParams::new(
            "claude-3-5-sonnet-20241022",
            1024,
            vec![MessageParam::user("Hello, Claude!")],
        )
    ).await?;

    println!("{}", message.text());
    Ok(())
}
```

## Examples

### Basic Text Message

```rust
use anthropic_sdk::{Client, MessageCreateParams, MessageParam};

let client = Client::from_env()?;

let message = client.messages().create(
    MessageCreateParams::new(
        "claude-3-5-sonnet-20241022",
        1024,
        vec![MessageParam::user("What is the capital of France?")],
    )
    .system("You are a helpful geography assistant.")
    .temperature(0.7)
).await?;

println!("Response: {}", message.text());
```

### Image Input (Base64)

```rust
use anthropic_sdk::{Client, MessageCreateParams, MessageParam, ContentBlockParam};

let client = Client::from_env()?;

// Read and encode image
let image_data = base64::encode(std::fs::read("image.png")?);

let message = client.messages().create(
    MessageCreateParams::new(
        "claude-3-5-sonnet-20241022",
        1024,
        vec![MessageParam::user_with_content(vec![
            ContentBlockParam::image_base64(image_data, "image/png"),
            ContentBlockParam::text("What's in this image?"),
        ])],
    )
).await?;

println!("{}", message.text());
```

### Tool Use

```rust
use anthropic_sdk::{Client, MessageCreateParams, MessageParam, Tool, ToolChoice};

let client = Client::from_env()?;

let weather_tool = Tool {
    name: "get_weather".to_string(),
    description: Some("Get the current weather for a location".to_string()),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "location": {
                "type": "string",
                "description": "City name"
            }
        },
        "required": ["location"]
    }),
};

let message = client.messages().create(
    MessageCreateParams::new(
        "claude-3-5-sonnet-20241022",
        1024,
        vec![MessageParam::user("What's the weather in Paris?")],
    )
    .tools(vec![weather_tool])
    .tool_choice(ToolChoice::Auto { disable_parallel_tool_use: None })
).await?;

// Check for tool use
for (id, name, input) in message.tool_uses() {
    println!("Tool: {} ({})", name, id);
    println!("Input: {}", input);
}
```

### Token Counting

```rust
use anthropic_sdk::{Client, CountTokensParams, MessageParam};

let client = Client::from_env()?;

let count = client.messages().count_tokens(
    CountTokensParams::new(
        "claude-3-5-sonnet-20241022",
        vec![MessageParam::user("Hello, world!")],
    )
    .system("You are a helpful assistant.")
).await?;

println!("Input tokens: {}", count.input_tokens);
```

### Multi-turn Conversation

```rust
use anthropic_sdk::{Client, MessageCreateParams, MessageParam, Role};

let client = Client::from_env()?;

let messages = vec![
    MessageParam::user("Hi, I'm Alice"),
    MessageParam::assistant("Hello Alice! How can I help you today?"),
    MessageParam::user("What's my name?"),
];

let response = client.messages().create(
    MessageCreateParams::new("claude-3-5-sonnet-20241022", 1024, messages)
).await?;

println!("{}", response.text());
// Output: Your name is Alice!
```

### Error Handling

```rust
use anthropic_sdk::{Client, MessageCreateParams, MessageParam, AnthropicError};

let client = Client::from_env()?;

match client.messages().create(
    MessageCreateParams::new("claude-3-5-sonnet-20241022", 1024, vec![])
).await {
    Ok(message) => println!("{}", message.text()),
    Err(AnthropicError::BadRequest(msg)) => eprintln!("Invalid request: {}", msg),
    Err(AnthropicError::RateLimited { retry_after }) => {
        eprintln!("Rate limited, retry after: {:?}", retry_after);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Supported Models

- `claude-3-5-sonnet-20241022` (Claude 3.5 Sonnet)
- `claude-3-opus-20240229` (Claude 3 Opus)
- `claude-3-sonnet-20240229` (Claude 3 Sonnet)
- `claude-3-haiku-20240307` (Claude 3 Haiku)

## API Reference

### Client

| Method | Description |
|--------|-------------|
| `Client::from_env()` | Create from `ANTHROPIC_API_KEY` env var |
| `Client::with_api_key(key)` | Create with explicit API key |
| `Client::new(config)` | Create with full configuration |
| `client.messages()` | Get the Messages resource |

### Messages Resource

| Method | Description |
|--------|-------------|
| `messages.create(params)` | Create a message |
| `messages.count_tokens(params)` | Count tokens for messages |

### Types

- `MessageParam` - Input message (user/assistant)
- `Message` - Response message
- `ContentBlockParam` - Input content (text/image/tool_result)
- `ContentBlock` - Output content (text/tool_use)
- `Tool` - Tool definition
- `Usage` - Token usage information

## Not Yet Implemented

- Streaming responses
- AWS Bedrock support
- Google Vertex AI support
- Message batches
- Tool helpers

## License

Apache-2.0
