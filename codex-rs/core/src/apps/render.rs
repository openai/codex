use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;

pub(crate) fn render_apps_section() -> String {
    format!(
        "## Apps\nApps are mentioned in the prompt in the format `[$app-name](apps://{{connector_id}})`.\nAn app is equivalent to a set of MCP tools within the `{CODEX_APPS_MCP_SERVER_NAME}` MCP.\nWhen you see an app mention, the app's MCP tools are either already provided in `{CODEX_APPS_MCP_SERVER_NAME}`, or do not exist because the user did not install it.\nDo not additionally call list_mcp_resources for apps that are already mentioned."
    )
}
