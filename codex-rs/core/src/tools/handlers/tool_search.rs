use crate::function_tool::FunctionCallError;
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ToolSearchIdentityTier {
    None,
    SourceAlias,
    ToolAlias,
    CanonicalAlias,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ToolSearchScore {
    pub(crate) index: usize,
    pub(crate) identity_tier: ToolSearchIdentityTier,
    pub(crate) bm25_score: f32,
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

impl CoreToolRuntime for ToolSearchHandler {}

impl ToolSearchHandler {
    fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LoadableToolSpec>, FunctionCallError> {
        let results = self.search_ranked(query).into_iter().take(limit);
        self.search_output_tools(
            results
                .map(|result| result.index)
                .filter_map(|id| self.search_infos.get(id))
                .map(|search_info| &search_info.entry),
        )
    }

    pub(crate) fn search_ranked(&self, query: &str) -> Vec<ToolSearchScore> {
        let mut bm25_scores = vec![None; self.search_infos.len()];
        for result in self.search_engine.search(query, None) {
            if let Some(score) = bm25_scores.get_mut(result.document.id) {
                *score = Some(result.score);
            }
        }

        let mut results = self
            .search_infos
            .iter()
            .enumerate()
            .filter_map(|(index, search_info)| {
                let identity_tier = identity_tier(query, &search_info.entry);
                let bm25_score = bm25_scores[index].unwrap_or(0.0);
                (identity_tier != ToolSearchIdentityTier::None || bm25_scores[index].is_some())
                    .then_some(ToolSearchScore {
                        index,
                        identity_tier,
                        bm25_score,
                    })
            })
            .collect::<Vec<_>>();

        results.sort_by(|left, right| {
            right
                .identity_tier
                .cmp(&left.identity_tier)
                .then_with(|| {
                    right
                        .bm25_score
                        .partial_cmp(&left.bm25_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| left.index.cmp(&right.index))
        });
        results
    }

    fn search_output_tools<'a>(
        &self,
        results: impl IntoIterator<Item = &'a ToolSearchEntry>,
    ) -> Result<Vec<LoadableToolSpec>, FunctionCallError> {
        Ok(coalesce_loadable_tool_specs(
            results.into_iter().map(|entry| entry.output.clone()),
        ))
    }
}

fn identity_tier(query: &str, entry: &ToolSearchEntry) -> ToolSearchIdentityTier {
    let query = normalized_identifier(query);
    if query.is_empty() {
        return ToolSearchIdentityTier::None;
    }

    if has_alias_match(&entry.identity.canonical_aliases, &query) {
        return ToolSearchIdentityTier::CanonicalAlias;
    }
    if has_alias_match(&entry.identity.tool_aliases, &query) {
        return ToolSearchIdentityTier::ToolAlias;
    }
    if has_alias_match(&entry.identity.source_aliases, &query) {
        return ToolSearchIdentityTier::SourceAlias;
    }
    ToolSearchIdentityTier::None
}

fn has_alias_match(aliases: &[String], normalized_query: &str) -> bool {
    aliases
        .iter()
        .any(|alias| normalized_identifier(alias) == normalized_query)
}

fn normalized_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
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
    use codex_tools::ToolSearchSourceInfo;
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
    fn identity_match_enters_limited_results_ahead_of_description_distractors() {
        let mut search_infos = (0..10)
            .map(|index| {
                namespace_search_info(
                    &format!("distractor_{index}"),
                    "semantic_match",
                    "OpenAI Topics list topics semantic overlap",
                    "OpenAI Topics list topics semantic overlap",
                    /*source_info*/ None,
                )
            })
            .collect::<Vec<_>>();
        search_infos.push(namespace_search_info(
            "openai_topics",
            "list_topics",
            "List OpenAI topic metadata.",
            "metadata",
            Some(ToolSearchSourceInfo {
                name: "OpenAI Topics".to_string(),
                description: None,
            }),
        ));
        let handler = ToolSearchHandler::new(search_infos);

        let tools = handler
            .search("OpenAI Topics list topics", TOOL_SEARCH_DEFAULT_LIMIT)
            .expect("tool search should serialize");

        assert!(
            namespace_tool_names(&tools, "openai_topics").contains(&"list_topics".to_string()),
            "identity target should enter the default top 8 before description distractors: {tools:?}"
        );
    }

    #[test]
    fn separator_variants_rank_canonical_tool_first() {
        let handler = ToolSearchHandler::new(vec![
            namespace_search_info(
                "openai_topics",
                "list_topics",
                "List OpenAI topic metadata.",
                "metadata",
                /*source_info*/ None,
            ),
            namespace_search_info(
                "openai_topics",
                "create_topic",
                "Create OpenAI topic metadata.",
                "metadata",
                /*source_info*/ None,
            ),
        ]);

        for query in [
            "openai_topics.list_topics",
            "openai_topics__list_topics",
            "OpenAI Topics list topics",
        ] {
            let first = handler
                .search_ranked(query)
                .into_iter()
                .next()
                .expect("identity query should match");
            assert_eq!(
                first,
                ToolSearchScore {
                    index: 0,
                    identity_tier: ToolSearchIdentityTier::CanonicalAlias,
                    bm25_score: 0.0,
                },
                "query {query:?} should rank list_topics first",
            );
        }
    }

    #[test]
    fn tool_and_source_identity_outrank_description_only_matches() {
        let target = namespace_search_info(
            "openai_topics",
            "list_topics",
            "List OpenAI topic metadata.",
            "metadata",
            Some(ToolSearchSourceInfo {
                name: "OpenAI Topics".to_string(),
                description: None,
            }),
        );
        let description_match = namespace_search_info(
            "notes",
            "read_note",
            "Description repeats list_topics and OpenAI Topics.",
            "list_topics OpenAI Topics OpenAI Topics",
            /*source_info*/ None,
        );
        let handler = ToolSearchHandler::new(vec![description_match, target]);

        assert_eq!(handler.search_ranked("list_topics")[0].index, 1);
        assert_eq!(
            handler.search_ranked("list_topics")[0].identity_tier,
            ToolSearchIdentityTier::ToolAlias
        );
        assert_eq!(handler.search_ranked("OpenAI Topics")[0].index, 1);
        assert_eq!(
            handler.search_ranked("OpenAI Topics")[0].identity_tier,
            ToolSearchIdentityTier::SourceAlias
        );
    }

    #[test]
    fn plugin_display_name_is_source_identity() {
        let mut cortex_tool = tool_info("openai_topics", "list_topics", "List topics");
        cortex_tool.connector_name = Some("OpenAI Topics".to_string());
        cortex_tool.plugin_display_names = vec!["Cortex".to_string()];
        let handler = ToolSearchHandler::new(vec![
            namespace_search_info(
                "notes",
                "read_note",
                "Description repeats Cortex.",
                "Cortex Cortex",
                /*source_info*/ None,
            ),
            McpHandler::new(cortex_tool)
                .expect("MCP tool should convert")
                .search_info()
                .expect("MCP handler should return search info"),
        ]);

        let first = handler.search_ranked("Cortex")[0];

        assert_eq!(first.index, 1);
        assert_eq!(first.identity_tier, ToolSearchIdentityTier::SourceAlias);
    }

    #[test]
    fn description_only_queries_retain_bm25_ordering() {
        let handler = ToolSearchHandler::new(vec![
            namespace_search_info(
                "calendar",
                "create_event",
                "Create calendar events.",
                "calendar event",
                /*source_info*/ None,
            ),
            namespace_search_info(
                "docs",
                "extract_text",
                "Extract text from uploaded documents.",
                "uploaded document uploaded document",
                /*source_info*/ None,
            ),
        ]);

        let results = handler.search_ranked("uploaded document");

        assert_eq!(results[0].index, 1);
        assert_eq!(results[0].identity_tier, ToolSearchIdentityTier::None);
        assert!(results[0].bm25_score > 0.0);
    }

    #[test]
    fn limit_applies_before_namespace_coalescing() {
        let handler = ToolSearchHandler::new(vec![
            namespace_search_info(
                "calendar",
                "create_event",
                "Calendar semantic match.",
                "calendar",
                /*source_info*/ None,
            ),
            namespace_search_info(
                "calendar",
                "list_events",
                "Calendar semantic match.",
                "calendar",
                /*source_info*/ None,
            ),
            namespace_search_info(
                "docs",
                "extract_text",
                "Calendar semantic match.",
                "calendar",
                /*source_info*/ None,
            ),
        ]);

        let tools = handler
            .search("calendar", /*limit*/ 2)
            .expect("tool search should serialize");

        assert_eq!(
            namespace_tool_names(&tools, "calendar"),
            vec!["create_event".to_string(), "list_events".to_string()]
        );
        assert_eq!(tools.len(), 1);
    }

    #[test]
    fn identity_ties_preserve_original_search_info_order() {
        let source = Some(ToolSearchSourceInfo {
            name: "OpenAI Topics".to_string(),
            description: None,
        });
        let handler = ToolSearchHandler::new(vec![
            namespace_search_info(
                "openai_topics",
                "list_topics",
                "List topics.",
                "metadata",
                source.clone(),
            ),
            namespace_search_info(
                "openai_topics",
                "create_topic",
                "Create topic.",
                "metadata",
                source,
            ),
        ]);

        let indexes = handler
            .search_ranked("OpenAI Topics")
            .into_iter()
            .map(|result| result.index)
            .collect::<Vec<_>>();

        assert_eq!(indexes, vec![0, 1]);
    }

    fn namespace_search_info(
        namespace: &str,
        tool_name: &str,
        description: &str,
        search_text: &str,
        source_info: Option<ToolSearchSourceInfo>,
    ) -> ToolSearchInfo {
        ToolSearchInfo::from_spec(
            search_text.to_string(),
            ToolSpec::Namespace(ResponsesApiNamespace {
                name: namespace.to_string(),
                description: format!("Tools in {namespace}."),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: tool_name.to_string(),
                    description: description.to_string(),
                    strict: false,
                    defer_loading: None,
                    parameters: codex_tools::JsonSchema::object(
                        Default::default(),
                        /*required*/ None,
                        Some(false.into()),
                    ),
                    output_schema: None,
                })],
            }),
            source_info,
        )
        .expect("namespace tool should be searchable")
    }

    fn namespace_tool_names(tools: &[LoadableToolSpec], namespace: &str) -> Vec<String> {
        tools
            .iter()
            .filter_map(|tool| match tool {
                LoadableToolSpec::Namespace(namespace_tool) if namespace_tool.name == namespace => {
                    Some(
                        namespace_tool
                            .tools
                            .iter()
                            .map(|tool| match tool {
                                ResponsesApiNamespaceTool::Function(tool) => tool.name.clone(),
                            })
                            .collect::<Vec<_>>(),
                    )
                }
                LoadableToolSpec::Function(_) | LoadableToolSpec::Namespace(_) => None,
            })
            .flatten()
            .collect()
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
            connector_id: None,
            connector_name: None,
            plugin_display_names: Vec::new(),
        }
    }
}
