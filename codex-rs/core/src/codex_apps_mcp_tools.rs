use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo as McpToolInfo;
use serde_json::Map;
use serde_json::Value;

pub(crate) const CODEX_APPS_META_KEY: &str = "_codex_apps";

const CODEX_APPS_PROVIDER_BUILTIN: &str = "builtin";
const CODEX_APPS_META_PROVIDER_KEY: &str = "provider";
const CODEX_APPS_META_DIRECT_EXPOSE_KEY: &str = "direct_expose";
const CODEX_APPS_META_MATERIALIZE_FILE_DOWNLOAD_KEY: &str = "materialize_file_download";

pub(crate) fn is_direct_exposed_codex_apps_builtin_tool_info(tool: &McpToolInfo) -> bool {
    is_direct_exposed_codex_apps_builtin(
        &tool.server_name,
        tool.connector_id.as_deref(),
        codex_apps_meta_from_tool_info(tool),
    )
}

pub(crate) fn is_direct_exposed_codex_apps_builtin(
    server_name: &str,
    connector_id: Option<&str>,
    codex_apps_meta: Option<&Map<String, Value>>,
) -> bool {
    if server_name != CODEX_APPS_MCP_SERVER_NAME || connector_id.is_some() {
        return false;
    }

    let Some(codex_apps_meta) = codex_apps_meta else {
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

pub(crate) fn should_materialize_codex_apps_file_download(
    server_name: &str,
    codex_apps_meta: Option<&Map<String, Value>>,
) -> bool {
    if server_name != CODEX_APPS_MCP_SERVER_NAME {
        return false;
    }

    let Some(codex_apps_meta) = codex_apps_meta else {
        return false;
    };

    codex_apps_meta
        .get(CODEX_APPS_META_PROVIDER_KEY)
        .and_then(Value::as_str)
        == Some(CODEX_APPS_PROVIDER_BUILTIN)
        && codex_apps_meta
            .get(CODEX_APPS_META_MATERIALIZE_FILE_DOWNLOAD_KEY)
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
