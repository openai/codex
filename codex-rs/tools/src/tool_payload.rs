use std::borrow::Cow;

use codex_protocol::models::SearchToolCallParams;
use codex_protocol::models::ShellToolCallParams;

/// Canonical payload shapes accepted by model-visible tool runtimes.
#[derive(Clone, Debug)]
pub enum ToolPayload {
    Function { arguments: String },
    ToolSearch { arguments: SearchToolCallParams },
    Custom { input: String },
    LocalShell { params: ShellToolCallParams },
}

impl ToolPayload {
    pub fn log_payload(&self) -> Cow<'_, str> {
        match self {
            ToolPayload::Function { arguments } => Cow::Borrowed(arguments),
            ToolPayload::ToolSearch { arguments } => Cow::Owned(arguments.query.clone()),
            ToolPayload::Custom { input } => Cow::Borrowed(input),
            ToolPayload::LocalShell { params } => Cow::Owned(params.command.join(" ")),
        }
    }
}
