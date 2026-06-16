use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::CreateCellRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::ImageDetail;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::ToolDefinition;
use codex_code_mode_protocol::wire;

pub(super) fn create_cell_request(request: CreateCellRequest) -> wire::CreateCellRequest {
    wire::CreateCellRequest {
        tool_call_id: request.tool_call_id,
        enabled_tools: request
            .enabled_tools
            .into_iter()
            .map(tool_definition)
            .collect(),
        source: request.source,
    }
}

pub(super) fn wire_cell_id(cell_id: &CellId) -> wire::CellId {
    wire::CellId::new(cell_id.as_str())
}

pub(super) fn protocol_cell_id(cell_id: &wire::CellId) -> CellId {
    CellId::new(cell_id.as_str().to_string())
}

pub(super) fn runtime_response(
    cell_id: &CellId,
    event: wire::CellEvent,
) -> Result<RuntimeResponse, String> {
    match event {
        wire::CellEvent::Yielded { content_items } => Ok(RuntimeResponse::Yielded {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
        }),
        wire::CellEvent::Completed {
            content_items,
            error_text,
        } => Ok(RuntimeResponse::Result {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
            error_text,
        }),
        wire::CellEvent::Terminated { content_items } => Ok(RuntimeResponse::Terminated {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
        }),
        wire::CellEvent::Pending { .. } => {
            Err("code-mode host returned an unexpected pending frontier".to_string())
        }
    }
}

pub(super) fn missing_cell_response(cell_id: CellId) -> RuntimeResponse {
    RuntimeResponse::Result {
        error_text: Some(format!("exec cell {cell_id} not found")),
        cell_id,
        content_items: Vec::new(),
    }
}

pub(super) fn nested_tool_call(invocation: wire::NestedToolCall) -> CodeModeNestedToolCall {
    CodeModeNestedToolCall {
        cell_id: protocol_cell_id(&invocation.cell_id),
        runtime_tool_call_id: invocation.runtime_tool_call_id,
        tool_name: codex_protocol::ToolName {
            name: invocation.tool_name.name,
            namespace: invocation.tool_name.namespace,
        },
        tool_kind: match invocation.tool_kind {
            wire::ToolKind::Function => CodeModeToolKind::Function,
            wire::ToolKind::Freeform => CodeModeToolKind::Freeform,
        },
        input: invocation.input,
    }
}

pub(super) fn wire_error_message(error: &wire::Error) -> String {
    match error {
        wire::Error::MissingSession { session_id } => {
            format!("code-mode session {session_id} not found")
        }
        wire::Error::ShuttingDown => "code mode session is shutting down".to_string(),
        wire::Error::DuplicateCell { cell_id } => {
            format!("exec cell {} already exists", cell_id.as_str())
        }
        wire::Error::MissingCell { cell_id } => {
            format!("exec cell {} not found", cell_id.as_str())
        }
        wire::Error::ClosedCell { cell_id } => {
            format!("exec cell {} closed unexpectedly", cell_id.as_str())
        }
        wire::Error::BusyObserver { cell_id } => {
            format!(
                "exec cell {} already has an active observer",
                cell_id.as_str()
            )
        }
        wire::Error::AlreadyTerminating { cell_id } => {
            format!("exec cell {} is already terminating", cell_id.as_str())
        }
        wire::Error::Cancelled => "code-mode request was cancelled".to_string(),
        wire::Error::Runtime { message }
        | wire::Error::InvalidRequest { message }
        | wire::Error::CallbackFailed { message }
        | wire::Error::Internal { message } => message.clone(),
    }
}

fn tool_definition(definition: ToolDefinition) -> wire::ToolDefinition {
    wire::ToolDefinition {
        name: definition.name,
        tool_name: wire::ToolName {
            name: definition.tool_name.name,
            namespace: definition.tool_name.namespace,
        },
        description: definition.description,
        kind: match definition.kind {
            CodeModeToolKind::Function => wire::ToolKind::Function,
            CodeModeToolKind::Freeform => wire::ToolKind::Freeform,
        },
    }
}

fn output_item(item: wire::OutputItem) -> FunctionCallOutputContentItem {
    match item {
        wire::OutputItem::Text { text } => FunctionCallOutputContentItem::InputText { text },
        wire::OutputItem::Image { image_url, detail } => {
            FunctionCallOutputContentItem::InputImage {
                image_url,
                detail: detail.map(|detail| match detail {
                    wire::ImageDetail::Auto => ImageDetail::Auto,
                    wire::ImageDetail::Low => ImageDetail::Low,
                    wire::ImageDetail::High => ImageDetail::High,
                    wire::ImageDetail::Original => ImageDetail::Original,
                }),
            }
        }
    }
}
