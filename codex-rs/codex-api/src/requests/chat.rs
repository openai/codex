use crate::common::ClaudeThinking;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::requests::headers::build_conversation_headers;
use crate::requests::headers::insert_header;
use crate::requests::headers::subagent_header;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use http::HeaderMap;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;
use tracing::debug;
use tracing::warn;

/// Assembled request body plus headers for Chat Completions streaming calls.
pub struct ChatRequest {
    pub body: Value,
    pub headers: HeaderMap,
}

pub struct ChatRequestBuilder<'a> {
    model: &'a str,
    instructions: &'a str,
    input: &'a [ResponseItem],
    tools: &'a [Value],
    conversation_id: Option<String>,
    session_source: Option<SessionSource>,
    /// Claude/Anthropic structured output schema.
    output_schema: Option<Value>,
    /// Claude/Anthropic extended thinking configuration.
    thinking: Option<ClaudeThinking>,
}

impl<'a> ChatRequestBuilder<'a> {
    pub fn new(
        model: &'a str,
        instructions: &'a str,
        input: &'a [ResponseItem],
        tools: &'a [Value],
    ) -> Self {
        Self {
            model,
            instructions,
            input,
            tools,
            conversation_id: None,
            session_source: None,
            output_schema: None,
            thinking: None,
        }
    }

    pub fn conversation_id(mut self, id: Option<String>) -> Self {
        self.conversation_id = id;
        self
    }

    pub fn session_source(mut self, source: Option<SessionSource>) -> Self {
        self.session_source = source;
        self
    }

    /// Set the structured output schema for Claude/Anthropic providers.
    pub fn output_schema(mut self, schema: Option<Value>) -> Self {
        self.output_schema = schema;
        self
    }

    /// Set the extended thinking configuration for Claude/Anthropic providers.
    pub fn thinking(mut self, thinking: Option<ClaudeThinking>) -> Self {
        self.thinking = thinking;
        self
    }

    pub fn build(self, provider: &Provider) -> Result<ChatRequest, ApiError> {
        let mut messages = Vec::<Value>::new();
        messages.push(json!({"role": "system", "content": self.instructions}));

        let input = self.input;
        let mut reasoning_by_anchor_index: HashMap<usize, String> = HashMap::new();
        let mut last_emitted_role: Option<&str> = None;
        for item in input {
            match item {
                ResponseItem::Message { role, .. } => last_emitted_role = Some(role.as_str()),
                ResponseItem::FunctionCall { .. } | ResponseItem::LocalShellCall { .. } => {
                    last_emitted_role = Some("assistant")
                }
                ResponseItem::FunctionCallOutput { .. } => last_emitted_role = Some("tool"),
                ResponseItem::Reasoning { .. } | ResponseItem::Other => {}
                ResponseItem::CustomToolCall { .. } => {}
                ResponseItem::CustomToolCallOutput { .. } => {}
                ResponseItem::WebSearchCall { .. } => {}
                ResponseItem::GhostSnapshot { .. } => {}
                ResponseItem::Compaction { .. } => {}
            }
        }

        let mut last_user_index: Option<usize> = None;
        for (idx, item) in input.iter().enumerate() {
            if let ResponseItem::Message { role, .. } = item
                && role == "user"
            {
                last_user_index = Some(idx);
            }
        }

        if !matches!(last_emitted_role, Some("user")) {
            for (idx, item) in input.iter().enumerate() {
                if let Some(u_idx) = last_user_index
                    && idx <= u_idx
                {
                    continue;
                }

                if let ResponseItem::Reasoning {
                    content: Some(items),
                    ..
                } = item
                {
                    let mut text = String::new();
                    for entry in items {
                        match entry {
                            ReasoningItemContent::ReasoningText { text: segment }
                            | ReasoningItemContent::Text { text: segment } => {
                                text.push_str(segment)
                            }
                        }
                    }
                    if text.trim().is_empty() {
                        continue;
                    }

                    let mut attached = false;
                    if idx > 0
                        && let ResponseItem::Message { role, .. } = &input[idx - 1]
                        && role == "assistant"
                    {
                        reasoning_by_anchor_index
                            .entry(idx - 1)
                            .and_modify(|v| v.push_str(&text))
                            .or_insert(text.clone());
                        attached = true;
                    }

                    if !attached && idx + 1 < input.len() {
                        match &input[idx + 1] {
                            ResponseItem::FunctionCall { .. }
                            | ResponseItem::LocalShellCall { .. } => {
                                reasoning_by_anchor_index
                                    .entry(idx + 1)
                                    .and_modify(|v| v.push_str(&text))
                                    .or_insert(text.clone());
                            }
                            ResponseItem::Message { role, .. } if role == "assistant" => {
                                reasoning_by_anchor_index
                                    .entry(idx + 1)
                                    .and_modify(|v| v.push_str(&text))
                                    .or_insert(text.clone());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let mut last_assistant_text: Option<String> = None;

        for (idx, item) in input.iter().enumerate() {
            match item {
                ResponseItem::Message { role, content, .. } => {
                    let mut text = String::new();
                    let mut items: Vec<Value> = Vec::new();
                    let mut saw_image = false;

                    for c in content {
                        match c {
                            ContentItem::InputText { text: t }
                            | ContentItem::OutputText { text: t } => {
                                text.push_str(t);
                                items.push(json!({"type":"text","text": t}));
                            }
                            ContentItem::InputImage { image_url } => {
                                saw_image = true;
                                items.push(
                                    json!({"type":"image_url","image_url": {"url": image_url}}),
                                );
                            }
                        }
                    }

                    if role == "assistant" {
                        if let Some(prev) = &last_assistant_text
                            && prev == &text
                        {
                            continue;
                        }
                        last_assistant_text = Some(text.clone());
                    }

                    let content_value = if role == "assistant" {
                        json!(text)
                    } else if saw_image {
                        json!(items)
                    } else {
                        json!(text)
                    };

                    let mut msg = json!({"role": role, "content": content_value});
                    if role == "assistant"
                        && let Some(reasoning) = reasoning_by_anchor_index.get(&idx)
                        && let Some(obj) = msg.as_object_mut()
                    {
                        obj.insert("reasoning".to_string(), json!(reasoning));
                    }
                    messages.push(msg);
                }
                ResponseItem::FunctionCall {
                    name,
                    arguments,
                    call_id,
                    ..
                } => {
                    let reasoning = reasoning_by_anchor_index.get(&idx).map(String::as_str);
                    let tool_call = json!({
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": arguments,
                        }
                    });
                    push_tool_call_message(&mut messages, tool_call, reasoning);
                }
                ResponseItem::LocalShellCall {
                    id,
                    call_id: _,
                    status,
                    action,
                } => {
                    let reasoning = reasoning_by_anchor_index.get(&idx).map(String::as_str);
                    let tool_call = json!({
                        "id": id.clone().unwrap_or_default(),
                        "type": "local_shell_call",
                        "status": status,
                        "action": action,
                    });
                    push_tool_call_message(&mut messages, tool_call, reasoning);
                }
                ResponseItem::FunctionCallOutput { call_id, output } => {
                    let content_value = if let Some(items) = &output.content_items {
                        let mapped: Vec<Value> = items
                            .iter()
                            .map(|it| match it {
                                FunctionCallOutputContentItem::InputText { text } => {
                                    json!({"type":"text","text": text})
                                }
                                FunctionCallOutputContentItem::InputImage { image_url } => {
                                    json!({"type":"image_url","image_url": {"url": image_url}})
                                }
                            })
                            .collect();
                        json!(mapped)
                    } else {
                        json!(output.content)
                    };

                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": content_value,
                    }));
                }
                ResponseItem::CustomToolCall {
                    id,
                    call_id: _,
                    name,
                    input,
                    status: _,
                } => {
                    let tool_call = json!({
                        "id": id,
                        "type": "custom",
                        "custom": {
                            "name": name,
                            "input": input,
                        }
                    });
                    let reasoning = reasoning_by_anchor_index.get(&idx).map(String::as_str);
                    push_tool_call_message(&mut messages, tool_call, reasoning);
                }
                ResponseItem::CustomToolCallOutput { call_id, output } => {
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": output,
                    }));
                }
                ResponseItem::GhostSnapshot { .. } => {
                    continue;
                }
                ResponseItem::Reasoning { .. }
                | ResponseItem::WebSearchCall { .. }
                | ResponseItem::Other
                | ResponseItem::Compaction { .. } => {
                    continue;
                }
            }
        }

        let payload = if provider.is_claude_provider() {
            // Bedrock uses native Anthropic message format
            // System message is separate, not in messages array
            let system_content = self.instructions;

            // Log input items for debugging
            debug!("=== Raw Input Items ({} total) ===", input.len());
            for (idx, item) in input.iter().enumerate() {
                match item {
                    ResponseItem::FunctionCall { call_id, name, .. } => {
                        debug!(
                            "  [{}] FunctionCall: name={}, call_id={}",
                            idx, name, call_id
                        );
                    }
                    ResponseItem::FunctionCallOutput { call_id, .. } => {
                        debug!("  [{}] FunctionCallOutput: call_id={}", idx, call_id);
                    }
                    ResponseItem::Message { role, .. } => {
                        debug!("  [{}] Message: role={}", idx, role);
                    }
                    ResponseItem::LocalShellCall { call_id, .. } => {
                        debug!("  [{}] LocalShellCall: call_id={:?}", idx, call_id);
                    }
                    ResponseItem::Reasoning { .. } => {
                        debug!("  [{}] Reasoning", idx);
                    }
                    _ => {
                        debug!("  [{}] Other", idx);
                    }
                }
            }
            debug!("=== End Raw Input Items ===");

            // Transform messages from OpenAI format to Claude format
            let bedrock_messages: Vec<Value> = transform_messages_for_claude(messages);

            // Log the final messages being sent to Bedrock
            debug!(
                "=== Final Bedrock Messages ({} total) ===",
                bedrock_messages.len()
            );
            for (idx, msg) in bedrock_messages.iter().enumerate() {
                let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("?");
                let content = msg.get("content");
                let content_preview = match content {
                    Some(Value::Array(arr)) => {
                        let items: Vec<String> = arr
                            .iter()
                            .map(|item| {
                                let t = item.get("type").and_then(|t| t.as_str()).unwrap_or("?");
                                if t == "tool_use" {
                                    let id = item.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                                    let name =
                                        item.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                                    format!("tool_use(id={id}, name={name})")
                                } else if t == "tool_result" {
                                    let id = item
                                        .get("tool_use_id")
                                        .and_then(|i| i.as_str())
                                        .unwrap_or("?");
                                    format!("tool_result(tool_use_id={id})")
                                } else if t == "text" {
                                    let text =
                                        item.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                    format!("text(len={})", text.len())
                                } else {
                                    format!("{t}(...)")
                                }
                            })
                            .collect();
                        format!("[{}]", items.join(", "))
                    }
                    Some(Value::String(s)) => format!("\"{}...\"", &s[..s.len().min(50)]),
                    _ => "null".to_string(),
                };
                debug!("  [{}] role={}: {}", idx, role, content_preview);
            }
            debug!("=== End Bedrock Messages ===");

            let mut bedrock_payload = json!({
                "anthropic_version": "bedrock-2023-05-31",
                "max_tokens": 16384,
                "system": system_content,
                "messages": bedrock_messages,
            });

            // Add tools if present - transform from OpenAI format to Claude format
            if !self.tools.is_empty() {
                let claude_tools: Vec<Value> = self
                    .tools
                    .iter()
                    .filter_map(|tool| {
                        // OpenAI format: { "type": "function", "function": { "name", "description", "parameters" } }
                        // Claude format: { "name", "description", "input_schema" }
                        if tool.get("type").and_then(|t| t.as_str()) == Some("function")
                            && let Some(func) = tool.get("function") {
                                return Some(json!({
                                    "name": func.get("name"),
                                    "description": func.get("description"),
                                    "input_schema": func.get("parameters")
                                }));
                            }
                        // If already in Claude format or unknown, pass through
                        Some(tool.clone())
                    })
                    .collect();
                bedrock_payload["tools"] = json!(claude_tools);
                // Explicitly set tool_choice to "auto" so Claude can decide when to use tools
                bedrock_payload["tool_choice"] = json!({"type": "auto"});
            }

            // Add structured output schema as output_format
            if let Some(schema) = &self.output_schema {
                bedrock_payload["output_format"] = json!({
                    "type": "json_schema",
                    "schema": schema
                });
            }

            // Add extended thinking configuration
            if let Some(thinking) = &self.thinking {
                bedrock_payload["thinking"] = json!(thinking);
            }

            // Keep model in payload for path extraction (will be used to build URL)
            bedrock_payload["model"] = json!(self.model);

            bedrock_payload
        } else {
            // Standard Chat Completions format for other providers
            json!({
                "model": self.model,
                "messages": messages,
                "stream": true,
                "tools": self.tools,
            })
        };

        // Don't add OpenAI-specific headers for Bedrock - they cause SigV4 signing issues
        let headers = if provider.is_claude_provider() {
            HeaderMap::new()
        } else {
            let mut h = build_conversation_headers(self.conversation_id);
            if let Some(subagent) = subagent_header(&self.session_source) {
                insert_header(&mut h, "x-openai-subagent", &subagent);
            }
            h
        };

        Ok(ChatRequest {
            body: payload,
            headers,
        })
    }
}

fn push_tool_call_message(messages: &mut Vec<Value>, tool_call: Value, reasoning: Option<&str>) {
    // Chat Completions requires that tool calls are grouped into a single assistant message
    // (with `tool_calls: [...]`) followed by tool role responses.
    if let Some(Value::Object(obj)) = messages.last_mut()
        && obj.get("role").and_then(Value::as_str) == Some("assistant")
        && obj.get("content").is_some_and(Value::is_null)
        && let Some(tool_calls) = obj.get_mut("tool_calls").and_then(Value::as_array_mut)
    {
        tool_calls.push(tool_call);
        if let Some(reasoning) = reasoning {
            if let Some(Value::String(existing)) = obj.get_mut("reasoning") {
                if !existing.is_empty() {
                    existing.push('\n');
                }
                existing.push_str(reasoning);
            } else {
                obj.insert(
                    "reasoning".to_string(),
                    Value::String(reasoning.to_string()),
                );
            }
        }
        return;
    }

    let mut msg = json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [tool_call],
    });
    if let Some(reasoning) = reasoning
        && let Some(obj) = msg.as_object_mut()
    {
        obj.insert("reasoning".to_string(), json!(reasoning));
    }
    messages.push(msg);
}

/// Sort assistant content so text blocks come before tool_use blocks.
/// Claude expects text content to precede tool_use in the same message.
fn sort_assistant_content(content: &mut Vec<Value>) {
    content.sort_by(|a, b| {
        let type_a = a.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let type_b = b.get("type").and_then(|t| t.as_str()).unwrap_or("");
        // text comes before tool_use, everything else stays in order
        match (type_a, type_b) {
            ("text", "tool_use") => std::cmp::Ordering::Less,
            ("tool_use", "text") => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    });
}

/// Transform OpenAI Chat Completions message format to Claude/Bedrock format
///
/// Key differences:
/// - OpenAI: role="tool" for tool results
/// - Claude: role="user" with content=[{type: "tool_result", ...}]
///
/// - OpenAI: assistant messages have tool_calls array
/// - Claude: assistant messages have content=[{type: "tool_use", ...}]
///
/// Claude requires:
/// - Strict alternation between user/assistant roles
/// - tool_result to IMMEDIATELY follow tool_use
/// - text content should come before tool_use blocks
/// So we must merge consecutive messages of the same role.
///
/// IMPORTANT: User messages may appear between tool_calls and tool_results
/// (e.g., warnings). We must defer flushing assistant content with tool_use
/// until we have the corresponding tool_results.
fn transform_messages_for_claude(messages: Vec<Value>) -> Vec<Value> {
    let mut result: Vec<Value> = Vec::new();
    let mut pending_tool_results: Vec<Value> = Vec::new();
    let mut pending_assistant_content: Vec<Value> = Vec::new();
    let mut pending_user_content: Vec<Value> = Vec::new();
    // Track tool_use IDs that are waiting for tool_results
    let mut pending_tool_use_ids: HashSet<String> = HashSet::new();

    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

        match role {
            "system" => {
                // Skip system messages - handled separately in Bedrock
                continue;
            }
            "tool" => {
                // Collect tool results - they'll be merged into a single user message
                let tool_call_id = msg
                    .get("tool_call_id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("");
                let content = msg.get("content").cloned().unwrap_or(json!(""));

                // Mark this tool_use as resolved
                pending_tool_use_ids.remove(tool_call_id);

                // Convert content to string for Claude
                let content_str = if content.is_string() {
                    content.as_str().unwrap_or("").to_string()
                } else {
                    serde_json::to_string(&content).unwrap_or_default()
                };

                pending_tool_results.push(json!({
                    "type": "tool_result",
                    "tool_use_id": tool_call_id,
                    "content": content_str
                }));

                // If all pending tool_use IDs are resolved, flush assistant content
                if pending_tool_use_ids.is_empty() && !pending_assistant_content.is_empty() {
                    sort_assistant_content(&mut pending_assistant_content);
                    result.push(json!({
                        "role": "assistant",
                        "content": std::mem::take(&mut pending_assistant_content)
                    }));
                }
                continue;
            }
            "assistant" => {
                // Flush any pending user content first (to maintain alternation)
                if !pending_user_content.is_empty() {
                    result.push(json!({
                        "role": "user",
                        "content": std::mem::take(&mut pending_user_content)
                    }));
                }

                // Flush any pending tool results (add to user message if needed)
                if !pending_tool_results.is_empty() {
                    // If we already flushed user content, we need to merge tool results
                    // Otherwise create a new user message
                    if let Some(last) = result.last_mut() {
                        if last.get("role").and_then(|r| r.as_str()) == Some("user") {
                            if let Some(content) =
                                last.get_mut("content").and_then(|c| c.as_array_mut())
                            {
                                content.append(&mut pending_tool_results);
                            }
                        } else {
                            result.push(json!({
                                "role": "user",
                                "content": std::mem::take(&mut pending_tool_results)
                            }));
                        }
                    } else {
                        result.push(json!({
                            "role": "user",
                            "content": std::mem::take(&mut pending_tool_results)
                        }));
                    }
                }

                // Collect assistant content - will be merged with other consecutive assistant messages
                // Add text content FIRST (before tool_use blocks)
                let content = msg.get("content").cloned().unwrap_or(json!(""));
                if content.is_string() {
                    let text = content.as_str().unwrap_or("");
                    // Skip empty or whitespace-only text (Bedrock rejects these)
                    if !text.trim().is_empty() {
                        pending_assistant_content.push(json!({"type": "text", "text": text}));
                    }
                } else if content.is_array() {
                    // Already array format, add items (filtering whitespace-only text)
                    if let Some(arr) = content.as_array() {
                        for item in arr {
                            // Skip text items that are empty or whitespace-only
                            if item.get("type").and_then(|t| t.as_str()) == Some("text")
                                && let Some(text) = item.get("text").and_then(|t| t.as_str())
                                    && text.trim().is_empty() {
                                        continue;
                                    }
                            pending_assistant_content.push(item.clone());
                        }
                    }
                } else if !content.is_null() {
                    pending_assistant_content.push(content);
                }

                // Then add tool_use blocks (after text content)
                if let Some(tool_calls) = msg.get("tool_calls").and_then(|t| t.as_array()) {
                    // Transform tool calls to Claude format
                    for tool_call in tool_calls {
                        let call_id = tool_call
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or("")
                            .to_string();

                        if let Some(func) = tool_call.get("function") {
                            let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            let arguments = func
                                .get("arguments")
                                .and_then(|a| a.as_str())
                                .unwrap_or("{}");

                            // Parse arguments JSON string to Value
                            let input: Value = serde_json::from_str(arguments).unwrap_or(json!({}));

                            // Track this tool_use ID as pending
                            pending_tool_use_ids.insert(call_id.clone());

                            pending_assistant_content.push(json!({
                                "type": "tool_use",
                                "id": call_id,
                                "name": name,
                                "input": input
                            }));
                        }
                    }
                }

                // Don't push yet - wait until we see a non-assistant message
                continue;
            }
            "user" => {
                // Only flush pending assistant content if there are no pending tool_use IDs
                // Otherwise, user messages that appear between tool_call and tool_result
                // would cause the assistant message to be emitted before the tool_result
                if !pending_tool_use_ids.is_empty() {
                    // Log warning: defensive handling is kicking in
                    warn!(
                        "User message encountered while {} tool_use IDs are pending: {:?}. Deferring assistant flush.",
                        pending_tool_use_ids.len(),
                        pending_tool_use_ids
                    );
                }
                if pending_tool_use_ids.is_empty() && !pending_assistant_content.is_empty() {
                    sort_assistant_content(&mut pending_assistant_content);
                    result.push(json!({
                        "role": "assistant",
                        "content": std::mem::take(&mut pending_assistant_content)
                    }));
                }

                // Flush any pending tool results (they go into user message)
                if !pending_tool_results.is_empty() {
                    // Add tool results to pending user content
                    pending_user_content.append(&mut pending_tool_results);
                }

                // Collect user message content (will be merged with consecutive user messages)
                let content = msg.get("content").cloned().unwrap_or(json!(""));
                if content.is_string() {
                    let text = content.as_str().unwrap_or("");
                    // Skip empty or whitespace-only text (Bedrock rejects these)
                    if !text.trim().is_empty() {
                        pending_user_content.push(json!({"type": "text", "text": text}));
                    }
                } else if content.is_array() {
                    // Already array format, transform items
                    if let Some(arr) = content.as_array() {
                        for item in arr {
                            let item_type = item.get("type").and_then(|t| t.as_str());
                            match item_type {
                                Some("text") => {
                                    // Skip text items that are empty or whitespace-only
                                    if let Some(text) = item.get("text").and_then(|t| t.as_str())
                                        && text.trim().is_empty() {
                                            continue;
                                        }
                                    pending_user_content.push(item.clone());
                                }
                                Some("image_url") => {
                                    // Transform OpenAI image format to Claude format
                                    if let Some(url) = item
                                        .get("image_url")
                                        .and_then(|u| u.get("url"))
                                        .and_then(|u| u.as_str())
                                        && url.starts_with("data:")
                                            && let Some(comma_pos) = url.find(',') {
                                                let header = &url[5..comma_pos];
                                                let data = &url[comma_pos + 1..];
                                                let media_type =
                                                    header.split(';').next().unwrap_or("image/png");
                                                pending_user_content.push(json!({
                                                    "type": "image",
                                                    "source": {
                                                        "type": "base64",
                                                        "media_type": media_type,
                                                        "data": data
                                                    }
                                                }));
                                            }
                                }
                                _ => {
                                    pending_user_content.push(item.clone());
                                }
                            }
                        }
                    }
                }

                // Don't push yet - wait until we see a non-user message
                continue;
            }
            _ => {
                // Unknown role, skip
                continue;
            }
        }
    }

    // Flush any remaining pending assistant content
    if !pending_assistant_content.is_empty() {
        sort_assistant_content(&mut pending_assistant_content);
        result.push(json!({
            "role": "assistant",
            "content": pending_assistant_content
        }));
    }

    // Flush any remaining pending tool results (they go into user message)
    if !pending_tool_results.is_empty() {
        pending_user_content.append(&mut pending_tool_results);
    }

    // Flush any remaining pending user content
    if !pending_user_content.is_empty() {
        result.push(json!({
            "role": "user",
            "content": pending_user_content
        }));
    }

    // Validate tool_use/tool_result pairing and log issues
    validate_tool_pairing(&result);

    result
}

/// Validate that every tool_use has a corresponding tool_result in the next message.
/// Logs detailed debug information for troubleshooting.
fn validate_tool_pairing(messages: &[Value]) {
    debug!("=== Claude Message Validation ===");
    debug!("Total messages: {}", messages.len());

    for (idx, msg) in messages.iter().enumerate() {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("?");
        let content = msg.get("content");

        // Log message summary
        let content_summary = if let Some(arr) = content.and_then(|c| c.as_array()) {
            let types: Vec<&str> = arr
                .iter()
                .filter_map(|item| item.get("type").and_then(|t| t.as_str()))
                .collect();
            format!("{types:?}")
        } else if let Some(s) = content.and_then(|c| c.as_str()) {
            format!("text({})", s.len().min(50))
        } else {
            "null".to_string()
        };
        debug!("  [{}] role={}, content={}", idx, role, content_summary);

        // Check for tool_use blocks
        if let Some(arr) = content.and_then(|c| c.as_array()) {
            let tool_use_ids: Vec<&str> = arr
                .iter()
                .filter_map(|item| {
                    if item.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        item.get("id").and_then(|i| i.as_str())
                    } else {
                        None
                    }
                })
                .collect();

            if !tool_use_ids.is_empty() {
                debug!("    tool_use ids: {:?}", tool_use_ids);

                // Check next message for matching tool_results
                if let Some(next_msg) = messages.get(idx + 1) {
                    let next_role = next_msg.get("role").and_then(|r| r.as_str()).unwrap_or("?");
                    if next_role != "user" {
                        debug!(
                            "    WARNING: Next message is role='{}', expected 'user' for tool_results",
                            next_role
                        );
                    }

                    if let Some(next_content) = next_msg.get("content").and_then(|c| c.as_array()) {
                        let tool_result_ids: HashSet<&str> = next_content
                            .iter()
                            .filter_map(|item| {
                                if item.get("type").and_then(|t| t.as_str()) == Some("tool_result")
                                {
                                    item.get("tool_use_id").and_then(|i| i.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect();

                        debug!("    tool_result ids in next msg: {:?}", tool_result_ids);

                        // Check for missing tool_results
                        for tool_id in &tool_use_ids {
                            if !tool_result_ids.contains(tool_id) {
                                debug!(
                                    "    ERROR: tool_use id '{}' has no matching tool_result in next message!",
                                    tool_id
                                );
                            }
                        }
                    } else {
                        debug!("    WARNING: Next message has no array content for tool_results");
                    }
                } else {
                    debug!("    ERROR: No message follows this tool_use block!");
                }
            }
        }
    }
    debug!("=== End Validation ===");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::RetryConfig;
    use crate::provider::WireApi;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    fn provider() -> Provider {
        Provider {
            name: "openai".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            query_params: None,
            wire: WireApi::Chat,
            headers: HeaderMap::new(),
            retry: RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(10),
                retry_429: false,
                retry_5xx: true,
                retry_transport: true,
            },
            stream_idle_timeout: Duration::from_secs(1),
        }
    }

    #[test]
    fn attaches_conversation_and_subagent_headers() {
        let prompt_input = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "hi".to_string(),
            }],
        }];
        let req = ChatRequestBuilder::new("gpt-test", "inst", &prompt_input, &[])
            .conversation_id(Some("conv-1".into()))
            .session_source(Some(SessionSource::SubAgent(SubAgentSource::Review)))
            .build(&provider())
            .expect("request");

        assert_eq!(
            req.headers.get("session_id"),
            Some(&HeaderValue::from_static("conv-1"))
        );
        assert_eq!(
            req.headers.get("x-openai-subagent"),
            Some(&HeaderValue::from_static("review"))
        );
    }

    #[test]
    fn groups_consecutive_tool_calls_into_a_single_assistant_message() {
        let prompt_input = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "read these".to_string(),
                }],
            },
            ResponseItem::FunctionCall {
                id: None,
                name: "read_file".to_string(),
                arguments: r#"{"path":"a.txt"}"#.to_string(),
                call_id: "call-a".to_string(),
            },
            ResponseItem::FunctionCall {
                id: None,
                name: "read_file".to_string(),
                arguments: r#"{"path":"b.txt"}"#.to_string(),
                call_id: "call-b".to_string(),
            },
            ResponseItem::FunctionCall {
                id: None,
                name: "read_file".to_string(),
                arguments: r#"{"path":"c.txt"}"#.to_string(),
                call_id: "call-c".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-a".to_string(),
                output: FunctionCallOutputPayload {
                    content: "A".to_string(),
                    ..Default::default()
                },
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-b".to_string(),
                output: FunctionCallOutputPayload {
                    content: "B".to_string(),
                    ..Default::default()
                },
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-c".to_string(),
                output: FunctionCallOutputPayload {
                    content: "C".to_string(),
                    ..Default::default()
                },
            },
        ];

        let req = ChatRequestBuilder::new("gpt-test", "inst", &prompt_input, &[])
            .build(&provider())
            .expect("request");

        let messages = req
            .body
            .get("messages")
            .and_then(|v| v.as_array())
            .expect("messages array");
        // system + user + assistant(tool_calls=[...]) + 3 tool outputs
        assert_eq!(messages.len(), 6);

        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");

        let tool_calls_msg = &messages[2];
        assert_eq!(tool_calls_msg["role"], "assistant");
        assert_eq!(tool_calls_msg["content"], serde_json::Value::Null);
        let tool_calls = tool_calls_msg["tool_calls"]
            .as_array()
            .expect("tool_calls array");
        assert_eq!(tool_calls.len(), 3);
        assert_eq!(tool_calls[0]["id"], "call-a");
        assert_eq!(tool_calls[1]["id"], "call-b");
        assert_eq!(tool_calls[2]["id"], "call-c");

        assert_eq!(messages[3]["role"], "tool");
        assert_eq!(messages[3]["tool_call_id"], "call-a");
        assert_eq!(messages[4]["role"], "tool");
        assert_eq!(messages[4]["tool_call_id"], "call-b");
        assert_eq!(messages[5]["role"], "tool");
        assert_eq!(messages[5]["tool_call_id"], "call-c");
    }
}
