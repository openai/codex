use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::codex_message_processor::CodexMessageProcessor;
use crate::codex_message_processor::CodexMessageProcessorArgs;
use crate::config_api::ConfigApi;
use crate::config_manager::ConfigManager;
use crate::device_key_api::DeviceKeyApi;
use crate::error_code::invalid_request;
use crate::external_agent_config_api::ExternalAgentConfigApi;
use crate::fs_api::FsApi;
use crate::fs_watch::FsWatchManager;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::RequestContext;
use crate::transport::AppServerTransport;
use crate::transport::ConnectionOrigin;
use crate::transport::RemoteControlHandle;
use async_trait::async_trait;
use axum::http::HeaderValue;
use codex_analytics::AnalyticsEventsClient;
use codex_analytics::AppServerRpcTransport;
use codex_app_server_protocol::AppListUpdatedNotification;
use codex_app_server_protocol::AuthMode as LoginAuthMode;
use codex_app_server_protocol::ChatgptAuthTokensRefreshParams;
use codex_app_server_protocol::ChatgptAuthTokensRefreshReason;
use codex_app_server_protocol::ChatgptAuthTokensRefreshResponse;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::DeviceKeyCreateParams;
use codex_app_server_protocol::DeviceKeyPublicParams;
use codex_app_server_protocol::DeviceKeySignParams;
use codex_app_server_protocol::ExperimentalApi;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetParams;
use codex_app_server_protocol::ExternalAgentConfigImportCompletedNotification;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequestPayload;
use codex_app_server_protocol::experimental_required_message;
use codex_arg0::Arg0DispatchPaths;
use codex_chatgpt::connectors;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_exec_server::EnvironmentManager;
use codex_features::Feature;
use codex_feedback::CodexFeedback;
use codex_login::AuthManager;
use codex_login::auth::ExternalAuth;
use codex_login::auth::ExternalAuthRefreshContext;
use codex_login::auth::ExternalAuthRefreshReason;
use codex_login::auth::ExternalAuthTokens;
use codex_login::default_client::SetOriginatorError;
use codex_login::default_client::USER_AGENT_SUFFIX;
use codex_login::default_client::get_codex_user_agent;
use codex_login::default_client::set_default_client_residency_requirement;
use codex_login::default_client::set_default_originator;
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::W3cTraceContext;
use codex_state::log_db::LogDbLayer;
use futures::FutureExt;
use tokio::sync::broadcast;
use tokio::sync::watch;
use tokio::time::Duration;
use tokio::time::timeout;
use tracing::Instrument;

const EXTERNAL_AUTH_REFRESH_TIMEOUT: Duration = Duration::from_secs(10);
#[derive(Clone)]
struct ExternalAuthRefreshBridge {
    outgoing: Arc<OutgoingMessageSender>,
}

impl ExternalAuthRefreshBridge {
    fn map_reason(reason: ExternalAuthRefreshReason) -> ChatgptAuthTokensRefreshReason {
        match reason {
            ExternalAuthRefreshReason::Unauthorized => ChatgptAuthTokensRefreshReason::Unauthorized,
        }
    }
}

#[async_trait]
impl ExternalAuth for ExternalAuthRefreshBridge {
    fn auth_mode(&self) -> LoginAuthMode {
        LoginAuthMode::Chatgpt
    }

    async fn refresh(
        &self,
        context: ExternalAuthRefreshContext,
    ) -> std::io::Result<ExternalAuthTokens> {
        let params = ChatgptAuthTokensRefreshParams {
            reason: Self::map_reason(context.reason),
            previous_account_id: context.previous_account_id,
        };

        let (request_id, rx) = self
            .outgoing
            .send_request(ServerRequestPayload::ChatgptAuthTokensRefresh(params))
            .await;

        let result = match timeout(EXTERNAL_AUTH_REFRESH_TIMEOUT, rx).await {
            Ok(result) => {
                // Two failure scenarios:
                // 1) `oneshot::Receiver` failed (sender dropped) => request canceled/channel closed.
                // 2) client answered with JSON-RPC error payload => propagate code/message.
                let result = result.map_err(|err| {
                    std::io::Error::other(format!("auth refresh request canceled: {err}"))
                })?;
                result.map_err(|err| {
                    std::io::Error::other(format!(
                        "auth refresh request failed: code={} message={}",
                        err.code, err.message
                    ))
                })?
            }
            Err(_) => {
                let _canceled = self.outgoing.cancel_request(&request_id).await;
                return Err(std::io::Error::other(format!(
                    "auth refresh request timed out after {}s",
                    EXTERNAL_AUTH_REFRESH_TIMEOUT.as_secs()
                )));
            }
        };

        let response: ChatgptAuthTokensRefreshResponse =
            serde_json::from_value(result).map_err(std::io::Error::other)?;

        Ok(ExternalAuthTokens::chatgpt(
            response.access_token,
            response.chatgpt_account_id,
            response.chatgpt_plan_type,
        ))
    }
}

pub(crate) struct MessageProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    codex_message_processor: CodexMessageProcessor,
    thread_manager: Arc<ThreadManager>,
    config_api: ConfigApi,
    device_key_api: DeviceKeyApi,
    external_agent_config_api: ExternalAgentConfigApi,
    fs_api: FsApi,
    auth_manager: Arc<AuthManager>,
    analytics_events_client: AnalyticsEventsClient,
    fs_watch_manager: FsWatchManager,
    config: Arc<Config>,
    config_warnings: Arc<Vec<ConfigWarningNotification>>,
    rpc_transport: AppServerRpcTransport,
    remote_control_handle: Option<RemoteControlHandle>,
}

#[derive(Debug)]
pub(crate) struct ConnectionSessionState {
    origin: ConnectionOrigin,
    initialized: OnceLock<InitializedConnectionSessionState>,
}

#[derive(Debug)]
struct InitializedConnectionSessionState {
    experimental_api_enabled: bool,
    opted_out_notification_methods: HashSet<String>,
    app_server_client_name: String,
    client_version: String,
}

impl Default for ConnectionSessionState {
    fn default() -> Self {
        Self::new(ConnectionOrigin::WebSocket)
    }
}

impl ConnectionSessionState {
    pub(crate) fn new(origin: ConnectionOrigin) -> Self {
        Self {
            origin,
            initialized: OnceLock::new(),
        }
    }

    pub(crate) fn initialized(&self) -> bool {
        self.initialized.get().is_some()
    }

    fn allows_device_key_requests(&self) -> bool {
        self.origin.allows_device_key_requests()
    }

    pub(crate) fn experimental_api_enabled(&self) -> bool {
        self.initialized
            .get()
            .is_some_and(|session| session.experimental_api_enabled)
    }

    pub(crate) fn opted_out_notification_methods(&self) -> HashSet<String> {
        self.initialized
            .get()
            .map(|session| session.opted_out_notification_methods.clone())
            .unwrap_or_default()
    }

    pub(crate) fn app_server_client_name(&self) -> Option<&str> {
        self.initialized
            .get()
            .map(|session| session.app_server_client_name.as_str())
    }

    pub(crate) fn client_version(&self) -> Option<&str> {
        self.initialized
            .get()
            .map(|session| session.client_version.as_str())
    }

    fn initialize(&self, session: InitializedConnectionSessionState) -> Result<(), ()> {
        self.initialized.set(session).map_err(|_| ())
    }
}

pub(crate) struct MessageProcessorArgs {
    pub(crate) outgoing: Arc<OutgoingMessageSender>,
    pub(crate) arg0_paths: Arg0DispatchPaths,
    pub(crate) config: Arc<Config>,
    pub(crate) config_manager: ConfigManager,
    pub(crate) environment_manager: Arc<EnvironmentManager>,
    pub(crate) feedback: CodexFeedback,
    pub(crate) log_db: Option<LogDbLayer>,
    pub(crate) config_warnings: Vec<ConfigWarningNotification>,
    pub(crate) session_source: SessionSource,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) rpc_transport: AppServerRpcTransport,
    pub(crate) remote_control_handle: Option<RemoteControlHandle>,
    pub(crate) plugin_startup_tasks: crate::PluginStartupTasks,
}

impl MessageProcessor {
    /// Create a new `MessageProcessor`, retaining a handle to the outgoing
    /// `Sender` so handlers can enqueue messages to be written to stdout.
    pub(crate) fn new(args: MessageProcessorArgs) -> Self {
        let MessageProcessorArgs {
            outgoing,
            arg0_paths,
            config,
            config_manager,
            environment_manager,
            feedback,
            log_db,
            config_warnings,
            session_source,
            auth_manager,
            rpc_transport,
            remote_control_handle,
            plugin_startup_tasks,
        } = args;
        auth_manager.set_external_auth(Arc::new(ExternalAuthRefreshBridge {
            outgoing: outgoing.clone(),
        }));
        let analytics_events_client = AnalyticsEventsClient::new(
            Arc::clone(&auth_manager),
            config.chatgpt_base_url.trim_end_matches('/').to_string(),
            config.analytics_enabled,
        );
        let thread_manager = Arc::new(ThreadManager::new(
            config.as_ref(),
            auth_manager.clone(),
            session_source,
            CollaborationModesConfig {
                default_mode_request_user_input: config
                    .features
                    .enabled(Feature::DefaultModeRequestUserInput),
            },
            environment_manager,
            Some(analytics_events_client.clone()),
        ));
        thread_manager
            .plugins_manager()
            .set_analytics_events_client(analytics_events_client.clone());

        let codex_message_processor = CodexMessageProcessor::new(CodexMessageProcessorArgs {
            auth_manager: auth_manager.clone(),
            thread_manager: Arc::clone(&thread_manager),
            outgoing: outgoing.clone(),
            analytics_events_client: analytics_events_client.clone(),
            arg0_paths,
            config: Arc::clone(&config),
            config_manager: config_manager.clone(),
            feedback,
            log_db,
        });
        if matches!(plugin_startup_tasks, crate::PluginStartupTasks::Start) {
            // Keep plugin startup warmups aligned at app-server startup.
            // TODO(xl): Move into PluginManager once this no longer depends on config feature gating.
            thread_manager
                .plugins_manager()
                .maybe_start_plugin_startup_tasks_for_config(&config, auth_manager.clone());
        }
        let config_api = ConfigApi::new(
            config_manager,
            thread_manager.clone(),
            analytics_events_client.clone(),
        );
        let device_key_api =
            DeviceKeyApi::new(config.sqlite_home.clone(), config.model_provider_id.clone());
        let external_agent_config_api =
            ExternalAgentConfigApi::new(config.codex_home.to_path_buf());
        let fs_api = FsApi::new(
            thread_manager
                .environment_manager()
                .local_environment()
                .get_filesystem(),
        );
        let fs_watch_manager = FsWatchManager::new(outgoing.clone());

        Self {
            outgoing,
            codex_message_processor,
            thread_manager: Arc::clone(&thread_manager),
            config_api,
            device_key_api,
            external_agent_config_api,
            fs_api,
            auth_manager,
            analytics_events_client,
            fs_watch_manager,
            config,
            config_warnings: Arc::new(config_warnings),
            rpc_transport,
            remote_control_handle,
        }
    }

    pub(crate) fn clear_runtime_references(&self) {
        self.auth_manager.clear_external_auth();
    }

    pub(crate) async fn process_request(
        self: &Arc<Self>,
        connection_id: ConnectionId,
        request: JSONRPCRequest,
        transport: &AppServerTransport,
        session: Arc<ConnectionSessionState>,
    ) {
        let request_method = request.method.as_str();
        tracing::trace!(
            ?connection_id,
            request_id = ?request.id,
            "app-server request: {request_method}"
        );
        let request_id = ConnectionRequestId {
            connection_id,
            request_id: request.id.clone(),
        };
        let request_span =
            crate::app_server_tracing::request_span(&request, transport, connection_id, &session);
        let request_trace = request.trace.as_ref().map(|trace| W3cTraceContext {
            traceparent: trace.traceparent.clone(),
            tracestate: trace.tracestate.clone(),
        });
        let request_context = RequestContext::new(request_id.clone(), request_span, request_trace);
        Self::run_request_with_context(
            Arc::clone(&self.outgoing),
            request_context.clone(),
            async {
                let result = async {
                    let request_json = serde_json::to_value(&request)
                        .map_err(|err| invalid_request(format!("Invalid request: {err}")))?;
                    let codex_request = serde_json::from_value::<ClientRequest>(request_json)
                        .map_err(|err| invalid_request(format!("Invalid request: {err}")))?;
                    // Websocket callers finalize outbound readiness in lib.rs after mirroring
                    // session state into outbound state and sending initialize notifications to
                    // this specific connection. Passing `None` avoids marking the connection
                    // ready too early from inside the shared request handler.
                    self.handle_client_request(
                        request_id.clone(),
                        codex_request,
                        Arc::clone(&session),
                        /*outbound_initialized*/ None,
                        request_context.clone(),
                    )
                    .await
                }
                .await;
                if let Err(error) = result {
                    self.outgoing.send_error(request_id.clone(), error).await;
                }
            },
        )
        .await;
    }

    /// Handles a typed request path used by in-process embedders.
    ///
    /// This bypasses JSON request deserialization but keeps identical request
    /// semantics by delegating to `handle_client_request`.
    pub(crate) async fn process_client_request(
        self: &Arc<Self>,
        connection_id: ConnectionId,
        request: ClientRequest,
        session: Arc<ConnectionSessionState>,
        outbound_initialized: &AtomicBool,
    ) {
        let request_id = ConnectionRequestId {
            connection_id,
            request_id: request.id().clone(),
        };
        let request_span =
            crate::app_server_tracing::typed_request_span(&request, connection_id, &session);
        let request_context =
            RequestContext::new(request_id.clone(), request_span, /*parent_trace*/ None);
        tracing::trace!(
            ?connection_id,
            request_id = ?request_id.request_id,
            "app-server typed request"
        );
        Self::run_request_with_context(
            Arc::clone(&self.outgoing),
            request_context.clone(),
            async {
                // In-process clients do not have the websocket transport loop that performs
                // post-initialize bookkeeping, so they still finalize outbound readiness in
                // the shared request handler.
                let result = self
                    .handle_client_request(
                        request_id.clone(),
                        request,
                        Arc::clone(&session),
                        Some(outbound_initialized),
                        request_context.clone(),
                    )
                    .await;
                if let Err(error) = result {
                    self.outgoing.send_error(request_id.clone(), error).await;
                }
            },
        )
        .await;
    }

    pub(crate) async fn process_notification(&self, notification: JSONRPCNotification) {
        // Currently, we do not expect to receive any notifications from the
        // client, so we just log them.
        tracing::info!("<- notification: {:?}", notification);
    }

    /// Handles typed notifications from in-process clients.
    pub(crate) async fn process_client_notification(&self, notification: ClientNotification) {
        // Currently, we do not expect to receive any typed notifications from
        // in-process clients, so we just log them.
        tracing::info!("<- typed notification: {:?}", notification);
    }

    async fn run_request_with_context<F>(
        outgoing: Arc<OutgoingMessageSender>,
        request_context: RequestContext,
        request_fut: F,
    ) where
        F: Future<Output = ()>,
    {
        outgoing
            .register_request_context(request_context.clone())
            .await;
        request_fut.instrument(request_context.span()).await;
    }

    pub(crate) fn thread_created_receiver(&self) -> broadcast::Receiver<ThreadId> {
        self.codex_message_processor.thread_created_receiver()
    }

    pub(crate) async fn send_initialize_notifications_to_connection(
        &self,
        connection_id: ConnectionId,
    ) {
        for notification in self.config_warnings.iter().cloned() {
            self.outgoing
                .send_server_notification_to_connections(
                    &[connection_id],
                    ServerNotification::ConfigWarning(notification),
                )
                .await;
        }
    }

    pub(crate) async fn connection_initialized(&self, connection_id: ConnectionId) {
        self.codex_message_processor
            .connection_initialized(connection_id)
            .await;
    }

    pub(crate) async fn send_initialize_notifications(&self) {
        for notification in self.config_warnings.iter().cloned() {
            self.outgoing
                .send_server_notification(ServerNotification::ConfigWarning(notification))
                .await;
        }
    }

    pub(crate) async fn try_attach_thread_listener(
        &self,
        thread_id: ThreadId,
        connection_ids: Vec<ConnectionId>,
    ) {
        self.codex_message_processor
            .try_attach_thread_listener(thread_id, connection_ids)
            .await;
    }

    pub(crate) async fn drain_background_tasks(&self) {
        self.codex_message_processor.drain_background_tasks().await;
    }

    pub(crate) async fn cancel_active_login(&self) {
        self.codex_message_processor.cancel_active_login().await;
    }

    pub(crate) async fn clear_all_thread_listeners(&self) {
        self.codex_message_processor
            .clear_all_thread_listeners()
            .await;
    }

    pub(crate) async fn shutdown_threads(&self) {
        self.codex_message_processor.shutdown_threads().await;
    }

    pub(crate) async fn connection_closed(&self, connection_id: ConnectionId) {
        self.outgoing.connection_closed(connection_id).await;
        self.fs_watch_manager.connection_closed(connection_id).await;
        self.codex_message_processor
            .connection_closed(connection_id)
            .await;
    }

    pub(crate) fn subscribe_running_assistant_turn_count(&self) -> watch::Receiver<usize> {
        self.codex_message_processor
            .subscribe_running_assistant_turn_count()
    }

    /// Handle a standalone JSON-RPC response originating from the peer.
    pub(crate) async fn process_response(&self, response: JSONRPCResponse) {
        tracing::info!("<- response: {:?}", response);
        let JSONRPCResponse { id, result, .. } = response;
        self.outgoing.notify_client_response(id, result).await
    }

    /// Handle an error object received from the peer.
    pub(crate) async fn process_error(&self, err: JSONRPCError) {
        tracing::error!("<- error: {:?}", err);
        self.outgoing.notify_client_error(err.id, err.error).await;
    }

    async fn handle_client_request(
        self: &Arc<Self>,
        connection_request_id: ConnectionRequestId,
        codex_request: ClientRequest,
        session: Arc<ConnectionSessionState>,
        // `Some(...)` means the caller wants initialize to immediately mark the
        // connection outbound-ready. Websocket JSON-RPC calls pass `None` so
        // lib.rs can deliver connection-scoped initialize notifications first.
        outbound_initialized: Option<&AtomicBool>,
        request_context: RequestContext,
    ) -> Result<(), JSONRPCErrorError> {
        let connection_id = connection_request_id.connection_id;
        if let ClientRequest::Initialize { request_id, params } = codex_request {
            // Handle Initialize internally so CodexMessageProcessor does not have to concern
            // itself with the `initialized` bool.
            let connection_request_id = ConnectionRequestId {
                connection_id,
                request_id,
            };
            if session.initialized() {
                return Err(invalid_request("Already initialized"));
            }

            // TODO(maxj): Revisit capability scoping for `experimental_api_enabled`.
            // Current behavior is per-connection. Reviewer feedback notes this can
            // create odd cross-client behavior (for example dynamic tool calls on a
            // shared thread when another connected client did not opt into
            // experimental API). Proposed direction is instance-global first-write-wins
            // with initialize-time mismatch rejection.
            let analytics_initialize_params = params.clone();
            let (experimental_api_enabled, opt_out_notification_methods) = match params.capabilities
            {
                Some(capabilities) => (
                    capabilities.experimental_api,
                    capabilities
                        .opt_out_notification_methods
                        .unwrap_or_default(),
                ),
                None => (false, Vec::new()),
            };
            let ClientInfo {
                name,
                title: _title,
                version,
            } = params.client_info;
            // Validate before committing; set_default_originator validates while
            // mutating process-global metadata.
            if HeaderValue::from_str(&name).is_err() {
                return Err(invalid_request(format!(
                    "Invalid clientInfo.name: '{name}'. Must be a valid HTTP header value."
                )));
            }
            let originator = name.clone();
            let user_agent_suffix = format!("{name}; {version}");
            let codex_home = self.config.codex_home.clone();
            if session
                .initialize(InitializedConnectionSessionState {
                    experimental_api_enabled,
                    opted_out_notification_methods: opt_out_notification_methods
                        .into_iter()
                        .collect(),
                    app_server_client_name: name.clone(),
                    client_version: version,
                })
                .is_err()
            {
                return Err(invalid_request("Already initialized"));
            }

            // Only the request that wins session initialization may mutate
            // process-global client metadata.
            if let Err(error) = set_default_originator(originator.clone()) {
                match error {
                    SetOriginatorError::InvalidHeaderValue => {
                        tracing::warn!(
                            client_info_name = %name,
                            "validated clientInfo.name was rejected while setting originator"
                        );
                    }
                    SetOriginatorError::AlreadyInitialized => {
                        // No-op. This is expected to happen if the originator is already set via env var.
                        // TODO(owen): Once we remove support for CODEX_INTERNAL_ORIGINATOR_OVERRIDE,
                        // this will be an unexpected state and we can return a JSON-RPC error indicating
                        // internal server error.
                    }
                }
            }
            if self.config.features.enabled(Feature::GeneralAnalytics) {
                self.analytics_events_client.track_initialize(
                    connection_id.0,
                    analytics_initialize_params,
                    originator,
                    self.rpc_transport,
                );
            }
            set_default_client_residency_requirement(self.config.enforce_residency.value());
            if let Ok(mut suffix) = USER_AGENT_SUFFIX.lock() {
                *suffix = Some(user_agent_suffix);
            }

            let user_agent = get_codex_user_agent();
            let response = InitializeResponse {
                user_agent,
                codex_home,
                platform_family: std::env::consts::FAMILY.to_string(),
                platform_os: std::env::consts::OS.to_string(),
            };

            self.outgoing
                .send_response(connection_request_id, response)
                .await;

            if let Some(outbound_initialized) = outbound_initialized {
                // In-process clients can complete readiness immediately here. The
                // websocket path defers this until lib.rs finishes transport-layer
                // initialize handling for the specific connection.
                outbound_initialized.store(true, Ordering::Release);
                self.codex_message_processor
                    .connection_initialized(connection_id)
                    .await;
            }
            return Ok(());
        }

        self.dispatch_initialized_client_request(
            connection_request_id,
            codex_request,
            session,
            request_context,
        )
        .await
    }

    async fn dispatch_initialized_client_request(
        self: &Arc<Self>,
        connection_request_id: ConnectionRequestId,
        codex_request: ClientRequest,
        session: Arc<ConnectionSessionState>,
        request_context: RequestContext,
    ) -> Result<(), JSONRPCErrorError> {
        if !session.initialized() {
            return Err(invalid_request("Not initialized"));
        }

        if let Some(reason) = codex_request.experimental_reason()
            && !session.experimental_api_enabled()
        {
            return Err(invalid_request(experimental_required_message(reason)));
        }
        let connection_id = connection_request_id.connection_id;
        if self.config.features.enabled(Feature::GeneralAnalytics)
            && let ClientRequest::TurnStart { request_id, .. }
            | ClientRequest::TurnSteer { request_id, .. } = &codex_request
        {
            self.analytics_events_client.track_request(
                connection_id.0,
                request_id.clone(),
                codex_request.clone(),
            );
        }

        let app_server_client_name = session.app_server_client_name().map(str::to_string);
        let client_version = session.client_version().map(str::to_string);
        let device_key_requests_allowed = session.allows_device_key_requests();
        Arc::clone(self)
            .handle_initialized_client_request(
                connection_request_id,
                codex_request,
                request_context,
                app_server_client_name,
                client_version,
                device_key_requests_allowed,
            )
            .await
    }

    async fn handle_initialized_client_request(
        self: Arc<Self>,
        connection_request_id: ConnectionRequestId,
        codex_request: ClientRequest,
        request_context: RequestContext,
        app_server_client_name: Option<String>,
        client_version: Option<String>,
        device_key_requests_allowed: bool,
    ) -> Result<(), JSONRPCErrorError> {
        let connection_id = connection_request_id.connection_id;
        let request_id_for_connection = |request_id| ConnectionRequestId {
            connection_id,
            request_id,
        };

        match codex_request {
            ClientRequest::ConfigRead { request_id, params } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.config_api.read(params).await,
                    )
                    .await;
            }
            ClientRequest::ExternalAgentConfigDetect { request_id, params } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.external_agent_config_api.detect(params).await,
                    )
                    .await;
            }
            ClientRequest::ExternalAgentConfigImport { request_id, params } => {
                self.handle_external_agent_config_import(
                    request_id_for_connection(request_id),
                    params,
                )
                .await?;
            }
            ClientRequest::ConfigValueWrite { request_id, params } => {
                self.handle_config_value_write(request_id_for_connection(request_id), params)
                    .await;
            }
            ClientRequest::ConfigBatchWrite { request_id, params } => {
                self.handle_config_batch_write(request_id_for_connection(request_id), params)
                    .await;
            }
            ClientRequest::ExperimentalFeatureEnablementSet { request_id, params } => {
                self.handle_experimental_feature_enablement_set(
                    request_id_for_connection(request_id),
                    params,
                )
                .await;
            }
            ClientRequest::ConfigRequirementsRead {
                request_id,
                params: _,
            } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.config_api.config_requirements_read().await,
                    )
                    .await;
            }
            ClientRequest::DeviceKeyCreate { request_id, params } => {
                self.handle_device_key_create(
                    request_id_for_connection(request_id),
                    params,
                    device_key_requests_allowed,
                );
            }
            ClientRequest::DeviceKeyPublic { request_id, params } => {
                self.handle_device_key_public(
                    request_id_for_connection(request_id),
                    params,
                    device_key_requests_allowed,
                );
            }
            ClientRequest::DeviceKeySign { request_id, params } => {
                self.handle_device_key_sign(
                    request_id_for_connection(request_id),
                    params,
                    device_key_requests_allowed,
                );
            }
            ClientRequest::FsReadFile { request_id, params } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.fs_api.read_file(params).await,
                    )
                    .await;
            }
            ClientRequest::FsWriteFile { request_id, params } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.fs_api.write_file(params).await,
                    )
                    .await;
            }
            ClientRequest::FsCreateDirectory { request_id, params } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.fs_api.create_directory(params).await,
                    )
                    .await;
            }
            ClientRequest::FsGetMetadata { request_id, params } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.fs_api.get_metadata(params).await,
                    )
                    .await;
            }
            ClientRequest::FsReadDirectory { request_id, params } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.fs_api.read_directory(params).await,
                    )
                    .await;
            }
            ClientRequest::FsRemove { request_id, params } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.fs_api.remove(params).await,
                    )
                    .await;
            }
            ClientRequest::FsCopy { request_id, params } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.fs_api.copy(params).await,
                    )
                    .await;
            }
            ClientRequest::FsWatch { request_id, params } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.fs_watch_manager.watch(connection_id, params).await,
                    )
                    .await;
            }
            ClientRequest::FsUnwatch { request_id, params } => {
                self.outgoing
                    .send_result(
                        request_id_for_connection(request_id),
                        self.fs_watch_manager.unwatch(connection_id, params).await,
                    )
                    .await;
            }
            other => {
                // Box the delegated future so this wrapper's async state machine does not
                // inline the full `CodexMessageProcessor::process_request` future, which
                // can otherwise push worker-thread stack usage over the edge.
                self.codex_message_processor
                    .process_request(
                        connection_id,
                        other,
                        app_server_client_name,
                        client_version,
                        request_context,
                    )
                    .boxed()
                    .await;
            }
        }
        Ok(())
    }

    async fn handle_config_value_write(
        &self,
        request_id: ConnectionRequestId,
        params: ConfigValueWriteParams,
    ) {
        let result = self.config_api.write_value(params).await;
        self.handle_config_mutation_result(request_id, result).await
    }

    async fn handle_config_batch_write(
        &self,
        request_id: ConnectionRequestId,
        params: ConfigBatchWriteParams,
    ) {
        let result = self.config_api.batch_write(params).await;
        self.handle_config_mutation_result(request_id, result).await;
    }

    async fn handle_experimental_feature_enablement_set(
        &self,
        request_id: ConnectionRequestId,
        params: ExperimentalFeatureEnablementSetParams,
    ) {
        let should_refresh_apps_list = params.enablement.get("apps").copied() == Some(true);
        let result = self
            .config_api
            .set_experimental_feature_enablement(params)
            .await;
        let is_ok = result.is_ok();
        self.handle_config_mutation_result(request_id, result).await;
        if should_refresh_apps_list && is_ok {
            self.refresh_apps_list_after_experimental_feature_enablement_set()
                .await;
        }
    }

    async fn refresh_apps_list_after_experimental_feature_enablement_set(&self) {
        let config = match self
            .config_api
            .load_latest_config(/*fallback_cwd*/ None)
            .await
        {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(
                    "failed to load config for apps list refresh after experimental feature enablement: {}",
                    error.message
                );
                return;
            }
        };
        let auth = self.auth_manager.auth().await;
        if !config.features.apps_enabled_for_auth(
            auth.as_ref()
                .is_some_and(codex_login::CodexAuth::uses_codex_backend),
        ) {
            return;
        }

        let outgoing = Arc::clone(&self.outgoing);
        let environment_manager = self.thread_manager.environment_manager();
        tokio::spawn(async move {
            let (all_connectors_result, accessible_connectors_result) = tokio::join!(
                connectors::list_all_connectors_with_options(&config, /*force_refetch*/ true),
                connectors::list_accessible_connectors_from_mcp_tools_with_environment_manager(
                    &config,
                    /*force_refetch*/ true,
                    &environment_manager,
                ),
            );
            let all_connectors = match all_connectors_result {
                Ok(connectors) => connectors,
                Err(err) => {
                    tracing::warn!(
                        "failed to force-refresh directory apps after experimental feature enablement: {err:#}"
                    );
                    return;
                }
            };
            let accessible_connectors = match accessible_connectors_result {
                Ok(status) => status.connectors,
                Err(err) => {
                    tracing::warn!(
                        "failed to force-refresh accessible apps after experimental feature enablement: {err:#}"
                    );
                    return;
                }
            };

            let data = connectors::with_app_enabled_state(
                connectors::merge_connectors_with_accessible(
                    all_connectors,
                    accessible_connectors,
                    /*all_connectors_loaded*/ true,
                ),
                &config,
            );
            outgoing
                .send_server_notification(ServerNotification::AppListUpdated(
                    AppListUpdatedNotification { data },
                ))
                .await;
        });
    }

    async fn handle_config_mutation_result<T: serde::Serialize>(
        &self,
        request_id: ConnectionRequestId,
        result: std::result::Result<T, JSONRPCErrorError>,
    ) {
        match result {
            Ok(response) => {
                self.handle_config_mutation().await;
                self.outgoing.send_response(request_id, response).await;
            }
            Err(error) => self.outgoing.send_error(request_id, error).await,
        }
    }

    async fn handle_config_mutation(&self) {
        self.codex_message_processor.handle_config_mutation();
        let Some(remote_control_handle) = &self.remote_control_handle else {
            return;
        };

        match self
            .config_api
            .load_latest_config(/*fallback_cwd*/ None)
            .await
        {
            Ok(config) => {
                remote_control_handle.set_enabled(config.features.enabled(Feature::RemoteControl));
            }
            Err(error) => {
                tracing::warn!(
                    "failed to load config for remote control enablement refresh after config mutation: {}",
                    error.message
                );
            }
        }
    }

    fn handle_device_key_create(
        &self,
        request_id: ConnectionRequestId,
        params: DeviceKeyCreateParams,
        device_key_requests_allowed: bool,
    ) {
        self.spawn_device_key_request(
            request_id,
            "device/key/create",
            device_key_requests_allowed,
            move |device_key_api| async move { device_key_api.create(params).await },
        );
    }

    fn handle_device_key_public(
        &self,
        request_id: ConnectionRequestId,
        params: DeviceKeyPublicParams,
        device_key_requests_allowed: bool,
    ) {
        self.spawn_device_key_request(
            request_id,
            "device/key/public",
            device_key_requests_allowed,
            move |device_key_api| async move { device_key_api.public(params).await },
        );
    }

    fn handle_device_key_sign(
        &self,
        request_id: ConnectionRequestId,
        params: DeviceKeySignParams,
        device_key_requests_allowed: bool,
    ) {
        self.spawn_device_key_request(
            request_id,
            "device/key/sign",
            device_key_requests_allowed,
            move |device_key_api| async move { device_key_api.sign(params).await },
        );
    }

    fn spawn_device_key_request<R, F, Fut>(
        &self,
        request_id: ConnectionRequestId,
        method: &'static str,
        device_key_requests_allowed: bool,
        run_request: F,
    ) where
        R: serde::Serialize + Send + 'static,
        F: FnOnce(DeviceKeyApi) -> Fut + Send + 'static,
        Fut: Future<Output = Result<R, JSONRPCErrorError>> + Send + 'static,
    {
        let device_key_api = self.device_key_api.clone();
        let outgoing = Arc::clone(&self.outgoing);
        tokio::spawn(async move {
            let result = async {
                if !device_key_requests_allowed {
                    return Err(invalid_request(format!(
                        "{method} is not available over remote transports"
                    )));
                }
                run_request(device_key_api).await
            }
            .await;
            outgoing.send_result(request_id, result).await;
        });
    }

    async fn handle_external_agent_config_import(
        &self,
        request_id: ConnectionRequestId,
        params: ExternalAgentConfigImportParams,
    ) -> Result<(), JSONRPCErrorError> {
        let has_plugin_imports = params.migration_items.iter().any(|item| {
            matches!(
                item.item_type,
                ExternalAgentConfigMigrationItemType::Plugins
            )
        });

        let pending_plugin_imports = self.external_agent_config_api.import(params).await?;
        if has_plugin_imports {
            self.handle_config_mutation().await;
        }
        self.outgoing
            .send_response(request_id, ExternalAgentConfigImportResponse {})
            .await;

        if !has_plugin_imports {
            return Ok(());
        }

        if pending_plugin_imports.is_empty() {
            self.outgoing
                .send_server_notification(ServerNotification::ExternalAgentConfigImportCompleted(
                    ExternalAgentConfigImportCompletedNotification {},
                ))
                .await;
            return Ok(());
        }

        let external_agent_config_api = self.external_agent_config_api.clone();
        let outgoing = Arc::clone(&self.outgoing);
        let thread_manager = Arc::clone(&self.thread_manager);
        tokio::spawn(async move {
            for pending_plugin_import in pending_plugin_imports {
                match external_agent_config_api
                    .complete_pending_plugin_import(pending_plugin_import)
                    .await
                {
                    Ok(()) => {}
                    Err(error) => {
                        tracing::warn!(
                            error = %error.message,
                            "external agent config plugin import failed"
                        );
                    }
                }
            }
            thread_manager.plugins_manager().clear_cache();
            thread_manager.skills_manager().clear_cache();
            outgoing
                .send_server_notification(ServerNotification::ExternalAgentConfigImportCompleted(
                    ExternalAgentConfigImportCompletedNotification {},
                ))
                .await;
        });

        Ok(())
    }
}

#[cfg(test)]
mod tracing_tests;
