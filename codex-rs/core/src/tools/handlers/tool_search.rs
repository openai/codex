use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::ToolSearchOutput;
use crate::tools::handlers::tool_search_spec::create_tool_search_tool;
use crate::tools::registry::ToolHandler;
use crate::tools::tool_search_entry::ToolSearchEntry;
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

pub struct ToolSearchHandler {
    entries: Vec<ToolSearchEntry>,
    search_source_infos: Vec<ToolSearchSourceInfo>,
    search_engine: SearchEngine<usize>,
}

impl ToolSearchHandler {
    pub(crate) fn new(
        entries: Vec<ToolSearchEntry>,
        search_source_infos: Vec<ToolSearchSourceInfo>,
    ) -> Self {
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
        }
    }
}

impl ToolHandler for ToolSearchHandler {
    type Output = ToolSearchOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain(TOOL_SEARCH_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_tool_search_tool(
            &self.search_source_infos,
            TOOL_SEARCH_DEFAULT_LIMIT,
        ))
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
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
        let limit = args.limit.unwrap_or(TOOL_SEARCH_DEFAULT_LIMIT);

        if limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        if self.entries.is_empty() {
            return Ok(ToolSearchOutput { tools: Vec::new() });
        }

        let tools = self.search(query, limit)?;

        Ok(ToolSearchOutput { tools })
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::make_session_and_context;
    use crate::tools::context::ToolCallSource;
    use crate::tools::tool_search_entry::build_tool_search_entries;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_mcp::ToolInfo;
    use codex_protocol::dynamic_tools::DynamicToolSpec;
    use codex_protocol::models::SearchToolCallParams;
    use codex_tools::ResponsesApiNamespace;
    use codex_tools::ResponsesApiNamespaceTool;
    use codex_tools::ResponsesApiTool;
    use pretty_assertions::assert_eq;
    use rmcp::model::Tool;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[test]
    fn mixed_search_results_coalesce_mcp_namespaces() {
        let dynamic_tools = vec![DynamicToolSpec {
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
        let mcp_tools = vec![
            tool_info("calendar", "create_event", "Create events"),
            tool_info("calendar", "list_events", "List events"),
        ];
        let handler = handler_from_tools(Some(&mcp_tools), &dynamic_tools);
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

    #[tokio::test]
    async fn omitted_limit_uses_default_tool_search_result_limit() {
        let tool_count = TOOL_SEARCH_DEFAULT_LIMIT + 5;
        let dynamic_tools = numbered_dynamic_tools(tool_count);
        let handler = handler_from_tools(/*mcp_tools*/ None, &dynamic_tools);

        let output = tool_search_output(&handler, /*limit*/ None).await;

        assert_eq!(output.tools.len(), TOOL_SEARCH_DEFAULT_LIMIT);
    }

    #[tokio::test]
    async fn explicit_limit_controls_tool_search_result_count() {
        let explicit_limit = 3;
        let tool_count = TOOL_SEARCH_DEFAULT_LIMIT + explicit_limit;
        let dynamic_tools = numbered_dynamic_tools(tool_count);
        let handler = handler_from_tools(/*mcp_tools*/ None, &dynamic_tools);

        let output = tool_search_output(&handler, Some(explicit_limit)).await;

        assert_eq!(output.tools.len(), explicit_limit);
    }

    async fn tool_search_output(
        handler: &ToolSearchHandler,
        limit: Option<usize>,
    ) -> ToolSearchOutput {
        let (session, turn) = make_session_and_context().await;
        handler
            .handle(ToolInvocation {
                session: Arc::new(session),
                turn: Arc::new(turn),
                cancellation_token: tokio_util::sync::CancellationToken::new(),
                tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
                call_id: "call-tool-search".to_string(),
                tool_name: ToolName::plain(TOOL_SEARCH_TOOL_NAME),
                source: ToolCallSource::Direct,
                payload: ToolPayload::ToolSearch {
                    arguments: SearchToolCallParams {
                        query: "calendar".to_string(),
                        limit,
                    },
                },
            })
            .await
            .expect("tool_search should succeed")
    }

    fn numbered_dynamic_tools(count: usize) -> Vec<DynamicToolSpec> {
        (0..count)
            .map(|index| DynamicToolSpec {
                namespace: None,
                name: format!("calendar_tool_{index:03}"),
                description: "Calendar search helper.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }),
                defer_loading: true,
            })
            .collect()
    }

    fn tool_info(server_name: &str, tool_name: &str, description_prefix: &str) -> ToolInfo {
        ToolInfo {
            server_name: server_name.to_string(),
            supports_parallel_tool_calls: false,
            server_origin: None,
            callable_name: tool_name.to_string(),
            callable_namespace: format!("mcp__{server_name}__"),
            namespace_description: None,
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
        }
    }

    fn handler_from_tools(
        mcp_tools: Option<&[ToolInfo]>,
        dynamic_tools: &[DynamicToolSpec],
    ) -> ToolSearchHandler {
        ToolSearchHandler::new(
            build_tool_search_entries(mcp_tools, dynamic_tools),
            Vec::new(),
        )
    }
}
