use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct McpSearchHandler;

const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT: usize = 50;

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

#[derive(Deserialize)]
struct McpSearchArgs {
    query: String,
    #[serde(default)]
    server: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchResult {
    qualified_name: String,
    server: String,
    tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchResponse {
    query: String,
    total_matches: usize,
    results: Vec<McpSearchResult>,
}

#[async_trait]
impl ToolHandler for McpSearchHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "mcp_search handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: McpSearchArgs = parse_arguments(&arguments)?;
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

        let limit = args.limit.min(MAX_LIMIT);
        let query_lc = query.to_lowercase();
        let server_filter = args
            .server
            .as_deref()
            .map(str::trim)
            .filter(|val| !val.is_empty());

        let tools = session
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .await;

        let mut matches = Vec::new();
        for (qualified_name, tool_info) in tools {
            if let Some(server) = server_filter
                && tool_info.server_name != server
            {
                continue;
            }

            let tool_name_lc = tool_info.tool_name.to_lowercase();
            let qualified_name_lc = qualified_name.to_lowercase();
            let description = tool_info.tool.description.as_deref().unwrap_or("");
            let description_lc = description.to_lowercase();
            let mut score = 0;
            if tool_name_lc.contains(&query_lc) || qualified_name_lc.contains(&query_lc) {
                score += 2;
            }
            if description_lc.contains(&query_lc) {
                score += 1;
            }

            if score == 0 {
                continue;
            }

            matches.push((
                score,
                qualified_name,
                tool_info.server_name,
                tool_info.tool_name,
                tool_info.tool.description,
            ));
        }

        matches.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let total_matches = matches.len();
        let results = matches
            .into_iter()
            .take(limit)
            .map(
                |(_, qualified_name, server, tool, description)| McpSearchResult {
                    qualified_name,
                    server,
                    tool,
                    description,
                },
            )
            .collect();

        let response = McpSearchResponse {
            query: query.to_string(),
            total_matches,
            results,
        };
        let content = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to serialize response: {err}"))
        })?;

        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(true),
        })
    }
}
