use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo as McpToolInfo;
use serde_json::Map;
use serde_json::Value;

pub(crate) const CODEX_APPS_META_KEY: &str = "_codex_apps";

const CODEX_APPS_PROVIDER_BUILTIN: &str = "builtin";
const CODEX_APPS_META_PROVIDER_KEY: &str = "provider";
const CODEX_APPS_META_DIRECT_EXPOSE_KEY: &str = "direct_expose";

pub(crate) fn is_direct_exposed_codex_apps_builtin_tool_info(tool: &McpToolInfo) -> bool {
    if tool.server_name != CODEX_APPS_MCP_SERVER_NAME || tool.connector_id.is_some() {
        return false;
    }

    let Some(codex_apps_meta) = codex_apps_meta_from_tool_info(tool) else {
        return false;
    };

    codex_apps_meta
        .get(CODEX_APPS_META_PROVIDER_KEY)
        .and_then(Value::as_str)
        == Some(CODEX_APPS_PROVIDER_BUILTIN)
        && codex_apps_meta
            .get(CODEX_APPS_META_DIRECT_EXPOSE_KEY)
            .and_then(Value::as_bool)
            == Some(true)
}

fn codex_apps_meta_from_tool_info(tool: &McpToolInfo) -> Option<&Map<String, Value>> {
    tool.tool
        .meta
        .as_ref()
        .and_then(|meta| meta.get(CODEX_APPS_META_KEY))
        .and_then(Value::as_object)
}
