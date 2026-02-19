use async_trait::async_trait;
use bm25::Document;
use bm25::Language;
use bm25::SearchEngineBuilder;
use codex_app_server_protocol::AppInfo;
use codex_protocol::models::FunctionCallOutputBody;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::connectors;
use crate::function_tool::FunctionCallError;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_connection_manager::ToolInfo;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct SearchToolBm25Handler;

pub(crate) const SEARCH_TOOL_BM25_TOOL_NAME: &str = "search_tool_bm25";
pub(crate) const DEFAULT_LIMIT: usize = 8;

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

#[derive(Deserialize)]
struct SearchToolBm25Args {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Clone)]
struct ToolEntry {
    name: String,
    server_name: String,
    title: Option<String>,
    description: Option<String>,
    connector_name: Option<String>,
    input_keys: Vec<String>,
    search_text: String,
}

impl ToolEntry {
    fn new(name: String, info: ToolInfo) -> Self {
        let input_keys = info
            .tool
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let search_text = build_search_text(&name, &info, &input_keys);
        Self {
            name,
            server_name: info.server_name,
            title: info.tool.title,
            description: info
                .tool
                .description
                .map(|description| description.to_string()),
            connector_name: info.connector_name,
            input_keys,
            search_text,
        }
    }
}

#[async_trait]
impl ToolHandler for SearchToolBm25Handler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            payload,
            session,
            turn,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{SEARCH_TOOL_BM25_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        let args: SearchToolBm25Args = parse_arguments(&arguments)?;
        let query = args.query.trim();
        if query.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "query must not be empty".to_string(),
            ));
        }

        if args.limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let limit = args.limit;

        let mcp_tools = session
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .await;

        let connectors = connectors::with_app_enabled_state(
            connectors::accessible_connectors_from_mcp_tools(&mcp_tools),
            &turn.config,
        );
        let mcp_tools = filter_codex_apps_mcp_tools(mcp_tools, &connectors);

        let mut entries: Vec<ToolEntry> = mcp_tools
            .into_iter()
            .map(|(name, info)| ToolEntry::new(name, info))
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        if entries.is_empty() {
            let active_selected_tools = session.get_mcp_tool_selection().await.unwrap_or_default();
            let content = json!({
                "query": query,
                "total_tools": 0,
                "active_selected_tools": active_selected_tools,
                "tools": [],
            })
            .to_string();
            return Ok(ToolOutput::Function {
                body: FunctionCallOutputBody::Text(content),
                success: Some(true),
            });
        }

        let documents: Vec<Document<usize>> = entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| Document::new(idx, entry.search_text.clone()))
            .collect();
        let search_engine =
            SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();
        let results = search_engine.search(query, limit);

        let mut selected_tools = Vec::new();
        let mut result_payloads = Vec::new();
        for result in results {
            let Some(entry) = entries.get(result.document.id) else {
                continue;
            };
            selected_tools.push(entry.name.clone());
            result_payloads.push(json!({
                "name": entry.name.clone(),
                "server": entry.server_name.clone(),
                "title": entry.title.clone(),
                "description": entry.description.clone(),
                "connector_name": entry.connector_name.clone(),
                "input_keys": entry.input_keys.clone(),
                "score": result.score,
            }));
        }

        let active_selected_tools = session.merge_mcp_tool_selection(selected_tools).await;

        let content = json!({
            "query": query,
            "total_tools": entries.len(),
            "active_selected_tools": active_selected_tools,
            "tools": result_payloads,
        })
        .to_string();

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}

fn filter_codex_apps_mcp_tools(
    mut mcp_tools: HashMap<String, ToolInfo>,
    connectors: &[AppInfo],
) -> HashMap<String, ToolInfo> {
    let enabled_connectors: HashSet<&str> = connectors
        .iter()
        .filter(|connector| connector.is_enabled)
        .map(|connector| connector.id.as_str())
        .collect();

    mcp_tools.retain(|_, tool| {
        if tool.server_name != CODEX_APPS_MCP_SERVER_NAME {
            return false;
        }

        tool.connector_id
            .as_deref()
            .is_some_and(|connector_id| enabled_connectors.contains(connector_id))
    });
    mcp_tools
}

fn build_search_text(name: &str, info: &ToolInfo, input_keys: &[String]) -> String {
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

    if !input_keys.is_empty() {
        parts.extend(input_keys.iter().cloned());
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::AppInfo;
    use pretty_assertions::assert_eq;
    use rmcp::model::JsonObject;
    use rmcp::model::Tool;
    use std::sync::Arc;

    fn make_connector(id: &str, enabled: bool) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: true,
            is_enabled: enabled,
        }
    }

    fn make_tool(
        qualified_name: &str,
        server_name: &str,
        tool_name: &str,
        connector_id: Option<&str>,
    ) -> (String, ToolInfo) {
        (
            qualified_name.to_string(),
            ToolInfo {
                server_name: server_name.to_string(),
                tool_name: tool_name.to_string(),
                tool: Tool {
                    name: tool_name.to_string().into(),
                    title: None,
                    description: Some(format!("Test tool: {tool_name}").into()),
                    input_schema: Arc::new(JsonObject::default()),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                connector_id: connector_id.map(str::to_string),
                connector_name: connector_id.map(str::to_string),
            },
        )
    }

    #[test]
    fn filter_codex_apps_mcp_tools_keeps_enabled_apps_only() {
        let mcp_tools = HashMap::from([
            make_tool(
                "mcp__codex_apps__calendar_create_event",
                CODEX_APPS_MCP_SERVER_NAME,
                "calendar_create_event",
                Some("calendar"),
            ),
            make_tool(
                "mcp__codex_apps__drive_search",
                CODEX_APPS_MCP_SERVER_NAME,
                "drive_search",
                Some("drive"),
            ),
            make_tool("mcp__rmcp__echo", "rmcp", "echo", None),
        ]);
        let connectors = vec![
            make_connector("calendar", false),
            make_connector("drive", true),
        ];

        let mut filtered: Vec<String> = filter_codex_apps_mcp_tools(mcp_tools, &connectors)
            .into_keys()
            .collect();
        filtered.sort();

        assert_eq!(filtered, vec!["mcp__codex_apps__drive_search".to_string()]);
    }

    #[test]
    fn filter_codex_apps_mcp_tools_drops_apps_without_connector_id() {
        let mcp_tools = HashMap::from([
            make_tool(
                "mcp__codex_apps__unknown",
                CODEX_APPS_MCP_SERVER_NAME,
                "unknown",
                None,
            ),
            make_tool("mcp__rmcp__echo", "rmcp", "echo", None),
        ]);

        let mut filtered: Vec<String> =
            filter_codex_apps_mcp_tools(mcp_tools, &[make_connector("calendar", true)])
                .into_keys()
                .collect();
        filtered.sort();

        assert_eq!(filtered, Vec::<String>::new());
    }
}
