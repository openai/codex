//! Streamable HTTP and trusted-host caller metadata for the shared MCP contract.
//! Backend selection and membership validation belong to the supplied adapter binding.

use crate::MessageBoardMcpServer;
use crate::list_tools_result;
use crate::server_info;
use axum::Router;
use axum::extract::Request;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::middleware;
use axum::middleware::Next;
use codex_message_board::MessageBoardTool;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use rmcp::ErrorData;
use rmcp::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResponse;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ServerConfig;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde::Deserialize;
use std::future::Future;
use std::sync::Arc;
use tokio::net::TcpListener;

/// Serves the discussion tools on `/mcp`. The binding must validate membership and
/// return the existing adapter with an authoritative board, caller and path.
/// The bearer authorizes a trusted host to assert `threadId` and `sessionId` metadata.
/// Use TLS at the deployment boundary outside loopback. Cancellation stops HTTP and MCP sessions.
pub async fn serve_http<F, Fut>(
    listener: TcpListener,
    token: String,
    config: StreamableHttpServerConfig,
    bind: F,
) -> anyhow::Result<()>
where
    F: Fn(ThreadId, SessionId) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<MessageBoardMcpServer, ErrorData>> + Send + 'static,
{
    anyhow::ensure!(!token.is_empty(), "MCP bearer token must not be empty");
    let mut authorization = HeaderValue::from_str(&format!("Bearer {token}"))?;
    authorization.set_sensitive(true);
    let cancellation = config.cancellation_token.clone();
    let app = Router::new()
        .nest_service(
            "/mcp",
            StreamableHttpService::new(
                move || {
                    Ok(HttpServer {
                        bind: bind.clone(),
                        invocation_scope: uuid::Uuid::now_v7().to_string(),
                    })
                },
                Arc::new(LocalSessionManager::default()),
                config,
            ),
        )
        .layer(middleware::from_fn(move |request: Request, next: Next| {
            let authorization = authorization.clone();
            async move {
                if request.headers().get(AUTHORIZATION) != Some(&authorization) {
                    return Err(StatusCode::UNAUTHORIZED);
                }
                Ok(next.run(request).await)
            }
        }));
    axum::serve(listener, app)
        .with_graceful_shutdown(cancellation.cancelled_owned())
        .await?;
    Ok(())
}

struct HttpServer<F> {
    bind: F,
    invocation_scope: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Caller {
    thread_id: ThreadId,
    session_id: SessionId,
}

impl<F, Fut> ServerHandler for HttpServer<F>
where
    F: Fn(ThreadId, SessionId) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<MessageBoardMcpServer, ErrorData>> + Send + 'static,
{
    fn get_info(&self) -> ServerConfig {
        server_info()
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        list_tools_result(
            request,
            MessageBoardTool::DISCUSSION
                .into_iter()
                .map(MessageBoardTool::spec),
        )
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let caller: Caller = serde_json::from_value(serde_json::Value::Object(
            context.meta.0.0.clone(),
        ))
        .map_err(|_| {
            ErrorData::invalid_params(
                "Calls require threadId and sessionId metadata",
                /*data*/ None,
            )
        })?;
        let mut adapter = (self.bind)(caller.thread_id, caller.session_id).await?;
        // Preserve retry identity across calls in this MCP session, scoped to the caller.
        adapter.invocation_scope = format!("{}:{}", self.invocation_scope, caller.thread_id);
        adapter.call_tool(request, context).await
    }
}
