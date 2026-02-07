# OpenAI SDK for Rust

Rust SDK for the OpenAI Responses API. This crate provides a full-featured client for interacting with OpenAI models, including streaming support.

Reference: [openai-python](https://github.com/openai/openai-python) @ `722d3fffb82e9150a16da01e432b70d126ca5254`

## Features

- **Response API** - Create, retrieve, cancel, and stream responses
- **Streaming** - Full SSE streaming with 53 event types
- **Embeddings API** - Generate text embeddings
- **Multi-turn Conversations** - Continue conversations with `previous_response_id`
- **12 Built-in Tool Types** - Web search, file search, code interpreter, computer use, custom tools, and more
- **16 Output Item Types** - Full coverage of response output variants
- **Extended Thinking** - Reasoning mode with configurable token budgets
- **Prompt Caching** - Cache system prompts for improved latency
- **Logprobs** - Token probability information

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
openai-sdk = { path = "../openai" }
```

## Quick Start

```rust
use openai_sdk::{Client, ResponseCreateParams, InputMessage};

#[tokio::main]
async fn main() -> openai_sdk::Result<()> {
    // Create client from OPENAI_API_KEY environment variable
    let client = Client::from_env()?;

    // Build request
    let params = ResponseCreateParams::new("gpt-4o", vec![
        InputMessage::user_text("What is the capital of France?")
    ]);

    // Make API call
    let response = client.responses().create(params).await?;

    // Get text response
    println!("{}", response.text());

    Ok(())
}
```

## API Reference

### Client

```rust
// From environment variable (OPENAI_API_KEY)
let client = Client::from_env()?;

// With explicit API key
let client = Client::new("sk-...");

// With custom configuration
let config = ClientConfig::new("sk-...")
    .base_url("https://api.openai.com/v1")
    .organization("org-...")
    .project("proj-...");
let client = Client::with_config(config)?;
```

### Response API

#### Create Response

```rust
let params = ResponseCreateParams::new("gpt-4o", vec![
    InputMessage::user_text("Hello!")
])
.max_output_tokens(1024)
.temperature(0.7);

let response = client.responses().create(params).await?;
```

#### Retrieve Response

```rust
let response = client.responses().retrieve("resp_abc123").await?;
```

#### Cancel Response

```rust
let response = client.responses().cancel("resp_abc123").await?;
```

### Streaming API

Stream responses in real-time with Server-Sent Events (SSE):

#### Basic Streaming

```rust
use openai_sdk::{Client, ResponseCreateParams, InputMessage, ResponseStreamEvent};

let client = Client::from_env()?;
let params = ResponseCreateParams::new("gpt-4o", vec![
    InputMessage::user_text("Write a short story about a robot.")
]);

let mut stream = client.responses().stream(params).await?;

while let Some(event) = stream.next().await {
    match event? {
        ResponseStreamEvent::OutputTextDelta { delta, .. } => {
            print!("{}", delta);
        }
        ResponseStreamEvent::ResponseCompleted { response, .. } => {
            println!("\n\nDone! Response ID: {}", response.id);
        }
        ResponseStreamEvent::Error { message, .. } => {
            eprintln!("Error: {}", message);
        }
        _ => {}
    }
}
```

#### Collect Full Response

```rust
let mut stream = client.responses().stream(params).await?;
let response = stream.collect_response().await?;
println!("{}", response.text());
```

#### Stream Text Only

```rust
let mut stream = client.responses().stream(params).await?;
let text = stream.text_deltas().await?;
println!("{}", text);
```

#### Resume Interrupted Stream

```rust
// Resume from sequence number 10
let mut stream = client.responses()
    .stream_from("resp_abc123", Some(10))
    .await?;

while let Some(event) = stream.next().await {
    // Process events starting after sequence 10
}
```

#### Use with futures Stream

```rust
use futures::StreamExt;

let stream = client.responses().stream(params).await?;
let mut event_stream = stream.into_stream();

while let Some(event) = event_stream.next().await {
    // Process events using futures combinators
}
```

### Multi-turn Conversations

```rust
// First turn
let response1 = client.responses().create(
    ResponseCreateParams::new("gpt-4o", vec![
        InputMessage::user_text("Remember the number 42")
    ])
).await?;

// Continue conversation
let response2 = client.responses().create(
    ResponseCreateParams::new("gpt-4o", vec![
        InputMessage::user_text("What number did I mention?")
    ])
    .previous_response_id(&response1.id)
).await?;
```

### Function Calling

```rust
use openai_sdk::{Tool, FunctionDefinition, InputContentBlock};

// Define function tool
let weather_tool = Tool::function(FunctionDefinition {
    name: "get_weather".into(),
    description: Some("Get current weather".into()),
    parameters: Some(serde_json::json!({
        "type": "object",
        "properties": {
            "location": { "type": "string" }
        },
        "required": ["location"]
    })),
    strict: Some(true),
});

let params = ResponseCreateParams::new("gpt-4o", vec![
    InputMessage::user_text("What's the weather in Tokyo?")
])
.tools(vec![weather_tool]);

let response = client.responses().create(params).await?;

// Check for function calls
if response.has_function_calls() {
    for (call_id, name, args) in response.function_calls() {
        println!("Call {}: {}({})", call_id, name, args);

        // Provide function output for next turn
        let result = r#"{"temp": "22C", "condition": "sunny"}"#;
        let output = InputContentBlock::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: result.to_string(),
        };
        // Include in next request...
    }
}
```

### Built-in Tools

The SDK supports 12 built-in tool types with fluent builder APIs:

```rust
use openai_sdk::{Tool, UserLocation, RankingOptions};

// Web Search
let web_search = Tool::web_search()
    .with_search_context_size("medium")
    .with_user_location(UserLocation {
        city: Some("San Francisco".into()),
        country: Some("US".into()),
        ..Default::default()
    });

// File Search
let file_search = Tool::file_search(vec!["vs_abc123".into()])
    .with_max_results(10)
    .with_ranking_options(RankingOptions {
        ranker: Some("auto".into()),
        score_threshold: Some(0.5),
    });

// Code Interpreter
let code_interpreter = Tool::code_interpreter()
    .with_container("container_abc")
    .with_environment("python:3.11");

// Computer Use
let computer = Tool::computer_use(1920, 1080)
    .with_model("computer-use-preview");

// Image Generation
let image_gen = Tool::image_generation()
    .with_size("1024x1024")
    .with_quality("hd");

// Local Shell
let shell = Tool::local_shell()
    .with_allowed_commands(vec!["ls".into(), "cat".into()]);

// MCP (Model Context Protocol)
let mcp = Tool::mcp("npx -y @anthropic-ai/mcp-server-fetch")
    .with_server_url("http://localhost:3000")
    .with_allowed_tools(vec!["fetch".into()])
    .with_require_approval("never");

// Text Editor
let editor = Tool::text_editor();

// Apply Patch
let patch = Tool::apply_patch();

// Custom Tool (with Lark grammar)
let custom = Tool::custom_with_grammar(
    "apply_patch",
    "Apply file patches",
    "lark",
    include_str!("grammar.lark"),
);

// Custom Tool (unconstrained text)
let custom_text = Tool::custom_text("my_tool", "Process free-form input");
```

### Tool Choice

Control how the model selects tools:

```rust
use openai_sdk::ToolChoice;

// Auto (default) - model decides
.tool_choice(ToolChoice::Auto)

// None - disable tool use
.tool_choice(ToolChoice::None)

// Required - must use a tool
.tool_choice(ToolChoice::Required)

// Force specific function
.tool_choice(ToolChoice::function("get_weather"))

// Force specific tool type
.tool_choice(ToolChoice::hosted_tool("web_search_preview"))
.tool_choice(ToolChoice::file_search())
.tool_choice(ToolChoice::code_interpreter())
.tool_choice(ToolChoice::web_search())
.tool_choice(ToolChoice::computer_use())
.tool_choice(ToolChoice::mcp("server_name"))
```

### Extended Thinking

Enable reasoning mode for complex tasks:

```rust
use openai_sdk::{ThinkingConfig, ReasoningSummary};

let params = ResponseCreateParams::new("o1", vec![
    InputMessage::user_text("Solve this complex math problem...")
])
.thinking(ThinkingConfig::enabled(8192))  // Budget in tokens
.reasoning(ReasoningConfig {
    effort: Some(ReasoningEffort::High),
    summary: Some(ReasoningSummary::Auto),
});

let response = client.responses().create(params).await?;

// Get reasoning content
if let Some(reasoning) = response.reasoning() {
    println!("Reasoning: {}", reasoning);
}
```

### Prompt Caching

Cache system prompts for improved latency:

```rust
use openai_sdk::{PromptCachingConfig, PromptCacheRetention};

let params = ResponseCreateParams::new("gpt-4o", messages)
    .prompt_caching(PromptCachingConfig {
        retention: Some(PromptCacheRetention::Auto),
    });

let response = client.responses().create(params).await?;

// Check cached tokens
let cached = response.cached_tokens();
println!("Used {} cached tokens", cached);
```

### Image Input

```rust
use openai_sdk::{InputMessage, InputContentBlock, ImageSource, ImageDetail};

// From URL
let message = InputMessage::user(vec![
    InputContentBlock::text("What's in this image?"),
    InputContentBlock::image_url("https://example.com/image.jpg", ImageDetail::Auto),
]);

// From base64
let message = InputMessage::user(vec![
    InputContentBlock::text("Describe this:"),
    InputContentBlock::image_base64(base64_data, ImageMediaType::Png, ImageDetail::High),
]);

// From file ID
let message = InputMessage::user(vec![
    InputContentBlock::text("Analyze this:"),
    InputContentBlock::image_file("file-abc123", ImageDetail::Low),
]);
```

### Response Helper Methods

Extract specific output types from responses:

```rust
// Text content
let text = response.text();

// Function calls
for (call_id, name, args) in response.function_calls() { ... }
let has_functions = response.has_function_calls();

// All tool calls (any type)
let has_tools = response.has_tool_calls();

// Web search results
for (call_id, status, results) in response.web_search_calls() { ... }

// File search results
for (call_id, queries, results) in response.file_search_calls() { ... }

// Computer use actions
for (call_id, action) in response.computer_calls() { ... }

// Code interpreter outputs
for (call_id, code, outputs) in response.code_interpreter_calls() { ... }

// MCP tool calls
for mcp_ref in response.mcp_calls() { ... }

// Image generation results
for (call_id, revised_prompt, result) in response.image_generation_calls() { ... }

// Local shell executions
for (call_id, action, result) in response.local_shell_calls() { ... }

// Reasoning/thinking content
let reasoning = response.reasoning();
```

### Multi-turn with Tool Outputs

Provide tool outputs to continue conversations:

```rust
use openai_sdk::InputContentBlock;

// Function call output
InputContentBlock::FunctionCallOutput {
    call_id: "call_123".into(),
    output: r#"{"result": "success"}"#.into(),
}

// Computer use output (screenshot)
InputContentBlock::ComputerCallOutput {
    call_id: "call_456".into(),
    output: ComputerCallOutputData::Screenshot { image_url: "data:...".into() },
    acknowledged_safety_checks: vec![],
}

// Web search output
InputContentBlock::WebSearchCallOutput {
    call_id: "call_789".into(),
    output: Some("Search results...".into()),
}

// Code interpreter output
InputContentBlock::CodeInterpreterCallOutput {
    call_id: "call_abc".into(),
    output: Some("Execution output...".into()),
}

// MCP tool output
InputContentBlock::McpCallOutput {
    call_id: "call_def".into(),
    output: Some("MCP result...".into()),
    error: None,
}

// Custom tool call output
InputContentBlock::custom_tool_call_output("call_xyz", "Tool result...")
```

### Embeddings API

```rust
use openai_sdk::{EmbeddingCreateParams, EncodingFormat};

let params = EmbeddingCreateParams::new(
    "text-embedding-3-small",
    "Hello, world!"
)
.dimensions(256)
.encoding_format(EncodingFormat::Float);

let response = client.embeddings().create(params).await?;

// Single embedding
if let Some(embedding) = response.embedding() {
    println!("Embedding: {:?}", embedding);
}

// Multiple embeddings
for emb in response.data {
    println!("Index {}: {} dimensions", emb.index, emb.embedding.len());
}
```

### Response Includables

Request additional data in responses:

```rust
use openai_sdk::ResponseIncludable;

let params = ResponseCreateParams::new("gpt-4o", messages)
    .include(vec![
        ResponseIncludable::FileSearchResults,
        ResponseIncludable::MessageInputImageUrls,
        ResponseIncludable::ComputerCallOutputImageUrls,
        ResponseIncludable::ReasoningEncryptedContent,
    ]);
```

### Error Handling

```rust
use openai_sdk::{OpenAIError, Result};

match client.responses().create(params).await {
    Ok(response) => println!("{}", response.text()),
    Err(OpenAIError::RateLimited { retry_after }) => {
        println!("Rate limited, retry after {:?}", retry_after);
    }
    Err(OpenAIError::ContextLengthExceeded { max_tokens }) => {
        println!("Context too long, max: {}", max_tokens);
    }
    Err(OpenAIError::QuotaExceeded) => {
        println!("API quota exceeded");
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

### Request Hooks

Intercept and modify HTTP requests before they are sent:

```rust
use openai_sdk::{Client, ClientConfig, HttpRequest, RequestHook};
use std::sync::Arc;

#[derive(Debug)]
struct CustomRequestHook;

impl RequestHook for CustomRequestHook {
    fn on_request(&self, request: &mut HttpRequest) {
        // Modify URL, headers, or body
        request.headers.insert(
            "X-Custom-Header".into(),
            "custom-value".into()
        );
    }
}

let config = ClientConfig::new("sk-...")
    .request_hook(Arc::new(CustomRequestHook));
let client = Client::new(config)?;
```

## Output Item Types

The SDK supports 16 output item types:

| Type | Description |
|------|-------------|
| `Message` | Text message response |
| `FunctionCall` | Function tool invocation |
| `Reasoning` | Extended thinking content |
| `FileSearchCall` | File search tool results |
| `WebSearchCall` | Web search tool results |
| `ComputerCall` | Computer use actions |
| `CodeInterpreterCall` | Code execution results |
| `ImageGenerationCall` | Generated images |
| `LocalShellCall` | Shell command execution |
| `McpCall` | MCP tool invocation |
| `McpListTools` | MCP tool listing |
| `McpApprovalRequest` | MCP approval requests |
| `ApplyPatchCall` | Code patch applications |
| `FunctionShellCall` | Shell function calls |
| `CustomToolCall` | Custom tool invocations |
| `Compaction` | Conversation compaction |

## Response Status

| Status | Description |
|--------|-------------|
| `Completed` | Successfully completed |
| `Failed` | Failed with error |
| `InProgress` | Currently processing |
| `Incomplete` | Stopped early (length, etc.) |
| `Cancelled` | Cancelled by user |
| `Queued` | Waiting in queue |

## Stream Events (53 Types)

All events include a `sequence_number` field for ordering.

### Lifecycle Events

| Event | Description |
|-------|-------------|
| `ResponseCreated` | Response object created |
| `ResponseInProgress` | Processing started |
| `ResponseCompleted` | Successfully completed |
| `ResponseFailed` | Failed with error |
| `ResponseIncomplete` | Stopped early |
| `ResponseQueued` | Waiting in queue |

### Text Output Events

| Event | Description |
|-------|-------------|
| `OutputTextDelta` | Incremental text content |
| `OutputTextDone` | Text content complete |
| `RefusalDelta` | Incremental refusal text |
| `RefusalDone` | Refusal complete |

### Function Call Events

| Event | Description |
|-------|-------------|
| `FunctionCallArgumentsDelta` | Incremental function arguments |
| `FunctionCallArgumentsDone` | Function arguments complete |

### Output Item Events

| Event | Description |
|-------|-------------|
| `OutputItemAdded` | New output item started |
| `OutputItemDone` | Output item complete |
| `ContentPartAdded` | New content part started |
| `ContentPartDone` | Content part complete |

### Reasoning Events

| Event | Description |
|-------|-------------|
| `ReasoningTextDelta` | Incremental reasoning text |
| `ReasoningTextDone` | Reasoning text complete |
| `ReasoningSummaryPartAdded` | Summary part started |
| `ReasoningSummaryPartDone` | Summary part complete |
| `ReasoningSummaryTextDelta` | Incremental summary text |
| `ReasoningSummaryTextDone` | Summary text complete |

### Audio Events

| Event | Description |
|-------|-------------|
| `AudioDelta` | Incremental audio data |
| `AudioDone` | Audio complete |
| `AudioTranscriptDelta` | Incremental transcript |
| `AudioTranscriptDone` | Transcript complete |

### MCP Events

| Event | Description |
|-------|-------------|
| `McpCallInProgress` | MCP call started |
| `McpCallCompleted` | MCP call complete |
| `McpCallFailed` | MCP call failed |
| `McpCallArgumentsDelta` | Incremental MCP arguments |
| `McpCallArgumentsDone` | MCP arguments complete |
| `McpListToolsInProgress` | Tool listing started |
| `McpListToolsCompleted` | Tool listing complete |
| `McpListToolsFailed` | Tool listing failed |

### Tool Call Events

| Event | Description |
|-------|-------------|
| `FileSearchCallInProgress` | File search started |
| `FileSearchCallSearching` | File search in progress |
| `FileSearchCallCompleted` | File search complete |
| `WebSearchCallInProgress` | Web search started |
| `WebSearchCallSearching` | Web search in progress |
| `WebSearchCallCompleted` | Web search complete |
| `CodeInterpreterCallInProgress` | Code interpreter started |
| `CodeInterpreterCallInterpreting` | Code executing |
| `CodeInterpreterCallCompleted` | Code interpreter complete |
| `CodeInterpreterCallCodeDelta` | Incremental code |
| `CodeInterpreterCallCodeDone` | Code complete |
| `ImageGenCallInProgress` | Image generation started |
| `ImageGenCallGenerating` | Image generating |
| `ImageGenCallPartialImage` | Partial image available |
| `ImageGenCallCompleted` | Image generation complete |
| `CustomToolCallInputDelta` | Incremental custom tool input |
| `CustomToolCallInputDone` | Custom tool input complete |

### Annotation Events

| Event | Description |
|-------|-------------|
| `OutputTextAnnotationAdded` | Text annotation added |

### Error Events

| Event | Description |
|-------|-------------|
| `Error` | Stream error occurred |

## Configuration Options

### ResponseCreateParams

| Parameter | Type | Description |
|-----------|------|-------------|
| `model` | `String` | Model ID (required) |
| `input` | `Vec<InputMessage>` | Input messages (required) |
| `max_output_tokens` | `i32` | Maximum response tokens |
| `temperature` | `f64` | Sampling temperature (0-2) |
| `top_p` | `f64` | Nucleus sampling |
| `presence_penalty` | `f64` | Presence penalty |
| `frequency_penalty` | `f64` | Frequency penalty |
| `stop` | `Vec<String>` | Stop sequences |
| `tools` | `Vec<Tool>` | Available tools |
| `tool_choice` | `ToolChoice` | Tool selection mode |
| `previous_response_id` | `String` | Continue conversation |
| `instructions` | `String` | System instructions |
| `thinking` | `ThinkingConfig` | Extended thinking |
| `reasoning` | `ReasoningConfig` | Reasoning options |
| `prompt_caching` | `PromptCachingConfig` | Caching config |
| `service_tier` | `ServiceTier` | Service tier |
| `truncation` | `Truncation` | Input truncation |
| `include` | `Vec<ResponseIncludable>` | Extra response data |
| `background` | `bool` | Background processing |
| `conversation` | `ConversationParam` | Conversation settings |

## License

MIT
