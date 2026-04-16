use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::ToolSearchOutput;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use bm25::Document;
use bm25::Language;
use bm25::SearchEngine;
use bm25::SearchEngineBuilder;
use codex_mcp::ToolInfo;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::TOOL_SEARCH_DEFAULT_LIMIT;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolSearchOutputTool;
use codex_tools::dynamic_tool_to_responses_api_tool;
use codex_tools::mcp_tool_to_deferred_responses_api_tool;

const COMPUTER_USE_MCP_SERVER_NAME: &str = "computer-use";
const COMPUTER_USE_TOOL_SEARCH_LIMIT: usize = 20;

pub struct ToolSearchHandler {
    entries: Vec<ToolSearchEntry>,
    search_engine: SearchEngine<usize>,
}

impl ToolSearchHandler {
    pub fn new(
        mcp_tools: std::collections::HashMap<String, ToolInfo>,
        dynamic_tools: Vec<DynamicToolSpec>,
    ) -> Self {
        let mut mcp_entries: Vec<ToolSearchEntry> = mcp_tools
            .into_values()
            .map(|info| ToolSearchEntry::Mcp {
                name: info.canonical_tool_name().display(),
                info: Box::new(info),
            })
            .collect();
        mcp_entries.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));

        let mut dynamic_entries: Vec<ToolSearchEntry> = dynamic_tools
            .into_iter()
            .map(|tool| ToolSearchEntry::Dynamic { tool })
            .collect();
        dynamic_entries.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));

        let mut entries = mcp_entries;
        entries.extend(dynamic_entries);

        let documents: Vec<Document<usize>> = entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| Document::new(idx, entry.search_text()))
            .collect();
        let search_engine =
            SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();

        Self {
            entries,
            search_engine,
        }
    }
}

impl ToolHandler for ToolSearchHandler {
    type Output = ToolSearchOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<ToolSearchOutput, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        let args = match payload {
            ToolPayload::ToolSearch { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{TOOL_SEARCH_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        let query = args.query.trim();
        if query.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "query must not be empty".to_string(),
            ));
        }
        let requested_limit = args.limit;
        let limit = requested_limit.unwrap_or(TOOL_SEARCH_DEFAULT_LIMIT);

        if limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        if self.entries.is_empty() {
            return Ok(ToolSearchOutput { tools: Vec::new() });
        }

        let tools = self.search(query, limit, requested_limit.is_none())?;

        Ok(ToolSearchOutput { tools })
    }
}

impl ToolSearchHandler {
    fn search(
        &self,
        query: &str,
        limit: usize,
        use_default_limit: bool,
    ) -> Result<Vec<ToolSearchOutputTool>, FunctionCallError> {
        let results = self.search_result_entries(query, limit, use_default_limit);
        search_output_tools(results)
    }

    fn search_result_entries(
        &self,
        query: &str,
        limit: usize,
        use_default_limit: bool,
    ) -> Vec<&ToolSearchEntry> {
        let mut results = self
            .search_engine
            .search(query, limit)
            .into_iter()
            .filter_map(|result| self.entries.get(result.document.id))
            .collect::<Vec<_>>();
        if !use_default_limit {
            return results;
        }

        if results
            .iter()
            .any(|entry| entry.mcp_server_name() == Some(COMPUTER_USE_MCP_SERVER_NAME))
        {
            results = self
                .search_engine
                .search(query, COMPUTER_USE_TOOL_SEARCH_LIMIT)
                .into_iter()
                .filter_map(|result| self.entries.get(result.document.id))
                .collect();
        }
        limit_results_per_server(results)
    }
}

fn search_output_tools<'a>(
    results: impl IntoIterator<Item = &'a ToolSearchEntry>,
) -> Result<Vec<ToolSearchOutputTool>, FunctionCallError> {
    let mut tools = Vec::new();
    // Preserve search order: group MCP tools under namespaces, emit dynamic tools directly.
    for entry in results {
        match entry {
            ToolSearchEntry::Mcp { info, .. } => {
                let tool_name = info.canonical_tool_name();
                let namespace = info.callable_namespace.as_str();
                let namespace_tool =
                    mcp_tool_to_deferred_responses_api_tool(&tool_name, &info.tool)
                        .map(ResponsesApiNamespaceTool::Function)
                        .map_err(tool_search_output_error)?;

                if let Some(output) = tools.iter_mut().find_map(|tool| match tool {
                    ToolSearchOutputTool::Namespace(output) if output.name == namespace => {
                        Some(output)
                    }
                    ToolSearchOutputTool::Namespace(_) | ToolSearchOutputTool::Function(_) => None,
                }) {
                    output.tools.push(namespace_tool);
                } else {
                    tools.push(ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
                        name: namespace.to_string(),
                        description: mcp_namespace_description(info),
                        tools: vec![namespace_tool],
                    }));
                }
            }
            ToolSearchEntry::Dynamic { tool } => {
                tools.push(ToolSearchOutputTool::Function(
                    dynamic_tool_to_responses_api_tool(tool).map_err(tool_search_output_error)?,
                ));
            }
        }
    }

    Ok(tools)
}

fn mcp_namespace_description(info: &ToolInfo) -> String {
    info.connector_description
        .clone()
        .or_else(|| {
            info.connector_name
                .as_deref()
                .map(str::trim)
                .filter(|connector_name| !connector_name.is_empty())
                .map(|connector_name| format!("Tools for working with {connector_name}."))
        })
        .unwrap_or_else(|| format!("Tools from the {} MCP server.", info.server_name))
}

fn limit_results_per_server(results: Vec<&ToolSearchEntry>) -> Vec<&ToolSearchEntry> {
    results
        .into_iter()
        .scan(
            std::collections::HashMap::<&str, usize>::new(),
            |counts, entry| {
                let Some(server_name) = entry.mcp_server_name() else {
                    return Some(Some(entry));
                };
                let count = counts.entry(server_name).or_default();
                if *count >= default_limit_for_server(server_name) {
                    Some(None)
                } else {
                    *count += 1;
                    Some(Some(entry))
                }
            },
        )
        .flatten()
        .collect()
}

fn default_limit_for_server(server_name: &str) -> usize {
    if server_name == COMPUTER_USE_MCP_SERVER_NAME {
        COMPUTER_USE_TOOL_SEARCH_LIMIT
    } else {
        TOOL_SEARCH_DEFAULT_LIMIT
    }
}

enum ToolSearchEntry {
    Mcp { name: String, info: Box<ToolInfo> },
    Dynamic { tool: DynamicToolSpec },
}

impl ToolSearchEntry {
    fn sort_key(&self) -> &str {
        match self {
            Self::Mcp { name, .. } => name.as_str(),
            Self::Dynamic { tool } => tool.name.as_str(),
        }
    }

    fn search_text(&self) -> String {
        match self {
            Self::Mcp { name, info } => build_mcp_search_text(name, info),
            Self::Dynamic { tool } => build_dynamic_search_text(tool),
        }
    }

    fn mcp_server_name(&self) -> Option<&str> {
        match self {
            Self::Mcp { info, .. } => Some(info.server_name.as_str()),
            Self::Dynamic { .. } => None,
        }
    }
}

fn tool_search_output_error(err: serde_json::Error) -> FunctionCallError {
    FunctionCallError::Fatal(format!(
        "failed to encode {TOOL_SEARCH_TOOL_NAME} output: {err}"
    ))
}

fn build_mcp_search_text(name: &str, info: &ToolInfo) -> String {
    let mut parts = vec![
        name.to_string(),
        info.callable_name.clone(),
        info.tool.name.to_string(),
        info.server_name.clone(),
    ];

    if let Some(title) = info.tool.title.as_deref()
        && !title.trim().is_empty()
    {
        parts.push(title.to_string());
    }

    if let Some(description) = info.tool.description.as_deref()
        && !description.trim().is_empty()
    {
        parts.push(description.to_string());
    }

    if let Some(connector_name) = info.connector_name.as_deref()
        && !connector_name.trim().is_empty()
    {
        parts.push(connector_name.to_string());
    }

    if let Some(connector_description) = info.connector_description.as_deref()
        && !connector_description.trim().is_empty()
    {
        parts.push(connector_description.to_string());
    }

    parts.extend(
        info.plugin_display_names
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string),
    );

    parts.extend(
        info.tool
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
    );

    parts.join(" ")
}

fn build_dynamic_search_text(tool: &DynamicToolSpec) -> String {
    let mut parts = vec![
        tool.name.clone(),
        tool.name.replace('_', " "),
        tool.description.clone(),
    ];

    parts.extend(
        tool.input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
    );

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_tools::ResponsesApiNamespace;
    use codex_tools::ResponsesApiNamespaceTool;
    use codex_tools::ResponsesApiTool;
    use pretty_assertions::assert_eq;
    use rmcp::model::Tool;
    use std::sync::Arc;

    #[test]
    fn mixed_search_results_coalesce_mcp_namespaces() {
        let entries = [
            ToolSearchEntry::Mcp {
                name: "mcp__calendar__create_event".to_string(),
                info: Box::new(tool_info("calendar", "create_event", "Create events")),
            },
            ToolSearchEntry::Dynamic {
                tool: DynamicToolSpec {
                    name: "automation_update".to_string(),
                    description: "Create, update, view, or delete recurring automations."
                        .to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "mode": { "type": "string" },
                        },
                        "required": ["mode"],
                        "additionalProperties": false,
                    }),
                    defer_loading: true,
                },
            },
            ToolSearchEntry::Mcp {
                name: "mcp__calendar__list_events".to_string(),
                info: Box::new(tool_info("calendar", "list_events", "List events")),
            },
        ];

        let tools =
            search_output_tools(entries.iter()).expect("mixed search output should serialize");

        assert_eq!(
            tools,
            vec![
                ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
                    name: "mcp__calendar__".to_string(),
                    description: "Tools from the calendar MCP server.".to_string(),
                    tools: vec![
                        ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                            name: "create_event".to_string(),
                            description: "Create events desktop tool".to_string(),
                            strict: false,
                            defer_loading: Some(true),
                            parameters: codex_tools::JsonSchema::object(
                                Default::default(),
                                /*required*/ None,
                                Some(false.into()),
                            ),
                            output_schema: None,
                        }),
                        ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                            name: "list_events".to_string(),
                            description: "List events desktop tool".to_string(),
                            strict: false,
                            defer_loading: Some(true),
                            parameters: codex_tools::JsonSchema::object(
                                Default::default(),
                                /*required*/ None,
                                Some(false.into()),
                            ),
                            output_schema: None,
                        }),
                    ],
                }),
                ToolSearchOutputTool::Function(ResponsesApiTool {
                    name: "automation_update".to_string(),
                    description: "Create, update, view, or delete recurring automations."
                        .to_string(),
                    strict: false,
                    defer_loading: Some(true),
                    parameters: codex_tools::JsonSchema::object(
                        std::collections::BTreeMap::from([(
                            "mode".to_string(),
                            codex_tools::JsonSchema::string(None),
                        )]),
                        Some(vec!["mode".to_string()]),
                        Some(false.into()),
                    ),
                    output_schema: None,
                }),
            ],
        );
    }

    #[test]
    fn computer_use_tool_search_uses_larger_limit() {
        let handler = ToolSearchHandler::new(
            numbered_tools(
                COMPUTER_USE_MCP_SERVER_NAME,
                "computer use",
                /*count*/ 100,
            ),
            Vec::new(),
        );

        let results = handler.search_result_entries(
            "computer use",
            TOOL_SEARCH_DEFAULT_LIMIT,
            /*use_default_limit*/ true,
        );

        assert_eq!(results.len(), COMPUTER_USE_TOOL_SEARCH_LIMIT);
        assert!(
            results
                .iter()
                .all(|entry| entry.mcp_server_name() == Some(COMPUTER_USE_MCP_SERVER_NAME))
        );

        let explicit_results = handler.search_result_entries(
            "computer use",
            /*limit*/ 100,
            /*use_default_limit*/ false,
        );

        assert_eq!(explicit_results.len(), 100);
    }

    #[test]
    fn non_computer_use_query_keeps_default_limit_with_computer_use_tools_installed() {
        let mut tools = numbered_tools(
            COMPUTER_USE_MCP_SERVER_NAME,
            "computer use",
            /*count*/ 100,
        );
        tools.extend(numbered_tools(
            "other-server",
            "calendar",
            /*count*/ 100,
        ));
        let handler = ToolSearchHandler::new(tools, Vec::new());

        let results = handler.search_result_entries(
            "calendar",
            TOOL_SEARCH_DEFAULT_LIMIT,
            /*use_default_limit*/ true,
        );

        assert_eq!(results.len(), TOOL_SEARCH_DEFAULT_LIMIT);
        assert!(
            results
                .iter()
                .all(|entry| entry.mcp_server_name() == Some("other-server"))
        );

        let explicit_results = handler.search_result_entries(
            "calendar", /*limit*/ 100, /*use_default_limit*/ false,
        );

        assert_eq!(explicit_results.len(), 100);
    }

    #[test]
    fn expanded_search_keeps_non_computer_use_servers_at_default_limit() {
        let mut tools = numbered_tools(
            COMPUTER_USE_MCP_SERVER_NAME,
            "computer use",
            /*count*/ 100,
        );
        tools.extend(numbered_tools(
            "other-server",
            "computer use",
            /*count*/ 100,
        ));
        let handler = ToolSearchHandler::new(tools, Vec::new());

        let results = handler.search_result_entries(
            "computer use",
            TOOL_SEARCH_DEFAULT_LIMIT,
            /*use_default_limit*/ true,
        );

        assert!(
            count_results_for_server(&results, COMPUTER_USE_MCP_SERVER_NAME)
                <= COMPUTER_USE_TOOL_SEARCH_LIMIT
        );
        assert!(count_results_for_server(&results, "other-server") <= TOOL_SEARCH_DEFAULT_LIMIT);
    }

    fn numbered_tools(
        server_name: &str,
        description_prefix: &str,
        count: usize,
    ) -> std::collections::HashMap<String, ToolInfo> {
        (0..count)
            .map(|index| {
                let tool_name = format!("tool_{index:03}");
                (
                    format!("mcp__{server_name}__{tool_name}"),
                    tool_info(server_name, &tool_name, description_prefix),
                )
            })
            .collect()
    }

    fn tool_info(server_name: &str, tool_name: &str, description_prefix: &str) -> ToolInfo {
        ToolInfo {
            server_name: server_name.to_string(),
            callable_name: tool_name.to_string(),
            callable_namespace: format!("mcp__{server_name}__"),
            server_instructions: None,
            tool: Tool {
                name: tool_name.to_string().into(),
                title: None,
                description: Some(format!("{description_prefix} desktop tool").into()),
                input_schema: Arc::new(rmcp::model::object(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }))),
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

    fn count_results_for_server(results: &[&ToolSearchEntry], server_name: &str) -> usize {
        results
            .iter()
            .filter(|entry| entry.mcp_server_name() == Some(server_name))
            .count()
    }
}
