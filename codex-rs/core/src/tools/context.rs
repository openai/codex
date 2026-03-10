use crate::codex::Session;
use crate::codex::TurnContext;
use crate::tools::TELEMETRY_PREVIEW_MAX_BYTES;
use crate::tools::TELEMETRY_PREVIEW_MAX_LINES;
use crate::tools::TELEMETRY_PREVIEW_TRUNCATION_NOTICE;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ShellToolCallParams;
use codex_utils_string::take_bytes_at_char_boundary;
use std::any::Any;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::Mutex;

pub type SharedTurnDiffTracker = Arc<Mutex<TurnDiffTracker>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCallSource {
    Direct,
    JsRepl,
    CodeMode,
}

#[derive(Clone)]
pub struct ToolInvocation {
    pub session: Arc<Session>,
    pub turn: Arc<TurnContext>,
    pub tracker: SharedTurnDiffTracker,
    pub call_id: String,
    pub tool_name: String,
    pub payload: ToolPayload,
}

#[derive(Clone, Debug)]
pub enum ToolPayload {
    Function {
        arguments: String,
    },
    Custom {
        input: String,
    },
    LocalShell {
        params: ShellToolCallParams,
    },
    Mcp {
        server: String,
        tool: String,
        raw_arguments: String,
    },
}

impl ToolPayload {
    pub fn log_payload(&self) -> Cow<'_, str> {
        match self {
            ToolPayload::Function { arguments } => Cow::Borrowed(arguments),
            ToolPayload::Custom { input } => Cow::Borrowed(input),
            ToolPayload::LocalShell { params } => Cow::Owned(params.command.join(" ")),
            ToolPayload::Mcp { raw_arguments, .. } => Cow::Borrowed(raw_arguments),
        }
    }
}

pub trait ToolOutput: Any + Send {
    fn log_preview(&self) -> String;

    fn success_for_logging(&self) -> bool;

    fn into_response(self: Box<Self>, call_id: &str, payload: &ToolPayload) -> ResponseInputItem;
}

pub type ToolOutputBox = Box<dyn ToolOutput>;

pub struct McpToolOutput {
    pub result: Result<CallToolResult, String>,
}

impl ToolOutput for McpToolOutput {
    fn log_preview(&self) -> String {
        format!("{:?}", self.result)
    }

    fn success_for_logging(&self) -> bool {
        self.result.is_ok()
    }

    fn into_response(self: Box<Self>, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        let Self { result } = *self;
        ResponseInputItem::McpToolCallOutput {
            call_id: call_id.to_string(),
            result,
        }
    }
}

pub struct TextToolOutput {
    pub text: String,
    pub success: Option<bool>,
}

impl ToolOutput for TextToolOutput {
    fn log_preview(&self) -> String {
        telemetry_preview(&self.text)
    }

    fn success_for_logging(&self) -> bool {
        self.success.unwrap_or(true)
    }

    fn into_response(self: Box<Self>, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        let Self { text, success } = *self;
        function_tool_response(
            call_id,
            payload,
            FunctionCallOutputBody::Text(text),
            success,
        )
    }
}

pub struct ContentToolOutput {
    pub content: Vec<FunctionCallOutputContentItem>,
    pub success: Option<bool>,
}

impl ToolOutput for ContentToolOutput {
    fn log_preview(&self) -> String {
        telemetry_preview(
            &FunctionCallOutputBody::ContentItems(self.content.clone())
                .to_text()
                .unwrap_or_default(),
        )
    }

    fn success_for_logging(&self) -> bool {
        self.success.unwrap_or(true)
    }

    fn into_response(self: Box<Self>, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        let Self { content, success } = *self;
        function_tool_response(
            call_id,
            payload,
            FunctionCallOutputBody::ContentItems(content),
            success,
        )
    }
}

fn function_tool_response(
    call_id: &str,
    payload: &ToolPayload,
    body: FunctionCallOutputBody,
    success: Option<bool>,
) -> ResponseInputItem {
    if matches!(payload, ToolPayload::Custom { .. }) {
        return ResponseInputItem::CustomToolCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload { body, success },
        };
    }

    ResponseInputItem::FunctionCallOutput {
        call_id: call_id.to_string(),
        output: FunctionCallOutputPayload { body, success },
    }
}

fn telemetry_preview(content: &str) -> String {
    let truncated_slice = take_bytes_at_char_boundary(content, TELEMETRY_PREVIEW_MAX_BYTES);
    let truncated_by_bytes = truncated_slice.len() < content.len();

    let mut preview = String::new();
    let mut lines_iter = truncated_slice.lines();
    for idx in 0..TELEMETRY_PREVIEW_MAX_LINES {
        match lines_iter.next() {
            Some(line) => {
                if idx > 0 {
                    preview.push('\n');
                }
                preview.push_str(line);
            }
            None => break,
        }
    }
    let truncated_by_lines = lines_iter.next().is_some();

    if !truncated_by_bytes && !truncated_by_lines {
        return content.to_string();
    }

    if preview.len() < truncated_slice.len()
        && truncated_slice
            .as_bytes()
            .get(preview.len())
            .is_some_and(|byte| *byte == b'\n')
    {
        preview.push('\n');
    }

    if !preview.is_empty() && !preview.ends_with('\n') {
        preview.push('\n');
    }
    preview.push_str(TELEMETRY_PREVIEW_TRUNCATION_NOTICE);

    preview
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn custom_tool_calls_should_roundtrip_as_custom_outputs() {
        let payload = ToolPayload::Custom {
            input: "patch".to_string(),
        };
        let response = Box::new(TextToolOutput {
            text: "patched".to_string(),
            success: Some(true),
        })
        .into_response("call-42", &payload);

        match response {
            ResponseInputItem::CustomToolCallOutput { call_id, output } => {
                assert_eq!(call_id, "call-42");
                assert_eq!(output.text_content(), Some("patched"));
                assert!(output.content_items().is_none());
                assert_eq!(output.success, Some(true));
            }
            other => panic!("expected CustomToolCallOutput, got {other:?}"),
        }
    }

    #[test]
    fn function_payloads_remain_function_outputs() {
        let payload = ToolPayload::Function {
            arguments: "{}".to_string(),
        };
        let response = Box::new(TextToolOutput {
            text: "ok".to_string(),
            success: Some(true),
        })
        .into_response("fn-1", &payload);

        match response {
            ResponseInputItem::FunctionCallOutput { call_id, output } => {
                assert_eq!(call_id, "fn-1");
                assert_eq!(output.text_content(), Some("ok"));
                assert!(output.content_items().is_none());
                assert_eq!(output.success, Some(true));
            }
            other => panic!("expected FunctionCallOutput, got {other:?}"),
        }
    }

    #[test]
    fn custom_tool_calls_can_derive_text_from_content_items() {
        let payload = ToolPayload::Custom {
            input: "patch".to_string(),
        };
        let response = Box::new(ContentToolOutput {
            content: vec![
                FunctionCallOutputContentItem::InputText {
                    text: "line 1".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,AAA".to_string(),
                    detail: None,
                },
                FunctionCallOutputContentItem::InputText {
                    text: "line 2".to_string(),
                },
            ],
            success: Some(true),
        })
        .into_response("call-99", &payload);

        match response {
            ResponseInputItem::CustomToolCallOutput { call_id, output } => {
                let expected = vec![
                    FunctionCallOutputContentItem::InputText {
                        text: "line 1".to_string(),
                    },
                    FunctionCallOutputContentItem::InputImage {
                        image_url: "data:image/png;base64,AAA".to_string(),
                        detail: None,
                    },
                    FunctionCallOutputContentItem::InputText {
                        text: "line 2".to_string(),
                    },
                ];
                assert_eq!(call_id, "call-99");
                assert_eq!(output.content_items(), Some(expected.as_slice()));
                assert_eq!(output.body.to_text().as_deref(), Some("line 1\nline 2"));
                assert_eq!(output.success, Some(true));
            }
            other => panic!("expected CustomToolCallOutput, got {other:?}"),
        }
    }

    #[test]
    fn log_preview_uses_content_items_when_plain_text_is_missing() {
        let output = ContentToolOutput {
            content: vec![FunctionCallOutputContentItem::InputText {
                text: "preview".to_string(),
            }],
            success: Some(true),
        };

        assert_eq!(output.log_preview(), "preview");
    }

    #[test]
    fn telemetry_preview_returns_original_within_limits() {
        let content = "short output";
        assert_eq!(telemetry_preview(content), content);
    }

    #[test]
    fn telemetry_preview_truncates_by_bytes() {
        let content = "x".repeat(TELEMETRY_PREVIEW_MAX_BYTES + 8);
        let preview = telemetry_preview(&content);

        assert!(preview.contains(TELEMETRY_PREVIEW_TRUNCATION_NOTICE));
        assert!(
            preview.len()
                <= TELEMETRY_PREVIEW_MAX_BYTES + TELEMETRY_PREVIEW_TRUNCATION_NOTICE.len() + 1
        );
    }

    #[test]
    fn telemetry_preview_truncates_by_lines() {
        let content = (0..(TELEMETRY_PREVIEW_MAX_LINES + 5))
            .map(|idx| format!("line {idx}"))
            .collect::<Vec<_>>()
            .join("\n");

        let preview = telemetry_preview(&content);
        let lines: Vec<&str> = preview.lines().collect();

        assert!(lines.len() <= TELEMETRY_PREVIEW_MAX_LINES + 1);
        assert_eq!(lines.last(), Some(&TELEMETRY_PREVIEW_TRUNCATION_NOTICE));
    }
}
