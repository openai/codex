//! Sparse Memento V2 tool definitions. Runtime data remains with consumers.

use super::ToolMessage;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Model-owned history namespace and public tool definitions.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct HistoryToolMessages {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_windows: Option<ToolMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_items: Option<ToolMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_item: Option<ToolMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_contents: Option<ToolMessage>,
}

/// Model-owned notes namespace and public tool definitions.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct NotesToolMessages {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_files_by_prefix: Option<ToolMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_file: Option<ToolMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_contents: Option<ToolMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub append_to_file: Option<ToolMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_file: Option<ToolMessage>,
}

/// Model-owned context-window control tool definitions.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct ContextManagementToolMessages {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_context: Option<ToolMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub get_context_remaining: Option<ToolMessage>,
}

#[cfg(test)]
#[path = "memento_v2_tests.rs"]
mod tests;
