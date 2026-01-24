# anthropic-sdk

Rust SDK for the Anthropic Claude API.

Reference: [anthropic-sdk-python](https://github.com/anthropics/anthropic-sdk-python/) @ `2eb941512885bdea844cb46e3f93b60ffa51973b`

## Features

- **Messages API**: Create messages with text, images, and tool calls
- **Streaming**: Real-time SSE streaming with state accumulation
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

### Streaming Responses

Stream responses in real-time using Server-Sent Events (SSE):

```rust
use anthropic_sdk::{Client, MessageCreateParams, MessageParam, RawMessageStreamEvent, ContentBlockDelta};
use futures::StreamExt;

let client = Client::from_env()?;

// Create a streaming request
let mut stream = client.messages().create_stream(
    MessageCreateParams::new(
        "claude-sonnet-4-20250514",
        1024,
        vec![MessageParam::user("Write a haiku about Rust")],
    )
).await?;

// Option 1: Process individual events
while let Some(event) = stream.next_event().await {
    match event? {
        RawMessageStreamEvent::ContentBlockDelta { delta, .. } => {
            if let ContentBlockDelta::TextDelta { text } = delta {
                print!("{}", text);  // Print text as it arrives
            }
        }
        RawMessageStreamEvent::MessageStop => {
            println!("\n--- Stream complete ---");
        }
        _ => {}
    }
}

// Option 2: Get only text deltas
let mut stream = client.messages().create_stream(params).await?;
let mut text_stream = stream.text_stream();
while let Some(text) = text_stream.next().await {
    print!("{}", text?);
}

// Option 3: Wait for the complete message
let stream = client.messages().create_stream(params).await?;
let message = stream.get_final_message().await?;
println!("{}", message.text());
```

#### Accessing Accumulated State

You can access the current accumulated message at any point during streaming:

```rust
let mut stream = client.messages().create_stream(params).await?;

while let Some(event) = stream.next_event().await {
    let _ = event?;

    // Get current snapshot (returns None until message_start is received)
    if let Some(Ok(snapshot)) = stream.current_message_snapshot() {
        println!("Current text: {}", snapshot.text());
        println!("Tokens so far: {}", snapshot.usage.output_tokens);
    }
}
```

#### Raw Event Stream

For low-level access to SSE events without state accumulation:

```rust
use futures::StreamExt;

let mut event_stream = client.messages().create_stream_raw(params).await?;

while let Some(event) = event_stream.next().await {
    println!("Raw event: {:?}", event?);
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
| `messages.create(params)` | Create a message (non-streaming) |
| `messages.create_stream(params)` | Create a streaming message with `MessageStream` wrapper |
| `messages.create_stream_raw(params)` | Create a raw SSE event stream |
| `messages.count_tokens(params)` | Count tokens for messages |

### Types

**Messages:**
- `MessageParam` - Input message (user/assistant)
- `Message` - Response message
- `ContentBlockParam` - Input content (text/image/tool_result)
- `ContentBlock` - Output content (text/tool_use/thinking)
- `Tool` - Tool definition
- `Usage` - Token usage information

**Streaming:**
- `MessageStream` - High-level streaming wrapper with state accumulation
- `EventStream` - Raw SSE event stream
- `RawMessageStreamEvent` - SSE event variants (MessageStart, ContentBlockDelta, etc.)
- `ContentBlockDelta` - Delta variants (TextDelta, InputJsonDelta, ThinkingDelta, etc.)
- `ContentBlockStartData` - Content block start variants

## Not Yet Implemented

- AWS Bedrock support
- Google Vertex AI support
- Message batches
- Tool helpers

### Streaming: High-Level Convenience Events

The Python SDK provides synthetic high-level events with built-in snapshots:

```python
# Python SDK high-level events (NOT available in Rust SDK)
class TextEvent:
    text: str       # The delta
    snapshot: str   # Accumulated text so far

class InputJsonEvent:
    partial_json: str  # The delta
    snapshot: object   # Parsed accumulated object

class MessageStopEvent:
    message: Message   # Final complete message
```

**Workaround:** Rust users can achieve equivalent functionality:

```rust
// Instead of TextEvent with snapshot, use:
while let Some(event) = stream.next_event().await {
    if let RawMessageStreamEvent::ContentBlockDelta {
        delta: ContentBlockDelta::TextDelta { text }, ..
    } = event? {
        print!("{}", text);  // Process delta

        // Get accumulated snapshot
        if let Some(Ok(snapshot)) = stream.current_message_snapshot() {
            // snapshot.text() gives accumulated text
        }
    }
}

// Instead of MessageStopEvent with message, use:
let message = stream.get_final_message().await?;
```

## License

Apache-2.0
