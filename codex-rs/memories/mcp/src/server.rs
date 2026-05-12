use crate::backend::DEFAULT_LIST_MAX_RESULTS;
use crate::backend::DEFAULT_READ_MAX_TOKENS;
use crate::backend::DEFAULT_SEARCH_MAX_RESULTS;
use crate::backend::ListMemoriesRequest;
use crate::backend::ListMemoriesResponse;
use crate::backend::MAX_LIST_RESULTS;
use crate::backend::MAX_SEARCH_RESULTS;
use crate::backend::MemoriesBackend;
use crate::backend::MemoriesBackendError;
use crate::backend::ReadMemoryRequest;
use crate::backend::ReadMemoryResponse;
use crate::backend::SearchMatchMode;
use crate::backend::SearchMemoriesRequest;
use crate::backend::SearchMemoriesResponse;
use crate::local::LocalMemoriesBackend;
use crate::schema;
use anyhow::Context;
use codex_utils_absolute_path::AbsolutePathBuf;
use rmcp::ErrorData as McpError;
use rmcp::ServiceExt;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::borrow::Cow;
use std::sync::Arc;

const LIST_TOOL_NAME: &str = "list";
const READ_TOOL_NAME: &str = "read";
const SEARCH_TOOL_NAME: &str = "search";

#[derive(Clone)]
pub struct MemoriesMcpServer<B> {
    backend: B,
    tools: Arc<Vec<Tool>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    path: Option<String>,
    cursor: Option<String>,
    #[schemars(range(min = 1))]
    max_results: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    path: String,
    #[schemars(range(min = 1))]
    line_offset: Option<usize>,
    #[schemars(range(min = 1))]
    max_lines: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SearchArgs {
    #[schemars(length(min = 1))]
    queries: Vec<String>,
    match_mode: Option<SearchMatchMode>,
    path: Option<String>,
    cursor: Option<String>,
    #[schemars(range(min = 0))]
    context_lines: Option<usize>,
    case_sensitive: Option<bool>,
    normalized: Option<bool>,
    #[schemars(range(min = 1))]
    max_results: Option<usize>,
}

impl<B: MemoriesBackend> MemoriesMcpServer<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            tools: Arc::new(vec![list_tool(), read_tool(), search_tool()]),
        }
    }
}

impl<B: MemoriesBackend> ServerHandler for MemoriesMcpServer<B> {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Use these tools to list, read, and search Codex memory files.".to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let tools = Arc::clone(&self.tools);
        async move {
            Ok(ListToolsResult {
                tools: (*tools).clone(),
                next_cursor: None,
                meta: None,
            })
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let value = serde_json::Value::Object(
            request
                .arguments
                .unwrap_or_default()
                .into_iter()
                .collect::<serde_json::Map<String, serde_json::Value>>(),
        );
        let structured_content = match request.name.as_ref() {
            LIST_TOOL_NAME => {
                let args: ListArgs = parse_args(value)?;
                json!(
                    self.backend
                        .list(ListMemoriesRequest {
                            path: args.path,
                            cursor: args.cursor,
                            max_results: clamp_max_results(
                                args.max_results,
                                DEFAULT_LIST_MAX_RESULTS,
                                MAX_LIST_RESULTS,
                            ),
                        })
                        .await
                        .map_err(backend_error_to_mcp)?
                )
            }
            READ_TOOL_NAME => {
                let args: ReadArgs = parse_args(value)?;
                json!(
                    self.backend
                        .read(ReadMemoryRequest {
                            path: args.path,
                            line_offset: args.line_offset.unwrap_or(1),
                            max_lines: args.max_lines,
                            max_tokens: DEFAULT_READ_MAX_TOKENS,
                        })
                        .await
                        .map_err(backend_error_to_mcp)?
                )
            }
            SEARCH_TOOL_NAME => {
                let args: SearchArgs = parse_args(value)?;
                let request = args.into_request();
                json!(
                    self.backend
                        .search(request)
                        .await
                        .map_err(backend_error_to_mcp)?
                )
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown tool: {other}"),
                    None,
                ));
            }
        };

        Ok(CallToolResult {
            content: vec![Content::text(structured_content.to_string())],
            structured_content: Some(structured_content),
            is_error: Some(false),
            meta: None,
        })
    }
}

pub async fn run_server<T, E, A>(codex_home: &AbsolutePathBuf, transport: T) -> anyhow::Result<()>
where
    T: rmcp::transport::IntoTransport<rmcp::RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let backend = LocalMemoriesBackend::from_codex_home(codex_home);
    tokio::fs::create_dir_all(backend.root())
        .await
        .with_context(|| format!("create memories root at {}", backend.root().display()))?;
    MemoriesMcpServer::new(backend)
        .serve(transport)
        .await?
        .waiting()
        .await?;
    Ok(())
}

pub async fn run_stdio_server(codex_home: &AbsolutePathBuf) -> anyhow::Result<()> {
    run_server(codex_home, (tokio::io::stdin(), tokio::io::stdout())).await
}

fn list_tool() -> Tool {
    let mut tool = Tool::new(
        Cow::Borrowed(LIST_TOOL_NAME),
        Cow::Borrowed(
            "List immediate files and directories under a path in the Codex memories store.",
        ),
        Arc::new(schema::input_schema_for::<ListArgs>()),
    );
    tool.output_schema = Some(Arc::new(schema::output_schema_for::<ListMemoriesResponse>()));
    tool.annotations = Some(ToolAnnotations::new().read_only(true));
    tool
}

fn read_tool() -> Tool {
    let mut tool = Tool::new(
        Cow::Borrowed(READ_TOOL_NAME),
        Cow::Borrowed(
            "Read a Codex memory file by relative path, optionally starting at a 1-indexed line offset and limiting the number of lines returned.",
        ),
        Arc::new(schema::input_schema_for::<ReadArgs>()),
    );
    tool.output_schema = Some(Arc::new(schema::output_schema_for::<ReadMemoryResponse>()));
    tool.annotations = Some(ToolAnnotations::new().read_only(true));
    tool
}

fn search_tool() -> Tool {
    let mut tool = Tool::new(
        Cow::Borrowed(SEARCH_TOOL_NAME),
        Cow::Borrowed(
            "Search Codex memory files for substring matches, optionally normalizing separators or requiring all query substrings on the same line or within a line window.",
        ),
        Arc::new(schema::input_schema_for::<SearchArgs>()),
    );
    tool.output_schema = Some(Arc::new(
        schema::output_schema_for::<SearchMemoriesResponse>(),
    ));
    tool.annotations = Some(ToolAnnotations::new().read_only(true));
    tool
}

fn parse_args<T: for<'de> Deserialize<'de>>(value: serde_json::Value) -> Result<T, McpError> {
    serde_json::from_value(value).map_err(|err| McpError::invalid_params(err.to_string(), None))
}

impl SearchArgs {
    fn into_request(self) -> SearchMemoriesRequest {
        SearchMemoriesRequest {
            queries: self.queries,
            match_mode: self.match_mode.unwrap_or(SearchMatchMode::Any),
            path: self.path,
            cursor: self.cursor,
            context_lines: self.context_lines.unwrap_or(0),
            case_sensitive: self.case_sensitive.unwrap_or(true),
            normalized: self.normalized.unwrap_or(false),
            max_results: clamp_max_results(
                self.max_results,
                DEFAULT_SEARCH_MAX_RESULTS,
                MAX_SEARCH_RESULTS,
            ),
        }
    }
}

fn clamp_max_results(requested: Option<usize>, default: usize, max: usize) -> usize {
    requested.unwrap_or(default).clamp(1, max)
}

fn backend_error_to_mcp(err: MemoriesBackendError) -> McpError {
    match err {
        MemoriesBackendError::InvalidPath { .. }
        | MemoriesBackendError::InvalidCursor { .. }
        | MemoriesBackendError::NotFound { .. }
        | MemoriesBackendError::InvalidLineOffset
        | MemoriesBackendError::InvalidMaxLines
        | MemoriesBackendError::LineOffsetExceedsFileLength
        | MemoriesBackendError::NotFile { .. }
        | MemoriesBackendError::EmptyQuery
        | MemoriesBackendError::InvalidMatchWindow => {
            McpError::invalid_params(err.to_string(), None)
        }
        MemoriesBackendError::Io(_) => McpError::internal_error(err.to_string(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn search_args_accept_multiple_queries() {
        let args: SearchArgs = parse_args(json!({
            "queries": ["alpha", "needle"],
            "case_sensitive": false
        }))
        .expect("multi-query args should parse");

        let request = args.into_request();

        assert_eq!(
            request,
            SearchMemoriesRequest {
                queries: vec!["alpha".to_string(), "needle".to_string()],
                match_mode: SearchMatchMode::Any,
                path: None,
                cursor: None,
                context_lines: 0,
                case_sensitive: false,
                normalized: false,
                max_results: DEFAULT_SEARCH_MAX_RESULTS,
            }
        );
    }

    #[test]
    fn search_args_accept_windowed_all_match_mode() {
        let args: SearchArgs = parse_args(json!({
            "queries": ["alpha", "needle"],
            "match_mode": {
                "type": "all_within_lines",
                "line_count": 3
            }
        }))
        .expect("windowed all args should parse");

        let request = args.into_request();

        assert_eq!(
            request,
            SearchMemoriesRequest {
                queries: vec!["alpha".to_string(), "needle".to_string()],
                match_mode: SearchMatchMode::AllWithinLines { line_count: 3 },
                path: None,
                cursor: None,
                context_lines: 0,
                case_sensitive: true,
                normalized: false,
                max_results: DEFAULT_SEARCH_MAX_RESULTS,
            }
        );
    }

    #[test]
    fn search_args_accept_normalized_matching() {
        let args: SearchArgs = parse_args(json!({
            "queries": ["multi agent v2"],
            "case_sensitive": false,
            "normalized": true
        }))
        .expect("normalized args should parse");

        let request = args.into_request();

        assert_eq!(
            request,
            SearchMemoriesRequest {
                queries: vec!["multi agent v2".to_string()],
                match_mode: SearchMatchMode::Any,
                path: None,
                cursor: None,
                context_lines: 0,
                case_sensitive: false,
                normalized: true,
                max_results: DEFAULT_SEARCH_MAX_RESULTS,
            }
        );
    }

    #[test]
    fn search_args_reject_legacy_single_query() {
        let err = parse_args::<SearchArgs>(json!({
            "query": "needle",
        }))
        .expect_err("legacy query field should be rejected");

        assert!(err.message.contains("unknown field"));
        assert!(err.message.contains("query"));
    }

    #[test]
    fn search_args_reject_unknown_fields() {
        let err = parse_args::<SearchArgs>(json!({
            "queries": ["needle"],
            "query": "needle"
        }))
        .expect_err("unknown fields should be rejected");

        assert!(err.message.contains("unknown field"));
        assert!(err.message.contains("query"));
    }
}
