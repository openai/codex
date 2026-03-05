//! Shared in-process app-server client facade for CLI surfaces.
//!
//! This crate wraps `codex_app_server::in_process` behind a single async API
//! used by surfaces like TUI and exec. It centralizes:
//! - runtime startup and initialize capabilities
//! - surface-to-session-source policy
//! - request/notification dispatch
//! - server request resolution and rejection
//! - event consumption and shutdown behavior
//!
//! The facade uses bounded channels between caller and worker task so overload
//! is visible instead of unbounded memory growth.

use std::error::Error;
use std::fmt;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::sync::Arc;
use std::time::Duration;

pub use codex_app_server::in_process::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
pub use codex_app_server::in_process::InProcessServerEvent;
use codex_app_server::in_process::InProcessStartArgs;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Result as JsonRpcResult;
use codex_arg0::Arg0DispatchPaths;
use codex_core::config::Config;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::LoaderOverrides;
use codex_feedback::CodexFeedback;
use codex_protocol::protocol::SessionSource;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;
use toml::Value as TomlValue;
use tracing::warn;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
/// JSON-RPC overload code used by app-server when bounded queues are saturated.
const OVERLOADED_ERROR_CODE: i64 = -32001;

/// Raw app-server request result for typed in-process requests.
///
/// Even on the in-process path, successful responses still travel back through
/// the same JSON-RPC result envelope used by socket/stdio transports because
/// `MessageProcessor` continues to produce that shape internally.
pub type RequestResult = std::result::Result<JsonRpcResult, JSONRPCErrorError>;

/// Layered error for [`InProcessAppServerClient::request_typed`].
///
/// This keeps transport failures, server-side JSON-RPC failures, and response
/// shape mismatches distinct so callers can decide whether to retry, surface a
/// server error, or treat the response as protocol drift.
#[derive(Debug)]
pub enum TypedRequestError {
    Transport {
        method: String,
        source: IoError,
    },
    Server {
        method: String,
        source: JSONRPCErrorError,
    },
    Deserialize {
        method: String,
        source: serde_json::Error,
    },
}

impl fmt::Display for TypedRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport { method, source } => {
                write!(f, "{method} transport error: {source}")
            }
            Self::Server { method, source } => {
                write!(f, "{method} failed: {}", source.message)
            }
            Self::Deserialize { method, source } => {
                write!(f, "{method} invalid response payload: {source}")
            }
        }
    }
}

impl Error for TypedRequestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport { source, .. } => Some(source),
            Self::Server { .. } => None,
            Self::Deserialize { source, .. } => Some(source),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientSurface {
    /// Non-interactive execution surface.
    Exec,
    /// Interactive terminal UI surface.
    Tui,
}

/// Maps facade surface identity to app-server `SessionSource`.
///
/// `ClientSurface::Tui` intentionally maps to `SessionSource::Cli` because the
/// TUI is the interactive CLI surface from the server's perspective.
pub fn session_source_for_surface(surface: ClientSurface) -> SessionSource {
    match surface {
        ClientSurface::Exec => SessionSource::Exec,
        ClientSurface::Tui => SessionSource::Cli,
    }
}

impl ClientSurface {
    fn default_client_name(self) -> &'static str {
        match self {
            ClientSurface::Exec => "codex-exec",
            ClientSurface::Tui => "codex-tui",
        }
    }
}

#[derive(Clone)]
pub struct InProcessClientStartArgs {
    /// Resolved argv0 dispatch paths used by command execution internals.
    pub arg0_paths: Arg0DispatchPaths,
    /// Shared config used to initialize app-server runtime.
    pub config: Arc<Config>,
    /// CLI config overrides that are already parsed into TOML values.
    pub cli_overrides: Vec<(String, TomlValue)>,
    /// Loader override knobs used by config API paths.
    pub loader_overrides: LoaderOverrides,
    /// Preloaded cloud requirements provider.
    pub cloud_requirements: CloudRequirementsLoader,
    /// Feedback sink used by app-server/core telemetry and logs.
    pub feedback: CodexFeedback,
    /// Startup warnings emitted after initialize succeeds.
    pub config_warnings: Vec<ConfigWarningNotification>,
    /// Surface identity that drives session source and default client name.
    pub surface: ClientSurface,
    /// Optional explicit client name; falls back to surface default.
    pub client_name: Option<String>,
    /// Client version reported during initialize.
    pub client_version: String,
    /// Whether experimental APIs are requested at initialize time.
    pub experimental_api: bool,
    /// Notification methods this client opts out of receiving.
    pub opt_out_notification_methods: Vec<String>,
    /// Queue capacity for command/event channels (clamped to at least 1).
    pub channel_capacity: usize,
}

impl InProcessClientStartArgs {
    /// Builds initialize params from surface and caller-provided metadata.
    ///
    /// This keeps the initialize handshake policy in one place so TUI and exec
    /// do not drift on client naming, experimental opt-in, or notification
    /// suppression.
    pub fn initialize_params(&self) -> InitializeParams {
        let client_name = self
            .client_name
            .clone()
            .unwrap_or_else(|| self.surface.default_client_name().to_string());
        let capabilities = InitializeCapabilities {
            experimental_api: self.experimental_api,
            opt_out_notification_methods: if self.opt_out_notification_methods.is_empty() {
                None
            } else {
                Some(self.opt_out_notification_methods.clone())
            },
        };

        InitializeParams {
            client_info: ClientInfo {
                name: client_name,
                title: None,
                version: self.client_version.clone(),
            },
            capabilities: Some(capabilities),
        }
    }

    fn into_runtime_start_args(self) -> InProcessStartArgs {
        let initialize = self.initialize_params();
        InProcessStartArgs {
            arg0_paths: self.arg0_paths,
            config: self.config,
            cli_overrides: self.cli_overrides,
            loader_overrides: self.loader_overrides,
            cloud_requirements: self.cloud_requirements,
            feedback: self.feedback,
            config_warnings: self.config_warnings,
            session_source: session_source_for_surface(self.surface),
            initialize,
            channel_capacity: self.channel_capacity,
        }
    }
}

enum ClientCommand {
    Request {
        request: Box<ClientRequest>,
        response_tx: oneshot::Sender<IoResult<RequestResult>>,
    },
    Notify {
        notification: ClientNotification,
        response_tx: oneshot::Sender<IoResult<()>>,
    },
    ResolveServerRequest {
        request_id: RequestId,
        result: JsonRpcResult,
        response_tx: oneshot::Sender<IoResult<()>>,
    },
    RejectServerRequest {
        request_id: RequestId,
        error: JSONRPCErrorError,
        response_tx: oneshot::Sender<IoResult<()>>,
    },
    Shutdown {
        response_tx: oneshot::Sender<IoResult<()>>,
    },
}

/// Async facade over the in-process app-server runtime.
///
/// This type owns a worker task that bridges between:
/// - caller-facing async `mpsc` channels used by TUI/exec
/// - [`codex_app_server::in_process::InProcessClientHandle`], which speaks to
///   the embedded `MessageProcessor`
///
/// The facade intentionally preserves the server's request/notification/event
/// model instead of exposing direct core runtime handles. That keeps in-process
/// callers aligned with app-server behavior while still avoiding a process
/// boundary.
pub struct InProcessAppServerClient {
    command_tx: mpsc::Sender<ClientCommand>,
    event_rx: mpsc::Receiver<InProcessServerEvent>,
    worker_handle: tokio::task::JoinHandle<()>,
}

impl InProcessAppServerClient {
    /// Starts the in-process runtime and facade worker task.
    ///
    /// The returned client is ready for requests and event consumption. If the
    /// internal event queue is saturated later, server requests are rejected
    /// with overload error instead of being silently dropped.
    pub async fn start(args: InProcessClientStartArgs) -> IoResult<Self> {
        let channel_capacity = args.channel_capacity.max(1);
        let mut handle =
            codex_app_server::in_process::start(args.into_runtime_start_args()).await?;
        let (command_tx, mut command_rx) = mpsc::channel::<ClientCommand>(channel_capacity);
        let (event_tx, event_rx) = mpsc::channel::<InProcessServerEvent>(channel_capacity);

        let worker_handle = tokio::spawn(async move {
            let mut event_stream_enabled = true;
            let mut skipped_events = 0usize;
            loop {
                tokio::select! {
                    command = command_rx.recv() => {
                        match command {
                            Some(ClientCommand::Request { request, response_tx }) => {
                                let result = handle.request(*request).await;
                                let _ = response_tx.send(result);
                            }
                            Some(ClientCommand::Notify {
                                notification,
                                response_tx,
                            }) => {
                                let result = handle.notify(notification);
                                let _ = response_tx.send(result);
                            }
                            Some(ClientCommand::ResolveServerRequest {
                                request_id,
                                result,
                                response_tx,
                            }) => {
                                let send_result = handle.respond_to_server_request(request_id, result);
                                let _ = response_tx.send(send_result);
                            }
                            Some(ClientCommand::RejectServerRequest {
                                request_id,
                                error,
                                response_tx,
                            }) => {
                                let send_result = handle.fail_server_request(request_id, error);
                                let _ = response_tx.send(send_result);
                            }
                            Some(ClientCommand::Shutdown { response_tx }) => {
                                let shutdown_result = handle.shutdown().await;
                                let _ = response_tx.send(shutdown_result);
                                break;
                            }
                            None => {
                                let _ = handle.shutdown().await;
                                break;
                            }
                        }
                    }
                    event = handle.next_event(), if event_stream_enabled => {
                        let Some(event) = event else {
                            break;
                        };

                        if skipped_events > 0 {
                            match event_tx.try_send(InProcessServerEvent::Lagged {
                                skipped: skipped_events,
                            }) {
                                Ok(()) => {
                                    skipped_events = 0;
                                }
                                Err(mpsc::error::TrySendError::Full(_)) => {
                                    skipped_events = skipped_events.saturating_add(1);
                                    warn!(
                                        "dropping in-process app-server event because consumer queue is full"
                                    );
                                    if let InProcessServerEvent::ServerRequest(request) = event {
                                        let _ = handle.fail_server_request(
                                            request.id().clone(),
                                            JSONRPCErrorError {
                                                code: OVERLOADED_ERROR_CODE,
                                                message: "in-process app-server event queue is full".to_string(),
                                                data: None,
                                            },
                                        );
                                    }
                                    continue;
                                }
                                Err(mpsc::error::TrySendError::Closed(_)) => {
                                    event_stream_enabled = false;
                                    continue;
                                }
                            }
                        }

                        match event_tx.try_send(event) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(event)) => {
                                skipped_events = skipped_events.saturating_add(1);
                                warn!("dropping in-process app-server event because consumer queue is full");
                                if let InProcessServerEvent::ServerRequest(request) = event {
                                    let _ = handle.fail_server_request(
                                        request.id().clone(),
                                        JSONRPCErrorError {
                                            code: OVERLOADED_ERROR_CODE,
                                            message: "in-process app-server event queue is full".to_string(),
                                            data: None,
                                        },
                                    );
                                }
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                event_stream_enabled = false;
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            command_tx,
            event_rx,
            worker_handle,
        })
    }

    /// Sends a typed client request and returns raw JSON-RPC result.
    ///
    /// Callers that expect a concrete response type should usually prefer
    /// [`request_typed`](Self::request_typed).
    pub async fn request(&self, request: ClientRequest) -> IoResult<RequestResult> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::Request {
                request: Box::new(request),
                response_tx,
            })
            .await
            .map_err(|_| {
                IoError::new(
                    ErrorKind::BrokenPipe,
                    "in-process app-server worker channel is closed",
                )
            })?;
        response_rx.await.map_err(|_| {
            IoError::new(
                ErrorKind::BrokenPipe,
                "in-process app-server request channel is closed",
            )
        })?
    }

    /// Sends a typed client request and decodes the successful response body.
    ///
    /// This still deserializes from a JSON value produced by app-server's
    /// JSON-RPC result envelope. Callers should treat `Deserialize` failures as
    /// a protocol-shape mismatch rather than a transport problem.
    pub async fn request_typed<T>(&self, request: ClientRequest) -> Result<T, TypedRequestError>
    where
        T: DeserializeOwned,
    {
        let method = request_method_name(&request);
        let response =
            self.request(request)
                .await
                .map_err(|source| TypedRequestError::Transport {
                    method: method.clone(),
                    source,
                })?;
        let result = response.map_err(|source| TypedRequestError::Server {
            method: method.clone(),
            source,
        })?;
        serde_json::from_value(result)
            .map_err(|source| TypedRequestError::Deserialize { method, source })
    }

    /// Sends a typed client notification.
    pub async fn notify(&self, notification: ClientNotification) -> IoResult<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::Notify {
                notification,
                response_tx,
            })
            .await
            .map_err(|_| {
                IoError::new(
                    ErrorKind::BrokenPipe,
                    "in-process app-server worker channel is closed",
                )
            })?;
        response_rx.await.map_err(|_| {
            IoError::new(
                ErrorKind::BrokenPipe,
                "in-process app-server notify channel is closed",
            )
        })?
    }

    /// Resolves a pending server request.
    ///
    /// This should only be called with request IDs obtained from the current
    /// client's event stream.
    pub async fn resolve_server_request(
        &self,
        request_id: RequestId,
        result: JsonRpcResult,
    ) -> IoResult<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::ResolveServerRequest {
                request_id,
                result,
                response_tx,
            })
            .await
            .map_err(|_| {
                IoError::new(
                    ErrorKind::BrokenPipe,
                    "in-process app-server worker channel is closed",
                )
            })?;
        response_rx.await.map_err(|_| {
            IoError::new(
                ErrorKind::BrokenPipe,
                "in-process app-server resolve channel is closed",
            )
        })?
    }

    /// Rejects a pending server request with JSON-RPC error payload.
    pub async fn reject_server_request(
        &self,
        request_id: RequestId,
        error: JSONRPCErrorError,
    ) -> IoResult<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::RejectServerRequest {
                request_id,
                error,
                response_tx,
            })
            .await
            .map_err(|_| {
                IoError::new(
                    ErrorKind::BrokenPipe,
                    "in-process app-server worker channel is closed",
                )
            })?;
        response_rx.await.map_err(|_| {
            IoError::new(
                ErrorKind::BrokenPipe,
                "in-process app-server reject channel is closed",
            )
        })?
    }

    /// Returns the next in-process event, or `None` when worker exits.
    ///
    /// Callers are expected to drain this stream promptly. If they fall behind,
    /// the worker emits [`InProcessServerEvent::Lagged`] markers and may reject
    /// pending server requests rather than letting approval flows hang.
    pub async fn next_event(&mut self) -> Option<InProcessServerEvent> {
        self.event_rx.recv().await
    }

    /// Shuts down worker and in-process runtime with bounded wait.
    ///
    /// If graceful shutdown exceeds timeout, the worker task is aborted to
    /// avoid leaking background tasks in embedding callers.
    pub async fn shutdown(self) -> IoResult<()> {
        let mut worker_handle = self.worker_handle;
        let (response_tx, response_rx) = oneshot::channel();
        if self
            .command_tx
            .send(ClientCommand::Shutdown { response_tx })
            .await
            .is_ok()
            && let Ok(command_result) = timeout(SHUTDOWN_TIMEOUT, response_rx).await
        {
            command_result.map_err(|_| {
                IoError::new(
                    ErrorKind::BrokenPipe,
                    "in-process app-server shutdown channel is closed",
                )
            })??;
        }

        if let Err(_elapsed) = timeout(SHUTDOWN_TIMEOUT, &mut worker_handle).await {
            warn!("in-process app-server worker did not shut down within timeout; aborting");
            worker_handle.abort();
            let _ = worker_handle.await;
        }
        Ok(())
    }
}

/// Extracts the JSON-RPC method name for diagnostics without extending the
/// protocol crate with in-process-only helpers.
fn request_method_name(request: &ClientRequest) -> String {
    serde_json::to_value(request)
        .ok()
        .and_then(|value| {
            value
                .get("method")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "<unknown>".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::ConfigRequirementsReadResponse;
    use codex_app_server_protocol::SessionSource as ApiSessionSource;
    use codex_app_server_protocol::ThreadStartParams;
    use codex_app_server_protocol::ThreadStartResponse;
    use codex_core::config::ConfigBuilder;
    use pretty_assertions::assert_eq;
    use tokio::time::Duration;
    use tokio::time::timeout;

    async fn build_test_config() -> Config {
        match ConfigBuilder::default().build().await {
            Ok(config) => config,
            Err(_) => Config::load_default_with_cli_overrides(Vec::new())
                .expect("default config should load"),
        }
    }

    async fn start_test_client_with_capacity(
        surface: ClientSurface,
        channel_capacity: usize,
    ) -> InProcessAppServerClient {
        InProcessAppServerClient::start(InProcessClientStartArgs {
            arg0_paths: Arg0DispatchPaths::default(),
            config: Arc::new(build_test_config().await),
            cli_overrides: Vec::new(),
            loader_overrides: LoaderOverrides::default(),
            cloud_requirements: CloudRequirementsLoader::default(),
            feedback: CodexFeedback::new(),
            config_warnings: Vec::new(),
            surface,
            client_name: Some("codex-app-server-client-test".to_string()),
            client_version: "0.0.0-test".to_string(),
            experimental_api: true,
            opt_out_notification_methods: Vec::new(),
            channel_capacity,
        })
        .await
        .expect("in-process app-server client should start")
    }

    async fn start_test_client(surface: ClientSurface) -> InProcessAppServerClient {
        start_test_client_with_capacity(surface, DEFAULT_IN_PROCESS_CHANNEL_CAPACITY).await
    }

    #[tokio::test]
    async fn typed_request_roundtrip_works() {
        let client = start_test_client(ClientSurface::Exec).await;
        let _response: ConfigRequirementsReadResponse = client
            .request_typed(ClientRequest::ConfigRequirementsRead {
                request_id: RequestId::Integer(1),
                params: None,
            })
            .await
            .expect("typed request should succeed");
        client.shutdown().await.expect("shutdown should complete");
    }

    #[tokio::test]
    async fn typed_request_reports_json_rpc_errors() {
        let client = start_test_client(ClientSurface::Exec).await;
        let err = client
            .request_typed::<ConfigRequirementsReadResponse>(ClientRequest::ThreadRead {
                request_id: RequestId::Integer(99),
                params: codex_app_server_protocol::ThreadReadParams {
                    thread_id: "missing-thread".to_string(),
                    include_turns: false,
                },
            })
            .await
            .expect_err("missing thread should return a JSON-RPC error");
        assert!(
            err.to_string().starts_with("thread/read failed:"),
            "expected method-qualified JSON-RPC failure message"
        );
        client.shutdown().await.expect("shutdown should complete");
    }

    #[tokio::test]
    async fn surface_to_session_source_mapping_is_applied() {
        for (surface, expected_source) in [
            (ClientSurface::Exec, ApiSessionSource::Exec),
            (ClientSurface::Tui, ApiSessionSource::Cli),
        ] {
            let client = start_test_client(surface).await;
            let parsed: ThreadStartResponse = client
                .request_typed(ClientRequest::ThreadStart {
                    request_id: RequestId::Integer(2),
                    params: ThreadStartParams {
                        ephemeral: Some(true),
                        ..ThreadStartParams::default()
                    },
                })
                .await
                .expect("thread/start should succeed");
            assert_eq!(parsed.thread.source, expected_source);
            client.shutdown().await.expect("shutdown should complete");
        }
    }

    #[tokio::test]
    async fn tiny_channel_capacity_still_supports_request_roundtrip() {
        let client = start_test_client_with_capacity(ClientSurface::Exec, 1).await;
        let _response: ConfigRequirementsReadResponse = client
            .request_typed(ClientRequest::ConfigRequirementsRead {
                request_id: RequestId::Integer(1),
                params: None,
            })
            .await
            .expect("typed request should succeed");
        client.shutdown().await.expect("shutdown should complete");
    }

    #[test]
    fn typed_request_error_exposes_sources() {
        let transport = TypedRequestError::Transport {
            method: "config/read".to_string(),
            source: IoError::new(ErrorKind::BrokenPipe, "closed"),
        };
        assert_eq!(std::error::Error::source(&transport).is_some(), true);

        let server = TypedRequestError::Server {
            method: "thread/read".to_string(),
            source: JSONRPCErrorError {
                code: -32603,
                data: None,
                message: "internal".to_string(),
            },
        };
        assert_eq!(std::error::Error::source(&server).is_some(), false);

        let deserialize = TypedRequestError::Deserialize {
            method: "thread/start".to_string(),
            source: serde_json::from_str::<u32>("\"nope\"")
                .expect_err("invalid integer should return deserialize error"),
        };
        assert_eq!(std::error::Error::source(&deserialize).is_some(), true);
    }

    #[tokio::test]
    async fn next_event_surfaces_lagged_markers() {
        let (command_tx, _command_rx) = mpsc::channel(1);
        let (event_tx, event_rx) = mpsc::channel(1);
        let worker_handle = tokio::spawn(async {});
        event_tx
            .send(InProcessServerEvent::Lagged { skipped: 3 })
            .await
            .expect("lagged marker should enqueue");
        drop(event_tx);

        let mut client = InProcessAppServerClient {
            command_tx,
            event_rx,
            worker_handle,
        };

        let event = timeout(Duration::from_secs(2), client.next_event())
            .await
            .expect("lagged marker should arrive before timeout");
        assert!(matches!(
            event,
            Some(InProcessServerEvent::Lagged { skipped: 3 })
        ));

        client.shutdown().await.expect("shutdown should complete");
    }
}
