# volcengine-ark-sdk

Rust SDK for the Volcengine Ark Response API.

Reference: [volcengine-python-sdk](https://github.com/volcengine/volcengine-python-sdk) @ `5a682e10b72f8f112e2db800b577b5dc5e546db9`

## Features

- **Response API**: Create responses with text, images, and tool calls (non-streaming)
- **Embeddings API**: Generate text embeddings for semantic search and similarity
- **Prompt Caching**: Cache configuration for cost optimization
- **Extended Thinking**: Support for thinking/reasoning mode
- **Reasoning Output**: Parse reasoning model outputs
- **Async-first**: Built on tokio for async/await support
- **Type-safe**: Full Rust type system for API requests and responses
- **Error handling**: Comprehensive error types with retry support

## Scope

This is a **minimal SDK** focused on core functionality:

| Feature | Status |
|---------|--------|
| Response API (non-streaming) | Supported |
| Embeddings API | Supported |
| Chat/text conversations | Supported |
| Image input (base64/URL) | Supported |
| Tool/function calling | Supported |
| Extended thinking | Supported |
| Reasoning output | Supported |
| Prompt caching | Supported |
| Streaming responses | Not implemented |
| Audio/Video input | Not implemented |
| Multimodal embeddings | Not implemented |
| Web search/MCP tools | Not implemented |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
volcengine-ark-sdk = { path = "../volcengine-ark-sdk" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Quick Start

```rust
use volcengine_ark_sdk::{Client, ResponseCreateParams, InputMessage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client from ARK_API_KEY env var
    let client = Client::from_env()?;

    // Send a message
    let response = client.responses().create(
        ResponseCreateParams::new(
            "ep-xxxxx",  // Your endpoint ID
            vec![InputMessage::user_text("Hello, Ark!")],
        )
    ).await?;

    println!("{}", response.text());
    Ok(())
}
```

## Examples

### Basic Text Message

```rust
use volcengine_ark_sdk::{Client, ResponseCreateParams, InputMessage};

let client = Client::from_env()?;

let response = client.responses().create(
    ResponseCreateParams::new(
        "ep-xxxxx",
        vec![InputMessage::user_text("What is the capital of France?")],
    )
    .instructions("You are a helpful geography assistant.")
    .temperature(0.7)
    .max_output_tokens(1024)
).await?;

println!("Response: {}", response.text());
```

### Image Input (Base64)

```rust
use volcengine_ark_sdk::{
    Client, ResponseCreateParams, InputMessage,
    InputContentBlock, ImageMediaType
};

let client = Client::from_env()?;

// Read and encode image
let image_data = base64::encode(std::fs::read("image.png")?);

let response = client.responses().create(
    ResponseCreateParams::new(
        "ep-xxxxx",
        vec![InputMessage::user(vec![
            InputContentBlock::image_base64(image_data, ImageMediaType::Png),
            InputContentBlock::text("What's in this image?"),
        ])],
    )
).await?;

println!("{}", response.text());
```

### Tool Use

```rust
use volcengine_ark_sdk::{
    Client, ResponseCreateParams, InputMessage,
    Tool, ToolChoice
};

let client = Client::from_env()?;

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
)?;

let response = client.responses().create(
    ResponseCreateParams::new(
        "ep-xxxxx",
        vec![InputMessage::user_text("What's the weather in Beijing?")],
    )
    .tools(vec![weather_tool])
    .tool_choice(ToolChoice::Auto)
).await?;

// Check for function calls
for (call_id, name, args) in response.function_calls() {
    println!("Function: {} ({})", name, call_id);
    println!("Arguments: {}", args);
}
```

### Extended Thinking

```rust
use volcengine_ark_sdk::{
    Client, ResponseCreateParams, InputMessage, ThinkingConfig
};

let client = Client::from_env()?;

let response = client.responses().create(
    ResponseCreateParams::new(
        "ep-xxxxx",
        vec![InputMessage::user_text("Solve this step by step: 23 * 47")],
    )
    .thinking(ThinkingConfig::enabled(2048))
).await?;

// Get thinking content
if let Some(thinking) = response.thinking() {
    println!("Thinking: {}", thinking);
}
println!("Answer: {}", response.text());
```

### Prompt Caching

```rust
use volcengine_ark_sdk::{
    Client, ResponseCreateParams, InputMessage, CachingConfig
};

let client = Client::from_env()?;

let response = client.responses().create(
    ResponseCreateParams::new(
        "ep-xxxxx",
        vec![InputMessage::user_text("Hello!")],
    )
    .store(true)
    .caching(CachingConfig { enabled: Some(true) })
).await?;

// Check caching info
if let Some(caching) = &response.caching {
    println!("Cached tokens: {:?}", caching.cached_tokens);
}
```

### Text Embeddings

```rust
use volcengine_ark_sdk::{Client, EmbeddingCreateParams, EncodingFormat};

let client = Client::from_env()?;

// Single text embedding
let response = client.embeddings().create(
    EmbeddingCreateParams::new(
        "doubao-embedding-text-240715",
        "Hello, world!"
    )
    .encoding_format(EncodingFormat::Float)
).await?;

println!("Embedding dimension: {}", response.embedding().unwrap().len());

// Multiple texts (batch)
let texts = vec!["Hello", "World", "Rust"];
let response = client.embeddings().create(
    EmbeddingCreateParams::new("doubao-embedding-text-240715", texts)
).await?;

for embedding in &response.data {
    println!("Index {}: {} dimensions", embedding.index, embedding.embedding.len());
}
println!("Total tokens used: {}", response.usage.total_tokens);
```

### Multi-turn Conversation

```rust
use volcengine_ark_sdk::{Client, ResponseCreateParams, InputMessage};

let client = Client::from_env()?;

let messages = vec![
    InputMessage::user_text("Hi, I'm Alice"),
    InputMessage::assistant_text("Hello Alice! How can I help you today?"),
    InputMessage::user_text("What's my name?"),
];

let response = client.responses().create(
    ResponseCreateParams::new("ep-xxxxx", messages)
).await?;

println!("{}", response.text());
// Output: Your name is Alice!
```

### Error Handling

```rust
use volcengine_ark_sdk::{Client, ResponseCreateParams, InputMessage, ArkError};

let client = Client::from_env()?;

match client.responses().create(
    ResponseCreateParams::new("ep-xxxxx", vec![])
).await {
    Ok(response) => println!("{}", response.text()),
    Err(ArkError::BadRequest(msg)) => eprintln!("Invalid request: {}", msg),
    Err(ArkError::RateLimited { retry_after }) => {
        eprintln!("Rate limited, retry after: {:?}", retry_after);
    }
    Err(ArkError::ContextWindowExceeded) => {
        eprintln!("Context too long, reduce input size");
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## API Reference

### Client

| Method | Description |
|--------|-------------|
| `Client::from_env()` | Create from `ARK_API_KEY` env var |
| `Client::with_api_key(key)` | Create with explicit API key |
| `Client::new(config)` | Create with full configuration |
| `client.responses()` | Get the Responses resource |
| `client.embeddings()` | Get the Embeddings resource |

### Responses Resource

| Method | Description |
|--------|-------------|
| `responses.create(params)` | Create a response |

### Embeddings Resource

| Method | Description |
|--------|-------------|
| `embeddings.create(params)` | Create text embeddings |

### Types

**Response API:**
- `InputMessage` - Input message (user/assistant/system)
- `Response` - API response with outputs
- `InputContentBlock` - Input content (text/image/function_call_output)
- `OutputContentBlock` - Output content (text/function_call/thinking)
- `OutputItem` - Output items (message/function_call/reasoning)
- `Tool` - Tool definition
- `Usage` - Token usage information
- `ThinkingConfig` - Extended thinking configuration
- `CachingConfig` - Prompt caching configuration

**Embeddings API:**
- `EmbeddingCreateParams` - Parameters for creating embeddings
- `EmbeddingInput` - Single or multiple text inputs
- `CreateEmbeddingResponse` - Embedding API response
- `Embedding` - Individual embedding result
- `EmbeddingUsage` - Token usage for embeddings
- `EncodingFormat` - Output format (float/base64)

## Configuration

| Environment Variable | Description |
|---------------------|-------------|
| `ARK_API_KEY` | API key for authentication |

### ClientConfig Options

```rust
use volcengine_ark_sdk::{Client, ClientConfig};
use std::time::Duration;

let config = ClientConfig::new("ark-xxxxx")
    .base_url("https://ark.cn-beijing.volces.com/api/v3")
    .timeout(Duration::from_secs(300))
    .max_retries(3);

let client = Client::new(config)?;
```

## Not Yet Implemented

- Streaming responses
- Multimodal embeddings (image/video)
- Audio/Video input
- Web search tools
- MCP protocol tools
- Knowledge search
- File upload
- Response retrieve/delete operations

## License

Apache-2.0
