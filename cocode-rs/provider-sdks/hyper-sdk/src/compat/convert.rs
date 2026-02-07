//! Conversion utilities between hyper-sdk and codex-api types.
//!
//! These functions help bridge the gap between hyper-sdk's unified types
//! and codex-api's wire-format types.

use crate::error::HyperError;
use crate::messages::ContentBlock;
use crate::messages::ImageSource;
use crate::messages::Message;
use crate::messages::Role;
use crate::request::GenerateRequest;
use crate::response::GenerateResponse;
use crate::response::TokenUsage;
use crate::stream::StreamEvent;
use crate::tools::ToolDefinition;
use crate::tools::ToolResultContent;
use serde_json::Value;

/// Convert a JSON object to a GenerateRequest.
///
/// This is useful for converting codex-api Prompt objects to hyper-sdk requests.
pub fn json_to_request(json: &Value) -> Result<GenerateRequest, HyperError> {
    let messages = json
        .get("messages")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|m| json_to_message(m).ok()).collect())
        .unwrap_or_default();

    let mut request = GenerateRequest::new(messages);

    if let Some(temp) = json.get("temperature").and_then(|v| v.as_f64()) {
        request = request.temperature(temp);
    }

    if let Some(max) = json.get("max_tokens").and_then(|v| v.as_i64()) {
        request = request.max_tokens(max as i32);
    }

    if let Some(top_p) = json.get("top_p").and_then(|v| v.as_f64()) {
        request = request.top_p(top_p);
    }

    if let Some(tools) = json.get("tools").and_then(|v| v.as_array()) {
        let tool_defs: Vec<ToolDefinition> = tools
            .iter()
            .filter_map(|t| json_to_tool_definition(t).ok())
            .collect();
        if !tool_defs.is_empty() {
            request = request.tools(tool_defs);
        }
    }

    Ok(request)
}

/// Convert a JSON object to a Message.
fn json_to_message(json: &Value) -> Result<Message, HyperError> {
    let role = json
        .get("role")
        .and_then(|v| v.as_str())
        .ok_or_else(|| HyperError::ParseError("Missing role".to_string()))?;

    let role = match role {
        "system" => Role::System,
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        _ => return Err(HyperError::ParseError(format!("Unknown role: {role}"))),
    };

    let content = if let Some(text) = json.get("content").and_then(|v| v.as_str()) {
        vec![ContentBlock::text(text)]
    } else if let Some(blocks) = json.get("content").and_then(|v| v.as_array()) {
        blocks
            .iter()
            .filter_map(|b| json_to_content_block(b).ok())
            .collect()
    } else {
        vec![]
    };

    let mut msg = Message::new(role, content);

    // Parse metadata if present
    if let Some(metadata) = json.get("metadata") {
        if let Some(provider) = metadata.get("source_provider").and_then(|v| v.as_str()) {
            msg.metadata.source_provider = Some(provider.to_string());
        }
        if let Some(model) = metadata.get("source_model").and_then(|v| v.as_str()) {
            msg.metadata.source_model = Some(model.to_string());
        }
        if let Some(extensions) = metadata.get("extensions").and_then(|v| v.as_object()) {
            for (key, value) in extensions {
                msg.metadata.extensions.insert(key.clone(), value.clone());
            }
        }
    }

    Ok(msg)
}

/// Convert a JSON object to a ContentBlock.
fn json_to_content_block(json: &Value) -> Result<ContentBlock, HyperError> {
    let block_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("text");

    match block_type {
        "text" => {
            let text = json
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            Ok(ContentBlock::text(text))
        }
        "image" | "image_url" => {
            if let Some(url) = json
                .get("image_url")
                .and_then(|v| v.get("url"))
                .and_then(|v| v.as_str())
            {
                Ok(ContentBlock::image_url(url))
            } else if let Some(data) = json
                .get("source")
                .and_then(|v| v.get("data"))
                .and_then(|v| v.as_str())
            {
                let media_type = json
                    .get("source")
                    .and_then(|v| v.get("media_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("image/png");
                Ok(ContentBlock::image_base64(data, media_type))
            } else {
                Err(HyperError::ParseError("Invalid image block".to_string()))
            }
        }
        "tool_use" => {
            let id = json.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let name = json
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let input = json.get("input").cloned().unwrap_or(Value::Null);
            Ok(ContentBlock::tool_use(id, name, input))
        }
        "tool_result" => {
            let tool_use_id = json
                .get("tool_use_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let content = if let Some(text) = json.get("content").and_then(|v| v.as_str()) {
                ToolResultContent::text(text)
            } else {
                ToolResultContent::text("")
            };
            let is_error = json
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Ok(ContentBlock::tool_result(tool_use_id, content, is_error))
        }
        _ => Err(HyperError::ParseError(format!(
            "Unknown block type: {block_type}"
        ))),
    }
}

/// Convert a JSON object to a ToolDefinition.
fn json_to_tool_definition(json: &Value) -> Result<ToolDefinition, HyperError> {
    // Handle OpenAI format (function wrapper) or direct format
    let func = json.get("function").unwrap_or(json);

    let name = func
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| HyperError::ParseError("Missing tool name".to_string()))?;

    let description = func.get("description").and_then(|v| v.as_str());
    let parameters = func
        .get("parameters")
        .cloned()
        .unwrap_or(Value::Object(Default::default()));

    let mut tool = ToolDefinition::new(name, parameters);
    if let Some(desc) = description {
        tool = tool.with_description(desc);
    }
    Ok(tool)
}

/// Convert a GenerateResponse to JSON.
pub fn response_to_json(response: &GenerateResponse) -> Value {
    serde_json::json!({
        "id": response.id,
        "model": response.model,
        "finish_reason": format!("{:?}", response.finish_reason).to_lowercase(),
        "content": response.content.iter().map(content_block_to_json).collect::<Vec<_>>(),
        "usage": response.usage.as_ref().map(usage_to_json),
    })
}

/// Convert a ContentBlock to JSON.
fn content_block_to_json(block: &ContentBlock) -> Value {
    match block {
        ContentBlock::Text { text } => serde_json::json!({
            "type": "text",
            "text": text,
        }),
        ContentBlock::Image { source, detail } => {
            let source_json = match source {
                ImageSource::Base64 { data, media_type } => serde_json::json!({
                    "type": "base64",
                    "data": data,
                    "media_type": media_type,
                }),
                ImageSource::Url { url } => serde_json::json!({
                    "type": "url",
                    "url": url,
                }),
            };
            serde_json::json!({
                "type": "image",
                "source": source_json,
                "detail": detail,
            })
        }
        ContentBlock::ToolUse { id, name, input } => serde_json::json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        }),
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            ..
        } => serde_json::json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": match content {
                ToolResultContent::Text(t) => Value::String(t.clone()),
                ToolResultContent::Json(v) => v.clone(),
                ToolResultContent::Blocks(_) => Value::Array(vec![]), // Simplified
            },
            "is_error": is_error,
        }),
        ContentBlock::Thinking { content, signature } => serde_json::json!({
            "type": "thinking",
            "content": content,
            "signature": signature,
        }),
    }
}

/// Convert a Message to JSON.
pub fn message_to_json(msg: &Message) -> Value {
    let mut json = serde_json::json!({
        "role": match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        },
        "content": msg.content.iter().map(content_block_to_json).collect::<Vec<_>>(),
    });

    // Add metadata if not empty
    if !msg.metadata.is_empty() {
        let mut metadata = serde_json::Map::new();
        if let Some(ref provider) = msg.metadata.source_provider {
            metadata.insert(
                "source_provider".to_string(),
                Value::String(provider.clone()),
            );
        }
        if let Some(ref model) = msg.metadata.source_model {
            metadata.insert("source_model".to_string(), Value::String(model.clone()));
        }
        if !msg.metadata.extensions.is_empty() {
            metadata.insert(
                "extensions".to_string(),
                Value::Object(
                    msg.metadata
                        .extensions
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                ),
            );
        }
        if let Some(obj) = json.as_object_mut() {
            obj.insert("metadata".to_string(), Value::Object(metadata));
        }
    }

    json
}

/// Convert TokenUsage to JSON.
fn usage_to_json(usage: &TokenUsage) -> Value {
    serde_json::json!({
        "prompt_tokens": usage.prompt_tokens,
        "completion_tokens": usage.completion_tokens,
        "total_tokens": usage.total_tokens,
        "cache_read_tokens": usage.cache_read_tokens,
        "cache_creation_tokens": usage.cache_creation_tokens,
        "reasoning_tokens": usage.reasoning_tokens,
    })
}

/// Convert a StreamEvent to JSON.
pub fn stream_event_to_json(event: &StreamEvent) -> Value {
    serde_json::to_value(event).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_to_request() {
        let json = serde_json::json!({
            "messages": [
                {"role": "user", "content": "Hello!"}
            ],
            "temperature": 0.7,
            "max_tokens": 1000
        });

        let request = json_to_request(&json).unwrap();
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(1000));
    }

    #[test]
    fn test_json_to_message_simple() {
        let json = serde_json::json!({
            "role": "user",
            "content": "Hello!"
        });

        let message = json_to_message(&json).unwrap();
        assert_eq!(message.role, Role::User);
        assert_eq!(message.text(), "Hello!");
    }

    #[test]
    fn test_json_to_message_blocks() {
        let json = serde_json::json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "What's in this image?"},
                {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
            ]
        });

        let message = json_to_message(&json).unwrap();
        assert_eq!(message.role, Role::User);
        assert_eq!(message.content.len(), 2);
    }

    #[test]
    fn test_response_to_json() {
        let response = GenerateResponse::new("resp_1", "gpt-4o")
            .with_content(vec![ContentBlock::text("Hello!")])
            .with_usage(TokenUsage::new(10, 5));

        let json = response_to_json(&response);
        assert_eq!(json["id"], "resp_1");
        assert_eq!(json["model"], "gpt-4o");
        assert!(json["content"].is_array());
    }
}
