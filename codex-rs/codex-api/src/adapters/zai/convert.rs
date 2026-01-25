//! Conversion functions between codex-api and Z.AI SDK types.
//!
//! This module stores the full `Completion` response in `Reasoning.encrypted_content`
//! for round-trip preservation. On sendback, we extract the Content directly
//! from the stored response.

use std::collections::HashSet;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use serde_json::Value;
use z_ai_sdk::Completion;
use z_ai_sdk::CompletionUsage;
use z_ai_sdk::ContentBlock;
use z_ai_sdk::MessageParam;
use z_ai_sdk::Role;
use z_ai_sdk::Tool;

use crate::common::Prompt;
use crate::common::ResponseEvent;
use crate::common_ext::EncryptedContent;
use crate::common_ext::PROVIDER_SDK_ZAI;
use crate::error::ApiError;

// ============================================================================
// Request conversion: Prompt -> Z.AI messages
// ============================================================================

/// Convert a codex-api Prompt to Z.AI MessageParams and optional system message.
///
/// This function handles the conversion of:
/// - Reasoning with encrypted_content -> Extract Content directly from stored response
/// - User messages -> MessageParam with role="user"
/// - Assistant messages -> MessageParam with role="assistant" (skipped if already processed)
/// - FunctionCall -> Skipped if already processed, otherwise converted to assistant message
/// - FunctionCallOutput -> MessageParam with role="tool"
///
/// # Arguments
/// - `prompt` - The codex-api Prompt
/// - `base_url` - Current API base URL (for cross-adapter detection)
/// - `model` - Current model name (for cross-adapter detection)
pub fn prompt_to_messages(
    prompt: &Prompt,
    base_url: &str,
    model: &str,
) -> (Vec<MessageParam>, Option<String>) {
    let mut messages: Vec<MessageParam> = Vec::new();
    let mut pending_function_calls: Vec<(String, String, String)> = Vec::new(); // (call_id, name, arguments)
    let mut pending_assistant_text: Option<String> = None;
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
                flush_pending_assistant(
                    &mut messages,
                    &mut pending_assistant_text,
                    &mut pending_function_calls,
                );

                // Try to extract from adapter-format encrypted_content
                if let Some(assistant_msg) = extract_full_response_message(enc, base_url, model) {
                    messages.push(assistant_msg);
                    processed_response_ids.insert(resp_id.clone());
                    continue;
                }

                // Fallback: encrypted_content is native OpenAI format (not parseable as adapter)
                // Extract text from summary/content fields and add as assistant message.
                // This handles OpenAI Native → Adapter switching.
                if let Some(text) = extract_text_from_reasoning(summary, reasoning_content) {
                    messages.push(MessageParam::assistant(&text));
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
                flush_pending_assistant(
                    &mut messages,
                    &mut pending_assistant_text,
                    &mut pending_function_calls,
                );

                if let Some(text) = extract_text_from_reasoning(summary, reasoning_content) {
                    messages.push(MessageParam::assistant(&text));
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
                    // Flush any pending assistant content
                    flush_pending_assistant(
                        &mut messages,
                        &mut pending_assistant_text,
                        &mut pending_function_calls,
                    );

                    // Collect text content
                    let text = content
                        .iter()
                        .filter_map(|c| match c {
                            ContentItem::OutputText { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("");

                    if !text.is_empty() {
                        pending_assistant_text = Some(text);
                    }
                } else {
                    // Flush pending assistant message first
                    flush_pending_assistant(
                        &mut messages,
                        &mut pending_assistant_text,
                        &mut pending_function_calls,
                    );

                    // Add user message with content blocks
                    let blocks: Vec<ContentBlock> =
                        content.iter().map(content_item_to_block).collect();
                    if !blocks.is_empty() {
                        messages.push(MessageParam::user_with_content(blocks));
                    }
                }
            }

            ResponseItem::FunctionCall {
                call_id,
                name,
                arguments,
                ..
            } => {
                // Collect function calls to be added to assistant message
                pending_function_calls.push((call_id.clone(), name.clone(), arguments.clone()));
            }

            ResponseItem::FunctionCallOutput { call_id, output } => {
                // Flush pending assistant message first
                flush_pending_assistant(
                    &mut messages,
                    &mut pending_assistant_text,
                    &mut pending_function_calls,
                );

                // Add tool result message
                messages.push(MessageParam::tool_result(call_id, &output.content));
            }

            _ => {}
        }
    }

    // Flush any remaining assistant content
    flush_pending_assistant(
        &mut messages,
        &mut pending_assistant_text,
        &mut pending_function_calls,
    );

    // Extract system prompt
    let system = if prompt.instructions.is_empty() {
        None
    } else {
        Some(prompt.instructions.clone())
    };

    (messages, system)
}

/// Extract MessageParam from stored Z.AI Completion body.
///
/// Supports cross-adapter conversion: if the stored response is from a different
/// adapter (detected via base_url/model mismatch), converts via normalized format.
fn extract_full_response_message(
    encrypted_content: &str,
    current_base_url: &str,
    current_model: &str,
) -> Option<MessageParam> {
    let ec = EncryptedContent::from_json_string(encrypted_content)?;

    // Fast path: same adapter context
    if ec.matches_context(current_base_url, current_model) {
        let completion: Completion = ec.parse_body()?;

        let choice = completion.choices.first()?;
        let message = &choice.message;

        // Build content from message fields
        let mut content: Vec<ContentBlock> = Vec::new();

        // Add text content
        if let Some(text) = &message.content {
            if !text.is_empty() {
                content.push(ContentBlock::text(text));
            }
        }

        if content.is_empty() {
            return None;
        }
        return Some(MessageParam {
            role: Role::Assistant,
            content,
            tool_call_id: None,
            name: None,
        });
    }

    // Cross-adapter path: normalize then convert
    let normalized = ec.to_normalized()?;
    Some(normalized_to_message(&normalized))
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

/// Flush pending assistant text and function calls to a single message.
fn flush_pending_assistant(
    messages: &mut Vec<MessageParam>,
    pending_text: &mut Option<String>,
    pending_calls: &mut Vec<(String, String, String)>,
) {
    if pending_text.is_none() && pending_calls.is_empty() {
        return;
    }

    // For Z.AI, we need to handle the case where assistant has text + tool calls
    // Since MessageParam doesn't directly support tool_calls in content,
    // we'll add separate messages

    if let Some(text) = pending_text.take() {
        messages.push(MessageParam::assistant(text));
    }

    // Note: Z.AI SDK uses a different pattern for assistant tool calls
    // The tool calls are returned in CompletionMessage.tool_calls, not in content
    // For now, we don't need to include tool calls in the request
    // as they are derived from the response
    pending_calls.clear();
}

/// Convert a ContentItem to a Z.AI ContentBlock.
fn content_item_to_block(item: &ContentItem) -> ContentBlock {
    match item {
        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
            ContentBlock::text(text)
        }
        ContentItem::InputImage { image_url } => {
            // Check if it's a data URL
            if image_url.starts_with("data:") {
                // Parse data URL: data:image/png;base64,<data>
                if let Some(rest) = image_url.strip_prefix("data:") {
                    if let Some((mime_encoding, data)) = rest.split_once(',') {
                        let media_type = mime_encoding.split(';').next().unwrap_or("image/png");
                        return ContentBlock::image_base64(data, media_type);
                    }
                }
            }
            ContentBlock::image_url(image_url)
        }
    }
}

// ============================================================================
// Tool conversion: JSON -> Z.AI Tool
// ============================================================================

/// Convert JSON tool definitions to Z.AI Tool structs.
///
/// Supports both OpenAI-style format:
/// ```json
/// {"type": "function", "function": {"name": "...", "description": "...", "parameters": {...}}}
/// ```
/// And direct function format:
/// ```json
/// {"name": "...", "description": "...", "parameters": {...}}
/// ```
pub fn tools_to_zai(tools: &[Value]) -> Vec<Tool> {
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

/// Convert a single tool JSON to a Z.AI Tool struct.
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

    Some(Tool::function(name, description, parameters))
}

// ============================================================================
// Response conversion: Z.AI Completion -> ResponseEvents
// ============================================================================

/// Convert a Z.AI Completion response to codex-api ResponseEvents.
///
/// Returns a vector of events, or an error if the response indicates a blocked/truncated generation.
///
/// The events include:
/// - Created (response start)
/// - OutputItemDone for each content block (Message, FunctionCall, Reasoning)
/// - Completed (response end with usage)
///
/// # Arguments
/// - `completion` - The Z.AI Completion response
/// - `base_url` - The API base URL (for model switch detection)
/// - `model` - The model name (for model switch detection)
///
/// # Errors
/// - `ApiError::ContextWindowExceeded` if finish_reason is "length"
/// - `ApiError::GenerationBlocked` for "content_filter" or other blocked reasons
pub fn completion_to_events(
    completion: &Completion,
    base_url: &str,
    model: &str,
) -> Result<Vec<ResponseEvent>, ApiError> {
    // Check finish_reason for error conditions
    if let Some(first) = completion.choices.first() {
        match first.finish_reason.as_str() {
            "length" => return Err(ApiError::ContextWindowExceeded),
            "content_filter" => {
                return Err(ApiError::GenerationBlocked("content filtered".to_string()));
            }
            // "stop" and "tool_calls" are normal
            _ => {}
        }
    }

    let mut events = Vec::new();

    // Add Created event
    events.push(ResponseEvent::Created);

    // Get raw response body from sdk_http_response for storage
    let full_response_json = completion
        .sdk_http_response
        .as_ref()
        .and_then(|r| r.body.clone())
        .and_then(|body| EncryptedContent::from_body_str(&body, PROVIDER_SDK_ZAI, base_url, model))
        .and_then(|ec| ec.to_json_string());

    let mut has_reasoning = false;

    // Process choices
    for choice in &completion.choices {
        let message = &choice.message;

        // Handle reasoning content (extended thinking)
        if let Some(reasoning) = &message.reasoning_content {
            if !reasoning.is_empty() {
                events.push(ResponseEvent::OutputItemDone(ResponseItem::Reasoning {
                    id: uuid::Uuid::new_v4().to_string(),
                    summary: vec![ReasoningItemReasoningSummary::SummaryText {
                        text: reasoning.clone(),
                    }],
                    content: Some(vec![ReasoningItemContent::ReasoningText {
                        text: reasoning.clone(),
                    }]),
                    encrypted_content: full_response_json.clone(),
                }));
                has_reasoning = true;
            }
        }

        // Handle text content
        if let Some(content) = &message.content {
            if !content.is_empty() {
                events.push(ResponseEvent::OutputItemDone(ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: content.clone(),
                    }],
                    end_turn: None,
                }));
            }
        }

        // Handle tool calls
        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                events.push(ResponseEvent::OutputItemDone(ResponseItem::FunctionCall {
                    id: None,
                    call_id: tool_call.id.clone(),
                    name: tool_call.function.name.clone(),
                    arguments: tool_call.function.arguments.clone(),
                }));
            }
        }
    }

    // If no reasoning block, create one to store full response for round-trip
    if !has_reasoning && full_response_json.is_some() {
        events.push(ResponseEvent::OutputItemDone(ResponseItem::Reasoning {
            id: uuid::Uuid::new_v4().to_string(),
            summary: vec![],
            content: None,
            encrypted_content: full_response_json,
        }));
    }

    // Extract token usage
    let usage = extract_usage(&completion.usage);

    // Add Completed event
    events.push(ResponseEvent::Completed {
        response_id: completion.id.clone().unwrap_or_default(),
        token_usage: Some(usage),
    });

    Ok(events)
}

/// Extract token usage from Z.AI CompletionUsage.
pub fn extract_usage(usage: &CompletionUsage) -> TokenUsage {
    let reasoning_tokens = usage
        .completion_tokens_details
        .as_ref()
        .map(|d| d.reasoning_tokens as i64)
        .unwrap_or(0);

    TokenUsage {
        input_tokens: usage.prompt_tokens as i64,
        output_tokens: usage.completion_tokens as i64,
        cached_input_tokens: usage
            .prompt_tokens_details
            .as_ref()
            .map(|d| d.cached_tokens as i64)
            .unwrap_or(0),
        total_tokens: usage.total_tokens as i64,
        reasoning_output_tokens: reasoning_tokens,
    }
}

// ============================================================================
// Cross-adapter conversion functions
// ============================================================================

use crate::normalized::NormalizedAssistantMessage;
use crate::normalized::NormalizedToolCall;

/// Extract NormalizedAssistantMessage from Z.AI Completion body JSON.
///
/// Used when switching from Z.AI to another adapter.
pub fn extract_normalized(body: &Value) -> Option<NormalizedAssistantMessage> {
    let completion: Completion = serde_json::from_value(body.clone()).ok()?;

    let mut msg = NormalizedAssistantMessage::new();

    for choice in &completion.choices {
        let message = &choice.message;

        // Extract reasoning/thinking content
        if let Some(reasoning) = &message.reasoning_content {
            if !reasoning.is_empty() {
                msg.thinking_content
                    .get_or_insert_with(Vec::new)
                    .push(reasoning.clone());
            }
        }

        // Extract text content
        if let Some(content) = &message.content {
            if !content.is_empty() {
                msg.text_content.push(content.clone());
            }
        }

        // Extract tool calls
        if let Some(tool_calls) = &message.tool_calls {
            for tc in tool_calls {
                msg.tool_calls.push(NormalizedToolCall::new(
                    &tc.id,
                    &tc.function.name,
                    &tc.function.arguments,
                ));
            }
        }
    }

    if msg.is_empty() { None } else { Some(msg) }
}

/// Convert NormalizedAssistantMessage to Z.AI MessageParam.
///
/// Used when switching from another adapter to Z.AI.
pub fn normalized_to_message(msg: &NormalizedAssistantMessage) -> MessageParam {
    // Z.AI MessageParam for assistant role uses simple text content
    // Tool calls are handled separately via tool_calls field in API request

    // Combine all text content
    let text = msg.text_content.join("\n");

    // Note: Z.AI doesn't support including previous reasoning/thinking in requests.
    // Tool calls in Z.AI are returned by the model, not sent as input.

    if text.is_empty() {
        // If no text, create empty assistant message
        MessageParam::assistant("")
    } else {
        MessageParam::assistant(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::FunctionCallOutputPayload;
    use pretty_assertions::assert_eq;
    use z_ai_sdk::Role;

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
        assert_eq!(messages[0].role, Role::User);
        assert!(system.is_some());
        assert_eq!(system.unwrap(), "You are helpful.");
    }

    #[test]
    fn test_prompt_to_messages_with_function_output() {
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

        // Should have: user, assistant, tool_result
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[2].role, Role::Tool);
    }

    #[test]
    fn test_tools_to_zai_openai_format() {
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

        let zai_tools = tools_to_zai(&tools);

        assert_eq!(zai_tools.len(), 1);
        match &zai_tools[0] {
            Tool::Function { function } => {
                assert_eq!(function.name, "get_weather");
                assert_eq!(
                    function.description,
                    Some("Get current weather".to_string())
                );
            }
        }
    }

    #[test]
    fn test_tools_to_zai_direct_format() {
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

        let zai_tools = tools_to_zai(&tools);

        assert_eq!(zai_tools.len(), 1);
        match &zai_tools[0] {
            Tool::Function { function } => {
                assert_eq!(function.name, "search");
            }
        }
    }

    #[test]
    fn test_content_item_to_block_text() {
        let item = ContentItem::InputText {
            text: "Hello".to_string(),
        };
        let block = content_item_to_block(&item);
        assert!(matches!(block, ContentBlock::Text { .. }));
    }

    #[test]
    fn test_content_item_to_block_image_url() {
        let item = ContentItem::InputImage {
            image_url: "https://example.com/image.png".to_string(),
        };
        let block = content_item_to_block(&item);
        assert!(matches!(block, ContentBlock::ImageUrl { .. }));
    }

    #[test]
    fn test_content_item_to_block_image_base64() {
        let item = ContentItem::InputImage {
            image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==".to_string(),
        };
        let block = content_item_to_block(&item);
        // Should be converted to image_url with data URL
        assert!(matches!(block, ContentBlock::ImageUrl { .. }));
    }
}
