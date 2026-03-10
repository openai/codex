use crate::codex::Session;
use crate::codex::TurnContext;
use crate::tools::TELEMETRY_PREVIEW_MAX_BYTES;
use crate::tools::TELEMETRY_PREVIEW_MAX_LINES;
use crate::tools::TELEMETRY_PREVIEW_TRUNCATION_NOTICE;
use crate::truncate::TruncationPolicy;
use crate::truncate::formatted_truncate_text;
use crate::turn_diff_tracker::TurnDiffTracker;
use crate::unified_exec::resolve_max_tokens;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ShellToolCallParams;
use codex_protocol::models::function_call_output_content_items_to_text;
use codex_utils_string::take_bytes_at_char_boundary;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;
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

pub trait ToolOutput: Send {
    fn log_preview(&self) -> String;

    fn success_for_logging(&self) -> bool;

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem;

    fn code_mode_result(&self, payload: &ToolPayload) -> JsonValue {
        response_input_to_code_mode_result(self.to_response_item("", payload))
    }
}

impl ToolOutput for CallToolResult {
    fn log_preview(&self) -> String {
        let output = self.as_function_call_output_payload();
        let preview = output.body.to_text().unwrap_or_else(|| output.to_string());
        telemetry_preview(&preview)
    }

    fn success_for_logging(&self) -> bool {
        self.success()
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::McpToolCallOutput {
            call_id: call_id.to_string(),
            output: self.clone(),
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        serde_json::to_value(self).unwrap_or_else(|err| {
            JsonValue::String(format!("failed to serialize mcp result: {err}"))
        })
    }
}

pub struct FunctionToolOutput {
    pub body: Vec<FunctionCallOutputContentItem>,
    pub success: Option<bool>,
}

impl FunctionToolOutput {
    pub fn from_text(text: String, success: Option<bool>) -> Self {
        Self {
            body: vec![FunctionCallOutputContentItem::InputText { text }],
            success,
        }
    }

    pub fn from_content(
        content: Vec<FunctionCallOutputContentItem>,
        success: Option<bool>,
    ) -> Self {
        Self {
            body: content,
            success,
        }
    }

    pub fn into_text(self) -> String {
        function_call_output_content_items_to_text(&self.body).unwrap_or_default()
    }
}

impl ToolOutput for FunctionToolOutput {
    fn log_preview(&self) -> String {
        telemetry_preview(
            &function_call_output_content_items_to_text(&self.body).unwrap_or_default(),
        )
    }

    fn success_for_logging(&self) -> bool {
        self.success.unwrap_or(true)
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        function_tool_response(call_id, payload, self.body.clone(), self.success)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecCommandToolOutput {
    pub event_call_id: String,
    pub chunk_id: String,
    pub wall_time: Duration,
    /// Raw bytes returned for this unified exec call before any truncation.
    pub raw_output: Vec<u8>,
    pub max_output_tokens: Option<usize>,
    pub process_id: Option<String>,
    pub exit_code: Option<i32>,
    pub original_token_count: Option<usize>,
    pub session_command: Option<Vec<String>>,
}

impl ToolOutput for ExecCommandToolOutput {
    fn log_preview(&self) -> String {
        telemetry_preview(&self.response_text())
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        function_tool_response(
            call_id,
            payload,
            vec![FunctionCallOutputContentItem::InputText {
                text: self.response_text(),
            }],
            Some(true),
        )
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        #[derive(Serialize)]
        struct UnifiedExecCodeModeResult {
            #[serde(skip_serializing_if = "Option::is_none")]
            chunk_id: Option<String>,
            wall_time_seconds: f64,
            #[serde(skip_serializing_if = "Option::is_none")]
            exit_code: Option<i32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            session_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            original_token_count: Option<usize>,
            output: String,
        }

        let result = UnifiedExecCodeModeResult {
            chunk_id: (!self.chunk_id.is_empty()).then(|| self.chunk_id.clone()),
            wall_time_seconds: self.wall_time.as_secs_f64(),
            exit_code: self.exit_code,
            session_id: self.process_id.clone(),
            original_token_count: self.original_token_count,
            output: self.truncated_output(),
        };

        serde_json::to_value(result).unwrap_or_else(|err| {
            JsonValue::String(format!("failed to serialize exec result: {err}"))
        })
    }
}

impl ExecCommandToolOutput {
    pub(crate) fn truncated_output(&self) -> String {
        let text = String::from_utf8_lossy(&self.raw_output).to_string();
        let max_tokens = resolve_max_tokens(self.max_output_tokens);
        formatted_truncate_text(&text, TruncationPolicy::Tokens(max_tokens))
    }

    fn response_text(&self) -> String {
        let mut sections = Vec::new();

        if !self.chunk_id.is_empty() {
            sections.push(format!("Chunk ID: {}", self.chunk_id));
        }

        let wall_time_seconds = self.wall_time.as_secs_f64();
        sections.push(format!("Wall time: {wall_time_seconds:.4} seconds"));

        if let Some(exit_code) = self.exit_code {
            sections.push(format!("Process exited with code {exit_code}"));
        }

        if let Some(process_id) = &self.process_id {
            sections.push(format!("Process running with session ID {process_id}"));
        }

        if let Some(original_token_count) = self.original_token_count {
            sections.push(format!("Original token count: {original_token_count}"));
        }

        sections.push("Output:".to_string());
        sections.push(self.truncated_output());

        sections.join("\n")
    }
}

fn response_input_to_code_mode_result(response: ResponseInputItem) -> JsonValue {
    match response {
        ResponseInputItem::Message { content, .. } => content_items_to_code_mode_result(
            &content
                .into_iter()
                .map(|item| match item {
                    codex_protocol::models::ContentItem::InputText { text }
                    | codex_protocol::models::ContentItem::OutputText { text } => {
                        FunctionCallOutputContentItem::InputText { text }
                    }
                    codex_protocol::models::ContentItem::InputImage { image_url } => {
                        FunctionCallOutputContentItem::InputImage {
                            image_url,
                            detail: None,
                        }
                    }
                })
                .collect::<Vec<_>>(),
        ),
        ResponseInputItem::FunctionCallOutput { output, .. }
        | ResponseInputItem::CustomToolCallOutput { output, .. } => match output.body {
            FunctionCallOutputBody::Text(text) => JsonValue::String(text),
            FunctionCallOutputBody::ContentItems(items) => {
                content_items_to_code_mode_result(&items)
            }
        },
        ResponseInputItem::McpToolCallOutput { output, .. } => {
            output.code_mode_result(&ToolPayload::Mcp {
                server: String::new(),
                tool: String::new(),
                raw_arguments: String::new(),
            })
        }
    }
}

fn content_items_to_code_mode_result(items: &[FunctionCallOutputContentItem]) -> JsonValue {
    JsonValue::String(
        items
            .iter()
            .filter_map(|item| match item {
                FunctionCallOutputContentItem::InputText { text } if !text.trim().is_empty() => {
                    Some(text.clone())
                }
                FunctionCallOutputContentItem::InputImage { image_url, .. }
                    if !image_url.trim().is_empty() =>
                {
                    Some(image_url.clone())
                }
                FunctionCallOutputContentItem::InputText { .. }
                | FunctionCallOutputContentItem::InputImage { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn function_tool_response(
    call_id: &str,
    payload: &ToolPayload,
    body: Vec<FunctionCallOutputContentItem>,
    success: Option<bool>,
) -> ResponseInputItem {
    let body = match body.as_slice() {
        [FunctionCallOutputContentItem::InputText { text }] => {
            FunctionCallOutputBody::Text(text.clone())
        }
        _ => FunctionCallOutputBody::ContentItems(body),
    };

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
    use core_test_support::assert_regex_match;
    use pretty_assertions::assert_eq;

    #[test]
    fn custom_tool_calls_should_roundtrip_as_custom_outputs() {
        let payload = ToolPayload::Custom {
            input: "patch".to_string(),
        };
        let response = FunctionToolOutput::from_text("patched".to_string(), Some(true))
            .to_response_item("call-42", &payload);

        match response {
            ResponseInputItem::CustomToolCallOutput { call_id, output } => {
                assert_eq!(call_id, "call-42");
                assert_eq!(output.content_items(), None);
                assert_eq!(output.body.to_text().as_deref(), Some("patched"));
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
        let response = FunctionToolOutput::from_text("ok".to_string(), Some(true))
            .to_response_item("fn-1", &payload);

        match response {
            ResponseInputItem::FunctionCallOutput { call_id, output } => {
                assert_eq!(call_id, "fn-1");
                assert_eq!(output.content_items(), None);
                assert_eq!(output.body.to_text().as_deref(), Some("ok"));
                assert_eq!(output.success, Some(true));
            }
            other => panic!("expected FunctionCallOutput, got {other:?}"),
        }
    }

    #[test]
    fn mcp_code_mode_result_serializes_full_call_tool_result() {
        let output = CallToolResult {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "ignored",
            })],
            structured_content: Some(serde_json::json!({
                "threadId": "thread_123",
                "content": "done",
            })),
            is_error: Some(false),
            meta: Some(serde_json::json!({
                "source": "mcp",
            })),
        };

        let result = output.code_mode_result(&ToolPayload::Mcp {
            server: "server".to_string(),
            tool: "tool".to_string(),
            raw_arguments: "{}".to_string(),
        });

        assert_eq!(
            result,
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": "ignored",
                }],
                "structuredContent": {
                    "threadId": "thread_123",
                    "content": "done",
                },
                "isError": false,
                "_meta": {
                    "source": "mcp",
                },
            })
        );
    }

    #[test]
    fn custom_tool_calls_can_derive_text_from_content_items() {
        let payload = ToolPayload::Custom {
            input: "patch".to_string(),
        };
        let response = FunctionToolOutput::from_content(
            vec![
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
            Some(true),
        )
        .to_response_item("call-99", &payload);

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
        let output = FunctionToolOutput::from_content(
            vec![FunctionCallOutputContentItem::InputText {
                text: "preview".to_string(),
            }],
            Some(true),
        );

        assert_eq!(output.log_preview(), "preview");
        assert_eq!(
            function_call_output_content_items_to_text(&output.body),
            Some("preview".to_string())
        );
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

    #[test]
    fn exec_command_tool_output_formats_truncated_response() {
        let payload = ToolPayload::Function {
            arguments: "{}".to_string(),
        };
        let response = ExecCommandToolOutput {
            event_call_id: "call-42".to_string(),
            chunk_id: "abc123".to_string(),
            wall_time: std::time::Duration::from_millis(1250),
            raw_output: b"token one token two token three token four token five".to_vec(),
            max_output_tokens: Some(4),
            process_id: None,
            exit_code: Some(0),
            original_token_count: Some(10),
            session_command: None,
        }
        .to_response_item("call-42", &payload);

        match response {
            ResponseInputItem::FunctionCallOutput { call_id, output } => {
                assert_eq!(call_id, "call-42");
                assert_eq!(output.success, Some(true));
                let text = output
                    .body
                    .to_text()
                    .expect("exec output should serialize as text");
                assert_regex_match(
                    r#"(?sx)
                    ^Chunk\ ID:\ abc123
                    \nWall\ time:\ \d+\.\d{4}\ seconds
                    \nProcess\ exited\ with\ code\ 0
                    \nOriginal\ token\ count:\ 10
                    \nOutput:
                    \n.*tokens\ truncated.*
                    $"#,
                    &text,
                );
            }
            other => panic!("expected FunctionCallOutput, got {other:?}"),
        }
    }
}
