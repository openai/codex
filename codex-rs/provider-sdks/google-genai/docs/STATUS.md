# Google GenAI Rust Client - Implementation Status

Reference: [python-genai](https://github.com/googleapis/python-genai) @ `feae46dd`

## Review Summary

**Reviewed**: 2024-12-10

### Types Alignment ✅
- Request body structure matches Python: `contents`, `systemInstruction`, `generationConfig`, `tools`, `safetySettings`, `toolConfig` at top level
- camelCase serialization for all API types
- All core types (Part, Content, FunctionCall, etc.) aligned with Python SDK

### Core API Flow ✅
- `generate_content()` correctly builds request with proper field placement
- `system_instruction` placed at request root (not inside generationConfig)
- `generation_config` only populated when generation params are set

### Multi-turn Conversation ✅
- Chat maintains history and appends to subsequent requests
- History updated after each response
- Function response flow supported

## Implemented Features

### Core Types ✅
- [x] `Part` - text, inline_data (Blob), file_data, function_call, function_response, thought
- [x] `Content` - Multi-part content with role (user/model)
- [x] `Blob` - Binary data with MIME type (base64 encoded)
- [x] `FileData` - URI-based file reference
- [x] `FunctionCall` - id, name, args
- [x] `FunctionResponse` - id, name, response

### Tools ✅
- [x] `FunctionDeclaration` - name, description, parameters (Schema)
- [x] `Schema` - type, description, enum, properties, required, items
- [x] `Tool` - function_declarations, google_search, code_execution
- [x] `ToolConfig` - function_calling_config
- [x] `FunctionCallingConfig` - mode (AUTO, ANY, NONE), allowed_function_names

### Generation Config ✅
- [x] `GenerateContentConfig` - User-facing configuration
  - [x] system_instruction
  - [x] temperature, top_p, top_k
  - [x] max_output_tokens, candidate_count
  - [x] stop_sequences
  - [x] response_logprobs, logprobs
  - [x] response_mime_type, response_schema
  - [x] presence_penalty, frequency_penalty
  - [x] seed, response_modalities
  - [x] safety_settings, tools, tool_config
  - [x] thinking_config

### Response Types ✅
- [x] `GenerateContentResponse` - candidates, usage_metadata, prompt_feedback, model_version
- [x] `Candidate` - content, finish_reason, safety_ratings, index, token_count
- [x] `UsageMetadata` - prompt_token_count, candidates_token_count, total_token_count, etc.
- [x] `PromptFeedback` - block_reason, safety_ratings
- [x] Helper methods: `.text()`, `.function_calls()`, `.finish_reason()`, `.parts()`, `.thought_text()`, `.thought_signatures()`, `.has_thoughts()`

### Thinking/Reasoning ✅
- [x] `Part.thought` - Boolean indicating thought/reasoning content
- [x] `Part.thought_signature` - Opaque signature for reusing thoughts
- [x] `Part::with_thought_signature()` - Create thought part with signature
- [x] `Part::is_thought()` - Check if part is thought content
- [x] `ThinkingConfig` - `include_thoughts`, `thinking_budget`
- [x] `ThinkingConfig::with_thoughts()` - Enable thought output
- [x] `ThinkingConfig::with_budget()` - Set thinking token budget
- [x] `.text()` automatically filters out thought parts
- [x] `.thought_text()` extracts only thought content
- [x] `.thought_signatures()` gets signatures for subsequent requests

### Client ✅
- [x] `Client` - Main API client
  - [x] API key auth (env: GOOGLE_API_KEY, GEMINI_API_KEY)
  - [x] Configurable base_url, api_version, timeout
  - [x] `generate_content()` - Full request with proper structure
  - [x] `generate_content_text()` - Simple text prompt
  - [x] `generate_content_with_tools()` - Function calling
  - [x] `generate_content_stream()` - SSE streaming response
  - [x] `generate_content_stream_text()` - Simple text streaming
  - [x] `generate_content_stream_with_tools()` - Streaming with tools

### Chat ✅
- [x] `Chat` - Stateful conversation session
  - [x] Dual history: `curated_history` (valid turns) + `comprehensive_history` (all turns)
  - [x] `history()` - Get curated history (sent to API)
  - [x] `get_history(curated: bool)` - Get curated or comprehensive history
  - [x] `add_to_history()`, `clear_history()` - History management
  - [x] `send_message()` - Text message
  - [x] `send_message_with_parts()` - Custom parts with config
  - [x] `send_message_with_image()` - Image bytes
  - [x] `send_message_with_image_uri()` - Image URI
  - [x] `send_function_response()` - Single function result
  - [x] `send_function_response_with_id()` - Function result with call ID pairing
  - [x] `send_function_responses()` - Batch function results
  - [x] `send_function_responses_with_ids()` - Batch with ID pairing
  - [x] Response validation before history (invalid responses excluded from curated)
  - [x] `send_message_stream()` - Text message with streaming (manual history)
  - [x] `send_message_stream_with_config()` - Streaming with config
  - [x] `send_message_stream_with_parts()` - Multi-part streaming
  - [x] `send_message_stream_auto()` - Streaming with auto history update (Python SDK aligned)
  - [x] `send_message_stream_auto_with_parts()` - Auto history with custom parts
- [x] `ChatBuilder` - Fluent chat configuration

### Safety ✅
- [x] `SafetySetting` - category + threshold
- [x] `SafetyRating` - category + probability + blocked
- [x] All enum types: HarmCategory, HarmBlockThreshold, HarmProbability, BlockedReason, FinishReason

### Error Handling ✅
- [x] `GenAiError` - Configuration, Network, Api, Parse, Validation, ContextLengthExceeded, QuotaExceeded, ContentBlocked
- [x] Retryable error detection
- [x] Error conversion from reqwest/serde_json

### Streaming ✅
- [x] `generate_content_stream()` - SSE streaming via `streamGenerateContent` endpoint
- [x] `ContentStream` - Async stream of `GenerateContentResponse` chunks
- [x] SSE parser handles chunked delivery and `[DONE]` marker
- [x] Custom base URL support (third-party providers like ByteDance)

## Test Coverage

- 25 unit tests + 7 doc tests
- Request serialization structure verification
- Response deserialization (text, function calls)
- Type constructors (Part, Content, Tool)
- Config param detection
- Response validation (`is_valid_response`)
- Curated vs comprehensive history management
- SSE stream parsing (single/multiple events, chunked delivery, DONE marker)
- URL building (native + custom base URLs)

## Not Implemented (Out of Scope)

### Vertex AI ❌
- [ ] Vertex AI authentication (service account, ADC)
- [ ] Vertex AI endpoints
- [ ] Project/location configuration

### Advanced Features ❌
- [ ] Automatic function calling (AFC)
- [ ] MCP integration
- [ ] Caching (cached_content)
- [ ] Files/Batches/Tuning/Live APIs

## Usage Example

```rust
use google_genai::{Client, Chat, ChatBuilder, types::*};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create client (uses GOOGLE_API_KEY env var)
    let client = Client::from_env()?;

    // Simple generation
    let resp = client
        .generate_content_text("gemini-2.0-flash", "Hello!", None)
        .await?;
    println!("{}", resp.text().unwrap_or_default());

    // Chat with history
    let mut chat = ChatBuilder::new(client.clone(), "gemini-2.0-flash")
        .system_instruction("You are helpful.")
        .temperature(0.7)
        .build();

    let resp = chat.send_message("What is Rust?").await?;
    println!("{}", resp.text().unwrap_or_default());

    // Continue conversation
    let resp = chat.send_message("Tell me more").await?;
    println!("{}", resp.text().unwrap_or_default());

    // Tool calling
    let tools = vec![Tool::functions(vec![
        FunctionDeclaration::new("get_weather")
            .with_description("Get weather for a city")
            .with_parameters(
                Schema::object(HashMap::from([
                    ("city".to_string(), Schema::string()),
                ]))
                .with_required(vec!["city".to_string()])
            )
    ])];

    let resp = client
        .generate_content_with_tools(
            "gemini-2.0-flash",
            vec![Content::user("Weather in Tokyo?")],
            tools,
            None,
        )
        .await?;

    if let Some(calls) = resp.function_calls() {
        for call in calls {
            println!("Function: {:?}, Args: {:?}", call.name, call.args);

            // Send function response
            chat.add_to_history(Content::with_parts("model", vec![
                Part::function_call(call.name.clone().unwrap(), call.args.clone().unwrap())
            ]));
            let result = chat.send_function_response(
                call.name.as_ref().unwrap(),
                serde_json::json!({"temperature": "22C", "condition": "sunny"})
            ).await?;
            println!("{}", result.text().unwrap_or_default());
        }
    }

    Ok(())
}
```

## Multi-turn Tool Calling Example

Complete example showing multi-turn conversation with function calling and thought handling:

```rust
use google_genai::{Client, Chat, ChatBuilder, types::*};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?;

    // Define tools
    let get_weather = FunctionDeclaration::new("get_weather")
        .with_description("Get current weather for a location")
        .with_parameters(
            Schema::object(HashMap::from([
                ("location".to_string(), Schema::string().with_description("City name")),
            ]))
            .with_required(vec!["location".to_string()])
        );

    let search_flights = FunctionDeclaration::new("search_flights")
        .with_description("Search available flights")
        .with_parameters(
            Schema::object(HashMap::from([
                ("from".to_string(), Schema::string()),
                ("to".to_string(), Schema::string()),
                ("date".to_string(), Schema::string()),
            ]))
            .with_required(vec!["from".to_string(), "to".to_string()])
        );

    // Create chat with tools and thinking enabled
    let mut config = GenerateContentConfig::default();
    config.tools = Some(vec![Tool::functions(vec![get_weather, search_flights])]);
    config.thinking_config = Some(ThinkingConfig::with_budget(1024));
    config.temperature = Some(0.7);

    let mut chat = Chat::with_config(client, "gemini-2.0-flash-thinking-exp", config);

    // Turn 1: User asks about travel
    let resp = chat.send_message("I want to travel from Tokyo to Paris next week. What's the weather like and what flights are available?").await?;

    // Check for thoughts (if thinking model is used)
    if resp.has_thoughts() {
        println!("[Thinking]: {}", resp.thought_text().unwrap_or_default());

        // Save thought signatures for subsequent requests
        let signatures = resp.thought_signatures();
        println!("[Signatures]: {:?}", signatures);
    }

    // Process function calls
    if let Some(calls) = resp.function_calls() {
        println!("[Model requested {} function calls]", calls.len());

        // Collect model's function call parts for history
        let model_parts: Vec<Part> = calls
            .iter()
            .map(|fc| Part {
                function_call: Some((*fc).clone()),
                ..Default::default()
            })
            .collect();

        // Add model's function calls to history
        chat.add_to_history(Content::with_parts("model", model_parts));

        // Execute functions and collect responses
        let mut response_parts = Vec::new();

        for call in calls {
            let name = call.name.as_ref().unwrap();
            let args = call.args.as_ref().unwrap();

            println!("[Executing]: {}({:?})", name, args);

            // Simulate function execution
            let result = match name.as_str() {
                "get_weather" => {
                    let location = args["location"].as_str().unwrap_or("unknown");
                    serde_json::json!({
                        "location": location,
                        "temperature": "15°C",
                        "condition": "Partly cloudy",
                        "forecast": "Rain expected mid-week"
                    })
                }
                "search_flights" => {
                    serde_json::json!({
                        "flights": [
                            {"airline": "JAL", "departure": "10:00", "price": "$850"},
                            {"airline": "AirFrance", "departure": "14:30", "price": "$920"}
                        ]
                    })
                }
                _ => serde_json::json!({"error": "Unknown function"})
            };

            // Create function response part (with id if available)
            let mut fr = FunctionResponse::new(name, result);
            if let Some(id) = &call.id {
                fr = fr.with_id(id);
            }
            response_parts.push(Part {
                function_response: Some(fr),
                ..Default::default()
            });
        }

        // Turn 2: Send all function responses back
        let resp = chat.send_message_with_parts(response_parts, None).await?;
        println!("[Response]: {}", resp.text().unwrap_or_default());

        // Turn 3: Follow-up question (history maintained)
        let resp = chat.send_message("What about hotels near the Eiffel Tower?").await?;
        println!("[Follow-up]: {}", resp.text().unwrap_or_default());
    }

    // Print conversation history
    println!("\n[Conversation History: {} messages]", chat.history().len());
    for (i, content) in chat.history().iter().enumerate() {
        let role = content.role.as_ref().unwrap_or(&"?".to_string());
        let preview = content.parts.as_ref()
            .and_then(|p| p.first())
            .map(|p| {
                if p.text.is_some() { "text" }
                else if p.function_call.is_some() { "function_call" }
                else if p.function_response.is_some() { "function_response" }
                else { "other" }
            })
            .unwrap_or("empty");
        println!("  {}: {} [{}]", i + 1, role, preview);
    }

    Ok(())
}
```

### Handling Thought Signatures in Subsequent Turns

When using thinking models, you can pass thought signatures back to maintain reasoning continuity:

```rust
// After receiving response with thoughts
let resp = chat.send_message("Complex question").await?;

// Get thought signatures
let signatures = resp.thought_signatures();

// Include signatures in next turn (if needed for reasoning continuity)
if !signatures.is_empty() {
    let mut parts = vec![Part::text("Follow-up question")];
    for sig in signatures {
        parts.push(Part::with_thought_signature(sig));
    }
    let resp = chat.send_message_with_parts(parts, None).await?;
}
```

### Function Calling Flow Summary

```
Turn 1: User Message
    └─> Model returns FunctionCall parts

Turn 2: Function Responses
    ├─> Add model's FunctionCall to history
    ├─> Execute functions
    └─> Send FunctionResponse parts

Turn 3+: Continue Conversation
    └─> History includes all previous turns
```

## Wire Format

Request body structure (matches Python SDK):
```json
{
  "contents": [{"role": "user", "parts": [{"text": "Hello"}]}],
  "systemInstruction": {"parts": [{"text": "You are helpful"}]},
  "generationConfig": {
    "temperature": 0.7,
    "maxOutputTokens": 1024
  },
  "tools": [{"functionDeclarations": [...]}],
  "safetySettings": [...],
  "toolConfig": {...}
}
```

## API Compatibility

| Python API | Rust API | Status |
|------------|----------|--------|
| `client.models.generate_content()` | `client.generate_content()` | ✅ |
| `client.chats.create()` | `Chat::new()` / `ChatBuilder` | ✅ |
| `chat.send_message()` | `chat.send_message()` | ✅ |
| `chat.get_history(curated=True)` | `chat.get_history(true)` | ✅ |
| `chat.get_history(curated=False)` | `chat.get_history(false)` | ✅ |
| `response.text` | `response.text()` | ✅ |
| `response.function_calls` | `response.function_calls()` | ✅ |
| `Part.from_bytes()` | `Part::from_bytes()` | ✅ |
| `Part.from_uri()` | `Part::from_uri()` | ✅ |
| Multiple FunctionResponse with IDs | `send_function_responses_with_ids()` | ✅ |
| Response validation | `is_valid_response()` (internal) | ✅ |
| `client.models.generate_content_stream()` | `client.generate_content_stream()` | ✅ |
| `chat.send_message_stream()` | `chat.send_message_stream()` | ✅ |
| `FunctionDeclaration.from_callable()` | N/A (manual definition) | ❌ |
| Automatic Function Calling (AFC) | N/A (out of scope) | ❌ |

## Streaming Example

```rust
use google_genai::{Client, ContentStream};
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?;

    // Streaming generation
    let mut stream: ContentStream = client
        .generate_content_stream_text("gemini-2.0-flash", "Tell me a story", None)
        .await?;

    // Process chunks as they arrive
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(response) => {
                if let Some(text) = response.text() {
                    print!("{}", text);  // Print incrementally
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    println!();

    Ok(())
}
```

### Streaming with Chat (Manual History)

```rust
use google_genai::{Client, Chat};
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?;
    let mut chat = Chat::new(client, "gemini-2.0-flash");

    // Send message with streaming (manual history)
    let mut stream = chat.send_message_stream("Explain quantum computing").await?;

    let mut full_response = String::new();
    while let Some(chunk) = stream.next().await {
        if let Ok(response) = chunk {
            if let Some(text) = response.text() {
                print!("{}", text);
                full_response.push_str(&text);
            }
        }
    }
    println!();

    // Manually update history
    chat.add_to_history(google_genai::Content::user("Explain quantum computing"));
    chat.add_to_history(google_genai::Content::model(&full_response));

    Ok(())
}
```

### Streaming with Auto History (Python SDK Aligned)

```rust
use google_genai::{Client, Chat};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?;
    let mut chat = Chat::new(client, "gemini-2.0-flash");

    // Streaming with automatic history update (matches Python SDK behavior)
    let full_text = chat.send_message_stream_auto(
        "Explain quantum computing",
        |response| {
            // Called for each chunk as it arrives
            if let Some(text) = response.text() {
                print!("{}", text);
            }
        }
    ).await?;
    println!();

    // History is automatically updated! Ready for next message.
    println!("Response length: {} chars", full_text.len());
    println!("History length: {} messages", chat.history().len());

    // Continue conversation - history already includes previous turn
    let _ = chat.send_message_stream_auto(
        "Can you simplify that?",
        |response| {
            if let Some(text) = response.text() {
                print!("{}", text);
            }
        }
    ).await?;

    Ok(())
}
```

### Custom Base URL (Third-Party Providers)

```rust
use google_genai::{Client, ClientConfig};

let client = Client::new(
    ClientConfig::with_api_key("your-api-key")
        .base_url("https://search.bytedance.net/gpt/openapi/online/multimodal/crawl/google/v1")
)?;

// Use same API - streaming works with any compatible endpoint
let stream = client
    .generate_content_stream_text("gemini-2.5-flash", "Hello!", None)
    .await?;
```
