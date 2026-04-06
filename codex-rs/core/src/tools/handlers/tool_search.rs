use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::ToolSearchOutput;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use bm25::Document;
use bm25::Language;
use bm25::SearchEngineBuilder;
use codex_mcp::mcp_connection_manager::ToolInfo;
use codex_tools::TOOL_SEARCH_DEFAULT_LIMIT;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolSearchResultSource;
use codex_tools::collect_tool_search_output_tools;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ToolSearchHandler {
    tools: HashMap<String, ToolInfo>,
}

impl ToolSearchHandler {
    pub fn new(mut tools: HashMap<String, ToolInfo>, include_watchdog_tools: bool) -> Self {
        if include_watchdog_tools {
            tools.extend(watchdog_tool_infos());
        }
        Self { tools }
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
        let limit = args.limit.unwrap_or(TOOL_SEARCH_DEFAULT_LIMIT);

        if limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let mut entries: Vec<(String, ToolInfo)> = self.tools.clone().into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        if entries.is_empty() {
            return Ok(ToolSearchOutput { tools: Vec::new() });
        }

        let documents: Vec<Document<usize>> = entries
            .iter()
            .enumerate()
            .map(|(idx, (name, info))| Document::new(idx, build_search_text(name, info)))
            .collect();
        let search_engine =
            SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();
        let results = search_engine.search(query, limit);

        let tools = collect_tool_search_output_tools(
            results
                .into_iter()
                .filter_map(|result| entries.get(result.document.id))
                .map(|(_name, tool)| ToolSearchResultSource {
                    tool_namespace: tool.tool_namespace.as_str(),
                    tool_name: tool.tool_name.as_str(),
                    tool: &tool.tool,
                    connector_name: tool.connector_name.as_deref(),
                    connector_description: tool.connector_description.as_deref(),
                }),
        )
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to encode {TOOL_SEARCH_TOOL_NAME} output: {err}"
            ))
        })?;

        Ok(ToolSearchOutput { tools })
    }
}

fn watchdog_tool_infos() -> HashMap<String, ToolInfo> {
    HashMap::from([
        (
            "watchdog:compact_parent_context".to_string(),
            watchdog_tool_info(
                "compact_parent_context",
                "Watchdog-only: request compaction for the watchdog helper's parent thread when it is idle and appears stuck.",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "reason": {"type": "string"},
                        "evidence": {"type": "string"}
                    },
                    "additionalProperties": false
                }),
            ),
        ),
        (
            "watchdog:watchdog_self_close".to_string(),
            watchdog_tool_info(
                "watchdog_self_close",
                "Watchdog-only: send an optional final message to the parent/root thread, close this watchdog's persistent handle, and end this check-in immediately.",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"}
                    },
                    "additionalProperties": false
                }),
            ),
        ),
        (
            "watchdog:snooze".to_string(),
            watchdog_tool_info(
                "snooze",
                "Watchdog-only: keep this watchdog running, skip reporting anything for this check-in, and wait before the next wakeup.",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "delay_seconds": {"type": "number"},
                        "reason": {"type": "string"}
                    },
                    "additionalProperties": false
                }),
            ),
        ),
    ])
}

fn watchdog_tool_info(
    tool_name: &str,
    description: &str,
    input_schema: serde_json::Value,
) -> ToolInfo {
    ToolInfo {
        server_name: "watchdog".to_string(),
        tool_name: tool_name.to_string(),
        tool_namespace: "watchdog".to_string(),
        tool: rmcp::model::Tool {
            name: tool_name.to_string().into(),
            title: None,
            description: Some(description.to_string().into()),
            input_schema: Arc::new(rmcp::model::object(input_schema)),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        connector_id: Some("watchdog".to_string()),
        connector_name: Some("watchdog".to_string()),
        connector_description: Some(
            "Watchdog-only tools for parent-thread recovery and watchdog check-in lifecycle control."
                .to_string(),
        ),
        plugin_display_names: Vec::new(),
    }
}

fn build_search_text(name: &str, info: &ToolInfo) -> String {
    let mut parts = vec![
        name.to_string(),
        info.tool_name.clone(),
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
        info.tool
            .input_schema
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
    use crate::codex::make_session_and_context;
    use crate::tools::context::ToolInvocation;
    use crate::tools::context::ToolPayload;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_protocol::models::SearchToolCallParams;
    use codex_tools::ResponsesApiNamespaceTool;
    use codex_tools::ToolSearchOutputTool;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn watchdog_tool_search_corpus_includes_snooze() {
        let (session, turn) = make_session_and_context().await;
        let output = ToolSearchHandler::new(HashMap::new(), /*include_watchdog_tools*/ true)
            .handle(ToolInvocation {
                session: Arc::new(session),
                turn: Arc::new(turn),
                tracker: Arc::new(Mutex::new(TurnDiffTracker::default())),
                call_id: "call-1".to_string(),
                tool_name: TOOL_SEARCH_TOOL_NAME.to_string(),
                tool_namespace: None,
                payload: ToolPayload::ToolSearch {
                    arguments: SearchToolCallParams {
                        query: "watchdog snooze".to_string(),
                        limit: Some(5),
                    },
                },
            })
            .await
            .expect("watchdog tool search should succeed");

        let names = output
            .tools
            .iter()
            .filter_map(|tool| match tool {
                ToolSearchOutputTool::Function(_) => None,
                ToolSearchOutputTool::Namespace(namespace) => Some((
                    namespace.name.as_str(),
                    namespace
                        .tools
                        .iter()
                        .map(|tool| match tool {
                            ResponsesApiNamespaceTool::Function(tool) => tool.name.as_str(),
                        })
                        .collect::<Vec<_>>(),
                )),
            })
            .collect::<Vec<_>>();

        assert!(
            names
                .iter()
                .any(|(namespace, tools)| *namespace == "watchdog" && tools.contains(&"snooze")),
            "expected watchdog.snooze in tool_search output, got {names:?}"
        );
    }
}
