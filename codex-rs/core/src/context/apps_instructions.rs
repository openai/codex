use codex_app_server_protocol::AppInfo;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_protocol::protocol::APPS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::APPS_INSTRUCTIONS_OPEN_TAG;

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AppsInstructions;

impl AppsInstructions {
    pub(crate) fn from_connectors(connectors: &[AppInfo]) -> Option<Self> {
        connectors
            .iter()
            .any(|connector| connector.is_accessible && connector.is_enabled)
            .then_some(Self)
    }
}

impl ContextualUserFragment for AppsInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = APPS_INSTRUCTIONS_OPEN_TAG;
    const END_MARKER: &'static str = APPS_INSTRUCTIONS_CLOSE_TAG;

    fn body(&self) -> String {
        format!(
            "\n## Apps (Connectors)\nApps (Connectors) can be explicitly triggered in user messages in the format `[$app-name](app://{{connector_id}})`. Apps can also be implicitly triggered as long as the context suggests usage of available apps.\nAn app is equivalent to a set of MCP tools within the `{CODEX_APPS_MCP_SERVER_NAME}` MCP.\nAn installed app's MCP tools are either provided to you already, or can be lazy-loaded through the `tool_search` tool. If `tool_search` is available, the apps that are searchable by `tools_search` will be listed by it.\nDo not additionally call list_mcp_resources or list_mcp_resource_templates for apps.\n"
        )
    }
}
