use crate::events::AppServerRpcTransport;
use crate::events::GuardianReviewAnalyticsResult;
use crate::events::GuardianReviewTrackContext;
use crate::events::TrackEventRequest;
use crate::events::TrackEventsRequest;
use crate::events::current_runtime_metadata;
use crate::facts::AnalyticsFact;
use crate::facts::AnalyticsJsonRpcError;
use crate::facts::AppInvocation;
use crate::facts::AppMentionedInput;
use crate::facts::AppUsedInput;
use crate::facts::CodexGoalEvent;
use crate::facts::CustomAnalyticsFact;
use crate::facts::ExternalAgentConfigImportCompletedInput;
use crate::facts::ExternalAgentConfigImportFailureInput;
use crate::facts::HookRunFact;
use crate::facts::HookRunInput;
use crate::facts::ImagePreparationFact;
use crate::facts::InternalToolInputLog; // copybara:strip-for-public
use crate::facts::PluginInstallFailedInput;
use crate::facts::PluginInstallRequested;
use crate::facts::PluginInstallRequestedInput;
use crate::facts::PluginInstallSource;
use crate::facts::PluginState;
use crate::facts::PluginStateChangedInput;
use crate::facts::SkillInvocation;
use crate::facts::SkillInvokedInput;
use crate::facts::SubAgentThreadStartedInput;
use crate::facts::TrackEventsContext;
use crate::facts::TurnCodexErrorFact;
use crate::facts::TurnProfileFact;
use crate::facts::TurnResolvedConfigFact;
use crate::facts::TurnTokenUsageFact;
use crate::now_unix_millis;
use crate::reducer::AnalyticsReducer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::default_client::create_client;
use codex_plugin::PluginId;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use serde_json::Value; // copybara:strip-for-public
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock; // copybara:strip-for-public
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

const ANALYTICS_EVENTS_QUEUE_SIZE: usize = 256;
const ANALYTICS_EVENTS_TIMEOUT: Duration = Duration::from_secs(10);
// Covers two sequential POSTs plus queue/barrier scheduling; additional queued sends remain best-effort.
const ANALYTICS_EVENTS_FLUSH_TIMEOUT: Duration = Duration::from_secs(25);
const ANALYTICS_EVENT_DEDUPE_MAX_KEYS: usize = 4096;
// copybara:strip-for-public begin
const MAX_INTERNAL_TOOL_COMMANDS: usize = 16;
const MAX_INTERNAL_TOOL_FLAGS: usize = 32;
// copybara:strip-for-public end

pub(crate) enum AnalyticsEventsQueueMessage {
    Fact(Box<AnalyticsFact>),
    InternalFact(Box<AnalyticsFact>, (String, String)), // copybara:strip-for-public
    Flush(oneshot::Sender<()>),
}

#[derive(Clone)]
pub(crate) struct AnalyticsEventsQueue {
    pub(crate) sender: mpsc::Sender<AnalyticsEventsQueueMessage>,
    pub(crate) app_used_emitted_keys: Arc<Mutex<HashSet<(String, String)>>>,
    pub(crate) plugin_used_emitted_keys: Arc<Mutex<HashSet<(String, String)>>>,
}

#[derive(Clone)]
pub struct AnalyticsEventsClient {
    queue: Option<AnalyticsEventsQueue>,
    // copybara:strip-for-public begin
    internal_tool_input: Option<InternalToolInputQueue>,
    // copybara:strip-for-public end
}
// copybara:strip-for-public begin
#[derive(Clone)]
struct InternalToolInputQueue {
    auth_manager: Arc<AuthManager>,
    destination: AnalyticsEventsDestination,
    queue: Arc<OnceLock<AnalyticsEventsQueue>>,
}
// copybara:strip-for-public end

#[derive(Clone, Debug, Eq, PartialEq)]
enum AnalyticsEventsDestination {
    Http {
        url: String,
    },
    #[cfg(debug_assertions)]
    CaptureFile {
        path: PathBuf,
    },
}

impl AnalyticsEventsDestination {
    fn from_base_url(base_url: String) -> Self {
        let capture_file = analytics_capture_file_from_env();
        Self::from_base_url_and_capture_file(base_url, capture_file)
    }

    fn from_base_url_and_capture_file(base_url: String, capture_file: Option<PathBuf>) -> Self {
        #[cfg(debug_assertions)]
        if let Some(path) = capture_file {
            if let Err(err) = crate::analytics_capture::initialize(&path) {
                tracing::error!(
                    path = %path.display(),
                    "failed to initialize analytics event capture; network delivery remains disabled: {err}"
                );
            }
            tracing::warn!(
                path = %path.display(),
                "analytics event capture enabled; network delivery is disabled"
            );
            return Self::CaptureFile { path };
        }

        #[cfg(not(debug_assertions))]
        let _ = capture_file;

        let base_url = base_url.trim_end_matches('/');
        Self::Http {
            url: format!("{base_url}/codex/analytics-events/events"),
        }
    }
}

fn analytics_capture_file_from_env() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    {
        std::env::var_os(crate::analytics_capture::ANALYTICS_EVENTS_CAPTURE_FILE_ENV_VAR)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    #[cfg(not(debug_assertions))]
    None
}

impl AnalyticsEventsQueue {
    fn new(auth_manager: Arc<AuthManager>, destination: AnalyticsEventsDestination) -> Self {
        let (sender, mut receiver) = mpsc::channel(ANALYTICS_EVENTS_QUEUE_SIZE);
        tokio::spawn(async move {
            let mut reducer = AnalyticsReducer::default();
            while let Some(input) = receiver.recv().await {
                let input = match input {
                    AnalyticsEventsQueueMessage::Fact(input) => *input,
                    // copybara:strip-for-public begin
                    AnalyticsEventsQueueMessage::InternalFact(input, identity) => {
                        let Some(auth) = auth_manager.auth().await else {
                            continue;
                        };
                        if internal_tool_input_identity(&auth) != Some(identity) {
                            continue;
                        }
                        let mut events = Vec::new();
                        reducer.ingest(*input, &mut events).await;
                        send_track_events_request(&auth, &destination, events).await;
                        continue;
                    }
                    // copybara:strip-for-public end
                    AnalyticsEventsQueueMessage::Flush(done_tx) => {
                        let _ = done_tx.send(());
                        continue;
                    }
                };
                let mut events = Vec::new();
                reducer.ingest(input, &mut events).await;
                send_track_events(&auth_manager, &destination, events).await;
            }
        });
        Self {
            sender,
            app_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
            plugin_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    fn try_send(&self, input: AnalyticsFact) {
        if self
            .sender
            .try_send(AnalyticsEventsQueueMessage::Fact(Box::new(input)))
            .is_err()
        {
            //TODO: add a metric for this
            tracing::warn!("dropping analytics events: queue is full");
        }
    }

    pub(crate) fn should_enqueue_app_used(
        &self,
        tracking: &TrackEventsContext,
        app: &AppInvocation,
    ) -> bool {
        let Some(connector_id) = app.connector_id.as_ref() else {
            return true;
        };
        let mut emitted = self
            .app_used_emitted_keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if emitted.len() >= ANALYTICS_EVENT_DEDUPE_MAX_KEYS {
            emitted.clear();
        }
        emitted.insert((tracking.turn_id.clone(), connector_id.clone()))
    }

    pub(crate) fn should_enqueue_plugin_used(
        &self,
        tracking: &TrackEventsContext,
        plugin: &PluginTelemetryMetadata,
    ) -> bool {
        let mut emitted = self
            .plugin_used_emitted_keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if emitted.len() >= ANALYTICS_EVENT_DEDUPE_MAX_KEYS {
            emitted.clear();
        }
        let Some(plugin_id) = plugin
            .plugin_id
            .as_ref()
            .map(PluginId::as_key)
            .or_else(|| plugin.remote_plugin_id.clone())
        else {
            return true;
        };
        emitted.insert((tracking.turn_id.clone(), plugin_id))
    }
}

impl AnalyticsEventsClient {
    pub fn new(
        auth_manager: Arc<AuthManager>,
        base_url: String,
        analytics_enabled: Option<bool>,
    ) -> Self {
        let destination = AnalyticsEventsDestination::from_base_url(base_url);
        // copybara:strip-for-public begin
        let internal_tool_input = (analytics_enabled != Some(false)
            && match &destination {
                AnalyticsEventsDestination::Http { url } => {
                    url.starts_with("https://chatgpt.com/")
                        || url.starts_with("https://chatgpt-staging.com/")
                        || (cfg!(debug_assertions)
                            && url
                                .strip_prefix("http://127.0.0.1:")
                                .and_then(|value| value.split_once('/'))
                                .is_some_and(|(port, path)| {
                                    port.parse::<u16>().is_ok()
                                        && path == "codex/analytics-events/events"
                                }))
                }
                #[cfg(debug_assertions)]
                AnalyticsEventsDestination::CaptureFile { .. } => true,
            })
        .then(|| InternalToolInputQueue {
            auth_manager: Arc::clone(&auth_manager),
            destination: destination.clone(),
            queue: Arc::new(OnceLock::new()),
        });
        // copybara:strip-for-public end
        Self {
            queue: (analytics_enabled != Some(false))
                .then(|| AnalyticsEventsQueue::new(Arc::clone(&auth_manager), destination)),
            // copybara:strip-for-public begin
            internal_tool_input,
            // copybara:strip-for-public end
        }
    }

    pub fn disabled() -> Self {
        // copybara:replace-for-public begin
        Self {
            queue: None,
            internal_tool_input: None,
        }
        // copybara:replace-for-public with
        // copybara:public Self { queue: None }
        // copybara:replace-for-public end
    }
    // copybara:strip-for-public begin
    pub fn without_internal_tool_input_logging(mut self) -> Self {
        self.internal_tool_input = None;
        self
    }

    pub fn is_internal_tool_input_logging_enabled(&self) -> bool {
        self.internal_tool_input
            .as_ref()
            .filter(|input| {
                input
                    .queue
                    .get()
                    .is_none_or(|queue| queue.sender.capacity() > 0)
            })
            .and_then(|input| input.auth_manager.auth_cached())
            .is_some_and(|auth| internal_tool_input_identity(&auth).is_some())
    }

    pub fn track_internal_tool_input(&self, input: InternalToolInputLog) {
        let Some(internal_tool_input) = self.internal_tool_input.as_ref() else {
            return;
        };
        let Some(identity) = internal_tool_input
            .auth_manager
            .auth_cached()
            .and_then(|auth| internal_tool_input_identity(&auth))
        else {
            return;
        };
        if std::iter::once(&input.arguments_before_hooks)
            .chain(input.hook_normalized_input.iter())
            .chain(input.arguments_after_hooks.iter())
            .any(|metadata| !is_valid_internal_tool_input_metadata(metadata))
        {
            return;
        }
        let queue = internal_tool_input.queue.get_or_init(|| {
            AnalyticsEventsQueue::new(
                Arc::clone(&internal_tool_input.auth_manager),
                internal_tool_input.destination.clone(),
            )
        });
        let Ok(permit) = queue.sender.try_reserve() else {
            return;
        };

        permit.send(AnalyticsEventsQueueMessage::InternalFact(
            Box::new(input.into()),
            identity,
        ));
    }
    // copybara:strip-for-public end

    pub async fn flush(&self) {
        let Some(queue) = self.queue.as_ref() else {
            return;
        };
        let (done_tx, done_rx) = oneshot::channel();
        let flushed = tokio::time::timeout(ANALYTICS_EVENTS_FLUSH_TIMEOUT, async {
            if queue
                .sender
                .send(AnalyticsEventsQueueMessage::Flush(done_tx))
                .await
                .is_err()
            {
                return false;
            }
            // copybara:strip-for-public begin
            if let Some(internal_queue) = self
                .internal_tool_input
                .as_ref()
                .and_then(|input| input.queue.get())
            {
                let (internal_done_tx, internal_done_rx) = oneshot::channel();
                if internal_queue
                    .sender
                    .send(AnalyticsEventsQueueMessage::Flush(internal_done_tx))
                    .await
                    .is_err()
                    || internal_done_rx.await.is_err()
                {
                    return false;
                }
            }
            // copybara:strip-for-public end
            done_rx.await.is_ok()
        })
        .await;

        if !matches!(flushed, Ok(true)) {
            tracing::warn!("timed out or failed while flushing analytics events");
        }
    }

    pub fn track_skill_invocations(
        &self,
        tracking: TrackEventsContext,
        invocations: Vec<SkillInvocation>,
    ) {
        if invocations.is_empty() {
            return;
        }
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::SkillInvoked(
            SkillInvokedInput {
                tracking,
                invocations,
            },
        )));
    }

    pub fn track_initialize(
        &self,
        connection_id: u64,
        params: InitializeParams,
        product_client_id: String,
        rpc_transport: AppServerRpcTransport,
    ) {
        self.record_fact(AnalyticsFact::Initialize {
            connection_id,
            params,
            product_client_id,
            runtime: current_runtime_metadata(),
            rpc_transport,
        });
    }

    pub fn track_subagent_thread_started(&self, input: SubAgentThreadStartedInput) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::SubAgentThreadStarted(input),
        ));
    }

    pub fn track_guardian_review(
        &self,
        tracking: &GuardianReviewTrackContext,
        result: GuardianReviewAnalyticsResult,
        completed_at_ms: u64,
    ) {
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::GuardianReview(
            Box::new(tracking.event_params(result, completed_at_ms)),
        )));
    }

    pub fn track_app_mentioned(&self, tracking: TrackEventsContext, mentions: Vec<AppInvocation>) {
        if mentions.is_empty() {
            return;
        }
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::AppMentioned(
            AppMentionedInput { tracking, mentions },
        )));
    }

    pub fn track_request(
        &self,
        connection_id: u64,
        request_id: RequestId,
        request: &ClientRequest,
    ) {
        if let ClientRequest::TurnInterrupt { params, .. } = request {
            if params.turn_id.is_empty() {
                return;
            }
            self.record_fact(AnalyticsFact::ExplicitClientInterruptRequest {
                connection_id,
                request_id,
                turn_id: params.turn_id.clone(),
                requested_at_ms: now_unix_millis(),
            });
            return;
        }
        if !matches!(
            request,
            ClientRequest::TurnStart { .. } | ClientRequest::TurnSteer { .. }
        ) {
            return;
        }
        self.record_fact(AnalyticsFact::ClientRequest {
            connection_id,
            request_id,
            request: Box::new(request.clone()),
        });
    }

    pub fn track_app_used(&self, tracking: TrackEventsContext, app: AppInvocation) {
        let Some(queue) = self.queue.as_ref() else {
            return;
        };
        if !queue.should_enqueue_app_used(&tracking, &app) {
            return;
        }
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::AppUsed(
            AppUsedInput { tracking, app },
        )));
    }

    pub fn track_hook_run(&self, tracking: TrackEventsContext, hook: HookRunFact) {
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::HookRun(
            HookRunInput { tracking, hook },
        )));
    }

    pub fn track_plugin_used(&self, tracking: TrackEventsContext, plugin: PluginTelemetryMetadata) {
        let Some(queue) = self.queue.as_ref() else {
            return;
        };
        if !queue.should_enqueue_plugin_used(&tracking, &plugin) {
            return;
        }
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::PluginUsed(
            crate::facts::PluginUsedInput { tracking, plugin },
        )));
    }

    pub fn track_plugin_install_requested(
        &self,
        tracking: TrackEventsContext,
        request: PluginInstallRequested,
    ) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::PluginInstallRequested(PluginInstallRequestedInput {
                tracking,
                request,
            }),
        ));
    }

    pub fn track_compaction(&self, event: crate::facts::CodexCompactionEvent) {
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::Compaction(
            Box::new(event),
        )));
    }

    pub fn track_goal_event(&self, event: CodexGoalEvent) {
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::Goal(Box::new(
            event,
        ))));
    }

    pub fn track_image_preparation(&self, fact: ImagePreparationFact) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::ImagePreparation(Box::new(fact)),
        ));
    }

    pub fn track_turn_resolved_config(&self, fact: TurnResolvedConfigFact) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::TurnResolvedConfig(Box::new(fact)),
        ));
    }

    pub fn track_turn_token_usage(&self, fact: TurnTokenUsageFact) {
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::TurnTokenUsage(
            Box::new(fact),
        )));
    }

    pub fn track_turn_profile(&self, fact: TurnProfileFact) {
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::TurnProfile(
            Box::new(fact),
        )));
    }

    pub fn track_turn_codex_error(&self, fact: TurnCodexErrorFact) {
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::TurnCodexError(
            Box::new(fact),
        )));
    }

    pub fn track_plugin_installed(&self, plugin: PluginTelemetryMetadata) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::PluginStateChanged(PluginStateChangedInput {
                plugin,
                state: PluginState::Installed,
            }),
        ));
    }

    pub fn track_plugin_install_failed(
        &self,
        plugin: PluginTelemetryMetadata,
        source: PluginInstallSource,
        error_type: String,
        sub_error_type: Option<String>,
    ) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::PluginInstallFailed(PluginInstallFailedInput {
                plugin,
                source,
                error_type,
                sub_error_type,
            }),
        ));
    }

    pub fn track_external_agent_config_import_completed(
        &self,
        input: ExternalAgentConfigImportCompletedInput,
    ) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::ExternalAgentConfigImportCompleted(input),
        ));
    }

    pub fn track_external_agent_config_import_failure(
        &self,
        input: ExternalAgentConfigImportFailureInput,
    ) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::ExternalAgentConfigImportFailure(input),
        ));
    }

    pub fn track_plugin_uninstalled(&self, plugin: PluginTelemetryMetadata) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::PluginStateChanged(PluginStateChangedInput {
                plugin,
                state: PluginState::Uninstalled,
            }),
        ));
    }

    pub fn track_plugin_enabled(&self, plugin: PluginTelemetryMetadata) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::PluginStateChanged(PluginStateChangedInput {
                plugin,
                state: PluginState::Enabled,
            }),
        ));
    }

    pub fn track_plugin_disabled(&self, plugin: PluginTelemetryMetadata) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::PluginStateChanged(PluginStateChangedInput {
                plugin,
                state: PluginState::Disabled,
            }),
        ));
    }

    pub(crate) fn record_fact(&self, input: AnalyticsFact) {
        if let Some(queue) = self.queue.as_ref() {
            queue.try_send(input);
        }
    }

    pub fn track_response(
        &self,
        connection_id: u64,
        request_id: RequestId,
        response: &ClientResponsePayload,
    ) {
        self.track_response_inner(
            connection_id,
            request_id,
            response,
            /*thread_originator*/ None,
        );
    }

    pub fn track_response_with_thread_originator(
        &self,
        connection_id: u64,
        request_id: RequestId,
        response: &ClientResponsePayload,
        thread_originator: String,
    ) {
        self.track_response_inner(connection_id, request_id, response, Some(thread_originator));
    }

    fn track_response_inner(
        &self,
        connection_id: u64,
        request_id: RequestId,
        response: &ClientResponsePayload,
        thread_originator: Option<String>,
    ) {
        if !matches!(
            response,
            ClientResponsePayload::ThreadStart(_)
                | ClientResponsePayload::ThreadResume(_)
                | ClientResponsePayload::ThreadFork(_)
                | ClientResponsePayload::TurnStart(_)
                | ClientResponsePayload::TurnSteer(_)
                | ClientResponsePayload::TurnInterrupt(_)
        ) {
            return;
        }
        if serde_json::to_writer(std::io::sink(), response).is_err() {
            return;
        }
        self.record_fact(AnalyticsFact::ClientResponse {
            connection_id,
            request_id,
            response: Box::new(response.clone()),
            thread_originator,
        });
    }

    pub fn track_error_response(
        &self,
        connection_id: u64,
        request_id: RequestId,
        error: JSONRPCErrorError,
        error_type: Option<AnalyticsJsonRpcError>,
    ) {
        self.record_fact(AnalyticsFact::ErrorResponse {
            connection_id,
            request_id,
            error,
            error_type,
        });
    }

    pub fn track_server_request(&self, connection_id: u64, request: ServerRequest) {
        self.record_fact(AnalyticsFact::ServerRequest {
            connection_id,
            request: Box::new(request),
        });
    }

    pub fn track_server_response(&self, completed_at_ms: u64, response: ServerResponse) {
        self.record_fact(AnalyticsFact::ServerResponse {
            completed_at_ms,
            response: Box::new(response),
        });
    }

    pub fn track_effective_permissions_approval_response(
        &self,
        completed_at_ms: u64,
        request_id: RequestId,
        response: RequestPermissionsResponse,
    ) {
        self.record_fact(AnalyticsFact::EffectivePermissionsApprovalResponse {
            completed_at_ms,
            request_id,
            response: Box::new(response),
        });
    }

    pub fn track_server_request_aborted(&self, completed_at_ms: u64, request_id: RequestId) {
        self.record_fact(AnalyticsFact::ServerRequestAborted {
            completed_at_ms,
            request_id,
        });
    }

    /// Records analytics-relevant notifications without cloning ignored variants.
    pub fn track_notification(&self, notification: &ServerNotification) {
        if !matches!(
            notification,
            ServerNotification::TurnStarted(_)
                | ServerNotification::TurnCompleted(_)
                | ServerNotification::TurnDiffUpdated(_)
                | ServerNotification::ItemStarted(_)
                | ServerNotification::ItemCompleted(_)
                | ServerNotification::ItemGuardianApprovalReviewStarted(_)
                | ServerNotification::ItemGuardianApprovalReviewCompleted(_)
        ) {
            return;
        }
        self.record_fact(AnalyticsFact::Notification(Box::new(notification.clone())));
    }
}

async fn send_track_events(
    auth_manager: &AuthManager,
    destination: &AnalyticsEventsDestination,
    mut events: Vec<TrackEventRequest>,
) {
    if events.is_empty() {
        return;
    }

    let Some(auth) = auth_manager.auth().await else {
        return;
    };
    // copybara:strip-for-public begin
    if internal_tool_input_identity(&auth).is_none() {
        events.retain(|event| !matches!(event, TrackEventRequest::InternalStructuredLog(_)));
    }
    // copybara:strip-for-public end
    if auth.is_api_key_auth() {
        events.retain(TrackEventRequest::can_send_with_api_key_auth);
    } else if !auth.uses_codex_backend() {
        return;
    }
    if events.is_empty() {
        return;
    }

    for events in track_event_request_batches(events) {
        send_track_events_request(&auth, destination, events).await;
    }
}
// copybara:strip-for-public begin
fn internal_tool_input_identity(auth: &CodexAuth) -> Option<(String, String)> {
    let email = auth.get_account_email()?;
    email
        .rsplit_once('@')
        .filter(|(_, domain)| domain.eq_ignore_ascii_case("openai.com"))?;
    let user_id = auth.get_chatgpt_user_id();
    let account_id = auth.get_account_id()?;

    // External token providers may carry the employee identity only in the ID token. When the
    // bearer token is also a parseable ChatGPT JWT, reject inconsistent credentials before queueing.
    if matches!(
        auth,
        CodexAuth::Chatgpt(_) | CodexAuth::ChatgptAuthTokens(_)
    ) && let Ok(token) = auth.get_token()
        && let Ok(claims) = codex_login::token_data::parse_chatgpt_jwt_claims(&token)
        && (claims
            .email
            .as_ref()
            .is_some_and(|token_email| !token_email.eq_ignore_ascii_case(&email))
            || claims
                .chatgpt_user_id
                .as_ref()
                .zip(user_id.as_ref())
                .is_some_and(|(token_user_id, user_id)| token_user_id != user_id)
            || claims
                .chatgpt_account_id
                .is_some_and(|token_account_id| token_account_id != account_id))
    {
        return None;
    }

    Some((account_id, user_id.unwrap_or(email)))
}

fn is_valid_internal_tool_input_metadata(value: &Value) -> bool {
    const COMMANDS: &str =
        "cargo curl docker gh git just kubectl make npm pnpm pytest python python3 rg yarn";
    const SUBCOMMANDS: &str = "add branch build checkout clean clone commit config describe diff exec fetch get init install issue list log login logs merge pr pull push rebase remote reset restore run show stash status switch test version view";
    const FLAGS: &str = "--all --api-key --cookie --data --data-binary --data-raw --dry-run --force --header --help --json --name-only --password --porcelain --quiet --recursive --request --short --stat --token --user --verbose -H -U -X -b -d -f -n -p -q -r -u -v";
    let is_allowlisted = |values: &str, value: &str| {
        values
            .split_ascii_whitespace()
            .any(|allowed| allowed == value)
    };

    let Some(metadata) = value.as_object() else {
        return false;
    };
    if metadata.is_empty() {
        return true;
    }
    let Some(commands) = metadata
        .get("commands")
        .and_then(Value::as_array)
        .filter(|commands| metadata.len() == 1 && commands.len() <= MAX_INTERNAL_TOOL_COMMANDS)
    else {
        return false;
    };

    commands.iter().all(|entry| {
        let Some(command) = entry.as_object() else {
            return false;
        };
        if !command
            .keys()
            .all(|key| matches!(key.as_str(), "command" | "subcommand" | "flags"))
        {
            return false;
        }
        if !command
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|name| is_allowlisted(COMMANDS, name))
        {
            return false;
        }
        if !command.get("subcommand").is_none_or(|subcommand| {
            subcommand
                .as_str()
                .is_some_and(|subcommand| is_allowlisted(SUBCOMMANDS, subcommand))
        }) {
            return false;
        }
        command
            .get("flags")
            .and_then(Value::as_array)
            .is_some_and(|flags| {
                flags.len() <= MAX_INTERNAL_TOOL_FLAGS
                    && flags.iter().all(|flag| {
                        flag.as_str()
                            .is_some_and(|flag| is_allowlisted(FLAGS, flag))
                    })
            })
    })
}
// copybara:strip-for-public end

fn track_event_request_batches(events: Vec<TrackEventRequest>) -> Vec<Vec<TrackEventRequest>> {
    let mut batches = Vec::new();
    let mut current_batch = Vec::new();

    for event in events {
        if event.should_send_in_isolated_request() {
            if !current_batch.is_empty() {
                batches.push(current_batch);
                current_batch = Vec::new();
            }
            batches.push(vec![event]);
        } else {
            current_batch.push(event);
        }
    }

    if !current_batch.is_empty() {
        batches.push(current_batch);
    }

    batches
}

async fn send_track_events_request(
    auth: &CodexAuth,
    destination: &AnalyticsEventsDestination,
    events: Vec<TrackEventRequest>,
) {
    if events.is_empty() {
        return;
    }

    let payload = TrackEventsRequest { events };

    #[cfg(debug_assertions)]
    if capture_track_events_request(destination, &payload) {
        return;
    }

    let url = match destination {
        AnalyticsEventsDestination::Http { url } => url,
        #[cfg(debug_assertions)]
        AnalyticsEventsDestination::CaptureFile { .. } => return,
    };
    let response = create_client()
        .post(url)
        .timeout(ANALYTICS_EVENTS_TIMEOUT)
        .headers(codex_model_provider::auth_provider_from_auth(auth).to_auth_headers())
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;

    match response {
        Ok(response) if response.status().is_success() => {}
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("events failed with status {status}: {body}");
        }
        Err(err) => {
            tracing::warn!("failed to send events request: {err}");
        }
    }
}

#[cfg(debug_assertions)]
fn capture_track_events_request(
    destination: &AnalyticsEventsDestination,
    payload: &TrackEventsRequest,
) -> bool {
    let AnalyticsEventsDestination::CaptureFile { path } = destination else {
        return false;
    };

    // copybara:strip-for-public begin
    use std::sync::PoisonError;
    static CAPTURE_LOCK: Mutex<()> = Mutex::new(());
    let _capture_guard = CAPTURE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    // copybara:strip-for-public end
    if let Err(err) = crate::analytics_capture::append_payload(path, payload) {
        tracing::error!(
            path = %path.display(),
            "failed to capture analytics events; network delivery remains disabled: {err}"
        );
    }
    true
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
