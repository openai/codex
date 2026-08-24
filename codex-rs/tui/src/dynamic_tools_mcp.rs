//! Approval-gated MCP transport for task tools owned by a local-daemon TUI.

use crate::app_event_sender::AppEventSender;
use crate::dynamic_tools;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::middleware;
use axum::middleware::Next;
use axum::response::Response;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallParams;
use codex_app_server_protocol::DynamicToolNamespaceTool;
use codex_app_server_protocol::DynamicToolSpec;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStatusChangedNotification;
use codex_config::McpServerConfig;
use codex_config::McpServerRequirement;
use codex_config::RawMcpServerConfig;
use rmcp::ErrorData as McpError;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::ContentBlock;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::Value;
use serde_json::json;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) enum ThreadToolTransport {
    Disabled,
    Dynamic,
    Mcp(Arc<DynamicToolMcpServer>),
}

impl ThreadToolTransport {
    pub(crate) fn configure(&self, params: &mut ThreadStartParams) {
        match self {
            Self::Disabled => params.dynamic_tools = None,
            Self::Dynamic => {
                params.dynamic_tools = Some(dynamic_tools::non_delegation_tool_specs());
            }
            Self::Mcp(_) => {
                params.dynamic_tools = None;
                self.configure_mcp(&mut params.config);
            }
        }
    }

    pub(crate) fn configure_mcp(&self, config: &mut Option<HashMap<String, Value>>) {
        if let Self::Mcp(server) = self {
            config.get_or_insert_default().insert(
                format!("mcp_servers.{}", dynamic_tools::NAMESPACE),
                server.config.clone(),
            );
        }
    }
}

pub(crate) struct DynamicToolMcpServer {
    config: Value,
    task: JoinHandle<()>,
}

impl DynamicToolMcpServer {
    pub(crate) async fn start(
        request_handle: AppServerRequestHandle,
        mut thread_start_params: ThreadStartParams,
        app_event_tx: AppEventSender,
        status_updates: broadcast::Sender<ThreadStatusChangedNotification>,
        managed_requirement: Option<&McpServerRequirement>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let authorization = Arc::new(format!("Bearer {}", Uuid::new_v4()));
        let server_config = json!({
            "url": format!("http://{address}/mcp"),
            "http_headers": {"Authorization": authorization.as_str()},
            "default_tools_approval_mode": "approve",
            "tools": {
                "create_thread": {"approval_mode": "prompt"},
                "send_message_to_thread": {"approval_mode": "prompt"},
                "fork_thread": {"approval_mode": "prompt"}
            }
        });
        if let Some(requirement) = managed_requirement {
            let raw_config = serde_json::from_value::<RawMcpServerConfig>(server_config.clone())
                .map_err(|error| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
                })?;
            let configured_server = McpServerConfig::try_from(raw_config)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            if !configured_server.matches_requirement(requirement) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "managed MCP requirements do not permit the TUI task-tools server",
                ));
            }
        }
        if let Some(overrides) = thread_start_params.config.as_mut() {
            overrides.remove("web_search");
        }
        let handler = DynamicToolMcpHandler {
            request_handle,
            thread_start_params,
            app_event_tx,
            status_updates,
            server_config: server_config.clone(),
        };
        let service = StreamableHttpService::new(
            move || Ok(handler.clone()),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default(),
        );
        let router =
            Router::new()
                .nest_service("/mcp", service)
                .layer(middleware::from_fn_with_state(
                    authorization,
                    require_authorization,
                ));
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router).await {
                tracing::warn!(%error, "TUI task-tools MCP server stopped");
            }
        });
        Ok(Self {
            config: server_config,
            task,
        })
    }
}

impl Drop for DynamicToolMcpServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn require_authorization(
    State(expected): State<Arc<String>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if request
        .headers()
        .get(AUTHORIZATION)
        .is_some_and(|value| value.as_bytes() == expected.as_bytes())
    {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[derive(Clone)]
struct DynamicToolMcpHandler {
    request_handle: AppServerRequestHandle,
    thread_start_params: ThreadStartParams,
    app_event_tx: AppEventSender,
    status_updates: broadcast::Sender<ThreadStatusChangedNotification>,
    server_config: Value,
}

impl ServerHandler for DynamicToolMcpHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut tools = Vec::new();
        for spec in dynamic_tools::tool_specs() {
            let functions = match spec {
                DynamicToolSpec::Function(function) => vec![function],
                DynamicToolSpec::Namespace(namespace) => namespace
                    .tools
                    .into_iter()
                    .map(|tool| match tool {
                        DynamicToolNamespaceTool::Function(function) => function,
                    })
                    .collect(),
            };
            for function in functions {
                let schema = serde_json::from_value::<JsonObject>(function.input_schema)
                    .map_err(|error| McpError::internal_error(error.to_string(), None))?;
                let mut tool = Tool::new(
                    Cow::Owned(function.name),
                    Cow::Owned(function.description),
                    Arc::new(schema),
                );
                tool.annotations = Some(ToolAnnotations::new().read_only(matches!(
                    tool.name.as_ref(),
                    "list_threads" | "list_archived_threads" | "read_thread" | "wait_threads"
                )));
                tools.push(tool);
            }
        }
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, McpError> {
        let metadata = &context.meta.0.0;
        let turn_metadata = metadata
            .get("x-codex-turn-metadata")
            .and_then(|value| match value {
                Value::Object(_) => Some(value.clone()),
                Value::String(value) => serde_json::from_str(value).ok(),
                _ => None,
            });
        let thread_id = metadata
            .get("threadId")
            .and_then(Value::as_str)
            .or_else(|| turn_metadata.as_ref()?.get("thread_id")?.as_str())
            .filter(|thread_id| !thread_id.is_empty())
            .ok_or_else(|| McpError::invalid_params("missing task metadata", None))?;
        let turn_id = metadata
            .get("turnId")
            .and_then(Value::as_str)
            .or_else(|| turn_metadata.as_ref()?.get("turn_id")?.as_str())
            .map_or_else(|| format!("mcp-turn-{}", Uuid::new_v4()), str::to_string);
        let call_id = metadata
            .get("callId")
            .and_then(Value::as_str)
            .map_or_else(|| format!("mcp-call-{}", Uuid::new_v4()), str::to_string);
        let params = DynamicToolCallParams {
            thread_id: thread_id.to_string(),
            turn_id,
            call_id,
            namespace: Some(dynamic_tools::NAMESPACE.to_string()),
            tool: request.name.into_owned(),
            arguments: Value::Object(request.arguments.unwrap_or_default()),
        };
        let mut thread_start_params = self.thread_start_params.clone();
        thread_start_params.config.get_or_insert_default().insert(
            format!("mcp_servers.{}", dynamic_tools::NAMESPACE),
            self.server_config.clone(),
        );
        let response = dynamic_tools::execute(
            self.request_handle.clone(),
            params,
            thread_start_params,
            self.status_updates.subscribe(),
            Some(&self.app_event_tx),
        )
        .await;
        let content = response
            .content_items
            .into_iter()
            .map(|item| match item {
                DynamicToolCallOutputContentItem::InputText { text } => ContentBlock::text(text),
                DynamicToolCallOutputContentItem::InputImage { image_url } => {
                    ContentBlock::text(image_url)
                }
                DynamicToolCallOutputContentItem::InputAudio { audio_url } => {
                    ContentBlock::text(audio_url)
                }
            })
            .collect();
        Ok(if response.success {
            CallToolResult::success(content)
        } else {
            CallToolResult::error(content)
        }
        .into())
    }
}
