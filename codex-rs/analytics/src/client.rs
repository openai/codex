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
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnAppAttribution;
use codex_app_server_protocol::TurnAttribution;
use codex_app_server_protocol::TurnPluginAttribution;
use codex_app_server_protocol::TurnSkillAttribution;
use codex_app_server_protocol::TurnToolAttribution;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::default_client::create_client;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use std::collections::HashMap;
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
    queue: Option<AnalyticsEventsQueue>,
    turn_attributions: Arc<Mutex<HashMap<String, TurnAttribution>>>,
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
            queue: (analytics_enabled != Some(false))
                .then(|| AnalyticsEventsQueue::new(Arc::clone(&auth_manager), base_url)),
            turn_attributions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn disabled() -> Self {
        Self {
            queue: None,
            turn_attributions: Arc::new(Mutex::new(HashMap::new())),
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
        self.record_skill_invocations(&tracking, &invocations);
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
            self.record_app_used(&tracking, &app);
            return;
        };
        if !queue.should_enqueue_app_used(&tracking, &app) {
            return;
        }
        self.record_app_used(&tracking, &app);
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
            self.record_plugin_used(&tracking, &plugin);
            return;
        };
        if !queue.should_enqueue_plugin_used(&tracking, &plugin) {
            return;
        }
        self.record_plugin_used(&tracking, &plugin);
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::PluginUsed(
            crate::facts::PluginUsedInput { tracking, plugin },
        )));
    }

    pub fn take_turn_attribution(&self, turn_id: &str) -> Option<TurnAttribution> {
        let attribution = self
            .turn_attributions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(turn_id)?;
        (!attribution.is_empty()).then_some(attribution)
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
        self.record_tool_from_notification(&notification);
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

    fn update_turn_attribution(&self, turn_id: &str, update: impl FnOnce(&mut TurnAttribution)) {
        let mut attributions = self
            .turn_attributions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        update(attributions.entry(turn_id.to_string()).or_default());
    }

    fn record_skill_invocations(
        &self,
        tracking: &TrackEventsContext,
        invocations: &[SkillInvocation],
    ) {
        self.update_turn_attribution(&tracking.turn_id, |attribution| {
            for invocation in invocations {
                let skill = TurnSkillAttribution {
                    skill_id: crate::reducer::skill_id_for_local_skill(
                        None,
                        None,
                        invocation.skill_path.as_path(),
                        invocation.skill_name.as_str(),
                    ),
                    skill_name: invocation.skill_name.clone(),
                    skill_scope: Some(skill_scope_name(invocation.skill_scope).to_string()),
                    plugin_id: invocation.plugin_id.clone(),
                    invoke_type: Some(invocation_type_name(invocation.invocation_type).to_string()),
                };
                push_unique(&mut attribution.skills, skill);
            }
        });
    }

    fn record_app_used(&self, tracking: &TrackEventsContext, app: &AppInvocation) {
        self.update_turn_attribution(&tracking.turn_id, |attribution| {
            let app = TurnAppAttribution {
                connector_id: app.connector_id.clone(),
                app_name: app.app_name.clone(),
                invoke_type: app
                    .invocation_type
                    .map(invocation_type_name)
                    .map(str::to_string),
            };
            push_unique(&mut attribution.apps, app);
        });
    }

    fn record_plugin_used(&self, tracking: &TrackEventsContext, plugin: &PluginTelemetryMetadata) {
        self.update_turn_attribution(&tracking.turn_id, |attribution| {
            let plugin = TurnPluginAttribution {
                plugin_id: plugin
                    .remote_plugin_id
                    .clone()
                    .unwrap_or_else(|| plugin.plugin_id.as_key()),
                plugin_name: plugin.plugin_id.plugin_name.clone(),
                marketplace_name: plugin.plugin_id.marketplace_name.clone(),
                display_name: plugin
                    .capability_summary
                    .as_ref()
                    .map(|summary| summary.display_name.clone()),
            };
            push_unique(&mut attribution.plugins, plugin);
        });
    }

    fn record_tool_from_notification(&self, notification: &ServerNotification) {
        let (turn_id, item) = match notification {
            ServerNotification::ItemStarted(notification) => {
                (&notification.turn_id, &notification.item)
            }
            _ => return,
        };
        let Some(tool) = tool_attribution_from_item(item) else {
            return;
        };
        self.update_turn_attribution(turn_id, |attribution| {
            push_unique(&mut attribution.tools, tool);
        });
    }
}

fn push_unique<T: PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) {
        items.push(item);
    }
}

fn invocation_type_name(invocation_type: crate::facts::InvocationType) -> &'static str {
    match invocation_type {
        crate::facts::InvocationType::Explicit => "explicit",
        crate::facts::InvocationType::Implicit => "implicit",
    }
}

fn skill_scope_name(skill_scope: codex_protocol::protocol::SkillScope) -> &'static str {
    match skill_scope {
        codex_protocol::protocol::SkillScope::User => "user",
        codex_protocol::protocol::SkillScope::Repo => "repo",
        codex_protocol::protocol::SkillScope::System => "system",
        codex_protocol::protocol::SkillScope::Admin => "admin",
    }
}

fn tool_attribution_from_item(item: &ThreadItem) -> Option<TurnToolAttribution> {
    match item {
        ThreadItem::CommandExecution { id, .. } => Some(TurnToolAttribution {
            id: id.clone(),
            kind: "command_execution".to_string(),
            name: Some("shell".to_string()),
            server: None,
            plugin_id: None,
        }),
        ThreadItem::McpToolCall {
            id,
            server,
            tool,
            plugin_id,
            ..
        } => Some(TurnToolAttribution {
            id: id.clone(),
            kind: "mcp".to_string(),
            name: Some(tool.clone()),
            server: Some(server.clone()),
            plugin_id: plugin_id.clone(),
        }),
        ThreadItem::DynamicToolCall {
            id,
            namespace,
            tool,
            ..
        } => Some(TurnToolAttribution {
            id: id.clone(),
            kind: "dynamic".to_string(),
            name: Some(tool.clone()),
            server: namespace.clone(),
            plugin_id: None,
        }),
        ThreadItem::CollabAgentToolCall { id, tool, .. } => Some(TurnToolAttribution {
            id: id.clone(),
            kind: "collab_agent".to_string(),
            name: Some(format!("{tool:?}")),
            server: None,
            plugin_id: None,
        }),
        ThreadItem::WebSearch { id, .. } => Some(TurnToolAttribution {
            id: id.clone(),
            kind: "web_search".to_string(),
            name: Some("web_search".to_string()),
            server: None,
            plugin_id: None,
        }),
        ThreadItem::ImageGeneration { id, .. } => Some(TurnToolAttribution {
            id: id.clone(),
            kind: "image_generation".to_string(),
            name: Some("image_generation".to_string()),
            server: None,
            plugin_id: None,
        }),
        _ => None,
    }
}

async fn send_track_events(
    auth_manager: &AuthManager,
    base_url: &str,
    events: Vec<TrackEventRequest>,
) {
    if events.is_empty() {
        return;
    }

    let Some(auth) = auth_manager.auth().await else {
        return;
    };
    if !auth.uses_codex_backend() {
        return;
    }

    let base_url = base_url.trim_end_matches('/');
    let url = format!("{base_url}/codex/analytics-events/events");
    for events in track_event_request_batches(events) {
        send_track_events_request(&auth, &url, events).await;
    }
}

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

async fn send_track_events_request(auth: &CodexAuth, url: &str, events: Vec<TrackEventRequest>) {
    if events.is_empty() {
        return;
    }

    let payload = TrackEventsRequest { events };

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

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
