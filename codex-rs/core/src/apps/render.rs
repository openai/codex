pub(crate) fn render_apps_section() -> &'static str {
    "## Apps\nApps are mentioned in the prompt in the format `[$app-name](apps://{connector_id})`.\nAn app is equivalent to a set of MCP tools.\nWhen you see an app mention, the app's MCP tools are either already provided, or do not exist because the user did not install it.\nDo not additionally call list_mcp_resources for apps that are already mentioned."
}
