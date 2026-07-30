use std::collections::HashSet;
use std::sync::Arc;

use codex_connectors::AppToolPolicyEvaluator;
use codex_connectors::AppToolPolicyInput;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo as McpToolInfo;
use codex_mcp::tool_is_model_visible;
use codex_tools::ToolExposure;
use tracing::instrument;
use tracing::warn;

use crate::config::Config;
use crate::connectors;
use crate::tools::handlers::McpHandler;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::override_tool_exposure;

#[instrument(level = "trace", skip_all)]
pub(crate) fn build_mcp_tool_runtimes<'a>(
    all_mcp_tools: &'a [McpToolInfo],
    connectors: Option<&'a [connectors::AppInfo]>,
    config: &'a Config,
    search_tool_enabled: bool,
) -> impl Iterator<Item = Arc<dyn CoreToolRuntime>> + 'a {
    // Keep regular MCP tools first; Apps tools also require connector and policy checks.
    let non_app_tools = filter_non_codex_apps_mcp_tools_only(all_mcp_tools);
    let app_tools = connectors
        .into_iter()
        .flat_map(move |connectors| filter_codex_apps_mcp_tools(all_mcp_tools, connectors, config));
    let exposure = if search_tool_enabled {
        ToolExposure::Deferred
    } else {
        ToolExposure::Direct
    };
    non_app_tools.chain(app_tools).filter_map(move |tool| {
        let tool_name = tool.canonical_tool_name();
        match McpHandler::new(tool.clone()) {
            Ok(handler) => {
                let handler: Arc<dyn CoreToolRuntime> = Arc::new(handler);
                Some(override_tool_exposure(handler, exposure))
            }
            Err(err) => {
                warn!("Skipping MCP tool `{tool_name}`: failed to build tool spec: {err}");
                None
            }
        }
    })
}

fn filter_non_codex_apps_mcp_tools_only(
    mcp_tools: &[McpToolInfo],
) -> impl Iterator<Item = &McpToolInfo> + '_ {
    mcp_tools.iter().filter(|tool| {
        tool.server_name != CODEX_APPS_MCP_SERVER_NAME && tool_is_model_visible(tool)
    })
}

fn filter_codex_apps_mcp_tools<'a>(
    mcp_tools: &'a [McpToolInfo],
    connectors: &'a [connectors::AppInfo],
    config: &'a Config,
) -> impl Iterator<Item = &'a McpToolInfo> + 'a {
    let allowed: HashSet<&str> = connectors
        .iter()
        .map(|connector| connector.id.as_str())
        .collect();
    let app_tool_policy = AppToolPolicyEvaluator::new(&config.config_layer_stack);

    mcp_tools.iter().filter(move |tool| {
        if tool.server_name != CODEX_APPS_MCP_SERVER_NAME {
            return false;
        }
        if !tool_is_model_visible(tool) {
            return false;
        }
        let Some(connector_id) = tool.connector_id.as_deref() else {
            return false;
        };
        let annotations = tool.tool.annotations.as_ref();
        allowed.contains(connector_id)
            && app_tool_policy
                .policy(AppToolPolicyInput {
                    connector_id: Some(connector_id),
                    tool_name: &tool.tool.name,
                    tool_title: tool.tool.title.as_deref(),
                    destructive_hint: annotations
                        .and_then(|annotations| annotations.destructive_hint),
                    open_world_hint: annotations
                        .and_then(|annotations| annotations.open_world_hint),
                })
                .enabled
    })
}

#[cfg(test)]
#[path = "mcp_tool_exposure_test.rs"]
mod tests;
