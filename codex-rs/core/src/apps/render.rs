use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_protocol::protocol::APPS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::APPS_INSTRUCTIONS_OPEN_TAG;

pub(crate) fn render_apps_section() -> String {
    let body = format!(
        "## Apps (Connectors)\nApps (Connectors) can be explicitly triggered in user messages in the format `[$app-name](app://{{connector_id}})`. Apps can also be implicitly triggered as long as the context suggests usage of available apps, the available apps will be listed by the `tool_search` tool.\nAn app is equivalent to a set of MCP tools within the `{CODEX_APPS_MCP_SERVER_NAME}` MCP.\nAn installed app's MCP tools are either provided to you already, or can be lazy-loaded through the `tool_search` tool.\nDo not additionally call list_mcp_resources or list_mcp_resource_templates for apps."
    );
    format!("{APPS_INSTRUCTIONS_OPEN_TAG}\n{body}\n{APPS_INSTRUCTIONS_CLOSE_TAG}")
}
