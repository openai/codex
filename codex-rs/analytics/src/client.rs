use crate::events::AppServerRpcTransport;
use crate::events::TrackEventRequest;
use crate::events::TrackEventsRequest;
use crate::events::current_runtime_metadata;
use crate::facts::AnalyticsFact;
use crate::facts::AppInvocation;
use crate::facts::AppMentionedInput;
use crate::facts::AppUsedInput;
use crate::facts::CustomAnalyticsFact;
use crate::facts::PluginState;
use crate::facts::PluginStateChangedInput;
use crate::facts::SkillInvocation;
use crate::facts::SkillInvokedInput;
use crate::facts::SubAgentThreadStartedInput;
use crate::facts::TrackEventsContext;
use crate::reducer::AnalyticsReducer;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
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

    pub fn new_disabled() -> Self {
        let (sender, _receiver) = mpsc::channel(1);
        Self {
            queue: AnalyticsEventsQueue {
                sender,
                app_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
                plugin_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
            },
            analytics_enabled: Some(false),
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
            crate::facts::PluginUsedInput { tracking, plugin },
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
        self.record_fact(AnalyticsFact::ClientResponse {
            connection_id,
            response: Box::new(response),
        });
    }

    pub fn track_server_request(&self, connection_id: u64, request: ServerRequest) {
        self.record_fact(AnalyticsFact::ServerRequest {
            connection_id,
            request: Box::new(request),
        });
    }

    pub fn track_server_response(&self, response: ServerResponse) {
        self.record_fact(AnalyticsFact::ServerResponse {
            response: Box::new(response),
        });
    }

    pub fn track_notification(&self, connection_id: u64, notification: ServerNotification) {
        self.record_fact(AnalyticsFact::Notification {
            connection_id,
            notification: Box::new(notification),
        });
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
