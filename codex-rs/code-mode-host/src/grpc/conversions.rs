use codex_code_mode_protocol::CodeModeSessionCellExecutionLimits;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::ImageDetail;
use codex_code_mode_protocol::MissingCodeModeHostDuration;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::ToolDefinition;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::grpc as proto;
use codex_protocol::ToolName;
use serde_json::Value as JsonValue;
use tonic::Status;

use super::validation;

pub(super) fn session_limits(
    limits: Option<proto::SessionCellExecutionLimits>,
) -> Result<CodeModeSessionCellExecutionLimits, Status> {
    let limits = limits.unwrap_or_default();
    Ok(CodeModeSessionCellExecutionLimits {
        max_yield_time_ms: limits.max_yield_time_ms,
        max_heap_size_bytes: limits
            .max_heap_size_bytes
            .map(usize::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("maximum heap size exceeds this platform"))?,
    })
}

pub(super) fn execute_request(request: proto::ExecuteRequest) -> Result<ExecuteRequest, Status> {
    validation::identifier(&request.tool_call_id, "tool call ID")?;
    Ok(ExecuteRequest {
        tool_call_id: request.tool_call_id,
        source: request.source,
        enabled_tools: request
            .enabled_tools
            .into_iter()
            .map(tool_definition)
            .collect::<Result<Vec<_>, _>>()?,
        yield_time_ms: request.yield_time_ms,
        max_output_tokens: request
            .max_output_tokens
            .map(usize::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("maximum output tokens exceeds this platform"))?,
    })
}

fn tool_definition(definition: proto::ToolDefinition) -> Result<ToolDefinition, Status> {
    validation::identifier(&definition.name, "tool definition name")?;
    let name = definition
        .tool_name
        .ok_or_else(|| Status::invalid_argument("tool definition is missing its tool name"))?;
    validation::tool_name(&name)?;
    Ok(ToolDefinition {
        name: definition.name,
        tool_name: ToolName::new(name.namespace, name.name),
        description: definition.description,
        kind: match proto::ToolKind::try_from(definition.kind) {
            Ok(proto::ToolKind::Function) => CodeModeToolKind::Function,
            Ok(proto::ToolKind::Freeform) => CodeModeToolKind::Freeform,
            Ok(proto::ToolKind::Unspecified) | Err(_) => {
                return Err(Status::invalid_argument(
                    "tool definition has an invalid kind",
                ));
            }
        },
        input_schema: json_field(definition.input_schema_json, "input schema")?,
        input_schema_max_bytes: None,
        output_schema: json_field(definition.output_schema_json, "output schema")?,
    })
}

fn json_field(value: Option<Vec<u8>>, field: &str) -> Result<Option<JsonValue>, Status> {
    value
        .map(|value| {
            serde_json::from_slice(&value)
                .map_err(|error| Status::invalid_argument(format!("invalid tool {field}: {error}")))
        })
        .transpose()
}

/// Preserves the response's timing; the host handler must record it first.
pub(super) fn execution_outcome(
    response: RuntimeResponse,
) -> Result<proto::ExecutionOutcome, MissingCodeModeHostDuration> {
    let (cell_id, content_items, outcome, code_mode_host_duration) = match response {
        RuntimeResponse::Yielded {
            cell_id,
            content_items,
            code_mode_host_duration,
        } => (
            cell_id,
            content_items,
            proto::execution_outcome::Outcome::Yielded(proto::ExecutionYielded {}),
            code_mode_host_duration,
        ),
        RuntimeResponse::Terminated {
            cell_id,
            content_items,
            code_mode_host_duration,
        } => (
            cell_id,
            content_items,
            proto::execution_outcome::Outcome::Terminated(proto::ExecutionTerminated {}),
            code_mode_host_duration,
        ),
        RuntimeResponse::Result {
            cell_id,
            content_items,
            error_text,
            code_mode_host_duration,
        } => (
            cell_id,
            content_items,
            proto::execution_outcome::Outcome::Completed(proto::ExecutionCompleted { error_text }),
            code_mode_host_duration,
        ),
    };
    let code_mode_host_duration = code_mode_host_duration.ok_or(MissingCodeModeHostDuration)?;
    Ok(proto::ExecutionOutcome {
        cell_id: cell_id.to_string(),
        content_items: content_items.into_iter().map(content_item).collect(),
        outcome: Some(outcome),
        code_mode_host_duration_ns: u64::try_from(code_mode_host_duration.as_nanos())
            .unwrap_or(u64::MAX),
    })
}

pub(super) fn wait_response(
    outcome: WaitOutcome,
) -> Result<proto::WaitResponse, MissingCodeModeHostDuration> {
    let state = match outcome {
        WaitOutcome::LiveCell(response) => {
            proto::wait_response::State::LiveCell(execution_outcome(response)?)
        }
        WaitOutcome::MissingCell(response) => {
            proto::wait_response::State::MissingCell(execution_outcome(response)?)
        }
    };
    Ok(proto::WaitResponse { state: Some(state) })
}

fn content_item(item: FunctionCallOutputContentItem) -> proto::ContentItem {
    let item = match item {
        FunctionCallOutputContentItem::InputText { text } => {
            proto::content_item::Item::Text(proto::TextContent { text })
        }
        FunctionCallOutputContentItem::InputImage { image_url, detail } => {
            proto::content_item::Item::Image(proto::ImageContent {
                image_url,
                detail: detail.map(|detail| {
                    (match detail {
                        ImageDetail::Auto => proto::ImageDetail::Auto,
                        ImageDetail::Low => proto::ImageDetail::Low,
                        ImageDetail::High => proto::ImageDetail::High,
                        ImageDetail::Original => proto::ImageDetail::Original,
                    }) as i32
                }),
            })
        }
        FunctionCallOutputContentItem::InputAudio { audio_url } => {
            proto::content_item::Item::Audio(proto::AudioContent { audio_url })
        }
    };
    proto::ContentItem { item: Some(item) }
}

pub(super) fn tool_kind(kind: CodeModeToolKind) -> i32 {
    match kind {
        CodeModeToolKind::Function => proto::ToolKind::Function as i32,
        CodeModeToolKind::Freeform => proto::ToolKind::Freeform as i32,
    }
}

#[cfg(test)]
#[path = "conversions_tests.rs"]
mod tests;
