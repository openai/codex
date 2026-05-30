use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::ToolSearchOutput;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::McpHandler;
use crate::tools::handlers::tool_search_spec::create_tool_search_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::LazyToolRegistry;
use crate::tools::registry::ToolExecutor;
use crate::tools::tool_search_entry::ToolSearchEntry;
use crate::tools::tool_search_entry::ToolSearchInfo;
use bm25::Document;
use bm25::Language;
use bm25::SearchEngine;
use bm25::SearchEngineBuilder;
use codex_tools::LoadableToolSpec;
use codex_tools::TOOL_SEARCH_DEFAULT_LIMIT;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolName;
use codex_tools::ToolSearchSourceInfo;
use codex_tools::ToolSpec;
use codex_tools::coalesce_loadable_tool_specs;
use futures::future::BoxFuture;
use std::sync::Arc;
use tracing::warn;

pub(crate) type LazyMcpToolSearchLoader =
    Arc<dyn Fn() -> BoxFuture<'static, Vec<codex_mcp::ToolInfo>> + Send + Sync>;

#[derive(Clone)]
pub(crate) struct LazyMcpToolSearchLoaders {
    pub(crate) available: LazyMcpToolSearchLoader,
    pub(crate) pending: LazyMcpToolSearchLoader,
}

pub struct ToolSearchHandler {
    entries: Vec<ToolSearchEntry>,
    search_source_infos: Vec<ToolSearchSourceInfo>,
    search_engine: SearchEngine<usize>,
    lazy_mcp_tools: Option<LazyMcpToolSearchLoaders>,
    lazy_tool_registry: LazyToolRegistry,
}

impl ToolSearchHandler {
    #[cfg(test)]
    pub(crate) fn new(search_infos: Vec<ToolSearchInfo>) -> Self {
        Self::new_with_lazy_mcp_tools(
            search_infos,
            /*reloaded_mcp_search_infos*/ Vec::new(),
            /*lazy_mcp_tools*/ None,
            LazyToolRegistry::default(),
        )
    }

    pub(crate) fn new_with_lazy_mcp_tools(
        search_infos: Vec<ToolSearchInfo>,
        reloaded_mcp_search_infos: Vec<ToolSearchInfo>,
        lazy_mcp_tools: Option<LazyMcpToolSearchLoaders>,
        lazy_tool_registry: LazyToolRegistry,
    ) -> Self {
        let mut entries = Vec::with_capacity(search_infos.len());
        let mut search_source_infos = Vec::new();
        for search_info in search_infos {
            entries.push(search_info.entry);
            if let Some(source_info) = search_info.source_info {
                search_source_infos.push(source_info);
            }
        }
        search_source_infos.extend(
            reloaded_mcp_search_infos
                .into_iter()
                .filter_map(|search_info| search_info.source_info),
        );
        if lazy_mcp_tools.is_some() {
            search_source_infos.push(ToolSearchSourceInfo {
                name: "MCP tools".to_string(),
                description: Some(
                    "Tools from MCP servers still starting in the background can be loaded on demand."
                        .to_string(),
                ),
            });
        }
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
            search_source_infos,
            search_engine,
            lazy_mcp_tools,
            lazy_tool_registry,
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for ToolSearchHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(TOOL_SEARCH_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_tool_search_tool(&self.search_source_infos, TOOL_SEARCH_DEFAULT_LIMIT)
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    async fn handle(
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

        let tools = self.search_with_lazy_mcp_fallback(query, limit).await?;

        Ok(boxed_tool_output(ToolSearchOutput { tools }))
    }
}

impl CoreToolRuntime for ToolSearchHandler {}

impl ToolSearchHandler {
    async fn search_with_lazy_mcp_fallback(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LoadableToolSpec>, FunctionCallError> {
        let tools = self.search(query, limit)?;
        let Some(load_mcp_tools) = &self.lazy_mcp_tools else {
            return Ok(tools);
        };
        let available_mcp_tools = (load_mcp_tools.available)().await;
        if !available_mcp_tools.is_empty() {
            let refreshed_tools =
                self.search_with_lazy_mcp_tools(query, limit, available_mcp_tools)?;
            if !refreshed_tools.is_empty() {
                return Ok(refreshed_tools);
            }
        }
        if !tools.is_empty() {
            return Ok(tools);
        }

        self.search_with_lazy_mcp_tools(query, limit, (load_mcp_tools.pending)().await)
    }

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
            .filter_map(|id| self.entries.get(id));
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

    fn search_with_lazy_mcp_tools(
        &self,
        query: &str,
        limit: usize,
        mcp_tools: Vec<codex_mcp::ToolInfo>,
    ) -> Result<Vec<LoadableToolSpec>, FunctionCallError> {
        struct SearchCandidate {
            entry: ToolSearchEntry,
            lazy_handler: Option<Arc<McpHandler>>,
        }

        let mut candidates = self
            .entries
            .iter()
            .cloned()
            .map(|entry| SearchCandidate {
                entry,
                lazy_handler: None,
            })
            .collect::<Vec<_>>();
        candidates.extend(mcp_tools.into_iter().filter_map(|tool| {
            let handler = match McpHandler::new(tool) {
                Ok(handler) => handler,
                Err(err) => {
                    warn!("Skipping lazily loaded MCP tool with invalid spec: {err}");
                    return None;
                }
            };
            let search_info = handler.search_info()?;
            let tool_name = handler.tool_name();
            if !self.lazy_tool_registry.can_register(&tool_name) {
                warn!(
                    "Skipping lazily loaded MCP tool `{tool_name}` shadowed by a registered tool"
                );
                return None;
            }
            Some(SearchCandidate {
                entry: search_info.entry,
                lazy_handler: Some(Arc::new(handler)),
            })
        }));
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let documents = candidates
            .iter()
            .map(|candidate| candidate.entry.search_text.clone())
            .enumerate()
            .map(|(idx, search_text)| Document::new(idx, search_text))
            .collect::<Vec<_>>();
        let search_engine =
            SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();
        let results = search_engine
            .search(query, limit)
            .into_iter()
            .map(|result| result.document.id)
            .filter_map(|id| candidates.get(id))
            .filter_map(|candidate| {
                if let Some(handler) = &candidate.lazy_handler
                    && !self.lazy_tool_registry.register(handler.clone())
                {
                    warn!(
                        "Skipping lazily loaded MCP tool `{}` shadowed by a registered tool",
                        handler.tool_name()
                    );
                    return None;
                }
                Some(&candidate.entry)
            });
        self.search_output_tools(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::handlers::DynamicToolHandler;
    use codex_mcp::ToolInfo;
    use codex_protocol::dynamic_tools::DynamicToolSpec;
    use codex_tools::ResponsesApiNamespace;
    use codex_tools::ResponsesApiNamespaceTool;
    use codex_tools::ResponsesApiTool;
    use pretty_assertions::assert_eq;
    use rmcp::model::Tool;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    #[test]
    fn mixed_search_results_coalesce_mcp_namespaces() {
        let dynamic_tools = [DynamicToolSpec {
            namespace: Some("codex_app".to_string()),
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
            DynamicToolHandler::new(tool)
                .expect("dynamic tool should convert")
                .search_info()
                .expect("dynamic handler should return search info")
        }));
        let handler = ToolSearchHandler::new(search_infos);
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

    fn tool_info(server_name: &str, tool_name: &str, description_prefix: &str) -> ToolInfo {
        tool_info_with_namespace(
            server_name,
            format!("mcp__{server_name}"),
            tool_name,
            description_prefix,
        )
    }

    fn tool_info_with_namespace(
        server_name: &str,
        callable_namespace: String,
        tool_name: &str,
        description_prefix: &str,
    ) -> ToolInfo {
        ToolInfo {
            server_name: server_name.to_string(),
            supports_parallel_tool_calls: false,
            server_origin: None,
            callable_name: tool_name.to_string(),
            callable_namespace,
            namespace_description: None,
            tool: Tool::new(
                tool_name.to_string(),
                format!("{description_prefix} desktop tool"),
                Arc::new(rmcp::model::object(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }))),
            ),
            connector_id: None,
            connector_name: None,
            plugin_display_names: Vec::new(),
        }
    }

    #[test]
    fn lazy_mcp_search_registers_only_returned_tools_for_dispatch() {
        let lazy_tool_registry = LazyToolRegistry::default();
        let registry = crate::tools::registry::ToolRegistry::from_tools_with_lazy_registry(
            Vec::<Arc<dyn CoreToolRuntime>>::new(),
            lazy_tool_registry.clone(),
        );
        let handler = ToolSearchHandler::new_with_lazy_mcp_tools(
            Vec::new(),
            /*reloaded_mcp_search_infos*/ Vec::new(),
            /*lazy_mcp_tools*/ None,
            lazy_tool_registry,
        );

        let tools = handler
            .search_with_lazy_mcp_tools(
                "calendar",
                /*limit*/ 1,
                vec![
                    tool_info("calendar", "create_event", "Create events"),
                    tool_info("mail", "draft_message", "Draft mail"),
                ],
            )
            .expect("lazy MCP search should produce a tool spec");

        assert_eq!(tools.len(), 1);
        assert_eq!(
            registry.tool_names_for_test(),
            vec![ToolName::namespaced("mcp__calendar", "create_event")]
        );
    }

    #[tokio::test]
    async fn lazy_mcp_search_does_not_load_pending_tools_when_existing_result_matches() {
        let dynamic_tool = DynamicToolHandler::new(&DynamicToolSpec {
            namespace: Some("codex_app".to_string()),
            name: "automation_update".to_string(),
            description: "Create recurring automations.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            defer_loading: true,
        })
        .expect("dynamic tool should convert");
        let search_info = dynamic_tool
            .search_info()
            .expect("dynamic handler should return search info");
        let loader_called = Arc::new(AtomicBool::new(/*v*/ false));
        let loader_called_for_future = Arc::clone(&loader_called);
        let pending_loader: LazyMcpToolSearchLoader = Arc::new(move || {
            loader_called_for_future.store(/*val*/ true, Ordering::SeqCst);
            Box::pin(async { Vec::new() })
        });
        let loaders = LazyMcpToolSearchLoaders {
            available: Arc::new(|| Box::pin(async { Vec::new() })),
            pending: pending_loader,
        };
        let handler = ToolSearchHandler::new_with_lazy_mcp_tools(
            vec![search_info],
            /*reloaded_mcp_search_infos*/ Vec::new(),
            Some(loaders),
            LazyToolRegistry::default(),
        );

        let tools = handler
            .search_with_lazy_mcp_fallback("automation", /*limit*/ 1)
            .await
            .expect("existing deferred tool search should succeed");

        assert_eq!(tools.len(), 1);
        assert!(!loader_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn lazy_mcp_search_refreshes_available_tools_when_existing_result_matches() {
        let dynamic_tool = DynamicToolHandler::new(&DynamicToolSpec {
            namespace: Some("codex_app".to_string()),
            name: "automation_update".to_string(),
            description: "Create recurring automations.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            defer_loading: true,
        })
        .expect("dynamic tool should convert");
        let search_info = dynamic_tool
            .search_info()
            .expect("dynamic handler should return search info");
        let handler = ToolSearchHandler::new_with_lazy_mcp_tools(
            vec![search_info],
            /*reloaded_mcp_search_infos*/ Vec::new(),
            Some(LazyMcpToolSearchLoaders {
                available: Arc::new(|| {
                    Box::pin(async {
                        vec![tool_info(
                            "calendar",
                            "create_event",
                            "Calendar automations",
                        )]
                    })
                }),
                pending: Arc::new(|| Box::pin(async { Vec::new() })),
            }),
            LazyToolRegistry::default(),
        );

        let tools = handler
            .search_with_lazy_mcp_fallback("calendar automation", /*limit*/ 1)
            .await
            .expect("available MCP tool refresh should succeed");

        assert_eq!(
            tools,
            vec![LoadableToolSpec::Namespace(ResponsesApiNamespace {
                name: "mcp__calendar".to_string(),
                description: "Tools in the mcp__calendar namespace.".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "create_event".to_string(),
                    description: "Calendar automations desktop tool".to_string(),
                    strict: false,
                    defer_loading: Some(true),
                    parameters: codex_tools::JsonSchema::object(
                        Default::default(),
                        /*required*/ None,
                        Some(false.into()),
                    ),
                    output_schema: None,
                })],
            })]
        );
    }

    #[test]
    fn lazy_mcp_search_advertises_background_mcp_source() {
        let loader: LazyMcpToolSearchLoader = Arc::new(|| Box::pin(async { Vec::new() }));
        let handler = ToolSearchHandler::new_with_lazy_mcp_tools(
            Vec::new(),
            /*reloaded_mcp_search_infos*/ Vec::new(),
            Some(LazyMcpToolSearchLoaders {
                available: Arc::clone(&loader),
                pending: loader,
            }),
            LazyToolRegistry::default(),
        );

        let ToolSpec::ToolSearch { description, .. } = handler.spec() else {
            panic!("expected tool_search spec");
        };
        assert!(description.contains("- MCP tools: Tools from MCP servers still starting"));
    }

    #[test]
    fn lazy_mcp_search_omits_tool_shadowed_by_static_handler() {
        let lazy_tool_registry = LazyToolRegistry::default();
        let dynamic_tool = DynamicToolHandler::new(&DynamicToolSpec {
            namespace: Some("mcp__calendar".to_string()),
            name: "create_event".to_string(),
            description: "Static dynamic handler".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            defer_loading: true,
        })
        .expect("dynamic tool should convert");
        let _registry = crate::tools::registry::ToolRegistry::from_tools_with_lazy_registry(
            vec![Arc::new(dynamic_tool) as Arc<dyn CoreToolRuntime>],
            lazy_tool_registry.clone(),
        );
        let handler = ToolSearchHandler::new_with_lazy_mcp_tools(
            Vec::new(),
            /*reloaded_mcp_search_infos*/ Vec::new(),
            /*lazy_mcp_tools*/ None,
            lazy_tool_registry,
        );

        let tools = handler
            .search_with_lazy_mcp_tools(
                "calendar",
                /*limit*/ 1,
                vec![tool_info("calendar", "create_event", "Create events")],
            )
            .expect("shadowed lazy MCP search should succeed");

        assert_eq!(tools, Vec::new());
    }

    #[test]
    fn lazy_mcp_search_keeps_equivalent_static_mcp_handler() {
        let lazy_tool_registry = LazyToolRegistry::default();
        let tool_name = ToolName::namespaced("mcp__calendar", "create_event");
        lazy_tool_registry.allow_equivalent_static_mcp_tool(tool_name);
        let static_mcp_handler =
            McpHandler::new(tool_info("calendar", "create_event", "Create events"))
                .expect("static MCP tool should convert");
        let _registry = crate::tools::registry::ToolRegistry::from_tools_with_lazy_registry(
            vec![Arc::new(static_mcp_handler) as Arc<dyn CoreToolRuntime>],
            lazy_tool_registry.clone(),
        );
        let handler = ToolSearchHandler::new_with_lazy_mcp_tools(
            Vec::new(),
            /*reloaded_mcp_search_infos*/ Vec::new(),
            /*lazy_mcp_tools*/ None,
            lazy_tool_registry,
        );

        let tools = handler
            .search_with_lazy_mcp_tools(
                "calendar",
                /*limit*/ 1,
                vec![tool_info("calendar", "create_event", "Create events")],
            )
            .expect("equivalent static MCP search should succeed");

        assert_eq!(tools.len(), 1);
    }

    #[test]
    fn lazy_mcp_search_replaces_entries_normalized_before_new_collision() {
        let initial_ready_search_info = McpHandler::new(tool_info_with_namespace(
            "foo-bar",
            "mcp__foo_bar".to_string(),
            "lookup",
            "ready unique",
        ))
        .expect("initial MCP tool should convert")
        .search_info()
        .expect("initial MCP handler should return search info");
        let handler = ToolSearchHandler::new_with_lazy_mcp_tools(
            Vec::new(),
            vec![initial_ready_search_info],
            /*lazy_mcp_tools*/ None,
            LazyToolRegistry::default(),
        );

        let tools = handler
            .search_with_lazy_mcp_tools(
                "lookup",
                /*limit*/ 2,
                vec![
                    tool_info_with_namespace(
                        "foo-bar",
                        "mcp__foo_bar_111111".to_string(),
                        "lookup",
                        "ready unique",
                    ),
                    tool_info_with_namespace(
                        "foo_bar",
                        "mcp__foo_bar_222222".to_string(),
                        "lookup",
                        "pending other",
                    ),
                ],
            )
            .expect("lazy MCP search should produce current normalized tool specs");

        assert_eq!(tools.len(), 2);
        let mut namespaces = tools
            .iter()
            .map(|tool| match tool {
                LoadableToolSpec::Namespace(namespace) => namespace.name.as_str(),
                LoadableToolSpec::Function(_) => panic!("expected MCP namespace output"),
            })
            .collect::<Vec<_>>();
        namespaces.sort_unstable();
        assert_eq!(
            namespaces,
            vec!["mcp__foo_bar_111111", "mcp__foo_bar_222222"]
        );
    }
}
