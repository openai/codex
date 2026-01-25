//! Conversion functions between codex-api and OpenAI SDK types.
//!
//! This module stores the full `Response` in `Reasoning.encrypted_content`
//! for round-trip preservation. On sendback, we extract the Content directly
//! from the stored response.

use std::collections::HashSet;

use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use openai_sdk::ImageMediaType;
use openai_sdk::IncompleteReason;
use openai_sdk::InputContentBlock;
use openai_sdk::InputMessage;
use openai_sdk::OutputContentBlock;
use openai_sdk::OutputItem;
use openai_sdk::Response;
use openai_sdk::ResponseStatus;
use openai_sdk::Tool;
use openai_sdk::ToolChoice;
use serde_json::Value;

use crate::common::Prompt;
use crate::common::ResponseEvent;
use crate::common_ext::EncryptedContent;
use crate::common_ext::PROVIDER_SDK_OPENAI;
use crate::error::ApiError;

// ============================================================================
// Request conversion: Prompt -> OpenAI messages
// ============================================================================

/// Convert a codex-api Prompt to OpenAI InputMessages and optional system instructions.
///
/// This function handles the conversion of:
/// - Reasoning with encrypted_content -> Extract Content directly from stored response
/// - User messages -> InputMessage with role="user"
/// - Assistant messages -> InputMessage with role="assistant" (skipped if already processed)
/// - FunctionCall -> Skipped if already processed, otherwise appended as text
/// - FunctionCallOutput -> InputMessage with function_call_output content
///
/// # Arguments
/// - `prompt` - The codex-api Prompt
/// - `base_url` - Current API base URL (for cross-adapter detection)
/// - `model` - Current model name (for cross-adapter detection)
pub fn prompt_to_messages(
    prompt: &Prompt,
    base_url: &str,
    model: &str,
) -> (Vec<InputMessage>, Option<String>) {
    let mut messages: Vec<InputMessage> = Vec::new();
    let mut current_assistant_content: Vec<InputContentBlock> = Vec::new();
    let mut processed_response_ids: HashSet<String> = HashSet::new();

    for item in &prompt.input {
        match item {
            // Handle Reasoning with stored full response first
            ResponseItem::Reasoning {
                id: resp_id,
                encrypted_content: Some(enc),
                summary,
                content: reasoning_content,
            } => {
                if processed_response_ids.contains(resp_id) {
                    continue;
                }
                // Flush any pending assistant content first
                flush_assistant_message(&mut messages, &mut current_assistant_content);

                // Try to extract from adapter-format encrypted_content
                if let Some(assistant_msg) = extract_full_response_message(enc, base_url, model) {
                    messages.push(assistant_msg);
                    processed_response_ids.insert(resp_id.clone());
                    continue;
                }

                // Fallback: encrypted_content is native OpenAI format (not parseable as adapter)
                // Extract text from summary/content fields and add as assistant message.
                // This handles OpenAI Native -> Adapter switching.
                if let Some(text) = extract_text_from_reasoning(summary, reasoning_content) {
                    messages.push(InputMessage::assistant(vec![InputContentBlock::text(
                        &text,
                    )]));
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
                // Flush any pending assistant content first
                flush_assistant_message(&mut messages, &mut current_assistant_content);

                if let Some(text) = extract_text_from_reasoning(summary, reasoning_content) {
                    messages.push(InputMessage::assistant(vec![InputContentBlock::text(
                        &text,
                    )]));
                    processed_response_ids.insert(resp_id.clone());
                }
            }

            // Skip assistant messages if already processed via Reasoning
            ResponseItem::Message {
                id: Some(resp_id),
                role,
                ..
            } if role == "assistant" && processed_response_ids.contains(resp_id) => {
                continue;
            }

            // Skip FunctionCall if already processed via Reasoning
            ResponseItem::FunctionCall {
                id: Some(resp_id), ..
            } if processed_response_ids.contains(resp_id) => {
                continue;
            }

            ResponseItem::Message { role, content, .. } => {
                if role == "assistant" {
                    // Continue or start assistant message
                    current_assistant_content.extend(content.iter().map(content_item_to_block));
                } else {
                    // Flush any pending assistant message first
                    flush_assistant_message(&mut messages, &mut current_assistant_content);

                    // Add user message
                    let blocks: Vec<InputContentBlock> =
                        content.iter().map(content_item_to_block).collect();
                    if !blocks.is_empty() {
                        messages.push(InputMessage::user(blocks));
                    }
                }
            }

            ResponseItem::FunctionCall {
                name, arguments, ..
            } => {
                // For OpenAI, function calls from assistant are represented as text in the conversation
                // The actual function call happens in the response. We include it as context.
                let text = format!("[Called function: {name} with arguments: {arguments}]");
                current_assistant_content.push(InputContentBlock::text(text));
            }

            ResponseItem::FunctionCallOutput { call_id, output } => {
                // Flush assistant message first (tool result must follow assistant message)
                flush_assistant_message(&mut messages, &mut current_assistant_content);

                // Add function call output as user message
                let content = function_output_to_block(call_id, output);
                messages.push(InputMessage::user(vec![content]));
            }

            // Skip types not applicable to OpenAI API
            _ => {}
        }
    }

    // Flush any remaining assistant content
    flush_assistant_message(&mut messages, &mut current_assistant_content);

    // Extract system prompt
    let system_prompt = if prompt.instructions.is_empty() {
        None
    } else {
        Some(prompt.instructions.clone())
    };

    (messages, system_prompt)
}

/// Extract InputMessage from stored OpenAI Response body.
///
/// Supports cross-adapter conversion: if the stored response is from a different
/// adapter (detected via base_url/model mismatch), converts via normalized format.
fn extract_full_response_message(
    encrypted_content: &str,
    current_base_url: &str,
    current_model: &str,
) -> Option<InputMessage> {
    let ec = EncryptedContent::from_json_string(encrypted_content)?;

    // Fast path: same adapter context
    if ec.matches_context(current_base_url, current_model) {
        let response: Response = ec.parse_body()?;

        // Convert Response.output to InputMessage content blocks
        let content: Vec<InputContentBlock> = response
            .output
            .iter()
            .flat_map(|item| match item {
                OutputItem::Message { content, .. } => content
                    .iter()
                    .filter_map(|c| match c {
                        OutputContentBlock::OutputText { text, .. } => {
                            Some(InputContentBlock::text(text))
                        }
                        OutputContentBlock::Refusal { .. } => None,
                    })
                    .collect::<Vec<_>>(),
                OutputItem::FunctionCall {
                    call_id,
                    name,
                    arguments,
                    ..
                } => {
                    // Include function call as descriptive text for model context
                    vec![InputContentBlock::text(format!(
                        "[Function call {call_id} ({name}): {arguments}]"
                    ))]
                }
                OutputItem::Reasoning { content, .. } => {
                    vec![InputContentBlock::text(content)]
                }
                _ => vec![],
            })
            .collect();

        if content.is_empty() {
            return None;
        }
        return Some(InputMessage::assistant(content));
    }

    // Cross-adapter path: normalize then convert
    let normalized = ec.to_normalized()?;
    Some(normalized_to_message(&normalized))
}

/// Extract text from Reasoning summary/content fields.
///
/// Used as fallback when encrypted_content is native OpenAI format (not parseable
/// as adapter format). This enables OpenAI Native -> Adapter model switching.
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

/// Flush the current assistant message content to the messages list.
fn flush_assistant_message(
    messages: &mut Vec<InputMessage>,
    current_content: &mut Vec<InputContentBlock>,
) {
    if !current_content.is_empty() {
        messages.push(InputMessage::assistant(std::mem::take(current_content)));
    }
}

/// Convert a ContentItem to an OpenAI InputContentBlock.
fn content_item_to_block(item: &ContentItem) -> InputContentBlock {
    match item {
        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
            InputContentBlock::text(text)
        }
        ContentItem::InputImage { image_url } => parse_image_url_to_block(image_url),
    }
}

/// Parse a MIME type string to an OpenAI ImageMediaType.
fn parse_media_type(mime_str: &str) -> ImageMediaType {
    if mime_str.contains("image/png") {
        ImageMediaType::Png
    } else if mime_str.contains("image/jpeg") {
        ImageMediaType::Jpeg
    } else if mime_str.contains("image/gif") {
        ImageMediaType::Gif
    } else if mime_str.contains("image/webp") {
        ImageMediaType::Webp
    } else {
        ImageMediaType::Png
    }
}

/// Parse an image URL (data URL or regular URL) to an OpenAI InputContentBlock.
fn parse_image_url_to_block(image_url: &str) -> InputContentBlock {
    if let Some(data_url) = image_url.strip_prefix("data:") {
        if let Some((mime_and_encoding, data)) = data_url.split_once(',') {
            let media_type = parse_media_type(mime_and_encoding);
            return InputContentBlock::image_base64(data, media_type);
        }
    }
    InputContentBlock::image_url(image_url)
}

/// Convert FunctionCallOutput to an InputContentBlock.
fn function_output_to_block(
    call_id: &str,
    output: &FunctionCallOutputPayload,
) -> InputContentBlock {
    let is_error = if output.success == Some(false) {
        Some(true)
    } else {
        None
    };
    InputContentBlock::function_call_output(call_id, &output.content, is_error)
}

// ============================================================================
// Tool conversion: JSON -> OpenAI Tool
// ============================================================================

/// Convert JSON tool definitions to OpenAI Tool structs.
///
/// Supports both OpenAI-style format:
/// ```json
/// {"type": "function", "function": {"name": "...", "description": "...", "parameters": {...}}}
/// ```
/// And direct function format:
/// ```json
/// {"name": "...", "description": "...", "parameters": {...}}
/// ```
pub fn tools_to_openai(tools: &[Value]) -> Vec<Tool> {
    tools
        .iter()
        .filter_map(|tool| {
            // Try OpenAI-style format first
            if let Some(func) = tool.get("function") {
                return tool_json_to_struct(func);
            }
            // Try direct format
            tool_json_to_struct(tool)
        })
        .collect()
}

/// Convert a single tool JSON to an OpenAI Tool struct.
fn tool_json_to_struct(json: &Value) -> Option<Tool> {
    let name = json.get("name")?.as_str()?;
    let description = json
        .get("description")
        .and_then(|d| d.as_str())
        .map(String::from);
    let parameters = json
        .get("parameters")
        .or_else(|| json.get("input_schema"))
        .cloned()
        .unwrap_or_else(|| {
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            })
        });

    Tool::function(name, description, parameters).ok()
}

/// Convert OpenAI ToolChoice enum from extra config.
///
/// Supports:
/// - String: "auto", "none", "required"
/// - Object: {"type": "function", "function": {"name": "tool_name"}}
pub fn parse_tool_choice(extra: &Option<Value>) -> Option<ToolChoice> {
    let tool_choice = extra.as_ref()?.get("tool_choice")?;

    // Handle string form
    if let Some(s) = tool_choice.as_str() {
        return match s {
            "auto" => Some(ToolChoice::Auto),
            "none" => Some(ToolChoice::None),
            "required" => Some(ToolChoice::Required),
            _ => None,
        };
    }

    // Handle object form: {"type": "function", "function": {"name": "..."}}
    if let Some(obj) = tool_choice.as_object() {
        let choice_type = obj.get("type")?.as_str()?;
        if choice_type == "function" {
            let function = obj.get("function")?;
            let name = function.get("name")?.as_str()?;
            return Some(ToolChoice::Function {
                name: name.to_string(),
            });
        }
    }

    None
}

// ============================================================================
// Response conversion: OpenAI Response -> ResponseEvents
// ============================================================================

/// Convert an OpenAI Response to codex-api ResponseEvents.
///
/// Stores the full response JSON in `Reasoning.encrypted_content` for round-trip preservation.
/// Returns a vector of events and optional token usage, or an error if the
/// response indicates a blocked/truncated generation.
///
/// # Arguments
/// - `response` - The OpenAI Response
/// - `base_url` - The API base URL (for model switch detection)
/// - `model` - The model name (for model switch detection)
///
/// # Errors
/// - `ApiError::ContextWindowExceeded` if status is Incomplete with reason MaxOutputTokens
/// - `ApiError::GenerationBlocked` for other incomplete reasons (ContentFilter, etc.)
pub fn response_to_events(
    response: &Response,
    base_url: &str,
    model: &str,
) -> Result<(Vec<ResponseEvent>, Option<TokenUsage>), ApiError> {
    // Check for incomplete/blocked response
    if response.status == ResponseStatus::Incomplete {
        if let Some(details) = &response.incomplete_details {
            match details.reason {
                Some(IncompleteReason::MaxOutputTokens) => {
                    return Err(ApiError::ContextWindowExceeded);
                }
                Some(IncompleteReason::ContentFilter) => {
                    return Err(ApiError::GenerationBlocked("content filtered".to_string()));
                }
                Some(other) => return Err(ApiError::GenerationBlocked(format!("{other:?}"))),
                None => {}
            }
        }
    }

    let mut events = Vec::new();

    // Extract raw body from sdk_http_response for storage
    let full_response_json = response
        .sdk_http_response
        .as_ref()
        .and_then(|r| r.body.clone())
        .and_then(|body| {
            EncryptedContent::from_body_str(&body, PROVIDER_SDK_OPENAI, base_url, model)
        })
        .and_then(|ec| ec.to_json_string());

    // Add Created event
    events.push(ResponseEvent::Created);

    // Collect reasoning content from all sources
    let mut reasoning_texts: Vec<String> = Vec::new();

    for item in &response.output {
        match item {
            OutputItem::Message { content, .. } => {
                // Collect text content
                let mut text_parts: Vec<String> = Vec::new();

                for block in content {
                    match block {
                        OutputContentBlock::OutputText { text, .. } => {
                            text_parts.push(text.clone());
                        }
                        OutputContentBlock::Refusal { refusal } => {
                            text_parts.push(format!("[Refusal: {refusal}]"));
                        }
                    }
                }

                // Flush text if we have any
                if !text_parts.is_empty() {
                    events.push(ResponseEvent::OutputItemDone(ResponseItem::Message {
                        id: Some(response.id.clone()),
                        role: "assistant".to_string(),
                        content: vec![ContentItem::OutputText {
                            text: text_parts.join(""),
                        }],
                        end_turn: None,
                    }));
                }
            }

            OutputItem::FunctionCall {
                call_id,
                name,
                arguments,
                ..
            } => {
                events.push(ResponseEvent::OutputItemDone(ResponseItem::FunctionCall {
                    id: Some(response.id.clone()),
                    call_id: call_id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                }));
            }

            OutputItem::Reasoning {
                content, summary, ..
            } => {
                // Collect reasoning content
                reasoning_texts.push(content.clone());
                if let Some(summaries) = summary {
                    for s in summaries {
                        reasoning_texts.push(s.text.clone());
                    }
                }
            }

            // Skip other output item types for now
            _ => {}
        }
    }

    // Always emit a Reasoning item with encrypted_content for round-trip support
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
        id: response.id.clone(),
        summary,
        content,
        encrypted_content: full_response_json,
    }));

    // Extract token usage
    let usage = extract_usage(&response.usage);

    // Add Completed event
    events.push(ResponseEvent::Completed {
        response_id: response.id.clone(),
        token_usage: Some(usage.clone()),
    });

    Ok((events, Some(usage)))
}

/// Extract token usage from OpenAI Usage.
fn extract_usage(usage: &openai_sdk::Usage) -> TokenUsage {
    TokenUsage {
        input_tokens: usage.input_tokens as i64,
        output_tokens: usage.output_tokens as i64,
        cached_input_tokens: usage.cached_tokens() as i64,
        total_tokens: usage.total_tokens as i64,
        reasoning_output_tokens: usage.reasoning_tokens() as i64,
    }
}

// ============================================================================
// Cross-adapter conversion functions
// ============================================================================

use crate::normalized::NormalizedAssistantMessage;
use crate::normalized::NormalizedToolCall;

/// Extract NormalizedAssistantMessage from OpenAI response body JSON.
///
/// Used when switching from OpenAI to another adapter.
pub fn extract_normalized(body: &Value) -> Option<NormalizedAssistantMessage> {
    let response: Response = serde_json::from_value(body.clone()).ok()?;

    let mut msg = NormalizedAssistantMessage::new();

    for item in &response.output {
        match item {
            OutputItem::Message { content, .. } => {
                for block in content {
                    if let OutputContentBlock::OutputText { text, .. } = block {
                        msg.text_content.push(text.clone());
                    }
                }
            }
            OutputItem::FunctionCall {
                call_id,
                name,
                arguments,
                ..
            } => {
                msg.tool_calls
                    .push(NormalizedToolCall::new(call_id, name, arguments));
            }
            OutputItem::Reasoning { content, .. } => {
                msg.thinking_content
                    .get_or_insert_with(Vec::new)
                    .push(content.clone());
            }
            _ => {}
        }
    }

    if msg.is_empty() { None } else { Some(msg) }
}

/// Convert NormalizedAssistantMessage to OpenAI InputMessage.
///
/// Used when switching from another adapter to OpenAI.
pub fn normalized_to_message(msg: &NormalizedAssistantMessage) -> InputMessage {
    let mut content: Vec<InputContentBlock> = Vec::new();

    // Text content
    for text in &msg.text_content {
        content.push(InputContentBlock::text(text));
    }

    // Note: Thinking content is model-generated, not sent in requests.
    // OpenAI doesn't have a way to include previous thinking in input messages.

    // Tool calls - OpenAI represents them as descriptive text in assistant messages
    for tc in &msg.tool_calls {
        content.push(InputContentBlock::text(format!(
            "[Function call {} ({}): {}]",
            tc.call_id, tc.name, tc.arguments
        )));
    }

    InputMessage::assistant(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_prompt_to_messages_simple_user() {
        let prompt = Prompt {
            instructions: "You are helpful.".to_string(),
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

        let (messages, system) = prompt_to_messages(&prompt, "", "");

        assert_eq!(messages.len(), 1);
        assert!(system.is_some());
        assert_eq!(system.unwrap(), "You are helpful.");
    }

    #[test]
    fn test_prompt_to_messages_with_function_call() {
        let prompt = Prompt {
            instructions: String::new(),
            input: vec![
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "Run a command".to_string(),
                    }],
                    end_turn: None,
                },
                ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "I'll run that for you.".to_string(),
                    }],
                    end_turn: None,
                },
                ResponseItem::FunctionCall {
                    id: None,
                    call_id: "call_123".to_string(),
                    name: "shell".to_string(),
                    arguments: r#"{"command": "ls"}"#.to_string(),
                },
                ResponseItem::FunctionCallOutput {
                    call_id: "call_123".to_string(),
                    output: FunctionCallOutputPayload {
                        content: "file1.txt\nfile2.txt".to_string(),
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

        let (messages, _) = prompt_to_messages(&prompt, "", "");

        // Should have: user, assistant (with text + function_call as text), user (function_output)
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn test_tools_to_openai_openai_format() {
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get current weather",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    },
                    "required": ["location"]
                }
            }
        })];

        let openai_tools = tools_to_openai(&tools);

        assert_eq!(openai_tools.len(), 1);
    }

    #[test]
    fn test_tools_to_openai_direct_format() {
        let tools = vec![serde_json::json!({
            "name": "search",
            "description": "Search the web",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            }
        })];

        let openai_tools = tools_to_openai(&tools);

        assert_eq!(openai_tools.len(), 1);
    }

    #[test]
    fn test_parse_image_url_data_url() {
        let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==";
        let block = parse_image_url_to_block(data_url);

        match block {
            InputContentBlock::InputImage { source } => {
                // Verify it's base64 encoded
                assert!(matches!(source, openai_sdk::ImageSource::Base64 { .. }));
            }
            _ => panic!("expected Image block"),
        }
    }

    #[test]
    fn test_parse_image_url_regular_url() {
        let url = "https://example.com/image.png";
        let block = parse_image_url_to_block(url);

        match block {
            InputContentBlock::InputImage { source } => match source {
                openai_sdk::ImageSource::Url {
                    url: parsed_url, ..
                } => {
                    assert_eq!(parsed_url, url);
                }
                _ => panic!("expected Url source"),
            },
            _ => panic!("expected Image block"),
        }
    }

    #[test]
    fn test_parse_tool_choice_string() {
        let extra = Some(serde_json::json!({"tool_choice": "auto"}));
        assert!(matches!(parse_tool_choice(&extra), Some(ToolChoice::Auto)));

        let extra = Some(serde_json::json!({"tool_choice": "none"}));
        assert!(matches!(parse_tool_choice(&extra), Some(ToolChoice::None)));

        let extra = Some(serde_json::json!({"tool_choice": "required"}));
        assert!(matches!(
            parse_tool_choice(&extra),
            Some(ToolChoice::Required)
        ));

        let extra = Some(serde_json::json!({"tool_choice": "invalid"}));
        assert!(parse_tool_choice(&extra).is_none());
    }

    #[test]
    fn test_parse_tool_choice_function_object() {
        let extra = Some(serde_json::json!({
            "tool_choice": {
                "type": "function",
                "function": {"name": "get_weather"}
            }
        }));

        match parse_tool_choice(&extra) {
            Some(ToolChoice::Function { name }) => {
                assert_eq!(name, "get_weather");
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_tool_choice_none_extra() {
        assert!(parse_tool_choice(&None).is_none());

        let extra = Some(serde_json::json!({}));
        assert!(parse_tool_choice(&extra).is_none());
    }

    // =========================================================================
    // Cross-adapter tests
    // =========================================================================

    #[test]
    fn test_extract_normalized_text_only() {
        let body = serde_json::json!({
            "id": "resp-123",
            "status": "completed",
            "output": [{
                "type": "message",
                "id": "msg-1",
                "role": "assistant",
                "content": [
                    {"type": "output_text", "text": "Hello, world!"}
                ]
            }],
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
        });

        let normalized = extract_normalized(&body).unwrap();
        assert_eq!(normalized.text_content, vec!["Hello, world!"]);
        assert!(normalized.tool_calls.is_empty());
        assert!(normalized.thinking_content.is_none());
    }

    #[test]
    fn test_extract_normalized_with_function_call() {
        let body = serde_json::json!({
            "id": "resp-123",
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "id": "msg-1",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "Let me search."}]
                },
                {
                    "type": "function_call",
                    "id": "fc-1",
                    "call_id": "call_abc",
                    "name": "search",
                    "arguments": "{\"query\": \"test\"}"
                }
            ],
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
        });

        let normalized = extract_normalized(&body).unwrap();
        assert_eq!(normalized.text_content, vec!["Let me search."]);
        assert_eq!(normalized.tool_calls.len(), 1);
        assert_eq!(normalized.tool_calls[0].call_id, "call_abc");
        assert_eq!(normalized.tool_calls[0].name, "search");
    }

    #[test]
    fn test_extract_normalized_with_reasoning() {
        let body = serde_json::json!({
            "id": "resp-123",
            "status": "completed",
            "output": [
                {
                    "type": "reasoning",
                    "id": "rs-1",
                    "content": "Let me think about this...",
                    "summary": [{"type": "text", "text": "Thinking summary"}]
                },
                {
                    "type": "message",
                    "id": "msg-1",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "The answer is 42."}]
                }
            ],
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
        });

        let normalized = extract_normalized(&body).unwrap();
        assert_eq!(normalized.text_content, vec!["The answer is 42."]);
        assert!(normalized.thinking_content.is_some());
        assert_eq!(
            normalized.thinking_content.unwrap(),
            vec!["Let me think about this..."]
        );
    }

    #[test]
    fn test_extract_normalized_empty_response() {
        let body = serde_json::json!({
            "id": "resp-123",
            "status": "completed",
            "output": [],
            "usage": {"input_tokens": 10, "output_tokens": 0, "total_tokens": 10}
        });

        assert!(extract_normalized(&body).is_none());
    }

    #[test]
    fn test_normalized_to_message_text_only() {
        let normalized = NormalizedAssistantMessage {
            text_content: vec!["Hello".to_string(), "World".to_string()],
            tool_calls: vec![],
            thinking_content: None,
        };

        let msg = normalized_to_message(&normalized);
        // Verify it's an assistant message with text content
        // (InputMessage internals are private, just verify it doesn't panic)
        let _ = msg;
    }

    #[test]
    fn test_normalized_to_message_with_tool_calls() {
        let normalized = NormalizedAssistantMessage {
            text_content: vec!["I'll help you.".to_string()],
            tool_calls: vec![NormalizedToolCall::new(
                "call_1",
                "shell",
                r#"{"cmd":"ls"}"#,
            )],
            thinking_content: Some(vec!["Thinking...".to_string()]),
        };

        let msg = normalized_to_message(&normalized);
        // Verify conversion succeeds
        let _ = msg;
    }
}
