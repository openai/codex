use codex_code_mode_protocol::CodeModeSessionCellExecutionLimits;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::ImageDetail;
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
    if request.enabled_tools.len() > validation::MAX_TOOL_DEFINITIONS {
        return Err(Status::invalid_argument(format!(
            "code-mode execution exceeds {} enabled tools",
            validation::MAX_TOOL_DEFINITIONS
        )));
    }
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
    validation::bounded(
        &definition.description,
        validation::MAX_TOOL_DESCRIPTION_BYTES,
        "tool description",
    )?;
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

pub(super) fn execution_outcome(response: RuntimeResponse) -> proto::ExecutionOutcome {
    let (cell_id, content_items, outcome) = match response {
        RuntimeResponse::Yielded {
            cell_id,
            content_items,
        } => (
            cell_id,
            content_items,
            proto::execution_outcome::Outcome::Yielded(proto::ExecutionYielded {}),
        ),
        RuntimeResponse::Terminated {
            cell_id,
            content_items,
        } => (
            cell_id,
            content_items,
            proto::execution_outcome::Outcome::Terminated(proto::ExecutionTerminated {}),
        ),
        RuntimeResponse::Result {
            cell_id,
            content_items,
            error_text,
        } => (
            cell_id,
            content_items,
            proto::execution_outcome::Outcome::Completed(proto::ExecutionCompleted { error_text }),
        ),
    };
    proto::ExecutionOutcome {
        cell_id: cell_id.to_string(),
        content_items: content_items.into_iter().map(content_item).collect(),
        outcome: Some(outcome),
    }
}

pub(super) fn wait_response(outcome: WaitOutcome) -> proto::WaitResponse {
    let state = match outcome {
        WaitOutcome::LiveCell(response) => {
            proto::wait_response::State::LiveCell(execution_outcome(response))
        }
        WaitOutcome::MissingCell(response) => {
            proto::wait_response::State::MissingCell(execution_outcome(response))
        }
    };
    proto::WaitResponse { state: Some(state) }
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
