use std::collections::HashSet;
use std::sync::Arc;

use codex_connectors::AppToolPolicyEvaluator;
use codex_connectors::AppToolPolicyInput;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo as McpToolInfo;
use codex_mcp::tool_is_model_visible;
use codex_tools::ToolExposure;
use codex_tools::ToolName;
use tracing::instrument;
use tracing::warn;

const MAX_AGENT_PLUGIN_MCP_SPEC_BYTES: usize = 8_000;
const MAX_AGENT_PLUGIN_MCP_TOTAL_BYTES: usize = 64_000;

use crate::config::Config;
use crate::tools::handlers::McpHandler;
use crate::tools::registry::ToolRegistry;

#[instrument(level = "trace", skip_all)]
pub(crate) fn append_mcp_tools(
    all_mcp_tools: &[McpToolInfo],
    config: &Config,
    apps_enabled: bool,
    mcp_server_catalog: &codex_mcp::ResolvedMcpCatalog,
    search_tool_enabled: bool,
    registry: &mut ToolRegistry,
) -> HashSet<ToolName> {
    // Keep regular MCP tools first; Apps tools also require connector and policy checks.
    let non_app_tools = filter_non_codex_apps_mcp_tools_only(all_mcp_tools);
    let app_tools = apps_enabled
        .then(|| filter_codex_apps_mcp_tools(all_mcp_tools, config))
        .into_iter()
        .flatten();
    let exposure = if search_tool_enabled {
        ToolExposure::Deferred
    } else {
        ToolExposure::Direct
    };
    let mut registered_tools = HashSet::new();
    let mut agent_plugin_bytes = 0usize;
    for tool in non_app_tools.chain(app_tools) {
        let tool_name = tool.canonical_tool_name();
        let agent_plugin = mcp_server_catalog
            .server(&tool.server_name)
            .is_some_and(|server| server.source().is_agent_plugin());
        let handler = if agent_plugin {
            McpHandler::new_agent_plugin(tool.clone())
        } else {
            McpHandler::new(tool.clone())
        };
        match handler {
            Ok(handler) => {
                let fits_agent_budget = if agent_plugin {
                    handler.model_spec_bytes().is_ok_and(|bytes| {
                        if bytes > MAX_AGENT_PLUGIN_MCP_SPEC_BYTES {
                            return false;
                        }
                        let next = agent_plugin_bytes.saturating_add(bytes);
                        if next <= MAX_AGENT_PLUGIN_MCP_TOTAL_BYTES {
                            agent_plugin_bytes = next;
                            true
                        } else {
                            false
                        }
                    })
                } else {
                    true
                };
                let tool_exposure = if fits_agent_budget {
                    exposure
                } else {
                    ToolExposure::Hidden
                };
                if registry.register_external_with_exposure(Arc::new(handler), tool_exposure)
                    && fits_agent_budget
                {
                    registered_tools.insert(tool_name);
                }
            }
            Err(err) => {
                warn!("Skipping MCP tool `{tool_name}`: failed to build tool spec: {err}");
            }
        }
    }
    registered_tools
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
    config: &'a Config,
) -> impl Iterator<Item = &'a McpToolInfo> + 'a {
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
        app_tool_policy
            .policy(AppToolPolicyInput {
                connector_id: Some(connector_id),
                tool_name: &tool.tool.name,
                tool_title: tool.tool.title.as_deref(),
                destructive_hint: annotations.and_then(|annotations| annotations.destructive_hint),
                open_world_hint: annotations.and_then(|annotations| annotations.open_world_hint),
            })
            .enabled
    })
}

#[cfg(test)]
#[path = "mcp_tool_exposure_test.rs"]
mod tests;
