use crate::events::AppServerRpcTransport;
use crate::events::GuardianReviewAnalyticsResult;
use crate::events::GuardianReviewTrackContext;
use crate::events::current_runtime_metadata;
use crate::facts::AnalyticsFact;
use crate::facts::AnalyticsJsonRpcError;
use crate::facts::AppInvocation;
use crate::facts::AppMentionedInput;
use crate::facts::AppUsedInput;
use crate::facts::CustomAnalyticsFact;
use crate::facts::HookRunFact;
use crate::facts::HookRunInput;
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
use crate::reducer::AnalyticsReducer;
use crate::sinks::AnalyticsEvent;
use crate::sinks::AnalyticsEventSink;
use crate::sinks::SharedLocalAnalyticsSink;
use crate::sinks::local_analytics_sink_from_env;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
use codex_login::AuthManager;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;

#[cfg(test)]
use crate::sinks::local_analytics_sink_for_path;
#[cfg(test)]
use std::path::PathBuf;

const ANALYTICS_EVENTS_QUEUE_SIZE: usize = 256;
const ANALYTICS_EVENT_DEDUPE_MAX_KEYS: usize = 4096;

#[derive(Clone)]
pub(crate) struct AnalyticsEventsQueue {
    pub(crate) sender: mpsc::Sender<AnalyticsFact>,
    pub(crate) app_used_emitted_keys: Arc<Mutex<HashSet<(String, String)>>>,
    pub(crate) plugin_used_emitted_keys: Arc<Mutex<HashSet<(String, String)>>>,
}

#[derive(Clone)]
pub struct AnalyticsEventsClient {
    queue: Option<AnalyticsEventsQueue>,
}

impl AnalyticsEventsQueue {
    pub(crate) fn new(sinks: Vec<AnalyticsEventSink>) -> Self {
        let (sender, mut receiver) = mpsc::channel(ANALYTICS_EVENTS_QUEUE_SIZE);
        tokio::spawn(async move {
            let mut reducer = AnalyticsReducer::default();
            while let Some(input) = receiver.recv().await {
                let mut codex_analytics_events = Vec::new();
                reducer.ingest(input, &mut codex_analytics_events).await;
                let events = codex_analytics_events
                    .into_iter()
                    .map(AnalyticsEvent::from)
                    .collect::<Vec<_>>();
                for sink in &sinks {
                    sink.write(&events).await;
                }
            }
        });
        Self {
            sender,
            app_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
            plugin_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    fn try_send(&self, input: AnalyticsFact) {
        if self.sender.try_send(input).is_err() {
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
        emitted.insert((tracking.turn_id.clone(), plugin.plugin_id.as_key()))
    }
}

impl AnalyticsEventsClient {
    pub fn new(
        auth_manager: Arc<AuthManager>,
        base_url: String,
        analytics_enabled: Option<bool>,
    ) -> Self {
        Self::new_with_local_sink(
            auth_manager,
            base_url,
            analytics_enabled,
            local_analytics_sink_from_env(),
        )
    }

    pub fn disabled() -> Self {
        Self { queue: None }
    }

    #[cfg(test)]
    pub(crate) fn new_with_local_sink_path(
        auth_manager: Arc<AuthManager>,
        base_url: String,
        analytics_enabled: Option<bool>,
        local_sink_path: Option<PathBuf>,
    ) -> Self {
        Self::new_with_local_sink(
            auth_manager,
            base_url,
            analytics_enabled,
            local_sink_path.and_then(local_analytics_sink_for_path),
        )
    }

    fn new_with_local_sink(
        auth_manager: Arc<AuthManager>,
        base_url: String,
        analytics_enabled: Option<bool>,
        local_sink: Option<SharedLocalAnalyticsSink>,
    ) -> Self {
        let mut sinks = Vec::new();
        if let Some(local_sink) = local_sink {
            sinks.push(AnalyticsEventSink::Local(local_sink));
        }
        if analytics_enabled != Some(false) {
            sinks.push(AnalyticsEventSink::CodexBackend {
                auth_manager,
                base_url,
            });
        }
        Self {
            queue: (!sinks.is_empty()).then(|| AnalyticsEventsQueue::new(sinks)),
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

    pub fn track_compaction(&self, event: crate::facts::CodexCompactionEvent) {
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::Compaction(
            Box::new(event),
        )));
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
        response: ClientResponsePayload,
    ) {
        if !matches!(
            response,
            ClientResponsePayload::ThreadStart(_)
                | ClientResponsePayload::ThreadResume(_)
                | ClientResponsePayload::ThreadFork(_)
                | ClientResponsePayload::TurnStart(_)
                | ClientResponsePayload::TurnSteer(_)
        ) {
            return;
        }
        self.record_fact(AnalyticsFact::ClientResponse {
            connection_id,
            request_id,
            response: Box::new(response),
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

    pub fn track_notification(&self, notification: ServerNotification) {
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
        self.record_fact(AnalyticsFact::Notification(Box::new(notification)));
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
