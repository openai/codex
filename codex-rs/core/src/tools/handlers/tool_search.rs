use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::ToolSearchOutput;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::tool_search_entry::ToolSearchEntry;
use bm25::Document;
use bm25::Language;
use bm25::SearchEngine;
use bm25::SearchEngineBuilder;
use codex_tools::TOOL_SEARCH_DEFAULT_LIMIT;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolSearchOutputTool;
use std::collections::HashMap;

const COMPUTER_USE_MCP_SERVER_NAME: &str = "computer-use";
const COMPUTER_USE_TOOL_SEARCH_LIMIT: usize = 20;

pub struct ToolSearchHandler {
    entries: Vec<ToolSearchEntry>,
    search_engine: SearchEngine<usize>,
}

impl ToolSearchHandler {
    pub(crate) fn new(entries: Vec<ToolSearchEntry>) -> Self {
        let documents: Vec<Document<usize>> = entries
            .iter()
            .map(|entry| entry.search_text.clone())
            .enumerate()
            .map(|(idx, search_text)| Document::new(idx, search_text))
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
        self.search_output_tools(results)
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
            .map(|result| result.document.id)
            .filter_map(|id| self.entries.get(id))
            .collect::<Vec<_>>();
        if !use_default_limit {
            return results;
        }

        if results.iter().any(|entry| {
            entry
                .limit_bucket
                .as_deref()
                .is_some_and(|bucket| bucket == COMPUTER_USE_MCP_SERVER_NAME)
        }) {
            results = self
                .search_engine
                .search(query, COMPUTER_USE_TOOL_SEARCH_LIMIT)
                .into_iter()
                .map(|result| result.document.id)
                .filter_map(|id| self.entries.get(id))
                .collect();
        }
        limit_results_by_bucket(results)
    }

    fn search_output_tools<'a>(
        &self,
        results: impl IntoIterator<Item = &'a ToolSearchEntry>,
    ) -> Result<Vec<ToolSearchOutputTool>, FunctionCallError> {
        let mut tools = Vec::new();
        // Preserve search order: group namespace children, emit standalone tools directly.
        for entry in results {
            match &entry.output {
                ToolSearchOutputTool::Function(tool) => {
                    tools.push(ToolSearchOutputTool::Function(tool.clone()));
                }
                ToolSearchOutputTool::Namespace(namespace) => {
                    if let Some(output) = tools.iter_mut().find_map(|tool| match tool {
                        ToolSearchOutputTool::Namespace(output)
                            if output.name == namespace.name =>
                        {
                            Some(output)
                        }
                        ToolSearchOutputTool::Namespace(_) | ToolSearchOutputTool::Function(_) => {
                            None
                        }
                    }) {
                        output.tools.extend(namespace.tools.clone());
                    } else {
                        tools.push(ToolSearchOutputTool::Namespace(namespace.clone()));
                    }
                }
            }
        }

        Ok(tools)
    }
}

fn limit_results_by_bucket(results: Vec<&ToolSearchEntry>) -> Vec<&ToolSearchEntry> {
    results
        .into_iter()
        .scan(HashMap::<&str, usize>::new(), |counts, result| {
            let Some(bucket) = result.limit_bucket.as_deref() else {
                return Some(Some(result));
            };
            let count = counts.entry(bucket).or_default();
            if *count >= default_limit_for_bucket(bucket) {
                Some(None)
            } else {
                *count += 1;
                Some(Some(result))
            }
        })
        .flatten()
        .collect()
}

fn default_limit_for_bucket(bucket: &str) -> usize {
    if bucket == COMPUTER_USE_MCP_SERVER_NAME {
        COMPUTER_USE_TOOL_SEARCH_LIMIT
    } else {
        TOOL_SEARCH_DEFAULT_LIMIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::tool_search_entry::build_tool_search_entries;
    use codex_mcp::ToolInfo;
    use codex_protocol::dynamic_tools::DynamicToolSpec;
    use codex_tools::ResponsesApiNamespace;
    use codex_tools::ResponsesApiNamespaceTool;
    use codex_tools::ResponsesApiTool;
    use pretty_assertions::assert_eq;
    use rmcp::model::Tool;
    use std::sync::Arc;

    #[test]
    fn mixed_search_results_coalesce_mcp_namespaces() {
        let dynamic_tools = vec![DynamicToolSpec {
            name: "automation_update".to_string(),
            description: "Create, update, view, or delete recurring automations.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "mode": { "type": "string" },
                },
                "required": ["mode"],
                "additionalProperties": false,
            }),
            defer_loading: true,
        }];
        let handler = handler_from_tools(
            Some(&std::collections::HashMap::from([
                (
                    "mcp__calendar__create_event".to_string(),
                    tool_info("calendar", "create_event", "Create events"),
                ),
                (
                    "mcp__calendar__list_events".to_string(),
                    tool_info("calendar", "list_events", "List events"),
                ),
            ])),
            &dynamic_tools,
        );
        let results = [
            &handler.entries[0],
            &handler.entries[2],
            &handler.entries[1],
        ];

        let tools = handler
            .search_output_tools(results)
            .expect("mixed search output should serialize");

        assert_eq!(
            tools,
            vec![
                ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
                    name: "mcp__calendar__".to_string(),
                    description: "Tools in the mcp__calendar__ namespace.".to_string(),
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
                            codex_tools::JsonSchema::string(/*description*/ None),
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
        let tools = numbered_tools(
            COMPUTER_USE_MCP_SERVER_NAME,
            "computer use",
            /*count*/ 100,
        );
        let handler = handler_from_tools(Some(&tools), &[]);

        let results = handler.search_result_entries(
            "computer use",
            TOOL_SEARCH_DEFAULT_LIMIT,
            /*use_default_limit*/ true,
        );

        assert_eq!(results.len(), COMPUTER_USE_TOOL_SEARCH_LIMIT);
        assert!(
            results
                .iter()
                .all(|entry| entry.limit_bucket.as_deref() == Some(COMPUTER_USE_MCP_SERVER_NAME))
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
        let handler = handler_from_tools(Some(&tools), &[]);

        let results = handler.search_result_entries(
            "calendar",
            TOOL_SEARCH_DEFAULT_LIMIT,
            /*use_default_limit*/ true,
        );

        assert_eq!(results.len(), TOOL_SEARCH_DEFAULT_LIMIT);
        assert!(
            results
                .iter()
                .all(|entry| entry.limit_bucket.as_deref() == Some("other-server"))
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
        let handler = handler_from_tools(Some(&tools), &[]);

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
            .filter(|entry| entry.limit_bucket.as_deref() == Some(server_name))
            .count()
    }

    fn handler_from_tools(
        mcp_tools: Option<&std::collections::HashMap<String, ToolInfo>>,
        dynamic_tools: &[DynamicToolSpec],
    ) -> ToolSearchHandler {
        ToolSearchHandler::new(build_tool_search_entries(mcp_tools, dynamic_tools))
    }
}
