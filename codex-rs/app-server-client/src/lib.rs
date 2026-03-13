//! Shared in-process app-server client facade for CLI surfaces.
//!
//! This crate wraps [`codex_app_server::in_process`] behind a single async API
//! used by surfaces like TUI and exec. It centralizes:
//!
//! - Runtime startup and initialize-capabilities handshake.
//! - Typed caller-provided startup identity (`SessionSource` + client name).
//! - Typed and raw request/notification dispatch.
//! - Server request resolution and rejection.
//! - Event consumption with backpressure signaling ([`InProcessServerEvent::Lagged`]).
//! - Bounded graceful shutdown with abort fallback.
//!
//! The facade interposes a worker task between the caller and the underlying
//! [`InProcessClientHandle`](codex_app_server::in_process::InProcessClientHandle),
//! bridging async `mpsc` channels on both sides. Queues are bounded so overload
//! surfaces as channel-full errors rather than unbounded memory growth.

use std::error::Error;
use std::fmt;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
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
use codex_core::AuthManager;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::LoaderOverrides;
use codex_core::models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_feedback::CodexFeedback;
use codex_protocol::protocol::SessionSource;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;
use toml::Value as TomlValue;
use tracing::warn;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Raw app-server request result for typed in-process requests.
///
/// Even on the in-process path, successful responses still travel back through
/// the same JSON-RPC result envelope used by socket/stdio transports because
/// `MessageProcessor` continues to produce that shape internally.
pub type RequestResult = std::result::Result<JsonRpcResult, JSONRPCErrorError>;

static NEXT_GENERATED_REQUEST_ID: AtomicI64 = AtomicI64::new(1);

fn event_requires_delivery(event: &InProcessServerEvent) -> bool {
    // These terminal events drive surface shutdown/completion state. Dropping
    // them under backpressure can leave exec/TUI waiting forever even though
    // the underlying turn has already ended.
    match event {
        InProcessServerEvent::ServerNotification(
            codex_app_server_protocol::ServerNotification::TurnCompleted(_),
        ) => true,
        InProcessServerEvent::LegacyNotification(notification) => matches!(
            notification
                .method
                .strip_prefix("codex/event/")
                .unwrap_or(&notification.method),
            "task_complete" | "turn_aborted" | "shutdown_complete"
        ),
        _ => false,
    }
}

/// Layered error for [`InProcessAppServerClient::request_typed`].
///
/// This keeps transport failures, server-side JSON-RPC failures, and response
/// decode failures distinct so callers can decide whether to retry, surface a
/// server error, or treat the response as an internal request/response mismatch.
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
                write!(f, "{method} response decode error: {source}")
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

#[derive(Clone)]
struct SharedCoreManagers {
    // Temporary bootstrap escape hatch for embedders that still need direct
    // core handles during the in-process app-server migration. Once TUI/exec
    // stop depending on direct manager access, remove this wrapper and keep
    // manager ownership entirely inside the app-server runtime.
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
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
    /// Session source recorded in app-server thread metadata.
    pub session_source: SessionSource,
    /// Whether auth loading should honor the `CODEX_API_KEY` environment variable.
    pub enable_codex_api_key_env: bool,
    /// Client name reported during initialize.
    pub client_name: String,
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
    fn shared_core_managers(&self) -> SharedCoreManagers {
        let auth_manager = AuthManager::shared(
            self.config.codex_home.clone(),
            self.enable_codex_api_key_env,
            self.config.cli_auth_credentials_store_mode,
        );
        let thread_manager = Arc::new(ThreadManager::new(
            self.config.as_ref(),
            auth_manager.clone(),
            self.session_source.clone(),
            CollaborationModesConfig {
                default_mode_request_user_input: self
                    .config
                    .features
                    .enabled(codex_core::features::Feature::DefaultModeRequestUserInput),
            },
        ));

        SharedCoreManagers {
            auth_manager,
            thread_manager,
        }
    }

    /// Builds initialize params from caller-provided metadata.
    pub fn initialize_params(&self) -> InitializeParams {
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
                name: self.client_name.clone(),
                title: None,
                version: self.client_version.clone(),
            },
            capabilities: Some(capabilities),
        }
    }

    fn into_runtime_start_args(self, shared_core: &SharedCoreManagers) -> InProcessStartArgs {
        let initialize = self.initialize_params();
        InProcessStartArgs {
            arg0_paths: self.arg0_paths,
            config: self.config,
            cli_overrides: self.cli_overrides,
            loader_overrides: self.loader_overrides,
            cloud_requirements: self.cloud_requirements,
            auth_manager: Some(shared_core.auth_manager.clone()),
            thread_manager: Some(shared_core.thread_manager.clone()),
            feedback: self.feedback,
            config_warnings: self.config_warnings,
            session_source: self.session_source,
            enable_codex_api_key_env: self.enable_codex_api_key_env,
            initialize,
            channel_capacity: self.channel_capacity,
        }
    }
}

/// Internal command sent from public facade methods to the worker task.
///
/// Each variant carries a oneshot sender so the caller can `await` the
/// result without holding a mutable reference to the client.
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
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
}

/// Cloneable request handle for detached app-server RPCs.
#[derive(Clone)]
pub struct InProcessAppServerRequester {
    command_tx: mpsc::Sender<ClientCommand>,
}

impl InProcessAppServerClient {
    /// Starts the in-process runtime and facade worker task.
    ///
    /// The returned client is ready for requests and event consumption. If the
    /// internal event queue is saturated later, server requests are rejected
    /// with overload error instead of being silently dropped.
    pub async fn start(args: InProcessClientStartArgs) -> IoResult<Self> {
        let channel_capacity = args.channel_capacity.max(1);
        let shared_core = args.shared_core_managers();
        let mut handle =
            codex_app_server::in_process::start(args.into_runtime_start_args(&shared_core)).await?;
        let request_sender = handle.sender();
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
                                let request_sender = request_sender.clone();
                                // Request waits happen on a detached task so
                                // this loop can keep draining runtime events
                                // while the request is blocked on client input.
                                tokio::spawn(async move {
                                    let result = request_sender.request(*request).await;
                                    let _ = response_tx.send(result);
                                });
                            }
                            Some(ClientCommand::Notify {
                                notification,
                                response_tx,
                            }) => {
                                let result = request_sender.notify(notification);
                                let _ = response_tx.send(result);
                            }
                            Some(ClientCommand::ResolveServerRequest {
                                request_id,
                                result,
                                response_tx,
                            }) => {
                                let send_result =
                                    request_sender.respond_to_server_request(request_id, result);
                                let _ = response_tx.send(send_result);
                            }
                            Some(ClientCommand::RejectServerRequest {
                                request_id,
                                error,
                                response_tx,
                            }) => {
                                let send_result = request_sender.fail_server_request(request_id, error);
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
                            if event_requires_delivery(&event) {
                                // Surface lag before the terminal event, but
                                // do not let the lag marker itself cause us to
                                // drop the completion/abort notification that
                                // the caller is blocked on.
                                if event_tx
                                    .send(InProcessServerEvent::Lagged {
                                        skipped: skipped_events,
                                    })
                                    .await
                                    .is_err()
                                {
                                    event_stream_enabled = false;
                                    continue;
                                }
                                skipped_events = 0;
                            } else {
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
                                            let _ = request_sender.fail_server_request(
                                                request.id().clone(),
                                                JSONRPCErrorError {
                                                    code: -32001,
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
                        }

                        if event_requires_delivery(&event) {
                            // Block until the consumer catches up for
                            // terminal notifications; this preserves the
                            // completion signal even when the queue is
                            // otherwise saturated.
                            if event_tx.send(event).await.is_err() {
                                event_stream_enabled = false;
                            }
                            continue;
                        }

                        match event_tx.try_send(event) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(event)) => {
                                skipped_events = skipped_events.saturating_add(1);
                                warn!("dropping in-process app-server event because consumer queue is full");
                                if let InProcessServerEvent::ServerRequest(request) = event {
                                    let _ = request_sender.fail_server_request(
                                        request.id().clone(),
                                        JSONRPCErrorError {
                                            code: -32001,
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
            auth_manager: shared_core.auth_manager,
            thread_manager: shared_core.thread_manager,
        })
    }

    /// Temporary bootstrap escape hatch for embedders migrating toward RPC-only usage.
    pub fn auth_manager(&self) -> Arc<AuthManager> {
        self.auth_manager.clone()
    }

    /// Temporary bootstrap escape hatch for embedders migrating toward RPC-only usage.
    pub fn thread_manager(&self) -> Arc<ThreadManager> {
        self.thread_manager.clone()
    }

    /// Returns a cloneable request handle for detached background RPCs.
    pub fn requester(&self) -> InProcessAppServerRequester {
        InProcessAppServerRequester {
            command_tx: self.command_tx.clone(),
        }
    }

    /// Sends a typed client request and returns raw JSON-RPC result.
    ///
    /// Callers that expect a concrete response type should usually prefer
    /// [`request_typed`](Self::request_typed).
    pub async fn request(&self, request: ClientRequest) -> IoResult<RequestResult> {
        send_request(&self.command_tx, request).await
    }

    /// Sends a typed client request and decodes the successful response body.
    ///
    /// This still deserializes from a JSON value produced by app-server's
    /// JSON-RPC result envelope. Because the caller chooses `T`, `Deserialize`
    /// failures indicate an internal request/response mismatch at the call site
    /// (or an in-process bug), not transport skew from an external client.
    pub async fn request_typed<T>(&self, request: ClientRequest) -> Result<T, TypedRequestError>
    where
        T: DeserializeOwned,
    {
        request_typed(&self.command_tx, request).await
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
        let Self {
            command_tx,
            event_rx,
            worker_handle,
            auth_manager: _,
            thread_manager: _,
        } = self;
        let mut worker_handle = worker_handle;
        // Drop the caller-facing receiver before asking the worker to shut
        // down. That unblocks any pending must-deliver `event_tx.send(..)`
        // so the worker can reach `handle.shutdown()` instead of timing out
        // and getting aborted with the runtime still attached.
        drop(event_rx);
        let (response_tx, response_rx) = oneshot::channel();
        if command_tx
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
            worker_handle.abort();
            let _ = worker_handle.await;
        }
        Ok(())
    }
}

impl InProcessAppServerRequester {
    /// Sends a typed client request and returns raw JSON-RPC result.
    pub async fn request(&self, request: ClientRequest) -> IoResult<RequestResult> {
        send_request(&self.command_tx, request).await
    }

    /// Sends a typed client request and decodes the successful response body.
    pub async fn request_typed<T>(&self, request: ClientRequest) -> Result<T, TypedRequestError>
    where
        T: DeserializeOwned,
    {
        request_typed(&self.command_tx, request).await
    }

    /// Sends a typed client request using a generated request ID.
    pub async fn request_typed_with_generated_id<T, F>(
        &self,
        build: F,
    ) -> Result<T, TypedRequestError>
    where
        T: DeserializeOwned,
        F: FnOnce(RequestId) -> ClientRequest,
    {
        let request_id =
            RequestId::Integer(NEXT_GENERATED_REQUEST_ID.fetch_add(1, Ordering::Relaxed));
        request_typed(&self.command_tx, build(request_id)).await
    }
}

async fn send_request(
    command_tx: &mpsc::Sender<ClientCommand>,
    request: ClientRequest,
) -> IoResult<RequestResult> {
    let (response_tx, response_rx) = oneshot::channel();
    command_tx
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

async fn request_typed<T>(
    command_tx: &mpsc::Sender<ClientCommand>,
    request: ClientRequest,
) -> Result<T, TypedRequestError>
where
    T: DeserializeOwned,
{
    let method = request_method_name(&request);
    let response =
        send_request(command_tx, request)
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
    use codex_core::AuthManager;
    use codex_core::ThreadManager;
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
        session_source: SessionSource,
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
            session_source,
            enable_codex_api_key_env: false,
            client_name: "codex-app-server-client-test".to_string(),
            client_version: "0.0.0-test".to_string(),
            experimental_api: true,
            opt_out_notification_methods: Vec::new(),
            channel_capacity,
        })
        .await
        .expect("in-process app-server client should start")
    }

    async fn start_test_client(session_source: SessionSource) -> InProcessAppServerClient {
        start_test_client_with_capacity(session_source, DEFAULT_IN_PROCESS_CHANNEL_CAPACITY).await
    }

    #[tokio::test]
    async fn typed_request_roundtrip_works() {
        let client = start_test_client(SessionSource::Exec).await;
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
        let client = start_test_client(SessionSource::Exec).await;
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
    async fn caller_provided_session_source_is_applied() {
        for (session_source, expected_source) in [
            (SessionSource::Exec, ApiSessionSource::Exec),
            (SessionSource::Cli, ApiSessionSource::Cli),
        ] {
            let client = start_test_client(session_source).await;
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
    async fn shared_thread_manager_tracks_threads_started_via_app_server() {
        let client = start_test_client(SessionSource::Cli).await;

        let response: ThreadStartResponse = client
            .request_typed(ClientRequest::ThreadStart {
                request_id: RequestId::Integer(3),
                params: ThreadStartParams {
                    ephemeral: Some(true),
                    ..ThreadStartParams::default()
                },
            })
            .await
            .expect("thread/start should succeed");
        let created_thread_id = codex_protocol::ThreadId::from_string(&response.thread.id)
            .expect("thread id should parse");
        timeout(
            Duration::from_secs(2),
            client.thread_manager().get_thread(created_thread_id),
        )
        .await
        .expect("timed out waiting for retained thread manager to observe started thread")
        .expect("started thread should be visible through the shared thread manager");
        let thread_ids = client.thread_manager().list_thread_ids().await;
        assert!(thread_ids.contains(&created_thread_id));

        client.shutdown().await.expect("shutdown should complete");
    }

    #[tokio::test]
    async fn tiny_channel_capacity_still_supports_request_roundtrip() {
        let client = start_test_client_with_capacity(SessionSource::Exec, 1).await;
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
        let config = build_test_config().await;
        let auth_manager = AuthManager::shared(
            config.codex_home.clone(),
            false,
            config.cli_auth_credentials_store_mode,
        );
        let thread_manager = Arc::new(ThreadManager::new(
            &config,
            auth_manager.clone(),
            SessionSource::Exec,
            CollaborationModesConfig {
                default_mode_request_user_input: config
                    .features
                    .enabled(codex_core::features::Feature::DefaultModeRequestUserInput),
            },
        ));
        event_tx
            .send(InProcessServerEvent::Lagged { skipped: 3 })
            .await
            .expect("lagged marker should enqueue");
        drop(event_tx);

        let mut client = InProcessAppServerClient {
            command_tx,
            event_rx,
            worker_handle,
            auth_manager,
            thread_manager,
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

    #[test]
    fn event_requires_delivery_marks_terminal_events() {
        assert!(event_requires_delivery(
            &InProcessServerEvent::ServerNotification(
                codex_app_server_protocol::ServerNotification::TurnCompleted(
                    codex_app_server_protocol::TurnCompletedNotification {
                        thread_id: "thread".to_string(),
                        turn: codex_app_server_protocol::Turn {
                            id: "turn".to_string(),
                            items: Vec::new(),
                            status: codex_app_server_protocol::TurnStatus::Completed,
                            error: None,
                        },
                    }
                )
            )
        ));
        assert!(event_requires_delivery(
            &InProcessServerEvent::LegacyNotification(
                codex_app_server_protocol::JSONRPCNotification {
                    method: "codex/event/turn_aborted".to_string(),
                    params: None,
                }
            )
        ));
        assert!(!event_requires_delivery(&InProcessServerEvent::Lagged {
            skipped: 1
        }));
    }

    #[tokio::test]
    async fn accessors_expose_retained_shared_managers() {
        let client = start_test_client(SessionSource::Cli).await;

        assert!(
            Arc::ptr_eq(&client.auth_manager(), &client.auth_manager()),
            "auth_manager accessor should clone the retained shared manager"
        );
        assert!(
            Arc::ptr_eq(&client.thread_manager(), &client.thread_manager()),
            "thread_manager accessor should clone the retained shared manager"
        );

        client.shutdown().await.expect("shutdown should complete");
    }

    #[tokio::test]
    async fn shutdown_completes_promptly_with_retained_shared_managers() {
        let client = start_test_client(SessionSource::Cli).await;

        timeout(Duration::from_secs(1), client.shutdown())
            .await
            .expect("shutdown should not wait for the 5s fallback timeout")
            .expect("shutdown should complete");
    }
}
