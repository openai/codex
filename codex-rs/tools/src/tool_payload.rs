use std::borrow::Cow;

use codex_protocol::models::SearchToolCallParams;
use serde::Deserialize;

pub const INVALID_TOOL_SEARCH_QUERY: &str = "[invalid tool_search arguments omitted]";

/// Canonical payload shapes accepted by model-visible tool runtimes.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolPayload {
    Function { arguments: String },
    ToolSearch { arguments: serde_json::Value },
    Custom { input: String },
}

impl ToolPayload {
    pub fn log_payload(&self) -> Cow<'_, str> {
        match self {
            ToolPayload::Function { arguments } => Cow::Borrowed(arguments),
            ToolPayload::ToolSearch { arguments } => SearchToolCallParams::deserialize(arguments)
                .map(|arguments| Cow::Owned(arguments.query))
                .unwrap_or(Cow::Borrowed(INVALID_TOOL_SEARCH_QUERY)),
            ToolPayload::Custom { input } => Cow::Borrowed(input),
        }
    }
}
