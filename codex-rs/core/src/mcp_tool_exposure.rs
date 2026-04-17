use std::collections::HashMap;
use std::collections::HashSet;

use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo as McpToolInfo;
use codex_mcp::filter_non_codex_apps_mcp_tools_only;
use codex_tools::ToolsConfig;

use crate::codex_apps_mcp_tools::is_direct_exposed_codex_apps_builtin_tool_info;
use crate::config::Config;
use crate::connectors;

pub(crate) const DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD: usize = 100;

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
    deferred_tools.extend(filter_direct_exposed_builtin_codex_apps_tools(
        all_mcp_tools,
    ));
    if let Some(connectors) = connectors {
        deferred_tools.extend(filter_codex_apps_mcp_tools(
            all_mcp_tools,
            connectors,
            config,
        ));
    }

    if !tools_config.search_tool || deferred_tools.len() < DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD {
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

    let mut direct_tools = filter_direct_exposed_builtin_codex_apps_tools(all_mcp_tools);
    direct_tools.extend(filter_codex_apps_mcp_tools(
        all_mcp_tools,
        explicitly_enabled_connectors,
        config,
    ));
    for direct_tool_name in direct_tools.keys() {
        deferred_tools.remove(direct_tool_name);
    }
    McpToolExposure {
        direct_tools,
        deferred_tools: (!deferred_tools.is_empty()).then_some(deferred_tools),
    }
}

fn filter_direct_exposed_builtin_codex_apps_tools(
    mcp_tools: &HashMap<String, McpToolInfo>,
) -> HashMap<String, McpToolInfo> {
    mcp_tools
        .iter()
        .filter(|(_, tool)| is_direct_exposed_codex_apps_builtin_tool_info(tool))
        .map(|(name, tool)| (name.clone(), tool.clone()))
        .collect()
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
