use std::collections::HashMap;

use base64::Engine;
use mcp_types::CallToolResult;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::ser::Serializer;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;

use crate::protocol::InputItem;
use crate::protocol::TokenUsage;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseInputItem {
    Message {
        role: String,
        content: Vec<ContentItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<String>,
    },
    FunctionCallOutput {
        call_id: String,
        output: FunctionCallOutputPayload,
    },
    McpToolCallOutput {
        call_id: String,
        result: Result<CallToolResult, String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentItem {
    InputText { text: String },
    InputImage { image_url: String },
    OutputText { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseItem {
    Message {
        id: Option<String>,
        role: String,
        content: Vec<ContentItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        token_usage: Option<TokenUsage>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<String>,
    },
    Reasoning {
        id: String,
        summary: Vec<ReasoningItemReasoningSummary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<Vec<ReasoningItemContent>>,
        encrypted_content: Option<String>,
    },
    LocalShellCall {
        /// Set when using the chat completions API.
        id: Option<String>,
        /// Set when using the Responses API.
        call_id: Option<String>,
        status: LocalShellStatus,
        action: LocalShellAction,
    },
    FunctionCall {
        id: Option<String>,
        name: String,
        // The Responses API returns the function call arguments as a *string* that contains
        // JSON, not as an already‑parsed object. We keep it as a raw string here and let
        // Session::handle_function_call parse it into a Value. This exactly matches the
        // Chat Completions + Responses API behavior.
        arguments: String,
        call_id: String,
    },
    // NOTE: The input schema for `function_call_output` objects that clients send to the
    // OpenAI /v1/responses endpoint is NOT the same shape as the objects the server returns on the
    // SSE stream. When *sending* we must wrap the string output inside an object that includes a
    // required `success` boolean. The upstream TypeScript CLI does this implicitly. To ensure we
    // serialize exactly the expected shape we introduce a dedicated payload struct and flatten it
    // here.
    FunctionCallOutput {
        call_id: String,
        output: FunctionCallOutputPayload,
    },
    #[serde(other)]
    Other,
}

impl From<ResponseInputItem> for ResponseItem {
    fn from(item: ResponseInputItem) -> Self {
        match item {
            ResponseInputItem::Message {
                role,
                content,
                timestamp,
            } => Self::Message {
                id: None,
                role,
                content,
                token_usage: None,
                timestamp,
            },
            ResponseInputItem::FunctionCallOutput { call_id, output } => {
                Self::FunctionCallOutput { call_id, output }
            }
            ResponseInputItem::McpToolCallOutput { call_id, result } => Self::FunctionCallOutput {
                call_id,
                output: FunctionCallOutputPayload {
                    success: Some(result.is_ok()),
                    content: result.map_or_else(
                        |tool_call_err| format!("err: {tool_call_err:?}"),
                        |result| {
                            serde_json::to_string(&result)
                                .unwrap_or_else(|e| format!("JSON serialization error: {e}"))
                        },
                    ),
                },
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LocalShellStatus {
    Completed,
    InProgress,
    Incomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LocalShellAction {
    Exec(LocalShellExecAction),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalShellExecAction {
    pub command: Vec<String>,
    pub timeout_ms: Option<u64>,
    pub working_directory: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReasoningItemReasoningSummary {
    SummaryText { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReasoningItemContent {
    ReasoningText { text: String },
}

impl From<Vec<InputItem>> for ResponseInputItem {
    fn from(items: Vec<InputItem>) -> Self {
        Self::Message {
            role: "user".to_string(),
            content: items
                .into_iter()
                .filter_map(|c| match c {
                    InputItem::Text { text } => Some(ContentItem::InputText { text }),
                    InputItem::Image { image_url } => Some(ContentItem::InputImage { image_url }),
                    InputItem::LocalImage { path } => match std::fs::read(&path) {
                        Ok(bytes) => {
                            let mime = mime_guess::from_path(&path)
                                .first()
                                .map(|m| m.essence_str().to_owned())
                                .unwrap_or_else(|| "application/octet-stream".to_string());
                            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                            Some(ContentItem::InputImage {
                                image_url: format!("data:{mime};base64,{encoded}"),
                            })
                        }
                        Err(err) => {
                            tracing::warn!(
                                "Skipping image {} – could not read file: {}",
                                path.display(),
                                err
                            );
                            None
                        }
                    },
                })
                .collect::<Vec<ContentItem>>(),
            timestamp: Some(generate_timestamp()),
        }
    }
}

/// If the `name` of a `ResponseItem::FunctionCall` is either `container.exec`
/// or shell`, the `arguments` field should deserialize to this struct.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ShellToolCallParams {
    pub command: Vec<String>,
    pub workdir: Option<String>,

    /// This is the maximum time in seconds that the command is allowed to run.
    #[serde(rename = "timeout")]
    // The wire format uses `timeout`, which has ambiguous units, so we use
    // `timeout_ms` as the field name so it is clear in code.
    pub timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub with_escalated_permissions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionCallOutputPayload {
    pub content: String,
    pub success: Option<bool>,
}

// The Responses API expects two *different* shapes depending on success vs failure:
//   • success → output is a plain string (no nested object)
//   • failure → output is an object { content, success:false }
// The upstream TypeScript CLI implements this by special‑casing the serialize path.
// We replicate that behavior with a manual Serialize impl.

impl Serialize for FunctionCallOutputPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // The upstream TypeScript CLI always serializes `output` as a *plain string* regardless
        // of whether the function call succeeded or failed. The boolean is purely informational
        // for local bookkeeping and is NOT sent to the OpenAI endpoint. Sending the nested object
        // form `{ content, success:false }` triggers the 400 we are still seeing. Mirror the JS CLI
        // exactly: always emit a bare string.

        serializer.serialize_str(&self.content)
    }
}

impl<'de> Deserialize<'de> for FunctionCallOutputPayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(FunctionCallOutputPayload {
            content: s,
            success: None,
        })
    }
}

// Implement Display so callers can treat the payload like a plain string when logging or doing
// trivial substring checks in tests (existing tests call `.contains()` on the output). Display
// returns the raw `content` field.

impl std::fmt::Display for FunctionCallOutputPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.content)
    }
}

impl std::ops::Deref for FunctionCallOutputPayload {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.content
    }
}

/// Generate a timestamp string in the same format as session timestamps.
/// Format: "YYYY-MM-DDTHH:MM:SS.sssZ" (ISO 8601 with millisecond precision in UTC)
pub fn generate_timestamp() -> String {
    let timestamp_format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");
    OffsetDateTime::now_utc()
        .format(timestamp_format)
        .unwrap_or_else(|_| "1970-01-01T00:00:00.000Z".to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn serializes_success_as_plain_string() {
        let item = ResponseInputItem::FunctionCallOutput {
            call_id: "call1".into(),
            output: FunctionCallOutputPayload {
                content: "ok".into(),
                success: None,
            },
        };

        let json = serde_json::to_string(&item).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Success case -> output should be a plain string
        assert_eq!(v.get("output").unwrap().as_str().unwrap(), "ok");
    }

    #[test]
    fn serializes_failure_as_string() {
        let item = ResponseInputItem::FunctionCallOutput {
            call_id: "call1".into(),
            output: FunctionCallOutputPayload {
                content: "bad".into(),
                success: Some(false),
            },
        };

        let json = serde_json::to_string(&item).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(v.get("output").unwrap().as_str().unwrap(), "bad");
    }

    #[test]
    fn message_with_token_usage_and_timestamp() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            cached_input_tokens: Some(25),
            reasoning_output_tokens: None,
        };

        let timestamp = "2025-07-15T10:30:45.123Z".to_string();

        let message = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "Hello".to_string(),
            }],
            token_usage: Some(usage.clone()),
            timestamp: Some(timestamp.clone()),
        };

        // Test serialization
        let json = serde_json::to_string(&message).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "message");
        assert_eq!(parsed["role"], "assistant");
        assert_eq!(parsed["content"][0]["text"], "Hello");
        assert_eq!(parsed["token_usage"]["input_tokens"], 100);
        assert_eq!(parsed["token_usage"]["output_tokens"], 50);
        assert_eq!(parsed["token_usage"]["total_tokens"], 150);
        assert_eq!(parsed["timestamp"], timestamp);

        // Test deserialization
        let deserialized: ResponseItem = serde_json::from_str(&json).unwrap();
        if let ResponseItem::Message {
            role,
            content,
            token_usage: Some(token_usage),
            timestamp: Some(ts),
            ..
        } = deserialized
        {
            assert_eq!(role, "assistant");
            assert_eq!(content.len(), 1);
            assert_eq!(token_usage.input_tokens, 100);
            assert_eq!(token_usage.output_tokens, 50);
            assert_eq!(ts, timestamp);
        } else {
            panic!("Expected Message with token_usage and timestamp");
        }
    }

    #[test]
    fn message_without_optional_fields() {
        let message = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Hi".to_string(),
            }],
            token_usage: None,
            timestamp: None,
        };

        // Test serialization - optional fields should be omitted
        let json = serde_json::to_string(&message).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "message");
        assert_eq!(parsed["role"], "user");
        assert!(parsed.get("token_usage").is_none());
        assert!(parsed.get("timestamp").is_none());

        // Test deserialization - should work with missing fields
        let deserialized: ResponseItem = serde_json::from_str(&json).unwrap();
        if let ResponseItem::Message {
            role,
            token_usage,
            timestamp,
            ..
        } = deserialized
        {
            assert_eq!(role, "user");
            assert!(token_usage.is_none());
            assert!(timestamp.is_none());
        } else {
            panic!("Expected Message without optional fields");
        }
    }

    #[test]
    fn generate_timestamp_format() {
        let timestamp = generate_timestamp();

        // Should be valid ISO 8601 format: YYYY-MM-DDTHH:MM:SS.sssZ
        let parts: Vec<&str> = timestamp.split('T').collect();
        assert_eq!(parts.len(), 2);

        let date_part = parts[0];
        let time_part = parts[1];

        // Date should be YYYY-MM-DD format
        assert_eq!(date_part.len(), 10);
        assert!(date_part.contains('-'));

        // Time should end with Z and have milliseconds
        assert!(time_part.ends_with('Z'));
        assert!(time_part.contains('.'));

        // Should be able to parse as a valid timestamp
        assert!(timestamp.len() >= 20); // Minimum ISO format length
        assert!(timestamp.len() <= 30); // Maximum reasonable length
    }

    #[test]
    fn user_message_from_input_items_has_timestamp() {
        use crate::protocol::InputItem;

        let input = vec![InputItem::Text {
            text: "Hello, world!".to_string(),
        }];

        let response_input_item = ResponseInputItem::from(input);

        if let ResponseInputItem::Message {
            role,
            content,
            timestamp,
        } = response_input_item
        {
            assert_eq!(role, "user");
            assert_eq!(content.len(), 1);
            assert!(timestamp.is_some());

            // Verify timestamp format
            let ts = timestamp.unwrap();
            assert!(ts.ends_with('Z'));
            assert!(ts.contains('T'));
            assert!(ts.len() >= 20);
        } else {
            panic!("Expected ResponseInputItem::Message");
        }
    }

    #[test]
    fn user_message_timestamp_preserved_in_conversion() {
        use crate::protocol::InputItem;

        let input = vec![InputItem::Text {
            text: "Test message".to_string(),
        }];

        let response_input_item = ResponseInputItem::from(input);
        let response_item = ResponseItem::from(response_input_item);

        if let ResponseItem::Message {
            role,
            content,
            timestamp,
            token_usage,
            ..
        } = response_item
        {
            assert_eq!(role, "user");
            assert_eq!(content.len(), 1);
            assert!(timestamp.is_some());
            assert!(token_usage.is_none());

            // Verify timestamp format is preserved
            let ts = timestamp.unwrap();
            assert!(ts.ends_with('Z'));
            assert!(ts.contains('T'));
        } else {
            panic!("Expected ResponseItem::Message");
        }
    }

    #[test]
    fn deserialize_shell_tool_call_params() {
        let json = r#"{
            "command": ["ls", "-l"],
            "workdir": "/tmp",
            "timeout": 1000
        }"#;

        let params: ShellToolCallParams = serde_json::from_str(json).unwrap();
        assert_eq!(
            ShellToolCallParams {
                command: vec!["ls".to_string(), "-l".to_string()],
                workdir: Some("/tmp".to_string()),
                timeout_ms: Some(1000),
                with_escalated_permissions: None,
                justification: None,
            },
            params
        );
    }
}
