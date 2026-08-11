use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::ImageDetail;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::ToolDefinition;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::grpc;
use codex_protocol::ToolName;

pub(super) fn execute_request(
    session_id: &str,
    execution_id: String,
    request: ExecuteRequest,
) -> Result<grpc::ExecuteRequest, String> {
    Ok(grpc::ExecuteRequest {
        session_id: session_id.to_string(),
        execution_id,
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
            .map(u64::try_from)
            .transpose()
            .map_err(|error| format!("invalid code-mode output token limit: {error}"))?,
    })
}

fn tool_definition(definition: ToolDefinition) -> Result<grpc::ToolDefinition, String> {
    Ok(grpc::ToolDefinition {
        name: definition.name,
        tool_name: Some(grpc::ToolName {
            name: definition.tool_name.name,
            namespace: definition.tool_name.namespace,
        }),
        description: definition.description,
        kind: match definition.kind {
            CodeModeToolKind::Function => grpc::ToolKind::Function as i32,
            CodeModeToolKind::Freeform => grpc::ToolKind::Freeform as i32,
        },
        input_schema_json: definition
            .input_schema
            .map(|schema| serde_json::to_vec(&schema))
            .transpose()
            .map_err(|error| format!("failed to encode code-mode tool input schema: {error}"))?,
        output_schema_json: definition
            .output_schema
            .map(|schema| serde_json::to_vec(&schema))
            .transpose()
            .map_err(|error| format!("failed to encode code-mode tool output schema: {error}"))?,
    })
}

pub(super) fn tool_call(call: grpc::ToolCall) -> Result<CodeModeNestedToolCall, String> {
    let name = call
        .tool_name
        .ok_or_else(|| "code-mode tool invocation omitted its tool name".to_string())?;
    let kind = match grpc::ToolKind::try_from(call.tool_kind) {
        Ok(grpc::ToolKind::Function) => CodeModeToolKind::Function,
        Ok(grpc::ToolKind::Freeform) => CodeModeToolKind::Freeform,
        Ok(grpc::ToolKind::Unspecified) | Err(_) => {
            return Err(format!(
                "code-mode tool invocation has invalid kind {}",
                call.tool_kind
            ));
        }
    };
    let input = call
        .input_json
        .map(|input| serde_json::from_slice(&input))
        .transpose()
        .map_err(|error| format!("code-mode tool invocation contains invalid JSON: {error}"))?;

    Ok(CodeModeNestedToolCall {
        cell_id: CellId::new(call.cell_id),
        runtime_tool_call_id: call.runtime_tool_call_id,
        tool_name: ToolName::new(name.namespace, name.name),
        tool_kind: kind,
        input,
    })
}

pub(super) fn runtime_response(outcome: grpc::ExecutionOutcome) -> Result<RuntimeResponse, String> {
    super::validate_identifier(&outcome.cell_id, "cell ID")?;
    let cell_id = CellId::new(outcome.cell_id);
    let content_items = outcome
        .content_items
        .into_iter()
        .map(content_item)
        .collect::<Result<Vec<_>, _>>()?;
    match outcome
        .outcome
        .ok_or_else(|| "code-mode execution omitted its outcome".to_string())?
    {
        grpc::execution_outcome::Outcome::Yielded(_) => Ok(RuntimeResponse::Yielded {
            cell_id,
            content_items,
        }),
        grpc::execution_outcome::Outcome::Terminated(_) => Ok(RuntimeResponse::Terminated {
            cell_id,
            content_items,
        }),
        grpc::execution_outcome::Outcome::Completed(completed) => Ok(RuntimeResponse::Result {
            cell_id,
            content_items,
            error_text: completed.error_text,
        }),
    }
}

pub(super) fn wait_outcome(response: grpc::WaitResponse) -> Result<WaitOutcome, String> {
    match response
        .state
        .ok_or_else(|| "code-mode wait omitted its outcome".to_string())?
    {
        grpc::wait_response::State::LiveCell(response) => {
            runtime_response(response).map(WaitOutcome::LiveCell)
        }
        grpc::wait_response::State::MissingCell(response) => {
            runtime_response(response).map(WaitOutcome::MissingCell)
        }
    }
}

fn content_item(item: grpc::ContentItem) -> Result<FunctionCallOutputContentItem, String> {
    match item
        .item
        .ok_or_else(|| "code-mode execution returned an empty content item".to_string())?
    {
        grpc::content_item::Item::Text(text) => {
            Ok(FunctionCallOutputContentItem::InputText { text: text.text })
        }
        grpc::content_item::Item::Image(image) => Ok(FunctionCallOutputContentItem::InputImage {
            image_url: image.image_url,
            detail: image.detail.map(image_detail).transpose()?,
        }),
        grpc::content_item::Item::Audio(audio) => Ok(FunctionCallOutputContentItem::InputAudio {
            audio_url: audio.audio_url,
        }),
    }
}

fn image_detail(value: i32) -> Result<ImageDetail, String> {
    match grpc::ImageDetail::try_from(value) {
        Ok(grpc::ImageDetail::Auto) => Ok(ImageDetail::Auto),
        Ok(grpc::ImageDetail::Low) => Ok(ImageDetail::Low),
        Ok(grpc::ImageDetail::High) => Ok(ImageDetail::High),
        Ok(grpc::ImageDetail::Original) => Ok(ImageDetail::Original),
        Ok(grpc::ImageDetail::Unspecified) | Err(_) => {
            Err(format!("code-mode image has invalid detail value {value}"))
        }
    }
}

#[cfg(test)]
#[path = "conversion_tests.rs"]
mod tests;
