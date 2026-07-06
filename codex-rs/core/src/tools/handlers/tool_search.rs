use crate::function_tool::FunctionCallError;
use crate::tool_search_debug::ToolSearchDebugResult;
use crate::tool_search_debug::ToolSearchDebugResultEntry;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::ToolSearchOutput;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::tool_search_spec::create_tool_search_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use bm25::Document;
use bm25::Language;
use bm25::SearchEngine;
use bm25::SearchEngineBuilder;
use codex_tools::LoadableToolSpec;
use codex_tools::TOOL_SEARCH_DEFAULT_LIMIT;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolName;
use codex_tools::ToolSearchEntry;
use codex_tools::ToolSearchInfo;
use codex_tools::ToolSpec;
use codex_tools::coalesce_loadable_tool_specs;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::instrument;

pub struct ToolSearchHandler {
    search_infos: Vec<ToolSearchInfo>,
    spec: ToolSpec,
    search_engine: SearchEngine<usize>,
}

#[derive(Default)]
pub(crate) struct ToolSearchHandlerCache {
    cached: Mutex<Option<Arc<ToolSearchHandler>>>,
}

impl ToolSearchHandlerCache {
    #[instrument(level = "trace", skip_all, fields(search_info_count = search_infos.len()))]
    pub(crate) fn get_or_build(&self, search_infos: Vec<ToolSearchInfo>) -> Arc<ToolSearchHandler> {
        {
            let cached = self.cached();
            if let Some(cached) = cached.as_ref()
                && cached.search_infos == search_infos
            {
                return Arc::clone(cached);
            }
        }

        let handler = Arc::new(ToolSearchHandler::new(search_infos));
        let mut cached = self.cached();
        if let Some(cached) = cached.as_ref()
            && cached.search_infos == handler.search_infos
        {
            return Arc::clone(cached);
        }

        *cached = Some(Arc::clone(&handler));
        handler
    }

    fn cached(&self) -> std::sync::MutexGuard<'_, Option<Arc<ToolSearchHandler>>> {
        match self.cached.lock() {
            Ok(cached) => cached,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl ToolSearchHandler {
    #[instrument(
        level = "trace",
        skip_all,
        fields(search_info_count = search_infos.len())
    )]
    pub(crate) fn new(search_infos: Vec<ToolSearchInfo>) -> Self {
        let search_source_infos = search_infos
            .iter()
            .filter_map(|search_info| search_info.source_info.clone())
            .collect::<Vec<_>>();
        let spec = create_tool_search_tool(&search_source_infos, TOOL_SEARCH_DEFAULT_LIMIT);
        let documents: Vec<Document<usize>> = search_infos
            .iter()
            .map(|search_info| search_info.entry.search_text.clone())
            .enumerate()
            .map(|(idx, search_text)| Document::new(idx, search_text))
            .collect();
        let search_engine =
            SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();

        Self {
            search_infos,
            spec,
            search_engine,
        }
    }
}

impl ToolExecutor<ToolInvocation> for ToolSearchHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(TOOL_SEARCH_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(invocation))
    }
}

impl ToolSearchHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
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
        let limit = args.limit.unwrap_or(TOOL_SEARCH_DEFAULT_LIMIT);

        if limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        if self.search_infos.is_empty() {
            return Ok(boxed_tool_output(ToolSearchOutput { tools: Vec::new() }));
        }

        let tools = self.search(query, limit)?;

        Ok(boxed_tool_output(ToolSearchOutput { tools }))
    }
}

impl CoreToolRuntime for ToolSearchHandler {
    fn search_tool_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Option<ToolSearchDebugResult>, FunctionCallError> {
        self.search_with_debug(query, limit).map(Some)
    }
}

impl ToolSearchHandler {
    fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LoadableToolSpec>, FunctionCallError> {
        let results = self
            .search_engine
            .search(query, limit)
            .into_iter()
            .map(|result| result.document.id)
            .filter_map(|id| self.search_infos.get(id))
            .map(|search_info| &search_info.entry);
        self.search_output_tools(results)
    }

    fn search_output_tools<'a>(
        &self,
        results: impl IntoIterator<Item = &'a ToolSearchEntry>,
    ) -> Result<Vec<LoadableToolSpec>, FunctionCallError> {
        Ok(coalesce_loadable_tool_specs(
            results.into_iter().map(|entry| entry.output.clone()),
        ))
    }

    fn search_with_debug(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<ToolSearchDebugResult, FunctionCallError> {
        let results = self.search_engine.search(query, limit);
        let matching_tool_count = self.search_engine.search(query, None).len();
        let effective_limit = results.len();
        let result_entries = results
            .iter()
            .enumerate()
            .filter_map(|(rank, result)| {
                let search_info = self.search_infos.get(result.document.id)?;
                Some(ToolSearchDebugResultEntry {
                    rank: rank + 1,
                    index: result.document.id,
                    score: result.score.is_finite().then_some(result.score),
                    source_name: search_info
                        .source_info
                        .as_ref()
                        .map(|source| source.name.clone()),
                    source_description: search_info
                        .source_info
                        .as_ref()
                        .and_then(|source| source.description.clone()),
                    searchable_text: search_info.entry.search_text.clone(),
                    tools: vec![search_info.entry.output.clone()],
                })
            })
            .collect::<Vec<_>>();
        let tools = self.search_output_tools(results.iter().filter_map(|result| {
            self.search_infos
                .get(result.document.id)
                .map(|search_info| &search_info.entry)
        }))?;

        Ok(ToolSearchDebugResult {
            indexed_tool_count: self.search_infos.len(),
            matching_tool_count,
            requested_limit: limit,
            effective_limit,
            top_k_truncated: matching_tool_count > limit,
            tools,
            results: result_entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::handlers::DynamicToolHandler;
    use crate::tools::handlers::McpHandler;
    use codex_mcp::ToolInfo;
    use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
    use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
    use codex_tools::ResponsesApiNamespace;
    use codex_tools::ResponsesApiNamespaceTool;
    use codex_tools::ResponsesApiTool;
    use pretty_assertions::assert_eq;
    use rmcp::model::Tool;
    use std::sync::Arc;

    #[test]
    fn cache_reuses_handler_for_identical_search_infos_and_rebuilds_for_changes() {
        let cache = ToolSearchHandlerCache::default();
        let search_infos = vec![
            McpHandler::new(tool_info("calendar", "create_event", "Create events"))
                .expect("MCP tool should convert")
                .search_info()
                .expect("MCP handler should return search info"),
        ];

        let first = cache.get_or_build(search_infos.clone());
        let second = cache.get_or_build(search_infos.clone());
        assert!(Arc::ptr_eq(&first, &second));

        let mut changed_search_infos = search_infos;
        changed_search_infos[0]
            .entry
            .search_text
            .push_str(" changed");
        let changed = cache.get_or_build(changed_search_infos);
        assert!(!Arc::ptr_eq(&first, &changed));
    }

    #[test]
    fn mixed_search_results_coalesce_mcp_namespaces() {
        let dynamic_namespace = DynamicToolNamespaceSpec {
            name: "codex_app".to_string(),
            description: "Tools in the codex_app namespace.".to_string(),
            tools: Vec::new(),
        };
        let dynamic_tools = [DynamicToolFunctionSpec {
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
        let mcp_tools = [
            tool_info("calendar", "create_event", "Create events"),
            tool_info("calendar", "list_events", "List events"),
        ];
        let mut search_infos = mcp_tools
            .iter()
            .map(|tool| {
                McpHandler::new(tool.clone())
                    .expect("MCP tool should convert")
                    .search_info()
                    .expect("MCP handler should return search info")
            })
            .collect::<Vec<_>>();
        search_infos.extend(dynamic_tools.iter().map(|tool| {
            DynamicToolHandler::new_in_namespace(&dynamic_namespace, tool)
                .expect("dynamic tool should convert")
                .search_info()
                .expect("dynamic handler should return search info")
        }));
        let handler = ToolSearchHandler::new(search_infos);
        let results = [
            &handler.search_infos[0].entry,
            &handler.search_infos[2].entry,
            &handler.search_infos[1].entry,
        ];

        let tools = handler
            .search_output_tools(results)
            .expect("mixed search output should serialize");

        assert_eq!(
            tools,
            vec![
                LoadableToolSpec::Namespace(ResponsesApiNamespace {
                    name: "mcp__calendar".to_string(),
                    description: "Tools in the mcp__calendar namespace.".to_string(),
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
                LoadableToolSpec::Namespace(ResponsesApiNamespace {
                    name: "codex_app".to_string(),
                    description: "Tools in the codex_app namespace.".to_string(),
                    tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
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
                    })],
                }),
            ],
        );
    }

    #[test]
    fn debug_search_returns_current_tool_search_output_and_truncation_metadata() {
        let search_infos = (0..10)
            .map(|index| {
                McpHandler::new(tool_info(
                    "calendar",
                    &format!("list_events_{index}"),
                    "Calendar events",
                ))
                .expect("MCP tool should convert")
                .search_info()
                .expect("MCP handler should return search info")
            })
            .collect::<Vec<_>>();
        let handler = ToolSearchHandler::new(search_infos);

        let debug = handler
            .search_with_debug("Calendar events", 8)
            .expect("debug search should succeed");
        assert_eq!(debug.indexed_tool_count, 10);
        assert_eq!(debug.matching_tool_count, 10);
        assert_eq!(debug.requested_limit, 8);
        assert_eq!(debug.effective_limit, 8);
        assert!(debug.top_k_truncated);
        assert_eq!(debug.results.len(), 8);
        assert!(debug.results.iter().all(|result| result.score.is_some()));
        assert_eq!(debug.tools.len(), 1);
        assert_eq!(canonical_names(&debug.tools).len(), 8);
        assert_eq!(
            canonical_names(&debug.tools),
            debug
                .results
                .iter()
                .flat_map(|result| canonical_names(&result.tools))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn debug_search_recovers_named_tool_with_runtime_query() {
        let search_infos = vec![
            McpHandler::new(tool_info("calendar", "list_events", "Calendar events"))
                .expect("MCP tool should convert")
                .search_info()
                .expect("MCP handler should return search info"),
            McpHandler::new(tool_info(
                "openai_topics",
                "list_topics",
                "List OpenAI topics",
            ))
            .expect("MCP tool should convert")
            .search_info()
            .expect("MCP handler should return search info"),
        ];
        let handler = ToolSearchHandler::new(search_infos);

        let debug = handler
            .search_with_debug("openai_topics list_topics", 8)
            .expect("debug search should succeed");

        assert_eq!(
            canonical_names(&debug.results[0].tools),
            vec!["mcp__openai_topics.list_topics"]
        );
        assert_eq!(debug.results[0].rank, 1);
    }

    #[test]
    fn debug_search_exposes_searchable_text_without_tool_arguments_or_results() {
        let search_infos = vec![
            McpHandler::new(tool_info_with_schema(
                "secrets",
                "lookup",
                "Lookup metadata",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "api_token": { "type": "string" }
                    },
                    "additionalProperties": false,
                }),
            ))
            .expect("MCP tool should convert")
            .search_info()
            .expect("MCP handler should return search info"),
        ];
        let handler = ToolSearchHandler::new(search_infos);

        let debug = handler
            .search_with_debug("api_token", 8)
            .expect("debug search should succeed");
        let serialized = format!("{debug:?}");

        assert!(debug.results[0].searchable_text.contains("api_token"));
        assert!(!serialized.contains("arguments:"));
        assert!(!serialized.contains("tool_input"));
        assert!(!serialized.contains("tool_response"));
    }

    fn tool_info(server_name: &str, tool_name: &str, description_prefix: &str) -> ToolInfo {
        tool_info_with_schema(
            server_name,
            tool_name,
            description_prefix,
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        )
    }

    fn tool_info_with_schema(
        server_name: &str,
        tool_name: &str,
        description_prefix: &str,
        input_schema: serde_json::Value,
    ) -> ToolInfo {
        ToolInfo {
            server_name: server_name.to_string(),
            supports_parallel_tool_calls: false,
            server_origin: None,
            callable_name: tool_name.to_string(),
            callable_namespace: format!("mcp__{server_name}"),
            namespace_description: None,
            tool: Tool::new(
                tool_name.to_string(),
                format!("{description_prefix} desktop tool"),
                Arc::new(rmcp::model::object(input_schema)),
            ),
            connector_id: None,
            connector_name: None,
            plugin_display_names: Vec::new(),
        }
    }

    fn canonical_names(tools: &[LoadableToolSpec]) -> Vec<String> {
        tools
            .iter()
            .flat_map(|tool| match tool {
                LoadableToolSpec::Function(tool) => vec![tool.name.clone()],
                LoadableToolSpec::Namespace(namespace) => namespace
                    .tools
                    .iter()
                    .map(|tool| {
                        let ResponsesApiNamespaceTool::Function(tool) = tool;
                        format!("{}.{}", namespace.name, tool.name)
                    })
                    .collect(),
            })
            .collect()
    }
}
