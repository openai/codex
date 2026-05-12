use crate::LoadableToolSpec;
use crate::ResponsesApiNamespace;
use crate::ResponsesApiNamespaceTool;
use crate::ToolName;
use crate::default_namespace_description;
use crate::mcp_tool_to_deferred_responses_api_tool;
use codex_app_server_protocol::AppInfo;
use serde::Deserialize;
use serde::Serialize;

const TUI_CLIENT_NAME: &str = "codex-tui";
pub const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
pub const TOOL_SEARCH_DEFAULT_LIMIT: usize = 8;
pub const REQUEST_PLUGIN_INSTALL_TOOL_NAME: &str = "request_plugin_install";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolSearchSourceInfo {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolSearchSource<'a> {
    pub server_name: &'a str,
    pub connector_name: Option<&'a str>,
    pub description: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolSearchResultSource<'a> {
    pub server_name: &'a str,
    pub tool_namespace: &'a str,
    pub tool_name: &'a str,
    pub tool: &'a rmcp::model::Tool,
    pub connector_name: Option<&'a str>,
    pub description: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableToolType {
    Connector,
    Plugin,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableToolAction {
    Install,
    Enable,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiscoverableTool {
    Connector(Box<AppInfo>),
    Plugin(Box<DiscoverablePluginInfo>),
}

impl DiscoverableTool {
    pub fn tool_type(&self) -> DiscoverableToolType {
        match self {
            Self::Connector(_) => DiscoverableToolType::Connector,
            Self::Plugin(_) => DiscoverableToolType::Plugin,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Connector(connector) => connector.id.as_str(),
            Self::Plugin(plugin) => plugin.id.as_str(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Connector(connector) => connector.name.as_str(),
            Self::Plugin(plugin) => plugin.name.as_str(),
        }
    }

    pub fn install_url(&self) -> Option<&str> {
        match self {
            Self::Connector(connector) => connector.install_url.as_deref(),
            Self::Plugin(_) => None,
        }
    }
}

impl From<AppInfo> for DiscoverableTool {
    fn from(value: AppInfo) -> Self {
        Self::Connector(Box::new(value))
    }
}

impl From<DiscoverablePluginInfo> for DiscoverableTool {
    fn from(value: DiscoverablePluginInfo) -> Self {
        Self::Plugin(Box::new(value))
    }
}

pub fn filter_request_plugin_install_discoverable_tools_for_client(
    discoverable_tools: Vec<DiscoverableTool>,
    app_server_client_name: Option<&str>,
) -> Vec<DiscoverableTool> {
    if app_server_client_name != Some(TUI_CLIENT_NAME) {
        return discoverable_tools;
    }

    discoverable_tools
        .into_iter()
        .filter(|tool| !matches!(tool, DiscoverableTool::Plugin(_)))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoverablePluginInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub has_skills: bool,
    pub mcp_server_names: Vec<String>,
    pub app_connector_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestPluginInstallEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tool_type: DiscoverableToolType,
    pub has_skills: bool,
    pub mcp_server_names: Vec<String>,
    pub app_connector_ids: Vec<String>,
}

pub fn tool_search_result_source_to_loadable_tool_spec(
    source: ToolSearchResultSource<'_>,
) -> Result<LoadableToolSpec, serde_json::Error> {
    Ok(LoadableToolSpec::Namespace(ResponsesApiNamespace {
        name: source.tool_namespace.to_string(),
        description: tool_search_result_source_namespace_description(source),
        tools: vec![tool_search_result_source_to_namespace_tool(source)?],
    }))
}

fn tool_search_result_source_namespace_description(source: ToolSearchResultSource<'_>) -> String {
    source
        .description
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(str::to_string)
        .or_else(|| {
            source
                .connector_name
                .map(str::trim)
                .filter(|connector_name| !connector_name.is_empty())
                .map(|connector_name| format!("Tools for working with {connector_name}."))
        })
        .unwrap_or_else(|| default_namespace_description(source.tool_namespace))
}

fn tool_search_result_source_to_namespace_tool(
    source: ToolSearchResultSource<'_>,
) -> Result<ResponsesApiNamespaceTool, serde_json::Error> {
    let tool_name = ToolName::namespaced(source.tool_namespace, source.tool_name);
    mcp_tool_to_deferred_responses_api_tool(&tool_name, source.tool)
        .map(ResponsesApiNamespaceTool::Function)
}

pub fn collect_tool_search_source_infos<'a>(
    searchable_tools: impl IntoIterator<Item = ToolSearchSource<'a>>,
) -> Vec<ToolSearchSourceInfo> {
    searchable_tools
        .into_iter()
        .filter_map(|tool| {
            if let Some(name) = tool
                .connector_name
                .map(str::trim)
                .filter(|connector_name| !connector_name.is_empty())
            {
                return Some(ToolSearchSourceInfo {
                    name: name.to_string(),
                    description: tool
                        .description
                        .map(str::trim)
                        .filter(|description| !description.is_empty())
                        .map(str::to_string),
                });
            }

            let name = tool.server_name.trim();
            if name.is_empty() {
                return None;
            }

            Some(ToolSearchSourceInfo {
                name: name.to_string(),
                description: tool
                    .description
                    .map(str::trim)
                    .filter(|description| !description.is_empty())
                    .map(str::to_string),
            })
        })
        .collect()
}

pub fn collect_request_plugin_install_entries(
    discoverable_tools: &[DiscoverableTool],
) -> Vec<RequestPluginInstallEntry> {
    discoverable_tools
        .iter()
        .map(|tool| match tool {
            DiscoverableTool::Connector(connector) => RequestPluginInstallEntry {
                id: connector.id.clone(),
                name: connector.name.clone(),
                description: connector.description.clone(),
                tool_type: DiscoverableToolType::Connector,
                has_skills: false,
                mcp_server_names: Vec::new(),
                app_connector_ids: Vec::new(),
            },
            DiscoverableTool::Plugin(plugin) => RequestPluginInstallEntry {
                id: plugin.id.clone(),
                name: plugin.name.clone(),
                description: plugin.description.clone(),
                tool_type: DiscoverableToolType::Plugin,
                has_skills: plugin.has_skills,
                mcp_server_names: plugin.mcp_server_names.clone(),
                app_connector_ids: plugin.app_connector_ids.clone(),
            },
        })
        .collect()
}

#[cfg(test)]
#[path = "tool_discovery_tests.rs"]
mod tests;
