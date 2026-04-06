use crate::JsonSchema;
use crate::ResponsesApiNamespace;
use crate::ResponsesApiNamespaceTool;
use crate::ResponsesApiTool;
use crate::ToolSearchOutputTool;
use crate::ToolSpec;
use crate::mcp_tool_to_deferred_responses_api_tool;
use codex_app_server_protocol::AppInfo;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

const TUI_CLIENT_NAME: &str = "codex-tui";
pub const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
pub const TOOL_SEARCH_DEFAULT_LIMIT: usize = 8;
pub const TOOL_SUGGEST_TOOL_NAME: &str = "tool_suggest";

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
    pub qualified_tool_name: &'a str,
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

pub fn filter_tool_suggest_discoverable_tools_for_client(
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
pub struct ToolSuggestEntry {
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
            JsonSchema::String {
                description: Some("Search query for MCP tools.".to_string()),
            },
        ),
        (
            "limit".to_string(),
            JsonSchema::Number {
                description: Some(format!(
                    "Maximum number of tools to return (defaults to {default_limit})."
                )),
            },
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
        "# MCP tool discovery\n\nSearches over MCP tool metadata with BM25 and exposes matching tools for the next model call.\n\nYou have access to tools from the following MCP servers/connectors:\n{source_descriptions}\nSome of the tools may not have been provided to you upfront, and you should use this tool (`{TOOL_SEARCH_TOOL_NAME}`) to search for the required MCP tools. For MCP tool discovery, always use `{TOOL_SEARCH_TOOL_NAME}` instead of `list_mcp_resources` or `list_mcp_resource_templates`."
    );

    ToolSpec::ToolSearch {
        execution: "client".to_string(),
        description,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["query".to_string()]),
            additional_properties: Some(false.into()),
        },
    }
}

pub fn collect_tool_search_output_tools<'a>(
    tool_sources: impl IntoIterator<Item = ToolSearchResultSource<'a>>,
) -> Result<Vec<ToolSearchOutputTool>, serde_json::Error> {
    let grouped = tool_sources.into_iter().fold(
        BTreeMap::<String, Vec<(String, ToolSearchResultSource<'a>)>>::new(),
        |mut grouped, tool| {
            let (tool_namespace, tool_name) = tool_search_output_namespace_and_name(
                tool.qualified_tool_name,
                tool.tool_namespace,
                tool.tool_name,
            );
            grouped
                .entry(tool_namespace)
                .or_default()
                .push((tool_name, tool));
            grouped
        },
    );

    let mut results = Vec::with_capacity(grouped.len());
    for (tool_namespace, tools) in grouped {
        let Some(first_tool) = tools.first() else {
            continue;
        };

        let description = first_tool
            .1
            .connector_description
            .map(str::to_string)
            .or_else(|| {
                first_tool
                    .1
                    .connector_name
                    .map(str::trim)
                    .filter(|connector_name| !connector_name.is_empty())
                    .map(|connector_name| format!("Tools for working with {connector_name}."))
            })
            .or_else(|| {
                Some(format!(
                    "Tools from the {} MCP server.",
                    first_tool.1.server_name
                ))
            });

        let tools = tools
            .iter()
            .map(|tool| {
                mcp_tool_to_deferred_responses_api_tool(tool.0.clone(), tool.1.tool)
                    .map(ResponsesApiNamespaceTool::Function)
            })
            .collect::<Result<Vec<_>, _>>()?;

        results.push(ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
            name: tool_namespace.to_string(),
            description: description.unwrap_or_default(),
            tools,
        }));
    }

    Ok(results)
}

pub fn tool_search_output_namespace_and_name(
    qualified_tool_name: &str,
    tool_namespace: &str,
    tool_name: &str,
) -> (String, String) {
    if let Some(child_name) = qualified_tool_name.strip_prefix(tool_namespace)
        && child_name == tool_name
    {
        return (tool_namespace.to_string(), tool_name.to_string());
    }

    if let Some((namespace, child_name)) =
        split_qualified_mcp_tool_name_for_search(qualified_tool_name)
    {
        return (namespace.to_string(), child_name.to_string());
    }

    (tool_namespace.to_string(), tool_name.to_string())
}

fn split_qualified_mcp_tool_name_for_search(qualified_tool_name: &str) -> Option<(&str, &str)> {
    let stripped = qualified_tool_name.strip_prefix("mcp__")?;
    let server_name_len = stripped.find("__")?;
    let child_name_start = server_name_len + "__".len();
    let child_name = stripped.get(child_name_start..)?;
    if child_name.is_empty() {
        return None;
    }

    let namespace_len = "mcp__".len() + server_name_len + "__".len();
    Some((&qualified_tool_name[..namespace_len], child_name))
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
                let description = connector_description(tool.connector_description);
                return Some(ToolSearchSourceInfo {
                    name: name.to_string(),
                    description,
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

fn connector_description(description: Option<&str>) -> Option<String> {
    description
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(str::to_string)
}

pub fn create_tool_suggest_tool(discoverable_tools: &[ToolSuggestEntry]) -> ToolSpec {
    let discoverable_tool_ids = discoverable_tools
        .iter()
        .map(|tool| tool.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let properties = BTreeMap::from([
        (
            "tool_type".to_string(),
            JsonSchema::String {
                description: Some(
                    "Type of discoverable tool to suggest. Use \"connector\" or \"plugin\"."
                        .to_string(),
                ),
            },
        ),
        (
            "action_type".to_string(),
            JsonSchema::String {
                description: Some(
                    "Suggested action for the tool. Use \"install\" or \"enable\".".to_string(),
                ),
            },
        ),
        (
            "tool_id".to_string(),
            JsonSchema::String {
                description: Some(format!(
                    "Connector or plugin id to suggest. Must be one of: {discoverable_tool_ids}."
                )),
            },
        ),
        (
            "suggest_reason".to_string(),
            JsonSchema::String {
                description: Some(
                    "Concise one-line user-facing reason why this tool can help with the current request."
                        .to_string(),
                ),
            },
        ),
    ]);

    let discoverable_tools = format_discoverable_tools(discoverable_tools);
    let description = format!(
        "# Tool suggestion discovery\n\nSuggests a missing connector in an installed plugin, or in narrower cases a not installed but discoverable plugin, when the user clearly wants a capability that is not currently available in the active `tools` list.\n\nUse this ONLY when:\n- You've already tried to find a matching available tool for the user's request but couldn't find a good match. This includes `{TOOL_SEARCH_TOOL_NAME}` (if available) and other means.\n- For connectors/apps that are not installed but needed for an installed plugin, suggest to install them if the task requirements match precisely.\n- For plugins that are not installed but discoverable, only suggest discoverable and installable plugins when the user's intent very explicitly and unambiguously matches that plugin itself. Do not suggest a plugin just because one of its connectors or capabilities seems relevant.\n\nTool suggestions should only use the discoverable tools listed here. DO NOT explore or recommend tools that are not on this list.\n\nDiscoverable tools:\n{discoverable_tools}\n\nWorkflow:\n\n1. Ensure all possible means have been exhausted to find an existing available tool but none of them matches the request intent.\n2. Match the user's request against the discoverable tools list above. Apply the stricter explicit-and-unambiguous rule for *discoverable tools* like plugin install suggestions; *missing tools* like connector install suggestions continue to use the normal clear-fit standard.\n3. If one tool clearly fits, call `{TOOL_SUGGEST_TOOL_NAME}` with:\n   - `tool_type`: `connector` or `plugin`\n   - `action_type`: `install` or `enable`\n   - `tool_id`: exact id from the discoverable tools list above\n   - `suggest_reason`: concise one-line user-facing reason this tool can help with the current request\n4. After the suggestion flow completes:\n   - if the user finished the install or enable flow, continue by searching again or using the newly available tool\n   - if the user did not finish, continue without that tool, and don't suggest that tool again unless the user explicitly asks for it."
    );

    ToolSpec::Function(ResponsesApiTool {
        name: TOOL_SUGGEST_TOOL_NAME.to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec![
                "tool_type".to_string(),
                "action_type".to_string(),
                "tool_id".to_string(),
                "suggest_reason".to_string(),
            ]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

pub fn collect_tool_suggest_entries(
    discoverable_tools: &[DiscoverableTool],
) -> Vec<ToolSuggestEntry> {
    discoverable_tools
        .iter()
        .map(|tool| match tool {
            DiscoverableTool::Connector(connector) => ToolSuggestEntry {
                id: connector.id.clone(),
                name: connector.name.clone(),
                description: connector.description.clone(),
                tool_type: DiscoverableToolType::Connector,
                has_skills: false,
                mcp_server_names: Vec::new(),
                app_connector_ids: Vec::new(),
            },
            DiscoverableTool::Plugin(plugin) => ToolSuggestEntry {
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

fn format_discoverable_tools(discoverable_tools: &[ToolSuggestEntry]) -> String {
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

fn tool_description_or_fallback(tool: &ToolSuggestEntry) -> String {
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

fn plugin_summary(tool: &ToolSuggestEntry) -> String {
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
