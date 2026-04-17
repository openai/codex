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
use codex_tools::TOOL_SEARCH_DEFAULT_LIMIT;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolSearchResultSource;
use codex_tools::collect_tool_search_output_tools;

const COMPUTER_USE_MCP_SERVER_NAME: &str = "computer-use";
const COMPUTER_USE_TOOL_SEARCH_LIMIT: usize = 20;

pub struct ToolSearchHandler {
    entries: Vec<(String, ToolInfo)>,
    search_engine: SearchEngine<usize>,
}

impl ToolSearchHandler {
    pub fn new(tools: std::collections::HashMap<String, ToolInfo>) -> Self {
        let mut entries: Vec<(String, ToolInfo)> = tools.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        let documents: Vec<Document<usize>> = entries
            .iter()
            .enumerate()
            .map(|(idx, (name, info))| Document::new(idx, build_search_text(name, info)))
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

        let results = self.search_result_entries(query, limit, requested_limit.is_none());

        let tools = collect_tool_search_output_tools(results.into_iter().map(|(_, tool)| {
            ToolSearchResultSource {
                server_name: tool.server_name.as_str(),
                tool_namespace: tool.callable_namespace.as_str(),
                tool_name: tool.callable_name.as_str(),
                tool: &tool.tool,
                connector_name: tool.connector_name.as_deref(),
                connector_description: tool.connector_description.as_deref(),
            }
        }))
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to encode {TOOL_SEARCH_TOOL_NAME} output: {err}"
            ))
        })?;

        Ok(ToolSearchOutput { tools })
    }
}

impl ToolSearchHandler {
    fn search_result_entries(
        &self,
        query: &str,
        limit: usize,
        use_default_limit: bool,
    ) -> Vec<&(String, ToolInfo)> {
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
            .any(|(_, tool)| tool.server_name == COMPUTER_USE_MCP_SERVER_NAME)
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

fn limit_results_per_server(results: Vec<&(String, ToolInfo)>) -> Vec<&(String, ToolInfo)> {
    results
        .into_iter()
        .scan(
            std::collections::HashMap::<&str, usize>::new(),
            |counts, entry| {
                let tool = &entry.1;
                let count = counts.entry(tool.server_name.as_str()).or_default();
                if *count >= default_limit_for_server(tool.server_name.as_str()) {
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

fn build_search_text(name: &str, info: &ToolInfo) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rmcp::model::Tool;
    use std::sync::Arc;

    #[test]
    fn computer_use_tool_search_uses_larger_limit() {
        let handler = ToolSearchHandler::new(numbered_tools(
            COMPUTER_USE_MCP_SERVER_NAME,
            "computer use",
            /*count*/ 100,
        ));

        let results = handler.search_result_entries(
            "computer use",
            TOOL_SEARCH_DEFAULT_LIMIT,
            /*use_default_limit*/ true,
        );

        assert_eq!(results.len(), COMPUTER_USE_TOOL_SEARCH_LIMIT);
        assert!(
            results
                .iter()
                .all(|(_, tool)| tool.server_name == COMPUTER_USE_MCP_SERVER_NAME)
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
        let handler = ToolSearchHandler::new(tools);

        let results = handler.search_result_entries(
            "calendar",
            TOOL_SEARCH_DEFAULT_LIMIT,
            /*use_default_limit*/ true,
        );

        assert_eq!(results.len(), TOOL_SEARCH_DEFAULT_LIMIT);
        assert!(
            results
                .iter()
                .all(|(_, tool)| tool.server_name == "other-server")
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
        let handler = ToolSearchHandler::new(tools);

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

    fn count_results_for_server(results: &[&(String, ToolInfo)], server_name: &str) -> usize {
        results
            .iter()
            .filter(|(_, tool)| tool.server_name == server_name)
            .count()
    }
}
