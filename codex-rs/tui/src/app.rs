use crate::app_backtrack::BacktrackState;
use crate::app_event::AppEvent;
use crate::app_event::AppServerAction;
use crate::app_event::ExitMode;
#[cfg(target_os = "windows")]
use crate::app_event::WindowsSandboxEnableMode;
#[cfg(target_os = "windows")]
use crate::app_event::WindowsSandboxFallbackReason;
use crate::app_event_sender::AppEventSender;
use crate::app_server_client::AppServerClient;
use crate::bottom_pane::ApprovalRequest;
use crate::bottom_pane::FeedbackAudience;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::chatwidget::ChatWidget;
use crate::chatwidget::ExternalEditorState;
use crate::cwd_prompt::CwdPromptAction;
use crate::diff_render::DiffSummary;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::external_editor;
use crate::file_search::FileSearchManager;
use crate::history_cell;
use crate::history_cell::HistoryCell;
#[cfg(not(debug_assertions))]
use crate::history_cell::UpdateAvailableHistoryCell;
use crate::model_migration::ModelMigrationOutcome;
use crate::model_migration::migration_copy_for_models;
use crate::model_migration::run_model_migration_prompt;
use crate::pager_overlay::Overlay;
use crate::render::highlight::highlight_bash_to_lines;
use crate::render::renderable::Renderable;
use crate::resume_picker::SessionSelection;
use crate::tui;
use crate::tui::TuiEvent;
use crate::update_action::UpdateAction;
use codex_ansi_escape::ansi_escape_line;
use codex_app_server_protocol::ConfigLayerSource;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ThreadItem as V2ThreadItem;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_core::config::edit::ConfigEdit;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_core::config_loader::ConfigLayerStackOrdering;
use codex_core::config_loader::LoaderOverrides;
#[cfg(target_os = "windows")]
use codex_core::features::Feature;
use codex_core::models_manager::manager::RefreshStrategy;
use codex_core::models_manager::model_presets::HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG;
use codex_core::models_manager::model_presets::HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::DeprecationNoticeEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::ExecCommandOutputDeltaEvent;
use codex_core::protocol::ExecCommandSource;
use codex_core::protocol::ExecOutputStream;
use codex_core::protocol::FinalOutput;
use codex_core::protocol::ListSkillsResponseEvent;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::SessionSource;
use codex_core::protocol::SkillErrorInfo;
use codex_core::protocol::TokenUsage;
#[cfg(target_os = "windows")]
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use codex_protocol::config_types::Personality;
#[cfg(target_os = "windows")]
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::ContextCompactionItem;
use codex_protocol::items::ReasoningItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelUpgrade;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_utils_absolute_path::AbsolutePathBuf;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tokio::select;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::unbounded_channel;
use toml::Value as TomlValue;

const EXTERNAL_EDITOR_HINT: &str = "Save and close external editor to continue.";
const THREAD_EVENT_CHANNEL_CAPACITY: usize = 32768;
const SHUTDOWN_FALLBACK_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct AppExitInfo {
    pub token_usage: TokenUsage,
    pub thread_id: Option<ThreadId>,
    pub update_action: Option<UpdateAction>,
    pub exit_reason: ExitReason,
}

impl AppExitInfo {
    pub fn fatal(message: impl Into<String>) -> Self {
        Self {
            token_usage: TokenUsage::default(),
            thread_id: None,
            update_action: None,
            exit_reason: ExitReason::Fatal(message.into()),
        }
    }
}

#[derive(Debug)]
pub(crate) enum AppRunControl {
    Continue,
    Exit(ExitReason),
}

#[derive(Debug, Clone)]
pub enum ExitReason {
    UserRequested,
    Fatal(String),
}

fn session_summary(token_usage: TokenUsage, thread_id: Option<ThreadId>) -> Option<SessionSummary> {
    if token_usage.is_zero() {
        return None;
    }

    let usage_line = FinalOutput::from(token_usage).to_string();
    let resume_command = thread_id.map(|thread_id| format!("codex resume {thread_id}"));
    Some(SessionSummary {
        usage_line,
        resume_command,
    })
}

fn errors_for_cwd(cwd: &Path, response: &ListSkillsResponseEvent) -> Vec<SkillErrorInfo> {
    response
        .skills
        .iter()
        .find(|entry| entry.cwd.as_path() == cwd)
        .map(|entry| entry.errors.clone())
        .unwrap_or_default()
}

fn emit_skill_load_warnings(app_event_tx: &AppEventSender, errors: &[SkillErrorInfo]) {
    if errors.is_empty() {
        return;
    }

    let error_count = errors.len();
    app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
        crate::history_cell::new_warning_event(format!(
            "Skipped loading {error_count} skill(s) due to invalid SKILL.md files."
        )),
    )));

    for error in errors {
        let path = error.path.display();
        let message = error.message.as_str();
        app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
            crate::history_cell::new_warning_event(format!("{path}: {message}")),
        )));
    }
}

fn emit_deprecation_notice(app_event_tx: &AppEventSender, notice: Option<DeprecationNoticeEvent>) {
    let Some(DeprecationNoticeEvent { summary, details }) = notice else {
        return;
    };
    app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
        crate::history_cell::new_deprecation_notice(summary, details),
    )));
}

fn emit_project_config_warnings(app_event_tx: &AppEventSender, config: &Config) {
    let mut disabled_folders = Vec::new();

    for layer in config
        .config_layer_stack
        .get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, true)
    {
        let ConfigLayerSource::Project { dot_codex_folder } = &layer.name else {
            continue;
        };
        if layer.disabled_reason.is_none() {
            continue;
        }
        disabled_folders.push((
            dot_codex_folder.as_path().display().to_string(),
            layer
                .disabled_reason
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "config.toml is disabled.".to_string()),
        ));
    }

    if disabled_folders.is_empty() {
        return;
    }

    let mut message = concat!(
        "Project config.toml files are disabled in the following folders. ",
        "Settings in those files are ignored, but skills and exec policies still load.\n",
    )
    .to_string();
    for (index, (folder, reason)) in disabled_folders.iter().enumerate() {
        let display_index = index + 1;
        message.push_str(&format!("    {display_index}. {folder}\n"));
        message.push_str(&format!("       {reason}\n"));
    }

    app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
        history_cell::new_warning_event(message),
    )));
}

fn app_server_error(err: JSONRPCErrorError) -> color_eyre::Report {
    let details = err
        .data
        .as_ref()
        .map(|data| format!(" (data: {data})"))
        .unwrap_or_default();
    color_eyre::eyre::eyre!("app server error {}: {}{details}", err.code, err.message)
}

fn map_skills_list_entry(
    entry: codex_app_server_protocol::SkillsListEntry,
) -> codex_core::protocol::SkillsListEntry {
    codex_core::protocol::SkillsListEntry {
        cwd: entry.cwd,
        skills: entry.skills.into_iter().map(map_skill_metadata).collect(),
        errors: entry.errors.into_iter().map(map_skill_error).collect(),
    }
}

fn map_skill_metadata(
    value: codex_app_server_protocol::SkillMetadata,
) -> codex_core::protocol::SkillMetadata {
    codex_core::protocol::SkillMetadata {
        name: value.name,
        description: value.description,
        short_description: value.short_description,
        interface: value.interface.map(map_skill_interface),
        dependencies: value.dependencies.map(map_skill_dependencies),
        path: value.path,
        scope: map_skill_scope(value.scope),
        enabled: value.enabled,
    }
}

fn map_skill_interface(
    value: codex_app_server_protocol::SkillInterface,
) -> codex_core::protocol::SkillInterface {
    codex_core::protocol::SkillInterface {
        display_name: value.display_name,
        short_description: value.short_description,
        icon_small: value.icon_small,
        icon_large: value.icon_large,
        brand_color: value.brand_color,
        default_prompt: value.default_prompt,
    }
}

fn map_skill_dependencies(
    value: codex_app_server_protocol::SkillDependencies,
) -> codex_core::protocol::SkillDependencies {
    codex_core::protocol::SkillDependencies {
        tools: value
            .tools
            .into_iter()
            .map(map_skill_tool_dependency)
            .collect(),
    }
}

fn map_skill_tool_dependency(
    value: codex_app_server_protocol::SkillToolDependency,
) -> codex_core::protocol::SkillToolDependency {
    codex_core::protocol::SkillToolDependency {
        r#type: value.r#type,
        value: value.value,
        description: value.description,
        transport: value.transport,
        command: value.command,
        url: value.url,
    }
}

fn map_skill_error(
    value: codex_app_server_protocol::SkillErrorInfo,
) -> codex_core::protocol::SkillErrorInfo {
    codex_core::protocol::SkillErrorInfo {
        path: value.path,
        message: value.message,
    }
}

fn map_skill_scope(
    scope: codex_app_server_protocol::SkillScope,
) -> codex_core::protocol::SkillScope {
    match scope {
        codex_app_server_protocol::SkillScope::User => codex_core::protocol::SkillScope::User,
        codex_app_server_protocol::SkillScope::Repo => codex_core::protocol::SkillScope::Repo,
        codex_app_server_protocol::SkillScope::System => codex_core::protocol::SkillScope::System,
        codex_app_server_protocol::SkillScope::Admin => codex_core::protocol::SkillScope::Admin,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionSummary {
    usage_line: String,
    resume_command: Option<String>,
}

#[derive(Debug, Clone)]
struct ThreadEventSnapshot {
    session_configured: Option<Event>,
    events: Vec<Event>,
}

#[derive(Debug, Clone)]
struct ThreadBootstrap {
    thread_id: ThreadId,
    session_event: Event,
}

#[derive(Debug)]
struct ThreadEventStore {
    session_configured: Option<Event>,
    buffer: VecDeque<Event>,
    user_message_ids: HashSet<String>,
    capacity: usize,
    active: bool,
}

impl ThreadEventStore {
    fn new(capacity: usize) -> Self {
        Self {
            session_configured: None,
            buffer: VecDeque::new(),
            user_message_ids: HashSet::new(),
            capacity,
            active: false,
        }
    }

    #[allow(dead_code)]
    fn new_with_session_configured(capacity: usize, event: Event) -> Self {
        let mut store = Self::new(capacity);
        store.session_configured = Some(event);
        store
    }

    fn push_event(&mut self, event: Event) {
        match &event.msg {
            EventMsg::SessionConfigured(_) => {
                self.session_configured = Some(event);
                return;
            }
            EventMsg::ItemCompleted(completed) => {
                if let TurnItem::UserMessage(item) = &completed.item {
                    if !event.id.is_empty() && self.user_message_ids.contains(&event.id) {
                        return;
                    }
                    let legacy = Event {
                        id: event.id,
                        msg: item.as_legacy_event(),
                    };
                    self.push_legacy_event(legacy);
                    return;
                }
            }
            _ => {}
        }

        self.push_legacy_event(event);
    }

    fn push_legacy_event(&mut self, event: Event) {
        if let EventMsg::UserMessage(_) = &event.msg
            && !event.id.is_empty()
            && !self.user_message_ids.insert(event.id.clone())
        {
            return;
        }
        self.buffer.push_back(event);
        if self.buffer.len() > self.capacity
            && let Some(removed) = self.buffer.pop_front()
            && matches!(removed.msg, EventMsg::UserMessage(_))
            && !removed.id.is_empty()
        {
            self.user_message_ids.remove(&removed.id);
        }
    }

    fn snapshot(&self) -> ThreadEventSnapshot {
        ThreadEventSnapshot {
            session_configured: self.session_configured.clone(),
            events: self.buffer.iter().cloned().collect(),
        }
    }
}

#[derive(Debug)]
struct ThreadEventChannel {
    sender: mpsc::Sender<Event>,
    receiver: Option<mpsc::Receiver<Event>>,
    store: Arc<Mutex<ThreadEventStore>>,
}

impl ThreadEventChannel {
    fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Self {
            sender,
            receiver: Some(receiver),
            store: Arc::new(Mutex::new(ThreadEventStore::new(capacity))),
        }
    }

    #[allow(dead_code)]
    fn new_with_session_configured(capacity: usize, event: Event) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Self {
            sender,
            receiver: Some(receiver),
            store: Arc::new(Mutex::new(ThreadEventStore::new_with_session_configured(
                capacity, event,
            ))),
        }
    }
}

fn should_show_model_migration_prompt(
    current_model: &str,
    target_model: &str,
    seen_migrations: &BTreeMap<String, String>,
    available_models: &[ModelPreset],
) -> bool {
    if target_model == current_model {
        return false;
    }

    if let Some(seen_target) = seen_migrations.get(current_model)
        && seen_target == target_model
    {
        return false;
    }

    if available_models
        .iter()
        .any(|preset| preset.model == current_model && preset.upgrade.is_some())
    {
        return true;
    }

    if available_models
        .iter()
        .any(|preset| preset.upgrade.as_ref().map(|u| u.id.as_str()) == Some(target_model))
    {
        return true;
    }

    false
}

fn migration_prompt_hidden(config: &Config, migration_config_key: &str) -> bool {
    match migration_config_key {
        HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG => config
            .notices
            .hide_gpt_5_1_codex_max_migration_prompt
            .unwrap_or(false),
        HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG => {
            config.notices.hide_gpt5_1_migration_prompt.unwrap_or(false)
        }
        _ => false,
    }
}

fn target_preset_for_upgrade<'a>(
    available_models: &'a [ModelPreset],
    target_model: &str,
) -> Option<&'a ModelPreset> {
    available_models
        .iter()
        .find(|preset| preset.model == target_model)
}

async fn handle_model_migration_prompt_if_needed(
    tui: &mut tui::Tui,
    config: &mut Config,
    model: &str,
    app_event_tx: &AppEventSender,
    available_models: Vec<ModelPreset>,
) -> Option<AppExitInfo> {
    let upgrade = available_models
        .iter()
        .find(|preset| preset.model == model)
        .and_then(|preset| preset.upgrade.as_ref());

    if let Some(ModelUpgrade {
        id: target_model,
        reasoning_effort_mapping,
        migration_config_key,
        model_link,
        upgrade_copy,
        migration_markdown,
    }) = upgrade
    {
        if migration_prompt_hidden(config, migration_config_key.as_str()) {
            return None;
        }

        let target_model = target_model.to_string();
        if !should_show_model_migration_prompt(
            model,
            &target_model,
            &config.notices.model_migrations,
            &available_models,
        ) {
            return None;
        }

        let current_preset = available_models.iter().find(|preset| preset.model == model);
        let target_preset = target_preset_for_upgrade(&available_models, &target_model);
        let target_preset = target_preset?;
        let target_display_name = target_preset.display_name.clone();
        let heading_label = if target_display_name == model {
            target_model.clone()
        } else {
            target_display_name.clone()
        };
        let target_description =
            (!target_preset.description.is_empty()).then(|| target_preset.description.clone());
        let can_opt_out = current_preset.is_some();
        let prompt_copy = migration_copy_for_models(
            model,
            &target_model,
            model_link.clone(),
            upgrade_copy.clone(),
            migration_markdown.clone(),
            heading_label,
            target_description,
            can_opt_out,
        );
        match run_model_migration_prompt(tui, prompt_copy).await {
            ModelMigrationOutcome::Accepted => {
                app_event_tx.send(AppEvent::PersistModelMigrationPromptAcknowledged {
                    from_model: model.to_string(),
                    to_model: target_model.clone(),
                });

                let mapped_effort = if let Some(reasoning_effort_mapping) = reasoning_effort_mapping
                    && let Some(reasoning_effort) = config.model_reasoning_effort
                {
                    reasoning_effort_mapping
                        .get(&reasoning_effort)
                        .cloned()
                        .or(config.model_reasoning_effort)
                } else {
                    config.model_reasoning_effort
                };

                config.model = Some(target_model.clone());
                config.model_reasoning_effort = mapped_effort;
                app_event_tx.send(AppEvent::UpdateModel(target_model.clone()));
                app_event_tx.send(AppEvent::UpdateReasoningEffort(mapped_effort));
                app_event_tx.send(AppEvent::PersistModelSelection {
                    model: target_model.clone(),
                    effort: mapped_effort,
                });
            }
            ModelMigrationOutcome::Rejected => {
                app_event_tx.send(AppEvent::PersistModelMigrationPromptAcknowledged {
                    from_model: model.to_string(),
                    to_model: target_model.clone(),
                });
            }
            ModelMigrationOutcome::Exit => {
                return Some(AppExitInfo {
                    token_usage: TokenUsage::default(),
                    thread_id: None,
                    update_action: None,
                    exit_reason: ExitReason::UserRequested,
                });
            }
        }
    }

    None
}

pub(crate) struct App {
    pub(crate) app_server: AppServerClient,
    pub(crate) models_manager: Arc<codex_core::models_manager::manager::ModelsManager>,
    pub(crate) otel_manager: OtelManager,
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) chat_widget: ChatWidget,
    pub(crate) auth_manager: Arc<AuthManager>,
    /// Config is stored here so we can recreate ChatWidgets as needed.
    pub(crate) config: Config,
    pub(crate) active_profile: Option<String>,
    cli_kv_overrides: Vec<(String, TomlValue)>,
    harness_overrides: ConfigOverrides,
    runtime_approval_policy_override: Option<AskForApproval>,
    runtime_sandbox_policy_override: Option<SandboxPolicy>,

    pub(crate) file_search: FileSearchManager,

    pub(crate) transcript_cells: Vec<Arc<dyn HistoryCell>>,

    // Pager overlay state (Transcript or Static like Diff)
    pub(crate) overlay: Option<Overlay>,
    pub(crate) deferred_history_lines: Vec<Line<'static>>,
    has_emitted_history_lines: bool,

    pub(crate) enhanced_keys_supported: bool,

    /// Controls the animation thread that sends CommitTick events.
    pub(crate) commit_anim_running: Arc<AtomicBool>,

    // Esc-backtracking state grouped
    pub(crate) backtrack: crate::app_backtrack::BacktrackState,
    /// When set, the next draw re-renders the transcript into terminal scrollback once.
    ///
    /// This is used after a confirmed thread rollback to ensure scrollback reflects the trimmed
    /// transcript cells.
    pub(crate) backtrack_render_pending: bool,
    pub(crate) feedback: codex_feedback::CodexFeedback,
    feedback_audience: FeedbackAudience,
    /// Set when the user confirms an update; propagated on exit.
    pub(crate) pending_update_action: Option<UpdateAction>,

    /// Ignore the next ShutdownComplete event when we're intentionally
    /// stopping a thread (e.g., before starting a new one).
    suppress_shutdown_complete: bool,

    windows_sandbox: WindowsSandboxState,

    thread_event_channels: HashMap<ThreadId, ThreadEventChannel>,
    active_thread_id: Option<ThreadId>,
    active_thread_rx: Option<mpsc::Receiver<Event>>,
    primary_thread_id: Option<ThreadId>,
    primary_session_configured: Option<SessionConfiguredEvent>,
    pending_primary_events: VecDeque<Event>,
}

#[derive(Default)]
struct WindowsSandboxState {
    setup_started_at: Option<Instant>,
    // One-shot suppression of the next world-writable scan after user confirmation.
    skip_world_writable_scan_once: bool,
}

fn normalize_harness_overrides_for_cwd(
    mut overrides: ConfigOverrides,
    base_cwd: &Path,
) -> Result<ConfigOverrides> {
    if overrides.additional_writable_roots.is_empty() {
        return Ok(overrides);
    }

    let mut normalized = Vec::with_capacity(overrides.additional_writable_roots.len());
    for root in overrides.additional_writable_roots.drain(..) {
        let absolute = AbsolutePathBuf::resolve_path_against_base(root, base_cwd)?;
        normalized.push(absolute.into_path_buf());
    }
    overrides.additional_writable_roots = normalized;
    Ok(overrides)
}

impl App {
    fn arm_shutdown_exit_fallback(&self) {
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(SHUTDOWN_FALLBACK_TIMEOUT).await;
            tx.send(AppEvent::Exit(ExitMode::Immediate));
        });
    }

    pub fn chatwidget_init_for_forked_or_resumed_thread(
        &self,
        tui: &mut tui::Tui,
        cfg: codex_core::config::Config,
    ) -> crate::chatwidget::ChatWidgetInit {
        crate::chatwidget::ChatWidgetInit {
            config: cfg,
            frame_requester: tui.frame_requester(),
            app_event_tx: self.app_event_tx.clone(),
            // Fork/resume bootstraps here don't carry any prefilled message content.
            initial_user_message: None,
            enhanced_keys_supported: self.enhanced_keys_supported,
            auth_manager: self.auth_manager.clone(),
            models_manager: self.models_manager.clone(),
            feedback: self.feedback.clone(),
            is_first_run: false,
            feedback_audience: self.feedback_audience,
            model: Some(self.chat_widget.current_model().to_string()),
            otel_manager: self.otel_manager.clone(),
        }
    }

    async fn rebuild_config_for_cwd(&self, cwd: PathBuf) -> Result<Config> {
        let mut overrides = self.harness_overrides.clone();
        overrides.cwd = Some(cwd.clone());
        let cwd_display = cwd.display().to_string();
        ConfigBuilder::default()
            .codex_home(self.config.codex_home.clone())
            .cli_overrides(self.cli_kv_overrides.clone())
            .harness_overrides(overrides)
            .build()
            .await
            .wrap_err_with(|| format!("Failed to rebuild config for cwd {cwd_display}"))
    }

    fn apply_runtime_policy_overrides(&mut self, config: &mut Config) {
        if let Some(policy) = self.runtime_approval_policy_override.as_ref()
            && let Err(err) = config.approval_policy.set(*policy)
        {
            tracing::warn!(%err, "failed to carry forward approval policy override");
            self.chat_widget.add_error_message(format!(
                "Failed to carry forward approval policy override: {err}"
            ));
        }
        if let Some(policy) = self.runtime_sandbox_policy_override.as_ref()
            && let Err(err) = config.sandbox_policy.set(policy.clone())
        {
            tracing::warn!(%err, "failed to carry forward sandbox policy override");
            self.chat_widget.add_error_message(format!(
                "Failed to carry forward sandbox policy override: {err}"
            ));
        }
    }

    async fn shutdown_current_thread(&mut self) {
        if let Some(thread_id) = self.chat_widget.thread_id() {
            // Clear any in-flight rollback guard when switching threads.
            self.backtrack.pending_rollback = None;
            self.suppress_shutdown_complete = true;
            let params = codex_app_server_protocol::ThreadShutdownParams {
                thread_id: thread_id.to_string(),
            };
            if let Ok(pending) = self
                .app_server
                .request(
                    |id| codex_app_server_protocol::ClientRequest::ThreadShutdown {
                        request_id: id,
                        params,
                    },
                )
                .await
            {
                let _ = pending.discard().await;
            }
        }
    }

    fn sandbox_mode_override(
        policy: &codex_core::protocol::SandboxPolicy,
    ) -> Option<codex_app_server_protocol::SandboxMode> {
        match policy {
            codex_core::protocol::SandboxPolicy::DangerFullAccess => {
                Some(codex_app_server_protocol::SandboxMode::DangerFullAccess)
            }
            codex_core::protocol::SandboxPolicy::ReadOnly => {
                Some(codex_app_server_protocol::SandboxMode::ReadOnly)
            }
            codex_core::protocol::SandboxPolicy::WorkspaceWrite { .. } => {
                Some(codex_app_server_protocol::SandboxMode::WorkspaceWrite)
            }
            codex_core::protocol::SandboxPolicy::ExternalSandbox { .. } => None,
        }
    }

    fn build_thread_start_params(
        &self,
        config: &Config,
    ) -> codex_app_server_protocol::ThreadStartParams {
        codex_app_server_protocol::ThreadStartParams {
            model: config.model.clone(),
            model_provider: Some(config.model_provider_id.clone()),
            cwd: Some(config.cwd.display().to_string()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::from(
                *config.approval_policy.get(),
            )),
            sandbox: Self::sandbox_mode_override(config.sandbox_policy.get()),
            config: None,
            base_instructions: None,
            developer_instructions: None,
            dynamic_tools: None,
            experimental_raw_events: false,
            personality: config.model_personality,
            ephemeral: None,
        }
    }

    fn build_thread_resume_params(
        &self,
        config: &Config,
        path: &Path,
    ) -> codex_app_server_protocol::ThreadResumeParams {
        codex_app_server_protocol::ThreadResumeParams {
            thread_id: String::new(),
            history: None,
            path: Some(path.to_path_buf()),
            model: config.model.clone(),
            model_provider: Some(config.model_provider_id.clone()),
            cwd: Some(config.cwd.display().to_string()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::from(
                *config.approval_policy.get(),
            )),
            sandbox: Self::sandbox_mode_override(config.sandbox_policy.get()),
            config: None,
            base_instructions: None,
            developer_instructions: None,
            personality: config.model_personality,
        }
    }

    fn build_thread_fork_params(
        &self,
        config: &Config,
        path: &Path,
    ) -> codex_app_server_protocol::ThreadForkParams {
        codex_app_server_protocol::ThreadForkParams {
            thread_id: String::new(),
            path: Some(path.to_path_buf()),
            model: config.model.clone(),
            model_provider: Some(config.model_provider_id.clone()),
            cwd: Some(config.cwd.display().to_string()),
            approval_policy: Some(codex_app_server_protocol::AskForApproval::from(
                *config.approval_policy.get(),
            )),
            sandbox: Self::sandbox_mode_override(config.sandbox_policy.get()),
            config: None,
            base_instructions: None,
            developer_instructions: None,
        }
    }

    async fn set_primary_thread(&mut self, thread_id: ThreadId) {
        self.primary_thread_id = Some(thread_id);
        self.ensure_thread_channel(thread_id);
        self.activate_thread_channel(thread_id).await;
    }

    async fn start_thread_for_config(&mut self, config: &Config) -> Result<ThreadId> {
        let params = self.build_thread_start_params(config);
        let pending = self
            .app_server
            .request(
                |request_id| codex_app_server_protocol::ClientRequest::ThreadStart {
                    request_id,
                    params,
                },
            )
            .await
            .map_err(app_server_error)?;
        let response: codex_app_server_protocol::ThreadStartResponse =
            pending.into_typed().await.map_err(app_server_error)?;
        let thread_id = ThreadId::from_string(&response.thread.id)
            .map_err(|err| color_eyre::eyre::eyre!("invalid thread id: {err}"))?;
        self.set_primary_thread(thread_id).await;
        if self.chat_widget.thread_id().is_none() {
            let event = session_configured_from_thread_response(
                thread_id,
                response.model,
                response.model_provider,
                response.approval_policy,
                response.sandbox,
                response.cwd,
                response.reasoning_effort,
                response.thread.path.clone(),
                None,
            );
            self.enqueue_thread_event(thread_id, event).await?;
        }
        Ok(thread_id)
    }

    async fn resume_thread_from_path(
        &mut self,
        config: &Config,
        path: &Path,
    ) -> Result<ThreadBootstrap> {
        let params = self.build_thread_resume_params(config, path);
        let pending = self
            .app_server
            .request(
                |request_id| codex_app_server_protocol::ClientRequest::ThreadResume {
                    request_id,
                    params,
                },
            )
            .await
            .map_err(app_server_error)?;
        let response: codex_app_server_protocol::ThreadResumeResponse =
            pending.into_typed().await.map_err(app_server_error)?;
        let thread_id = ThreadId::from_string(&response.thread.id)
            .map_err(|err| color_eyre::eyre::eyre!("invalid thread id: {err}"))?;
        let initial_messages = thread_turns_to_initial_messages(
            &response.thread.turns,
            self.config.show_raw_agent_reasoning,
        );
        let session_event = session_configured_from_thread_response(
            thread_id,
            response.model,
            response.model_provider,
            response.approval_policy,
            response.sandbox,
            response.cwd,
            response.reasoning_effort,
            response.thread.path,
            initial_messages,
        );
        Ok(ThreadBootstrap {
            thread_id,
            session_event,
        })
    }

    async fn fork_thread_from_path(
        &mut self,
        config: &Config,
        path: &Path,
    ) -> Result<ThreadBootstrap> {
        let params = self.build_thread_fork_params(config, path);
        let pending = self
            .app_server
            .request(
                |request_id| codex_app_server_protocol::ClientRequest::ThreadFork {
                    request_id,
                    params,
                },
            )
            .await
            .map_err(app_server_error)?;
        let response: codex_app_server_protocol::ThreadForkResponse =
            pending.into_typed().await.map_err(app_server_error)?;
        let thread_id = ThreadId::from_string(&response.thread.id)
            .map_err(|err| color_eyre::eyre::eyre!("invalid thread id: {err}"))?;
        let initial_messages = thread_turns_to_initial_messages(
            &response.thread.turns,
            self.config.show_raw_agent_reasoning,
        );
        let session_event = session_configured_from_thread_response(
            thread_id,
            response.model,
            response.model_provider,
            response.approval_policy,
            response.sandbox,
            response.cwd,
            response.reasoning_effort,
            response.thread.path,
            initial_messages,
        );
        Ok(ThreadBootstrap {
            thread_id,
            session_event,
        })
    }

    fn ensure_thread_channel(&mut self, thread_id: ThreadId) -> &mut ThreadEventChannel {
        self.thread_event_channels
            .entry(thread_id)
            .or_insert_with(|| ThreadEventChannel::new(THREAD_EVENT_CHANNEL_CAPACITY))
    }

    async fn set_thread_active(&mut self, thread_id: ThreadId, active: bool) {
        if let Some(channel) = self.thread_event_channels.get_mut(&thread_id) {
            let mut store = channel.store.lock().await;
            store.active = active;
        }
    }

    async fn activate_thread_channel(&mut self, thread_id: ThreadId) {
        if self.active_thread_id.is_some() {
            return;
        }
        self.set_thread_active(thread_id, true).await;
        let receiver = if let Some(channel) = self.thread_event_channels.get_mut(&thread_id) {
            channel.receiver.take()
        } else {
            None
        };
        self.active_thread_id = Some(thread_id);
        self.active_thread_rx = receiver;
    }

    async fn store_active_thread_receiver(&mut self) {
        let Some(active_id) = self.active_thread_id else {
            return;
        };
        let Some(receiver) = self.active_thread_rx.take() else {
            return;
        };
        if let Some(channel) = self.thread_event_channels.get_mut(&active_id) {
            let mut store = channel.store.lock().await;
            store.active = false;
            channel.receiver = Some(receiver);
        }
    }

    async fn activate_thread_for_replay(
        &mut self,
        thread_id: ThreadId,
    ) -> Option<(mpsc::Receiver<Event>, ThreadEventSnapshot)> {
        let channel = self.thread_event_channels.get_mut(&thread_id)?;
        let receiver = channel.receiver.take()?;
        let mut store = channel.store.lock().await;
        store.active = true;
        let snapshot = store.snapshot();
        Some((receiver, snapshot))
    }

    async fn clear_active_thread(&mut self) {
        if let Some(active_id) = self.active_thread_id.take() {
            self.set_thread_active(active_id, false).await;
        }
        self.active_thread_rx = None;
    }

    async fn enqueue_thread_event(&mut self, thread_id: ThreadId, event: Event) -> Result<()> {
        let (sender, store) = {
            let channel = self.ensure_thread_channel(thread_id);
            (channel.sender.clone(), Arc::clone(&channel.store))
        };

        let should_send = {
            let mut guard = store.lock().await;
            guard.push_event(event.clone());
            guard.active
        };

        if should_send {
            // Never await a bounded channel send on the main TUI loop: if the receiver falls behind,
            // `send().await` can block and the UI stops drawing. If the channel is full, wait in a
            // spawned task instead.
            match sender.try_send(event) {
                Ok(()) => {}
                Err(TrySendError::Full(event)) => {
                    tokio::spawn(async move {
                        if let Err(err) = sender.send(event).await {
                            tracing::warn!("thread {thread_id} event channel closed: {err}");
                        }
                    });
                }
                Err(TrySendError::Closed(_)) => {
                    tracing::warn!("thread {thread_id} event channel closed");
                }
            }
        }
        Ok(())
    }

    async fn enqueue_primary_event(&mut self, event: Event) -> Result<()> {
        if let Some(thread_id) = self.primary_thread_id {
            return self.enqueue_thread_event(thread_id, event).await;
        }

        if let EventMsg::SessionConfigured(session) = &event.msg {
            let thread_id = session.session_id;
            self.primary_thread_id = Some(thread_id);
            self.primary_session_configured = Some(session.clone());
            self.ensure_thread_channel(thread_id);
            self.activate_thread_channel(thread_id).await;

            let pending = std::mem::take(&mut self.pending_primary_events);
            for pending_event in pending {
                self.enqueue_thread_event(thread_id, pending_event).await?;
            }
            self.enqueue_thread_event(thread_id, event).await?;
        } else {
            self.pending_primary_events.push_back(event);
        }
        Ok(())
    }

    async fn enqueue_thread_event_with_primary(
        &mut self,
        thread_id: ThreadId,
        event: Event,
    ) -> Result<()> {
        if self.primary_thread_id.is_some() {
            return self.enqueue_thread_event(thread_id, event).await;
        }

        if let EventMsg::SessionConfigured(session) = &event.msg {
            self.primary_thread_id = Some(thread_id);
            self.primary_session_configured = Some(session.clone());
            self.ensure_thread_channel(thread_id);
            self.activate_thread_channel(thread_id).await;

            let pending = std::mem::take(&mut self.pending_primary_events);
            for pending_event in pending {
                self.enqueue_thread_event(thread_id, pending_event).await?;
            }
            self.enqueue_thread_event(thread_id, event).await?;
        } else {
            self.pending_primary_events.push_back(event);
        }
        Ok(())
    }

    fn open_agent_picker(&mut self) {
        if self.thread_event_channels.is_empty() {
            self.chat_widget
                .add_info_message("No agents available yet.".to_string(), None);
            return;
        }

        let mut thread_ids: Vec<ThreadId> = self.thread_event_channels.keys().cloned().collect();
        thread_ids.sort_by_key(ToString::to_string);

        let mut initial_selected_idx = None;
        let items: Vec<SelectionItem> = thread_ids
            .iter()
            .enumerate()
            .map(|(idx, thread_id)| {
                if self.active_thread_id == Some(*thread_id) {
                    initial_selected_idx = Some(idx);
                }
                let id = *thread_id;
                SelectionItem {
                    name: thread_id.to_string(),
                    is_current: self.active_thread_id == Some(*thread_id),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::SelectAgentThread(id));
                    })],
                    dismiss_on_select: true,
                    search_value: Some(thread_id.to_string()),
                    ..Default::default()
                }
            })
            .collect();

        self.chat_widget.show_selection_view(SelectionViewParams {
            title: Some("Agents".to_string()),
            subtitle: Some("Select a thread to focus".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            initial_selected_idx,
            ..Default::default()
        });
    }

    async fn select_agent_thread(&mut self, tui: &mut tui::Tui, thread_id: ThreadId) -> Result<()> {
        if self.active_thread_id == Some(thread_id) {
            return Ok(());
        }

        let previous_thread_id = self.active_thread_id;
        self.store_active_thread_receiver().await;
        self.active_thread_id = None;
        let Some((receiver, snapshot)) = self.activate_thread_for_replay(thread_id).await else {
            self.chat_widget
                .add_error_message(format!("Agent thread {thread_id} is already active."));
            if let Some(previous_thread_id) = previous_thread_id {
                self.activate_thread_channel(previous_thread_id).await;
            }
            return Ok(());
        };

        self.active_thread_id = Some(thread_id);
        self.active_thread_rx = Some(receiver);

        let init = self.chatwidget_init_for_forked_or_resumed_thread(tui, self.config.clone());
        self.chat_widget = ChatWidget::new(init);

        self.reset_for_thread_switch(tui)?;
        self.replay_thread_snapshot(snapshot);
        self.drain_active_thread_events(tui).await?;

        Ok(())
    }

    fn reset_for_thread_switch(&mut self, tui: &mut tui::Tui) -> Result<()> {
        self.overlay = None;
        self.transcript_cells.clear();
        self.deferred_history_lines.clear();
        self.has_emitted_history_lines = false;
        self.backtrack = BacktrackState::default();
        self.backtrack_render_pending = false;
        tui.terminal.clear_scrollback()?;
        tui.terminal.clear()?;
        Ok(())
    }

    fn reset_thread_event_state(&mut self) {
        self.thread_event_channels.clear();
        self.active_thread_id = None;
        self.active_thread_rx = None;
        self.primary_thread_id = None;
        self.pending_primary_events.clear();
    }

    async fn drain_active_thread_events(&mut self, tui: &mut tui::Tui) -> Result<()> {
        let Some(mut rx) = self.active_thread_rx.take() else {
            return Ok(());
        };

        let mut disconnected = false;
        loop {
            match rx.try_recv() {
                Ok(event) => self.handle_codex_event_now(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        if !disconnected {
            self.active_thread_rx = Some(rx);
        } else {
            self.clear_active_thread().await;
        }

        if self.backtrack_render_pending {
            tui.frame_requester().schedule_frame();
        }
        Ok(())
    }

    fn replay_thread_snapshot(&mut self, snapshot: ThreadEventSnapshot) {
        if let Some(event) = snapshot.session_configured {
            self.handle_codex_event_replay(event);
        }
        for event in snapshot.events {
            self.handle_codex_event_replay(event);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        tui: &mut tui::Tui,
        auth_manager: Arc<AuthManager>,
        mut config: Config,
        cli_kv_overrides: Vec<(String, TomlValue)>,
        harness_overrides: ConfigOverrides,
        active_profile: Option<String>,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
        session_selection: SessionSelection,
        feedback: codex_feedback::CodexFeedback,
        is_first_run: bool,
        ollama_chat_support_notice: Option<DeprecationNoticeEvent>,
    ) -> Result<AppExitInfo> {
        use tokio_stream::StreamExt;
        let (app_event_tx, mut app_event_rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(app_event_tx);
        emit_deprecation_notice(&app_event_tx, ollama_chat_support_notice);
        emit_project_config_warnings(&app_event_tx, &config);
        tui.set_notification_method(config.tui_notification_method);

        let harness_overrides =
            normalize_harness_overrides_for_cwd(harness_overrides, &config.cwd)?;
        let models_manager = Arc::new(codex_core::models_manager::manager::ModelsManager::new(
            config.codex_home.clone(),
            auth_manager.clone(),
        ));
        let mut model = models_manager
            .get_default_model(&config.model, &config, RefreshStrategy::Offline)
            .await;
        let available_models = models_manager
            .list_models(&config, RefreshStrategy::Offline)
            .await;
        let exit_info = handle_model_migration_prompt_if_needed(
            tui,
            &mut config,
            model.as_str(),
            &app_event_tx,
            available_models,
        )
        .await;
        if let Some(exit_info) = exit_info {
            return Ok(exit_info);
        }
        if let Some(updated_model) = config.model.clone() {
            model = updated_model;
        }

        let auth = auth_manager.auth().await;
        let auth_ref = auth.as_ref();
        // Determine who should see internal Slack routing. We treat
        // `@openai.com` emails as employees and default to `External` when the
        // email is unavailable (for example, API key auth).
        let feedback_audience = if auth_ref
            .and_then(CodexAuth::get_account_email)
            .is_some_and(|email| email.ends_with("@openai.com"))
        {
            FeedbackAudience::OpenAiEmployee
        } else {
            FeedbackAudience::External
        };
        let otel_manager = OtelManager::new(
            ThreadId::new(),
            model.as_str(),
            model.as_str(),
            auth_ref.and_then(CodexAuth::get_account_id),
            auth_ref.and_then(CodexAuth::get_account_email),
            auth_ref.map(|auth| auth.mode),
            config.otel.log_user_prompt,
            codex_core::terminal::user_agent(),
            SessionSource::Cli,
        );

        let enhanced_keys_supported = tui.enhanced_keys_supported();
        let app_server = AppServerClient::spawn(
            app_event_tx.clone(),
            Arc::new(config.clone()),
            cli_kv_overrides.clone(),
            LoaderOverrides::default(),
            feedback.clone(),
            Vec::new(),
            SessionSource::Cli,
        );
        let init_params = codex_app_server_protocol::InitializeParams {
            client_info: codex_app_server_protocol::ClientInfo {
                name: "codex_tui".to_string(),
                title: Some("Codex TUI".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };
        match app_server
            .request(
                |request_id| codex_app_server_protocol::ClientRequest::Initialize {
                    request_id,
                    params: init_params,
                },
            )
            .await
        {
            Ok(pending) => {
                pending.discard().await.map_err(app_server_error)?;
                app_server
                    .send_notification(codex_app_server_protocol::ClientNotification::Initialized)
                    .await
                    .map_err(app_server_error)?;
            }
            Err(err) => {
                return Ok(AppExitInfo::fatal(format!(
                    "Failed to initialize app server: {err:?}"
                )));
            }
        }

        let init = crate::chatwidget::ChatWidgetInit {
            config: config.clone(),
            frame_requester: tui.frame_requester(),
            app_event_tx: app_event_tx.clone(),
            initial_user_message: crate::chatwidget::create_initial_user_message(
                initial_prompt.clone(),
                initial_images.clone(),
                // CLI prompt args are plain strings, so they don't provide element ranges.
                Vec::new(),
            ),
            enhanced_keys_supported,
            auth_manager: auth_manager.clone(),
            models_manager: models_manager.clone(),
            feedback: feedback.clone(),
            is_first_run,
            feedback_audience,
            model: Some(model.clone()),
            otel_manager: otel_manager.clone(),
        };
        let mut chat_widget = ChatWidget::new(init);

        chat_widget.maybe_prompt_windows_sandbox_enable();

        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());
        #[cfg(not(debug_assertions))]
        let upgrade_version = crate::updates::get_upgrade_version(&config);

        let mut app = Self {
            app_server,
            models_manager,
            otel_manager: otel_manager.clone(),
            app_event_tx,
            chat_widget,
            auth_manager: auth_manager.clone(),
            config,
            active_profile,
            cli_kv_overrides,
            harness_overrides,
            runtime_approval_policy_override: None,
            runtime_sandbox_policy_override: None,
            file_search,
            enhanced_keys_supported,
            transcript_cells: Vec::new(),
            overlay: None,
            deferred_history_lines: Vec::new(),
            has_emitted_history_lines: false,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            backtrack: BacktrackState::default(),
            backtrack_render_pending: false,
            feedback: feedback.clone(),
            feedback_audience,
            pending_update_action: None,
            suppress_shutdown_complete: false,
            windows_sandbox: WindowsSandboxState::default(),
            thread_event_channels: HashMap::new(),
            active_thread_id: None,
            active_thread_rx: None,
            primary_thread_id: None,
            primary_session_configured: None,
            pending_primary_events: VecDeque::new(),
        };

        match session_selection {
            SessionSelection::Resume(path) => {
                let config = app.config.clone();
                let bootstrap = app
                    .resume_thread_from_path(&config, &path)
                    .await
                    .wrap_err_with(|| {
                        let path_display = path.display();
                        format!("Failed to resume session from {path_display}")
                    })?;
                app.set_primary_thread(bootstrap.thread_id).await;
                app.enqueue_thread_event(bootstrap.thread_id, bootstrap.session_event)
                    .await?;
            }
            SessionSelection::Fork(path) => {
                let config = app.config.clone();
                let bootstrap = app
                    .fork_thread_from_path(&config, &path)
                    .await
                    .wrap_err_with(|| {
                        let path_display = path.display();
                        format!("Failed to fork session from {path_display}")
                    })?;
                app.set_primary_thread(bootstrap.thread_id).await;
                app.enqueue_thread_event(bootstrap.thread_id, bootstrap.session_event)
                    .await?;
            }
            SessionSelection::StartFresh | SessionSelection::Exit => {
                let config = app.config.clone();
                app.start_thread_for_config(&config).await?;
            }
        }

        // On startup, if Agent mode (workspace-write) or ReadOnly is active, warn about world-writable dirs on Windows.
        #[cfg(target_os = "windows")]
        {
            let should_check = WindowsSandboxLevel::from_config(&app.config)
                != WindowsSandboxLevel::Disabled
                && matches!(
                    app.config.sandbox_policy.get(),
                    codex_core::protocol::SandboxPolicy::WorkspaceWrite { .. }
                        | codex_core::protocol::SandboxPolicy::ReadOnly
                )
                && !app
                    .config
                    .notices
                    .hide_world_writable_warning
                    .unwrap_or(false);
            if should_check {
                let cwd = app.config.cwd.clone();
                let env_map: std::collections::HashMap<String, String> = std::env::vars().collect();
                let tx = app.app_event_tx.clone();
                let logs_base_dir = app.config.codex_home.clone();
                let sandbox_policy = app.config.sandbox_policy.get().clone();
                Self::spawn_world_writable_scan(cwd, env_map, logs_base_dir, sandbox_policy, tx);
            }
        }

        #[cfg(not(debug_assertions))]
        if let Some(latest_version) = upgrade_version {
            let control = app
                .handle_event(
                    tui,
                    AppEvent::InsertHistoryCell(Box::new(UpdateAvailableHistoryCell::new(
                        latest_version,
                        crate::update_action::get_update_action(),
                    ))),
                )
                .await?;
            if let AppRunControl::Exit(exit_reason) = control {
                return Ok(AppExitInfo {
                    token_usage: app.token_usage(),
                    thread_id: app.chat_widget.thread_id(),
                    update_action: app.pending_update_action,
                    exit_reason,
                });
            }
        }

        let tui_events = tui.event_stream();
        tokio::pin!(tui_events);

        tui.frame_requester().schedule_frame();

        let exit_reason = loop {
            let control = select! {
                Some(event) = app_event_rx.recv() => {
                    app.handle_event(tui, event).await?
                }
                active = async {
                    if let Some(rx) = app.active_thread_rx.as_mut() {
                        rx.recv().await
                    } else {
                        None
                    }
                }, if app.active_thread_rx.is_some() => {
                    if let Some(event) = active {
                        app.handle_active_thread_event(tui, event)?;
                    } else {
                        app.clear_active_thread().await;
                    }
                    AppRunControl::Continue
                }
                Some(event) = tui_events.next() => {
                    app.handle_tui_event(tui, event).await?
                }
            };
            match control {
                AppRunControl::Continue => {}
                AppRunControl::Exit(reason) => break reason,
            }
        };
        tui.terminal.clear()?;
        Ok(AppExitInfo {
            token_usage: app.token_usage(),
            thread_id: app.chat_widget.thread_id(),
            update_action: app.pending_update_action,
            exit_reason,
        })
    }

    pub(crate) async fn handle_tui_event(
        &mut self,
        tui: &mut tui::Tui,
        event: TuiEvent,
    ) -> Result<AppRunControl> {
        if self.overlay.is_some() {
            let _ = self.handle_backtrack_overlay_event(tui, event).await?;
        } else {
            match event {
                TuiEvent::Key(key_event) => {
                    self.handle_key_event(tui, key_event).await;
                }
                TuiEvent::Paste(pasted) => {
                    // Many terminals convert newlines to \r when pasting (e.g., iTerm2),
                    // but tui-textarea expects \n. Normalize CR to LF.
                    // [tui-textarea]: https://github.com/rhysd/tui-textarea/blob/4d18622eeac13b309e0ff6a55a46ac6706da68cf/src/textarea.rs#L782-L783
                    // [iTerm2]: https://github.com/gnachman/iTerm2/blob/5d0c0d9f68523cbd0494dad5422998964a2ecd8d/sources/iTermPasteHelper.m#L206-L216
                    let pasted = pasted.replace("\r", "\n");
                    self.chat_widget.handle_paste(pasted);
                }
                TuiEvent::Draw => {
                    if self.backtrack_render_pending {
                        self.backtrack_render_pending = false;
                        self.render_transcript_once(tui);
                    }
                    self.chat_widget.maybe_post_pending_notification(tui);
                    if self
                        .chat_widget
                        .handle_paste_burst_tick(tui.frame_requester())
                    {
                        return Ok(AppRunControl::Continue);
                    }
                    tui.draw(
                        self.chat_widget.desired_height(tui.terminal.size()?.width),
                        |frame| {
                            self.chat_widget.render(frame.area(), frame.buffer);
                            if let Some((x, y)) = self.chat_widget.cursor_pos(frame.area()) {
                                frame.set_cursor_position((x, y));
                            }
                        },
                    )?;
                    if self.chat_widget.external_editor_state() == ExternalEditorState::Requested {
                        self.chat_widget
                            .set_external_editor_state(ExternalEditorState::Active);
                        self.app_event_tx.send(AppEvent::LaunchExternalEditor);
                    }
                }
            }
        }
        Ok(AppRunControl::Continue)
    }

    async fn handle_event(&mut self, tui: &mut tui::Tui, event: AppEvent) -> Result<AppRunControl> {
        match event {
            AppEvent::NewSession => {
                let model = self.chat_widget.current_model().to_string();
                let summary =
                    session_summary(self.chat_widget.token_usage(), self.chat_widget.thread_id());
                self.shutdown_current_thread().await;
                self.reset_thread_event_state();
                let init = crate::chatwidget::ChatWidgetInit {
                    config: self.config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: self.app_event_tx.clone(),
                    // New sessions start without prefilled message content.
                    initial_user_message: None,
                    enhanced_keys_supported: self.enhanced_keys_supported,
                    auth_manager: self.auth_manager.clone(),
                    models_manager: self.models_manager.clone(),
                    feedback: self.feedback.clone(),
                    is_first_run: false,
                    feedback_audience: self.feedback_audience,
                    model: Some(model),
                    otel_manager: self.otel_manager.clone(),
                };
                self.chat_widget = ChatWidget::new(init);
                let config = self.config.clone();
                if let Err(err) = self.start_thread_for_config(&config).await {
                    self.chat_widget
                        .add_error_message(format!("Failed to start new session: {err}"));
                }
                if let Some(summary) = summary {
                    let mut lines: Vec<Line<'static>> = vec![summary.usage_line.clone().into()];
                    if let Some(command) = summary.resume_command {
                        let spans = vec!["To continue this session, run ".into(), command.cyan()];
                        lines.push(spans.into());
                    }
                    self.chat_widget.add_plain_history_lines(lines);
                }
                tui.frame_requester().schedule_frame();
            }
            AppEvent::OpenResumePicker => {
                match crate::resume_picker::run_resume_picker(
                    tui,
                    &self.config.codex_home,
                    &self.config.model_provider_id,
                    false,
                )
                .await?
                {
                    SessionSelection::Resume(path) => {
                        let current_cwd = self.config.cwd.clone();
                        let resume_cwd = match crate::resolve_cwd_for_resume_or_fork(
                            tui,
                            &current_cwd,
                            &path,
                            CwdPromptAction::Resume,
                            true,
                        )
                        .await?
                        {
                            Some(cwd) => cwd,
                            None => current_cwd.clone(),
                        };
                        let mut resume_config = if crate::cwds_differ(&current_cwd, &resume_cwd) {
                            match self.rebuild_config_for_cwd(resume_cwd).await {
                                Ok(cfg) => cfg,
                                Err(err) => {
                                    self.chat_widget.add_error_message(format!(
                                        "Failed to rebuild configuration for resume: {err}"
                                    ));
                                    return Ok(AppRunControl::Continue);
                                }
                            }
                        } else {
                            // No rebuild needed: current_cwd comes from self.config.cwd.
                            self.config.clone()
                        };
                        self.apply_runtime_policy_overrides(&mut resume_config);
                        let summary = session_summary(
                            self.chat_widget.token_usage(),
                            self.chat_widget.thread_id(),
                        );
                        match self.resume_thread_from_path(&resume_config, &path).await {
                            Ok(bootstrap) => {
                                self.shutdown_current_thread().await;
                                self.config = resume_config;
                                tui.set_notification_method(self.config.tui_notification_method);
                                self.file_search = FileSearchManager::new(
                                    self.config.cwd.clone(),
                                    self.app_event_tx.clone(),
                                );
                                let init = self.chatwidget_init_for_forked_or_resumed_thread(
                                    tui,
                                    self.config.clone(),
                                );
                                self.chat_widget = ChatWidget::new(init);
                                self.reset_thread_event_state();
                                self.set_primary_thread(bootstrap.thread_id).await;
                                self.enqueue_thread_event(
                                    bootstrap.thread_id,
                                    bootstrap.session_event,
                                )
                                .await?;
                                if let Some(summary) = summary {
                                    let mut lines: Vec<Line<'static>> =
                                        vec![summary.usage_line.clone().into()];
                                    if let Some(command) = summary.resume_command {
                                        let spans = vec![
                                            "To continue this session, run ".into(),
                                            command.cyan(),
                                        ];
                                        lines.push(spans.into());
                                    }
                                    self.chat_widget.add_plain_history_lines(lines);
                                }
                            }
                            Err(err) => {
                                let path_display = path.display();
                                self.chat_widget.add_error_message(format!(
                                    "Failed to resume session from {path_display}: {err}"
                                ));
                            }
                        }
                    }
                    SessionSelection::Exit
                    | SessionSelection::StartFresh
                    | SessionSelection::Fork(_) => {}
                }

                // Leaving alt-screen may blank the inline viewport; force a redraw either way.
                tui.frame_requester().schedule_frame();
            }
            AppEvent::ForkCurrentSession => {
                let summary =
                    session_summary(self.chat_widget.token_usage(), self.chat_widget.thread_id());
                if let Some(path) = self.chat_widget.rollout_path() {
                    let config = self.config.clone();
                    match self.fork_thread_from_path(&config, &path).await {
                        Ok(bootstrap) => {
                            self.shutdown_current_thread().await;
                            let init = self.chatwidget_init_for_forked_or_resumed_thread(
                                tui,
                                self.config.clone(),
                            );
                            self.chat_widget = ChatWidget::new(init);
                            self.reset_thread_event_state();
                            self.set_primary_thread(bootstrap.thread_id).await;
                            self.enqueue_thread_event(bootstrap.thread_id, bootstrap.session_event)
                                .await?;
                            if let Some(summary) = summary {
                                let mut lines: Vec<Line<'static>> =
                                    vec![summary.usage_line.clone().into()];
                                if let Some(command) = summary.resume_command {
                                    let spans = vec![
                                        "To continue this session, run ".into(),
                                        command.cyan(),
                                    ];
                                    lines.push(spans.into());
                                }
                                self.chat_widget.add_plain_history_lines(lines);
                            }
                        }
                        Err(err) => {
                            let path_display = path.display();
                            self.chat_widget.add_error_message(format!(
                                "Failed to fork current session from {path_display}: {err}"
                            ));
                        }
                    }
                } else {
                    self.chat_widget
                        .add_error_message("Current session is not ready to fork yet.".to_string());
                }

                tui.frame_requester().schedule_frame();
            }
            AppEvent::InsertHistoryCell(cell) => {
                let cell: Arc<dyn HistoryCell> = cell.into();
                if let Some(Overlay::Transcript(t)) = &mut self.overlay {
                    t.insert_cell(cell.clone());
                    tui.frame_requester().schedule_frame();
                }
                self.transcript_cells.push(cell.clone());
                let mut display = cell.display_lines(tui.terminal.last_known_screen_size.width);
                if !display.is_empty() {
                    // Only insert a separating blank line for new cells that are not
                    // part of an ongoing stream. Streaming continuations should not
                    // accrue extra blank lines between chunks.
                    if !cell.is_stream_continuation() {
                        if self.has_emitted_history_lines {
                            display.insert(0, Line::from(""));
                        } else {
                            self.has_emitted_history_lines = true;
                        }
                    }
                    if self.overlay.is_some() {
                        self.deferred_history_lines.extend(display);
                    } else {
                        tui.insert_history_lines(display);
                    }
                }
            }
            AppEvent::StartCommitAnimation => {
                if self
                    .commit_anim_running
                    .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    let tx = self.app_event_tx.clone();
                    let running = self.commit_anim_running.clone();
                    thread::spawn(move || {
                        while running.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_millis(50));
                            tx.send(AppEvent::CommitTick);
                        }
                    });
                }
            }
            AppEvent::StopCommitAnimation => {
                self.commit_anim_running.store(false, Ordering::Release);
            }
            AppEvent::CommitTick => {
                self.chat_widget.on_commit_tick();
            }
            AppEvent::CodexEvent(event) => {
                self.enqueue_primary_event(event).await?;
            }
            AppEvent::CodexThreadEvent { thread_id, event } => {
                self.enqueue_thread_event_with_primary(thread_id, event)
                    .await?;
            }
            AppEvent::Exit(mode) => match mode {
                ExitMode::ShutdownFirst => {
                    self.suppress_shutdown_complete = false;
                    if self.chat_widget.thread_id().is_none() {
                        return Ok(AppRunControl::Exit(ExitReason::UserRequested));
                    }
                    self.handle_app_server_action(AppServerAction::Shutdown)
                        .await?;
                    self.arm_shutdown_exit_fallback();
                }
                ExitMode::Immediate => {
                    return Ok(AppRunControl::Exit(ExitReason::UserRequested));
                }
            },
            AppEvent::FatalExitRequest(message) => {
                return Ok(AppRunControl::Exit(ExitReason::Fatal(message)));
            }
            AppEvent::AppServerAction(action) => {
                self.handle_app_server_action(action).await?;
            }
            AppEvent::DiffResult(text) => {
                // Clear the in-progress state in the bottom pane
                self.chat_widget.on_diff_complete();
                // Enter alternate screen using TUI helper and build pager lines
                let _ = tui.enter_alt_screen();
                let pager_lines: Vec<ratatui::text::Line<'static>> = if text.trim().is_empty() {
                    vec!["No changes detected.".italic().into()]
                } else {
                    text.lines().map(ansi_escape_line).collect()
                };
                self.overlay = Some(Overlay::new_static_with_lines(
                    pager_lines,
                    "D I F F".to_string(),
                ));
                tui.frame_requester().schedule_frame();
            }
            AppEvent::OpenAppLink {
                title,
                description,
                instructions,
                url,
                is_installed,
            } => {
                self.chat_widget.open_app_link_view(
                    title,
                    description,
                    instructions,
                    url,
                    is_installed,
                );
            }
            AppEvent::StartFileSearch(query) => {
                self.file_search.on_user_query(query);
            }
            AppEvent::FileSearchResult { query, matches } => {
                self.chat_widget.apply_file_search_result(query, matches);
            }
            AppEvent::RateLimitSnapshotFetched(snapshot) => {
                self.chat_widget.on_rate_limit_snapshot(Some(snapshot));
            }
            AppEvent::ConnectorsLoaded(result) => {
                self.chat_widget.on_connectors_loaded(result);
            }
            AppEvent::UpdateReasoningEffort(effort) => {
                self.on_update_reasoning_effort(effort);
            }
            AppEvent::UpdateModel(model) => {
                self.chat_widget.set_model(&model);
            }
            AppEvent::UpdateCollaborationMode(mask) => {
                self.chat_widget.set_collaboration_mask(mask);
            }
            AppEvent::UpdatePersonality(personality) => {
                self.on_update_personality(personality);
            }
            AppEvent::OpenReasoningPopup { model } => {
                self.chat_widget.open_reasoning_popup(model);
            }
            AppEvent::OpenAllModelsPopup { models } => {
                self.chat_widget.open_all_models_popup(models);
            }
            AppEvent::OpenFullAccessConfirmation {
                preset,
                return_to_permissions,
            } => {
                self.chat_widget
                    .open_full_access_confirmation(preset, return_to_permissions);
            }
            AppEvent::OpenWorldWritableWarningConfirmation {
                preset,
                sample_paths,
                extra_count,
                failed_scan,
            } => {
                self.chat_widget.open_world_writable_warning_confirmation(
                    preset,
                    sample_paths,
                    extra_count,
                    failed_scan,
                );
            }
            AppEvent::OpenFeedbackNote {
                category,
                include_logs,
            } => {
                self.chat_widget.open_feedback_note(category, include_logs);
            }
            AppEvent::OpenFeedbackConsent { category } => {
                self.chat_widget.open_feedback_consent(category);
            }
            AppEvent::LaunchExternalEditor => {
                if self.chat_widget.external_editor_state() == ExternalEditorState::Active {
                    self.launch_external_editor(tui).await;
                }
            }
            AppEvent::OpenWindowsSandboxEnablePrompt { preset } => {
                self.chat_widget.open_windows_sandbox_enable_prompt(preset);
            }
            AppEvent::OpenWindowsSandboxFallbackPrompt { preset, reason } => {
                self.otel_manager
                    .counter("codex.windows_sandbox.fallback_prompt_shown", 1, &[]);
                self.chat_widget.clear_windows_sandbox_setup_status();
                if let Some(started_at) = self.windows_sandbox.setup_started_at.take() {
                    self.otel_manager.record_duration(
                        "codex.windows_sandbox.elevated_setup_duration_ms",
                        started_at.elapsed(),
                        &[("result", "failure")],
                    );
                }
                self.chat_widget
                    .open_windows_sandbox_fallback_prompt(preset, reason);
            }
            AppEvent::BeginWindowsSandboxElevatedSetup { preset } => {
                #[cfg(target_os = "windows")]
                {
                    let policy = preset.sandbox.clone();
                    let policy_cwd = self.config.cwd.clone();
                    let command_cwd = policy_cwd.clone();
                    let env_map: std::collections::HashMap<String, String> =
                        std::env::vars().collect();
                    let codex_home = self.config.codex_home.clone();
                    let tx = self.app_event_tx.clone();

                    // If the elevated setup already ran on this machine, don't prompt for
                    // elevation again - just flip the config to use the elevated path.
                    if codex_core::windows_sandbox::sandbox_setup_is_complete(codex_home.as_path())
                    {
                        tx.send(AppEvent::EnableWindowsSandboxForAgentMode {
                            preset,
                            mode: WindowsSandboxEnableMode::Elevated,
                        });
                        return Ok(AppRunControl::Continue);
                    }

                    self.chat_widget.show_windows_sandbox_setup_status();
                    self.windows_sandbox.setup_started_at = Some(Instant::now());
                    let otel_manager = self.otel_manager.clone();
                    tokio::task::spawn_blocking(move || {
                        let result = codex_core::windows_sandbox::run_elevated_setup(
                            &policy,
                            policy_cwd.as_path(),
                            command_cwd.as_path(),
                            &env_map,
                            codex_home.as_path(),
                        );
                        let event = match result {
                            Ok(()) => {
                                otel_manager.counter(
                                    "codex.windows_sandbox.elevated_setup_success",
                                    1,
                                    &[],
                                );
                                AppEvent::EnableWindowsSandboxForAgentMode {
                                    preset: preset.clone(),
                                    mode: WindowsSandboxEnableMode::Elevated,
                                }
                            }
                            Err(err) => {
                                let mut code_tag: Option<String> = None;
                                let mut message_tag: Option<String> = None;
                                if let Some((code, message)) =
                                    codex_core::windows_sandbox::elevated_setup_failure_details(
                                        &err,
                                    )
                                {
                                    code_tag = Some(code);
                                    message_tag = Some(message);
                                }
                                let mut tags: Vec<(&str, &str)> = Vec::new();
                                if let Some(code) = code_tag.as_deref() {
                                    tags.push(("code", code));
                                }
                                if let Some(message) = message_tag.as_deref() {
                                    tags.push(("message", message));
                                }
                                otel_manager.counter(
                                    "codex.windows_sandbox.elevated_setup_failure",
                                    1,
                                    &tags,
                                );
                                tracing::error!(
                                    error = %err,
                                    "failed to run elevated Windows sandbox setup"
                                );
                                AppEvent::OpenWindowsSandboxFallbackPrompt {
                                    preset,
                                    reason: WindowsSandboxFallbackReason::ElevationFailed,
                                }
                            }
                        };
                        tx.send(event);
                    });
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = preset;
                }
            }
            AppEvent::EnableWindowsSandboxForAgentMode { preset, mode } => {
                #[cfg(target_os = "windows")]
                {
                    self.chat_widget.clear_windows_sandbox_setup_status();
                    if let Some(started_at) = self.windows_sandbox.setup_started_at.take() {
                        self.otel_manager.record_duration(
                            "codex.windows_sandbox.elevated_setup_duration_ms",
                            started_at.elapsed(),
                            &[("result", "success")],
                        );
                    }
                    let profile = self.active_profile.as_deref();
                    let feature_key = Feature::WindowsSandbox.key();
                    let elevated_key = Feature::WindowsSandboxElevated.key();
                    let elevated_enabled = matches!(mode, WindowsSandboxEnableMode::Elevated);
                    let mut builder =
                        ConfigEditsBuilder::new(&self.config.codex_home).with_profile(profile);
                    if elevated_enabled {
                        builder = builder.set_feature_enabled(elevated_key, true);
                    } else {
                        builder = builder
                            .set_feature_enabled(feature_key, true)
                            .set_feature_enabled(elevated_key, false);
                    }
                    match builder.apply().await {
                        Ok(()) => {
                            if elevated_enabled {
                                self.config.set_windows_elevated_sandbox_enabled(true);
                                self.chat_widget
                                    .set_feature_enabled(Feature::WindowsSandboxElevated, true);
                            } else {
                                self.config.set_windows_sandbox_enabled(true);
                                self.config.set_windows_elevated_sandbox_enabled(false);
                                self.chat_widget
                                    .set_feature_enabled(Feature::WindowsSandbox, true);
                                self.chat_widget
                                    .set_feature_enabled(Feature::WindowsSandboxElevated, false);
                            }
                            self.chat_widget.clear_forced_auto_mode_downgrade();
                            if let Some((sample_paths, extra_count, failed_scan)) =
                                self.chat_widget.world_writable_warning_details()
                            {
                                self.app_event_tx.send(
                                    AppEvent::OpenWorldWritableWarningConfirmation {
                                        preset: Some(preset.clone()),
                                        sample_paths,
                                        extra_count,
                                        failed_scan,
                                    },
                                );
                            } else {
                                self.app_event_tx
                                    .send(AppEvent::UpdateAskForApprovalPolicy(preset.approval));
                                self.app_event_tx
                                    .send(AppEvent::UpdateSandboxPolicy(preset.sandbox.clone()));
                                self.chat_widget.add_info_message(
                                    match mode {
                                        WindowsSandboxEnableMode::Elevated => {
                                            "Enabled elevated agent sandbox.".to_string()
                                        }
                                        WindowsSandboxEnableMode::Legacy => {
                                            "Enabled non-elevated agent sandbox.".to_string()
                                        }
                                    },
                                    None,
                                );
                            }
                        }
                        Err(err) => {
                            tracing::error!(
                                error = %err,
                                "failed to enable Windows sandbox feature"
                            );
                            self.chat_widget.add_error_message(format!(
                                "Failed to enable the Windows sandbox feature: {err}"
                            ));
                        }
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = (preset, mode);
                }
            }
            AppEvent::PersistModelSelection { model, effort } => {
                let profile = self.active_profile.as_deref();
                match ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_profile(profile)
                    .set_model(Some(model.as_str()), effort)
                    .apply()
                    .await
                {
                    Ok(()) => {
                        let mut message = format!("Model changed to {model}");
                        if let Some(label) = Self::reasoning_label_for(&model, effort) {
                            message.push(' ');
                            message.push_str(label);
                        }
                        if let Some(profile) = profile {
                            message.push_str(" for ");
                            message.push_str(profile);
                            message.push_str(" profile");
                        }
                        self.chat_widget.add_info_message(message, None);
                    }
                    Err(err) => {
                        tracing::error!(
                            error = %err,
                            "failed to persist model selection"
                        );
                        if let Some(profile) = profile {
                            self.chat_widget.add_error_message(format!(
                                "Failed to save model for profile `{profile}`: {err}"
                            ));
                        } else {
                            self.chat_widget
                                .add_error_message(format!("Failed to save default model: {err}"));
                        }
                    }
                }
            }
            AppEvent::PersistPersonalitySelection { personality } => {
                let profile = self.active_profile.as_deref();
                match ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_profile(profile)
                    .set_model_personality(Some(personality))
                    .apply()
                    .await
                {
                    Ok(()) => {
                        let label = Self::personality_label(personality);
                        let mut message = format!("Personality set to {label}");
                        if let Some(profile) = profile {
                            message.push_str(" for ");
                            message.push_str(profile);
                            message.push_str(" profile");
                        }
                        self.chat_widget.add_info_message(message, None);
                    }
                    Err(err) => {
                        tracing::error!(
                            error = %err,
                            "failed to persist personality selection"
                        );
                        if let Some(profile) = profile {
                            self.chat_widget.add_error_message(format!(
                                "Failed to save personality for profile `{profile}`: {err}"
                            ));
                        } else {
                            self.chat_widget.add_error_message(format!(
                                "Failed to save default personality: {err}"
                            ));
                        }
                    }
                }
            }
            AppEvent::UpdateAskForApprovalPolicy(policy) => {
                self.runtime_approval_policy_override = Some(policy);
                if let Err(err) = self.config.approval_policy.set(policy) {
                    tracing::warn!(%err, "failed to set approval policy on app config");
                    self.chat_widget
                        .add_error_message(format!("Failed to set approval policy: {err}"));
                    return Ok(AppRunControl::Continue);
                }
                self.chat_widget.set_approval_policy(policy);
            }
            AppEvent::UpdateSandboxPolicy(policy) => {
                #[cfg(target_os = "windows")]
                let policy_is_workspace_write_or_ro = matches!(
                    &policy,
                    codex_core::protocol::SandboxPolicy::WorkspaceWrite { .. }
                        | codex_core::protocol::SandboxPolicy::ReadOnly
                );

                if let Err(err) = self.config.sandbox_policy.set(policy.clone()) {
                    tracing::warn!(%err, "failed to set sandbox policy on app config");
                    self.chat_widget
                        .add_error_message(format!("Failed to set sandbox policy: {err}"));
                    return Ok(AppRunControl::Continue);
                }
                #[cfg(target_os = "windows")]
                if !matches!(&policy, codex_core::protocol::SandboxPolicy::ReadOnly)
                    || WindowsSandboxLevel::from_config(&self.config)
                        != WindowsSandboxLevel::Disabled
                {
                    self.config.forced_auto_mode_downgraded_on_windows = false;
                }
                if let Err(err) = self.chat_widget.set_sandbox_policy(policy) {
                    tracing::warn!(%err, "failed to set sandbox policy on chat config");
                    self.chat_widget
                        .add_error_message(format!("Failed to set sandbox policy: {err}"));
                    return Ok(AppRunControl::Continue);
                }
                self.runtime_sandbox_policy_override =
                    Some(self.config.sandbox_policy.get().clone());

                // If sandbox policy becomes workspace-write or read-only, run the Windows world-writable scan.
                #[cfg(target_os = "windows")]
                {
                    // One-shot suppression if the user just confirmed continue.
                    if self.windows_sandbox.skip_world_writable_scan_once {
                        self.windows_sandbox.skip_world_writable_scan_once = false;
                        return Ok(AppRunControl::Continue);
                    }

                    let should_check = WindowsSandboxLevel::from_config(&self.config)
                        != WindowsSandboxLevel::Disabled
                        && policy_is_workspace_write_or_ro
                        && !self.chat_widget.world_writable_warning_hidden();
                    if should_check {
                        let cwd = self.config.cwd.clone();
                        let env_map: std::collections::HashMap<String, String> =
                            std::env::vars().collect();
                        let tx = self.app_event_tx.clone();
                        let logs_base_dir = self.config.codex_home.clone();
                        let sandbox_policy = self.config.sandbox_policy.get().clone();
                        Self::spawn_world_writable_scan(
                            cwd,
                            env_map,
                            logs_base_dir,
                            sandbox_policy,
                            tx,
                        );
                    }
                }
            }
            AppEvent::UpdateFeatureFlags { updates } => {
                if updates.is_empty() {
                    return Ok(AppRunControl::Continue);
                }
                let mut builder = ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_profile(self.active_profile.as_deref());
                for (feature, enabled) in &updates {
                    let feature_key = feature.key();
                    if *enabled {
                        // Update the in-memory configs.
                        self.config.features.enable(*feature);
                        self.chat_widget.set_feature_enabled(*feature, true);
                        builder = builder.set_feature_enabled(feature_key, true);
                    } else {
                        // Update the in-memory configs.
                        self.config.features.disable(*feature);
                        self.chat_widget.set_feature_enabled(*feature, false);
                        if feature.default_enabled() {
                            builder = builder.set_feature_enabled(feature_key, false);
                        } else {
                            // If the feature already default to `false`, we drop the key
                            // in the config file so that the user does not miss the feature
                            // once it gets globally released.
                            builder = builder.with_edits(vec![ConfigEdit::ClearPath {
                                segments: vec!["features".to_string(), feature_key.to_string()],
                            }]);
                        }
                    }
                }
                if let Err(err) = builder.apply().await {
                    tracing::error!(error = %err, "failed to persist feature flags");
                    self.chat_widget.add_error_message(format!(
                        "Failed to update experimental features: {err}"
                    ));
                }
            }
            AppEvent::SkipNextWorldWritableScan => {
                self.windows_sandbox.skip_world_writable_scan_once = true;
            }
            AppEvent::UpdateFullAccessWarningAcknowledged(ack) => {
                self.chat_widget.set_full_access_warning_acknowledged(ack);
            }
            AppEvent::UpdateWorldWritableWarningAcknowledged(ack) => {
                self.chat_widget
                    .set_world_writable_warning_acknowledged(ack);
            }
            AppEvent::UpdateRateLimitSwitchPromptHidden(hidden) => {
                self.chat_widget.set_rate_limit_switch_prompt_hidden(hidden);
            }
            AppEvent::PersistFullAccessWarningAcknowledged => {
                if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
                    .set_hide_full_access_warning(true)
                    .apply()
                    .await
                {
                    tracing::error!(
                        error = %err,
                        "failed to persist full access warning acknowledgement"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to save full access confirmation preference: {err}"
                    ));
                }
            }
            AppEvent::PersistWorldWritableWarningAcknowledged => {
                if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
                    .set_hide_world_writable_warning(true)
                    .apply()
                    .await
                {
                    tracing::error!(
                        error = %err,
                        "failed to persist world-writable warning acknowledgement"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to save Agent mode warning preference: {err}"
                    ));
                }
            }
            AppEvent::PersistRateLimitSwitchPromptHidden => {
                if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
                    .set_hide_rate_limit_model_nudge(true)
                    .apply()
                    .await
                {
                    tracing::error!(
                        error = %err,
                        "failed to persist rate limit switch prompt preference"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to save rate limit reminder preference: {err}"
                    ));
                }
            }
            AppEvent::PersistModelMigrationPromptAcknowledged {
                from_model,
                to_model,
            } => {
                if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
                    .record_model_migration_seen(from_model.as_str(), to_model.as_str())
                    .apply()
                    .await
                {
                    tracing::error!(
                        error = %err,
                        "failed to persist model migration prompt acknowledgement"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to save model migration prompt preference: {err}"
                    ));
                }
            }
            AppEvent::OpenApprovalsPopup => {
                self.chat_widget.open_approvals_popup();
            }
            AppEvent::OpenAgentPicker => {
                self.open_agent_picker();
            }
            AppEvent::SelectAgentThread(thread_id) => {
                self.select_agent_thread(tui, thread_id).await?;
            }
            AppEvent::OpenSkillsList => {
                self.chat_widget.open_skills_list();
            }
            AppEvent::OpenManageSkillsPopup => {
                self.chat_widget.open_manage_skills_popup();
            }
            AppEvent::SetSkillEnabled { path, enabled } => {
                let edits = [ConfigEdit::SetSkillConfig {
                    path: path.clone(),
                    enabled,
                }];
                match ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_edits(edits)
                    .apply()
                    .await
                {
                    Ok(()) => {
                        self.chat_widget.update_skill_enabled(path.clone(), enabled);
                    }
                    Err(err) => {
                        let path_display = path.display();
                        self.chat_widget.add_error_message(format!(
                            "Failed to update skill config for {path_display}: {err}"
                        ));
                    }
                }
            }
            AppEvent::OpenPermissionsPopup => {
                self.chat_widget.open_permissions_popup();
            }
            AppEvent::OpenReviewBranchPicker(cwd) => {
                self.chat_widget.show_review_branch_picker(&cwd).await;
            }
            AppEvent::OpenReviewCommitPicker(cwd) => {
                self.chat_widget.show_review_commit_picker(&cwd).await;
            }
            AppEvent::OpenReviewCustomPrompt => {
                self.chat_widget.show_review_custom_prompt();
            }
            AppEvent::SubmitUserMessageWithMode {
                text,
                collaboration_mode,
            } => {
                self.chat_widget
                    .submit_user_message_with_mode(text, collaboration_mode);
            }
            AppEvent::ManageSkillsClosed => {
                self.chat_widget.handle_manage_skills_closed();
            }
            AppEvent::FullScreenApprovalRequest(request) => match request {
                ApprovalRequest::ApplyPatch { cwd, changes, .. } => {
                    let _ = tui.enter_alt_screen();
                    let diff_summary = DiffSummary::new(changes, cwd);
                    self.overlay = Some(Overlay::new_static_with_renderables(
                        vec![diff_summary.into()],
                        "P A T C H".to_string(),
                    ));
                }
                ApprovalRequest::Exec { command, .. } => {
                    let _ = tui.enter_alt_screen();
                    let full_cmd = strip_bash_lc_and_escape(&command);
                    let full_cmd_lines = highlight_bash_to_lines(&full_cmd);
                    self.overlay = Some(Overlay::new_static_with_lines(
                        full_cmd_lines,
                        "E X E C".to_string(),
                    ));
                }
                ApprovalRequest::McpElicitation {
                    server_name,
                    message,
                    ..
                } => {
                    let _ = tui.enter_alt_screen();
                    let paragraph = Paragraph::new(vec![
                        Line::from(vec!["Server: ".into(), server_name.bold()]),
                        Line::from(""),
                        Line::from(message),
                    ])
                    .wrap(Wrap { trim: false });
                    self.overlay = Some(Overlay::new_static_with_renderables(
                        vec![Box::new(paragraph)],
                        "E L I C I T A T I O N".to_string(),
                    ));
                }
            },
        }
        Ok(AppRunControl::Continue)
    }

    async fn handle_app_server_action(&mut self, action: AppServerAction) -> Result<()> {
        match action {
            AppServerAction::TurnStart(request) => {
                let Some(thread_id) = self.chat_widget.thread_id() else {
                    return Ok(());
                };
                let params = codex_app_server_protocol::TurnStartParams {
                    thread_id: thread_id.to_string(),
                    input: request.items.into_iter().map(Into::into).collect(),
                    cwd: Some(request.cwd),
                    approval_policy: Some(codex_app_server_protocol::AskForApproval::from(
                        request.approval_policy,
                    )),
                    sandbox_policy: Some(request.sandbox_policy.into()),
                    windows_sandbox_level: Some(request.windows_sandbox_level),
                    model: Some(request.model),
                    effort: request.effort,
                    summary: request.summary,
                    personality: request.personality,
                    output_schema: request.output_schema,
                    collaboration_mode: request.collaboration_mode,
                };
                let pending = self
                    .app_server
                    .request(
                        |request_id| codex_app_server_protocol::ClientRequest::TurnStart {
                            request_id,
                            params,
                        },
                    )
                    .await
                    .map_err(app_server_error)?;
                pending.discard().await.map_err(app_server_error)?;
            }
            AppServerAction::ReviewStart { review_request } => {
                let Some(thread_id) = self.chat_widget.thread_id() else {
                    return Ok(());
                };
                let target = match review_request.target {
                    codex_protocol::protocol::ReviewTarget::UncommittedChanges => {
                        codex_app_server_protocol::ReviewTarget::UncommittedChanges
                    }
                    codex_protocol::protocol::ReviewTarget::BaseBranch { branch } => {
                        codex_app_server_protocol::ReviewTarget::BaseBranch { branch }
                    }
                    codex_protocol::protocol::ReviewTarget::Commit { sha, title } => {
                        codex_app_server_protocol::ReviewTarget::Commit { sha, title }
                    }
                    codex_protocol::protocol::ReviewTarget::Custom { instructions } => {
                        codex_app_server_protocol::ReviewTarget::Custom { instructions }
                    }
                };
                let params = codex_app_server_protocol::ReviewStartParams {
                    thread_id: thread_id.to_string(),
                    target,
                    delivery: None,
                };
                let pending = self
                    .app_server
                    .request(
                        |request_id| codex_app_server_protocol::ClientRequest::ReviewStart {
                            request_id,
                            params,
                        },
                    )
                    .await
                    .map_err(app_server_error)?;
                pending.discard().await.map_err(app_server_error)?;
            }
            AppServerAction::Interrupt => {
                if let Some(thread_id) = self.chat_widget.thread_id() {
                    self.app_server
                        .interrupt_current_turn(thread_id)
                        .await
                        .map_err(app_server_error)?;
                }
            }
            AppServerAction::Shutdown => {
                if let Some(thread_id) = self.chat_widget.thread_id() {
                    let params = codex_app_server_protocol::ThreadShutdownParams {
                        thread_id: thread_id.to_string(),
                    };
                    let pending = self
                        .app_server
                        .request(|request_id| {
                            codex_app_server_protocol::ClientRequest::ThreadShutdown {
                                request_id,
                                params,
                            }
                        })
                        .await
                        .map_err(app_server_error)?;
                    pending.discard().await.map_err(app_server_error)?;
                }
            }
            AppServerAction::Compact => {
                if let Some(thread_id) = self.chat_widget.thread_id() {
                    let params = codex_app_server_protocol::ThreadCompactParams {
                        thread_id: thread_id.to_string(),
                    };
                    let pending = self
                        .app_server
                        .request(|request_id| {
                            codex_app_server_protocol::ClientRequest::ThreadCompact {
                                request_id,
                                params,
                            }
                        })
                        .await
                        .map_err(app_server_error)?;
                    pending.discard().await.map_err(app_server_error)?;
                }
            }
            AppServerAction::ThreadRollback { num_turns } => {
                if let Some(thread_id) = self.chat_widget.thread_id() {
                    let params = codex_app_server_protocol::ThreadRollbackParams {
                        thread_id: thread_id.to_string(),
                        num_turns,
                    };
                    let pending = self
                        .app_server
                        .request(|request_id| {
                            codex_app_server_protocol::ClientRequest::ThreadRollback {
                                request_id,
                                params,
                            }
                        })
                        .await
                        .map_err(app_server_error)?;
                    pending.discard().await.map_err(app_server_error)?;
                }
            }
            AppServerAction::ListSkills { cwds, force_reload } => {
                let params = codex_app_server_protocol::SkillsListParams { cwds, force_reload };
                let pending = self
                    .app_server
                    .request(
                        |request_id| codex_app_server_protocol::ClientRequest::SkillsList {
                            request_id,
                            params,
                        },
                    )
                    .await
                    .map_err(app_server_error)?;
                let response: codex_app_server_protocol::SkillsListResponse =
                    pending.into_typed().await.map_err(app_server_error)?;
                let skills = response
                    .data
                    .into_iter()
                    .map(map_skills_list_entry)
                    .collect();
                let event = Event {
                    id: String::new(),
                    msg: EventMsg::ListSkillsResponse(ListSkillsResponseEvent { skills }),
                };
                if let Some(thread_id) = self.chat_widget.thread_id() {
                    self.enqueue_thread_event(thread_id, event).await?;
                } else {
                    self.enqueue_primary_event(event).await?;
                }
            }
            AppServerAction::RefreshMcpServers { config: _ } => {
                let pending = self
                    .app_server
                    .request(|request_id| {
                        codex_app_server_protocol::ClientRequest::McpServerRefresh {
                            request_id,
                            params: None,
                        }
                    })
                    .await
                    .map_err(app_server_error)?;
                pending.discard().await.map_err(app_server_error)?;
            }
            AppServerAction::ListMcpTools => {
                let params = codex_app_server_protocol::ListMcpServerStatusParams {
                    cursor: None,
                    limit: None,
                };
                let pending = self
                    .app_server
                    .request(|request_id| {
                        codex_app_server_protocol::ClientRequest::McpServerStatusList {
                            request_id,
                            params,
                        }
                    })
                    .await
                    .map_err(app_server_error)?;
                let response: codex_app_server_protocol::ListMcpServerStatusResponse =
                    pending.into_typed().await.map_err(app_server_error)?;
                let mut tools = HashMap::new();
                let mut resources = HashMap::new();
                let mut resource_templates = HashMap::new();
                let mut auth_statuses = HashMap::new();
                for server in response.data {
                    let server_name = server.name.clone();
                    auth_statuses.insert(server_name.clone(), server.auth_status.to_core());
                    resources.insert(server_name.clone(), server.resources);
                    resource_templates.insert(server_name.clone(), server.resource_templates);
                    for (tool_name, tool) in server.tools {
                        let qualified = format!("mcp__{server_name}__{tool_name}");
                        tools.insert(qualified, tool);
                    }
                }
                let event = Event {
                    id: String::new(),
                    msg: EventMsg::McpListToolsResponse(
                        codex_core::protocol::McpListToolsResponseEvent {
                            tools,
                            resources,
                            resource_templates,
                            auth_statuses,
                        },
                    ),
                };
                if let Some(thread_id) = self.chat_widget.thread_id() {
                    self.enqueue_thread_event(thread_id, event).await?;
                } else {
                    self.enqueue_primary_event(event).await?;
                }
            }
            AppServerAction::ListCustomPrompts => {
                let custom_prompts =
                    if let Some(dir) = codex_core::custom_prompts::default_prompts_dir() {
                        codex_core::custom_prompts::discover_prompts_in(&dir).await
                    } else {
                        Vec::new()
                    };
                let event = Event {
                    id: String::new(),
                    msg: EventMsg::ListCustomPromptsResponse(
                        codex_core::protocol::ListCustomPromptsResponseEvent { custom_prompts },
                    ),
                };
                if let Some(thread_id) = self.chat_widget.thread_id() {
                    self.enqueue_thread_event(thread_id, event).await?;
                } else {
                    self.enqueue_primary_event(event).await?;
                }
            }
            AppServerAction::RunUserShellCommand { command } => {
                let Some(thread_id) = self.chat_widget.thread_id() else {
                    return Ok(());
                };
                let shell = codex_core::shell::default_user_shell();
                let command_args = shell.derive_exec_args(&command, true);
                let parsed_cmd = codex_core::parse_command::parse_command(&command_args);
                let call_id = uuid::Uuid::new_v4().to_string();
                let turn_id = uuid::Uuid::new_v4().to_string();
                let cwd = self.config.cwd.clone();
                let begin_event = Event {
                    id: String::new(),
                    msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                        call_id: call_id.clone(),
                        process_id: None,
                        turn_id: turn_id.clone(),
                        command: command_args.clone(),
                        cwd: cwd.clone(),
                        parsed_cmd: parsed_cmd.clone(),
                        source: ExecCommandSource::UserShell,
                        interaction_input: None,
                    }),
                };
                self.enqueue_thread_event(thread_id, begin_event).await?;

                let started_at = Instant::now();
                let params = codex_app_server_protocol::CommandExecParams {
                    command: command_args.clone(),
                    timeout_ms: None,
                    cwd: Some(cwd.clone()),
                    sandbox_policy: Some(self.config.sandbox_policy.get().clone().into()),
                };
                let pending = self
                    .app_server
                    .request(|request_id| {
                        codex_app_server_protocol::ClientRequest::OneOffCommandExec {
                            request_id,
                            params,
                        }
                    })
                    .await
                    .map_err(app_server_error)?;
                let response: codex_app_server_protocol::CommandExecResponse =
                    pending.into_typed().await.map_err(app_server_error)?;
                let duration = started_at.elapsed();
                if !response.stdout.is_empty() {
                    let delta = Event {
                        id: String::new(),
                        msg: EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                            call_id: call_id.clone(),
                            stream: ExecOutputStream::Stdout,
                            chunk: response.stdout.as_bytes().to_vec(),
                        }),
                    };
                    self.enqueue_thread_event(thread_id, delta).await?;
                }
                if !response.stderr.is_empty() {
                    let delta = Event {
                        id: String::new(),
                        msg: EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                            call_id: call_id.clone(),
                            stream: ExecOutputStream::Stderr,
                            chunk: response.stderr.as_bytes().to_vec(),
                        }),
                    };
                    self.enqueue_thread_event(thread_id, delta).await?;
                }
                let aggregated_output = format!("{}{}", response.stdout, response.stderr);
                let exec_output = codex_core::exec::ExecToolCallOutput {
                    exit_code: response.exit_code,
                    stdout: codex_core::exec::StreamOutput::new(response.stdout.clone()),
                    stderr: codex_core::exec::StreamOutput::new(response.stderr.clone()),
                    aggregated_output: codex_core::exec::StreamOutput::new(
                        aggregated_output.clone(),
                    ),
                    duration,
                    timed_out: false,
                };
                let formatted_output = codex_core::format_exec_output_str(
                    &exec_output,
                    codex_core::TruncationPolicy::Bytes(10_000),
                );
                let end_event = Event {
                    id: String::new(),
                    msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                        call_id,
                        process_id: None,
                        turn_id,
                        command: command_args,
                        cwd,
                        parsed_cmd,
                        source: ExecCommandSource::UserShell,
                        interaction_input: None,
                        stdout: response.stdout,
                        stderr: response.stderr,
                        aggregated_output,
                        exit_code: response.exit_code,
                        duration,
                        formatted_output,
                    }),
                };
                self.enqueue_thread_event(thread_id, end_event).await?;
            }
            AppServerAction::AddToHistory { text } => {
                if let Some(thread_id) = self.chat_widget.thread_id()
                    && let Err(err) =
                        codex_core::message_history::append_entry(&text, &thread_id, &self.config)
                            .await
                {
                    tracing::warn!(error = %err, "failed to append history entry");
                }
            }
            AppServerAction::GetHistoryEntry { log_id, offset } => {
                let entry = codex_core::message_history::lookup(log_id, offset, &self.config).map(
                    |entry| codex_protocol::message_history::HistoryEntry {
                        conversation_id: entry.session_id,
                        ts: entry.ts,
                        text: entry.text,
                    },
                );
                let event = Event {
                    id: String::new(),
                    msg: EventMsg::GetHistoryEntryResponse(
                        codex_core::protocol::GetHistoryEntryResponseEvent {
                            offset,
                            log_id,
                            entry,
                        },
                    ),
                };
                self.enqueue_primary_event(event).await?;
            }
            AppServerAction::ExecApproval { call_id, decision } => {
                self.app_server
                    .respond_exec_approval(call_id, decision)
                    .await
                    .map_err(app_server_error)?;
            }
            AppServerAction::PatchApproval { call_id, decision } => {
                self.app_server
                    .respond_patch_approval(call_id, decision)
                    .await
                    .map_err(app_server_error)?;
            }
            AppServerAction::UserInputAnswer { call_id, response } => {
                self.app_server
                    .respond_user_input(call_id, response)
                    .await
                    .map_err(app_server_error)?;
            }
            AppServerAction::ResolveElicitation {
                server_name,
                request_id,
                decision,
            } => {
                self.app_server
                    .respond_elicitation(server_name, request_id, decision)
                    .await
                    .map_err(app_server_error)?;
            }
        }
        Ok(())
    }

    fn handle_codex_event_now(&mut self, event: Event) {
        if self.suppress_shutdown_complete && matches!(event.msg, EventMsg::ShutdownComplete) {
            self.suppress_shutdown_complete = false;
            return;
        }
        if let EventMsg::ListSkillsResponse(response) = &event.msg {
            let cwd = self.chat_widget.config_ref().cwd.clone();
            let errors = errors_for_cwd(&cwd, response);
            emit_skill_load_warnings(&self.app_event_tx, &errors);
        }
        self.handle_backtrack_event(&event.msg);
        self.chat_widget.handle_codex_event(event);
    }

    fn handle_codex_event_replay(&mut self, event: Event) {
        self.handle_backtrack_event(&event.msg);
        self.chat_widget.handle_codex_event_replay(event);
    }

    fn handle_active_thread_event(&mut self, tui: &mut tui::Tui, event: Event) -> Result<()> {
        self.handle_codex_event_now(event);
        if self.backtrack_render_pending {
            tui.frame_requester().schedule_frame();
        }
        Ok(())
    }

    fn reasoning_label(reasoning_effort: Option<ReasoningEffortConfig>) -> &'static str {
        match reasoning_effort {
            Some(ReasoningEffortConfig::Minimal) => "minimal",
            Some(ReasoningEffortConfig::Low) => "low",
            Some(ReasoningEffortConfig::Medium) => "medium",
            Some(ReasoningEffortConfig::High) => "high",
            Some(ReasoningEffortConfig::XHigh) => "xhigh",
            None | Some(ReasoningEffortConfig::None) => "default",
        }
    }

    fn reasoning_label_for(
        model: &str,
        reasoning_effort: Option<ReasoningEffortConfig>,
    ) -> Option<&'static str> {
        (!model.starts_with("codex-auto-")).then(|| Self::reasoning_label(reasoning_effort))
    }

    pub(crate) fn token_usage(&self) -> codex_core::protocol::TokenUsage {
        self.chat_widget.token_usage()
    }

    fn on_update_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        // TODO(aibrahim): Remove this and don't use config as a state object.
        // Instead, explicitly pass the stored collaboration mode's effort into new sessions.
        self.config.model_reasoning_effort = effort;
        self.chat_widget.set_reasoning_effort(effort);
    }

    fn on_update_personality(&mut self, personality: Personality) {
        self.config.model_personality = Some(personality);
        self.chat_widget.set_personality(personality);
    }

    fn personality_label(personality: Personality) -> &'static str {
        match personality {
            Personality::Friendly => "Friendly",
            Personality::Pragmatic => "Pragmatic",
        }
    }

    async fn launch_external_editor(&mut self, tui: &mut tui::Tui) {
        let editor_cmd = match external_editor::resolve_editor_command() {
            Ok(cmd) => cmd,
            Err(external_editor::EditorError::MissingEditor) => {
                self.chat_widget
                    .add_to_history(history_cell::new_error_event(
                    "Cannot open external editor: set $VISUAL or $EDITOR before starting Codex."
                        .to_string(),
                ));
                self.reset_external_editor_state(tui);
                return;
            }
            Err(err) => {
                self.chat_widget
                    .add_to_history(history_cell::new_error_event(format!(
                        "Failed to open editor: {err}",
                    )));
                self.reset_external_editor_state(tui);
                return;
            }
        };

        let seed = self.chat_widget.composer_text_with_pending();
        let editor_result = tui
            .with_restored(tui::RestoreMode::KeepRaw, || async {
                external_editor::run_editor(&seed, &editor_cmd).await
            })
            .await;
        self.reset_external_editor_state(tui);

        match editor_result {
            Ok(new_text) => {
                // Trim trailing whitespace
                let cleaned = new_text.trim_end().to_string();
                self.chat_widget.apply_external_edit(cleaned);
            }
            Err(err) => {
                self.chat_widget
                    .add_to_history(history_cell::new_error_event(format!(
                        "Failed to open editor: {err}",
                    )));
            }
        }
        tui.frame_requester().schedule_frame();
    }

    fn request_external_editor_launch(&mut self, tui: &mut tui::Tui) {
        self.chat_widget
            .set_external_editor_state(ExternalEditorState::Requested);
        self.chat_widget.set_footer_hint_override(Some(vec![(
            EXTERNAL_EDITOR_HINT.to_string(),
            String::new(),
        )]));
        tui.frame_requester().schedule_frame();
    }

    fn reset_external_editor_state(&mut self, tui: &mut tui::Tui) {
        self.chat_widget
            .set_external_editor_state(ExternalEditorState::Closed);
        self.chat_widget.set_footer_hint_override(None);
        tui.frame_requester().schedule_frame();
    }

    async fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } => {
                // Enter alternate screen and set viewport to full size.
                let _ = tui.enter_alt_screen();
                self.overlay = Some(Overlay::new_transcript(self.transcript_cells.clone()));
                tui.frame_requester().schedule_frame();
            }
            KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } => {
                // Only launch the external editor if there is no overlay and the bottom pane is not in use.
                // Note that it can be launched while a task is running to enable editing while the previous turn is ongoing.
                if self.overlay.is_none()
                    && self.chat_widget.can_launch_external_editor()
                    && self.chat_widget.external_editor_state() == ExternalEditorState::Closed
                {
                    self.request_external_editor_launch(tui);
                }
            }
            // Esc primes/advances backtracking only in normal (not working) mode
            // with the composer focused and empty. In any other state, forward
            // Esc so the active UI (e.g. status indicator, modals, popups)
            // handles it.
            KeyEvent {
                code: KeyCode::Esc,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                if self.chat_widget.is_normal_backtrack_mode()
                    && self.chat_widget.composer_is_empty()
                {
                    self.handle_backtrack_esc_key(tui);
                } else {
                    self.chat_widget.handle_key_event(key_event);
                }
            }
            // Enter confirms backtrack when primed + count > 0. Otherwise pass to widget.
            KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            } if self.backtrack.primed
                && self.backtrack.nth_user_message != usize::MAX
                && self.chat_widget.composer_is_empty() =>
            {
                if let Some(selection) = self.confirm_backtrack_from_main() {
                    self.apply_backtrack_selection(tui, selection);
                }
            }
            KeyEvent {
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                // Any non-Esc key press should cancel a primed backtrack.
                // This avoids stale "Esc-primed" state after the user starts typing
                // (even if they later backspace to empty).
                if key_event.code != KeyCode::Esc && self.backtrack.primed {
                    self.reset_backtrack_state();
                }
                self.chat_widget.handle_key_event(key_event);
            }
            _ => {
                // Ignore Release key events.
            }
        };
    }

    #[cfg(target_os = "windows")]
    fn spawn_world_writable_scan(
        cwd: PathBuf,
        env_map: std::collections::HashMap<String, String>,
        logs_base_dir: PathBuf,
        sandbox_policy: codex_core::protocol::SandboxPolicy,
        tx: AppEventSender,
    ) {
        tokio::task::spawn_blocking(move || {
            let result = codex_windows_sandbox::apply_world_writable_scan_and_denies(
                &logs_base_dir,
                &cwd,
                &env_map,
                &sandbox_policy,
                Some(logs_base_dir.as_path()),
            );
            if result.is_err() {
                // Scan failed: warn without examples.
                tx.send(AppEvent::OpenWorldWritableWarningConfirmation {
                    preset: None,
                    sample_paths: Vec::new(),
                    extra_count: 0usize,
                    failed_scan: true,
                });
            }
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn session_configured_from_thread_response(
    thread_id: ThreadId,
    model: String,
    model_provider: String,
    approval_policy: codex_app_server_protocol::AskForApproval,
    sandbox: codex_app_server_protocol::SandboxPolicy,
    cwd: PathBuf,
    reasoning_effort: Option<ReasoningEffortConfig>,
    rollout_path: Option<PathBuf>,
    initial_messages: Option<Vec<EventMsg>>,
) -> Event {
    Event {
        id: String::new(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: thread_id,
            forked_from_id: None,
            model,
            model_provider_id: model_provider,
            approval_policy: approval_policy.to_core(),
            sandbox_policy: sandbox.to_core(),
            cwd,
            reasoning_effort,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages,
            rollout_path,
        }),
    }
}

fn thread_turns_to_initial_messages(
    turns: &[codex_app_server_protocol::Turn],
    show_raw_agent_reasoning: bool,
) -> Option<Vec<EventMsg>> {
    let mut events = Vec::new();
    for turn in turns {
        for item in &turn.items {
            if let Some(turn_item) = thread_item_to_turn_item(item) {
                events.extend(turn_item.as_legacy_events(show_raw_agent_reasoning));
            }
        }
    }
    (!events.is_empty()).then_some(events)
}

fn thread_item_to_turn_item(item: &V2ThreadItem) -> Option<TurnItem> {
    match item {
        V2ThreadItem::UserMessage { id, content } => {
            let content = content
                .iter()
                .cloned()
                .map(V2UserInput::into_core)
                .collect();
            Some(TurnItem::UserMessage(UserMessageItem {
                id: id.clone(),
                content,
            }))
        }
        V2ThreadItem::AgentMessage { id, text } => Some(TurnItem::AgentMessage(AgentMessageItem {
            id: id.clone(),
            content: vec![AgentMessageContent::Text { text: text.clone() }],
        })),
        V2ThreadItem::Reasoning {
            id,
            summary,
            content,
        } => Some(TurnItem::Reasoning(ReasoningItem {
            id: id.clone(),
            summary_text: summary.clone(),
            raw_content: content.clone(),
        })),
        V2ThreadItem::ContextCompaction { id } => {
            Some(TurnItem::ContextCompaction(ContextCompactionItem {
                id: id.clone(),
            }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_backtrack::BacktrackState;
    use crate::app_backtrack::user_count;
    use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
    use crate::file_search::FileSearchManager;
    use crate::history_cell::AgentMessageCell;
    use crate::history_cell::HistoryCell;
    use crate::history_cell::UserHistoryCell;
    use crate::history_cell::new_session_info;
    use codex_core::AuthManager;
    use codex_core::CodexAuth;
    use codex_core::config::ConfigBuilder;
    use codex_core::config::ConfigOverrides;
    use codex_core::config_loader::LoaderOverrides;
    use codex_core::models_manager::manager::ModelsManager;
    use codex_core::protocol::AskForApproval;
    use codex_core::protocol::Event;
    use codex_core::protocol::EventMsg;
    use codex_core::protocol::SandboxPolicy;
    use codex_core::protocol::SessionConfiguredEvent;
    use codex_core::protocol::SessionSource;
    use codex_otel::OtelManager;
    use codex_protocol::ThreadId;
    use codex_protocol::user_input::TextElement;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::prelude::Line;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tempfile::tempdir;
    use tokio::time;

    #[test]
    fn normalize_harness_overrides_resolves_relative_add_dirs() -> Result<()> {
        let temp_dir = tempdir()?;
        let base_cwd = temp_dir.path().join("base");
        std::fs::create_dir_all(&base_cwd)?;

        let overrides = ConfigOverrides {
            additional_writable_roots: vec![PathBuf::from("rel")],
            ..Default::default()
        };
        let normalized = normalize_harness_overrides_for_cwd(overrides, &base_cwd)?;

        assert_eq!(
            normalized.additional_writable_roots,
            vec![base_cwd.join("rel")]
        );
        Ok(())
    }

    #[tokio::test]
    async fn enqueue_thread_event_does_not_block_when_channel_full() -> Result<()> {
        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        app.thread_event_channels
            .insert(thread_id, ThreadEventChannel::new(1));
        app.set_thread_active(thread_id, true).await;

        let event = Event {
            id: String::new(),
            msg: EventMsg::ShutdownComplete,
        };

        app.enqueue_thread_event(thread_id, event.clone()).await?;
        time::timeout(
            Duration::from_millis(50),
            app.enqueue_thread_event(thread_id, event),
        )
        .await
        .expect("enqueue_thread_event blocked on a full channel")?;

        let mut rx = app
            .thread_event_channels
            .get_mut(&thread_id)
            .expect("missing thread channel")
            .receiver
            .take()
            .expect("missing receiver");

        time::timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("timed out waiting for first event")
            .expect("channel closed unexpectedly");
        time::timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("timed out waiting for second event")
            .expect("channel closed unexpectedly");

        Ok(())
    }

    async fn make_test_app() -> App {
        let (chat_widget, app_event_tx, _rx) = make_chatwidget_manual_with_sender().await;
        let config = chat_widget.config_ref().clone();
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
        let models_manager = Arc::new(ModelsManager::new(
            config.codex_home.clone(),
            auth_manager.clone(),
        ));
        let feedback = codex_feedback::CodexFeedback::new();
        let app_server = AppServerClient::spawn(
            app_event_tx.clone(),
            Arc::new(config.clone()),
            Vec::new(),
            LoaderOverrides::default(),
            feedback.clone(),
            Vec::new(),
            SessionSource::Cli,
        );
        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());
        let model = ModelsManager::get_model_offline(config.model.as_deref());
        let otel_manager = test_otel_manager(&config, model.as_str());

        App {
            app_server,
            models_manager,
            otel_manager,
            app_event_tx,
            chat_widget,
            auth_manager,
            config,
            active_profile: None,
            cli_kv_overrides: Vec::new(),
            harness_overrides: ConfigOverrides::default(),
            runtime_approval_policy_override: None,
            runtime_sandbox_policy_override: None,
            file_search,
            transcript_cells: Vec::new(),
            overlay: None,
            deferred_history_lines: Vec::new(),
            has_emitted_history_lines: false,
            enhanced_keys_supported: false,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            backtrack: BacktrackState::default(),
            backtrack_render_pending: false,
            feedback,
            feedback_audience: FeedbackAudience::External,
            pending_update_action: None,
            suppress_shutdown_complete: false,
            windows_sandbox: WindowsSandboxState::default(),
            thread_event_channels: HashMap::new(),
            active_thread_id: None,
            active_thread_rx: None,
            primary_thread_id: None,
            primary_session_configured: None,
            pending_primary_events: VecDeque::new(),
        }
    }

    async fn make_test_app_with_channels() -> (App, tokio::sync::mpsc::UnboundedReceiver<AppEvent>)
    {
        let (chat_widget, app_event_tx, rx) = make_chatwidget_manual_with_sender().await;
        let config = chat_widget.config_ref().clone();
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
        let models_manager = Arc::new(ModelsManager::new(
            config.codex_home.clone(),
            auth_manager.clone(),
        ));
        let feedback = codex_feedback::CodexFeedback::new();
        let app_server = AppServerClient::spawn(
            app_event_tx.clone(),
            Arc::new(config.clone()),
            Vec::new(),
            LoaderOverrides::default(),
            feedback.clone(),
            Vec::new(),
            SessionSource::Cli,
        );
        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());
        let model = ModelsManager::get_model_offline(config.model.as_deref());
        let otel_manager = test_otel_manager(&config, model.as_str());

        (
            App {
                app_server,
                models_manager,
                otel_manager,
                app_event_tx,
                chat_widget,
                auth_manager,
                config,
                active_profile: None,
                cli_kv_overrides: Vec::new(),
                harness_overrides: ConfigOverrides::default(),
                runtime_approval_policy_override: None,
                runtime_sandbox_policy_override: None,
                file_search,
                transcript_cells: Vec::new(),
                overlay: None,
                deferred_history_lines: Vec::new(),
                has_emitted_history_lines: false,
                enhanced_keys_supported: false,
                commit_anim_running: Arc::new(AtomicBool::new(false)),
                backtrack: BacktrackState::default(),
                backtrack_render_pending: false,
                feedback,
                feedback_audience: FeedbackAudience::External,
                pending_update_action: None,
                suppress_shutdown_complete: false,
                windows_sandbox: WindowsSandboxState::default(),
                thread_event_channels: HashMap::new(),
                active_thread_id: None,
                active_thread_rx: None,
                primary_thread_id: None,
                primary_session_configured: None,
                pending_primary_events: VecDeque::new(),
            },
            rx,
        )
    }

    fn test_otel_manager(config: &Config, model: &str) -> OtelManager {
        let model_info = ModelsManager::construct_model_info_offline(model, config);
        OtelManager::new(
            ThreadId::new(),
            model,
            model_info.slug.as_str(),
            None,
            None,
            None,
            false,
            "test".to_string(),
            SessionSource::Cli,
        )
    }

    fn all_model_presets() -> Vec<ModelPreset> {
        codex_core::models_manager::model_presets::all_model_presets().clone()
    }

    fn model_migration_copy_to_plain_text(
        copy: &crate::model_migration::ModelMigrationCopy,
    ) -> String {
        if let Some(markdown) = copy.markdown.as_ref() {
            return markdown.clone();
        }
        let mut s = String::new();
        for span in &copy.heading {
            s.push_str(&span.content);
        }
        s.push('\n');
        s.push('\n');
        for line in &copy.content {
            for span in &line.spans {
                s.push_str(&span.content);
            }
            s.push('\n');
        }
        s
    }

    #[tokio::test]
    async fn model_migration_prompt_only_shows_for_deprecated_models() {
        let seen = BTreeMap::new();
        assert!(should_show_model_migration_prompt(
            "gpt-5",
            "gpt-5.1",
            &seen,
            &all_model_presets()
        ));
        assert!(should_show_model_migration_prompt(
            "gpt-5-codex",
            "gpt-5.1-codex",
            &seen,
            &all_model_presets()
        ));
        assert!(should_show_model_migration_prompt(
            "gpt-5-codex-mini",
            "gpt-5.1-codex-mini",
            &seen,
            &all_model_presets()
        ));
        assert!(should_show_model_migration_prompt(
            "gpt-5.1-codex",
            "gpt-5.1-codex-max",
            &seen,
            &all_model_presets()
        ));
        assert!(!should_show_model_migration_prompt(
            "gpt-5.1-codex",
            "gpt-5.1-codex",
            &seen,
            &all_model_presets()
        ));
    }

    #[tokio::test]
    async fn model_migration_prompt_respects_hide_flag_and_self_target() {
        let mut seen = BTreeMap::new();
        seen.insert("gpt-5".to_string(), "gpt-5.1".to_string());
        assert!(!should_show_model_migration_prompt(
            "gpt-5",
            "gpt-5.1",
            &seen,
            &all_model_presets()
        ));
        assert!(!should_show_model_migration_prompt(
            "gpt-5.1",
            "gpt-5.1",
            &seen,
            &all_model_presets()
        ));
    }

    #[tokio::test]
    async fn model_migration_prompt_skips_when_target_missing() {
        let mut available = all_model_presets();
        let mut current = available
            .iter()
            .find(|preset| preset.model == "gpt-5-codex")
            .cloned()
            .expect("preset present");
        current.upgrade = Some(ModelUpgrade {
            id: "missing-target".to_string(),
            reasoning_effort_mapping: None,
            migration_config_key: HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG.to_string(),
            model_link: None,
            upgrade_copy: None,
            migration_markdown: None,
        });
        available.retain(|preset| preset.model != "gpt-5-codex");
        available.push(current.clone());

        assert!(should_show_model_migration_prompt(
            &current.model,
            "missing-target",
            &BTreeMap::new(),
            &available,
        ));

        assert!(target_preset_for_upgrade(&available, "missing-target").is_none());
    }

    #[tokio::test]
    async fn model_migration_prompt_shows_for_hidden_model() {
        let codex_home = tempdir().expect("temp codex home");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config");

        let available_models = all_model_presets();
        let current = available_models
            .iter()
            .find(|preset| preset.model == "gpt-5.1-codex")
            .cloned()
            .expect("gpt-5.1-codex preset present");
        assert!(
            !current.show_in_picker,
            "expected gpt-5.1-codex to be hidden from picker for this test"
        );

        let upgrade = current.upgrade.as_ref().expect("upgrade configured");
        assert!(
            should_show_model_migration_prompt(
                &current.model,
                &upgrade.id,
                &config.notices.model_migrations,
                &available_models,
            ),
            "expected migration prompt to be eligible for hidden model"
        );

        let target = target_preset_for_upgrade(&available_models, &upgrade.id)
            .expect("upgrade target present");
        let target_description =
            (!target.description.is_empty()).then(|| target.description.clone());
        let can_opt_out = true;
        let copy = migration_copy_for_models(
            &current.model,
            &upgrade.id,
            upgrade.model_link.clone(),
            upgrade.upgrade_copy.clone(),
            upgrade.migration_markdown.clone(),
            target.display_name.clone(),
            target_description,
            can_opt_out,
        );

        // Snapshot the copy we would show; rendering is covered by model_migration snapshots.
        assert_snapshot!(
            "model_migration_prompt_shows_for_hidden_model",
            model_migration_copy_to_plain_text(&copy)
        );
    }

    #[tokio::test]
    async fn update_reasoning_effort_updates_collaboration_mode() {
        let mut app = make_test_app().await;
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::Medium));

        app.on_update_reasoning_effort(Some(ReasoningEffortConfig::High));

        assert_eq!(
            app.chat_widget.current_reasoning_effort(),
            Some(ReasoningEffortConfig::High)
        );
        assert_eq!(
            app.config.model_reasoning_effort,
            Some(ReasoningEffortConfig::High)
        );
    }

    #[tokio::test]
    async fn backtrack_selection_with_duplicate_history_targets_unique_turn() {
        let (mut app, mut app_event_rx) = make_test_app_with_channels().await;

        let user_cell = |text: &str,
                         text_elements: Vec<TextElement>,
                         local_image_paths: Vec<PathBuf>|
         -> Arc<dyn HistoryCell> {
            Arc::new(UserHistoryCell {
                message: text.to_string(),
                text_elements,
                local_image_paths,
            }) as Arc<dyn HistoryCell>
        };
        let agent_cell = |text: &str| -> Arc<dyn HistoryCell> {
            Arc::new(AgentMessageCell::new(
                vec![Line::from(text.to_string())],
                true,
            )) as Arc<dyn HistoryCell>
        };

        let make_header = |is_first| {
            let event = SessionConfiguredEvent {
                session_id: ThreadId::new(),
                forked_from_id: None,
                model: "gpt-test".to_string(),
                model_provider_id: "test-provider".to_string(),
                approval_policy: AskForApproval::Never,
                sandbox_policy: SandboxPolicy::ReadOnly,
                cwd: PathBuf::from("/home/user/project"),
                reasoning_effort: None,
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                rollout_path: Some(PathBuf::new()),
            };
            Arc::new(new_session_info(
                app.chat_widget.config_ref(),
                app.chat_widget.current_model(),
                event,
                is_first,
            )) as Arc<dyn HistoryCell>
        };

        let placeholder = "[Image #1]";
        let edited_text = format!("follow-up (edited) {placeholder}");
        let edited_range = edited_text.len().saturating_sub(placeholder.len())..edited_text.len();
        let edited_text_elements = vec![TextElement::new(edited_range.into(), None)];
        let edited_local_image_paths = vec![PathBuf::from("/tmp/fake-image.png")];

        // Simulate a transcript with duplicated history (e.g., from prior backtracks)
        // and an edited turn appended after a session header boundary.
        app.transcript_cells = vec![
            make_header(true),
            user_cell("first question", Vec::new(), Vec::new()),
            agent_cell("answer first"),
            user_cell("follow-up", Vec::new(), Vec::new()),
            agent_cell("answer follow-up"),
            make_header(false),
            user_cell("first question", Vec::new(), Vec::new()),
            agent_cell("answer first"),
            user_cell(
                &edited_text,
                edited_text_elements.clone(),
                edited_local_image_paths.clone(),
            ),
            agent_cell("answer edited"),
        ];

        assert_eq!(user_count(&app.transcript_cells), 2);

        let base_id = ThreadId::new();
        app.chat_widget.handle_codex_event(Event {
            id: String::new(),
            msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                session_id: base_id,
                forked_from_id: None,
                model: "gpt-test".to_string(),
                model_provider_id: "test-provider".to_string(),
                approval_policy: AskForApproval::Never,
                sandbox_policy: SandboxPolicy::ReadOnly,
                cwd: PathBuf::from("/home/user/project"),
                reasoning_effort: None,
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                rollout_path: Some(PathBuf::new()),
            }),
        });

        app.backtrack.base_id = Some(base_id);
        app.backtrack.primed = true;
        app.backtrack.nth_user_message = user_count(&app.transcript_cells).saturating_sub(1);

        let selection = app
            .confirm_backtrack_from_main()
            .expect("backtrack selection");
        assert_eq!(selection.nth_user_message, 1);
        assert_eq!(selection.prefill, edited_text);
        assert_eq!(selection.text_elements, edited_text_elements);
        assert_eq!(selection.local_image_paths, edited_local_image_paths);

        app.apply_backtrack_rollback(selection);

        let mut rollback_turns = None;
        while let Ok(event) = app_event_rx.try_recv() {
            if let AppEvent::AppServerAction(AppServerAction::ThreadRollback { num_turns }) = event
            {
                rollback_turns = Some(num_turns);
            }
        }

        assert_eq!(rollback_turns, Some(1));
    }

    #[tokio::test]
    async fn shutdown_current_thread_sets_suppress_flag() {
        let (mut app, _app_event_rx) = make_test_app_with_channels().await;

        let thread_id = ThreadId::new();
        let event = SessionConfiguredEvent {
            session_id: thread_id,
            forked_from_id: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            cwd: PathBuf::from("/home/user/project"),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            rollout_path: Some(PathBuf::new()),
        };

        app.chat_widget.handle_codex_event(Event {
            id: String::new(),
            msg: EventMsg::SessionConfigured(event),
        });

        app.shutdown_current_thread().await;

        assert!(
            app.suppress_shutdown_complete,
            "shutdown should set suppress_shutdown_complete"
        );
    }

    #[tokio::test]
    async fn session_summary_skip_zero_usage() {
        assert!(session_summary(TokenUsage::default(), None).is_none());
    }

    #[tokio::test]
    async fn session_summary_includes_resume_hint() {
        let usage = TokenUsage {
            input_tokens: 10,
            output_tokens: 2,
            total_tokens: 12,
            ..Default::default()
        };
        let conversation = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();

        let summary = session_summary(usage, Some(conversation)).expect("summary");
        assert_eq!(
            summary.usage_line,
            "Token usage: total=12 input=10 output=2"
        );
        assert_eq!(
            summary.resume_command,
            Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }
}
