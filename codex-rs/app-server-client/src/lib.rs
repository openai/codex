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
use std::time::Duration;

pub use codex_app_server::in_process::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
pub use codex_app_server::in_process::InProcessServerEvent;
use codex_app_server::in_process::InProcessStartArgs;
use codex_app_server::in_process::LogDbLayer;
pub use codex_app_server::in_process::StateDbHandle;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::RpcError;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::NoopThreadConfigLoader;
use codex_config::RemoteThreadConfigLoader;
use codex_config::ThreadConfigLoader;
use codex_core::config::Config;
pub use codex_exec_server::EnvironmentManager;
pub use codex_exec_server::ExecServerRuntimePaths;
use codex_feedback::CodexFeedback;
use codex_protocol::protocol::SessionSource;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;
use toml::Value as TomlValue;
use tracing::warn;

/// Transitional access to core-only embedded app-server types.
///
/// New TUI behavior should prefer the app-server protocol methods. This
/// module exists so clients can remove a direct `codex-core` dependency
/// while legacy startup/config paths are migrated to RPCs.
pub mod legacy_core {
    pub use codex_core::DEFAULT_AGENTS_MD_FILENAME;
    pub use codex_core::LOCAL_AGENTS_MD_FILENAME;
    pub use codex_core::McpManager;
    pub use codex_core::check_execpolicy_for_warnings;
    pub use codex_core::format_exec_policy_error_with_source;
    pub use codex_core::grant_read_root_non_elevated;
    pub use codex_core::web_search_detail;

    pub mod config {
        pub use codex_core::config::*;

        pub mod edit {
            pub use codex_core::config::edit::*;
        }
    }

    pub mod connectors {
        pub use codex_core::connectors::*;
    }

    pub mod otel_init {
        pub use codex_core::otel_init::*;
    }

    pub mod personality_migration {
        pub use codex_core::personality_migration::*;
    }

    pub mod review_format {
        pub use codex_core::review_format::*;
    }

    pub mod review_prompts {
        pub use codex_core::review_prompts::*;
    }

    pub mod test_support {
        pub use codex_core::test_support::*;
    }

    pub mod util {
        pub use codex_core::util::*;
    }

    pub mod windows_sandbox {
        pub use codex_core::windows_sandbox::*;
    }
}

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Native app-server response or RPC error returned by an in-process request.
pub type RequestResult = std::result::Result<ClientResponse, RpcError>;

#[derive(Debug, Clone)]
pub enum AppServerEvent {
    Lagged { skipped: usize },
    ServerNotification(ServerNotification),
    ServerRequest(ServerRequest),
}

impl From<InProcessServerEvent> for AppServerEvent {
    fn from(value: InProcessServerEvent) -> Self {
        match value {
            InProcessServerEvent::Lagged { skipped } => Self::Lagged { skipped },
            InProcessServerEvent::ServerNotification(notification) => {
                Self::ServerNotification(notification)
            }
            InProcessServerEvent::ServerRequest(request) => Self::ServerRequest(request),
        }
    }
}

/// Converts the native app-server response enum into its method-specific payload.
pub trait FromClientResponse: Sized {
    fn from_client_response(
        response: ClientResponse,
    ) -> std::result::Result<Self, UnexpectedClientResponse>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnexpectedClientResponse {
    expected: &'static str,
    actual: String,
}

impl fmt::Display for UnexpectedClientResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "expected {} response, got {}",
            self.expected, self.actual
        )
    }
}

impl Error for UnexpectedClientResponse {}

macro_rules! client_response_conversions {
    ($($response:ty => $($variant:ident)|+),* $(,)?) => {
        $(
            impl FromClientResponse for $response {
                fn from_client_response(
                    response: ClientResponse,
                ) -> std::result::Result<Self, UnexpectedClientResponse> {
                    match response {
                        $(
                            ClientResponse::$variant { response, .. } => Ok(response),
                        )+
                        other => Err(UnexpectedClientResponse {
                            expected: stringify!($($variant)|+),
                            actual: other.method(),
                        }),
                    }
                }
            }
        )*
    };
}

client_response_conversions! {
    codex_app_server_protocol::InitializeResponse => Initialize,
    codex_app_server_protocol::ThreadStartResponse => ThreadStart,
    codex_app_server_protocol::ThreadResumeResponse => ThreadResume,
    codex_app_server_protocol::ThreadForkResponse => ThreadFork,
    codex_app_server_protocol::ThreadArchiveResponse => ThreadArchive,
    codex_app_server_protocol::ThreadUnsubscribeResponse => ThreadUnsubscribe,
    codex_app_server_protocol::ThreadIncrementElicitationResponse => ThreadIncrementElicitation,
    codex_app_server_protocol::ThreadDecrementElicitationResponse => ThreadDecrementElicitation,
    codex_app_server_protocol::ThreadSetNameResponse => ThreadSetName,
    codex_app_server_protocol::ThreadGoalSetResponse => ThreadGoalSet,
    codex_app_server_protocol::ThreadGoalGetResponse => ThreadGoalGet,
    codex_app_server_protocol::ThreadGoalClearResponse => ThreadGoalClear,
    codex_app_server_protocol::ThreadMetadataUpdateResponse => ThreadMetadataUpdate,
    codex_app_server_protocol::ThreadSettingsUpdateResponse => ThreadSettingsUpdate,
    codex_app_server_protocol::ThreadMemoryModeSetResponse => ThreadMemoryModeSet,
    codex_app_server_protocol::MemoryResetResponse => MemoryReset,
    codex_app_server_protocol::ThreadUnarchiveResponse => ThreadUnarchive,
    codex_app_server_protocol::ThreadCompactStartResponse => ThreadCompactStart,
    codex_app_server_protocol::ThreadShellCommandResponse => ThreadShellCommand,
    codex_app_server_protocol::ThreadApproveGuardianDeniedActionResponse
        => ThreadApproveGuardianDeniedAction,
    codex_app_server_protocol::ThreadBackgroundTerminalsCleanResponse
        => ThreadBackgroundTerminalsClean,
    codex_app_server_protocol::ThreadRollbackResponse => ThreadRollback,
    codex_app_server_protocol::ThreadListResponse => ThreadList,
    codex_app_server_protocol::ThreadSearchResponse => ThreadSearch,
    codex_app_server_protocol::ThreadLoadedListResponse => ThreadLoadedList,
    codex_app_server_protocol::ThreadReadResponse => ThreadRead,
    codex_app_server_protocol::ThreadTurnsListResponse => ThreadTurnsList,
    codex_app_server_protocol::ThreadTurnsItemsListResponse => ThreadTurnsItemsList,
    codex_app_server_protocol::ThreadInjectItemsResponse => ThreadInjectItems,
    codex_app_server_protocol::SkillsListResponse => SkillsList,
    codex_app_server_protocol::SkillsExtraRootsSetResponse => SkillsExtraRootsSet,
    codex_app_server_protocol::HooksListResponse => HooksList,
    codex_app_server_protocol::MarketplaceAddResponse => MarketplaceAdd,
    codex_app_server_protocol::MarketplaceRemoveResponse => MarketplaceRemove,
    codex_app_server_protocol::MarketplaceUpgradeResponse => MarketplaceUpgrade,
    codex_app_server_protocol::PluginListResponse => PluginList,
    codex_app_server_protocol::PluginInstalledResponse => PluginInstalled,
    codex_app_server_protocol::PluginReadResponse => PluginRead,
    codex_app_server_protocol::PluginSkillReadResponse => PluginSkillRead,
    codex_app_server_protocol::PluginShareSaveResponse => PluginShareSave,
    codex_app_server_protocol::PluginShareUpdateTargetsResponse => PluginShareUpdateTargets,
    codex_app_server_protocol::PluginShareListResponse => PluginShareList,
    codex_app_server_protocol::PluginShareCheckoutResponse => PluginShareCheckout,
    codex_app_server_protocol::PluginShareDeleteResponse => PluginShareDelete,
    codex_app_server_protocol::AppsListResponse => AppsList,
    codex_app_server_protocol::FsReadFileResponse => FsReadFile,
    codex_app_server_protocol::FsWriteFileResponse => FsWriteFile,
    codex_app_server_protocol::FsCreateDirectoryResponse => FsCreateDirectory,
    codex_app_server_protocol::FsGetMetadataResponse => FsGetMetadata,
    codex_app_server_protocol::FsReadDirectoryResponse => FsReadDirectory,
    codex_app_server_protocol::FsRemoveResponse => FsRemove,
    codex_app_server_protocol::FsCopyResponse => FsCopy,
    codex_app_server_protocol::FsWatchResponse => FsWatch,
    codex_app_server_protocol::FsUnwatchResponse => FsUnwatch,
    codex_app_server_protocol::SkillsConfigWriteResponse => SkillsConfigWrite,
    codex_app_server_protocol::PluginInstallResponse => PluginInstall,
    codex_app_server_protocol::PluginUninstallResponse => PluginUninstall,
    codex_app_server_protocol::TurnStartResponse => TurnStart,
    codex_app_server_protocol::TurnSteerResponse => TurnSteer,
    codex_app_server_protocol::TurnInterruptResponse => TurnInterrupt,
    codex_app_server_protocol::ThreadRealtimeStartResponse => ThreadRealtimeStart,
    codex_app_server_protocol::ThreadRealtimeAppendAudioResponse => ThreadRealtimeAppendAudio,
    codex_app_server_protocol::ThreadRealtimeAppendTextResponse => ThreadRealtimeAppendText,
    codex_app_server_protocol::ThreadRealtimeStopResponse => ThreadRealtimeStop,
    codex_app_server_protocol::ThreadRealtimeListVoicesResponse => ThreadRealtimeListVoices,
    codex_app_server_protocol::ReviewStartResponse => ReviewStart,
    codex_app_server_protocol::ModelListResponse => ModelList,
    codex_app_server_protocol::ModelProviderCapabilitiesReadResponse
        => ModelProviderCapabilitiesRead,
    codex_app_server_protocol::ExperimentalFeatureListResponse => ExperimentalFeatureList,
    codex_app_server_protocol::PermissionProfileListResponse => PermissionProfileList,
    codex_app_server_protocol::ExperimentalFeatureEnablementSetResponse
        => ExperimentalFeatureEnablementSet,
    codex_app_server_protocol::RemoteControlEnableResponse => RemoteControlEnable,
    codex_app_server_protocol::RemoteControlDisableResponse => RemoteControlDisable,
    codex_app_server_protocol::RemoteControlStatusReadResponse => RemoteControlStatusRead,
    codex_app_server_protocol::RemoteControlPairingStartResponse => RemoteControlPairingStart,
    codex_app_server_protocol::RemoteControlClientsListResponse => RemoteControlClientsList,
    codex_app_server_protocol::RemoteControlClientsRevokeResponse => RemoteControlClientsRevoke,
    codex_app_server_protocol::CollaborationModeListResponse => CollaborationModeList,
    codex_app_server_protocol::MockExperimentalMethodResponse => MockExperimentalMethod,
    codex_app_server_protocol::EnvironmentAddResponse => EnvironmentAdd,
    codex_app_server_protocol::McpServerOauthLoginResponse => McpServerOauthLogin,
    codex_app_server_protocol::McpServerRefreshResponse => McpServerRefresh,
    codex_app_server_protocol::ListMcpServerStatusResponse => McpServerStatusList,
    codex_app_server_protocol::McpResourceReadResponse => McpResourceRead,
    codex_app_server_protocol::McpServerToolCallResponse => McpServerToolCall,
    codex_app_server_protocol::WindowsSandboxSetupStartResponse => WindowsSandboxSetupStart,
    codex_app_server_protocol::WindowsSandboxReadinessResponse => WindowsSandboxReadiness,
    codex_app_server_protocol::LoginAccountResponse => LoginAccount,
    codex_app_server_protocol::CancelLoginAccountResponse => CancelLoginAccount,
    codex_app_server_protocol::LogoutAccountResponse => LogoutAccount,
    codex_app_server_protocol::GetAccountRateLimitsResponse => GetAccountRateLimits,
    codex_app_server_protocol::SendAddCreditsNudgeEmailResponse => SendAddCreditsNudgeEmail,
    codex_app_server_protocol::FeedbackUploadResponse => FeedbackUpload,
    codex_app_server_protocol::CommandExecResponse => OneOffCommandExec,
    codex_app_server_protocol::CommandExecWriteResponse => CommandExecWrite,
    codex_app_server_protocol::CommandExecTerminateResponse => CommandExecTerminate,
    codex_app_server_protocol::CommandExecResizeResponse => CommandExecResize,
    codex_app_server_protocol::ProcessSpawnResponse => ProcessSpawn,
    codex_app_server_protocol::ProcessWriteStdinResponse => ProcessWriteStdin,
    codex_app_server_protocol::ProcessKillResponse => ProcessKill,
    codex_app_server_protocol::ProcessResizePtyResponse => ProcessResizePty,
    codex_app_server_protocol::ConfigReadResponse => ConfigRead,
    codex_app_server_protocol::ExternalAgentConfigDetectResponse => ExternalAgentConfigDetect,
    codex_app_server_protocol::ExternalAgentConfigImportResponse => ExternalAgentConfigImport,
    codex_app_server_protocol::ConfigWriteResponse => ConfigValueWrite | ConfigBatchWrite,
    codex_app_server_protocol::ConfigRequirementsReadResponse => ConfigRequirementsRead,
    codex_app_server_protocol::GetAccountResponse => GetAccount,
    codex_app_server_protocol::GetConversationSummaryResponse => GetConversationSummary,
    codex_app_server_protocol::GitDiffToRemoteResponse => GitDiffToRemote,
    codex_app_server_protocol::GetAuthStatusResponse => GetAuthStatus,
    codex_app_server_protocol::FuzzyFileSearchResponse => FuzzyFileSearch,
    codex_app_server_protocol::FuzzyFileSearchSessionStartResponse => FuzzyFileSearchSessionStart,
    codex_app_server_protocol::FuzzyFileSearchSessionUpdateResponse => FuzzyFileSearchSessionUpdate,
    codex_app_server_protocol::FuzzyFileSearchSessionStopResponse => FuzzyFileSearchSessionStop,
}

fn event_requires_delivery(event: &InProcessServerEvent) -> bool {
    // These transcript and terminal events must remain lossless. Dropping
    // streamed assistant text or the authoritative completed item can leave
    // the TUI with permanently corrupted markdown, while dropping completion
    // notifications can leave surfaces waiting forever.
    match event {
        InProcessServerEvent::ServerNotification(notification) => {
            server_notification_requires_delivery(notification)
        }
        _ => false,
    }
}

/// Returns `true` for notifications that must survive backpressure.
///
/// Transcript events (`AgentMessageDelta`, `PlanDelta`, reasoning deltas) and
/// the authoritative `ItemCompleted` / `TurnCompleted` form the lossless tier
/// of the event stream. Dropping any of these corrupts the visible assistant
/// output or leaves surfaces waiting for a completion signal that already
/// fired. Everything else (`CommandExecutionOutputDelta`, progress, etc.) is
/// best-effort and may be dropped with only cosmetic impact.
///
pub(crate) fn server_notification_requires_delivery(notification: &ServerNotification) -> bool {
    matches!(
        notification,
        ServerNotification::TurnCompleted(_)
            | ServerNotification::ThreadSettingsUpdated(_)
            | ServerNotification::ItemCompleted(_)
            | ServerNotification::AgentMessageDelta(_)
            | ServerNotification::PlanDelta(_)
            | ServerNotification::ReasoningSummaryTextDelta(_)
            | ServerNotification::ReasoningTextDelta(_)
    )
}

/// Outcome of attempting to forward a single event to the consumer channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForwardEventResult {
    /// The event was delivered (or intentionally dropped); the stream is healthy.
    Continue,
    /// The consumer channel is closed; the caller should stop producing events.
    DisableStream,
}

/// Forwards a single in-process event to the consumer, respecting the
/// lossless/best-effort split.
///
/// Lossless events (transcript deltas, item/turn completions) block until the
/// consumer drains capacity. Best-effort events use `try_send` and increment
/// `skipped_events` on failure. When a lag marker needs to be flushed before a
/// lossless event, the flush itself blocks so the marker is never lost.
///
/// If a dropped event is a `ServerRequest`, `reject_server_request` is called
/// so the server does not wait for a response that will never come.
async fn forward_in_process_event<F>(
    event_tx: &mpsc::Sender<InProcessServerEvent>,
    skipped_events: &mut usize,
    event: InProcessServerEvent,
    mut reject_server_request: F,
) -> ForwardEventResult
where
    F: FnMut(ServerRequest),
{
    if *skipped_events > 0 {
        if event_requires_delivery(&event) {
            // Surface lag before the lossless event, but do not let the lag marker itself cause
            // us to drop the transcript/completion notification the caller is blocked on.
            if event_tx
                .send(InProcessServerEvent::Lagged {
                    skipped: *skipped_events,
                })
                .await
                .is_err()
            {
                return ForwardEventResult::DisableStream;
            }
            *skipped_events = 0;
        } else {
            match event_tx.try_send(InProcessServerEvent::Lagged {
                skipped: *skipped_events,
            }) {
                Ok(()) => {
                    *skipped_events = 0;
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    *skipped_events = skipped_events.saturating_add(1);
                    warn!("dropping in-process app-server event because consumer queue is full");
                    if let InProcessServerEvent::ServerRequest(request) = event {
                        reject_server_request(request);
                    }
                    return ForwardEventResult::Continue;
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    return ForwardEventResult::DisableStream;
                }
            }
        }
    }

    if event_requires_delivery(&event) {
        // Block until the consumer catches up for transcript/completion notifications; this
        // preserves the visible assistant output even when the queue is otherwise saturated.
        if event_tx.send(event).await.is_err() {
            return ForwardEventResult::DisableStream;
        }
        return ForwardEventResult::Continue;
    }

    match event_tx.try_send(event) {
        Ok(()) => ForwardEventResult::Continue,
        Err(mpsc::error::TrySendError::Full(event)) => {
            *skipped_events = skipped_events.saturating_add(1);
            warn!("dropping in-process app-server event because consumer queue is full");
            if let InProcessServerEvent::ServerRequest(request) = event {
                reject_server_request(request);
            }
            ForwardEventResult::Continue
        }
        Err(mpsc::error::TrySendError::Closed(_)) => ForwardEventResult::DisableStream,
    }
}

/// Layered error for [`InProcessAppServerClient::request_typed`].
#[derive(Debug)]
pub enum TypedRequestError {
    Transport {
        method: String,
        source: IoError,
    },
    Server {
        method: String,
        source: RpcError,
    },
    UnexpectedResponse {
        method: String,
        source: UnexpectedClientResponse,
    },
}

impl fmt::Display for TypedRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport { method, source } => {
                write!(f, "{method} transport error: {source}")
            }
            Self::Server { method, source } => {
                write!(
                    f,
                    "{method} failed: {} (code {})",
                    source.message, source.code
                )?;
                if let Some(data) = source.data.as_ref() {
                    write!(f, ", data: {data}")?;
                }
                Ok(())
            }
            Self::UnexpectedResponse { method, source } => {
                write!(f, "{method} response type error: {source}")
            }
        }
    }
}

impl Error for TypedRequestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport { source, .. } => Some(source),
            Self::Server { .. } => None,
            Self::UnexpectedResponse { source, .. } => Some(source),
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
    /// Whether config API paths should reject unknown config fields.
    pub strict_config: bool,
    /// Preloaded cloud config bundle provider.
    pub cloud_config_bundle: CloudConfigBundleLoader,
    /// Feedback sink used by app-server/core telemetry and logs.
    pub feedback: CodexFeedback,
    /// SQLite tracing layer used to flush recently emitted logs before feedback upload.
    pub log_db: Option<LogDbLayer>,
    /// Process-wide SQLite state handle shared with the embedded app-server.
    pub state_db: Option<StateDbHandle>,
    /// Environment manager used by core execution and filesystem operations.
    pub environment_manager: Arc<EnvironmentManager>,
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

fn configured_thread_config_loader(config: &Config) -> Arc<dyn ThreadConfigLoader> {
    match config.experimental_thread_config_endpoint.as_deref() {
        Some(endpoint) => Arc::new(RemoteThreadConfigLoader::new(endpoint)),
        None => Arc::new(NoopThreadConfigLoader),
    }
}

impl InProcessClientStartArgs {
    /// Builds initialize params from caller-provided metadata.
    pub fn initialize_params(&self) -> InitializeParams {
        let capabilities = InitializeCapabilities {
            experimental_api: self.experimental_api,
            request_attestation: false,
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

    fn into_runtime_start_args(self) -> InProcessStartArgs {
        let initialize = self.initialize_params();
        let thread_config_loader = configured_thread_config_loader(&self.config);
        InProcessStartArgs {
            arg0_paths: self.arg0_paths,
            config: self.config,
            cli_overrides: self.cli_overrides,
            loader_overrides: self.loader_overrides,
            strict_config: self.strict_config,
            cloud_config_bundle: self.cloud_config_bundle,
            thread_config_loader,
            feedback: self.feedback,
            log_db: self.log_db,
            state_db: self.state_db,
            environment_manager: self.environment_manager,
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
        response: ServerResponse,
        response_tx: oneshot::Sender<IoResult<()>>,
    },
    RejectServerRequest {
        request_id: RequestId,
        error: RpcError,
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

#[derive(Clone)]
pub struct InProcessAppServerRequestHandle {
    command_tx: mpsc::Sender<ClientCommand>,
}

#[derive(Clone)]
pub enum AppServerRequestHandle {
    InProcess(InProcessAppServerRequestHandle),
}

pub enum AppServerClient {
    InProcess(InProcessAppServerClient),
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
                                response,
                                response_tx,
                            }) => {
                                let send_result = request_sender.respond_to_server_request(response);
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
                        if let InProcessServerEvent::ServerRequest(
                            ServerRequest::ChatgptAuthTokensRefresh { request_id, .. }
                        ) = &event
                        {
                            let send_result = request_sender.fail_server_request(
                                request_id.clone(),
                                RpcError {
                                    code: -32000,
                                    message: "chatgpt auth token refresh is not supported for in-process app-server clients".to_string(),
                                    data: None,
                                },
                            );
                            if let Err(err) = send_result {
                                warn!(
                                    "failed to reject unsupported chatgpt auth token refresh request: {err}"
                                );
                            }
                            continue;
                        }

                        match forward_in_process_event(
                            &event_tx,
                            &mut skipped_events,
                            event,
                            |request| {
                                let _ = request_sender.fail_server_request(
                                    request.id().clone(),
                                    RpcError {
                                        code: -32001,
                                        message: "in-process app-server event queue is full"
                                            .to_string(),
                                        data: None,
                                    },
                                );
                            },
                        )
                        .await
                        {
                            ForwardEventResult::Continue => {}
                            ForwardEventResult::DisableStream => {
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

    pub fn request_handle(&self) -> InProcessAppServerRequestHandle {
        InProcessAppServerRequestHandle {
            command_tx: self.command_tx.clone(),
        }
    }

    /// Sends a typed client request and returns the native response enum.
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

    pub async fn request_typed<T>(&self, request: ClientRequest) -> Result<T, TypedRequestError>
    where
        T: FromClientResponse,
    {
        let method = request.method();
        let response =
            self.request(request)
                .await
                .map_err(|source| TypedRequestError::Transport {
                    method: method.clone(),
                    source,
                })?;
        let response = response.map_err(|source| TypedRequestError::Server {
            method: method.clone(),
            source,
        })?;
        T::from_client_response(response)
            .map_err(|source| TypedRequestError::UnexpectedResponse { method, source })
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
    /// The response must carry the request ID from the current client's event stream.
    pub async fn resolve_server_request(&self, response: ServerResponse) -> IoResult<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::ResolveServerRequest {
                response,
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

    /// Rejects a pending server request with an RPC error.
    pub async fn reject_server_request(
        &self,
        request_id: RequestId,
        error: RpcError,
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

impl InProcessAppServerRequestHandle {
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

    pub async fn request_typed<T>(&self, request: ClientRequest) -> Result<T, TypedRequestError>
    where
        T: FromClientResponse,
    {
        let method = request.method();
        let response =
            self.request(request)
                .await
                .map_err(|source| TypedRequestError::Transport {
                    method: method.clone(),
                    source,
                })?;
        let response = response.map_err(|source| TypedRequestError::Server {
            method: method.clone(),
            source,
        })?;
        T::from_client_response(response)
            .map_err(|source| TypedRequestError::UnexpectedResponse { method, source })
    }
}

impl AppServerRequestHandle {
    pub async fn request(&self, request: ClientRequest) -> IoResult<RequestResult> {
        match self {
            Self::InProcess(handle) => handle.request(request).await,
        }
    }

    pub async fn request_typed<T>(&self, request: ClientRequest) -> Result<T, TypedRequestError>
    where
        T: FromClientResponse,
    {
        match self {
            Self::InProcess(handle) => handle.request_typed(request).await,
        }
    }
}

impl AppServerClient {
    pub async fn request(&self, request: ClientRequest) -> IoResult<RequestResult> {
        match self {
            Self::InProcess(client) => client.request(request).await,
        }
    }

    pub async fn request_typed<T>(&self, request: ClientRequest) -> Result<T, TypedRequestError>
    where
        T: FromClientResponse,
    {
        match self {
            Self::InProcess(client) => client.request_typed(request).await,
        }
    }

    pub async fn notify(&self, notification: ClientNotification) -> IoResult<()> {
        match self {
            Self::InProcess(client) => client.notify(notification).await,
        }
    }

    pub async fn resolve_server_request(&self, response: ServerResponse) -> IoResult<()> {
        match self {
            Self::InProcess(client) => client.resolve_server_request(response).await,
        }
    }

    pub async fn reject_server_request(
        &self,
        request_id: RequestId,
        error: RpcError,
    ) -> IoResult<()> {
        match self {
            Self::InProcess(client) => client.reject_server_request(request_id, error).await,
        }
    }

    pub async fn next_event(&mut self) -> Option<AppServerEvent> {
        match self {
            Self::InProcess(client) => client.next_event().await.map(Into::into),
        }
    }

    pub async fn shutdown(self) -> IoResult<()> {
        match self {
            Self::InProcess(client) => client.shutdown().await,
        }
    }

    pub fn request_handle(&self) -> AppServerRequestHandle {
        match self {
            Self::InProcess(client) => AppServerRequestHandle::InProcess(client.request_handle()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::ConfigRequirementsReadResponse;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::SessionSource as ApiSessionSource;
    use codex_app_server_protocol::ThreadStartParams;
    use codex_app_server_protocol::ThreadStartResponse;
    use codex_core::config::ConfigBuilder;
    use codex_core::init_state_db;
    use pretty_assertions::assert_eq;
    use std::ops::Deref;
    use std::path::Path;
    use tempfile::TempDir;
    use tokio::time::Duration;
    use tokio::time::timeout;

    async fn build_test_config() -> Config {
        match ConfigBuilder::default().build().await {
            Ok(config) => config,
            Err(_) => Config::load_default_with_cli_overrides(Vec::new())
                .await
                .expect("default config should load"),
        }
    }

    async fn build_test_config_for_codex_home(codex_home: &Path) -> Config {
        match ConfigBuilder::default()
            .codex_home(codex_home.to_path_buf())
            .build()
            .await
        {
            Ok(config) => config,
            Err(_) => Config::load_default_with_cli_overrides_for_codex_home(
                codex_home.to_path_buf(),
                Vec::new(),
            )
            .await
            .expect("default config should load"),
        }
    }

    struct TestClient {
        _codex_home: TempDir,
        client: InProcessAppServerClient,
    }

    impl Deref for TestClient {
        type Target = InProcessAppServerClient;

        fn deref(&self) -> &Self::Target {
            &self.client
        }
    }

    impl TestClient {
        async fn shutdown(self) -> IoResult<()> {
            self.client.shutdown().await
        }
    }

    async fn start_test_client_with_capacity(
        session_source: SessionSource,
        channel_capacity: usize,
    ) -> TestClient {
        let codex_home = TempDir::new().expect("temp dir");
        let config = Arc::new(build_test_config_for_codex_home(codex_home.path()).await);
        let state_db = init_state_db(config.as_ref())
            .await
            .expect("state db should initialize for in-process test");
        let client = InProcessAppServerClient::start(InProcessClientStartArgs {
            arg0_paths: Arg0DispatchPaths::default(),
            config,
            cli_overrides: Vec::new(),
            loader_overrides: LoaderOverrides::default(),
            strict_config: false,
            cloud_config_bundle: CloudConfigBundleLoader::default(),
            feedback: CodexFeedback::new(),
            log_db: None,
            state_db: Some(state_db),
            environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
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
        .expect("in-process app-server client should start");

        TestClient {
            _codex_home: codex_home,
            client,
        }
    }

    async fn start_test_client(session_source: SessionSource) -> TestClient {
        start_test_client_with_capacity(session_source, DEFAULT_IN_PROCESS_CHANNEL_CAPACITY).await
    }

    fn command_execution_output_delta_notification(delta: &str) -> ServerNotification {
        ServerNotification::CommandExecutionOutputDelta(
            codex_app_server_protocol::CommandExecutionOutputDeltaNotification {
                thread_id: "thread".to_string(),
                turn_id: "turn".to_string(),
                item_id: "item".to_string(),
                delta: delta.to_string(),
            },
        )
    }

    fn agent_message_delta_notification(delta: &str) -> ServerNotification {
        ServerNotification::AgentMessageDelta(
            codex_app_server_protocol::AgentMessageDeltaNotification {
                thread_id: "thread".to_string(),
                turn_id: "turn".to_string(),
                item_id: "item".to_string(),
                delta: delta.to_string(),
            },
        )
    }

    fn item_completed_notification(text: &str) -> ServerNotification {
        ServerNotification::ItemCompleted(codex_app_server_protocol::ItemCompletedNotification {
            thread_id: "thread".to_string(),
            turn_id: "turn".to_string(),
            completed_at_ms: 0,
            item: codex_app_server_protocol::ThreadItem::AgentMessage {
                id: "item".to_string(),
                text: text.to_string(),
                phase: None,
                memory_citation: None,
            },
        })
    }

    fn turn_completed_notification() -> ServerNotification {
        ServerNotification::TurnCompleted(codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread".to_string(),
            turn: codex_app_server_protocol::Turn {
                id: "turn".to_string(),
                items_view: codex_app_server_protocol::TurnItemsView::Full,
                items: Vec::new(),
                status: codex_app_server_protocol::TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: Some(0),
                duration_ms: Some(1),
            },
        })
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
    async fn typed_request_reports_rpc_errors() {
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
            .expect_err("missing thread should return an RPC error");
        assert!(
            err.to_string().starts_with("thread/read failed:"),
            "expected method-qualified RPC failure message"
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
    async fn threads_started_via_app_server_are_visible_through_typed_requests() {
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
        let read = client
            .request_typed::<codex_app_server_protocol::ThreadReadResponse>(
                ClientRequest::ThreadRead {
                    request_id: RequestId::Integer(4),
                    params: codex_app_server_protocol::ThreadReadParams {
                        thread_id: response.thread.id.clone(),
                        include_turns: false,
                    },
                },
            )
            .await
            .expect("thread/read should return the newly started thread");
        assert_eq!(read.thread.id, response.thread.id);

        client.shutdown().await.expect("shutdown should complete");
    }

    #[tokio::test]
    async fn tiny_channel_capacity_still_supports_request_roundtrip() {
        let client =
            start_test_client_with_capacity(SessionSource::Exec, /*channel_capacity*/ 1).await;
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
    async fn forward_in_process_event_preserves_transcript_notifications_under_backpressure() {
        let (event_tx, mut event_rx) = mpsc::channel(1);
        event_tx
            .send(InProcessServerEvent::ServerNotification(
                command_execution_output_delta_notification("stdout-1"),
            ))
            .await
            .expect("initial event should enqueue");

        let mut skipped_events = 0usize;
        let result = forward_in_process_event(
            &event_tx,
            &mut skipped_events,
            InProcessServerEvent::ServerNotification(command_execution_output_delta_notification(
                "stdout-2",
            )),
            |_| {},
        )
        .await;
        assert_eq!(result, ForwardEventResult::Continue);
        assert_eq!(skipped_events, 1);

        let receive_task = tokio::spawn(async move {
            let mut events = Vec::new();
            for _ in 0..5 {
                events.push(
                    timeout(Duration::from_secs(2), event_rx.recv())
                        .await
                        .expect("event should arrive before timeout")
                        .expect("event stream should stay open"),
                );
            }
            events
        });

        for notification in [
            agent_message_delta_notification("hello"),
            item_completed_notification("hello"),
            turn_completed_notification(),
        ] {
            let result = forward_in_process_event(
                &event_tx,
                &mut skipped_events,
                InProcessServerEvent::ServerNotification(notification),
                |_| {},
            )
            .await;
            assert_eq!(result, ForwardEventResult::Continue);
        }
        assert_eq!(skipped_events, 0);

        let events = receive_task
            .await
            .expect("receiver task should join successfully");
        assert!(matches!(
            &events[0],
            InProcessServerEvent::ServerNotification(
                ServerNotification::CommandExecutionOutputDelta(notification)
            ) if notification.delta == "stdout-1"
        ));
        assert!(matches!(
            &events[1],
            InProcessServerEvent::Lagged { skipped: 1 }
        ));
        assert!(matches!(
            &events[2],
            InProcessServerEvent::ServerNotification(ServerNotification::AgentMessageDelta(
                notification
            )) if notification.delta == "hello"
        ));
        assert!(matches!(
            &events[3],
            InProcessServerEvent::ServerNotification(ServerNotification::ItemCompleted(
                notification
            )) if matches!(
                &notification.item,
                codex_app_server_protocol::ThreadItem::AgentMessage { text, .. } if text == "hello"
            )
        ));
        assert!(matches!(
            &events[4],
            InProcessServerEvent::ServerNotification(ServerNotification::TurnCompleted(
                notification
            )) if notification.turn.status == codex_app_server_protocol::TurnStatus::Completed
        ));
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
            source: RpcError {
                code: -32603,
                data: None,
                message: "internal".to_string(),
            },
        };
        assert_eq!(std::error::Error::source(&server).is_some(), false);
        assert_eq!(
            server.to_string(),
            "thread/read failed: internal (code -32603)"
        );

        let unexpected_response = TypedRequestError::UnexpectedResponse {
            method: "thread/start".to_string(),
            source: UnexpectedClientResponse {
                expected: "ThreadStart",
                actual: "config/read".to_string(),
            },
        };
        assert_eq!(
            std::error::Error::source(&unexpected_response).is_some(),
            true
        );
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

    #[test]
    fn event_requires_delivery_marks_transcript_and_terminal_events() {
        assert!(event_requires_delivery(
            &InProcessServerEvent::ServerNotification(
                codex_app_server_protocol::ServerNotification::TurnCompleted(
                    codex_app_server_protocol::TurnCompletedNotification {
                        thread_id: "thread".to_string(),
                        turn: codex_app_server_protocol::Turn {
                            id: "turn".to_string(),
                            items_view: codex_app_server_protocol::TurnItemsView::Full,
                            items: Vec::new(),
                            status: codex_app_server_protocol::TurnStatus::Completed,
                            error: None,
                            started_at: None,
                            completed_at: Some(0),
                            duration_ms: None,
                        },
                    }
                )
            )
        ));
        assert!(event_requires_delivery(
            &InProcessServerEvent::ServerNotification(
                codex_app_server_protocol::ServerNotification::AgentMessageDelta(
                    codex_app_server_protocol::AgentMessageDeltaNotification {
                        thread_id: "thread".to_string(),
                        turn_id: "turn".to_string(),
                        item_id: "item".to_string(),
                        delta: "hello".to_string(),
                    }
                )
            )
        ));
        assert!(event_requires_delivery(
            &InProcessServerEvent::ServerNotification(
                codex_app_server_protocol::ServerNotification::ItemCompleted(
                    codex_app_server_protocol::ItemCompletedNotification {
                        thread_id: "thread".to_string(),
                        turn_id: "turn".to_string(),
                        completed_at_ms: 0,
                        item: codex_app_server_protocol::ThreadItem::AgentMessage {
                            id: "item".to_string(),
                            text: "hello".to_string(),
                            phase: None,
                            memory_citation: None,
                        },
                    }
                )
            )
        ));
        assert!(!event_requires_delivery(&InProcessServerEvent::Lagged {
            skipped: 1
        }));
        assert!(!event_requires_delivery(
            &InProcessServerEvent::ServerNotification(
                codex_app_server_protocol::ServerNotification::CommandExecutionOutputDelta(
                    codex_app_server_protocol::CommandExecutionOutputDeltaNotification {
                        thread_id: "thread".to_string(),
                        turn_id: "turn".to_string(),
                        item_id: "item".to_string(),
                        delta: "stdout".to_string(),
                    }
                )
            )
        ));
    }

    #[tokio::test]
    async fn runtime_start_args_forward_environment_manager() {
        let config = Arc::new(build_test_config().await);
        let environment_manager = Arc::new(
            EnvironmentManager::create_for_tests(
                Some("ws://127.0.0.1:8765".to_string()),
                Some(
                    ExecServerRuntimePaths::new(
                        std::env::current_exe().expect("current exe"),
                        /*codex_linux_sandbox_exe*/ None,
                    )
                    .expect("runtime paths"),
                ),
            )
            .await,
        );

        let runtime_args = InProcessClientStartArgs {
            arg0_paths: Arg0DispatchPaths::default(),
            config: config.clone(),
            cli_overrides: Vec::new(),
            loader_overrides: LoaderOverrides::default(),
            strict_config: false,
            cloud_config_bundle: CloudConfigBundleLoader::default(),
            feedback: CodexFeedback::new(),
            log_db: None,
            state_db: None,
            environment_manager: environment_manager.clone(),
            config_warnings: Vec::new(),
            session_source: SessionSource::Exec,
            enable_codex_api_key_env: false,
            client_name: "codex-app-server-client-test".to_string(),
            client_version: "0.0.0-test".to_string(),
            experimental_api: true,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
        }
        .into_runtime_start_args();

        assert_eq!(runtime_args.config, config);
        assert!(Arc::ptr_eq(
            &runtime_args.environment_manager,
            &environment_manager
        ));
        assert!(
            runtime_args
                .environment_manager
                .default_environment()
                .expect("default environment")
                .is_remote()
        );
    }

    #[tokio::test]
    async fn runtime_start_args_use_remote_thread_config_loader_when_configured() {
        let mut config = build_test_config().await;
        config.experimental_thread_config_endpoint = Some("not-a-valid-endpoint".to_string());

        let runtime_args = InProcessClientStartArgs {
            arg0_paths: Arg0DispatchPaths::default(),
            config: Arc::new(config),
            cli_overrides: Vec::new(),
            loader_overrides: LoaderOverrides::default(),
            strict_config: false,
            cloud_config_bundle: CloudConfigBundleLoader::default(),
            feedback: CodexFeedback::new(),
            log_db: None,
            state_db: None,
            environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
            config_warnings: Vec::new(),
            session_source: SessionSource::Exec,
            enable_codex_api_key_env: false,
            client_name: "codex-app-server-client-test".to_string(),
            client_version: "0.0.0-test".to_string(),
            experimental_api: true,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
        }
        .into_runtime_start_args();

        let err = runtime_args
            .thread_config_loader
            .load(Default::default())
            .await
            .expect_err("configured remote loader should try to connect");
        assert_eq!(
            err.code(),
            codex_config::ThreadConfigLoadErrorCode::RequestFailed
        );
    }

    #[tokio::test]
    async fn shutdown_completes_promptly_without_retained_managers() {
        let client = start_test_client(SessionSource::Cli).await;

        timeout(Duration::from_secs(1), client.shutdown())
            .await
            .expect("shutdown should not wait for the 5s fallback timeout")
            .expect("shutdown should complete");
    }
}
