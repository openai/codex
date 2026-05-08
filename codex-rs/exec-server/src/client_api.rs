use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use futures::future::BoxFuture;

use crate::ExecServerError;
use crate::HttpRequestParams;
use crate::HttpRequestResponse;
use crate::HttpResponseBodyStream;

pub(crate) const DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);

/// Connection options for any exec-server client transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecServerClientConnectOptions {
    pub client_name: String,
    pub initialize_timeout: Duration,
    pub resume_session_id: Option<String>,
}

/// WebSocket connection arguments for a remote exec-server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteExecServerConnectArgs {
    pub websocket_url: String,
    pub client_name: String,
    pub connect_timeout: Duration,
    pub initialize_timeout: Duration,
    pub resume_session_id: Option<String>,
}

/// Stdio connection arguments for a command-backed exec-server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StdioExecServerConnectArgs {
    pub command: StdioExecServerCommand,
    pub client_name: String,
    pub initialize_timeout: Duration,
    pub resume_session_id: Option<String>,
}

/// Structured process command used to start an exec-server over stdio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StdioExecServerCommand {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
}

/// Parameters used to connect to a remote exec-server environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExecServerTransportParams {
    WebSocketUrl {
        websocket_url: String,
        connect_timeout: Duration,
        initialize_timeout: Duration,
    },
    #[allow(dead_code)]
    StdioCommand {
        command: StdioExecServerCommand,
        initialize_timeout: Duration,
    },
}

impl ExecServerTransportParams {
    pub(crate) fn websocket_url(websocket_url: String) -> Self {
        Self::WebSocketUrl {
            websocket_url,
            connect_timeout: DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
            initialize_timeout: DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT,
        }
    }
}

/// Sends HTTP requests through a runtime-selected transport.
///
/// This is the HTTP capability counterpart to [`crate::ExecBackend`]. Callers
/// use it when they need environment-owned network requests but should not
/// depend on the concrete connection type or how that connection is established.
pub trait HttpClient: Send + Sync {
    /// Perform an HTTP request and buffer the response body.
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>>;

    /// Perform an HTTP request and return a streamed body handle.
    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>>;
}
