//! Type conversion between codex-api and google-genai types.
//!
//! This module stores the full `GenerateContentResponse` in `Reasoning.encrypted_content`
//! for perfect round-trip preservation. On sendback, we extract the Content directly
//! from the stored response.
//!
//! # Conversion Rules
//!
//! ## Output (google-genai → codex-api)
//!
//! | google-genai Part      | codex-api ResponseItem |
//! |------------------------|------------------------|
//! | Part.text (thought=false) | Message(role="assistant", OutputText) |
//! | Part.function_call     | FunctionCall |
//! | Part.thought=true      | Reasoning |
//!
//! The full response JSON is stored in `Reasoning.encrypted_content` as:
//! `{"__genai_full_response_body": <raw JSON>}`
//!
//! ## Input (codex-api → google-genai)
//!
//! | codex-api ResponseItem | google-genai Content/Part |
//! |------------------------|---------------------------|
//! | Message(role="user")   | Content(role="user", parts=[Part::text]) |
//! | Reasoning (with stored response) | Content extracted from stored response |
//! | FunctionCallOutput     | Content(role="user", parts=[Part::function_response]) |

use crate::common::Prompt;
use crate::common::ResponseEvent;
use crate::common_ext::EncryptedContent;
use crate::common_ext::PROVIDER_SDK_GENAI;
use crate::common_ext::enhance_server_call_id;
use crate::common_ext::extract_original_call_id;
use crate::common_ext::generate_client_call_id;
use crate::common_ext::is_client_generated_call_id;
use crate::common_ext::log_unexpected_response_item;
use crate::common_ext::parse_function_name_from_call_id;
use crate::error::ApiError;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use google_genai::types::Blob;
use google_genai::types::Content;
use google_genai::types::FinishReason;
use google_genai::types::FunctionDeclaration;
use google_genai::types::FunctionResponse;
use google_genai::types::GenerateContentResponse;
use google_genai::types::Part;
use google_genai::types::Schema;
use google_genai::types::SchemaType;
use std::collections::HashMap;
use std::collections::HashSet;

/// Convert a Prompt to a list of Gemini Contents.
///
/// This simplified implementation:
/// - Extracts full Content from stored response in `Reasoning.encrypted_content`
/// - Handles user messages directly
/// - Handles FunctionCallOutput as user role with function_response Part
///
/// # Arguments
/// - `prompt` - The codex-api Prompt
/// - `base_url` - Current API base URL (for cross-adapter detection)
/// - `model` - Current model name (for cross-adapter detection)
pub fn prompt_to_contents(prompt: &Prompt, base_url: &str, model: &str) -> Vec<Content> {
    let mut contents: Vec<Content> = Vec::new();
    let mut processed_response_ids: HashSet<String> = HashSet::new();

    for item in &prompt.input {
        match item {
            // User messages - convert to user Content
            ResponseItem::Message { role, content, .. } if role == "user" => {
                let parts: Vec<Part> = content
                    .iter()
                    .map(|c| match c {
                        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                            Part::text(text)
                        }
                        ContentItem::InputImage { image_url } => {
                            if let Some(data_url) = parse_data_url(image_url) {
                                Part {
                                    inline_data: Some(Blob::new(
                                        &data_url.base64_data,
                                        &data_url.mime_type,
                                    )),
                                    ..Default::default()
                                }
                            } else {
                                Part::from_uri(image_url, "image/*")
                            }
                        }
                    })
                    .collect();
                contents.push(Content::with_parts("user", parts));
            }

            // Reasoning with full response - extract Content directly
            ResponseItem::Reasoning {
                id: resp_id,
                encrypted_content: Some(enc),
                summary,
                content: reasoning_content,
            } => {
                if processed_response_ids.contains(resp_id) {
                    continue;
                }

                // Try to extract from adapter-format encrypted_content
                if let Some(content) = extract_full_response_content(enc, base_url, model) {
                    contents.push(content);
                    processed_response_ids.insert(resp_id.clone());
                    continue;
                }

                // Fallback: encrypted_content is native OpenAI format (not parseable as adapter)
                // Extract text from summary/content fields instead.
                // This handles OpenAI Native → Adapter switching.
                if let Some(text) = extract_text_from_reasoning(summary, reasoning_content) {
                    contents.push(Content::with_parts("model", vec![Part::text(&text)]));
                    processed_response_ids.insert(resp_id.clone());
                } else {
                    // Neither adapter-format nor text extraction worked - log and skip
                    tracing::warn!(
                        response_id = %resp_id,
                        encrypted_content_prefix = %enc.chars().take(20).collect::<String>(),
                        "Unable to extract content from Reasoning with encrypted_content, skipping"
                    );
                    processed_response_ids.insert(resp_id.clone());
                }
            }

            // Reasoning without encrypted_content - extract from summary/content
            ResponseItem::Reasoning {
                id: resp_id,
                encrypted_content: None,
                summary,
                content: reasoning_content,
            } => {
                if processed_response_ids.contains(resp_id) {
                    continue;
                }
                if let Some(text) = extract_text_from_reasoning(summary, reasoning_content) {
                    contents.push(Content::with_parts("model", vec![Part::text(&text)]));
                    processed_response_ids.insert(resp_id.clone());
                }
            }

            // Message with response_id (assistant) - skip if already extracted from Reasoning
            ResponseItem::Message {
                id: Some(resp_id),
                role,
                ..
            } if role == "assistant" => {
                if processed_response_ids.contains(resp_id) {
                    continue;
                }
                // Orphan case: no Reasoning with full response found
                // This shouldn't happen with new format, but log for debugging
                tracing::warn!(
                    response_id = resp_id,
                    "Assistant message without corresponding Reasoning item"
                );
            }

            // FunctionCall with response_id - skip if already extracted from Reasoning
            ResponseItem::FunctionCall {
                id: Some(resp_id), ..
            } => {
                if processed_response_ids.contains(resp_id) {
                    continue;
                }
                // Orphan case
                tracing::warn!(
                    response_id = resp_id,
                    "FunctionCall without corresponding Reasoning item"
                );
            }

            // FunctionCallOutput - create user Content with function_response
            ResponseItem::FunctionCallOutput { call_id, output } => {
                let response_value = convert_function_output(output);

                // Extract function name directly from enhanced call_id (no history lookup)
                let function_name = parse_function_name_from_call_id(call_id).map(String::from);

                // Determine what to send back to server
                let response_id = if is_client_generated_call_id(call_id) {
                    // Client-generated: rely on function name for matching
                    None
                } else {
                    // Server-generated (srvgen@): extract and send original call_id
                    extract_original_call_id(call_id).map(String::from)
                };

                contents.push(Content::with_parts(
                    "user",
                    vec![Part {
                        function_response: Some(FunctionResponse {
                            id: response_id,
                            name: function_name,
                            response: Some(response_value),
                            will_continue: None,
                            scheduling: None,
                            parts: None,
                        }),
                        ..Default::default()
                    }],
                ));
            }

            // Unexpected variants - log for debugging
            item @ (ResponseItem::Message { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::GhostSnapshot { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::Other) => {
                log_unexpected_response_item(item, "genai", "prompt_to_contents");
            }
        }
    }

    contents
}

/// Extract Content from stored full-response format.
///
/// Supports cross-adapter conversion: if the stored response is from a different
/// adapter (detected via base_url/model mismatch), converts via normalized format.
fn extract_full_response_content(
    encrypted_content: &str,
    current_base_url: &str,
    current_model: &str,
) -> Option<Content> {
    let ec = EncryptedContent::from_json_string(encrypted_content)?;

    // Fast path: same adapter context
    if ec.matches_context(current_base_url, current_model) {
        let response: GenerateContentResponse = ec.parse_body()?;
        return response.candidates?.first()?.content.clone();
    }

    // Cross-adapter path: normalize then convert
    let normalized = ec.to_normalized()?;
    Some(normalized_to_content(&normalized))
}

/// Extract text from Reasoning summary/content fields.
///
/// Used as fallback when encrypted_content is native OpenAI format (not parseable
/// as adapter format). This enables OpenAI Native → Adapter model switching.
fn extract_text_from_reasoning(
    summary: &[ReasoningItemReasoningSummary],
    content: &Option<Vec<ReasoningItemContent>>,
) -> Option<String> {
    // Prefer content field if available (contains full reasoning text)
    if let Some(content_items) = content {
        let texts: Vec<&str> = content_items
            .iter()
            .filter_map(|c| match c {
                ReasoningItemContent::ReasoningText { text } => Some(text.as_str()),
                ReasoningItemContent::Text { text } => Some(text.as_str()),
            })
            .collect();
        if !texts.is_empty() {
            return Some(texts.join("\n"));
        }
    }

    // Fall back to summary field
    let texts: Vec<&str> = summary
        .iter()
        .filter_map(|s| match s {
            ReasoningItemReasoningSummary::SummaryText { text } => Some(text.as_str()),
        })
        .collect();
    if !texts.is_empty() {
        return Some(texts.join("\n"));
    }

    None
}

/// Convert FunctionCallOutputPayload to JSON value for FunctionResponse.
fn convert_function_output(output: &FunctionCallOutputPayload) -> serde_json::Value {
    if let Some(items) = &output.content_items {
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
    }
}

/// Convert Gemini response to codex-api ResponseEvents.
///
/// Stores the full response JSON in `Reasoning.encrypted_content` for round-trip preservation.
/// Returns (events, response_id) where response_id is generated for the response,
/// or an error if the response indicates a blocked/truncated generation.
///
/// # Arguments
/// - `response` - The Gemini GenerateContentResponse
/// - `base_url` - The API base URL (for model switch detection)
/// - `model` - The model name (for model switch detection)
///
/// # Errors
/// - `ApiError::ContextWindowExceeded` if finish_reason is MaxTokens
/// - `ApiError::GenerationBlocked` for Safety, Recitation, Language, or unspecified reasons
pub fn response_to_events(
    response: &GenerateContentResponse,
    base_url: &str,
    model: &str,
) -> Result<(Vec<ResponseEvent>, String), ApiError> {
    // Check finish_reason for error conditions
    if let Some(reason) = response.finish_reason() {
        match reason {
            FinishReason::MaxTokens => return Err(ApiError::ContextWindowExceeded),
            FinishReason::Safety => {
                return Err(ApiError::GenerationBlocked(
                    "blocked for safety".to_string(),
                ));
            }
            FinishReason::Recitation => {
                return Err(ApiError::GenerationBlocked(
                    "blocked for recitation".to_string(),
                ));
            }
            FinishReason::Language => {
                return Err(ApiError::GenerationBlocked(
                    "unsupported language".to_string(),
                ));
            }
            FinishReason::FinishReasonUnspecified => {
                return Err(ApiError::GenerationBlocked(
                    "unspecified reason".to_string(),
                ));
            }
            // Stop → normal completion
            FinishReason::Stop => {}
            // Other blocked reasons (Blocklist, ProhibitedContent, Spii, MalformedFunctionCall, etc.)
            other => return Err(ApiError::GenerationBlocked(format!("{other:?}"))),
        }
    }

    let mut events = Vec::new();

    // Use server-provided response_id if available, otherwise generate one
    let response_id = response.response_id.clone().unwrap_or_else(generate_uuid);

    // Get raw response body from sdk_http_response for storage
    let full_response_json = response
        .sdk_http_response
        .as_ref()
        .and_then(|r| r.body.clone())
        .and_then(|body| {
            EncryptedContent::from_body_str(&body, PROVIDER_SDK_GENAI, base_url, model)
        })
        .and_then(|ec| ec.to_json_string());

    // Get parts from first candidate
    let Some(parts) = response.parts() else {
        // Even with no parts, emit Created and Completed events
        events.push(ResponseEvent::Created);
        events.push(ResponseEvent::Completed {
            response_id: response_id.clone(),
            token_usage: extract_usage(response),
        });
        return Ok((events, response_id));
    };

    // Emit Created event first
    events.push(ResponseEvent::Created);

    // Collect parts by type for event emission
    let mut text_parts: Vec<String> = Vec::new();
    let mut reasoning_texts: Vec<String> = Vec::new();
    let mut function_calls: Vec<ResponseItem> = Vec::new();
    let mut fc_index: usize = 0;

    for part in parts {
        if part.thought == Some(true) {
            if let Some(text) = &part.text {
                reasoning_texts.push(text.clone());
            }
        } else if let Some(fc) = &part.function_call {
            let name = fc.name.clone().unwrap_or_default();
            // Always embed function name in call_id for later extraction
            let call_id = match fc.id.as_ref() {
                Some(server_id) => enhance_server_call_id(server_id, &name),
                None => {
                    let id = generate_client_call_id(&name, fc_index);
                    fc_index += 1;
                    id
                }
            };
            let arguments = fc
                .args
                .as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_default())
                .unwrap_or_default();

            function_calls.push(ResponseItem::FunctionCall {
                id: Some(response_id.clone()),
                name,
                arguments,
                call_id,
            });
        } else if let Some(text) = &part.text {
            text_parts.push(text.clone());
        }
    }

    // Emit message item first (if we have text content)
    if !text_parts.is_empty() {
        events.push(ResponseEvent::OutputItemDone(ResponseItem::Message {
            id: Some(response_id.clone()),
            role: "assistant".to_string(),
            content: text_parts
                .iter()
                .map(|t| ContentItem::OutputText { text: t.clone() })
                .collect(),
            end_turn: None,
        }));
    }

    // Always emit Reasoning item with full response stored in encrypted_content
    let summary: Vec<ReasoningItemReasoningSummary> = reasoning_texts
        .iter()
        .map(|t| ReasoningItemReasoningSummary::SummaryText { text: t.clone() })
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

    events.push(ResponseEvent::OutputItemDone(ResponseItem::Reasoning {
        id: response_id.clone(),
        summary,
        content,
        encrypted_content: full_response_json,
    }));

    // Emit function calls
    for fc in function_calls {
        events.push(ResponseEvent::OutputItemDone(fc));
    }

    // Emit Completed event at the end
    events.push(ResponseEvent::Completed {
        response_id: response_id.clone(),
        token_usage: extract_usage(response),
    });

    Ok((events, response_id))
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

/// Generate a proper UUID v4 string (for response_id).
fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

// =============================================================================
// Cross-adapter conversion functions
// =============================================================================

use crate::normalized::NormalizedAssistantMessage;
use crate::normalized::NormalizedToolCall;
use google_genai::types::FunctionCall;
use serde_json::Value;

/// Extract NormalizedAssistantMessage from Genai response body JSON.
///
/// Used when switching from Genai to another adapter.
pub fn extract_normalized(body: &Value) -> Option<NormalizedAssistantMessage> {
    let response: GenerateContentResponse = serde_json::from_value(body.clone()).ok()?;
    let candidates = response.candidates?;
    let first_candidate = candidates.first()?;
    let content = first_candidate.content.as_ref()?;
    let parts = content.parts.as_ref()?;

    let mut msg = NormalizedAssistantMessage::new();

    for part in parts {
        // Thinking/reasoning content
        if part.thought == Some(true) {
            if let Some(text) = &part.text {
                msg.thinking_content
                    .get_or_insert_with(Vec::new)
                    .push(text.clone());
            }
        }
        // Function calls
        else if let Some(fc) = &part.function_call {
            let name = fc.name.clone().unwrap_or_default();
            let call_id = fc.id.clone().unwrap_or_default();
            let arguments = fc
                .args
                .as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_default())
                .unwrap_or_default();

            msg.tool_calls
                .push(NormalizedToolCall::new(call_id, name, arguments));
        }
        // Regular text content
        else if let Some(text) = &part.text {
            msg.text_content.push(text.clone());
        }
    }

    if msg.is_empty() { None } else { Some(msg) }
}

/// Convert NormalizedAssistantMessage to Genai Content.
///
/// Used when switching from another adapter to Genai.
pub fn normalized_to_content(msg: &NormalizedAssistantMessage) -> Content {
    let mut parts = Vec::new();

    // Text content
    for text in &msg.text_content {
        parts.push(Part::text(text));
    }

    // Thinking content
    if let Some(thinking) = &msg.thinking_content {
        for thought in thinking {
            parts.push(Part {
                thought: Some(true),
                text: Some(thought.clone()),
                ..Default::default()
            });
        }
    }

    // Tool calls
    for tc in &msg.tool_calls {
        let args: Option<Value> = serde_json::from_str(&tc.arguments).ok();
        parts.push(Part {
            function_call: Some(FunctionCall {
                name: Some(tc.name.clone()),
                args,
                id: if tc.call_id.is_empty() {
                    None
                } else {
                    Some(tc.call_id.clone())
                },
                ..Default::default()
            }),
            ..Default::default()
        });
    }

    Content::with_parts("model", parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common_ext::parse_call_index;
    use google_genai::types::Candidate;
    use google_genai::types::FunctionCall;
    use google_genai::types::SdkHttpResponse;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_prompt_to_contents_simple_user_message() {
        let prompt = Prompt {
            instructions: String::new(),
            input: vec![ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Hello".to_string(),
                }],
                end_turn: None,
            }],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        let contents = prompt_to_contents(&prompt, "", "");

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, Some("user".to_string()));
        let parts = contents[0].parts.as_ref().unwrap();
        assert_eq!(parts[0].text, Some("Hello".to_string()));
    }

    #[test]
    fn test_prompt_to_contents_function_output() {
        // Use enhanced server call_id format (as produced by response_to_events)
        let enhanced_call_id = enhance_server_call_id("server_call_123", "get_weather");

        let prompt = Prompt {
            instructions: String::new(),
            input: vec![
                // FunctionCall with enhanced call_id
                ResponseItem::FunctionCall {
                    id: Some("resp-1".to_string()),
                    name: "get_weather".to_string(),
                    arguments: "{}".to_string(),
                    call_id: enhanced_call_id.clone(),
                },
                ResponseItem::FunctionCallOutput {
                    call_id: enhanced_call_id,
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

        let contents = prompt_to_contents(&prompt, "", "");

        // Should have 1 content (FunctionCall is skipped as orphan, FunctionCallOutput creates user content)
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, Some("user".to_string()));
        let parts = contents[0].parts.as_ref().unwrap();
        let fr = parts[0].function_response.as_ref().unwrap();
        // Original call_id should be extracted from enhanced format
        assert_eq!(fr.id, Some("server_call_123".to_string()));
        // Function name should be parsed from enhanced call_id
        assert_eq!(fr.name, Some("get_weather".to_string()));
    }

    #[test]
    fn test_prompt_to_contents_client_generated_id_stripped() {
        // Use client-generated call_id format (as produced by response_to_events)
        let client_call_id = generate_client_call_id("search_files", 0);

        let prompt = Prompt {
            instructions: String::new(),
            input: vec![
                // FunctionCall with client-generated ID
                ResponseItem::FunctionCall {
                    id: Some("resp-1".to_string()),
                    name: "search_files".to_string(),
                    arguments: "{}".to_string(),
                    call_id: client_call_id.clone(),
                },
                ResponseItem::FunctionCallOutput {
                    call_id: client_call_id,
                    output: FunctionCallOutputPayload {
                        content: r#"{"result": "ok"}"#.to_string(),
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

        let contents = prompt_to_contents(&prompt, "", "");

        let fr = contents[0].parts.as_ref().unwrap()[0]
            .function_response
            .as_ref()
            .unwrap();
        assert_eq!(fr.id, None, "Client-generated ID should be stripped");
        assert_eq!(
            fr.name,
            Some("search_files".to_string()),
            "Name should be parsed from client-generated call_id"
        );
    }

    #[test]
    fn test_response_to_events_stores_full_response() {
        // Create a response with sdk_http_response containing the raw body
        let raw_body =
            r#"{"candidates":[{"content":{"parts":[{"text":"Hello!"}],"role":"model"}}]}"#;

        let response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![Part::text("Hello!")]),
                }),
                ..Default::default()
            }]),
            response_id: Some("resp-123".to_string()),
            sdk_http_response: Some(SdkHttpResponse::from_status_and_body(
                200,
                raw_body.to_string(),
            )),
            ..Default::default()
        };

        let (events, resp_id) = response_to_events(
            &response,
            "https://generativelanguage.googleapis.com",
            "gemini-2.0",
        )
        .unwrap();

        assert_eq!(resp_id, "resp-123");

        // Find Reasoning event and verify it contains full response
        let reasoning_event = events.iter().find(|e| {
            matches!(
                e,
                ResponseEvent::OutputItemDone(ResponseItem::Reasoning { .. })
            )
        });
        assert!(reasoning_event.is_some(), "Should have Reasoning event");

        if let ResponseEvent::OutputItemDone(ResponseItem::Reasoning {
            encrypted_content, ..
        }) = reasoning_event.unwrap()
        {
            let enc = encrypted_content
                .as_ref()
                .expect("Should have encrypted_content");
            // Verify it uses the unified format
            assert!(
                enc.contains("_full_response_body"),
                "Should contain _full_response_body key"
            );
            assert!(
                enc.contains("_provider_sdk"),
                "Should contain _provider_sdk key"
            );
            assert!(enc.contains("genai"), "Should have genai provider");
        }
    }

    #[test]
    fn test_roundtrip_with_full_response() {
        // Create a response with sdk_http_response
        let raw_body = r#"{"candidates":[{"content":{"parts":[{"text":"Hello!"},{"functionCall":{"id":"call_1","name":"test","args":{}}}],"role":"model"}}]}"#;

        let response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![
                        Part::text("Hello!"),
                        Part {
                            function_call: Some(FunctionCall {
                                id: Some("call_1".to_string()),
                                name: Some("test".to_string()),
                                args: Some(serde_json::json!({})),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    ]),
                }),
                ..Default::default()
            }]),
            response_id: Some("resp-1".to_string()),
            sdk_http_response: Some(SdkHttpResponse::from_status_and_body(
                200,
                raw_body.to_string(),
            )),
            ..Default::default()
        };

        // Convert to events
        let (events, _) = response_to_events(
            &response,
            "https://generativelanguage.googleapis.com",
            "gemini-2.0",
        )
        .unwrap();

        // Extract items from events
        let items: Vec<ResponseItem> = events
            .iter()
            .filter_map(|e| match e {
                ResponseEvent::OutputItemDone(item) => Some(item.clone()),
                _ => None,
            })
            .collect();

        // Convert back to Contents
        let prompt = Prompt {
            instructions: String::new(),
            input: items,
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };

        let contents = prompt_to_contents(&prompt, "", "");

        // Should have extracted the Content from stored response
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, Some("model".to_string()));
        let parts = contents[0].parts.as_ref().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].text, Some("Hello!".to_string()));
        assert!(parts[1].function_call.is_some());
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
            sdk_http_response: None,
        };

        let usage = extract_usage(&response).unwrap();

        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn test_client_generated_call_id() {
        let call_id = generate_client_call_id("get_weather", 0);
        assert!(call_id.starts_with("cligen@get_weather#0@"));
        assert!(is_client_generated_call_id(&call_id));
        assert_eq!(
            parse_function_name_from_call_id(&call_id),
            Some("get_weather")
        );
        assert_eq!(parse_call_index(&call_id), Some(0));
    }

    #[test]
    fn test_server_enhanced_call_id() {
        let call_id = enhance_server_call_id("server_call_123", "search_files");
        assert_eq!(call_id, "srvgen@search_files@server_call_123");
        assert!(!is_client_generated_call_id(&call_id));
        assert_eq!(
            parse_function_name_from_call_id(&call_id),
            Some("search_files")
        );
        assert_eq!(extract_original_call_id(&call_id), Some("server_call_123"));
    }

    #[test]
    fn test_convert_function_output_plain_json() {
        let output = FunctionCallOutputPayload {
            content: r#"{"temp": 20}"#.to_string(),
            content_items: None,
            success: Some(true),
        };

        let result = convert_function_output(&output);
        assert_eq!(result, serde_json::json!({"temp": 20}));
    }

    #[test]
    fn test_convert_function_output_plain_text() {
        let output = FunctionCallOutputPayload {
            content: "just some text".to_string(),
            content_items: None,
            success: Some(true),
        };

        let result = convert_function_output(&output);
        assert_eq!(result, serde_json::json!({"result": "just some text"}));
    }

    #[test]
    fn test_convert_function_output_multimodal() {
        let output = FunctionCallOutputPayload {
            content: "fallback".to_string(),
            content_items: Some(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "Caption".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,abc".to_string(),
                },
            ]),
            success: Some(true),
        };

        let result = convert_function_output(&output);
        let items = result.as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["type"], "text");
        assert_eq!(items[1]["type"], "image_url");
    }
}
