use std::collections::HashSet;

use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo as McpToolInfo;
use codex_tools::ToolsConfig;
use codex_utils_output_truncation::approx_token_count;

use crate::config::Config;
use crate::connectors;
use crate::tools::handlers::mcp_tool_spec;

pub(crate) const DIRECT_MCP_TOOL_EXPOSURE_RENDERED_TOKEN_LIMIT: usize = 2_500;

pub(crate) struct McpToolExposure {
    pub(crate) direct_tools: Vec<McpToolInfo>,
    pub(crate) deferred_tools: Option<Vec<McpToolInfo>>,
}

pub(crate) fn build_mcp_tool_exposure(
    all_mcp_tools: &[McpToolInfo],
    connectors: Option<&[connectors::AppInfo]>,
    explicitly_enabled_connectors: &[connectors::AppInfo],
    config: &Config,
    tools_config: &ToolsConfig,
) -> McpToolExposure {
    let mut deferred_tools = filter_non_codex_apps_mcp_tools_only(all_mcp_tools);
    if let Some(connectors) = connectors {
        deferred_tools.extend(filter_codex_apps_mcp_tools(
            all_mcp_tools,
            connectors,
            config,
        ));
    }

    let should_defer = tools_config.search_tool
        && (config
            .features
            .enabled(Feature::ToolSearchAlwaysDeferMcpTools)
            || rendered_tool_list_token_count(&deferred_tools)
                >= DIRECT_MCP_TOOL_EXPOSURE_RENDERED_TOKEN_LIMIT);

    if !should_defer {
        return McpToolExposure {
            direct_tools: deferred_tools,
            deferred_tools: None,
        };
    }

    let direct_tools =
        filter_codex_apps_mcp_tools(all_mcp_tools, explicitly_enabled_connectors, config);
    let direct_tool_names = direct_tools
        .iter()
        .map(McpToolInfo::canonical_tool_name)
        .collect::<HashSet<_>>();
    deferred_tools.retain(|tool| !direct_tool_names.contains(&tool.canonical_tool_name()));

    McpToolExposure {
        direct_tools,
        deferred_tools: (!deferred_tools.is_empty()).then_some(deferred_tools),
    }
}

fn rendered_tool_list_token_count(mcp_tools: &[McpToolInfo]) -> usize {
    mcp_tools
        .iter()
        .map(rendered_tool_token_count)
        .sum::<usize>()
}

fn rendered_tool_token_count(tool: &McpToolInfo) -> usize {
    let Some(spec) = mcp_tool_spec(tool) else {
        return 0;
    };
    serde_json::to_string(&spec)
        .map(|rendered| approx_token_count(&rendered))
        .unwrap_or_default()
}

fn filter_non_codex_apps_mcp_tools_only(mcp_tools: &[McpToolInfo]) -> Vec<McpToolInfo> {
    mcp_tools
        .iter()
        .filter(|tool| tool.server_name != CODEX_APPS_MCP_SERVER_NAME)
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

    mcp_tools
        .iter()
        .filter(|tool| {
            if tool.server_name != CODEX_APPS_MCP_SERVER_NAME {
                return false;
            }
            let Some(connector_id) = tool.connector_id.as_deref() else {
                return false;
            };
            allowed.contains(connector_id) && connectors::codex_app_tool_is_enabled(config, tool)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "mcp_tool_exposure_test.rs"]
mod tests;
