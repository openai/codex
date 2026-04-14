use std::collections::HashMap;
use std::collections::HashSet;

use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo as McpToolInfo;
use codex_mcp::filter_non_codex_apps_mcp_tools_only;
use codex_protocol::models::ResponseItem;
use codex_tools::ToolsConfig;

use crate::config::Config;
use crate::connectors;

pub(crate) const DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnavailableMcpTool {
    pub(crate) qualified_name: String,
    pub(crate) namespace: Option<String>,
    pub(crate) name: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct McpToolExposure {
    pub(crate) direct_tools: Option<HashMap<String, McpToolInfo>>,
    pub(crate) deferred_tools: Option<HashMap<String, McpToolInfo>>,
    pub(crate) unavailable_called_tools: Vec<UnavailableMcpTool>,
}

pub(crate) fn build_mcp_tool_exposure(
    has_mcp_servers: bool,
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

    if !tools_config.search_tool || deferred_tools.len() < DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD {
        return McpToolExposure {
            direct_tools: has_mcp_servers.then_some(deferred_tools),
            deferred_tools: None,
            unavailable_called_tools: Vec::new(),
        };
    }

    let direct_tools =
        filter_codex_apps_mcp_tools(all_mcp_tools, explicitly_enabled_connectors, config);
    McpToolExposure {
        direct_tools: has_mcp_servers.then_some(direct_tools),
        deferred_tools: Some(deferred_tools),
        unavailable_called_tools: Vec::new(),
    }
}

pub(crate) fn collect_unavailable_called_mcp_tools(
    input: &[ResponseItem],
    all_mcp_tools: &HashMap<String, McpToolInfo>,
) -> Vec<UnavailableMcpTool> {
    let mut unavailable_tools = std::collections::BTreeMap::new();

    for item in input {
        let ResponseItem::FunctionCall {
            name, namespace, ..
        } = item
        else {
            continue;
        };
        if !is_mcp_function_call(name, namespace.as_deref()) {
            continue;
        }

        let qualified_name = qualified_tool_name(name, namespace.as_deref());
        if all_mcp_tools.contains_key(&qualified_name) {
            continue;
        }

        unavailable_tools
            .entry(qualified_name.clone())
            .or_insert_with(|| UnavailableMcpTool {
                qualified_name,
                namespace: namespace.clone(),
                name: name.clone(),
            });
    }

    unavailable_tools.into_values().collect()
}

fn is_mcp_function_call(name: &str, namespace: Option<&str>) -> bool {
    namespace.is_some_and(|namespace| namespace.starts_with("mcp__")) || name.starts_with("mcp__")
}

fn qualified_tool_name(name: &str, namespace: Option<&str>) -> String {
    match namespace {
        Some(namespace) if name.starts_with(namespace) => name.to_string(),
        Some(namespace) => format!("{namespace}{name}"),
        None => name.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;

    fn function_call(name: &str, namespace: Option<&str>) -> ResponseItem {
        ResponseItem::FunctionCall {
            id: None,
            name: name.to_string(),
            namespace: namespace.map(str::to_string),
            arguments: "{}".to_string(),
            call_id: format!("call-{name}"),
        }
    }

    fn mcp_tool_info(server_name: &str, tool_name: &str) -> McpToolInfo {
        McpToolInfo {
            server_name: server_name.to_string(),
            callable_name: tool_name.to_string(),
            callable_namespace: format!("mcp__{server_name}__"),
            server_instructions: None,
            tool: rmcp::model::Tool {
                name: tool_name.to_string().into(),
                title: None,
                description: Some("Test tool".to_string().into()),
                input_schema: Arc::new(rmcp::model::object(json!({"type": "object"}))),
                output_schema: None,
                annotations: None,
                execution: None,
                icons: None,
                meta: None,
            },
            connector_id: None,
            connector_name: None,
            plugin_display_names: Vec::new(),
            connector_description: None,
        }
    }

    #[test]
    fn collect_unavailable_called_mcp_tools_detects_mcp_function_calls() {
        let input = vec![
            function_call("shell", None),
            function_call("mcp__server__lookup", None),
            function_call("_create_event", Some("mcp__codex_apps__calendar")),
        ];

        let tools = collect_unavailable_called_mcp_tools(&input, &HashMap::new());

        assert_eq!(
            tools,
            vec![
                UnavailableMcpTool {
                    qualified_name: "mcp__codex_apps__calendar_create_event".to_string(),
                    namespace: Some("mcp__codex_apps__calendar".to_string()),
                    name: "_create_event".to_string(),
                },
                UnavailableMcpTool {
                    qualified_name: "mcp__server__lookup".to_string(),
                    namespace: None,
                    name: "mcp__server__lookup".to_string(),
                },
            ]
        );
    }

    #[test]
    fn collect_unavailable_called_mcp_tools_skips_currently_available_tools() {
        let input = vec![
            function_call("mcp__server__lookup", None),
            function_call("mcp__server__missing", None),
        ];
        let all_mcp_tools = HashMap::from([(
            "mcp__server__lookup".to_string(),
            mcp_tool_info("server", "lookup"),
        )]);

        let tools = collect_unavailable_called_mcp_tools(&input, &all_mcp_tools);

        assert_eq!(
            tools,
            vec![UnavailableMcpTool {
                qualified_name: "mcp__server__missing".to_string(),
                namespace: None,
                name: "mcp__server__missing".to_string(),
            }]
        );
    }
}
