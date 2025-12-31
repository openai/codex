//! Type conversion between codex-api and google-genai types.
//!
//! **IMPORTANT**: This module must align with `google-genai/src/chat.rs` to ensure
//! round-trip consistency. The sendback format from this adapter should match what
//! `chat.rs` produces when building conversation history.
//!
//! See: `google-genai/src/chat.rs` for reference implementation of:
//! - `send_function_response_with_id()` - FunctionResponse with call ID pairing
//! - `send_function_responses_with_ids()` - Multiple function responses
//! - History management via `add_to_history()` / `curated_history`
//!
//! # Conversion Rules
//!
//! ## Input (codex-api → google-genai)
//!
//! | codex-api ResponseItem | google-genai Content/Part |
//! |------------------------|---------------------------|
//! | Message(role="user")   | Content(role="user", parts=[Part::text]) |
//! | Message(role="assistant") | Content(role="model", parts=[Part::text]) |
//! | FunctionCall           | Part::function_call (in model Content) |
//! | FunctionCallOutput     | Content(role="user", parts=[Part::function_response]) |
//! | Reasoning              | Part with thought=true, thought_signature |
//!
//! ## Output (google-genai → codex-api)
//!
//! | google-genai Part      | codex-api ResponseItem |
//! |------------------------|------------------------|
//! | Part.text (thought=false) | Message(role="assistant", OutputText) |
//! | Part.function_call     | FunctionCall |
//! | Part.thought=true      | Reasoning |

use crate::common::Prompt;
use crate::common::ResponseEvent;
use base64::Engine;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use google_genai::types::Content;
use google_genai::types::FunctionCall;
use google_genai::types::FunctionDeclaration;
use google_genai::types::FunctionResponse;
use google_genai::types::GenerateContentResponse;
use google_genai::types::Part;
use google_genai::types::Schema;
use google_genai::types::SchemaType;
use std::collections::HashMap;

/// Convert a Prompt to a list of Gemini Contents.
///
/// **Alignment with chat.rs**: The output format must match what `google-genai/src/chat.rs`
/// produces when building `curated_history`. Specifically:
/// - FunctionCall → Content(role="model", Part::function_call with id)
/// - FunctionCallOutput → Content(role="user", Part::function_response with id)
/// - Signatures are attached via `Part::thought_signature` for round-trip preservation
pub fn prompt_to_contents(prompt: &Prompt) -> Vec<Content> {
    let mut contents: Vec<Content> = Vec::new();
    let mut current_parts: Vec<Part> = Vec::new();
    let mut current_role: Option<String> = None;

    // ========== Pass 1: Collect call_signatures from all Reasoning items ==========
    // This ensures signatures are available when processing FunctionCalls,
    // regardless of item order in the input array.
    let mut collected_call_signatures: HashMap<String, String> = HashMap::new();
    for item in &prompt.input {
        if let ResponseItem::Reasoning {
            encrypted_content: Some(enc),
            ..
        } = item
        {
            if let Ok(sig_data) = serde_json::from_str::<serde_json::Value>(enc) {
                if let Some(call_sigs) = sig_data.get("call_signatures").and_then(|v| v.as_object())
                {
                    for (call_id, sig_val) in call_sigs {
                        if let Some(sig) = sig_val.as_str().filter(|s| !s.is_empty()) {
                            collected_call_signatures.insert(call_id.clone(), sig.to_string());
                        }
                    }
                }
            }
        }
    }

    // ========== Pass 2: Build Contents ==========
    // Helper to flush accumulated parts into a Content
    let flush = |contents: &mut Vec<Content>, parts: &mut Vec<Part>, role: &Option<String>| {
        if !parts.is_empty() {
            contents.push(Content {
                parts: Some(std::mem::take(parts)),
                role: role.clone(),
            });
        }
    };

    for item in &prompt.input {
        match item {
            ResponseItem::Message { role, content, .. } => {
                let gemini_role = if role == "assistant" {
                    "model".to_string()
                } else {
                    role.clone()
                };

                // Flush if role changes
                if current_role.as_ref() != Some(&gemini_role) {
                    flush(&mut contents, &mut current_parts, &current_role);
                    current_role = Some(gemini_role.clone());
                }

                // Convert content items to parts
                for content_item in content {
                    match content_item {
                        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                            current_parts.push(Part::text(text));
                        }
                        ContentItem::InputImage { image_url } => {
                            // Handle data URLs or regular URLs
                            if let Some(data_url) = parse_data_url(image_url) {
                                // Use inline_data with the base64 string directly
                                current_parts.push(Part {
                                    inline_data: Some(google_genai::types::Blob::new(
                                        &data_url.base64_data,
                                        &data_url.mime_type,
                                    )),
                                    ..Default::default()
                                });
                            } else {
                                // Regular URL - use file_data
                                current_parts.push(Part::from_uri(image_url, "image/*"));
                            }
                        }
                    }
                }
            }

            ResponseItem::FunctionCall {
                name,
                arguments,
                call_id,
                ..
            } => {
                // Function calls belong to model role
                if current_role.as_ref() != Some(&"model".to_string()) {
                    flush(&mut contents, &mut current_parts, &current_role);
                    current_role = Some("model".to_string());
                }

                // Parse arguments as JSON
                let args = serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);

                // Build Part with function_call
                let mut part = Part {
                    function_call: Some(FunctionCall {
                        id: Some(call_id.clone()),
                        name: Some(name.clone()),
                        args: Some(args),
                        partial_args: None,
                        will_continue: None,
                    }),
                    ..Default::default()
                };

                // Apply per-call signature if available (from Pass 1)
                // Signatures are base64 encoded in encrypted_content JSON
                if let Some(sig_base64) = collected_call_signatures.get(call_id) {
                    if let Ok(sig_bytes) =
                        base64::engine::general_purpose::STANDARD.decode(sig_base64)
                    {
                        part.thought_signature = Some(sig_bytes);
                    }
                }

                current_parts.push(part);
            }

            ResponseItem::FunctionCallOutput { call_id, output } => {
                // Function outputs are user role
                flush(&mut contents, &mut current_parts, &current_role);
                current_role = Some("user".to_string());

                // Convert output to response value, preferring content_items for multimodal
                let response_value = if let Some(items) = &output.content_items {
                    // Multimodal content - map to array of parts
                    let mapped: Vec<serde_json::Value> = items
                        .iter()
                        .map(|item| match item {
                            FunctionCallOutputContentItem::InputText { text } => {
                                serde_json::json!({"type": "text", "text": text})
                            }
                            FunctionCallOutputContentItem::InputImage { image_url } => {
                                serde_json::json!({
                                    "type": "image_url",
                                    "image_url": {"url": image_url}
                                })
                            }
                        })
                        .collect();
                    serde_json::json!(mapped)
                } else {
                    // Plain text - try to parse as JSON, otherwise wrap
                    serde_json::from_str(&output.content)
                        .unwrap_or_else(|_| serde_json::json!({ "result": output.content.clone() }))
                };

                current_parts.push(Part {
                    function_response: Some(FunctionResponse {
                        id: Some(call_id.clone()),
                        name: None, // Name is optional in response
                        response: Some(response_value),
                        will_continue: None,
                        scheduling: None,
                        parts: None,
                    }),
                    ..Default::default()
                });

                // Flush immediately after function response
                flush(&mut contents, &mut current_parts, &current_role);
                current_role = None;
            }

            ResponseItem::Reasoning {
                summary: _,
                content,
                encrypted_content,
                ..
            } => {
                // Reasoning belongs to model role
                if current_role.as_ref() != Some(&"model".to_string()) {
                    flush(&mut contents, &mut current_parts, &current_role);
                    current_role = Some("model".to_string());
                }

                // Extract signatures from encrypted_content JSON
                // Format: {"message_signature": "base64...", "call_signatures": {"call_id": "base64..."}}
                if let Some(encrypted) = encrypted_content {
                    if let Ok(sig_data) = serde_json::from_str::<serde_json::Value>(encrypted) {
                        // Extract message-level signature (base64 encoded)
                        if let Some(msg_sig_base64) = sig_data
                            .get("message_signature")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                        {
                            if let Ok(sig_bytes) =
                                base64::engine::general_purpose::STANDARD.decode(msg_sig_base64)
                            {
                                current_parts.push(Part::with_thought_signature(sig_bytes));
                            }
                        }
                        // Note: call_signatures are handled per-FunctionCall, not here
                    } else {
                        // Fallback: use whole string as signature (legacy format - raw bytes)
                        current_parts
                            .push(Part::with_thought_signature(encrypted.as_bytes().to_vec()));
                    }
                }

                // Convert reasoning content to thought text
                if let Some(content_items) = content {
                    for item in content_items {
                        match item {
                            ReasoningItemContent::ReasoningText { text }
                            | ReasoningItemContent::Text { text } => {
                                current_parts.push(Part {
                                    text: Some(text.clone()),
                                    thought: Some(true),
                                    ..Default::default()
                                });
                            }
                        }
                    }
                }
            }

            // Skip items that don't translate to Gemini format
            ResponseItem::LocalShellCall { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::GhostSnapshot { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::Other => {}
        }
    }

    // Flush remaining parts
    flush(&mut contents, &mut current_parts, &current_role);

    contents
}

/// Convert Gemini response to codex-api ResponseEvents.
///
/// **Alignment with chat.rs**: The output ResponseItems must be convertible back to
/// Contents that match `chat.rs` format. Key points:
/// - FunctionCall.call_id uses `call_genai_@cligen_{UUID}` prefix when server doesn't provide ID
/// - Signatures are stored in Reasoning.encrypted_content as JSON for round-trip
///
/// Returns (events, response_id) where response_id is generated for the response.
pub fn response_to_events(response: &GenerateContentResponse) -> (Vec<ResponseEvent>, String) {
    let mut events = Vec::new();

    // Generate a response_id for this response (Gemini doesn't provide one)
    let response_id = generate_uuid();

    // Get parts from first candidate
    let Some(parts) = response.parts() else {
        // Even with no parts, emit Created and Completed events
        events.push(ResponseEvent::Created);
        events.push(ResponseEvent::Completed {
            response_id: response_id.clone(),
            token_usage: extract_usage(response),
        });
        return (events, response_id);
    };

    // Emit Created event first
    events.push(ResponseEvent::Created);

    // Collect parts by type
    let mut text_parts: Vec<String> = Vec::new();
    let mut reasoning_texts: Vec<String> = Vec::new();
    let mut message_signature: Option<String> = None;
    let mut call_signatures: HashMap<String, Vec<u8>> = HashMap::new();
    let mut function_calls: Vec<ResponseItem> = Vec::new();

    for part in parts {
        // Handle thought/reasoning parts
        if part.thought == Some(true) {
            if let Some(text) = &part.text {
                reasoning_texts.push(text.clone());
            }
            if let Some(sig) = &part.thought_signature {
                // Store as message_signature (first one wins)
                // Use base64 encoding to preserve binary data without loss
                if message_signature.is_none() {
                    message_signature = Some(base64::engine::general_purpose::STANDARD.encode(sig));
                }
            }
            continue;
        }

        // Handle function calls
        if let Some(fc) = &part.function_call {
            let call_id = fc.id.clone().unwrap_or_else(generate_call_id);
            let name = fc.name.clone().unwrap_or_default();
            let arguments = fc
                .args
                .as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_default())
                .unwrap_or_default();

            // Extract per-call signature if present in part
            if let Some(sig) = &part.thought_signature {
                call_signatures.insert(call_id.clone(), sig.clone());
            }

            function_calls.push(ResponseItem::FunctionCall {
                id: Some(response_id.clone()), // Use response_id as message_id
                name,
                arguments,
                call_id,
            });
            continue;
        }

        // Handle text parts
        if let Some(text) = &part.text {
            text_parts.push(text.clone());
        }
    }

    // Emit message item first (if we have text content)
    if !text_parts.is_empty() {
        let combined_text = text_parts.join("");
        events.push(ResponseEvent::OutputItemDone(ResponseItem::Message {
            id: Some(response_id.clone()),
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: combined_text,
            }],
        }));
    }

    // Emit reasoning item (if we have reasoning content or signatures)
    if !reasoning_texts.is_empty() || message_signature.is_some() || !call_signatures.is_empty() {
        let summary: Vec<ReasoningItemReasoningSummary> = reasoning_texts
            .iter()
            .take(1)
            .map(|t| ReasoningItemReasoningSummary::SummaryText {
                text: t.clone(), // Don't truncate - preserve full text
            })
            .collect();

        let content: Option<Vec<ReasoningItemContent>> = if !reasoning_texts.is_empty() {
            Some(
                reasoning_texts
                    .iter()
                    .map(|t| ReasoningItemContent::ReasoningText { text: t.clone() })
                    .collect(),
            )
        } else {
            None
        };

        // Build encrypted_content with proper structure for round-trip
        // Use base64 encoding to preserve binary signatures without data loss
        let call_signatures_strings: HashMap<String, String> = call_signatures
            .into_iter()
            .map(|(k, v)| (k, base64::engine::general_purpose::STANDARD.encode(&v)))
            .collect();

        let encrypted_content =
            if message_signature.is_some() || !call_signatures_strings.is_empty() {
                Some(
                    serde_json::json!({
                        "message_signature": message_signature,
                        "call_signatures": call_signatures_strings
                    })
                    .to_string(),
                )
            } else {
                None
            };

        events.push(ResponseEvent::OutputItemDone(ResponseItem::Reasoning {
            id: response_id.clone(), // Use same response_id for consistency
            summary,
            content,
            encrypted_content,
        }));
    }

    // Emit function calls
    for fc in function_calls {
        events.push(ResponseEvent::OutputItemDone(fc));
    }

    // Emit Completed event at the end
    events.push(ResponseEvent::Completed {
        response_id: response_id.clone(),
        token_usage: extract_usage(response),
    });

    (events, response_id)
}

/// Extract token usage from Gemini response.
pub fn extract_usage(response: &GenerateContentResponse) -> Option<TokenUsage> {
    let usage = response.usage_metadata.as_ref()?;

    Some(TokenUsage {
        input_tokens: usage.prompt_token_count.unwrap_or(0) as i64,
        cached_input_tokens: usage.cached_content_token_count.unwrap_or(0) as i64,
        output_tokens: usage.candidates_token_count.unwrap_or(0) as i64,
        reasoning_output_tokens: usage.thoughts_token_count.unwrap_or(0) as i64,
        total_tokens: usage.total_token_count.unwrap_or(0) as i64,
    })
}

/// Convert a JSON tool definition to Gemini FunctionDeclaration.
pub fn tool_json_to_declaration(tool: &serde_json::Value) -> Option<FunctionDeclaration> {
    // Handle OpenAI-style function tool format
    let function = if tool.get("type").and_then(|t| t.as_str()) == Some("function") {
        tool.get("function")?
    } else {
        tool
    };

    let name = function.get("name")?.as_str()?;
    let description = function.get("description").and_then(|d| d.as_str());

    let mut decl = FunctionDeclaration::new(name);

    if let Some(desc) = description {
        decl = decl.with_description(desc);
    }

    // Convert parameters schema
    if let Some(params) = function.get("parameters") {
        if let Some(schema) = json_schema_to_gemini(params) {
            decl = decl.with_parameters(schema);
        }
    }

    Some(decl)
}

/// Convert JSON Schema to Gemini Schema.
fn json_schema_to_gemini(json: &serde_json::Value) -> Option<Schema> {
    let schema_type = match json.get("type").and_then(|t| t.as_str()) {
        Some("string") => SchemaType::String,
        Some("number") => SchemaType::Number,
        Some("integer") => SchemaType::Integer,
        Some("boolean") => SchemaType::Boolean,
        Some("array") => SchemaType::Array,
        Some("object") => SchemaType::Object,
        Some("null") => SchemaType::Null,
        _ => return None,
    };

    let mut schema = Schema {
        schema_type: Some(schema_type),
        ..Default::default()
    };

    // Add description
    if let Some(desc) = json.get("description").and_then(|d| d.as_str()) {
        schema.description = Some(desc.to_string());
    }

    // Handle enum values
    if let Some(enum_vals) = json.get("enum").and_then(|e| e.as_array()) {
        schema.enum_values = Some(
            enum_vals
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        );
    }

    // Handle object properties
    if let Some(props) = json.get("properties").and_then(|p| p.as_object()) {
        let mut properties = HashMap::new();
        for (key, value) in props {
            if let Some(prop_schema) = json_schema_to_gemini(value) {
                properties.insert(key.clone(), prop_schema);
            }
        }
        if !properties.is_empty() {
            schema.properties = Some(properties);
        }
    }

    // Handle required fields
    if let Some(required) = json.get("required").and_then(|r| r.as_array()) {
        schema.required = Some(
            required
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        );
    }

    // Handle array items
    if let Some(items) = json.get("items") {
        if let Some(items_schema) = json_schema_to_gemini(items) {
            schema.items = Some(Box::new(items_schema));
        }
    }

    Some(schema)
}

/// Parse a data URL into mime type and base64 data string.
/// Returns (mime_type, base64_data) if valid.
struct DataUrl {
    mime_type: String,
    base64_data: String,
}

fn parse_data_url(url: &str) -> Option<DataUrl> {
    if !url.starts_with("data:") {
        return None;
    }

    let rest = url.strip_prefix("data:")?;
    let (header, data) = rest.split_once(',')?;

    let mime_type = if header.contains(';') {
        header.split(';').next()?.to_string()
    } else {
        header.to_string()
    };

    // Only support base64-encoded data URLs
    if !header.contains("base64") {
        return None;
    }

    Some(DataUrl {
        mime_type,
        base64_data: data.to_string(),
    })
}

/// Prefix for client-generated call IDs (distinguishes from server-provided IDs).
const CLIENT_GENERATED_CALL_ID_PREFIX: &str = "call_genai_@cligen_";

/// Check if a call_id was generated by the client (adapter).
pub fn is_client_generated_call_id(call_id: &str) -> bool {
    call_id.starts_with(CLIENT_GENERATED_CALL_ID_PREFIX)
}

/// Generate a call_id with special prefix to mark it as client-generated.
fn generate_call_id() -> String {
    format!(
        "{}{}",
        CLIENT_GENERATED_CALL_ID_PREFIX,
        uuid::Uuid::new_v4()
    )
}

/// Generate a proper UUID v4 string (for response_id, not call_id).
fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_prompt_to_contents_simple_message() {
        let prompt = Prompt {
            instructions: String::new(),
            input: vec![ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Hello".to_string(),
                }],
            }],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        let contents = prompt_to_contents(&prompt);

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, Some("user".to_string()));
        let parts = contents[0].parts.as_ref().unwrap();
        assert_eq!(parts[0].text, Some("Hello".to_string()));
    }

    #[test]
    fn test_prompt_to_contents_assistant_becomes_model() {
        let prompt = Prompt {
            instructions: String::new(),
            input: vec![ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "Hi there".to_string(),
                }],
            }],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        let contents = prompt_to_contents(&prompt);

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, Some("model".to_string()));
    }

    #[test]
    fn test_prompt_to_contents_function_call() {
        let prompt = Prompt {
            instructions: String::new(),
            input: vec![ResponseItem::FunctionCall {
                id: None,
                name: "get_weather".to_string(),
                arguments: r#"{"location":"Tokyo"}"#.to_string(),
                call_id: "call-123".to_string(),
            }],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        let contents = prompt_to_contents(&prompt);

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, Some("model".to_string()));
        let parts = contents[0].parts.as_ref().unwrap();
        let fc = parts[0].function_call.as_ref().unwrap();
        assert_eq!(fc.id, Some("call-123".to_string()));
        assert_eq!(fc.name, Some("get_weather".to_string()));
    }

    #[test]
    fn test_prompt_to_contents_function_output() {
        let prompt = Prompt {
            instructions: String::new(),
            input: vec![ResponseItem::FunctionCallOutput {
                call_id: "call-123".to_string(),
                output: FunctionCallOutputPayload {
                    content: r#"{"temp": 20}"#.to_string(),
                    content_items: None,
                    success: Some(true),
                },
            }],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        let contents = prompt_to_contents(&prompt);

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, Some("user".to_string()));
        let parts = contents[0].parts.as_ref().unwrap();
        let fr = parts[0].function_response.as_ref().unwrap();
        assert_eq!(fr.id, Some("call-123".to_string()));
    }

    #[test]
    fn test_prompt_to_contents_function_output_multimodal() {
        let prompt = Prompt {
            instructions: String::new(),
            input: vec![ResponseItem::FunctionCallOutput {
                call_id: "call-img".to_string(),
                output: FunctionCallOutputPayload {
                    content: "fallback text".to_string(),
                    content_items: Some(vec![
                        FunctionCallOutputContentItem::InputText {
                            text: "Caption".to_string(),
                        },
                        FunctionCallOutputContentItem::InputImage {
                            image_url: "data:image/png;base64,abc123".to_string(),
                        },
                    ]),
                    success: Some(true),
                },
            }],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        let contents = prompt_to_contents(&prompt);

        assert_eq!(contents.len(), 1);
        let parts = contents[0].parts.as_ref().unwrap();
        let fr = parts[0].function_response.as_ref().unwrap();
        assert_eq!(fr.id, Some("call-img".to_string()));

        // Verify response uses content_items, not fallback
        let response = fr.response.as_ref().unwrap();
        let items = response.as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["type"], "text");
        assert_eq!(items[0]["text"], "Caption");
        assert_eq!(items[1]["type"], "image_url");
        assert_eq!(items[1]["image_url"]["url"], "data:image/png;base64,abc123");
    }

    #[test]
    fn test_tool_json_to_declaration() {
        let tool = serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get the weather for a location",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "The city name"
                        }
                    },
                    "required": ["location"]
                }
            }
        });

        let decl = tool_json_to_declaration(&tool).unwrap();

        assert_eq!(decl.name, Some("get_weather".to_string()));
        assert_eq!(
            decl.description,
            Some("Get the weather for a location".to_string())
        );
        assert!(decl.parameters.is_some());
    }

    #[test]
    fn test_parse_data_url() {
        let url = "data:image/png;base64,iVBORw0KGgo=";
        let result = parse_data_url(url).unwrap();

        assert_eq!(result.mime_type, "image/png");
        assert_eq!(result.base64_data, "iVBORw0KGgo=");
    }

    #[test]
    fn test_parse_non_data_url_returns_none() {
        let url = "https://example.com/image.png";
        assert!(parse_data_url(url).is_none());
    }

    #[test]
    fn test_extract_usage() {
        let response = GenerateContentResponse {
            candidates: None,
            prompt_feedback: None,
            usage_metadata: Some(google_genai::types::UsageMetadata {
                prompt_token_count: Some(10),
                candidates_token_count: Some(20),
                total_token_count: Some(30),
                cached_content_token_count: None,
                thoughts_token_count: None,
                tool_use_prompt_token_count: None,
                prompt_tokens_details: None,
                cache_tokens_details: None,
                candidates_tokens_details: None,
            }),
            model_version: None,
            response_id: None,
            create_time: None,
        };

        let usage = extract_usage(&response).unwrap();

        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    // ========== P2 Tests: call_id format and round-trip consistency ==========

    #[test]
    fn test_generated_call_id_has_prefix() {
        let call_id = generate_call_id();
        assert!(call_id.starts_with("call_genai_@cligen_"));
        assert!(is_client_generated_call_id(&call_id));
    }

    #[test]
    fn test_server_call_id_not_detected_as_client() {
        assert!(!is_client_generated_call_id("server_call_123"));
        assert!(!is_client_generated_call_id("call_abc")); // No @cligen
        assert!(!is_client_generated_call_id("genai_call_123")); // Wrong prefix
    }

    #[test]
    fn test_round_trip_single_function_call() {
        // Input: ResponseItem::FunctionCall with call_id "call_abc"
        let prompt = Prompt {
            instructions: String::new(),
            input: vec![ResponseItem::FunctionCall {
                id: Some("resp-1".to_string()),
                name: "get_weather".to_string(),
                arguments: r#"{"location":"Tokyo"}"#.to_string(),
                call_id: "call_abc".to_string(),
            }],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        // Convert to Contents
        let contents = prompt_to_contents(&prompt);

        // Verify Part.function_call.id == "call_abc"
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, Some("model".to_string()));
        let parts = contents[0].parts.as_ref().unwrap();
        assert_eq!(parts.len(), 1);
        let fc = parts[0].function_call.as_ref().unwrap();
        assert_eq!(fc.id, Some("call_abc".to_string()));
        assert_eq!(fc.name, Some("get_weather".to_string()));
    }

    #[test]
    fn test_round_trip_multiple_function_calls_with_signatures() {
        use base64::Engine;

        // Create Reasoning with encrypted_content containing call_signatures (base64 encoded)
        // Create FunctionCall items with call_ids "call_1" and "call_2"
        let sig_msg_b64 = base64::engine::general_purpose::STANDARD.encode(b"sig_msg");
        let sig_1_b64 = base64::engine::general_purpose::STANDARD.encode(b"sig_1");
        let sig_2_b64 = base64::engine::general_purpose::STANDARD.encode(b"sig_2");

        let prompt = Prompt {
            instructions: String::new(),
            input: vec![
                // Reasoning comes BEFORE FunctionCalls (tests two-pass approach)
                ResponseItem::Reasoning {
                    id: "resp-1".to_string(),
                    summary: vec![],
                    content: None,
                    encrypted_content: Some(
                        serde_json::json!({
                            "message_signature": sig_msg_b64,
                            "call_signatures": {
                                "call_1": sig_1_b64,
                                "call_2": sig_2_b64
                            }
                        })
                        .to_string(),
                    ),
                },
                ResponseItem::FunctionCall {
                    id: Some("resp-1".to_string()),
                    name: "tool_a".to_string(),
                    arguments: "{}".to_string(),
                    call_id: "call_1".to_string(),
                },
                ResponseItem::FunctionCall {
                    id: Some("resp-1".to_string()),
                    name: "tool_b".to_string(),
                    arguments: "{}".to_string(),
                    call_id: "call_2".to_string(),
                },
            ],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        let contents = prompt_to_contents(&prompt);

        // Should have model content with reasoning + function calls
        assert!(!contents.is_empty());
        let model_content = contents
            .iter()
            .find(|c| c.role == Some("model".to_string()));
        assert!(model_content.is_some());

        let parts = model_content.unwrap().parts.as_ref().unwrap();

        // Find function call parts and verify signatures
        let fc_parts: Vec<_> = parts.iter().filter(|p| p.function_call.is_some()).collect();
        assert_eq!(fc_parts.len(), 2);

        // Verify call_1 has signature "sig_1"
        let call_1_part = fc_parts
            .iter()
            .find(|p| {
                p.function_call
                    .as_ref()
                    .and_then(|fc| fc.id.as_ref())
                    .map(|id| id == "call_1")
                    .unwrap_or(false)
            })
            .unwrap();
        assert_eq!(
            call_1_part.thought_signature,
            Some(b"sig_1".to_vec()),
            "call_1 should have signature sig_1"
        );

        // Verify call_2 has signature "sig_2"
        let call_2_part = fc_parts
            .iter()
            .find(|p| {
                p.function_call
                    .as_ref()
                    .and_then(|fc| fc.id.as_ref())
                    .map(|id| id == "call_2")
                    .unwrap_or(false)
            })
            .unwrap();
        assert_eq!(
            call_2_part.thought_signature,
            Some(b"sig_2".to_vec()),
            "call_2 should have signature sig_2"
        );
    }

    #[test]
    fn test_function_call_missing_id_gets_client_generated() {
        use google_genai::types::Candidate;

        // Create GenerateContentResponse with FunctionCall.id = None
        let response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![Part {
                        function_call: Some(FunctionCall {
                            id: None, // No server-provided ID
                            name: Some("get_weather".to_string()),
                            args: Some(serde_json::json!({"location": "Tokyo"})),
                            partial_args: None,
                            will_continue: None,
                        }),
                        ..Default::default()
                    }]),
                }),
                finish_reason: None,
                safety_ratings: None,
                index: None,
                token_count: None,
                avg_logprobs: None,
                citation_metadata: None,
                finish_message: None,
                grounding_metadata: None,
                logprobs_result: None,
            }]),
            prompt_feedback: None,
            usage_metadata: None,
            model_version: None,
            response_id: None,
            create_time: None,
        };

        // Convert via response_to_events
        let (events, _response_id) = response_to_events(&response);

        // Find FunctionCall event
        let fc_event = events.iter().find(|e| {
            matches!(
                e,
                crate::common::ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { .. })
            )
        });
        assert!(fc_event.is_some(), "Should have FunctionCall event");

        if let crate::common::ResponseEvent::OutputItemDone(ResponseItem::FunctionCall {
            call_id,
            ..
        }) = fc_event.unwrap()
        {
            // Verify call_id starts with client-generated prefix
            assert!(
                call_id.starts_with("call_genai_@cligen_"),
                "Missing server ID should get client-generated prefix, got: {}",
                call_id
            );
        } else {
            panic!("Expected FunctionCall event");
        }
    }

    #[test]
    fn test_multi_turn_conversation_consistency() {
        use base64::Engine;

        // Turn 1: User "Hello" → Model "Let me search" + FunctionCall(get_weather)
        // Turn 2: FunctionCallOutput(temp=20) → continue conversation

        // Base64 encode signatures for proper roundtrip
        let sig_msg_1_b64 = base64::engine::general_purpose::STANDARD.encode(b"sig_msg_1");
        let sig_weather_b64 = base64::engine::general_purpose::STANDARD.encode(b"sig_weather");

        let prompt = Prompt {
            instructions: "You are a helpful assistant.".to_string(),
            input: vec![
                // User message
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "What's the weather in Tokyo?".to_string(),
                    }],
                },
                // Model response with function call
                ResponseItem::Message {
                    id: Some("resp-1".to_string()),
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "Let me check the weather for you.".to_string(),
                    }],
                },
                // Reasoning with signatures (base64 encoded)
                ResponseItem::Reasoning {
                    id: "resp-1".to_string(),
                    summary: vec![],
                    content: None,
                    encrypted_content: Some(
                        serde_json::json!({
                            "message_signature": sig_msg_1_b64,
                            "call_signatures": {"call_weather": sig_weather_b64}
                        })
                        .to_string(),
                    ),
                },
                // Function call
                ResponseItem::FunctionCall {
                    id: Some("resp-1".to_string()),
                    name: "get_weather".to_string(),
                    arguments: r#"{"location":"Tokyo"}"#.to_string(),
                    call_id: "call_weather".to_string(),
                },
                // Function output
                ResponseItem::FunctionCallOutput {
                    call_id: "call_weather".to_string(),
                    output: FunctionCallOutputPayload {
                        content: r#"{"temperature": 20, "condition": "sunny"}"#.to_string(),
                        content_items: None,
                        success: Some(true),
                    },
                },
            ],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        // Convert to Contents
        let contents = prompt_to_contents(&prompt);

        // Verify structure:
        // 1. User content with question
        // 2. Model content with text + reasoning signature + function call with signature
        // 3. User content with function response

        // Should have at least 3 contents (user, model, user for function response)
        assert!(contents.len() >= 2, "Should have multiple contents");

        // First content should be user role
        assert_eq!(contents[0].role, Some("user".to_string()));

        // Model content should have function call with signature
        let model_content = contents
            .iter()
            .find(|c| c.role == Some("model".to_string()));
        assert!(model_content.is_some(), "Should have model content");

        let model_parts = model_content.unwrap().parts.as_ref().unwrap();
        let fc_part = model_parts.iter().find(|p| p.function_call.is_some());
        assert!(fc_part.is_some(), "Model should have function call");

        // Verify function call has signature from encrypted_content
        let fc_part = fc_part.unwrap();
        assert_eq!(
            fc_part.thought_signature,
            Some(b"sig_weather".to_vec()),
            "Function call should have its signature"
        );

        // Verify function response is in user role
        let fn_response_content = contents.iter().find(|c| {
            c.parts.as_ref().map_or(false, |parts| {
                parts.iter().any(|p| p.function_response.is_some())
            })
        });
        assert!(
            fn_response_content.is_some(),
            "Should have function response"
        );
        assert_eq!(
            fn_response_content.unwrap().role,
            Some("user".to_string()),
            "Function response should be in user role"
        );
    }

    #[test]
    fn test_sendback_format_matches_chat_rs() {
        // Build a history that matches what chat.rs would produce
        // via send_function_response_with_id

        // Create FunctionCall response then convert back to Contents
        let prompt = Prompt {
            instructions: String::new(),
            input: vec![
                // Previous model response with function call
                ResponseItem::FunctionCall {
                    id: Some("resp-1".to_string()),
                    name: "get_weather".to_string(),
                    arguments: r#"{"location":"Tokyo"}"#.to_string(),
                    call_id: "call_123".to_string(),
                },
                // Function response (like chat.rs send_function_response_with_id)
                ResponseItem::FunctionCallOutput {
                    call_id: "call_123".to_string(),
                    output: FunctionCallOutputPayload {
                        content: r#"{"temp": 20}"#.to_string(),
                        content_items: None,
                        success: Some(true),
                    },
                },
            ],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        let contents = prompt_to_contents(&prompt);

        // chat.rs would produce:
        // - Content(role="model", parts=[Part{function_call: {id: "call_123", ...}}])
        // - Content(role="user", parts=[Part{function_response: {id: "call_123", ...}}])

        // Verify model content with function_call
        let model_content = contents
            .iter()
            .find(|c| c.role == Some("model".to_string()));
        assert!(model_content.is_some());
        let model_parts = model_content.unwrap().parts.as_ref().unwrap();
        let fc = model_parts
            .iter()
            .find(|p| p.function_call.is_some())
            .unwrap()
            .function_call
            .as_ref()
            .unwrap();
        assert_eq!(fc.id, Some("call_123".to_string()));

        // Verify user content with function_response (matches chat.rs format)
        let user_content = contents
            .iter()
            .find(|c| {
                c.role == Some("user".to_string())
                    && c.parts.as_ref().map_or(false, |p| {
                        p.iter().any(|part| part.function_response.is_some())
                    })
            })
            .expect("Should have user content with function_response");

        let fr_part = user_content
            .parts
            .as_ref()
            .unwrap()
            .iter()
            .find(|p| p.function_response.is_some())
            .unwrap();
        let fr = fr_part.function_response.as_ref().unwrap();
        assert_eq!(
            fr.id,
            Some("call_123".to_string()),
            "FunctionResponse.id should match call_id"
        );
    }

    // ========== Python SDK Alignment Tests ==========

    #[test]
    fn test_reasoning_roundtrip_with_call_signatures() {
        use google_genai::types::Candidate;

        // Simulate Gemini response with thought parts and function calls with signatures
        let gemini_response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![
                        // Thought part with signature (message-level)
                        Part {
                            thought: Some(true),
                            thought_signature: Some(b"msg_sig_123".to_vec()),
                            text: Some("I'm reasoning about this...".to_string()),
                            ..Default::default()
                        },
                        // Function call part with its own signature
                        Part {
                            function_call: Some(FunctionCall {
                                id: Some("call_1".to_string()),
                                name: Some("search".to_string()),
                                args: Some(serde_json::json!({"query": "rust programming"})),
                                partial_args: None,
                                will_continue: None,
                            }),
                            thought_signature: Some(b"call_1_sig_456".to_vec()),
                            ..Default::default()
                        },
                        // Text output
                        Part {
                            text: Some("Here's what I found.".to_string()),
                            ..Default::default()
                        },
                    ]),
                }),
                ..Default::default()
            }]),
            prompt_feedback: None,
            usage_metadata: None,
            model_version: None,
            response_id: None,
            create_time: None,
        };

        // Convert Gemini response -> codex-api events
        let (events, response_id) = response_to_events(&gemini_response);

        // Verify we get expected events
        assert!(!response_id.is_empty(), "Should generate response_id");

        // Extract ResponseItems from events
        let items: Vec<ResponseItem> = events
            .iter()
            .filter_map(|e| match e {
                ResponseEvent::OutputItemDone(item) => Some(item.clone()),
                _ => None,
            })
            .collect();

        // Should have: Message (text), Reasoning (thought), FunctionCall
        assert!(
            items.len() >= 2,
            "Should have multiple items, got: {:?}",
            items.len()
        );

        // Find Reasoning item and verify encrypted_content has signatures
        let reasoning = items
            .iter()
            .find(|item| matches!(item, ResponseItem::Reasoning { .. }));
        assert!(reasoning.is_some(), "Should have Reasoning item");

        if let Some(ResponseItem::Reasoning {
            encrypted_content, ..
        }) = reasoning
        {
            assert!(encrypted_content.is_some(), "Should have encrypted_content");
            let enc = encrypted_content.as_ref().unwrap();
            let sig_data: serde_json::Value = serde_json::from_str(enc).expect("valid JSON");

            // Verify message_signature is present
            assert!(
                sig_data.get("message_signature").is_some(),
                "Should have message_signature"
            );

            // Verify call_signatures contains call_1
            let call_sigs = sig_data.get("call_signatures").and_then(|v| v.as_object());
            assert!(call_sigs.is_some(), "Should have call_signatures");
        }

        // Now convert back: codex-api items -> Gemini Contents
        let prompt = Prompt {
            instructions: String::new(),
            input: items,
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        let contents = prompt_to_contents(&prompt);

        // Find model content with function call
        let model_content = contents
            .iter()
            .find(|c| c.role == Some("model".to_string()));
        assert!(
            model_content.is_some(),
            "Should have model content in roundtrip"
        );

        let model_parts = model_content.unwrap().parts.as_ref().unwrap();

        // Find function call part and verify signature is preserved
        let fc_part = model_parts.iter().find(|p| p.function_call.is_some());
        assert!(fc_part.is_some(), "Should have function call part");

        let fc_part = fc_part.unwrap();
        // The signature should be preserved in the roundtrip
        // Note: signature is stored as bytes, convert from the call_signatures
        assert!(
            fc_part.thought_signature.is_some(),
            "Function call should preserve its signature in roundtrip"
        );
    }

    #[test]
    fn test_parallel_function_calls_response() {
        use google_genai::types::Candidate;

        // Gemini response with multiple parallel function calls
        let gemini_response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![
                        Part {
                            function_call: Some(FunctionCall {
                                id: Some("call_a".to_string()),
                                name: Some("get_weather".to_string()),
                                args: Some(serde_json::json!({"city": "Tokyo"})),
                                partial_args: None,
                                will_continue: None,
                            }),
                            ..Default::default()
                        },
                        Part {
                            function_call: Some(FunctionCall {
                                id: Some("call_b".to_string()),
                                name: Some("get_time".to_string()),
                                args: Some(serde_json::json!({"timezone": "JST"})),
                                partial_args: None,
                                will_continue: None,
                            }),
                            ..Default::default()
                        },
                    ]),
                }),
                ..Default::default()
            }]),
            prompt_feedback: None,
            usage_metadata: None,
            model_version: None,
            response_id: None,
            create_time: None,
        };

        let (events, _) = response_to_events(&gemini_response);

        // Count FunctionCall events
        let fc_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { .. })
                )
            })
            .collect();

        assert_eq!(fc_events.len(), 2, "Should have 2 function call events");

        // Verify call_ids are preserved
        let call_ids: Vec<String> = fc_events
            .iter()
            .filter_map(|e| {
                if let ResponseEvent::OutputItemDone(ResponseItem::FunctionCall {
                    call_id, ..
                }) = e
                {
                    Some(call_id.clone())
                } else {
                    None
                }
            })
            .collect();

        assert!(call_ids.contains(&"call_a".to_string()));
        assert!(call_ids.contains(&"call_b".to_string()));
    }

    #[test]
    fn test_function_call_without_server_id() {
        use google_genai::types::Candidate;

        // Gemini response where FunctionCall has no id (server didn't provide one)
        let gemini_response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![Part {
                        function_call: Some(FunctionCall {
                            id: None, // No server-provided ID
                            name: Some("my_tool".to_string()),
                            args: Some(serde_json::json!({})),
                            partial_args: None,
                            will_continue: None,
                        }),
                        ..Default::default()
                    }]),
                }),
                ..Default::default()
            }]),
            prompt_feedback: None,
            usage_metadata: None,
            model_version: None,
            response_id: None,
            create_time: None,
        };

        let (events, _) = response_to_events(&gemini_response);

        // Find FunctionCall event
        let fc_event = events.iter().find(|e| {
            matches!(
                e,
                ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { .. })
            )
        });
        assert!(fc_event.is_some());

        if let Some(ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { call_id, .. })) =
            fc_event
        {
            // Should have client-generated ID with special prefix
            assert!(
                is_client_generated_call_id(call_id),
                "Missing server ID should result in client-generated ID: {}",
                call_id
            );
            assert!(call_id.starts_with("call_genai_@cligen_"));
        }
    }

    #[test]
    fn test_binary_signature_roundtrip() {
        use google_genai::types::Candidate;

        // Binary signature with non-UTF8 bytes (would be corrupted by from_utf8_lossy)
        let binary_sig = vec![0x00, 0x01, 0xFF, 0xFE, 0x80, 0x90, 0xAB, 0xCD];

        // Create Gemini response with binary signature
        let gemini_response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![
                        // Thought part with binary signature
                        Part {
                            thought: Some(true),
                            thought_signature: Some(binary_sig.clone()),
                            text: Some("Thinking...".to_string()),
                            ..Default::default()
                        },
                        // Function call with binary signature
                        Part {
                            function_call: Some(FunctionCall {
                                id: Some("call_binary".to_string()),
                                name: Some("test_tool".to_string()),
                                args: Some(serde_json::json!({})),
                                partial_args: None,
                                will_continue: None,
                            }),
                            thought_signature: Some(binary_sig.clone()),
                            ..Default::default()
                        },
                    ]),
                }),
                ..Default::default()
            }]),
            prompt_feedback: None,
            usage_metadata: None,
            model_version: None,
            response_id: None,
            create_time: None,
        };

        // Convert to codex-api events
        let (events, _) = response_to_events(&gemini_response);

        // Extract ResponseItems
        let items: Vec<ResponseItem> = events
            .iter()
            .filter_map(|e| match e {
                ResponseEvent::OutputItemDone(item) => Some(item.clone()),
                _ => None,
            })
            .collect();

        // Find Reasoning item and verify encrypted_content has base64-encoded signature
        let reasoning = items
            .iter()
            .find(|item| matches!(item, ResponseItem::Reasoning { .. }));
        assert!(reasoning.is_some(), "Should have Reasoning item");

        let encrypted_content = if let ResponseItem::Reasoning {
            encrypted_content, ..
        } = reasoning.unwrap()
        {
            encrypted_content.clone()
        } else {
            panic!("Not a Reasoning item")
        };

        assert!(encrypted_content.is_some(), "Should have encrypted_content");
        let enc = encrypted_content.unwrap();

        // Verify it's valid JSON with base64-encoded signatures
        let sig_data: serde_json::Value = serde_json::from_str(&enc).expect("valid JSON");
        let msg_sig_base64 = sig_data
            .get("message_signature")
            .and_then(|v| v.as_str())
            .unwrap();
        let call_sigs = sig_data
            .get("call_signatures")
            .and_then(|v| v.as_object())
            .unwrap();
        let call_sig_base64 = call_sigs
            .get("call_binary")
            .and_then(|v| v.as_str())
            .unwrap();

        // Verify base64 can be decoded back to original bytes
        use base64::Engine;
        let decoded_msg_sig = base64::engine::general_purpose::STANDARD
            .decode(msg_sig_base64)
            .expect("decode message sig");
        let decoded_call_sig = base64::engine::general_purpose::STANDARD
            .decode(call_sig_base64)
            .expect("decode call sig");

        assert_eq!(
            decoded_msg_sig, binary_sig,
            "Message signature should roundtrip correctly"
        );
        assert_eq!(
            decoded_call_sig, binary_sig,
            "Call signature should roundtrip correctly"
        );

        // Now test the full roundtrip: convert back to Contents
        let prompt = Prompt {
            instructions: String::new(),
            input: items,
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        let contents = prompt_to_contents(&prompt);

        // Find model content with function call and verify signature is preserved
        let model_content = contents
            .iter()
            .find(|c| c.role == Some("model".to_string()));
        assert!(model_content.is_some(), "Should have model content");

        let parts = model_content.unwrap().parts.as_ref().unwrap();
        let fc_part = parts.iter().find(|p| p.function_call.is_some());
        assert!(fc_part.is_some(), "Should have function call part");

        // Verify the binary signature was preserved through the full roundtrip
        assert_eq!(
            fc_part.unwrap().thought_signature,
            Some(binary_sig),
            "Binary signature should be preserved through full roundtrip"
        );
    }
}
