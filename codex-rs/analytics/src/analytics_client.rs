use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_git_utils::collect_git_info;
use codex_git_utils::get_git_repo_root;
use codex_login::AuthManager;
use codex_login::default_client::create_client;
use codex_login::default_client::originator;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct TrackEventsContext {
    pub model_slug: String,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Clone)]
pub struct TurnResolvedConfigFact {
    pub turn_id: String,
    pub thread_id: String,
    pub num_input_images: usize,
    pub submission_type: Option<TurnSubmissionType>,
    pub model: String,
    pub model_provider: String,
    pub sandbox_policy: SandboxPolicy,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_summary: Option<ReasoningSummary>,
    pub service_tier: Option<ServiceTier>,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    pub sandbox_network_access: bool,
    pub collaboration_mode: ModeKind,
    pub personality: Option<Personality>,
    pub is_first_turn: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnSubmissionType {
    Default,
    Queued,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone)]
struct ThreadInitializedInput {
    pub connection_id: u64,
    pub thread_id: String,
    pub model: String,
    pub ephemeral: bool,
    pub session_source: SessionSource,
    pub initialization_mode: InitializationMode,
    pub created_at: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InitializationMode {
    New,
    Forked,
    Resumed,
}

pub fn build_track_events_context(
    model_slug: String,
    thread_id: String,
    turn_id: String,
) -> TrackEventsContext {
    TrackEventsContext {
        model_slug,
        thread_id,
        turn_id,
    }
}

#[derive(Clone, Debug)]
pub struct SkillInvocation {
    pub skill_name: String,
    pub skill_scope: SkillScope,
    pub skill_path: PathBuf,
    pub invocation_type: InvocationType,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InvocationType {
    Explicit,
    Implicit,
}

pub struct AppInvocation {
    pub connector_id: Option<String>,
    pub app_name: Option<String>,
    pub invocation_type: Option<InvocationType>,
}

pub enum AnalyticsFact {
    Initialize {
        connection_id: u64,
        params: InitializeParams,
    },
    Request {
        connection_id: u64,
        request_id: RequestId,
        request: Box<ClientRequest>,
    },
    Response {
        connection_id: u64,
        response: Box<ClientResponse>,
    },
    Notification(Box<ServerNotification>),
    // Facts that do not naturally exist on the app-server protocol surface, or
    // would require non-trivial protocol reshaping on this branch.
    Custom(CustomAnalyticsFact),
}

pub enum CustomAnalyticsFact {
    TurnResolvedConfig(Box<TurnResolvedConfigFact>),
    SkillInvoked(SkillInvokedInput),
    AppMentioned(AppMentionedInput),
    AppUsed(AppUsedInput),
    PluginUsed(PluginUsedInput),
    PluginStateChanged(PluginStateChangedInput),
}

pub struct SkillInvokedInput {
    pub tracking: TrackEventsContext,
    pub invocations: Vec<SkillInvocation>,
}

pub struct AppMentionedInput {
    pub tracking: TrackEventsContext,
    pub mentions: Vec<AppInvocation>,
}

pub struct AppUsedInput {
    pub tracking: TrackEventsContext,
    pub app: AppInvocation,
}

pub struct PluginUsedInput {
    pub tracking: TrackEventsContext,
    pub plugin: PluginTelemetryMetadata,
}

pub struct PluginStateChangedInput {
    pub plugin: PluginTelemetryMetadata,
    pub state: PluginState,
}

#[derive(Clone, Copy)]
pub enum PluginState {
    Installed,
    Uninstalled,
    Enabled,
    Disabled,
}

#[derive(Default)]
pub struct AnalyticsReducer {
    requests: HashMap<(u64, RequestId), RequestState>,
    turns: HashMap<String, TurnState>,
    connections: HashMap<u64, ConnectionState>,
}

struct ConnectionState {
    product_client_id: String,
    client_name: Option<String>,
    client_version: Option<String>,
    experimental_api_enabled: Option<bool>,
}

enum RequestState {
    TurnStart(PendingTurnStartState),
}

struct PendingTurnStartState {
    thread_id: String,
    num_input_images: usize,
}

#[derive(Clone)]
struct CompletedTurnState {
    status: Option<TurnStatus>,
    turn_error: Option<CodexErrorInfo>,
    completed_at_secs: u64,
    duration_ms: Option<u64>,
}

struct TurnState {
    connection_id: Option<u64>,
    thread_id: Option<String>,
    num_input_images: Option<usize>,
    resolved_config: Option<TurnResolvedConfigFact>,
    started_at_ms: Option<u64>,
    completed: Option<CompletedTurnState>,
}

#[derive(Clone)]
pub(crate) struct AnalyticsEventsQueue {
    sender: mpsc::Sender<AnalyticsFact>,
    app_used_emitted_keys: Arc<Mutex<HashSet<(String, String)>>>,
    plugin_used_emitted_keys: Arc<Mutex<HashSet<(String, String)>>>,
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
            while let Some(job) = receiver.recv().await {
                let mut events = Vec::new();
                reducer.ingest(job, &mut events).await;
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

    fn should_enqueue_app_used(&self, tracking: &TrackEventsContext, app: &AppInvocation) -> bool {
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

    fn should_enqueue_plugin_used(
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

    pub fn track_initialize(&self, connection_id: u64, params: InitializeParams) {
        self.record_fact(AnalyticsFact::Initialize {
            connection_id,
            params,
        });
    }

    pub fn track_request(&self, connection_id: u64, request_id: RequestId, request: ClientRequest) {
        self.record_fact(AnalyticsFact::Request {
            connection_id,
            request_id,
            request: Box::new(request),
        });
    }

    pub fn track_app_mentioned(&self, tracking: TrackEventsContext, mentions: Vec<AppInvocation>) {
        if mentions.is_empty() {
            return;
        }
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::AppMentioned(
            AppMentionedInput { tracking, mentions },
        )));
    }

    pub fn track_app_used(&self, tracking: TrackEventsContext, app: AppInvocation) {
        if !self.queue.should_enqueue_app_used(&tracking, &app) {
            return;
        }
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::AppUsed(
            AppUsedInput { tracking, app },
        )));
    }

    pub fn track_plugin_used(&self, tracking: TrackEventsContext, plugin: PluginTelemetryMetadata) {
        if !self.queue.should_enqueue_plugin_used(&tracking, &plugin) {
            return;
        }
        self.record_fact(AnalyticsFact::Custom(CustomAnalyticsFact::PluginUsed(
            PluginUsedInput { tracking, plugin },
        )));
    }

    pub fn track_turn_resolved_config(&self, fact: TurnResolvedConfigFact) {
        self.record_fact(AnalyticsFact::Custom(
            CustomAnalyticsFact::TurnResolvedConfig(Box::new(fact)),
        ));
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

    pub fn record_fact(&self, input: AnalyticsFact) {
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

    pub fn track_notification(&self, notification: ServerNotification) {
        self.record_fact(AnalyticsFact::Notification(Box::new(notification)));
    }
}

const ANALYTICS_EVENTS_QUEUE_SIZE: usize = 256;
const ANALYTICS_EVENTS_TIMEOUT: Duration = Duration::from_secs(10);
const ANALYTICS_EVENT_DEDUPE_MAX_KEYS: usize = 4096;

#[derive(Serialize)]
struct TrackEventsRequest {
    events: Vec<TrackEventRequest>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum TrackEventRequest {
    SkillInvocation(SkillInvocationEventRequest),
    ThreadInitialized(ThreadInitializedEvent),
    AppMentioned(CodexAppMentionedEventRequest),
    AppUsed(CodexAppUsedEventRequest),
    TurnEvent(Box<CodexTurnEventRequest>),
    PluginUsed(CodexPluginUsedEventRequest),
    PluginInstalled(CodexPluginEventRequest),
    PluginUninstalled(CodexPluginEventRequest),
    PluginEnabled(CodexPluginEventRequest),
    PluginDisabled(CodexPluginEventRequest),
}

#[derive(Serialize)]
struct SkillInvocationEventRequest {
    event_type: &'static str,
    skill_id: String,
    skill_name: String,
    event_params: SkillInvocationEventParams,
}

#[derive(Serialize)]
struct SkillInvocationEventParams {
    product_client_id: Option<String>,
    skill_scope: Option<String>,
    repo_url: Option<String>,
    thread_id: Option<String>,
    invoke_type: Option<InvocationType>,
    model_slug: Option<String>,
}

#[derive(Serialize)]
struct ThreadInitializedEventParams {
    thread_id: String,
    product_client_id: String,
    client_name: Option<String>,
    client_version: Option<String>,
    experimental_api_enabled: Option<bool>,
    model: String,
    ephemeral: bool,
    session_source: Option<&'static str>,
    initialization_mode: InitializationMode,
    subagent_source: Option<String>,
    parent_thread_id: Option<String>,
    created_at: u64,
}

#[derive(Serialize)]
struct ThreadInitializedEvent {
    event_type: &'static str,
    event_params: ThreadInitializedEventParams,
}

#[derive(Serialize)]
struct CodexAppMetadata {
    connector_id: Option<String>,
    thread_id: Option<String>,
    turn_id: Option<String>,
    app_name: Option<String>,
    product_client_id: Option<String>,
    invoke_type: Option<InvocationType>,
    model_slug: Option<String>,
}

#[derive(Serialize)]
struct CodexAppMentionedEventRequest {
    event_type: &'static str,
    event_params: CodexAppMetadata,
}

#[derive(Serialize)]
struct CodexAppUsedEventRequest {
    event_type: &'static str,
    event_params: CodexAppMetadata,
}

#[derive(Serialize)]
struct CodexTurnEventParams {
    thread_id: String,
    turn_id: String,
    product_client_id: Option<String>,
    submission_type: Option<TurnSubmissionType>,
    model: Option<String>,
    model_provider: String,
    sandbox_policy: Option<&'static str>,
    reasoning_effort: Option<String>,
    reasoning_summary: Option<String>,
    service_tier: String,
    approval_policy: String,
    approvals_reviewer: String,
    sandbox_network_access: bool,
    collaboration_mode: Option<&'static str>,
    personality: Option<String>,
    num_input_images: usize,
    is_first_turn: bool,
    status: Option<TurnStatus>,
    turn_error: Option<CodexErrorInfo>,
    steer_count: Option<usize>,
    total_tool_call_count: Option<usize>,
    shell_command_count: Option<usize>,
    file_change_count: Option<usize>,
    mcp_tool_call_count: Option<usize>,
    dynamic_tool_call_count: Option<usize>,
    subagent_tool_call_count: Option<usize>,
    web_search_count: Option<usize>,
    image_generation_count: Option<usize>,
    input_tokens: Option<i64>,
    cached_input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    reasoning_output_tokens: Option<i64>,
    total_tokens: Option<i64>,
    duration_ms: Option<u64>,
    started_at: Option<u64>,
    completed_at: Option<u64>,
}

#[derive(Serialize)]
struct CodexTurnEventRequest {
    event_type: &'static str,
    event_params: CodexTurnEventParams,
}

#[derive(Serialize)]
struct CodexPluginMetadata {
    plugin_id: Option<String>,
    plugin_name: Option<String>,
    marketplace_name: Option<String>,
    has_skills: Option<bool>,
    mcp_server_count: Option<usize>,
    connector_ids: Option<Vec<String>>,
    product_client_id: Option<String>,
}

#[derive(Serialize)]
struct CodexPluginUsedMetadata {
    #[serde(flatten)]
    plugin: CodexPluginMetadata,
    thread_id: Option<String>,
    turn_id: Option<String>,
    model_slug: Option<String>,
}

#[derive(Serialize)]
struct CodexPluginEventRequest {
    event_type: &'static str,
    event_params: CodexPluginMetadata,
}

#[derive(Serialize)]
struct CodexPluginUsedEventRequest {
    event_type: &'static str,
    event_params: CodexPluginUsedMetadata,
}

impl AnalyticsReducer {
    async fn ingest(&mut self, input: AnalyticsFact, out: &mut Vec<TrackEventRequest>) {
        match input {
            AnalyticsFact::Initialize {
                connection_id,
                params,
            } => {
                self.ingest_initialize(connection_id, params, out);
            }
            AnalyticsFact::Request {
                connection_id,
                request_id,
                request,
            } => {
                self.ingest_request(connection_id, request_id, *request);
            }
            AnalyticsFact::Response {
                connection_id,
                response,
            } => {
                self.ingest_response(connection_id, *response, out);
            }
            AnalyticsFact::Notification(notification) => {
                self.ingest_notification(*notification, out);
            }
            AnalyticsFact::Custom(input) => match input {
                CustomAnalyticsFact::TurnResolvedConfig(input) => {
                    self.ingest_turn_resolved_config(*input, out);
                }
                CustomAnalyticsFact::SkillInvoked(input) => {
                    self.ingest_skill_invoked(input, out).await;
                }
                CustomAnalyticsFact::AppMentioned(input) => {
                    self.ingest_app_mentioned(input, out);
                }
                CustomAnalyticsFact::AppUsed(input) => {
                    self.ingest_app_used(input, out);
                }
                CustomAnalyticsFact::PluginUsed(input) => {
                    self.ingest_plugin_used(input, out);
                }
                CustomAnalyticsFact::PluginStateChanged(input) => {
                    self.ingest_plugin_state_changed(input, out);
                }
            },
        }
    }

    fn ingest_initialize(
        &mut self,
        connection_id: u64,
        params: InitializeParams,
        out: &mut Vec<TrackEventRequest>,
    ) {
        self.connections.insert(
            connection_id,
            ConnectionState {
                product_client_id: params.client_info.name.clone(),
                client_name: Some(params.client_info.name),
                client_version: Some(params.client_info.version),
                experimental_api_enabled: params
                    .capabilities
                    .map(|capabilities| capabilities.experimental_api),
            },
        );
        let ready_turn_ids = self
            .turns
            .iter()
            .filter_map(|(turn_id, state)| {
                (state.connection_id == Some(connection_id)
                    && state.resolved_config.is_some()
                    && state.completed.is_some())
                .then_some(turn_id.clone())
            })
            .collect::<Vec<_>>();
        for turn_id in ready_turn_ids {
            self.maybe_emit_turn_event(&turn_id, out);
        }
    }

    fn ingest_thread_initialized(
        &mut self,
        input: ThreadInitializedInput,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let Some(connection_state) = self.connections.get(&input.connection_id) else {
            return;
        };
        out.push(TrackEventRequest::ThreadInitialized(
            thread_initialized_event_request(connection_state, input),
        ));
    }

    fn ingest_request(
        &mut self,
        connection_id: u64,
        request_id: RequestId,
        request: ClientRequest,
    ) {
        let ClientRequest::TurnStart { params, .. } = request else {
            return;
        };
        self.requests.insert(
            (connection_id, request_id),
            RequestState::TurnStart(PendingTurnStartState {
                thread_id: params.thread_id,
                num_input_images: params
                    .input
                    .iter()
                    .filter(|item| {
                        matches!(
                            item,
                            codex_app_server_protocol::UserInput::Image { .. }
                                | codex_app_server_protocol::UserInput::LocalImage { .. }
                        )
                    })
                    .count(),
            }),
        );
    }

    fn ingest_turn_resolved_config(
        &mut self,
        input: TurnResolvedConfigFact,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let turn_id = input.turn_id.clone();
        let thread_id = input.thread_id.clone();
        let num_input_images = input.num_input_images;
        let turn_state = self.turns.entry(turn_id.clone()).or_insert(TurnState {
            connection_id: None,
            thread_id: None,
            num_input_images: None,
            resolved_config: None,
            started_at_ms: None,
            completed: None,
        });
        turn_state.thread_id = Some(thread_id);
        turn_state.num_input_images = Some(num_input_images);
        turn_state.resolved_config = Some(input);
        self.maybe_emit_turn_event(&turn_id, out);
    }

    fn ingest_response(
        &mut self,
        connection_id: u64,
        response: ClientResponse,
        out: &mut Vec<TrackEventRequest>,
    ) {
        match response {
            ClientResponse::ThreadStart { response, .. } => {
                let thread = response.thread;
                self.ingest_thread_initialized(
                    ThreadInitializedInput {
                        connection_id,
                        thread_id: thread.id,
                        model: response.model,
                        ephemeral: thread.ephemeral,
                        session_source: thread.source.into(),
                        initialization_mode: InitializationMode::New,
                        created_at: u64::try_from(thread.created_at).unwrap_or_default(),
                    },
                    out,
                );
            }
            ClientResponse::ThreadResume { response, .. } => {
                let thread = response.thread;
                self.ingest_thread_initialized(
                    ThreadInitializedInput {
                        connection_id,
                        thread_id: thread.id,
                        model: response.model,
                        ephemeral: thread.ephemeral,
                        session_source: thread.source.into(),
                        initialization_mode: InitializationMode::Resumed,
                        created_at: u64::try_from(thread.created_at).unwrap_or_default(),
                    },
                    out,
                );
            }
            ClientResponse::ThreadFork { response, .. } => {
                let thread = response.thread;
                self.ingest_thread_initialized(
                    ThreadInitializedInput {
                        connection_id,
                        thread_id: thread.id,
                        model: response.model,
                        ephemeral: thread.ephemeral,
                        session_source: thread.source.into(),
                        initialization_mode: InitializationMode::Forked,
                        created_at: u64::try_from(thread.created_at).unwrap_or_default(),
                    },
                    out,
                );
            }
            ClientResponse::TurnStart {
                request_id,
                response,
            } => {
                let turn_id = response.turn.id;
                let Some(RequestState::TurnStart(pending_request)) =
                    self.requests.remove(&(connection_id, request_id))
                else {
                    return;
                };
                let turn_state = self.turns.entry(turn_id.clone()).or_insert(TurnState {
                    connection_id: None,
                    thread_id: None,
                    num_input_images: None,
                    resolved_config: None,
                    started_at_ms: None,
                    completed: None,
                });
                turn_state.connection_id = Some(connection_id);
                turn_state.thread_id = Some(pending_request.thread_id);
                turn_state.num_input_images = Some(pending_request.num_input_images);
                self.maybe_emit_turn_event(&turn_id, out);
            }
            _ => {}
        }
    }

    fn ingest_notification(
        &mut self,
        notification: ServerNotification,
        out: &mut Vec<TrackEventRequest>,
    ) {
        match notification {
            ServerNotification::TurnStarted(notification) => {
                let turn_state = self.turns.entry(notification.turn.id).or_insert(TurnState {
                    connection_id: None,
                    thread_id: None,
                    num_input_images: None,
                    resolved_config: None,
                    started_at_ms: None,
                    completed: None,
                });
                turn_state.started_at_ms = Some(now_unix_timestamp_millis());
            }
            ServerNotification::TurnCompleted(notification) => {
                let turn_state =
                    self.turns
                        .entry(notification.turn.id.clone())
                        .or_insert(TurnState {
                            connection_id: None,
                            thread_id: None,
                            num_input_images: None,
                            resolved_config: None,
                            started_at_ms: None,
                            completed: None,
                        });
                let completed_at_secs = now_unix_timestamp_secs();
                let completed_at_ms = completed_at_secs.saturating_mul(1000);
                turn_state.completed = Some(CompletedTurnState {
                    status: analytics_turn_status(notification.turn.status),
                    turn_error: notification.turn.error.and_then(|error| {
                        error
                            .codex_error_info
                            .map(app_server_codex_error_info_to_core)
                    }),
                    completed_at_secs,
                    duration_ms: turn_state
                        .started_at_ms
                        .map(|started_at_ms| completed_at_ms.saturating_sub(started_at_ms)),
                });
                let turn_id = notification.turn.id;
                self.maybe_emit_turn_event(&turn_id, out);
            }
            _ => {}
        }
    }

    fn maybe_emit_turn_event(&mut self, turn_id: &str, out: &mut Vec<TrackEventRequest>) {
        let Some(turn_state) = self.turns.get(turn_id) else {
            return;
        };
        let Some(thread_id) = turn_state.thread_id.clone() else {
            return;
        };
        let Some(num_input_images) = turn_state.num_input_images else {
            return;
        };
        let Some(resolved_config) = turn_state.resolved_config.clone() else {
            return;
        };
        let Some(completed) = turn_state.completed.clone() else {
            return;
        };
        out.push(TrackEventRequest::TurnEvent(Box::new(
            CodexTurnEventRequest {
                event_type: "codex_turn_event",
                event_params: codex_turn_event_params(
                    turn_state
                        .connection_id
                        .and_then(|connection_id| self.connections.get(&connection_id))
                        .map(|connection_state| connection_state.product_client_id.clone())
                        .unwrap_or_else(|| originator().value),
                    thread_id,
                    turn_id.to_string(),
                    num_input_images,
                    resolved_config,
                    completed,
                    turn_state.started_at_ms.map(|value| value / 1000),
                ),
            },
        )));
        self.turns.remove(turn_id);
    }

    async fn ingest_skill_invoked(
        &mut self,
        input: SkillInvokedInput,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let SkillInvokedInput {
            tracking,
            invocations,
        } = input;
        for invocation in invocations {
            let skill_scope = match invocation.skill_scope {
                SkillScope::User => "user",
                SkillScope::Repo => "repo",
                SkillScope::System => "system",
                SkillScope::Admin => "admin",
            };
            let repo_root = get_git_repo_root(invocation.skill_path.as_path());
            let repo_url = if let Some(root) = repo_root.as_ref() {
                collect_git_info(root)
                    .await
                    .and_then(|info| info.repository_url)
            } else {
                None
            };
            let skill_id = skill_id_for_local_skill(
                repo_url.as_deref(),
                repo_root.as_deref(),
                invocation.skill_path.as_path(),
                invocation.skill_name.as_str(),
            );
            out.push(TrackEventRequest::SkillInvocation(
                SkillInvocationEventRequest {
                    event_type: "skill_invocation",
                    skill_id,
                    skill_name: invocation.skill_name.clone(),
                    event_params: SkillInvocationEventParams {
                        thread_id: Some(tracking.thread_id.clone()),
                        invoke_type: Some(invocation.invocation_type),
                        model_slug: Some(tracking.model_slug.clone()),
                        product_client_id: Some(originator().value),
                        repo_url,
                        skill_scope: Some(skill_scope.to_string()),
                    },
                },
            ));
        }
    }
    fn ingest_app_mentioned(&mut self, input: AppMentionedInput, out: &mut Vec<TrackEventRequest>) {
        let AppMentionedInput { tracking, mentions } = input;
        out.extend(mentions.into_iter().map(|mention| {
            let event_params = codex_app_metadata(&tracking, mention);
            TrackEventRequest::AppMentioned(CodexAppMentionedEventRequest {
                event_type: "codex_app_mentioned",
                event_params,
            })
        }));
    }

    fn ingest_app_used(&mut self, input: AppUsedInput, out: &mut Vec<TrackEventRequest>) {
        let AppUsedInput { tracking, app } = input;
        let event_params = codex_app_metadata(&tracking, app);
        out.push(TrackEventRequest::AppUsed(CodexAppUsedEventRequest {
            event_type: "codex_app_used",
            event_params,
        }));
    }

    fn ingest_plugin_used(&mut self, input: PluginUsedInput, out: &mut Vec<TrackEventRequest>) {
        let PluginUsedInput { tracking, plugin } = input;
        out.push(TrackEventRequest::PluginUsed(CodexPluginUsedEventRequest {
            event_type: "codex_plugin_used",
            event_params: codex_plugin_used_metadata(&tracking, plugin),
        }));
    }

    fn ingest_plugin_state_changed(
        &mut self,
        input: PluginStateChangedInput,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let PluginStateChangedInput { plugin, state } = input;
        let event = CodexPluginEventRequest {
            event_type: plugin_state_event_type(state),
            event_params: codex_plugin_metadata(plugin),
        };
        out.push(match state {
            PluginState::Installed => TrackEventRequest::PluginInstalled(event),
            PluginState::Uninstalled => TrackEventRequest::PluginUninstalled(event),
            PluginState::Enabled => TrackEventRequest::PluginEnabled(event),
            PluginState::Disabled => TrackEventRequest::PluginDisabled(event),
        });
    }
}

fn plugin_state_event_type(state: PluginState) -> &'static str {
    match state {
        PluginState::Installed => "codex_plugin_installed",
        PluginState::Uninstalled => "codex_plugin_uninstalled",
        PluginState::Enabled => "codex_plugin_enabled",
        PluginState::Disabled => "codex_plugin_disabled",
    }
}

fn codex_app_metadata(tracking: &TrackEventsContext, app: AppInvocation) -> CodexAppMetadata {
    CodexAppMetadata {
        connector_id: app.connector_id,
        thread_id: Some(tracking.thread_id.clone()),
        turn_id: Some(tracking.turn_id.clone()),
        app_name: app.app_name,
        product_client_id: Some(originator().value),
        invoke_type: app.invocation_type,
        model_slug: Some(tracking.model_slug.clone()),
    }
}

fn codex_turn_event_params(
    product_client_id: String,
    thread_id: String,
    turn_id: String,
    num_input_images: usize,
    resolved_config: TurnResolvedConfigFact,
    completed: CompletedTurnState,
    started_at: Option<u64>,
) -> CodexTurnEventParams {
    let TurnResolvedConfigFact {
        turn_id: _resolved_turn_id,
        thread_id: _resolved_thread_id,
        num_input_images: _resolved_num_input_images,
        submission_type,
        model,
        model_provider,
        sandbox_policy,
        reasoning_effort,
        reasoning_summary,
        service_tier,
        approval_policy,
        approvals_reviewer,
        sandbox_network_access,
        collaboration_mode,
        personality,
        is_first_turn,
    } = resolved_config;
    CodexTurnEventParams {
        thread_id,
        turn_id,
        product_client_id: Some(product_client_id),
        submission_type,
        model: Some(model),
        model_provider,
        sandbox_policy: Some(sandbox_policy_mode(&sandbox_policy)),
        reasoning_effort: reasoning_effort.map(|value| value.to_string()),
        reasoning_summary: reasoning_summary_mode(reasoning_summary),
        service_tier: service_tier
            .map(|value| value.to_string())
            .unwrap_or_else(|| "default".to_string()),
        approval_policy: approval_policy.to_string(),
        approvals_reviewer: approvals_reviewer.to_string(),
        sandbox_network_access,
        collaboration_mode: Some(collaboration_mode_mode(collaboration_mode)),
        personality: personality_mode(personality),
        num_input_images,
        is_first_turn,
        status: completed.status,
        turn_error: completed.turn_error,
        steer_count: None,
        total_tool_call_count: None,
        shell_command_count: None,
        file_change_count: None,
        mcp_tool_call_count: None,
        dynamic_tool_call_count: None,
        subagent_tool_call_count: None,
        web_search_count: None,
        image_generation_count: None,
        input_tokens: None,
        cached_input_tokens: None,
        output_tokens: None,
        reasoning_output_tokens: None,
        total_tokens: None,
        duration_ms: completed.duration_ms,
        started_at,
        completed_at: Some(completed.completed_at_secs),
    }
}

fn sandbox_policy_mode(sandbox_policy: &SandboxPolicy) -> &'static str {
    match sandbox_policy {
        SandboxPolicy::DangerFullAccess => "full_access",
        SandboxPolicy::ReadOnly { .. } => "read_only",
        SandboxPolicy::WorkspaceWrite { .. } => "workspace_write",
        SandboxPolicy::ExternalSandbox { .. } => "external_sandbox",
    }
}

fn collaboration_mode_mode(mode: ModeKind) -> &'static str {
    match mode {
        ModeKind::Plan => "plan",
        ModeKind::Default | ModeKind::PairProgramming | ModeKind::Execute => "default",
    }
}

fn reasoning_summary_mode(summary: Option<ReasoningSummary>) -> Option<String> {
    match summary {
        Some(ReasoningSummary::None) | None => None,
        Some(summary) => Some(summary.to_string()),
    }
}

fn personality_mode(personality: Option<Personality>) -> Option<String> {
    match personality {
        Some(Personality::None) | None => None,
        Some(personality) => Some(personality.to_string()),
    }
}

fn thread_initialized_event_request(
    connection_state: &ConnectionState,
    input: ThreadInitializedInput,
) -> ThreadInitializedEvent {
    ThreadInitializedEvent {
        event_type: "codex_thread_initialized",
        event_params: thread_initialized_event_params(connection_state, input),
    }
}

fn thread_initialized_event_params(
    connection_state: &ConnectionState,
    input: ThreadInitializedInput,
) -> ThreadInitializedEventParams {
    ThreadInitializedEventParams {
        thread_id: input.thread_id,
        product_client_id: connection_state.product_client_id.clone(),
        client_name: connection_state.client_name.clone(),
        client_version: connection_state.client_version.clone(),
        experimental_api_enabled: connection_state.experimental_api_enabled,
        model: input.model,
        ephemeral: input.ephemeral,
        session_source: session_source_name(&input.session_source),
        initialization_mode: input.initialization_mode,
        subagent_source: None,
        parent_thread_id: None,
        created_at: input.created_at,
    }
}

fn codex_plugin_metadata(plugin: PluginTelemetryMetadata) -> CodexPluginMetadata {
    let capability_summary = plugin.capability_summary;
    CodexPluginMetadata {
        plugin_id: Some(plugin.plugin_id.as_key()),
        plugin_name: Some(plugin.plugin_id.plugin_name),
        marketplace_name: Some(plugin.plugin_id.marketplace_name),
        has_skills: capability_summary
            .as_ref()
            .map(|summary| summary.has_skills),
        mcp_server_count: capability_summary
            .as_ref()
            .map(|summary| summary.mcp_server_names.len()),
        connector_ids: capability_summary.map(|summary| {
            summary
                .app_connector_ids
                .into_iter()
                .map(|connector_id| connector_id.0)
                .collect()
        }),
        product_client_id: Some(originator().value),
    }
}

fn codex_plugin_used_metadata(
    tracking: &TrackEventsContext,
    plugin: PluginTelemetryMetadata,
) -> CodexPluginUsedMetadata {
    CodexPluginUsedMetadata {
        plugin: codex_plugin_metadata(plugin),
        thread_id: Some(tracking.thread_id.clone()),
        turn_id: Some(tracking.turn_id.clone()),
        model_slug: Some(tracking.model_slug.clone()),
    }
}

fn session_source_name(session_source: &SessionSource) -> Option<&'static str> {
    match session_source {
        SessionSource::Cli | SessionSource::VSCode | SessionSource::Exec => Some("user"),
        SessionSource::SubAgent(_)
        | SessionSource::Mcp
        | SessionSource::Custom(_)
        | SessionSource::Unknown => None,
    }
}

fn now_unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_unix_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn analytics_turn_status(status: codex_app_server_protocol::TurnStatus) -> Option<TurnStatus> {
    match status {
        codex_app_server_protocol::TurnStatus::Completed => Some(TurnStatus::Completed),
        codex_app_server_protocol::TurnStatus::Failed => Some(TurnStatus::Failed),
        codex_app_server_protocol::TurnStatus::Interrupted => Some(TurnStatus::Interrupted),
        codex_app_server_protocol::TurnStatus::InProgress => None,
    }
}

fn app_server_codex_error_info_to_core(
    info: codex_app_server_protocol::CodexErrorInfo,
) -> CodexErrorInfo {
    match info {
        codex_app_server_protocol::CodexErrorInfo::ContextWindowExceeded => {
            CodexErrorInfo::ContextWindowExceeded
        }
        codex_app_server_protocol::CodexErrorInfo::UsageLimitExceeded => {
            CodexErrorInfo::UsageLimitExceeded
        }
        codex_app_server_protocol::CodexErrorInfo::ServerOverloaded => {
            CodexErrorInfo::ServerOverloaded
        }
        codex_app_server_protocol::CodexErrorInfo::HttpConnectionFailed { http_status_code } => {
            CodexErrorInfo::HttpConnectionFailed { http_status_code }
        }
        codex_app_server_protocol::CodexErrorInfo::ResponseStreamConnectionFailed {
            http_status_code,
        } => CodexErrorInfo::ResponseStreamConnectionFailed { http_status_code },
        codex_app_server_protocol::CodexErrorInfo::InternalServerError => {
            CodexErrorInfo::InternalServerError
        }
        codex_app_server_protocol::CodexErrorInfo::Unauthorized => CodexErrorInfo::Unauthorized,
        codex_app_server_protocol::CodexErrorInfo::BadRequest => CodexErrorInfo::BadRequest,
        codex_app_server_protocol::CodexErrorInfo::ThreadRollbackFailed => {
            CodexErrorInfo::ThreadRollbackFailed
        }
        codex_app_server_protocol::CodexErrorInfo::SandboxError => CodexErrorInfo::SandboxError,
        codex_app_server_protocol::CodexErrorInfo::ResponseStreamDisconnected {
            http_status_code,
        } => CodexErrorInfo::ResponseStreamDisconnected { http_status_code },
        codex_app_server_protocol::CodexErrorInfo::ResponseTooManyFailedAttempts {
            http_status_code,
        } => CodexErrorInfo::ResponseTooManyFailedAttempts { http_status_code },
        codex_app_server_protocol::CodexErrorInfo::ActiveTurnNotSteerable { turn_kind } => {
            CodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: match turn_kind {
                    codex_app_server_protocol::NonSteerableTurnKind::Review => {
                        codex_protocol::protocol::NonSteerableTurnKind::Review
                    }
                    codex_app_server_protocol::NonSteerableTurnKind::Compact => {
                        codex_protocol::protocol::NonSteerableTurnKind::Compact
                    }
                },
            }
        }
        codex_app_server_protocol::CodexErrorInfo::Other => CodexErrorInfo::Other,
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
    if !auth.is_chatgpt_auth() {
        return;
    }
    let access_token = match auth.get_token() {
        Ok(token) => token,
        Err(_) => return,
    };
    let Some(account_id) = auth.get_account_id() else {
        return;
    };

    let base_url = base_url.trim_end_matches('/');
    let url = format!("{base_url}/codex/analytics-events/events");
    let payload = TrackEventsRequest { events };

    let response = create_client()
        .post(&url)
        .timeout(ANALYTICS_EVENTS_TIMEOUT)
        .bearer_auth(&access_token)
        .header("chatgpt-account-id", &account_id)
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

pub(crate) fn skill_id_for_local_skill(
    repo_url: Option<&str>,
    repo_root: Option<&Path>,
    skill_path: &Path,
    skill_name: &str,
) -> String {
    let path = normalize_path_for_skill_id(repo_url, repo_root, skill_path);
    let prefix = if let Some(url) = repo_url {
        format!("repo_{url}")
    } else {
        "personal".to_string()
    };
    let raw_id = format!("{prefix}_{path}_{skill_name}");
    let mut hasher = Sha1::new();
    hasher.update(raw_id.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Returns a normalized path for skill ID construction.
///
/// - Repo-scoped skills use a path relative to the repo root.
/// - User/admin/system skills use an absolute path.
fn normalize_path_for_skill_id(
    repo_url: Option<&str>,
    repo_root: Option<&Path>,
    skill_path: &Path,
) -> String {
    let resolved_path =
        std::fs::canonicalize(skill_path).unwrap_or_else(|_| skill_path.to_path_buf());
    match (repo_url, repo_root) {
        (Some(_), Some(root)) => {
            let resolved_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
            resolved_path
                .strip_prefix(&resolved_root)
                .unwrap_or(resolved_path.as_path())
                .to_string_lossy()
                .replace('\\', "/")
        }
        _ => resolved_path.to_string_lossy().replace('\\', "/"),
    }
}

#[cfg(test)]
#[path = "analytics_client_tests.rs"]
mod tests;
