use std::num::TryFromIntError;

use codex_protocol::ToolName;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::CellId;
use crate::CodeModeNestedToolCall;
use crate::CodeModeToolKind;
use crate::ExecuteRequest;
use crate::FunctionCallOutputContentItem;
use crate::ImageDetail;
use crate::RuntimeResponse;
use crate::ToolDefinition;
use crate::WaitOutcome;
use crate::WaitRequest;

/// A cell identifier with a wire representation owned by protocol V1.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WireCellId(String);

impl WireCellId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<CellId> for WireCellId {
    fn from(value: CellId) -> Self {
        Self(value.as_str().to_string())
    }
}

impl From<&CellId> for WireCellId {
    fn from(value: &CellId) -> Self {
        Self(value.as_str().to_string())
    }
}

impl From<WireCellId> for CellId {
    fn from(value: WireCellId) -> Self {
        Self::new(value.0)
    }
}

/// The V1 wire representation of a tool's stable name.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WireToolName {
    pub name: String,
    pub namespace: Option<String>,
}

impl From<ToolName> for WireToolName {
    fn from(value: ToolName) -> Self {
        Self {
            name: value.name,
            namespace: value.namespace,
        }
    }
}

impl From<WireToolName> for ToolName {
    fn from(value: WireToolName) -> Self {
        Self::new(value.namespace, value.name)
    }
}

/// The tool invocation shape supported by protocol V1.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireToolKind {
    Function,
    Freeform,
}

impl From<CodeModeToolKind> for WireToolKind {
    fn from(value: CodeModeToolKind) -> Self {
        match value {
            CodeModeToolKind::Function => Self::Function,
            CodeModeToolKind::Freeform => Self::Freeform,
        }
    }
}

impl From<WireToolKind> for CodeModeToolKind {
    fn from(value: WireToolKind) -> Self {
        match value {
            WireToolKind::Function => Self::Function,
            WireToolKind::Freeform => Self::Freeform,
        }
    }
}

/// A V1 tool definition embedded in an execute request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WireToolDefinition {
    pub name: String,
    pub tool_name: WireToolName,
    pub description: String,
    pub kind: WireToolKind,
    pub input_schema: Option<JsonValue>,
    pub output_schema: Option<JsonValue>,
}

impl From<ToolDefinition> for WireToolDefinition {
    fn from(value: ToolDefinition) -> Self {
        Self {
            name: value.name,
            tool_name: value.tool_name.into(),
            description: value.description,
            kind: value.kind.into(),
            input_schema: value.input_schema,
            output_schema: value.output_schema,
        }
    }
}

impl From<WireToolDefinition> for ToolDefinition {
    fn from(value: WireToolDefinition) -> Self {
        Self {
            name: value.name,
            tool_name: value.tool_name.into(),
            description: value.description,
            kind: value.kind.into(),
            input_schema: value.input_schema,
            output_schema: value.output_schema,
        }
    }
}

/// The complete execute request shape supported by protocol V1.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WireExecuteRequest {
    pub tool_call_id: String,
    pub enabled_tools: Vec<WireToolDefinition>,
    pub source: String,
    pub yield_time_ms: Option<u64>,
    pub max_output_tokens: Option<i32>,
}

impl TryFrom<ExecuteRequest> for WireExecuteRequest {
    type Error = TryFromIntError;

    fn try_from(value: ExecuteRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            tool_call_id: value.tool_call_id,
            enabled_tools: value.enabled_tools.into_iter().map(Into::into).collect(),
            source: value.source,
            yield_time_ms: value.yield_time_ms,
            max_output_tokens: value.max_output_tokens.map(i32::try_from).transpose()?,
        })
    }
}

impl TryFrom<WireExecuteRequest> for ExecuteRequest {
    type Error = TryFromIntError;

    fn try_from(value: WireExecuteRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            tool_call_id: value.tool_call_id,
            enabled_tools: value.enabled_tools.into_iter().map(Into::into).collect(),
            source: value.source,
            yield_time_ms: value.yield_time_ms,
            max_output_tokens: value.max_output_tokens.map(usize::try_from).transpose()?,
        })
    }
}

/// The complete wait request shape supported by protocol V1.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WireWaitRequest {
    pub cell_id: WireCellId,
    pub yield_time_ms: u64,
}

impl From<WaitRequest> for WireWaitRequest {
    fn from(value: WaitRequest) -> Self {
        Self {
            cell_id: value.cell_id.into(),
            yield_time_ms: value.yield_time_ms,
        }
    }
}

impl From<WireWaitRequest> for WaitRequest {
    fn from(value: WireWaitRequest) -> Self {
        Self {
            cell_id: value.cell_id.into(),
            yield_time_ms: value.yield_time_ms,
        }
    }
}

/// Image detail values accepted in a V1 runtime response.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WireImageDetail {
    Auto,
    Low,
    High,
    Original,
}

impl From<ImageDetail> for WireImageDetail {
    fn from(value: ImageDetail) -> Self {
        match value {
            ImageDetail::Auto => Self::Auto,
            ImageDetail::Low => Self::Low,
            ImageDetail::High => Self::High,
            ImageDetail::Original => Self::Original,
        }
    }
}

impl From<WireImageDetail> for ImageDetail {
    fn from(value: WireImageDetail) -> Self {
        match value {
            WireImageDetail::Auto => Self::Auto,
            WireImageDetail::Low => Self::Low,
            WireImageDetail::High => Self::High,
            WireImageDetail::Original => Self::Original,
        }
    }
}

/// One output item emitted by a V1 runtime response.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]
pub enum WireContentItem {
    InputText {
        text: String,
    },
    InputImage {
        image_url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<WireImageDetail>,
    },
    InputAudio {
        audio_url: String,
    },
}

impl From<FunctionCallOutputContentItem> for WireContentItem {
    fn from(value: FunctionCallOutputContentItem) -> Self {
        match value {
            FunctionCallOutputContentItem::InputText { text } => Self::InputText { text },
            FunctionCallOutputContentItem::InputImage { image_url, detail } => Self::InputImage {
                image_url,
                detail: detail.map(Into::into),
            },
            FunctionCallOutputContentItem::InputAudio { audio_url } => {
                Self::InputAudio { audio_url }
            }
        }
    }
}

impl From<WireContentItem> for FunctionCallOutputContentItem {
    fn from(value: WireContentItem) -> Self {
        match value {
            WireContentItem::InputText { text } => Self::InputText { text },
            WireContentItem::InputImage { image_url, detail } => Self::InputImage {
                image_url,
                detail: detail.map(Into::into),
            },
            WireContentItem::InputAudio { audio_url } => Self::InputAudio { audio_url },
        }
    }
}

/// Runtime output returned over the V1 host connection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum WireRuntimeResponse {
    Yielded {
        cell_id: WireCellId,
        content_items: Vec<WireContentItem>,
    },
    Terminated {
        cell_id: WireCellId,
        content_items: Vec<WireContentItem>,
    },
    Result {
        cell_id: WireCellId,
        content_items: Vec<WireContentItem>,
        error_text: Option<String>,
    },
}

impl From<RuntimeResponse> for WireRuntimeResponse {
    fn from(value: RuntimeResponse) -> Self {
        match value {
            RuntimeResponse::Yielded {
                cell_id,
                content_items,
            } => Self::Yielded {
                cell_id: cell_id.into(),
                content_items: content_items.into_iter().map(Into::into).collect(),
            },
            RuntimeResponse::Terminated {
                cell_id,
                content_items,
            } => Self::Terminated {
                cell_id: cell_id.into(),
                content_items: content_items.into_iter().map(Into::into).collect(),
            },
            RuntimeResponse::Result {
                cell_id,
                content_items,
                error_text,
            } => Self::Result {
                cell_id: cell_id.into(),
                content_items: content_items.into_iter().map(Into::into).collect(),
                error_text,
            },
        }
    }
}

impl From<WireRuntimeResponse> for RuntimeResponse {
    fn from(value: WireRuntimeResponse) -> Self {
        match value {
            WireRuntimeResponse::Yielded {
                cell_id,
                content_items,
            } => Self::Yielded {
                cell_id: cell_id.into(),
                content_items: content_items.into_iter().map(Into::into).collect(),
            },
            WireRuntimeResponse::Terminated {
                cell_id,
                content_items,
            } => Self::Terminated {
                cell_id: cell_id.into(),
                content_items: content_items.into_iter().map(Into::into).collect(),
            },
            WireRuntimeResponse::Result {
                cell_id,
                content_items,
                error_text,
            } => Self::Result {
                cell_id: cell_id.into(),
                content_items: content_items.into_iter().map(Into::into).collect(),
                error_text,
            },
        }
    }
}

/// Whether a waited-for cell remained live in protocol V1.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum WireWaitOutcome {
    LiveCell(WireRuntimeResponse),
    MissingCell(WireRuntimeResponse),
}

impl From<WaitOutcome> for WireWaitOutcome {
    fn from(value: WaitOutcome) -> Self {
        match value {
            WaitOutcome::LiveCell(response) => Self::LiveCell(response.into()),
            WaitOutcome::MissingCell(response) => Self::MissingCell(response.into()),
        }
    }
}

impl From<WireWaitOutcome> for WaitOutcome {
    fn from(value: WireWaitOutcome) -> Self {
        match value {
            WireWaitOutcome::LiveCell(response) => Self::LiveCell(response.into()),
            WireWaitOutcome::MissingCell(response) => Self::MissingCell(response.into()),
        }
    }
}

/// A nested tool invocation sent over the V1 host connection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WireNestedToolCall {
    pub cell_id: WireCellId,
    pub runtime_tool_call_id: String,
    pub tool_name: WireToolName,
    pub tool_kind: WireToolKind,
    pub input: Option<JsonValue>,
}

impl From<CodeModeNestedToolCall> for WireNestedToolCall {
    fn from(value: CodeModeNestedToolCall) -> Self {
        Self {
            cell_id: value.cell_id.into(),
            runtime_tool_call_id: value.runtime_tool_call_id,
            tool_name: value.tool_name.into(),
            tool_kind: value.tool_kind.into(),
            input: value.input,
        }
    }
}

impl From<WireNestedToolCall> for CodeModeNestedToolCall {
    fn from(value: WireNestedToolCall) -> Self {
        Self {
            cell_id: value.cell_id.into(),
            runtime_tool_call_id: value.runtime_tool_call_id,
            tool_name: value.tool_name.into(),
            tool_kind: value.tool_kind.into(),
            input: value.input,
        }
    }
}
