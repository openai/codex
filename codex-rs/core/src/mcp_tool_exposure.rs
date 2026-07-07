use std::collections::HashSet;

use codex_connectors::AppToolPolicyEvaluator;
use codex_connectors::AppToolPolicyInput;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo as McpToolInfo;
use codex_mcp::tool_is_model_visible;
use tracing::instrument;

use crate::config::Config;
use crate::connectors;
use crate::tools::tool_search_diagnostics::DiagnosticToolIdentity;
use crate::tools::tool_search_diagnostics::ToolExposureDecision;

pub(crate) struct McpToolExposure {
    pub(crate) direct_tools: Vec<McpToolInfo>,
    pub(crate) deferred_tools: Option<Vec<McpToolInfo>>,
    pub(crate) diagnostic_decisions: Vec<ToolExposureDecision>,
}

#[instrument(level = "trace", skip_all)]
pub(crate) fn build_mcp_tool_exposure(
    all_mcp_tools: &[McpToolInfo],
    connectors: Option<&[connectors::AppInfo]>,
    config: &Config,
    search_tool_enabled: bool,
) -> McpToolExposure {
    let mut deferred_tools = filter_non_codex_apps_mcp_tools_only(all_mcp_tools);
    if let Some(connectors) = connectors {
        deferred_tools.extend(filter_codex_apps_mcp_tools(
            all_mcp_tools,
            connectors,
            config,
        ));
    }
    let diagnostic_decisions = tool_exposure_decisions(all_mcp_tools, connectors, config);

    if !search_tool_enabled {
        return McpToolExposure {
            direct_tools: deferred_tools,
            deferred_tools: None,
            diagnostic_decisions,
        };
    }

    McpToolExposure {
        direct_tools: Vec::new(),
        deferred_tools: (!deferred_tools.is_empty()).then_some(deferred_tools),
        diagnostic_decisions,
    }
}

fn filter_non_codex_apps_mcp_tools_only(mcp_tools: &[McpToolInfo]) -> Vec<McpToolInfo> {
    mcp_tools
        .iter()
        .filter(|tool| {
            tool.server_name != CODEX_APPS_MCP_SERVER_NAME && tool_is_model_visible(tool)
        })
        .cloned()
        .collect()
}

fn filter_codex_apps_mcp_tools(
    mcp_tools: &[McpToolInfo],
    connectors: &[connectors::AppInfo],
    config: &Config,
) -> Vec<McpToolInfo> {
    let allowed: HashSet<&str> = connectors
        .iter()
        .map(|connector| connector.id.as_str())
        .collect();
    let app_tool_policy = AppToolPolicyEvaluator::new(&config.config_layer_stack);

    mcp_tools
        .iter()
        .filter(|tool| {
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
        .cloned()
        .collect()
}

fn tool_exposure_decisions(
    all_mcp_tools: &[McpToolInfo],
    connectors: Option<&[connectors::AppInfo]>,
    config: &Config,
) -> Vec<ToolExposureDecision> {
    let allowed = connectors
        .into_iter()
        .flatten()
        .map(|connector| connector.id.as_str())
        .collect::<HashSet<_>>();
    let app_tool_policy = AppToolPolicyEvaluator::new(&config.config_layer_stack);

    all_mcp_tools
        .iter()
        .map(|tool| {
            let is_codex_apps_tool = tool.server_name == CODEX_APPS_MCP_SERVER_NAME;
            let model_visible = tool_is_model_visible(tool);
            let connector_id_present = tool.connector_id.is_some();
            let connector_allowed = !is_codex_apps_tool
                || (connectors.is_some()
                    && tool
                        .connector_id
                        .as_deref()
                        .is_some_and(|connector_id| allowed.contains(connector_id)));
            let policy_enabled = !is_codex_apps_tool
                || tool.connector_id.as_deref().is_some_and(|connector_id| {
                    let annotations = tool.tool.annotations.as_ref();
                    app_tool_policy
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
                });
            ToolExposureDecision {
                identity: DiagnosticToolIdentity::from_tool_info(tool),
                included: model_visible && connector_allowed && policy_enabled,
                model_visible,
                connector_id_present,
                connector_allowed,
                policy_enabled,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "mcp_tool_exposure_test.rs"]
mod tests;
