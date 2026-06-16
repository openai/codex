use std::time::Duration;

use codex_code_mode::session_runtime as runtime;
use codex_code_mode_protocol::wire;

pub(super) fn create_cell_request(request: wire::CreateCellRequest) -> runtime::CreateCellRequest {
    runtime::CreateCellRequest {
        tool_call_id: request.tool_call_id,
        enabled_tools: request
            .enabled_tools
            .into_iter()
            .map(|definition| runtime::ToolDefinition {
                name: definition.name,
                tool_name: runtime::ToolName {
                    name: definition.tool_name.name,
                    namespace: definition.tool_name.namespace,
                },
                description: definition.description,
                kind: tool_kind(definition.kind),
            })
            .collect(),
        source: request.source,
    }
}

pub(super) fn observe_mode(mode: wire::ObserveMode) -> runtime::ObserveMode {
    match mode {
        wire::ObserveMode::YieldAfter { duration_ms } => {
            runtime::ObserveMode::YieldAfter(Duration::from_millis(duration_ms))
        }
        wire::ObserveMode::PendingFrontier => runtime::ObserveMode::PendingFrontier,
    }
}

pub(super) fn runtime_cell_id(cell_id: &wire::CellId) -> runtime::CellId {
    runtime::CellId::new(cell_id.as_str())
}

pub(super) fn wire_cell_id(cell_id: &runtime::CellId) -> wire::CellId {
    wire::CellId::new(cell_id.as_str())
}

pub(super) fn cell_event(event: runtime::CellEvent) -> wire::CellEvent {
    match event {
        runtime::CellEvent::Yielded { content_items } => wire::CellEvent::Yielded {
            content_items: content_items.into_iter().map(output_item).collect(),
        },
        runtime::CellEvent::Pending {
            content_items,
            pending_tool_call_ids,
        } => wire::CellEvent::Pending {
            content_items: content_items.into_iter().map(output_item).collect(),
            pending_tool_call_ids,
        },
        runtime::CellEvent::Completed {
            content_items,
            error_text,
        } => wire::CellEvent::Completed {
            content_items: content_items.into_iter().map(output_item).collect(),
            error_text,
        },
        runtime::CellEvent::Terminated { content_items } => wire::CellEvent::Terminated {
            content_items: content_items.into_iter().map(output_item).collect(),
        },
    }
}

pub(super) fn runtime_error(error: runtime::Error) -> wire::Error {
    match error {
        runtime::Error::ShuttingDown => wire::Error::ShuttingDown,
        runtime::Error::DuplicateCell(cell_id) => wire::Error::DuplicateCell {
            cell_id: wire_cell_id(&cell_id),
        },
        runtime::Error::MissingCell(cell_id) => wire::Error::MissingCell {
            cell_id: wire_cell_id(&cell_id),
        },
        runtime::Error::ClosedCell(cell_id) => wire::Error::ClosedCell {
            cell_id: wire_cell_id(&cell_id),
        },
        runtime::Error::BusyObserver(cell_id) => wire::Error::BusyObserver {
            cell_id: wire_cell_id(&cell_id),
        },
        runtime::Error::AlreadyTerminating(cell_id) => wire::Error::AlreadyTerminating {
            cell_id: wire_cell_id(&cell_id),
        },
        runtime::Error::Runtime(message) => wire::Error::Runtime { message },
    }
}

pub(super) fn nested_tool_call(invocation: runtime::NestedToolCall) -> wire::NestedToolCall {
    wire::NestedToolCall {
        cell_id: wire_cell_id(&invocation.cell_id),
        runtime_tool_call_id: invocation.runtime_tool_call_id,
        tool_name: wire::ToolName {
            name: invocation.tool_name.name,
            namespace: invocation.tool_name.namespace,
        },
        tool_kind: match invocation.tool_kind {
            runtime::ToolKind::Function => wire::ToolKind::Function,
            runtime::ToolKind::Freeform => wire::ToolKind::Freeform,
        },
        input: invocation.input,
    }
}

fn tool_kind(kind: wire::ToolKind) -> runtime::ToolKind {
    match kind {
        wire::ToolKind::Function => runtime::ToolKind::Function,
        wire::ToolKind::Freeform => runtime::ToolKind::Freeform,
    }
}

fn output_item(item: runtime::OutputItem) -> wire::OutputItem {
    match item {
        runtime::OutputItem::Text { text } => wire::OutputItem::Text { text },
        runtime::OutputItem::Image { image_url, detail } => wire::OutputItem::Image {
            image_url,
            detail: detail.map(|detail| match detail {
                runtime::ImageDetail::Auto => wire::ImageDetail::Auto,
                runtime::ImageDetail::Low => wire::ImageDetail::Low,
                runtime::ImageDetail::High => wire::ImageDetail::High,
                runtime::ImageDetail::Original => wire::ImageDetail::Original,
            }),
        },
    }
}
