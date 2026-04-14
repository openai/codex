use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::protocol::SkillScope;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct TrackEventsContext {
    pub model_slug: String,
    pub thread_id: String,
    pub turn_id: String,
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
        params: codex_app_server_protocol::InitializeParams,
    },
    Request {
        connection_id: u64,
        request_id: codex_app_server_protocol::RequestId,
        request: Box<codex_app_server_protocol::ClientRequest>,
    },
    Response {
        connection_id: u64,
        response: Box<codex_app_server_protocol::ClientResponse>,
    },
    Notification(Box<codex_app_server_protocol::ServerNotification>),
    Custom(CustomAnalyticsFact),
}

pub enum CustomAnalyticsFact {
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
pub struct AnalyticsReducer;

#[derive(Clone, Default)]
pub struct AnalyticsEventsClient {
    analytics_enabled: Option<bool>,
    _marker: Arc<()>,
}

impl AnalyticsEventsClient {
    pub fn new<T>(
        _auth_manager: Arc<T>,
        _base_url: String,
        analytics_enabled: Option<bool>,
    ) -> Self {
        Self {
            analytics_enabled,
            _marker: Arc::new(()),
        }
    }

    pub fn track_skill_invocations(
        &self,
        _tracking: TrackEventsContext,
        _invocations: Vec<SkillInvocation>,
    ) {
        let _ = self.analytics_enabled;
    }

    pub fn track_app_mentioned(
        &self,
        _tracking: TrackEventsContext,
        _mentions: Vec<AppInvocation>,
    ) {
        let _ = self.analytics_enabled;
    }

    pub fn track_app_used(&self, _tracking: TrackEventsContext, _app: AppInvocation) {
        let _ = self.analytics_enabled;
    }

    pub fn track_plugin_used(
        &self,
        _tracking: TrackEventsContext,
        _plugin: PluginTelemetryMetadata,
    ) {
        let _ = self.analytics_enabled;
    }

    pub fn track_plugin_installed(&self, _plugin: PluginTelemetryMetadata) {
        let _ = self.analytics_enabled;
    }

    pub fn track_plugin_uninstalled(&self, _plugin: PluginTelemetryMetadata) {
        let _ = self.analytics_enabled;
    }

    pub fn track_plugin_enabled(&self, _plugin: PluginTelemetryMetadata) {
        let _ = self.analytics_enabled;
    }

    pub fn track_plugin_disabled(&self, _plugin: PluginTelemetryMetadata) {
        let _ = self.analytics_enabled;
    }
}
