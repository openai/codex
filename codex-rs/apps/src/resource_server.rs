use std::sync::Arc;

use rmcp::ServerHandler;
use rmcp::model::Implementation;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use tokio_util::sync::CancellationToken;

use crate::AppsUpstream;
use crate::CodexAppsAccessGuard;
use crate::upstream::CODEX_APPS_RESOURCE_MCP_SERVER_NAME;

#[derive(Clone)]
pub(super) struct CodexAppsResourceServer {
    pub(super) upstream: Arc<AppsUpstream>,
    pub(super) access_guard: CodexAppsAccessGuard,
    pub(super) shutdown: CancellationToken,
}

impl ServerHandler for CodexAppsResourceServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_resources().build()).with_server_info(
            Implementation::new(
                CODEX_APPS_RESOURCE_MCP_SERVER_NAME,
                env!("CARGO_PKG_VERSION"),
            ),
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        ensure_access_is_current(&self.access_guard)?;
        Ok(ListToolsResult {
            tools: Vec::new(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        ensure_access_is_current(&self.access_guard)?;
        let cancellation = context.ct.clone();
        let bridge = Arc::clone(&self.upstream.elicitation_bridge);
        let _elicitation_call = tokio::select! {
            call = bridge.begin_call(context.peer.clone()) => call.map_err(proxy_error),
            _ = cancellation.cancelled() => return Err(proxy_cancelled("resources/list")),
            _ = self.shutdown.cancelled() => return Err(proxy_shutdown()),
        }?;
        ensure_access_is_current(&self.access_guard)?;
        let upstream = tokio::select! {
            result = self.upstream.client() => result.map_err(proxy_error),
            _ = cancellation.cancelled() => return Err(proxy_cancelled("resources/list")),
            _ = self.shutdown.cancelled() => return Err(proxy_shutdown()),
        }?;
        ensure_access_is_current(&self.access_guard)?;
        tokio::select! {
            // The downstream MCP client owns the operation timeout. The proxy must not impose the
            // shorter inventory-startup deadline on ordinary resource requests.
            result = upstream.list_resources(request, /*timeout*/ None) => {
                result.map_err(proxy_error)
            }
            _ = cancellation.cancelled() => Err(proxy_cancelled("resources/list")),
            _ = self.shutdown.cancelled() => Err(proxy_shutdown()),
        }
    }

    async fn list_resource_templates(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, rmcp::ErrorData> {
        ensure_access_is_current(&self.access_guard)?;
        let cancellation = context.ct.clone();
        let bridge = Arc::clone(&self.upstream.elicitation_bridge);
        let _elicitation_call = tokio::select! {
            call = bridge.begin_call(context.peer.clone()) => call.map_err(proxy_error),
            _ = cancellation.cancelled() => return Err(proxy_cancelled("resources/templates/list")),
            _ = self.shutdown.cancelled() => return Err(proxy_shutdown()),
        }?;
        ensure_access_is_current(&self.access_guard)?;
        let upstream = tokio::select! {
            result = self.upstream.client() => result.map_err(proxy_error),
            _ = cancellation.cancelled() => return Err(proxy_cancelled("resources/templates/list")),
            _ = self.shutdown.cancelled() => return Err(proxy_shutdown()),
        }?;
        ensure_access_is_current(&self.access_guard)?;
        tokio::select! {
            result = upstream.list_resource_templates(request, /*timeout*/ None) => {
                result.map_err(proxy_error)
            }
            _ = cancellation.cancelled() => Err(proxy_cancelled("resources/templates/list")),
            _ = self.shutdown.cancelled() => Err(proxy_shutdown()),
        }
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        proxy_read_resource(
            &self.upstream,
            &self.access_guard,
            &self.shutdown,
            request,
            context,
        )
        .await
    }
}

impl CodexAppsResourceServer {
    pub(super) fn for_http_session(&self) -> Self {
        Self {
            upstream: self.upstream.fork(),
            access_guard: self.access_guard.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

pub(super) async fn proxy_read_resource(
    upstream: &Arc<AppsUpstream>,
    access_guard: &CodexAppsAccessGuard,
    shutdown: &CancellationToken,
    request: ReadResourceRequestParams,
    context: RequestContext<RoleServer>,
) -> Result<ReadResourceResult, rmcp::ErrorData> {
    ensure_access_is_current(access_guard)?;
    let cancellation = context.ct.clone();
    let _elicitation_call = tokio::select! {
        call = upstream.elicitation_bridge.begin_call(context.peer.clone()) => call.map_err(proxy_error),
        _ = cancellation.cancelled() => return Err(proxy_cancelled("resources/read")),
        _ = shutdown.cancelled() => return Err(proxy_shutdown()),
    }?;
    ensure_access_is_current(access_guard)?;
    let upstream = tokio::select! {
        result = upstream.client() => result.map_err(proxy_error),
        _ = cancellation.cancelled() => return Err(proxy_cancelled("resources/read")),
        _ = shutdown.cancelled() => return Err(proxy_shutdown()),
    }?;
    ensure_access_is_current(access_guard)?;
    tokio::select! {
        result = upstream.read_resource(request, /*timeout*/ None) => result.map_err(proxy_error),
        _ = cancellation.cancelled() => Err(proxy_cancelled("resources/read")),
        _ = shutdown.cancelled() => Err(proxy_shutdown()),
    }
}

fn ensure_access_is_current(access_guard: &CodexAppsAccessGuard) -> Result<(), rmcp::ErrorData> {
    access_guard.is_current().then_some(()).ok_or_else(|| {
        rmcp::ErrorData::internal_error("Codex Apps credentials are no longer current", None)
    })
}

pub(super) fn proxy_error(error: anyhow::Error) -> rmcp::ErrorData {
    if let Some(error) = codex_rmcp_client::mcp_error_data(&error) {
        return error.clone();
    }
    if let Some(error) = error
        .chain()
        .find_map(|source| source.downcast_ref::<rmcp::ErrorData>().cloned())
    {
        return error;
    }
    rmcp::ErrorData::internal_error(error.to_string(), None)
}

pub(super) fn proxy_cancelled(method: &str) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(format!("Codex Apps MCP `{method}` was cancelled"), None)
}

pub(super) fn proxy_shutdown() -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error("Codex Apps MCP server is shutting down", None)
}
