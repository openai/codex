use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use axum::http::HeaderValue;
use codex_analytics::AppServerRpcTransport;
use codex_login::default_client::SetOriginatorError;
use codex_login::default_client::USER_AGENT_SUFFIX;
use codex_login::default_client::get_codex_user_agent;
use codex_login::default_client::set_default_client_residency_requirement;
use codex_login::default_client::set_default_originator;

use super::*;
use crate::message_processor::ConnectionSessionState;
use crate::message_processor::InitializedConnectionSessionState;

#[derive(Clone)]
pub(crate) struct InitializeRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    analytics_events_client: AnalyticsEventsClient,
    config: Arc<Config>,
    config_warnings: Arc<Vec<ConfigWarningNotification>>,
    rpc_transport: AppServerRpcTransport,
}

impl InitializeRequestProcessor {
    pub(crate) fn new(
        outgoing: Arc<OutgoingMessageSender>,
        analytics_events_client: AnalyticsEventsClient,
        config: Arc<Config>,
        config_warnings: Vec<ConfigWarningNotification>,
        rpc_transport: AppServerRpcTransport,
    ) -> Self {
        Self {
            outgoing,
            analytics_events_client,
            config,
            config_warnings: Arc::new(config_warnings),
            rpc_transport,
        }
    }

    pub(crate) async fn initialize(
        &self,
        connection_id: ConnectionId,
        request_id: RequestId,
        params: InitializeParams,
        session: &ConnectionSessionState,
        // `Some(...)` means the caller wants initialize to immediately mark the
        // connection outbound-ready. Websocket JSON-RPC calls pass `None` so
        // lib.rs can deliver connection-scoped initialize notifications first.
        outbound_initialized: Option<&AtomicBool>,
    ) -> Result<bool, JSONRPCErrorError> {
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
        let (experimental_api_enabled, request_attestation, opt_out_notification_methods) =
            match params.capabilities {
                Some(capabilities) => (
                    capabilities.experimental_api,
                    capabilities.request_attestation,
                    capabilities
                        .opt_out_notification_methods
                        .unwrap_or_default(),
                ),
                None => (false, false, Vec::new()),
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
                opted_out_notification_methods: opt_out_notification_methods.into_iter().collect(),
                app_server_client_name: name.clone(),
                client_version: version,
                request_attestation,
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
        self.analytics_events_client.track_initialize(
            connection_id.0,
            analytics_initialize_params,
            originator,
            self.rpc_transport,
        );
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
            outbound_initialized.store(true, Ordering::Release);
            return Ok(true);
        }

        Ok(false)
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

    pub(crate) async fn send_initialize_notifications(&self) {
        for notification in self.config_warnings.iter().cloned() {
            self.outgoing
                .send_server_notification(ServerNotification::ConfigWarning(notification))
                .await;
        }
    }

    pub(crate) fn track_initialized_request(
        &self,
        connection_id: ConnectionId,
        request_id: RequestId,
        request: &ClientRequest,
    ) {
        self.analytics_events_client
            .track_request(connection_id.0, request_id, request);
    }
}
