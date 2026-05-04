use crate::JsonSchema;
use crate::LoadableToolSpec;
use crate::ResponsesApiNamespace;
use crate::ResponsesApiNamespaceTool;
use crate::ResponsesApiTool;
use crate::ToolName;
use crate::ToolSpec;
use crate::default_namespace_description;
use crate::mcp_tool_to_deferred_responses_api_tool;
use codex_app_server_protocol::AppInfo;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

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
    pub connector_description: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolSearchResultSource<'a> {
    pub server_name: &'a str,
    pub tool_namespace: &'a str,
    pub tool_name: &'a str,
    pub tool: &'a rmcp::model::Tool,
    pub connector_name: Option<&'a str>,
    pub connector_description: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableToolType {
    Connector,
    Plugin,
}

impl DiscoverableToolType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Connector => "connector",
            Self::Plugin => "plugin",
        }
    }
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

pub fn create_tool_search_tool(
    searchable_sources: &[ToolSearchSourceInfo],
    default_limit: usize,
) -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "query".to_string(),
            JsonSchema::string(Some("Search query for deferred tools.".to_string())),
        ),
        (
            "limit".to_string(),
            JsonSchema::number(Some(format!(
                "Maximum number of tools to return (defaults to {default_limit})."
            ))),
        ),
    ]);

    let mut source_descriptions = BTreeMap::new();
    for source in searchable_sources {
        source_descriptions
            .entry(source.name.clone())
            .and_modify(|existing: &mut Option<String>| {
                if existing.is_none() {
                    *existing = source.description.clone();
                }
            })
            .or_insert(source.description.clone());
    }

    let source_descriptions = if source_descriptions.is_empty() {
        "None currently enabled.".to_string()
    } else {
        source_descriptions
            .into_iter()
            .map(|(name, description)| match description {
                Some(description) => format!("- {name}: {description}"),
                None => format!("- {name}"),
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let description = format!(
        "# Tool discovery\n\nSearches over deferred tool metadata with BM25 and exposes matching tools for the next model call.\n\nYou have access to tools from the following sources:\n{source_descriptions}\nSome of the tools may not have been provided to you upfront, and you should use this tool (`{TOOL_SEARCH_TOOL_NAME}`) to search for the required tools. For MCP tool discovery, always use `{TOOL_SEARCH_TOOL_NAME}` instead of `list_mcp_resources` or `list_mcp_resource_templates`."
    );

    ToolSpec::ToolSearch {
        execution: "client".to_string(),
        description,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["query".to_string()]),
            Some(false.into()),
        ),
    }
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
        .connector_description
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
                        .connector_description
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
                description: None,
            })
        })
        .collect()
}

pub fn create_request_plugin_install_tool(
    discoverable_tools: &[RequestPluginInstallEntry],
) -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "tool_type".to_string(),
            JsonSchema::string(Some(
                "Type of discoverable tool to suggest. Use \"connector\" or \"plugin\"."
                    .to_string(),
            )),
        ),
        (
            "action_type".to_string(),
            JsonSchema::string(Some("Suggested action for the tool. Use \"install\".".to_string())),
        ),
        (
            "tool_id".to_string(),
            JsonSchema::string(Some("Connector or plugin id to suggest.".to_string())),
        ),
        (
            "suggest_reason".to_string(),
            JsonSchema::string(Some(
                "Concise one-line user-facing reason why this plugin or connector can help with the current request."
                    .to_string(),
            )),
        ),
    ]);

    let discoverable_tools = format_discoverable_tools(discoverable_tools);
    let description = format!(
        "# Request plugin/connector install\n\nUse this tool only to ask the user to install one known plugin or connector from the list below. The list contains known candidates that are not currently installed.\n\nUse this ONLY when all of the following are true:\n- The user explicitly asks to use a specific plugin or connector that is not already available in the current context or active `tools` list.\n- `{TOOL_SEARCH_TOOL_NAME}` is not available, or it has already been called and did not find or make the requested tool callable.\n- The plugin or connector is one of the known installable plugins or connectors listed below. Only ask to install plugins or connectors from this list.\n\nDo not use this tool for adjacent capabilities, broad recommendations, or tools that merely seem useful. Only use when the user explicitly asks to use that exact listed plugin or connector.\n\nKnown plugins/connectors available to install:\n{discoverable_tools}\n\nWorkflow:\n\n1. Check the current context and active `tools` list first. If current active tools aren't relevant and `{TOOL_SEARCH_TOOL_NAME}` is available, only call this tool after `{TOOL_SEARCH_TOOL_NAME}` has already been tried and found no relevant tool.\n2. Match the user's explicit request against the known plugin/connector list above. Only proceed when one listed plugin or connector exactly fits.\n3. If we found both connectors and plugins to install, use plugins first, only use connectors if the corresponding plugin is installed but the connector is not.\n4. If one plugin or connector clearly fits, call `{REQUEST_PLUGIN_INSTALL_TOOL_NAME}` with:\n   - `tool_type`: `connector` or `plugin`\n   - `action_type`: `install`\n   - `tool_id`: exact id from the known plugin/connector list above\n   - `suggest_reason`: concise one-line user-facing reason this plugin or connector can help with the current request\n5. After the request flow completes:\n   - if the user finished the install flow, continue by searching again or using the newly available plugin or connector\n   - if the user did not finish, continue without that plugin or connector, and don't request it again unless the user explicitly asks for it.\n\nIMPORTANT: DO NOT call this tool in parallel with other tools."
    );

    ToolSpec::Function(ResponsesApiTool {
        name: REQUEST_PLUGIN_INSTALL_TOOL_NAME.to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec![
                "tool_type".to_string(),
                "action_type".to_string(),
                "tool_id".to_string(),
                "suggest_reason".to_string(),
            ]),
            Some(false.into()),
        ),
        output_schema: None,
    })
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

fn format_discoverable_tools(discoverable_tools: &[RequestPluginInstallEntry]) -> String {
    let mut discoverable_tools = discoverable_tools.to_vec();
    discoverable_tools.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });

    discoverable_tools
        .into_iter()
        .map(|tool| {
            let description = tool_description_or_fallback(&tool);
            format!(
                "- {} (id: `{}`, type: {}, action: install): {}",
                tool.name,
                tool.id,
                tool.tool_type.as_str(),
                description
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool_description_or_fallback(tool: &RequestPluginInstallEntry) -> String {
    if let Some(description) = tool
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty())
    {
        return description.to_string();
    }

    match tool.tool_type {
        DiscoverableToolType::Connector => "No description provided.".to_string(),
        DiscoverableToolType::Plugin => plugin_summary(tool),
    }
}

fn plugin_summary(tool: &RequestPluginInstallEntry) -> String {
    let mut details = Vec::new();
    if tool.has_skills {
        details.push("skills".to_string());
    }
    if !tool.mcp_server_names.is_empty() {
        details.push(format!("MCP servers: {}", tool.mcp_server_names.join(", ")));
    }
    if !tool.app_connector_ids.is_empty() {
        details.push(format!(
            "app connectors: {}",
            tool.app_connector_ids.join(", ")
        ));
    }

    if details.is_empty() {
        "No description provided.".to_string()
    } else {
        details.join("; ")
    }
}

#[cfg(test)]
#[path = "tool_discovery_tests.rs"]
mod tests;
