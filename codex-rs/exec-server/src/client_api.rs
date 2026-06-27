use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;

use crate::EnvironmentRegistryTransportPolicy;
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

/// Registry-authorized bundle and transport policy for one physical connection.
pub struct NoiseRendezvousConnectAttempt {
    pub bundle: NoiseRendezvousConnectBundle,
    pub transport_policy: EnvironmentRegistryTransportPolicy,
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

/// Policy-aware connection arguments for one authenticated Noise rendezvous
/// attempt.
pub struct NoiseRendezvousConnectAttemptArgs {
    pub attempt: NoiseRendezvousConnectAttempt,
    pub harness_identity: NoiseChannelIdentity,
    pub client_name: String,
    pub connect_timeout: Duration,
    pub initialize_timeout: Duration,
    pub resume_session_id: Option<String>,
}

impl From<NoiseRendezvousConnectArgs> for NoiseRendezvousConnectAttemptArgs {
    fn from(args: NoiseRendezvousConnectArgs) -> Self {
        Self {
            attempt: NoiseRendezvousConnectAttempt {
                bundle: args.bundle,
                transport_policy: EnvironmentRegistryTransportPolicy::default(),
            },
            harness_identity: args.harness_identity,
            client_name: args.client_name,
            connect_timeout: args.connect_timeout,
            initialize_timeout: args.initialize_timeout,
            resume_session_id: args.resume_session_id,
        }
    }
}

/// Supplies fresh registry-authorized material for Noise rendezvous connections.
pub trait NoiseRendezvousConnectProvider: Send + Sync {
    /// Fetch a bundle authorizing this harness key for one physical connection.
    fn connect_bundle(
        &self,
        harness_public_key: NoiseChannelPublicKey,
    ) -> BoxFuture<'_, Result<NoiseRendezvousConnectBundle, ExecServerError>>;

    /// Fetch a policy-aware connection attempt.
    ///
    /// Existing providers remain source-compatible and use the legacy/off
    /// transport policy until they explicitly override this method.
    fn connect_attempt(
        &self,
        harness_public_key: NoiseChannelPublicKey,
    ) -> BoxFuture<'_, Result<NoiseRendezvousConnectAttempt, ExecServerError>> {
        Box::pin(async move {
            let bundle = self.connect_bundle(harness_public_key).await?;
            Ok(NoiseRendezvousConnectAttempt {
                bundle,
                transport_policy: EnvironmentRegistryTransportPolicy::default(),
            })
        })
    }
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
#[derive(Clone)]
pub(crate) enum ExecServerTransportParams {
    WebSocketUrl {
        websocket_url: String,
        connect_timeout: Duration,
        initialize_timeout: Duration,
    },
    NoiseRendezvous {
        provider: Arc<dyn NoiseRendezvousConnectProvider>,
        identity: NoiseChannelIdentity,
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
            Self::NoiseRendezvous { .. } => {
                f.debug_struct("NoiseRendezvous").finish_non_exhaustive()
            }
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
    pub(crate) fn websocket_url(websocket_url: String, connect_timeout: Duration) -> Self {
        Self::WebSocketUrl {
            websocket_url,
            connect_timeout,
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
