use crate::backend::DEFAULT_LIST_MAX_RESULTS;
use crate::backend::DEFAULT_READ_MAX_TOKENS;
use crate::backend::DEFAULT_SEARCH_MAX_RESULTS;
use crate::backend::ListMemoriesRequest;
use crate::backend::MAX_LIST_RESULTS;
use crate::backend::MAX_SEARCH_RESULTS;
use crate::backend::MemoriesBackend;
use crate::backend::MemoriesBackendError;
use crate::backend::ReadMemoryRequest;
use crate::backend::SearchMemoriesRequest;
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

#[derive(Deserialize)]
struct ListArgs {
    path: Option<String>,
    cursor: Option<String>,
    max_results: Option<usize>,
}

#[derive(Deserialize)]
struct ReadArgs {
    path: String,
    line_offset: Option<usize>,
    max_lines: Option<usize>,
}

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
    path: Option<String>,
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
                json!(
                    self.backend
                        .search(SearchMemoriesRequest {
                            query: args.query,
                            path: args.path,
                            max_results: clamp_max_results(
                                args.max_results,
                                DEFAULT_SEARCH_MAX_RESULTS,
                                MAX_SEARCH_RESULTS,
                            ),
                        })
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

pub async fn run_stdio_server(codex_home: &AbsolutePathBuf) -> anyhow::Result<()> {
    let backend = LocalMemoriesBackend::from_codex_home(codex_home);
    tokio::fs::create_dir_all(backend.root())
        .await
        .with_context(|| format!("create memories root at {}", backend.root().display()))?;
    MemoriesMcpServer::new(backend)
        .serve((tokio::io::stdin(), tokio::io::stdout()))
        .await?
        .waiting()
        .await?;
    Ok(())
}

fn list_tool() -> Tool {
    let mut tool = Tool::new(
        Cow::Borrowed(LIST_TOOL_NAME),
        Cow::Borrowed("List files and directories under the Codex memories store."),
        Arc::new(schema::list_input_schema()),
    );
    tool.output_schema = Some(Arc::new(schema::list_output_schema()));
    tool.annotations = Some(ToolAnnotations::new().read_only(true));
    tool
}

fn read_tool() -> Tool {
    let mut tool = Tool::new(
        Cow::Borrowed(READ_TOOL_NAME),
        Cow::Borrowed(
            "Read a Codex memory file by relative path, optionally starting at a 1-indexed line offset and limiting the number of lines returned.",
        ),
        Arc::new(schema::read_input_schema()),
    );
    tool.output_schema = Some(Arc::new(schema::read_output_schema()));
    tool.annotations = Some(ToolAnnotations::new().read_only(true));
    tool
}

fn search_tool() -> Tool {
    let mut tool = Tool::new(
        Cow::Borrowed(SEARCH_TOOL_NAME),
        Cow::Borrowed("Search Codex memory files for exact text matches."),
        Arc::new(schema::search_input_schema()),
    );
    tool.output_schema = Some(Arc::new(schema::search_output_schema()));
    tool.annotations = Some(ToolAnnotations::new().read_only(true));
    tool
}

fn parse_args<T: for<'de> Deserialize<'de>>(value: serde_json::Value) -> Result<T, McpError> {
    serde_json::from_value(value).map_err(|err| McpError::invalid_params(err.to_string(), None))
}

fn clamp_max_results(requested: Option<usize>, default: usize, max: usize) -> usize {
    requested.unwrap_or(default).clamp(1, max)
}

fn backend_error_to_mcp(err: MemoriesBackendError) -> McpError {
    match err {
        MemoriesBackendError::InvalidPath { .. }
        | MemoriesBackendError::InvalidCursor { .. }
        | MemoriesBackendError::InvalidLineOffset
        | MemoriesBackendError::InvalidMaxLines
        | MemoriesBackendError::LineOffsetExceedsFileLength
        | MemoriesBackendError::NotFile { .. }
        | MemoriesBackendError::EmptyQuery => McpError::invalid_params(err.to_string(), None),
        MemoriesBackendError::Io(_) => McpError::internal_error(err.to_string(), None),
    }
}
