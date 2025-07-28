use std::collections::HashMap;

use base64::Engine;
use mcp_types::CallToolResult;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::ser::Serializer;

use crate::protocol::InputItem;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseInputItem {
    Message {
        role: String,
        content: Vec<ContentItem>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentItem {
    InputText { text: String },
    InputImage { image_url: String },
    OutputText { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseItem {
    Message {
        id: Option<String>,
        role: String,
        content: Vec<ContentItem>,
    },
    Reasoning {
        id: String,
        summary: Vec<ReasoningItemReasoningSummary>,
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
            ResponseInputItem::Message { role, content } => Self::Message {
                role,
                content,
                id: None,
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
                    is_user_feedback: false,
                },
            },
        }
    }
}

impl ResponseItem {
    /// Returns true if this item represents user feedback
    #[allow(dead_code)]
    pub(crate) fn is_user_feedback(&self) -> bool {
        match self {
            Self::FunctionCallOutput { output, .. } => output.is_user_feedback,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalShellStatus {
    Completed,
    InProgress,
    Incomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LocalShellAction {
    Exec(LocalShellExecAction),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalShellExecAction {
    pub command: Vec<String>,
    pub timeout_ms: Option<u64>,
    pub working_directory: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReasoningItemReasoningSummary {
    SummaryText { text: String },
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
}

#[derive(Debug, Clone)]
pub struct FunctionCallOutputPayload {
    pub content: String,
    #[allow(dead_code)]
    pub success: Option<bool>,
    pub is_user_feedback: bool,
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
        use serde::ser::SerializeStruct;
        // Always emit an object with all three fields
        let mut state = serializer.serialize_struct("FunctionCallOutputPayload", 3)?;
        state.serialize_field("content", &self.content)?;
        state.serialize_field("success", &self.success)?;
        state.serialize_field("is_user_feedback", &self.is_user_feedback)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for FunctionCallOutputPayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // The Responses API (and our sanitize step) send this payload as a bare string.
        // That form carries no `success` or `is_user_feedback` flags, so we must supply
        // defaults here. We pick `None` for success and `false` for is_user_feedback as
        // conservative fallbacks; the full object form is preserved in rollout JSONL.
        let s = String::deserialize(deserializer)?;
        Ok(FunctionCallOutputPayload {
            content: s,
            success: None,
            is_user_feedback: false,
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn serializes_success_as_object_with_flag() {
        let item = ResponseInputItem::FunctionCallOutput {
            call_id: "call1".into(),
            output: FunctionCallOutputPayload {
                content: "ok".into(),
                success: None,
                is_user_feedback: false,
            },
        };

        let json = serde_json::to_string(&item).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Output should be an object with content and is_user_feedback
        assert_eq!(v.get("output").unwrap()["content"].as_str().unwrap(), "ok");
        assert!(
            !v.get("output").unwrap()["is_user_feedback"]
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn serializes_failure_with_flag() {
        let item = ResponseInputItem::FunctionCallOutput {
            call_id: "call1".into(),
            output: FunctionCallOutputPayload {
                content: "bad".into(),
                success: Some(false),
                is_user_feedback: true,
            },
        };

        let json = serde_json::to_string(&item).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(v.get("output").unwrap()["content"].as_str().unwrap(), "bad");
        assert!(
            v.get("output").unwrap()["is_user_feedback"]
                .as_bool()
                .unwrap()
        );
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
            },
            params
        );
    }

    #[test]
    fn deserialize_user_feedback() {
        // The wire format we deserialize from the LLM side is a bare string for `output`.
        let json = r#"{"type":"function_call_output","call_id":"call_123","output":"This is a test feedback"}"#;
        let feedback: ResponseItem = serde_json::from_str(json).unwrap();
        if let ResponseItem::FunctionCallOutput { call_id, output } = feedback {
            assert_eq!(call_id, "call_123");
            assert_eq!(output.content, "This is a test feedback");
            assert_eq!(output.success, None);
            assert!(!output.is_user_feedback);
        } else {
            panic!("Expected FunctionCallOutput variant");
        }
    }

    #[test]
    fn serialize_deserialize_response_input_user_feedback() {
        let user_feedback = ResponseInputItem::FunctionCallOutput {
            call_id: "call_456".to_string(),
            output: FunctionCallOutputPayload {
                content: "Test user feedback".to_string(),
                success: None,
                is_user_feedback: true,
            },
        };

        let json = serde_json::to_string(&user_feedback).unwrap();

        // Now the output is an object with the flag
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["output"]["content"], "Test user feedback");
        assert_eq!(v["output"]["is_user_feedback"], true);
    }

    #[test]
    fn user_feedback_to_llm_compatible_conversion() {
        let user_feedback = ResponseItem::FunctionCallOutput {
            call_id: "call_6789".to_string(),
            output: FunctionCallOutputPayload {
                content: "This is user feedback".to_string(),
                success: None,
                is_user_feedback: true,
            },
        };

        // Test that we can identify user feedback
        assert!(user_feedback.is_user_feedback());

        if let ResponseItem::FunctionCallOutput { call_id, output } = user_feedback {
            assert_eq!(call_id, "call_6789");
            assert_eq!(output.content, "This is user feedback");
            assert_eq!(output.success, None);
            assert!(output.is_user_feedback);
        } else {
            panic!("Expected FunctionCallOutput variant");
        }
    }

    #[test]
    fn non_user_feedback_to_llm_compatible_unchanged() {
        let message = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Hello".to_string(),
            }],
        };

        // Test that regular messages are not identified as user feedback
        assert!(!message.is_user_feedback());

        if let ResponseItem::Message { role, content, .. } = message {
            assert_eq!(role, "user");
            assert_eq!(content.len(), 1);
        } else {
            panic!("Expected Message variant to remain unchanged");
        }
    }
}
