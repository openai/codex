use std::collections::HashMap;
use std::collections::HashSet;

use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo as McpToolInfo;
use codex_mcp::filter_non_codex_apps_mcp_tools_only;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolsConfig;
use codex_utils_string::approx_bytes_for_tokens;

use crate::config::Config;
use crate::connectors;

pub(crate) const DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD: usize = 100;
const DIRECT_MCP_TOOL_EXPOSURE_TOKEN_THRESHOLD: usize = 2_000;

pub(crate) struct McpToolExposure {
    pub(crate) direct_tools: HashMap<String, McpToolInfo>,
    pub(crate) deferred_tools: Option<HashMap<String, McpToolInfo>>,
}

pub(crate) fn build_mcp_tool_exposure(
    all_mcp_tools: &HashMap<String, McpToolInfo>,
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

    if !tools_config.search_tool {
        return McpToolExposure {
            direct_tools: deferred_tools,
            deferred_tools: None,
        };
    }

    let should_defer = if config
        .features
        .enabled(Feature::ToolSearchTokenBudgetDeferral)
    {
        estimate_direct_mcp_tool_schema_bytes(&deferred_tools)
            >= approx_bytes_for_tokens(DIRECT_MCP_TOOL_EXPOSURE_TOKEN_THRESHOLD)
    } else {
        deferred_tools.len() >= DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD
    };

    if !should_defer {
        return McpToolExposure {
            direct_tools: deferred_tools,
            deferred_tools: None,
        };
    }

    let direct_tools =
        filter_codex_apps_mcp_tools(all_mcp_tools, explicitly_enabled_connectors, config);
    for direct_tool_name in direct_tools.keys() {
        deferred_tools.remove(direct_tool_name);
    }

    McpToolExposure {
        direct_tools,
        deferred_tools: (!deferred_tools.is_empty()).then_some(deferred_tools),
    }
}

fn filter_codex_apps_mcp_tools(
    mcp_tools: &HashMap<String, McpToolInfo>,
    connectors: &[connectors::AppInfo],
    config: &Config,
) -> HashMap<String, McpToolInfo> {
    let allowed: HashSet<&str> = connectors
        .iter()
        .map(|connector| connector.id.as_str())
        .collect();

    mcp_tools
        .iter()
        .filter(|(_, tool)| {
            if tool.server_name != CODEX_APPS_MCP_SERVER_NAME {
                return false;
            }
            let Some(connector_id) = tool.connector_id.as_deref() else {
                return false;
            };
            allowed.contains(connector_id) && connectors::codex_app_tool_is_enabled(config, tool)
        })
        .map(|(name, tool)| (name.clone(), tool.clone()))
        .collect()
}

fn estimate_direct_mcp_tool_schema_bytes(mcp_tools: &HashMap<String, McpToolInfo>) -> usize {
    mcp_tools
        .values()
        .filter_map(|tool| {
            let responses_tool = codex_tools::mcp_tool_to_responses_api_tool(
                &tool.canonical_tool_name(),
                &tool.tool,
            )
            .ok()?;
            serde_json::to_vec(&ResponsesApiNamespaceTool::Function(responses_tool))
                .ok()
                .map(|bytes| bytes.len())
        })
        .sum()
}

#[cfg(test)]
#[path = "mcp_tool_exposure_test.rs"]
mod tests;
