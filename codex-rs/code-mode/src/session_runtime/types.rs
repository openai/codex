use std::time::Duration;

/// Selects the next observable frontier for a running cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObserveMode {
    YieldAfter(Duration),
    PendingFrontier,
}

/// An observable cell lifecycle event.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CellEvent {
    Yielded {
        content_items: Vec<OutputItem>,
    },
    Pending {
        content_items: Vec<OutputItem>,
        pending_tool_call_ids: Vec<String>,
    },
    Completed {
        content_items: Vec<OutputItem>,
        error_text: Option<String>,
    },
    Terminated {
        content_items: Vec<OutputItem>,
    },
}

/// Output emitted by a cell since its preceding observation.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OutputItem {
    Text {
        text: String,
    },
    Image {
        image_url: String,
        detail: Option<ImageDetail>,
    },
}

/// Requested image fidelity for an output image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImageDetail {
    Auto,
    Low,
    High,
    Original,
}

/// Transport-neutral input for creating a cell.
///
/// The owning session assigns the cell ID when it admits the request.
pub(crate) struct CreateCellRequest {
    pub(crate) tool_call_id: String,
    pub(crate) enabled_tools: Vec<ToolDefinition>,
    pub(crate) source: String,
}

/// Tool metadata exposed to code running inside a cell.
pub(crate) struct ToolDefinition {
    pub(crate) name: String,
    pub(crate) tool_name: ToolName,
    pub(crate) description: String,
    pub(crate) kind: ToolKind,
}

/// A tool name with an optional namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ToolName {
    pub(crate) name: String,
    pub(crate) namespace: Option<String>,
}

/// The JavaScript calling convention for a tool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ToolKind {
    Function,
    Freeform,
}
