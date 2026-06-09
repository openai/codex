use crate::connectors;
use crate::mcp_tool_exposure::build_mcp_tool_exposure;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::handlers::McpHandler;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::LateToolRegistry;
use crate::tools::registry::ToolExecutor;
use codex_features::Feature;
use codex_tools::ToolSearchInfo;
use std::sync::Arc;
use tracing::warn;

pub(crate) fn enabled(turn_context: &TurnContext) -> bool {
    turn_context
        .config
        .features
        .enabled(Feature::ToolSearchAlwaysDeferMcpTools)
        && turn_context.model_info.supports_search_tool
        && turn_context.provider.capabilities().namespace_tools
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "MCP tool listing borrows the read guard across the asynchronous inventory read"
)]
pub(crate) async fn register_eligible_tools(
    session: &Arc<Session>,
    turn_context: &TurnContext,
    late_tools: &LateToolRegistry,
) -> Vec<ToolSearchInfo> {
    let registry_generation = late_tools.generation();
    let all_mcp_tools = session
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await;
    let connectors = if turn_context.apps_enabled() {
        let loaded_plugins = session
            .services
            .plugins_manager
            .plugins_for_config(&turn_context.config.plugins_config_input())
            .await;
        let connectors = codex_connectors::merge::merge_plugin_connectors_with_accessible(
            loaded_plugins
                .effective_apps()
                .into_iter()
                .map(|connector_id| connector_id.0),
            connectors::accessible_connectors_from_mcp_tools(&all_mcp_tools),
        );
        Some(connectors::with_app_enabled_state(
            connectors,
            &turn_context.config,
        ))
    } else {
        None
    };

    let exposure = build_mcp_tool_exposure(
        &all_mcp_tools,
        connectors.as_deref(),
        &turn_context.config,
        /*search_tool_enabled*/ true,
    );
    let mut search_infos = Vec::new();
    let mut handlers: Vec<Arc<dyn CoreToolRuntime>> = Vec::new();
    for tool_info in exposure.deferred_tools.unwrap_or(exposure.direct_tools) {
        match McpHandler::new(tool_info) {
            Ok(handler) => {
                if let Some(search_info) = handler.search_info() {
                    search_infos.push(search_info);
                }
                handlers.push(Arc::new(handler));
            }
            Err(err) => warn!("Skipping deferred MCP tool: failed to build tool spec: {err}"),
        }
    }
    if late_tools.replace_if_generation(registry_generation, handlers) {
        search_infos
    } else {
        warn!("Skipping deferred MCP tools because the MCP inventory changed during tool_search");
        Vec::new()
    }
}
