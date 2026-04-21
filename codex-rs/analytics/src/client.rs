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
use crate::facts::CodexResponsesApiCallFact;
use crate::facts::CodexResponsesApiCallInput;
use crate::facts::CodexResponsesApiItemPhase;
use crate::facts::CustomAnalyticsFact;
use crate::facts::HookRunFact;
use crate::facts::HookRunInput;
use crate::facts::PluginState;
use crate::facts::PluginStateChangedInput;
use crate::facts::SkillInvocation;
use crate::facts::SkillInvokedInput;
use crate::facts::SubAgentThreadStartedInput;
use crate::facts::TrackEventsContext;
use crate::facts::TurnResolvedConfigFact;
use crate::facts::TurnTokenUsageFact;
use crate::reducer::AnalyticsReducer;
use crate::response_items::response_items_metadata;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_login::AuthManager;
use codex_login::default_client::create_client;
use codex_plugin::PluginTelemetryMetadata;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc;

const ANALYTICS_EVENTS_QUEUE_SIZE: usize = 256;
const ANALYTICS_EVENTS_TIMEOUT: Duration = Duration::from_secs(10);
const ANALYTICS_EVENT_DEDUPE_MAX_KEYS: usize = 4096;

#[derive(Clone)]
pub(crate) struct AnalyticsEventsQueue {
    pub(crate) sender: mpsc::Sender<AnalyticsFact>,
    pub(crate) app_used_emitted_keys: Arc<Mutex<HashSet<(String, String)>>>,
    pub(crate) plugin_used_emitted_keys: Arc<Mutex<HashSet<(String, String)>>>,
}

#[derive(Clone)]
pub struct AnalyticsEventsClient {
    queue: AnalyticsEventsQueue,
    analytics_enabled: Option<bool>,
}

impl AnalyticsEventsQueue {
    pub(crate) fn new(auth_manager: Arc<AuthManager>, base_url: String) -> Self {
        let (sender, mut receiver) = mpsc::channel(ANALYTICS_EVENTS_QUEUE_SIZE);
        tokio::spawn(async move {
            let mut reducer = AnalyticsReducer::default();
            while let Some(input) = receiver.recv().await {
                let mut events = Vec::new();
                reducer.ingest(input, &mut events).await;
                send_track_events(&auth_manager, &base_url, events).await;
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
        Self {
            queue: AnalyticsEventsQueue::new(Arc::clone(&auth_manager), base_url),
            analytics_enabled,
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
    ) {
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::GuardianReview(
            Box::new(tracking.event_params(result)),
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

    pub fn track_request(&self, connection_id: u64, request_id: RequestId, request: ClientRequest) {
        self.record_fact(AnalyticsFact::Request {
            connection_id,
            request_id,
            request: Box::new(request),
        });
    }

    pub fn track_app_used(&self, tracking: TrackEventsContext, app: AppInvocation) {
        if !self.queue.should_enqueue_app_used(&tracking, &app) {
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
        if !self.queue.should_enqueue_plugin_used(&tracking, &plugin) {
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

    pub fn track_responses_api_call(
        &self,
        tracking: TrackEventsContext,
        input: CodexResponsesApiCallInput,
    ) {
        let mut items =
            response_items_metadata(CodexResponsesApiItemPhase::Input, &input.input_items);
        items.extend(response_items_metadata(
            CodexResponsesApiItemPhase::Output,
            &input.output_items,
        ));
        let token_usage = input.token_usage;
        let event = CodexResponsesApiCallFact {
            thread_id: tracking.thread_id,
            turn_id: tracking.turn_id,
            responses_id: input.responses_id,
            turn_responses_call_index: input.turn_responses_call_index,
            status: input.status,
            error: input.error,
            started_at: input.started_at,
            completed_at: input.completed_at,
            duration_ms: input.duration_ms,
            input_item_count: input.input_items.len(),
            output_item_count: input.output_items.len(),
            input_tokens: token_usage.as_ref().map(|usage| usage.input_tokens),
            cached_input_tokens: token_usage.as_ref().map(|usage| usage.cached_input_tokens),
            output_tokens: token_usage.as_ref().map(|usage| usage.output_tokens),
            reasoning_output_tokens: token_usage
                .as_ref()
                .map(|usage| usage.reasoning_output_tokens),
            total_tokens: token_usage.as_ref().map(|usage| usage.total_tokens),
            items,
        };

        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::ResponsesApiCall(Box::new(event)),
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
        if self.analytics_enabled == Some(false) {
            return;
        }
        self.queue.try_send(input);
    }

    pub fn track_response(&self, connection_id: u64, response: ClientResponse) {
        self.record_fact(AnalyticsFact::Response {
            connection_id,
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

    pub fn track_notification(&self, notification: ServerNotification) {
        self.record_fact(AnalyticsFact::Notification(Box::new(notification)));
    }
}

async fn send_track_events(
    auth_manager: &Arc<AuthManager>,
    base_url: &str,
    events: Vec<TrackEventRequest>,
) {
    if events.is_empty() {
        return;
    }
    let Some(auth) = auth_manager.auth().await else {
        return;
    };
    if !auth.is_chatgpt_auth() {
        return;
    }
    let Some(authorization_header_value) = auth_manager
        .chatgpt_authorization_header_for_auth(&auth)
        .await
    else {
        return;
    };
    let Some(account_id) = auth.get_account_id() else {
        return;
    };

    let base_url = base_url.trim_end_matches('/');
    let url = format!("{base_url}/codex/analytics-events/events");
    let payload = TrackEventsRequest { events };

    let mut request = create_client()
        .post(&url)
        .timeout(ANALYTICS_EVENTS_TIMEOUT)
        .header("authorization", authorization_header_value)
        .header("chatgpt-account-id", &account_id)
        .header("Content-Type", "application/json")
        .json(&payload);
    if auth.is_fedramp_account() {
        request = request.header("X-OpenAI-Fedramp", "true");
    }
    let response = request.send().await;

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
