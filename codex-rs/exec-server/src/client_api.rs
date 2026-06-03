use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;

use crate::ExecServerError;
use crate::HttpRequestParams;
use crate::HttpRequestResponse;
use crate::HttpResponseBodyStream;
use crate::NoiseChannelIdentity;
use crate::NoiseChannelPublicKey;

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

/// Registry-authorized material for one Noise rendezvous connection attempt.
///
/// Treat this as an atomic, single-use bundle. The URL authorization, executor
/// registration, pinned executor key, and harness-key authorization describe one
/// physical connection attempt and must not be mixed with values from another
/// registry response.
pub struct NoiseRendezvousConnectBundle {
    pub websocket_url: String,
    pub environment_id: String,
    pub executor_registration_id: String,
    pub executor_public_key: NoiseChannelPublicKey,
    pub harness_key_authorization: String,
}

impl std::fmt::Debug for NoiseRendezvousConnectBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseRendezvousConnectBundle")
            .field(
                "websocket_url",
                &redacted_websocket_url(&self.websocket_url),
            )
            .field("environment_id", &self.environment_id)
            .field("executor_registration_id", &self.executor_registration_id)
            .field("executor_public_key", &self.executor_public_key)
            .field("harness_key_authorization", &"<redacted>")
            .finish()
    }
}

/// Connection arguments for an authenticated Noise rendezvous exec-server.
///
/// `harness_identity` identifies the logical harness endpoint and may be reused
/// across reconnects. In contrast, callers must supply a fresh
/// [`NoiseRendezvousConnectBundle`] for each physical connection attempt.
pub struct NoiseRendezvousConnectArgs {
    pub bundle: NoiseRendezvousConnectBundle,
    pub harness_identity: NoiseChannelIdentity,
    pub client_name: String,
    pub connect_timeout: Duration,
    pub initialize_timeout: Duration,
    pub resume_session_id: Option<String>,
}

impl std::fmt::Debug for NoiseRendezvousConnectArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseRendezvousConnectArgs")
            .field("bundle", &self.bundle)
            .field("harness_identity", &"<redacted>")
            .field("client_name", &self.client_name)
            .field("connect_timeout", &self.connect_timeout)
            .field("initialize_timeout", &self.initialize_timeout)
            .field("resume_session_id", &self.resume_session_id)
            .finish()
    }
}

/// Supplies fresh registry-authorized material for Noise rendezvous connections.
///
/// Implementations preserve one endpoint-local harness identity while fetching
/// a fresh atomic connect bundle for every physical connection attempt. A
/// failed secure connection must remain a failure; providers must not fall back
/// to an unauthenticated transport.
pub trait NoiseRendezvousConnectProvider: Send + Sync {
    /// Environment ID this provider is authorized to connect to.
    fn environment_id(&self) -> &str;

    /// Returns fresh arguments for one physical connection attempt.
    fn connect_args(&self) -> BoxFuture<'_, Result<NoiseRendezvousConnectArgs, ExecServerError>>;
}

/// Shared provider used by reconnect-capable remote environments.
pub type SharedNoiseRendezvousConnectProvider = Arc<dyn NoiseRendezvousConnectProvider>;

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
#[derive(Clone)]
pub(crate) enum ExecServerTransportParams {
    WebSocketUrl {
        websocket_url: String,
        connect_timeout: Duration,
        initialize_timeout: Duration,
    },
    NoiseRendezvous {
        provider: SharedNoiseRendezvousConnectProvider,
    },
    #[allow(dead_code)]
    StdioCommand {
        command: StdioExecServerCommand,
        initialize_timeout: Duration,
    },
}

impl std::fmt::Debug for ExecServerTransportParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WebSocketUrl {
                websocket_url,
                connect_timeout,
                initialize_timeout,
            } => f
                .debug_struct("WebSocketUrl")
                .field("websocket_url", websocket_url)
                .field("connect_timeout", connect_timeout)
                .field("initialize_timeout", initialize_timeout)
                .finish(),
            Self::NoiseRendezvous { provider } => f
                .debug_struct("NoiseRendezvous")
                .field("environment_id", &provider.environment_id())
                .finish(),
            Self::StdioCommand {
                command,
                initialize_timeout,
            } => f
                .debug_struct("StdioCommand")
                .field("command", command)
                .field("initialize_timeout", initialize_timeout)
                .finish(),
        }
    }
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

/// Removes URL query and fragment data before a rendezvous URL reaches logs or errors.
pub(crate) fn redacted_websocket_url(websocket_url: &str) -> String {
    match url::Url::parse(websocket_url) {
        Ok(mut url) => {
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => "<redacted websocket url>".to_string(),
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
