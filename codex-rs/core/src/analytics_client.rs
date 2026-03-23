use crate::AuthManager;
use crate::config::Config;
use crate::default_client::create_client;
use crate::git_info::collect_git_info;
use crate::git_info::get_git_repo_root;
use crate::plugins::PluginTelemetryMetadata;
use codex_protocol::protocol::SkillScope;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Clone)]
pub(crate) struct TrackEventsContext {
    pub(crate) model_slug: String,
    pub(crate) thread_id: String,
    pub(crate) turn_id: String,
}

pub(crate) fn build_track_events_context(
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
pub(crate) struct SkillInvocation {
    pub(crate) skill_name: String,
    pub(crate) skill_scope: SkillScope,
    pub(crate) skill_path: PathBuf,
    pub(crate) invocation_type: InvocationType,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum InvocationType {
    Explicit,
    Implicit,
}

pub(crate) struct AppInvocation {
    pub(crate) connector_id: Option<String>,
    pub(crate) app_name: Option<String>,
    pub(crate) invocation_type: Option<InvocationType>,
}

#[derive(Clone)]
pub(crate) struct AnalyticsEventsQueue {
    sender: mpsc::Sender<TrackEventsJob>,
    app_used_emitted_keys: Arc<Mutex<HashSet<(String, String)>>>,
    plugin_used_emitted_keys: Arc<Mutex<HashSet<(String, String)>>>,
}

#[derive(Clone)]
pub struct AnalyticsEventsClient {
    queue: AnalyticsEventsQueue,
    config: Arc<Config>,
}

impl AnalyticsEventsQueue {
    pub(crate) fn new(auth_manager: Arc<AuthManager>) -> Self {
        let (sender, mut receiver) = mpsc::channel(ANALYTICS_EVENTS_QUEUE_SIZE);
        tokio::spawn(async move {
            while let Some(job) = receiver.recv().await {
                match job {
                    TrackEventsJob::SkillInvocations(job) => {
                        send_track_skill_invocations(&auth_manager, job).await;
                    }
                    TrackEventsJob::AppMentioned(job) => {
                        send_track_app_mentioned(&auth_manager, job).await;
                    }
                    TrackEventsJob::AppUsed(job) => {
                        send_track_app_used(&auth_manager, job).await;
                    }
                    TrackEventsJob::PluginUsed(job) => {
                        send_track_plugin_used(&auth_manager, job).await;
                    }
                    TrackEventsJob::PluginInstalled(job) => {
                        send_track_plugin_installed(&auth_manager, job).await;
                    }
                    TrackEventsJob::PluginUninstalled(job) => {
                        send_track_plugin_uninstalled(&auth_manager, job).await;
                    }
                    TrackEventsJob::PluginEnabled(job) => {
                        send_track_plugin_enabled(&auth_manager, job).await;
                    }
                    TrackEventsJob::PluginDisabled(job) => {
                        send_track_plugin_disabled(&auth_manager, job).await;
                    }
                }
            }
        });
        Self {
            sender,
            app_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
            plugin_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    fn try_send(&self, job: TrackEventsJob) {
        if self.sender.try_send(job).is_err() {
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
    pub fn new(config: Arc<Config>, auth_manager: Arc<AuthManager>) -> Self {
        Self {
            queue: AnalyticsEventsQueue::new(Arc::clone(&auth_manager)),
            config,
        }
    }

    pub(crate) fn track_skill_invocations(
        &self,
        tracking: TrackEventsContext,
        invocations: Vec<SkillInvocation>,
    ) {
        track_skill_invocations(
            &self.queue,
            Arc::clone(&self.config),
            Some(tracking),
            invocations,
        );
    }

    pub(crate) fn track_app_mentioned(
        &self,
        tracking: TrackEventsContext,
        mentions: Vec<AppInvocation>,
    ) {
        track_app_mentioned(
            &self.queue,
            Arc::clone(&self.config),
            Some(tracking),
            mentions,
        );
    }

    pub(crate) fn track_app_used(&self, tracking: TrackEventsContext, app: AppInvocation) {
        track_app_used(&self.queue, Arc::clone(&self.config), Some(tracking), app);
    }

    pub(crate) fn track_plugin_used(
        &self,
        tracking: TrackEventsContext,
        plugin: PluginTelemetryMetadata,
    ) {
        track_plugin_used(
            &self.queue,
            Arc::clone(&self.config),
            Some(tracking),
            plugin,
        );
    }

    pub fn track_plugin_installed(&self, plugin: PluginTelemetryMetadata) {
        track_plugin_management(
            &self.queue,
            Arc::clone(&self.config),
            PluginManagementEventType::Installed,
            plugin,
        );
    }

    pub fn track_plugin_uninstalled(&self, plugin: PluginTelemetryMetadata) {
        track_plugin_management(
            &self.queue,
            Arc::clone(&self.config),
            PluginManagementEventType::Uninstalled,
            plugin,
        );
    }

    pub fn track_plugin_enabled(&self, plugin: PluginTelemetryMetadata) {
        track_plugin_management(
            &self.queue,
            Arc::clone(&self.config),
            PluginManagementEventType::Enabled,
            plugin,
        );
    }

    pub fn track_plugin_disabled(&self, plugin: PluginTelemetryMetadata) {
        track_plugin_management(
            &self.queue,
            Arc::clone(&self.config),
            PluginManagementEventType::Disabled,
            plugin,
        );
    }
}

enum TrackEventsJob {
    SkillInvocations(TrackSkillInvocationsJob),
    AppMentioned(TrackAppMentionedJob),
    AppUsed(TrackAppUsedJob),
    PluginUsed(TrackPluginUsedJob),
    PluginInstalled(TrackPluginManagementJob),
    PluginUninstalled(TrackPluginManagementJob),
    PluginEnabled(TrackPluginManagementJob),
    PluginDisabled(TrackPluginManagementJob),
}

struct TrackSkillInvocationsJob {
    config: Arc<Config>,
    tracking: TrackEventsContext,
    invocations: Vec<SkillInvocation>,
}

struct TrackAppMentionedJob {
    config: Arc<Config>,
    tracking: TrackEventsContext,
    mentions: Vec<AppInvocation>,
}

struct TrackAppUsedJob {
    config: Arc<Config>,
    tracking: TrackEventsContext,
    app: AppInvocation,
}

struct TrackPluginUsedJob {
    config: Arc<Config>,
    tracking: TrackEventsContext,
    plugin: PluginTelemetryMetadata,
}

struct TrackPluginManagementJob {
    config: Arc<Config>,
    plugin: PluginTelemetryMetadata,
}

#[derive(Clone, Copy)]
enum PluginManagementEventType {
    Installed,
    Uninstalled,
    Enabled,
    Disabled,
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
    AppMentioned(CodexAppMentionedEventRequest),
    AppUsed(CodexAppUsedEventRequest),
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

pub(crate) fn track_skill_invocations(
    queue: &AnalyticsEventsQueue,
    config: Arc<Config>,
    tracking: Option<TrackEventsContext>,
    invocations: Vec<SkillInvocation>,
) {
    if config.analytics_enabled == Some(false) {
        return;
    }
    let Some(tracking) = tracking else {
        return;
    };
    if invocations.is_empty() {
        return;
    }
    let job = TrackEventsJob::SkillInvocations(TrackSkillInvocationsJob {
        config,
        tracking,
        invocations,
    });
    queue.try_send(job);
}

pub(crate) fn track_app_mentioned(
    queue: &AnalyticsEventsQueue,
    config: Arc<Config>,
    tracking: Option<TrackEventsContext>,
    mentions: Vec<AppInvocation>,
) {
    if config.analytics_enabled == Some(false) {
        return;
    }
    let Some(tracking) = tracking else {
        return;
    };
    if mentions.is_empty() {
        return;
    }
    let job = TrackEventsJob::AppMentioned(TrackAppMentionedJob {
        config,
        tracking,
        mentions,
    });
    queue.try_send(job);
}

pub(crate) fn track_app_used(
    queue: &AnalyticsEventsQueue,
    config: Arc<Config>,
    tracking: Option<TrackEventsContext>,
    app: AppInvocation,
) {
    if config.analytics_enabled == Some(false) {
        return;
    }
    let Some(tracking) = tracking else {
        return;
    };
    if !queue.should_enqueue_app_used(&tracking, &app) {
        return;
    }
    let job = TrackEventsJob::AppUsed(TrackAppUsedJob {
        config,
        tracking,
        app,
    });
    queue.try_send(job);
}

pub(crate) fn track_plugin_used(
    queue: &AnalyticsEventsQueue,
    config: Arc<Config>,
    tracking: Option<TrackEventsContext>,
    plugin: PluginTelemetryMetadata,
) {
    if config.analytics_enabled == Some(false) {
        return;
    }
    let Some(tracking) = tracking else {
        return;
    };
    if !queue.should_enqueue_plugin_used(&tracking, &plugin) {
        return;
    }
    let job = TrackEventsJob::PluginUsed(TrackPluginUsedJob {
        config,
        tracking,
        plugin,
    });
    queue.try_send(job);
}

fn track_plugin_management(
    queue: &AnalyticsEventsQueue,
    config: Arc<Config>,
    event_type: PluginManagementEventType,
    plugin: PluginTelemetryMetadata,
) {
    if config.analytics_enabled == Some(false) {
        return;
    }
    let job = TrackPluginManagementJob { config, plugin };
    let job = match event_type {
        PluginManagementEventType::Installed => TrackEventsJob::PluginInstalled(job),
        PluginManagementEventType::Uninstalled => TrackEventsJob::PluginUninstalled(job),
        PluginManagementEventType::Enabled => TrackEventsJob::PluginEnabled(job),
        PluginManagementEventType::Disabled => TrackEventsJob::PluginDisabled(job),
    };
    queue.try_send(job);
}

async fn send_track_skill_invocations(auth_manager: &AuthManager, job: TrackSkillInvocationsJob) {
    let TrackSkillInvocationsJob {
        config,
        tracking,
        invocations,
    } = job;
    let mut events = Vec::with_capacity(invocations.len());
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
        events.push(TrackEventRequest::SkillInvocation(
            SkillInvocationEventRequest {
                event_type: "skill_invocation",
                skill_id,
                skill_name: invocation.skill_name.clone(),
                event_params: SkillInvocationEventParams {
                    thread_id: Some(tracking.thread_id.clone()),
                    invoke_type: Some(invocation.invocation_type),
                    model_slug: Some(tracking.model_slug.clone()),
                    product_client_id: Some(crate::default_client::originator().value),
                    repo_url,
                    skill_scope: Some(skill_scope.to_string()),
                },
            },
        ));
    }

    send_track_events(auth_manager, config, events).await;
}

async fn send_track_app_mentioned(auth_manager: &AuthManager, job: TrackAppMentionedJob) {
    let TrackAppMentionedJob {
        config,
        tracking,
        mentions,
    } = job;
    let events = mentions
        .into_iter()
        .map(|mention| {
            let event_params = codex_app_metadata(&tracking, mention);
            TrackEventRequest::AppMentioned(CodexAppMentionedEventRequest {
                event_type: "codex_app_mentioned",
                event_params,
            })
        })
        .collect::<Vec<_>>();

    send_track_events(auth_manager, config, events).await;
}

async fn send_track_app_used(auth_manager: &AuthManager, job: TrackAppUsedJob) {
    let TrackAppUsedJob {
        config,
        tracking,
        app,
    } = job;
    let event_params = codex_app_metadata(&tracking, app);
    let events = vec![TrackEventRequest::AppUsed(CodexAppUsedEventRequest {
        event_type: "codex_app_used",
        event_params,
    })];

    send_track_events(auth_manager, config, events).await;
}

async fn send_track_plugin_used(auth_manager: &AuthManager, job: TrackPluginUsedJob) {
    let TrackPluginUsedJob {
        config,
        tracking,
        plugin,
    } = job;
    let events = vec![TrackEventRequest::PluginUsed(CodexPluginUsedEventRequest {
        event_type: "codex_plugin_used",
        event_params: codex_plugin_used_metadata(&tracking, plugin),
    })];

    send_track_events(auth_manager, config, events).await;
}

async fn send_track_plugin_installed(auth_manager: &AuthManager, job: TrackPluginManagementJob) {
    send_track_plugin_management_event(auth_manager, job, "codex_plugin_installed").await;
}

async fn send_track_plugin_uninstalled(auth_manager: &AuthManager, job: TrackPluginManagementJob) {
    send_track_plugin_management_event(auth_manager, job, "codex_plugin_uninstalled").await;
}

async fn send_track_plugin_enabled(auth_manager: &AuthManager, job: TrackPluginManagementJob) {
    send_track_plugin_management_event(auth_manager, job, "codex_plugin_enabled").await;
}

async fn send_track_plugin_disabled(auth_manager: &AuthManager, job: TrackPluginManagementJob) {
    send_track_plugin_management_event(auth_manager, job, "codex_plugin_disabled").await;
}

async fn send_track_plugin_management_event(
    auth_manager: &AuthManager,
    job: TrackPluginManagementJob,
    event_type: &'static str,
) {
    let TrackPluginManagementJob { config, plugin } = job;
    let event_params = codex_plugin_metadata(plugin);
    let event = CodexPluginEventRequest {
        event_type,
        event_params,
    };
    let events = vec![match event_type {
        "codex_plugin_installed" => TrackEventRequest::PluginInstalled(event),
        "codex_plugin_uninstalled" => TrackEventRequest::PluginUninstalled(event),
        "codex_plugin_enabled" => TrackEventRequest::PluginEnabled(event),
        "codex_plugin_disabled" => TrackEventRequest::PluginDisabled(event),
        _ => unreachable!("unknown plugin management event type"),
    }];

    send_track_events(auth_manager, config, events).await;
}

fn codex_app_metadata(tracking: &TrackEventsContext, app: AppInvocation) -> CodexAppMetadata {
    CodexAppMetadata {
        connector_id: app.connector_id,
        thread_id: Some(tracking.thread_id.clone()),
        turn_id: Some(tracking.turn_id.clone()),
        app_name: app.app_name,
        product_client_id: Some(crate::default_client::originator().value),
        invoke_type: app.invocation_type,
        model_slug: Some(tracking.model_slug.clone()),
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
        product_client_id: Some(crate::default_client::originator().value),
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

async fn send_track_events(
    auth_manager: &AuthManager,
    config: Arc<Config>,
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

    let base_url = config.chatgpt_base_url.trim_end_matches('/');
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
