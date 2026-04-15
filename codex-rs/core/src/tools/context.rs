use crate::codex::Session;
use crate::codex::TurnContext;
use crate::original_image_detail::sanitize_original_image_detail;
use crate::tools::TELEMETRY_PREVIEW_MAX_BYTES;
use crate::tools::TELEMETRY_PREVIEW_MAX_LINES;
use crate::tools::TELEMETRY_PREVIEW_TRUNCATION_NOTICE;
use crate::turn_diff_tracker::TurnDiffTracker;
use crate::unified_exec::resolve_max_tokens;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::SearchToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use codex_protocol::models::function_call_output_content_items_to_text;
use codex_tools::ToolName;
use codex_tools::ToolSearchOutputTool;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::formatted_truncate_text;
use codex_utils_string::take_bytes_at_char_boundary;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub type SharedTurnDiffTracker = Arc<Mutex<TurnDiffTracker>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolCallSource {
    Direct,
    JsRepl,
    CodeMode {
        /// Runtime cell that issued the nested tool request.
        cell_id: String,
        /// Code-mode's per-cell tool invocation id. This is useful for
        /// debugging the JS/runtime bridge, but it is not the Codex tool call id
        /// because the runtime id only needs to be unique within one cell.
        runtime_tool_call_id: String,
    },
}

#[derive(Clone)]
pub struct ToolInvocation {
    pub session: Arc<Session>,
    pub turn: Arc<TurnContext>,
    pub tracker: SharedTurnDiffTracker,
    pub call_id: String,
    pub tool_name: ToolName,
    pub source: ToolCallSource,
    pub payload: ToolPayload,
}

#[derive(Clone, Debug)]
pub enum ToolPayload {
    Function {
        arguments: String,
    },
    ToolSearch {
        arguments: SearchToolCallParams,
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
            ToolPayload::ToolSearch { arguments } => Cow::Owned(arguments.query.clone()),
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

    fn post_tool_use_response(&self, _call_id: &str, _payload: &ToolPayload) -> Option<JsonValue> {
        None
    }

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

#[derive(Clone, Debug)]
pub struct McpToolOutput {
    pub result: CallToolResult,
    pub wall_time: Duration,
    pub original_image_detail_supported: bool,
}

impl ToolOutput for McpToolOutput {
    fn log_preview(&self) -> String {
        let payload = self.response_payload();
        let preview = payload.body.to_text().unwrap_or_else(|| {
            serde_json::to_string(&self.result.content)
                .unwrap_or_else(|err| format!("failed to serialize mcp result: {err}"))
        });
        telemetry_preview(&preview)
    }

    fn success_for_logging(&self) -> bool {
        self.result.success()
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: self.response_payload(),
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        serde_json::to_value(&self.result).unwrap_or_else(|err| {
            JsonValue::String(format!("failed to serialize mcp result: {err}"))
        })
    }
}

impl McpToolOutput {
    fn response_payload(&self) -> FunctionCallOutputPayload {
        let mut payload = self.result.as_function_call_output_payload();
        if let Some(items) = payload.content_items_mut() {
            sanitize_original_image_detail(self.original_image_detail_supported, items);
        }

        let wall_time_seconds = self.wall_time.as_secs_f64();
        let header = format!("Wall time: {wall_time_seconds:.4} seconds\nOutput:");

        match &mut payload.body {
            FunctionCallOutputBody::Text(text) => {
                if text.is_empty() {
                    *text = header;
                } else {
                    *text = format!("{header}\n{text}");
                }
            }
            FunctionCallOutputBody::ContentItems(items) => {
                items.insert(0, FunctionCallOutputContentItem::InputText { text: header });
            }
        }

        payload
    }
}

#[derive(Clone)]
pub struct ToolSearchOutput {
    pub tools: Vec<ToolSearchOutputTool>,
}

impl ToolOutput for ToolSearchOutput {
    fn log_preview(&self) -> String {
        let tools = self
            .tools
            .iter()
            .map(|tool| {
                serde_json::to_value(tool).unwrap_or_else(|err| {
                    JsonValue::String(format!("failed to serialize tool_search output: {err}"))
                })
            })
            .collect();
        telemetry_preview(&JsonValue::Array(tools).to_string())
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::ToolSearchOutput {
            call_id: call_id.to_string(),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: self
                .tools
                .iter()
                .map(|tool| {
                    serde_json::to_value(tool).unwrap_or_else(|err| {
                        JsonValue::String(format!("failed to serialize tool_search output: {err}"))
                    })
                })
                .collect(),
        }
    }
}

pub struct FunctionToolOutput {
    pub body: Vec<FunctionCallOutputContentItem>,
    pub success: Option<bool>,
    pub post_tool_use_response: Option<JsonValue>,
}

impl FunctionToolOutput {
    pub fn from_text(text: String, success: Option<bool>) -> Self {
        Self {
            body: vec![FunctionCallOutputContentItem::InputText { text }],
            success,
            post_tool_use_response: None,
        }
    }

    pub fn from_content(
        content: Vec<FunctionCallOutputContentItem>,
        success: Option<bool>,
    ) -> Self {
        Self {
            body: content,
            success,
            post_tool_use_response: None,
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

    fn post_tool_use_response(&self, _call_id: &str, _payload: &ToolPayload) -> Option<JsonValue> {
        self.post_tool_use_response.clone()
    }
}

pub struct ApplyPatchToolOutput {
    pub text: String,
}

impl ApplyPatchToolOutput {
    pub fn from_text(text: String) -> Self {
        Self { text }
    }
}

impl ToolOutput for ApplyPatchToolOutput {
    fn log_preview(&self) -> String {
        telemetry_preview(&self.text)
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        function_tool_response(
            call_id,
            payload,
            vec![FunctionCallOutputContentItem::InputText {
                text: self.text.clone(),
            }],
            Some(true),
        )
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        JsonValue::Object(serde_json::Map::new())
    }
}

pub struct AbortedToolOutput {
    pub message: String,
}

impl ToolOutput for AbortedToolOutput {
    fn log_preview(&self) -> String {
        telemetry_preview(&self.message)
    }

    fn success_for_logging(&self) -> bool {
        false
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        match payload {
            ToolPayload::ToolSearch { .. } => ResponseInputItem::ToolSearchOutput {
                call_id: call_id.to_string(),
                status: "completed".to_string(),
                execution: "client".to_string(),
                tools: Vec::new(),
            },
            ToolPayload::Mcp { .. } => ResponseInputItem::McpToolCallOutput {
                call_id: call_id.to_string(),
                output: CallToolResult::from_error_text(self.message.clone()),
            },
            _ => function_tool_response(
                call_id,
                payload,
                vec![FunctionCallOutputContentItem::InputText {
                    text: self.message.clone(),
                }],
                /*success*/ None,
            ),
        }
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
    pub process_id: Option<i32>,
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

    fn post_tool_use_response(&self, _call_id: &str, _payload: &ToolPayload) -> Option<JsonValue> {
        if self.process_id.is_some() || self.session_command.is_none() {
            return None;
        }

        Some(JsonValue::String(self.truncated_output()))
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
            session_id: Option<i32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            original_token_count: Option<usize>,
            output: String,
        }

        let result = UnifiedExecCodeModeResult {
            chunk_id: (!self.chunk_id.is_empty()).then(|| self.chunk_id.clone()),
            wall_time_seconds: self.wall_time.as_secs_f64(),
            exit_code: self.exit_code,
            session_id: self.process_id,
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

pub(crate) fn response_input_to_code_mode_result(response: ResponseInputItem) -> JsonValue {
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
        ResponseInputItem::ToolSearchOutput { tools, .. } => JsonValue::Array(tools),
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
            name: None,
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
#[path = "context_tests.rs"]
mod tests;
