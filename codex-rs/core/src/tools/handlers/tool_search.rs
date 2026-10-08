use crate::function_tool::FunctionCallError;
use crate::tools::code_mode::prepare_code_mode_tool_definitions;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::ToolSearchOutput;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::tool_search_spec::ToolSearchSourceListing;
use crate::tools::handlers::tool_search_spec::create_tool_search_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolRegistry;
use crate::tools::router::ToolRouter;
use bm25::Embedder;
use bm25::EmbedderBuilder;
use bm25::Language;
use bm25::Scorer;
use codex_features::Feature;
use codex_prompts::ResolvedModelMessages;
use codex_tools::IndirectNamespacePrefixes;
use codex_tools::JsonToolOutput;
use codex_tools::LoadableToolSpec;
use codex_tools::TOOL_SEARCH_DEFAULT_LIMIT;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolName;
use codex_tools::ToolSearchEntry;
use codex_tools::ToolSearchInfo;
use codex_tools::ToolSpec;
use codex_tools::coalesce_loadable_tool_specs;
use serde::Serialize;
use std::borrow::Cow;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use tracing::instrument;

#[derive(Serialize)]
struct CodeModeSearchResult {
    name: String,
    description: String,
}

pub struct ToolSearchHandler {
    search_infos: Vec<ToolSearchInfo>,
    source_listing: ToolSearchSourceListing,
    spec: ToolSpec,
    search_embedder: Embedder,
    search_scorer: Scorer<usize>,
}

#[derive(Default)]
pub(crate) struct ToolSearchHandlerCache {
    cached: Mutex<Option<CachedToolSearchHandler>>,
}

struct CachedToolSearchHandler {
    handler: Arc<ToolSearchHandler>,
    sources: Vec<ToolSearchSource>,
}

enum ToolSearchSource {
    Immutable(Weak<dyn CoreToolRuntime>),
    Dynamic(Box<ToolSearchInfo>),
}

impl ToolSearchHandlerCache {
    #[instrument(level = "trace", skip_all)]
    pub(crate) fn get_or_build(
        &self,
        registry: &ToolRegistry,
        source_listing: ToolSearchSourceListing,
    ) -> Arc<ToolSearchHandler> {
        let sources = registry
            .entries()
            .filter(|tool| tool.exposure.is_deferred())
            .filter_map(|tool| {
                if let Some(runtime) = tool.cached_runtime() {
                    Some(ToolSearchSource::Immutable(Arc::downgrade(runtime)))
                } else {
                    registry
                        .search_info(tool)
                        .map(Box::new)
                        .map(ToolSearchSource::Dynamic)
                }
            })
            .collect::<Vec<_>>();

        {
            let cached = self.cached();
            if let Some(cached) = cached.as_ref()
                && cached.handler.source_listing == source_listing
                && Self::sources_match(&cached.sources, &sources)
            {
                return Arc::clone(&cached.handler);
            }
        }

        let search_infos = sources
            .iter()
            .filter_map(|source| match source {
                ToolSearchSource::Immutable(runtime) => {
                    runtime.upgrade().and_then(|runtime| runtime.search_info())
                }
                ToolSearchSource::Dynamic(search_info) => Some(search_info.as_ref().clone()),
            })
            .collect();

        let handler = Arc::new(ToolSearchHandler::new(search_infos, source_listing));
        let mut cached = self.cached();
        if let Some(cached) = cached.as_ref()
            && cached.handler.source_listing == source_listing
            && Self::sources_match(&cached.sources, &sources)
        {
            return Arc::clone(&cached.handler);
        }
        *cached = Some(CachedToolSearchHandler {
            handler: Arc::clone(&handler),
            sources,
        });
        handler
    }

    fn sources_match(cached_sources: &[ToolSearchSource], sources: &[ToolSearchSource]) -> bool {
        cached_sources.len() == sources.len()
            && cached_sources
                .iter()
                .zip(sources)
                .all(|(cached, current)| match (cached, current) {
                    (ToolSearchSource::Immutable(cached), ToolSearchSource::Immutable(current)) => {
                        Weak::ptr_eq(cached, current)
                    }
                    (ToolSearchSource::Dynamic(cached), ToolSearchSource::Dynamic(current)) => {
                        cached == current
                    }
                    (ToolSearchSource::Immutable(_), ToolSearchSource::Dynamic(_))
                    | (ToolSearchSource::Dynamic(_), ToolSearchSource::Immutable(_)) => false,
                })
    }

    fn cached(&self) -> std::sync::MutexGuard<'_, Option<CachedToolSearchHandler>> {
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
    pub(crate) fn new(
        search_infos: Vec<ToolSearchInfo>,
        source_listing: ToolSearchSourceListing,
    ) -> Self {
        let search_source_infos = search_infos
            .iter()
            .filter_map(|search_info| search_info.source_info.clone())
            .collect::<Vec<_>>();
        let spec = create_tool_search_tool(
            &search_source_infos,
            TOOL_SEARCH_DEFAULT_LIMIT,
            source_listing,
        );
        let corpus = search_infos
            .iter()
            .map(|search_info| search_info.entry.search_text.as_str())
            .collect::<Vec<_>>();
        let search_embedder =
            EmbedderBuilder::with_fit_to_corpus(Language::English, &corpus).build();
        let mut search_scorer = Scorer::new();
        for (id, text) in corpus.into_iter().enumerate() {
            search_scorer.upsert(&id, search_embedder.embed(text));
        }

        Self {
            search_infos,
            source_listing,
            spec,
            search_embedder,
            search_scorer,
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

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(self.handle_call(invocation))
    }
}

impl ToolSearchHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            payload,
            source,
            step_context,
            ..
        } = invocation;
        let nested = matches!(source, ToolCallSource::CodeMode { .. });

        let args = match payload {
            ToolPayload::ToolSearch { arguments } => arguments,
            ToolPayload::Function { arguments } if nested => super::parse_arguments(&arguments)?,
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

        let model_messages = ResolvedModelMessages::from_model(&step_context.settings.model_info);
        let indirect_prefixes = IndirectNamespacePrefixes::new(
            model_messages.indirect_description_prefixes(),
            step_context.tool_router.mcp_namespaces(),
        )
        .map_err(|error| FunctionCallError::Fatal(error.to_string()))?;

        if nested {
            let tools = self.search_code_mode(
                query,
                limit,
                &step_context.tool_router,
                step_context
                    .turn
                    .config
                    .code_mode
                    .tool_input_schema_max_bytes,
                step_context
                    .turn
                    .config
                    .features
                    .enabled(Feature::CodeModeToolDescriptionFirst),
                &indirect_prefixes,
            );
            return Ok(boxed_tool_output(JsonToolOutput::new(serde_json::json!({
                "tools": tools,
            }))));
        }

        if self.search_infos.is_empty() {
            return Ok(boxed_tool_output(ToolSearchOutput { tools: Vec::new() }));
        }

        let mut tools = self.search(query, limit);
        indirect_prefixes.apply_search(&mut tools);

        Ok(boxed_tool_output(ToolSearchOutput { tools }))
    }
}

impl CoreToolRuntime for ToolSearchHandler {}

impl ToolSearchHandler {
    fn search_code_mode(
        &self,
        query: &str,
        limit: usize,
        router: &ToolRouter,
        code_mode_input_schema_max_bytes: Option<usize>,
        tool_description_first: bool,
        indirect_prefixes: &IndirectNamespacePrefixes<'_>,
    ) -> Vec<CodeModeSearchResult> {
        let mut tools = Vec::new();
        // Apply the exact step's nested-tool policy before limiting ranked results.
        for entry in self.search_entries(query) {
            for candidate_name in entry.tool_names() {
                let candidate_name = candidate_name.with_default_namespace();
                let name = codex_code_mode::normalize_code_mode_identifier(
                    &codex_tools::code_mode_name_for_tool_name(&candidate_name),
                );
                let Some(tool_name) =
                    router
                        .code_mode_tool_names()
                        .get(&name)
                        .filter(|tool_name| {
                            (*tool_name).clone().with_default_namespace() == candidate_name
                        })
                else {
                    continue;
                };
                let Some(tool) = router.registered_tool(tool_name) else {
                    continue;
                };
                let definitions = prepare_code_mode_tool_definitions(
                    tool.cached_runtime().map(Arc::as_ref),
                    || Cow::Owned(Arc::unwrap_or_clone(tool.spec())),
                    code_mode_input_schema_max_bytes,
                    tool_description_first,
                );
                let Some(definition) = definitions.iter().find(|definition| {
                    definition.tool_name.clone().with_default_namespace() == candidate_name
                }) else {
                    continue;
                };
                let mut description = definition.description.clone();
                indirect_prefixes
                    .apply_code_mode_description(&definition.tool_name, &mut description);
                tools.push(CodeModeSearchResult { name, description });
                if tools.len() == limit {
                    return tools;
                }
            }
        }
        tools
    }

    fn search(&self, query: &str, limit: usize) -> Vec<LoadableToolSpec> {
        coalesce_loadable_tool_specs(
            self.search_entries(query)
                .take(limit)
                .map(ToolSearchEntry::to_loadable_spec),
        )
    }

    fn search_entries(&self, query: &str) -> impl Iterator<Item = &ToolSearchEntry> + '_ {
        self.search_scorer
            .matches(&self.search_embedder.embed(query))
            .into_iter()
            .filter_map(|result| self.search_infos.get(result.id))
            .map(|search_info| &search_info.entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::handlers::DynamicToolHandler;
    use crate::tools::handlers::McpHandler;
    use crate::tools::registry::ToolExposure;
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
    fn cache_reuses_immutable_handlers_and_rebuilds_for_current_registry_changes() {
        let cache = ToolSearchHandlerCache::default();
        let mut info = tool_info("calendar", "create_event", "Create events");
        info.callable_namespace = "functions".to_string();
        let runtime: Arc<dyn CoreToolRuntime> =
            Arc::new(McpHandler::new(info.clone()).expect("MCP tool should convert"));
        let mut registry = ToolRegistry::default();
        registry.register_trusted_with_exposure(Arc::clone(&runtime), ToolExposure::Deferred);

        let first = cache.get_or_build(&registry, ToolSearchSourceListing::Include);
        let second = cache.get_or_build(&registry, ToolSearchSourceListing::Include);
        assert!(Arc::ptr_eq(&first, &second));

        let without_sources = cache.get_or_build(&registry, ToolSearchSourceListing::Omit);
        assert!(!Arc::ptr_eq(&first, &without_sources));

        let mut replacement_registry = ToolRegistry::default();
        let replacement =
            Arc::new(McpHandler::new(info).expect("replacement MCP tool should convert"));
        replacement_registry.register_trusted_with_exposure(replacement, ToolExposure::Deferred);
        let replacement = cache.get_or_build(&replacement_registry, ToolSearchSourceListing::Omit);
        assert!(!Arc::ptr_eq(&without_sources, &replacement));

        registry
            .apply_functions_namespace_function_prefixes(
                Some(&std::collections::BTreeMap::from([(
                    "create_event".to_string(),
                    "Guidance.".to_string(),
                )])),
                [ToolName::plain("create_event")],
            )
            .expect("valid function prefixes");
        let prefixed = cache.get_or_build(&registry, ToolSearchSourceListing::Include);
        let reused = cache.get_or_build(&registry, ToolSearchSourceListing::Include);
        assert!(Arc::ptr_eq(&prefixed, &reused));
        assert_eq!(
            prefixed.search_infos[0].entry.search_text,
            first.search_infos[0].entry.search_text
        );
        let mut expected = first.search("Create events", /*limit*/ 1);
        let LoadableToolSpec::Namespace(namespace) = &mut expected[0] else {
            panic!("namespace")
        };
        let ResponsesApiNamespaceTool::Function(tool) = &mut namespace.tools[0] else {
            panic!("function")
        };
        tool.description = format!("Guidance.\n\n{}", tool.description);
        assert_eq!(prefixed.search("Create events", /*limit*/ 1), expected);
        registry
            .apply_functions_namespace_function_prefixes(/*prefixes*/ None, [])
            .expect("clear prefixes");
        let cleared = cache.get_or_build(&registry, ToolSearchSourceListing::Include);
        assert_eq!(
            cleared.search("Create events", /*limit*/ 1),
            first.search("Create events", /*limit*/ 1)
        );

        let mut disabled_registry = ToolRegistry::default();
        disabled_registry.register_trusted_with_exposure(runtime, ToolExposure::Direct);
        let disabled = cache.get_or_build(&disabled_registry, ToolSearchSourceListing::Omit);
        assert!(!Arc::ptr_eq(&replacement, &disabled));
        assert!(disabled.search_infos.is_empty());
    }

    #[test]
    fn cache_rechecks_dynamic_tool_metadata_while_reusing_immutable_mcp_handlers() {
        let cache = ToolSearchHandlerCache::default();
        let mcp_runtime: Arc<dyn CoreToolRuntime> = Arc::new(
            McpHandler::new(tool_info("calendar", "create_event", "Create events"))
                .expect("MCP tool should convert"),
        );
        let mut dynamic_tool = DynamicToolFunctionSpec {
            name: "lookup".to_string(),
            description: "Search current records".to_string(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
            defer_loading: true,
        };

        let mut first_registry = ToolRegistry::default();
        first_registry
            .register_trusted_with_exposure(Arc::clone(&mcp_runtime), ToolExposure::Deferred);
        first_registry.register_external_with_exposure(
            Arc::new(DynamicToolHandler::new(&dynamic_tool).expect("dynamic tool should convert")),
            ToolExposure::Deferred,
        );
        let first = cache.get_or_build(&first_registry, ToolSearchSourceListing::Include);

        let mut equivalent_registry = ToolRegistry::default();
        equivalent_registry
            .register_trusted_with_exposure(Arc::clone(&mcp_runtime), ToolExposure::Deferred);
        equivalent_registry.register_external_with_exposure(
            Arc::new(DynamicToolHandler::new(&dynamic_tool).expect("dynamic tool should convert")),
            ToolExposure::Deferred,
        );
        let equivalent = cache.get_or_build(&equivalent_registry, ToolSearchSourceListing::Include);
        assert!(Arc::ptr_eq(&first, &equivalent));

        dynamic_tool.description = "Search refreshed records".to_string();
        let mut refreshed_registry = ToolRegistry::default();
        refreshed_registry.register_trusted_with_exposure(mcp_runtime, ToolExposure::Deferred);
        refreshed_registry.register_external_with_exposure(
            Arc::new(DynamicToolHandler::new(&dynamic_tool).expect("dynamic tool should convert")),
            ToolExposure::Deferred,
        );
        let refreshed = cache.get_or_build(&refreshed_registry, ToolSearchSourceListing::Include);
        assert!(!Arc::ptr_eq(&first, &refreshed));
        assert!(
            refreshed.search_infos[1]
                .entry
                .search_text
                .contains("refreshed")
        );
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
        let handler = ToolSearchHandler::new(search_infos, ToolSearchSourceListing::Include);
        let results = [
            &handler.search_infos[0].entry,
            &handler.search_infos[2].entry,
            &handler.search_infos[1].entry,
        ];

        let tools = coalesce_loadable_tool_specs(
            results.into_iter().map(ToolSearchEntry::to_loadable_spec),
        );

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
                Arc::new(rmcp::model::object(serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }))),
            ),
            openai_file_input_optional_fields: Default::default(),
            connector_id: None,
            connector_name: None,
            plugin_display_names: Vec::new(),
        }
    }
}
