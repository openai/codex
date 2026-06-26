use serde::Serialize;

pub const APPROVAL_KIND_KEY: &str = "codex_approval_kind";
pub const APPROVAL_KIND_MCP_TOOL_CALL: &str = "mcp_tool_call";
pub const APPROVAL_KIND_TOOL_SUGGESTION: &str = "tool_suggestion";
pub const REQUEST_TYPE_KEY: &str = "codex_request_type";
pub const REQUEST_TYPE_APPROVAL_REQUEST: &str = "approval_request";
pub const APPROVALS_REVIEWER_KEY: &str = "approvals_reviewer";
pub const PERSIST_KEY: &str = "persist";
pub const PERSIST_SESSION: &str = "session";
pub const PERSIST_ALWAYS: &str = "always";
pub const SOURCE_KEY: &str = "source";
pub const SOURCE_CONNECTOR: &str = "connector";
pub const CONNECTOR_ID_KEY: &str = "connector_id";
pub const CONNECTOR_NAME_KEY: &str = "connector_name";
pub const CONNECTOR_DESCRIPTION_KEY: &str = "connector_description";
pub const TOOL_NAME_KEY: &str = "tool_name";
pub const TOOL_TITLE_KEY: &str = "tool_title";
pub const TOOL_DESCRIPTION_KEY: &str = "tool_description";
pub const TOOL_PARAMS_KEY: &str = "tool_params";
pub const TOOL_PARAMS_DISPLAY_KEY: &str = "tool_params_display";

/// Stable identity supplied by a trusted runtime MCP registration owner.
///
/// The serialized field names preserve the Guardian approval contract that
/// predates runtime MCP registrations. Callers should use the generic
/// accessors instead of depending on that wire representation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct McpToolSource {
    #[serde(rename = "connector_id")]
    id: String,
    #[serde(rename = "connector_name")]
    name: String,
    #[serde(
        rename = "connector_description",
        skip_serializing_if = "Option::is_none"
    )]
    description: Option<String>,
}

impl McpToolSource {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: Option<String>,
    ) -> Option<Self> {
        let id = id.into();
        let name = name.into();
        let id = id.trim();
        let name = name.trim();
        if id.is_empty() || name.is_empty() {
            return None;
        }
        Some(Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description
                .map(|description| description.trim().to_string())
                .filter(|description| !description.is_empty()),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

#[cfg(test)]
#[path = "mcp_approval_meta_tests.rs"]
mod tests;
