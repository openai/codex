use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;

pub(crate) fn render_apps_section() -> String {
    format!(
        "## Apps\nApps are mentioned in user messages in the format `[$app-name](app://{{connector_id}})`.\nAn app is equivalent to a set of MCP tools within the `{CODEX_APPS_MCP_SERVER_NAME}` MCP.\nWhen you see an app mention, the app's MCP tools are either available tools in the `{CODEX_APPS_MCP_SERVER_NAME}` MCP server, or the tools do not exist because the user has not installed the app.\nDo not additionally call list_mcp_resources for apps that are already mentioned."
    )
}
