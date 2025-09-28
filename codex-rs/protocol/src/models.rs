use std::collections::HashMap;

use base64::Engine;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::ser::Serializer;
use ts_rs::TS;

use crate::protocol::InputItem;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
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
    CustomToolCallOutput {
        call_id: String,
        output: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentItem {
    InputText { text: String },
    InputImage { image_url: String },
    OutputText { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseItem {
    Message {
        #[serde(skip_serializing)]
        id: Option<String>,
        role: String,
        content: Vec<ContentItem>,
    },
    Reasoning {
        #[serde(default, skip_serializing)]
        id: String,
        summary: Vec<ReasoningItemReasoningSummary>,
        #[serde(default, skip_serializing_if = "should_serialize_reasoning_content")]
        content: Option<Vec<ReasoningItemContent>>,
        encrypted_content: Option<String>,
    },
    LocalShellCall {
        /// Set when using the chat completions API.
        #[serde(skip_serializing)]
        id: Option<String>,
        /// Set when using the Responses API.
        call_id: Option<String>,
        status: LocalShellStatus,
        action: LocalShellAction,
    },
    FunctionCall {
        #[serde(skip_serializing)]
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
    CustomToolCall {
        #[serde(skip_serializing)]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,

        call_id: String,
        name: String,
        input: String,
    },
    CustomToolCallOutput {
        call_id: String,
        output: String,
    },
    // Emitted by the Responses API when the agent triggers a web search.
    // Example payload (from SSE `response.output_item.done`):
    // {
    //   "id":"ws_...",
    //   "type":"web_search_call",
    //   "status":"completed",
    //   "action": {"type":"search","query":"weather: San Francisco, CA"}
    // }
    WebSearchCall {
        #[serde(skip_serializing)]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
        action: WebSearchAction,
    },
    #[serde(other)]
    Other,
}

fn should_serialize_reasoning_content(content: &Option<Vec<ReasoningItemContent>>) -> bool {
    match content {
        Some(content) => !content
            .iter()
            .any(|c| matches!(c, ReasoningItemContent::ReasoningText { .. })),
        None => false,
    }
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
            ResponseInputItem::McpToolCallOutput { call_id, result } => {
                let output = match result {
                    Ok(result) => FunctionCallOutputPayload::from_call_tool_result(&result),
                    Err(tool_call_err) => FunctionCallOutputPayload {
                        content: format!("err: {tool_call_err:?}"),
                        content_items: None,
                        success: Some(false),
                    },
                };

                Self::FunctionCallOutput { call_id, output }
            }
            ResponseInputItem::CustomToolCallOutput { call_id, output } => {
                Self::CustomToolCallOutput { call_id, output }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "snake_case")]
pub enum LocalShellStatus {
    Completed,
    InProgress,
    Incomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LocalShellAction {
    Exec(LocalShellExecAction),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct LocalShellExecAction {
    pub command: Vec<String>,
    pub timeout_ms: Option<u64>,
    pub working_directory: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSearchAction {
    Search {
        query: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReasoningItemReasoningSummary {
    SummaryText { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReasoningItemContent {
    ReasoningText { text: String },
    Text { text: String },
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
                                .unwrap_or_else(|| "image".to_string());
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
#[derive(Deserialize, Debug, Clone, PartialEq, TS)]
pub struct ShellToolCallParams {
    pub command: Vec<String>,
    pub workdir: Option<String>,

    /// This is the maximum time in milliseconds that the command is allowed to run.
    #[serde(alias = "timeout")]
    pub timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub with_escalated_permissions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FunctionCallOutputContentItem {
    InputText { text: String },
    InputImage { image_url: String },
}

#[derive(Debug, Clone, PartialEq, TS)]
pub struct FunctionCallOutputPayload {
    pub content: String,
    pub content_items: Option<Vec<FunctionCallOutputContentItem>>,
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
        if let Some(items) = &self.content_items {
            items.serialize(serializer)
        } else {
            // The upstream TypeScript CLI historically serialized `output` as a plain string.
            // Preserve that behavior for text-only payloads so we do not regress existing
            // integrations. When images are present we instead emit the array form introduced by
            // the Responses API so the model receives a renderable image instead of raw base64.
            serializer.serialize_str(&self.content)
        }
    }
}

impl<'de> Deserialize<'de> for FunctionCallOutputPayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::String(s) => Ok(FunctionCallOutputPayload {
                content: s,
                content_items: None,
                success: None,
            }),
            serde_json::Value::Array(values) => {
                let mut items = Vec::with_capacity(values.len());
                for value in values {
                    let item: FunctionCallOutputContentItem =
                        serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                    items.push(item);
                }

                let content = serde_json::to_string(&items)
                    .map_err(|err| serde::de::Error::custom(err.to_string()))?;

                Ok(FunctionCallOutputPayload {
                    content,
                    content_items: Some(items),
                    success: None,
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "expected string or array for function call output payload, got {other}"
            ))),
        }
    }
}

impl FunctionCallOutputPayload {
    pub fn from_call_tool_result(call_tool_result: &CallToolResult) -> Self {
        let CallToolResult {
            content,
            is_error,
            structured_content,
        } = call_tool_result;

        let is_success = is_error != &Some(true);

        if let Some(structured_content) = structured_content
            && structured_content != &serde_json::Value::Null
        {
            match serde_json::to_string(structured_content) {
                Ok(serialized_structured_content) => {
                    return FunctionCallOutputPayload {
                        content: serialized_structured_content,
                        content_items: None,
                        success: Some(is_success),
                    };
                }
                Err(err) => {
                    return FunctionCallOutputPayload {
                        content: err.to_string(),
                        content_items: None,
                        success: Some(false),
                    };
                }
            }
        }

        let serialized_content = match serde_json::to_string(content) {
            Ok(serialized_content) => serialized_content,
            Err(err) => {
                return FunctionCallOutputPayload {
                    content: err.to_string(),
                    content_items: None,
                    success: Some(false),
                };
            }
        };

        let content_items = convert_content_blocks_to_items(content);

        FunctionCallOutputPayload {
            content: serialized_content,
            content_items,
            success: Some(is_success),
        }
    }
}

fn convert_content_blocks_to_items(
    blocks: &[ContentBlock],
) -> Option<Vec<FunctionCallOutputContentItem>> {
    let mut saw_image = false;
    let mut items = Vec::with_capacity(blocks.len());

    for block in blocks {
        match block {
            ContentBlock::TextContent(text) => {
                items.push(FunctionCallOutputContentItem::InputText {
                    text: text.text.clone(),
                });
            }
            ContentBlock::ImageContent(image) => {
                saw_image = true;
                items.push(FunctionCallOutputContentItem::InputImage {
                    image_url: format!("data:{};base64,{}", image.mime_type, image.data),
                });
            }
            _ => return None,
        }
    }

    if saw_image { Some(items) } else { None }
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

// (Moved event mapping logic into codex-core to avoid coupling protocol to UI-facing events.)

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use mcp_types::ImageContent;
    use mcp_types::TextContent;

    #[test]
    fn serializes_success_as_plain_string() -> Result<()> {
        let item = ResponseInputItem::FunctionCallOutput {
            call_id: "call1".into(),
            output: FunctionCallOutputPayload {
                content: "ok".into(),
                content_items: None,
                success: None,
            },
        };

        let json = serde_json::to_string(&item)?;
        let v: serde_json::Value = serde_json::from_str(&json)?;

        // Success case -> output should be a plain string
        assert_eq!(v.get("output").unwrap().as_str().unwrap(), "ok");
        Ok(())
    }

    #[test]
    fn serializes_failure_as_string() -> Result<()> {
        let item = ResponseInputItem::FunctionCallOutput {
            call_id: "call1".into(),
            output: FunctionCallOutputPayload {
                content: "bad".into(),
                content_items: None,
                success: Some(false),
            },
        };

        let json = serde_json::to_string(&item)?;
        let v: serde_json::Value = serde_json::from_str(&json)?;

        assert_eq!(v.get("output").unwrap().as_str().unwrap(), "bad");
        Ok(())
    }

    #[test]
    fn serializes_image_outputs_as_array() -> Result<()> {
        let call_tool_result = CallToolResult {
            content: vec![
                ContentBlock::TextContent(TextContent {
                    annotations: None,
                    text: "caption".into(),
                    r#type: "text".into(),
                }),
                ContentBlock::ImageContent(ImageContent {
                    annotations: None,
                    data: "BASE64".into(),
                    mime_type: "image/png".into(),
                    r#type: "image".into(),
                }),
            ],
            is_error: None,
            structured_content: None,
        };

        let payload = FunctionCallOutputPayload::from_call_tool_result(&call_tool_result);
        assert_eq!(payload.success, Some(true));
        let items = payload.content_items.clone().expect("content items");
        assert_eq!(
            items,
            vec![
                FunctionCallOutputContentItem::InputText {
                    text: "caption".into(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,BASE64".into(),
                },
            ]
        );

        let item = ResponseInputItem::FunctionCallOutput {
            call_id: "call1".into(),
            output: payload,
        };

        let json = serde_json::to_string(&item)?;
        let v: serde_json::Value = serde_json::from_str(&json)?;

        let output = v.get("output").expect("output field");
        assert!(output.is_array(), "expected array output");

        Ok(())
    }

    #[test]
    fn deserializes_array_payload_into_items() -> Result<()> {
        let json = r#"[
            {"type": "input_text", "text": "note"},
            {"type": "input_image", "image_url": "data:image/png;base64,XYZ"}
        ]"#;

        let payload: FunctionCallOutputPayload = serde_json::from_str(json)?;

        assert_eq!(payload.success, None);
        let expected_items = vec![
            FunctionCallOutputContentItem::InputText {
                text: "note".into(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,XYZ".into(),
            },
        ];
        assert_eq!(payload.content_items, Some(expected_items.clone()));

        let expected_content = serde_json::to_string(&expected_items)?;
        assert_eq!(payload.content, expected_content);

        Ok(())
    }

    #[test]
    fn deserialize_shell_tool_call_params() -> Result<()> {
        let json = r#"{
            "command": ["ls", "-l"],
            "workdir": "/tmp",
            "timeout": 1000
        }"#;

        let params: ShellToolCallParams = serde_json::from_str(json)?;
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
        Ok(())
    }
}
