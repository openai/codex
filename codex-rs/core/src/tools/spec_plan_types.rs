use crate::tools::handlers::multi_agents_spec::WaitAgentTimeoutOptions;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tool_api::ToolBundle as ExtensionToolBundle;
use codex_tools::DiscoverableTool;
use codex_tools::ToolName;
use codex_tools::ToolsConfig;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct ToolRegistryBuildParams<'a> {
    pub mcp_tools: Option<&'a [ToolRegistryBuildMcpTool<'a>]>,
    pub deferred_mcp_tools: Option<&'a [ToolRegistryBuildDeferredTool<'a>]>,
    pub tool_namespaces: Option<&'a HashMap<String, ToolNamespace>>,
    pub discoverable_tools: Option<&'a [DiscoverableTool]>,
    pub extension_tool_bundles: &'a [ExtensionToolBundle],
    pub dynamic_tools: &'a [DynamicToolSpec],
    pub default_agent_type_description: &'a str,
    pub wait_agent_timeouts: WaitAgentTimeoutOptions,
    pub tool_search_entries: &'a [crate::tools::tool_search_entry::ToolSearchEntry],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolNamespace {
    pub name: String,
    pub description: Option<String>,
}

/// Direct MCP tool metadata needed to expose the Responses API namespace tool
/// while registering its runtime handler with the canonical namespace/name
/// identity.
#[derive(Debug, Clone)]
pub struct ToolRegistryBuildMcpTool<'a> {
    pub name: ToolName,
    pub tool: &'a rmcp::model::Tool,
}

#[derive(Debug, Clone)]
pub struct ToolRegistryBuildDeferredTool<'a> {
    pub name: ToolName,
    pub server_name: &'a str,
    pub connector_name: Option<&'a str>,
    pub description: Option<&'a str>,
}

pub(crate) fn agent_type_description(
    config: &ToolsConfig,
    default_agent_type_description: &str,
) -> String {
    if config.agent_type_description.is_empty() {
        default_agent_type_description.to_string()
    } else {
        config.agent_type_description.clone()
    }
}
