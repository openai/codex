//! MCP-facing aliases for the connector-owned runtime.

use codex_login::CodexAuth;
use codex_protocol::mcp::McpServerInfo;

use crate::tools::ToolInfo;

impl codex_connectors::ConnectorRuntimePayload for ToolInfo {
    const TOOLS_CACHE_DIR: &'static str = "cache/codex_apps_tools";
    const TOOLS_CACHE_SCHEMA_VERSION: u8 = 4;
    const SERVER_INFO_CACHE_DIR: &'static str = "cache/codex_apps_server_info";
    const SERVER_INFO_CACHE_SCHEMA_VERSION: u8 = 1;
}

pub type ConnectorRuntimeContextKey = codex_connectors::ConnectorRuntimeContextKey;
pub type ConnectorRuntimeManager = codex_connectors::ConnectorRuntimeManager<ToolInfo>;
pub type ConnectorRuntimeSnapshot = codex_connectors::ConnectorRuntimeSnapshot<ToolInfo>;

/// Compatibility alias for existing cache call sites.
pub type CodexAppsToolsCacheKey = ConnectorRuntimeContextKey;

/// Compatibility alias for existing cache call sites.
pub type CodexAppsToolsCache = ConnectorRuntimeManager;

pub(crate) type CodexAppsToolsCacheContext = codex_connectors::ConnectorRuntimeContext<ToolInfo>;
pub(crate) use codex_connectors::ConnectorRuntimeFetchSource as CodexAppsToolsFetchSource;

/// Builds the CodexAuth-backed connector runtime context key.
pub fn connector_runtime_context_key(auth: Option<&CodexAuth>) -> ConnectorRuntimeContextKey {
    let account_id = auth.and_then(CodexAuth::get_account_id);
    let chatgpt_user_id = auth.and_then(CodexAuth::get_chatgpt_user_id);
    if auth.is_some_and(CodexAuth::is_workspace_account) {
        ConnectorRuntimeContextKey::workspace(account_id, chatgpt_user_id)
    } else {
        ConnectorRuntimeContextKey::personal(account_id, chatgpt_user_id)
    }
}

/// Compatibility helper for existing cache call sites.
pub fn codex_apps_tools_cache_key(auth: Option<&CodexAuth>) -> CodexAppsToolsCacheKey {
    connector_runtime_context_key(auth)
}

pub(crate) fn load_startup_cached_codex_apps_server_info(
    cache_context: &CodexAppsToolsCacheContext,
) -> Option<McpServerInfo> {
    cache_context.cached_server_info()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
