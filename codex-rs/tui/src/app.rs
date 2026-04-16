use crate::app_backtrack::BacktrackState;
use crate::app_command::AppCommand;
use crate::app_command::AppCommandView;
use crate::app_event::AppEvent;
use crate::app_event::ExitMode;
use crate::app_event::FeedbackCategory;
use crate::app_event::RateLimitRefreshOrigin;
use crate::app_event::RealtimeAudioDeviceKind;
#[cfg(target_os = "windows")]
use crate::app_event::WindowsSandboxEnableMode;
use crate::app_event_sender::AppEventSender;
use crate::app_server_approval_conversions::network_approval_context_to_core;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::AppServerStartedThread;
use crate::app_server_session::ThreadSessionState;
use crate::app_server_session::app_server_rate_limit_snapshots_to_core;
use crate::bottom_pane::ApprovalRequest;
use crate::bottom_pane::FeedbackAudience;
use crate::bottom_pane::McpServerElicitationFormRequest;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::chatwidget::ChatWidget;
use crate::chatwidget::ExternalEditorState;
use crate::chatwidget::ReplayKind;
use crate::chatwidget::ThreadInputState;
use crate::cwd_prompt::CwdPromptAction;
use crate::diff_render::DiffSummary;
use crate::exec_command::split_command_string;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::external_editor;
use crate::file_search::FileSearchManager;
use crate::history_cell;
use crate::history_cell::HistoryCell;
#[cfg(not(debug_assertions))]
use crate::history_cell::UpdateAvailableHistoryCell;
use crate::legacy_core::append_message_history_entry;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use crate::legacy_core::config::edit::ConfigEdit;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use crate::legacy_core::config_loader::ConfigLayerStackOrdering;
use crate::legacy_core::lookup_message_history_entry;
use crate::legacy_core::plugins::PluginsManager;
#[cfg(target_os = "windows")]
use crate::legacy_core::windows_sandbox::WindowsSandboxLevelExt;
use crate::model_catalog::ModelCatalog;
use crate::model_migration::ModelMigrationOutcome;
use crate::model_migration::migration_copy_for_models;
use crate::model_migration::run_model_migration_prompt;
use crate::multi_agents::agent_picker_status_dot_spans;
use crate::multi_agents::format_agent_picker_item_name;
use crate::multi_agents::next_agent_shortcut_matches;
use crate::multi_agents::previous_agent_shortcut_matches;
use crate::pager_overlay::Overlay;
use crate::read_session_model;
use crate::render::highlight::highlight_bash_to_lines;
use crate::render::renderable::Renderable;
use crate::resume_picker::SessionSelection;
use crate::resume_picker::SessionTarget;
#[cfg(test)]
use crate::test_support::PathBufExt;
#[cfg(test)]
use crate::test_support::test_path_buf;
#[cfg(test)]
use crate::test_support::test_path_display;
use crate::tui;
use crate::tui::TuiEvent;
use crate::update_action::UpdateAction;
use crate::version::CODEX_CLI_VERSION;
use codex_ansi_escape::ansi_escape_line;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CodexErrorInfo as AppServerCodexErrorInfo;
use codex_app_server_protocol::ConfigLayerSource;
use codex_app_server_protocol::FeedbackUploadParams;
use codex_app_server_protocol::FeedbackUploadResponse;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::PluginInstallParams;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginReadResponse;
use codex_app_server_protocol::PluginUninstallParams;
use codex_app_server_protocol::PluginUninstallResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SkillsListResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadMemoryMode;
use codex_app_server_protocol::ThreadRollbackResponse;
use codex_app_server_protocol::ThreadStartSource;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnError as AppServerTurnError;
use codex_app_server_protocol::TurnStatus;
use codex_config::types::ApprovalsReviewer;
use codex_config::types::ModelAvailabilityNuxConfig;
use codex_exec_server::EnvironmentManager;
use codex_features::Feature;
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_models_manager::model_presets::HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG;
use codex_models_manager::model_presets::HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::approvals::ExecApprovalRequestEvent;
use codex_protocol::config_types::Personality;
#[cfg(target_os = "windows")]
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::openai_models::ModelAvailabilityNux;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelUpgrade;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::FinalOutput;
use codex_protocol::protocol::GetHistoryEntryResponseEvent;
use codex_protocol::protocol::ListSkillsResponseEvent;
#[cfg(test)]
use codex_protocol::protocol::McpAuthStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillErrorInfo;
use codex_protocol::protocol::TokenUsage;
use codex_terminal_detection::user_agent;
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
use tokio::task::JoinHandle;
use toml::Value as TomlValue;
use uuid::Uuid;
mod agent_navigation;
mod app_server_adapter;
pub(crate) mod app_server_requests;
mod loaded_threads;
mod pending_interactive_replay;

use self::agent_navigation::AgentNavigationDirection;
use self::agent_navigation::AgentNavigationState;
use self::app_server_requests::PendingAppServerRequests;
use self::loaded_threads::find_loaded_subagent_threads_for_primary;
use self::pending_interactive_replay::PendingInteractiveReplayState;

const EXTERNAL_EDITOR_HINT: &str = "Save and close external editor to continue.";
const THREAD_EVENT_CHANNEL_CAPACITY: usize = 32768;

enum ThreadInteractiveRequest {
    Approval(ApprovalRequest),
    McpServerElicitation(McpServerElicitationFormRequest),
}

fn app_server_request_id_to_mcp_request_id(
    request_id: &codex_app_server_protocol::RequestId,
) -> codex_protocol::mcp::RequestId {
    match request_id {
        codex_app_server_protocol::RequestId::String(value) => {
            codex_protocol::mcp::RequestId::String(value.clone())
        }
        codex_app_server_protocol::RequestId::Integer(value) => {
            codex_protocol::mcp::RequestId::Integer(*value)
        }
    }
}

fn command_execution_decision_to_review_decision(
    decision: codex_app_server_protocol::CommandExecutionApprovalDecision,
) -> codex_protocol::protocol::ReviewDecision {
    match decision {
        codex_app_server_protocol::CommandExecutionApprovalDecision::Accept => {
            codex_protocol::protocol::ReviewDecision::Approved
        }
        codex_app_server_protocol::CommandExecutionApprovalDecision::AcceptForSession => {
            codex_protocol::protocol::ReviewDecision::ApprovedForSession
        }
        codex_app_server_protocol::CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
            execpolicy_amendment,
        } => codex_protocol::protocol::ReviewDecision::ApprovedExecpolicyAmendment {
            proposed_execpolicy_amendment: execpolicy_amendment.into_core(),
        },
        codex_app_server_protocol::CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
            network_policy_amendment,
        } => codex_protocol::protocol::ReviewDecision::NetworkPolicyAmendment {
            network_policy_amendment: network_policy_amendment.into_core(),
        },
        codex_app_server_protocol::CommandExecutionApprovalDecision::Decline => {
            codex_protocol::protocol::ReviewDecision::Denied
        }
        codex_app_server_protocol::CommandExecutionApprovalDecision::Cancel => {
            codex_protocol::protocol::ReviewDecision::Abort
        }
    }
}

/// Extracts `receiver_thread_ids` from collab agent tool-call notifications.
///
/// Only `ItemStarted` and `ItemCompleted` notifications with a `CollabAgentToolCall` item carry
/// receiver thread ids. All other notification variants return `None`.
fn collab_receiver_thread_ids(notification: &ServerNotification) -> Option<&[String]> {
    match notification {
        ServerNotification::ItemStarted(notification) => match &notification.item {
            ThreadItem::CollabAgentToolCall {
                receiver_thread_ids,
                ..
            } => Some(receiver_thread_ids),
            _ => None,
        },
        ServerNotification::ItemCompleted(notification) => match &notification.item {
            ThreadItem::CollabAgentToolCall {
                receiver_thread_ids,
                ..
            } => Some(receiver_thread_ids),
            _ => None,
        },
        _ => None,
    }
}

fn default_exec_approval_decisions(
    network_approval_context: Option<&codex_protocol::protocol::NetworkApprovalContext>,
    proposed_execpolicy_amendment: Option<&codex_protocol::approvals::ExecPolicyAmendment>,
    proposed_network_policy_amendments: Option<
        &[codex_protocol::approvals::NetworkPolicyAmendment],
    >,
    additional_permissions: Option<&codex_protocol::models::PermissionProfile>,
) -> Vec<codex_protocol::protocol::ReviewDecision> {
    ExecApprovalRequestEvent::default_available_decisions(
        network_approval_context,
        proposed_execpolicy_amendment,
        proposed_network_policy_amendments,
        additional_permissions,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GuardianApprovalsMode {
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    sandbox_policy: SandboxPolicy,
}

/// Enabling the Guardian Approvals experiment in the TUI should also switch the
/// current `/approvals` settings to the matching Guardian Approvals mode. Users
/// can still change `/approvals` afterward; this just assumes that opting into
/// the experiment means they want guardian review enabled immediately.
fn guardian_approvals_mode() -> GuardianApprovalsMode {
    GuardianApprovalsMode {
        approval_policy: AskForApproval::OnRequest,
        approvals_reviewer: ApprovalsReviewer::GuardianSubagent,
        sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
    }
}
/// Baseline cadence for periodic stream commit animation ticks.
///
/// Smooth-mode streaming drains one line per tick, so this interval controls
/// perceived typing speed for non-backlogged output.
const COMMIT_ANIMATION_TICK: Duration = tui::TARGET_FRAME_INTERVAL;

#[derive(Debug, Clone)]
pub struct AppExitInfo {
    pub token_usage: TokenUsage,
    pub thread_id: Option<ThreadId>,
    pub thread_name: Option<String>,
    pub update_action: Option<UpdateAction>,
    pub exit_reason: ExitReason,
}

impl AppExitInfo {
    pub fn fatal(message: impl Into<String>) -> Self {
        Self {
            token_usage: TokenUsage::default(),
            thread_id: None,
            thread_name: None,
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

fn session_summary(
    token_usage: TokenUsage,
    thread_id: Option<ThreadId>,
    thread_name: Option<String>,
) -> Option<SessionSummary> {
    let usage_line = (!token_usage.is_zero()).then(|| FinalOutput::from(token_usage).to_string());
    let resume_command =
        crate::legacy_core::util::resume_command(thread_name.as_deref(), thread_id);

    if usage_line.is_none() && resume_command.is_none() {
        return None;
    }

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

fn list_skills_response_to_core(response: SkillsListResponse) -> ListSkillsResponseEvent {
    ListSkillsResponseEvent {
        skills: response
            .data
            .into_iter()
            .map(|entry| codex_protocol::protocol::SkillsListEntry {
                cwd: entry.cwd,
                skills: entry
                    .skills
                    .into_iter()
                    .map(|skill| codex_protocol::protocol::SkillMetadata {
                        name: skill.name,
                        description: skill.description,
                        short_description: skill.short_description,
                        interface: skill.interface.map(|interface| {
                            codex_protocol::protocol::SkillInterface {
                                display_name: interface.display_name,
                                short_description: interface.short_description,
                                icon_small: interface.icon_small,
                                icon_large: interface.icon_large,
                                brand_color: interface.brand_color,
                                default_prompt: interface.default_prompt,
                            }
                        }),
                        dependencies: skill.dependencies.map(|dependencies| {
                            codex_protocol::protocol::SkillDependencies {
                                tools: dependencies
                                    .tools
                                    .into_iter()
                                    .map(|tool| codex_protocol::protocol::SkillToolDependency {
                                        r#type: tool.r#type,
                                        value: tool.value,
                                        description: tool.description,
                                        transport: tool.transport,
                                        command: tool.command,
                                        url: tool.url,
                                    })
                                    .collect(),
                            }
                        }),
                        path: skill.path,
                        scope: match skill.scope {
                            codex_app_server_protocol::SkillScope::User => {
                                codex_protocol::protocol::SkillScope::User
                            }
                            codex_app_server_protocol::SkillScope::Repo => {
                                codex_protocol::protocol::SkillScope::Repo
                            }
                            codex_app_server_protocol::SkillScope::System => {
                                codex_protocol::protocol::SkillScope::System
                            }
                            codex_app_server_protocol::SkillScope::Admin => {
                                codex_protocol::protocol::SkillScope::Admin
                            }
                        },
                        enabled: skill.enabled,
                    })
                    .collect(),
                errors: entry
                    .errors
                    .into_iter()
                    .map(|error| codex_protocol::protocol::SkillErrorInfo {
                        path: error.path,
                        message: error.message,
                    })
                    .collect(),
            })
            .collect(),
    }
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

fn emit_project_config_warnings(app_event_tx: &AppEventSender, config: &Config) {
    let mut disabled_folders = Vec::new();

    for layer in config.config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ true,
    ) {
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

fn emit_system_bwrap_warning(app_event_tx: &AppEventSender, config: &Config) {
    let Some(message) =
        crate::legacy_core::config::system_bwrap_warning(config.permissions.sandbox_policy.get())
    else {
        return;
    };

    app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
        history_cell::new_warning_event(message),
    )));
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionSummary {
    usage_line: Option<String>,
    resume_command: Option<String>,
}

#[derive(Debug, Clone)]
struct ThreadEventSnapshot {
    session: Option<ThreadSessionState>,
    turns: Vec<Turn>,
    events: Vec<ThreadBufferedEvent>,
    input_state: Option<ThreadInputState>,
}

#[derive(Debug, Clone)]
enum ThreadBufferedEvent {
    Notification(ServerNotification),
    Request(ServerRequest),
    HistoryEntryResponse(GetHistoryEntryResponseEvent),
    FeedbackSubmission(FeedbackThreadEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FeedbackThreadEvent {
    category: FeedbackCategory,
    include_logs: bool,
    feedback_audience: FeedbackAudience,
    result: Result<String, String>,
}

#[derive(Debug)]
struct ThreadEventStore {
    session: Option<ThreadSessionState>,
    turns: Vec<Turn>,
    buffer: VecDeque<ThreadBufferedEvent>,
    pending_interactive_replay: PendingInteractiveReplayState,
    active_turn_id: Option<String>,
    input_state: Option<ThreadInputState>,
    capacity: usize,
    active: bool,
}

impl ThreadEventStore {
    fn event_survives_session_refresh(event: &ThreadBufferedEvent) -> bool {
        matches!(
            event,
            ThreadBufferedEvent::Request(_)
                | ThreadBufferedEvent::Notification(ServerNotification::HookStarted(_))
                | ThreadBufferedEvent::Notification(ServerNotification::HookCompleted(_))
                | ThreadBufferedEvent::FeedbackSubmission(_)
        )
    }

    fn new(capacity: usize) -> Self {
        Self {
            session: None,
            turns: Vec::new(),
            buffer: VecDeque::new(),
            pending_interactive_replay: PendingInteractiveReplayState::default(),
            active_turn_id: None,
            input_state: None,
            capacity,
            active: false,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn new_with_session(capacity: usize, session: ThreadSessionState, turns: Vec<Turn>) -> Self {
        let mut store = Self::new(capacity);
        store.session = Some(session);
        store.set_turns(turns);
        store
    }

    fn set_session(&mut self, session: ThreadSessionState, turns: Vec<Turn>) {
        self.session = Some(session);
        self.set_turns(turns);
    }

    fn rebase_buffer_after_session_refresh(&mut self) {
        self.buffer.retain(Self::event_survives_session_refresh);
    }

    fn set_turns(&mut self, turns: Vec<Turn>) {
        self.active_turn_id = turns
            .iter()
            .rev()
            .find(|turn| matches!(turn.status, TurnStatus::InProgress))
            .map(|turn| turn.id.clone());
        self.turns = turns;
    }

    fn push_notification(&mut self, notification: ServerNotification) {
        self.pending_interactive_replay
            .note_server_notification(&notification);
        match &notification {
            ServerNotification::TurnStarted(turn) => {
                self.active_turn_id = Some(turn.turn.id.clone());
            }
            ServerNotification::TurnCompleted(turn) => {
                if self.active_turn_id.as_deref() == Some(turn.turn.id.as_str()) {
                    self.active_turn_id = None;
                }
            }
            ServerNotification::ThreadClosed(_) => {
                self.active_turn_id = None;
            }
            _ => {}
        }
        self.buffer
            .push_back(ThreadBufferedEvent::Notification(notification));
        if self.buffer.len() > self.capacity
            && let Some(removed) = self.buffer.pop_front()
            && let ThreadBufferedEvent::Request(request) = &removed
        {
            self.pending_interactive_replay
                .note_evicted_server_request(request);
        }
    }

    fn push_request(&mut self, request: ServerRequest) {
        self.pending_interactive_replay
            .note_server_request(&request);
        self.buffer.push_back(ThreadBufferedEvent::Request(request));
        if self.buffer.len() > self.capacity
            && let Some(removed) = self.buffer.pop_front()
            && let ThreadBufferedEvent::Request(request) = &removed
        {
            self.pending_interactive_replay
                .note_evicted_server_request(request);
        }
    }

    fn apply_thread_rollback(&mut self, response: &ThreadRollbackResponse) {
        self.turns = response.thread.turns.clone();
        self.buffer.clear();
        self.pending_interactive_replay = PendingInteractiveReplayState::default();
        self.active_turn_id = None;
    }

    fn snapshot(&self) -> ThreadEventSnapshot {
        ThreadEventSnapshot {
            session: self.session.clone(),
            turns: self.turns.clone(),
            // Thread switches replay buffered events into a rebuilt ChatWidget. Only replay
            // interactive prompts that are still pending, or answered approvals/input will reappear.
            events: self
                .buffer
                .iter()
                .filter(|event| match event {
                    ThreadBufferedEvent::Request(request) => self
                        .pending_interactive_replay
                        .should_replay_snapshot_request(request),
                    ThreadBufferedEvent::Notification(_)
                    | ThreadBufferedEvent::HistoryEntryResponse(_)
                    | ThreadBufferedEvent::FeedbackSubmission(_) => true,
                })
                .cloned()
                .collect(),
            input_state: self.input_state.clone(),
        }
    }

    fn note_outbound_op<T>(&mut self, op: T)
    where
        T: Into<AppCommand>,
    {
        self.pending_interactive_replay.note_outbound_op(op);
    }

    fn op_can_change_pending_replay_state<T>(op: T) -> bool
    where
        T: Into<AppCommand>,
    {
        PendingInteractiveReplayState::op_can_change_state(op)
    }

    fn has_pending_thread_approvals(&self) -> bool {
        self.pending_interactive_replay
            .has_pending_thread_approvals()
    }

    fn active_turn_id(&self) -> Option<&str> {
        self.active_turn_id.as_deref()
    }

    fn clear_active_turn_id(&mut self) {
        self.active_turn_id = None;
    }
}

#[derive(Debug)]
struct ThreadEventChannel {
    sender: mpsc::Sender<ThreadBufferedEvent>,
    receiver: Option<mpsc::Receiver<ThreadBufferedEvent>>,
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

    #[cfg_attr(not(test), allow(dead_code))]
    fn new_with_session(capacity: usize, session: ThreadSessionState, turns: Vec<Turn>) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Self {
            sender,
            receiver: Some(receiver),
            store: Arc::new(Mutex::new(ThreadEventStore::new_with_session(
                capacity, session, turns,
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

    if !available_models
        .iter()
        .any(|preset| preset.model == target_model && preset.show_in_picker)
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
        .find(|preset| preset.model == target_model && preset.show_in_picker)
}

const MODEL_AVAILABILITY_NUX_MAX_SHOW_COUNT: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
struct StartupTooltipOverride {
    model_slug: String,
    message: String,
}

fn select_model_availability_nux(
    available_models: &[ModelPreset],
    nux_config: &ModelAvailabilityNuxConfig,
) -> Option<StartupTooltipOverride> {
    available_models.iter().find_map(|preset| {
        let ModelAvailabilityNux { message } = preset.availability_nux.as_ref()?;
        let shown_count = nux_config
            .shown_count
            .get(&preset.model)
            .copied()
            .unwrap_or_default();
        (shown_count < MODEL_AVAILABILITY_NUX_MAX_SHOW_COUNT).then(|| StartupTooltipOverride {
            model_slug: preset.model.clone(),
            message: message.clone(),
        })
    })
}

async fn prepare_startup_tooltip_override(
    config: &mut Config,
    available_models: &[ModelPreset],
    is_first_run: bool,
) -> Option<String> {
    if is_first_run || !config.show_tooltips {
        return None;
    }

    let tooltip_override =
        select_model_availability_nux(available_models, &config.model_availability_nux)?;

    let shown_count = config
        .model_availability_nux
        .shown_count
        .get(&tooltip_override.model_slug)
        .copied()
        .unwrap_or_default();
    let next_count = shown_count.saturating_add(1);
    let mut updated_shown_count = config.model_availability_nux.shown_count.clone();
    updated_shown_count.insert(tooltip_override.model_slug.clone(), next_count);

    if let Err(err) = ConfigEditsBuilder::new(&config.codex_home)
        .set_model_availability_nux_count(&updated_shown_count)
        .apply()
        .await
    {
        tracing::error!(
            error = %err,
            model = %tooltip_override.model_slug,
            "failed to persist model availability nux count"
        );
        return Some(tooltip_override.message);
    }

    config.model_availability_nux.shown_count = updated_shown_count;
    Some(tooltip_override.message)
}

async fn handle_model_migration_prompt_if_needed(
    tui: &mut tui::Tui,
    config: &mut Config,
    model: &str,
    app_event_tx: &AppEventSender,
    available_models: &[ModelPreset],
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
            available_models,
        ) {
            return None;
        }

        let current_preset = available_models.iter().find(|preset| preset.model == model);
        let target_preset = target_preset_for_upgrade(available_models, &target_model);
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
                    thread_name: None,
                    update_action: None,
                    exit_reason: ExitReason::UserRequested,
                });
            }
        }
    }

    None
}

pub(crate) struct App {
    model_catalog: Arc<ModelCatalog>,
    pub(crate) session_telemetry: SessionTelemetry,
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) chat_widget: ChatWidget,
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
    // Shared across ChatWidget instances so invalid status-line config warnings only emit once.
    status_line_invalid_items_warned: Arc<AtomicBool>,
    // Shared across ChatWidget instances so invalid terminal-title config warnings only emit once.
    terminal_title_invalid_items_warned: Arc<AtomicBool>,

    // Esc-backtracking state grouped
    pub(crate) backtrack: crate::app_backtrack::BacktrackState,
    /// When set, the next draw re-renders the transcript into terminal scrollback once.
    ///
    /// This is used after a confirmed thread rollback to ensure scrollback reflects the trimmed
    /// transcript cells.
    pub(crate) backtrack_render_pending: bool,
    pub(crate) feedback: codex_feedback::CodexFeedback,
    feedback_audience: FeedbackAudience,
    environment_manager: Arc<EnvironmentManager>,
    remote_app_server_url: Option<String>,
    remote_app_server_auth_token: Option<String>,
    /// Set when the user confirms an update; propagated on exit.
    pub(crate) pending_update_action: Option<UpdateAction>,

    /// Tracks the thread we intentionally shut down while exiting the app.
    ///
    /// When this matches the active thread, its `ShutdownComplete` should lead to
    /// process exit instead of being treated as an unexpected sub-agent death that
    /// triggers failover to the primary thread.
    ///
    /// This is thread-scoped state (`Option<ThreadId>`) instead of a global bool
    /// so shutdown events from other threads still take the normal failover path.
    pending_shutdown_exit_thread_id: Option<ThreadId>,

    windows_sandbox: WindowsSandboxState,

    thread_event_channels: HashMap<ThreadId, ThreadEventChannel>,
    thread_event_listener_tasks: HashMap<ThreadId, JoinHandle<()>>,
    agent_navigation: AgentNavigationState,
    active_thread_id: Option<ThreadId>,
    active_thread_rx: Option<mpsc::Receiver<ThreadBufferedEvent>>,
    primary_thread_id: Option<ThreadId>,
    last_subagent_backfill_attempt: Option<ThreadId>,
    primary_session_configured: Option<ThreadSessionState>,
    pending_primary_events: VecDeque<ThreadBufferedEvent>,
    pending_app_server_requests: PendingAppServerRequests,
}

#[derive(Default)]
struct WindowsSandboxState {
    setup_started_at: Option<Instant>,
    // One-shot suppression of the next world-writable scan after user confirmation.
    skip_world_writable_scan_once: bool,
}

fn normalize_harness_overrides_for_cwd(
    mut overrides: ConfigOverrides,
    base_cwd: &AbsolutePathBuf,
) -> Result<ConfigOverrides> {
    if overrides.additional_writable_roots.is_empty() {
        return Ok(overrides);
    }

    let mut normalized = Vec::with_capacity(overrides.additional_writable_roots.len());
    for root in overrides.additional_writable_roots.drain(..) {
        let absolute = base_cwd.join(root);
        normalized.push(absolute.into_path_buf());
    }
    overrides.additional_writable_roots = normalized;
    Ok(overrides)
}

fn active_turn_not_steerable_turn_error(error: &TypedRequestError) -> Option<AppServerTurnError> {
    let TypedRequestError::Server { source, .. } = error else {
        return None;
    };
    let turn_error: AppServerTurnError = serde_json::from_value(source.data.clone()?).ok()?;
    matches!(
        turn_error.codex_error_info,
        Some(AppServerCodexErrorInfo::ActiveTurnNotSteerable { .. })
    )
    .then_some(turn_error)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ActiveTurnSteerRace {
    Missing,
    ExpectedTurnMismatch { actual_turn_id: String },
}

fn active_turn_steer_race(error: &TypedRequestError) -> Option<ActiveTurnSteerRace> {
    let TypedRequestError::Server { method, source } = error else {
        return None;
    };
    if method != "turn/steer" {
        return None;
    }
    if source.message == "no active turn to steer" {
        return Some(ActiveTurnSteerRace::Missing);
    }

    // App-server steer mismatches mean our cached active turn id is stale, but the response
    // includes the server's current active turn so we can resynchronize and retry once.
    let mismatch_prefix = "expected active turn id `";
    let mismatch_separator = "` but found `";
    let actual_turn_id = source
        .message
        .strip_prefix(mismatch_prefix)?
        .split_once(mismatch_separator)?
        .1
        .strip_suffix('`')?
        .to_string();
    Some(ActiveTurnSteerRace::ExpectedTurnMismatch { actual_turn_id })
}

impl App {
    pub fn chatwidget_init_for_forked_or_resumed_thread(
        &self,
        tui: &mut tui::Tui,
        cfg: crate::legacy_core::config::Config,
    ) -> crate::chatwidget::ChatWidgetInit {
        crate::chatwidget::ChatWidgetInit {
            config: cfg,
            frame_requester: tui.frame_requester(),
            app_event_tx: self.app_event_tx.clone(),
            // Fork/resume bootstraps here don't carry any prefilled message content.
            initial_user_message: None,
            enhanced_keys_supported: self.enhanced_keys_supported,
            has_chatgpt_account: self.chat_widget.has_chatgpt_account(),
            model_catalog: self.model_catalog.clone(),
            feedback: self.feedback.clone(),
            is_first_run: false,
            status_account_display: self.chat_widget.status_account_display().cloned(),
            initial_plan_type: self.chat_widget.current_plan_type(),
            model: Some(self.chat_widget.current_model().to_string()),
            startup_tooltip_override: None,
            status_line_invalid_items_warned: self.status_line_invalid_items_warned.clone(),
            terminal_title_invalid_items_warned: self.terminal_title_invalid_items_warned.clone(),
            session_telemetry: self.session_telemetry.clone(),
        }
    }

    async fn rebuild_config_for_cwd(&self, cwd: PathBuf) -> Result<Config> {
        let mut overrides = self.harness_overrides.clone();
        overrides.cwd = Some(cwd.clone());
        let cwd_display = cwd.display().to_string();
        ConfigBuilder::default()
            .codex_home(self.config.codex_home.to_path_buf())
            .cli_overrides(self.cli_kv_overrides.clone())
            .harness_overrides(overrides)
            .build()
            .await
            .wrap_err_with(|| format!("Failed to rebuild config for cwd {cwd_display}"))
    }

    async fn refresh_in_memory_config_from_disk(&mut self) -> Result<()> {
        let mut config = self
            .rebuild_config_for_cwd(self.chat_widget.config_ref().cwd.to_path_buf())
            .await?;
        self.apply_runtime_policy_overrides(&mut config);
        self.config = config;
        self.chat_widget.sync_plugin_mentions_config(&self.config);
        Ok(())
    }

    async fn refresh_in_memory_config_from_disk_best_effort(&mut self, action: &str) {
        if let Err(err) = self.refresh_in_memory_config_from_disk().await {
            tracing::warn!(
                error = %err,
                action,
                "failed to refresh config before thread transition; continuing with current in-memory config"
            );
        }
    }

    async fn rebuild_config_for_resume_or_fallback(
        &mut self,
        current_cwd: &Path,
        resume_cwd: PathBuf,
    ) -> Result<Config> {
        match self.rebuild_config_for_cwd(resume_cwd.clone()).await {
            Ok(config) => Ok(config),
            Err(err) => {
                if crate::cwds_differ(current_cwd, &resume_cwd) {
                    Err(err)
                } else {
                    let resume_cwd_display = resume_cwd.display().to_string();
                    tracing::warn!(
                        error = %err,
                        cwd = %resume_cwd_display,
                        "failed to rebuild config for same-cwd resume; using current in-memory config"
                    );
                    Ok(self.config.clone())
                }
            }
        }
    }

    fn apply_runtime_policy_overrides(&mut self, config: &mut Config) {
        if let Some(policy) = self.runtime_approval_policy_override.as_ref()
            && let Err(err) = config.permissions.approval_policy.set(*policy)
        {
            tracing::warn!(%err, "failed to carry forward approval policy override");
            self.chat_widget.add_error_message(format!(
                "Failed to carry forward approval policy override: {err}"
            ));
        }
        if let Some(policy) = self.runtime_sandbox_policy_override.as_ref()
            && let Err(err) = config.permissions.sandbox_policy.set(policy.clone())
        {
            tracing::warn!(%err, "failed to carry forward sandbox policy override");
            self.chat_widget.add_error_message(format!(
                "Failed to carry forward sandbox policy override: {err}"
            ));
        }
    }

    fn set_approvals_reviewer_in_app_and_widget(&mut self, reviewer: ApprovalsReviewer) {
        self.config.approvals_reviewer = reviewer;
        self.chat_widget.set_approvals_reviewer(reviewer);
    }

    fn try_set_approval_policy_on_config(
        &mut self,
        config: &mut Config,
        policy: AskForApproval,
        user_message_prefix: &str,
        log_message: &str,
    ) -> bool {
        if let Err(err) = config.permissions.approval_policy.set(policy) {
            tracing::warn!(error = %err, "{log_message}");
            self.chat_widget
                .add_error_message(format!("{user_message_prefix}: {err}"));
            return false;
        }

        true
    }

    fn try_set_sandbox_policy_on_config(
        &mut self,
        config: &mut Config,
        policy: SandboxPolicy,
        user_message_prefix: &str,
        log_message: &str,
    ) -> bool {
        if let Err(err) = config.permissions.sandbox_policy.set(policy) {
            tracing::warn!(error = %err, "{log_message}");
            self.chat_widget
                .add_error_message(format!("{user_message_prefix}: {err}"));
            return false;
        }

        true
    }

    async fn update_feature_flags(&mut self, updates: Vec<(Feature, bool)>) {
        if updates.is_empty() {
            return;
        }

        let guardian_approvals_preset = guardian_approvals_mode();
        let mut next_config = self.config.clone();
        let active_profile = self.active_profile.clone();
        let scoped_segments = |key: &str| {
            if let Some(profile) = active_profile.as_deref() {
                vec!["profiles".to_string(), profile.to_string(), key.to_string()]
            } else {
                vec![key.to_string()]
            }
        };
        let windows_sandbox_changed = updates.iter().any(|(feature, _)| {
            matches!(
                feature,
                Feature::WindowsSandbox | Feature::WindowsSandboxElevated
            )
        });
        let mut approval_policy_override = None;
        let mut approvals_reviewer_override = None;
        let mut sandbox_policy_override = None;
        let mut feature_updates_to_apply = Vec::with_capacity(updates.len());
        // Guardian Approvals owns `approvals_reviewer`, but disabling the feature
        // from inside a profile should not silently clear a value configured at
        // the root scope.
        let (root_approvals_reviewer_blocks_profile_disable, profile_approvals_reviewer_configured) = {
            let effective_config = next_config.config_layer_stack.effective_config();
            let root_blocks_disable = effective_config
                .as_table()
                .and_then(|table| table.get("approvals_reviewer"))
                .is_some_and(|value| value != &TomlValue::String("user".to_string()));
            let profile_configured = active_profile.as_deref().is_some_and(|profile| {
                effective_config
                    .as_table()
                    .and_then(|table| table.get("profiles"))
                    .and_then(TomlValue::as_table)
                    .and_then(|profiles| profiles.get(profile))
                    .and_then(TomlValue::as_table)
                    .is_some_and(|profile_config| profile_config.contains_key("approvals_reviewer"))
            });
            (root_blocks_disable, profile_configured)
        };
        let mut permissions_history_label: Option<&'static str> = None;
        let mut builder = ConfigEditsBuilder::new(&self.config.codex_home)
            .with_profile(self.active_profile.as_deref());

        for (feature, enabled) in updates {
            let feature_key = feature.key();
            let mut feature_edits = Vec::new();
            if feature == Feature::GuardianApproval
                && !enabled
                && self.active_profile.is_some()
                && root_approvals_reviewer_blocks_profile_disable
            {
                self.chat_widget.add_error_message(
                        "Cannot disable Guardian Approvals in this profile because `approvals_reviewer` is configured outside the active profile.".to_string(),
                    );
                continue;
            }
            let mut feature_config = next_config.clone();
            if let Err(err) = feature_config.features.set_enabled(feature, enabled) {
                tracing::error!(
                    error = %err,
                    feature = feature_key,
                    "failed to update constrained feature flags"
                );
                self.chat_widget.add_error_message(format!(
                    "Failed to update experimental feature `{feature_key}`: {err}"
                ));
                continue;
            }
            let effective_enabled = feature_config.features.enabled(feature);
            if feature == Feature::GuardianApproval {
                let previous_approvals_reviewer = feature_config.approvals_reviewer;
                if effective_enabled {
                    // Persist the reviewer setting so future sessions keep the
                    // experiment's matching `/approvals` mode until the user
                    // changes it explicitly.
                    feature_config.approvals_reviewer =
                        guardian_approvals_preset.approvals_reviewer;
                    feature_edits.push(ConfigEdit::SetPath {
                        segments: scoped_segments("approvals_reviewer"),
                        value: guardian_approvals_preset
                            .approvals_reviewer
                            .to_string()
                            .into(),
                    });
                    if previous_approvals_reviewer != guardian_approvals_preset.approvals_reviewer {
                        permissions_history_label = Some("Guardian Approvals");
                    }
                } else if !effective_enabled {
                    if profile_approvals_reviewer_configured || self.active_profile.is_none() {
                        feature_edits.push(ConfigEdit::ClearPath {
                            segments: scoped_segments("approvals_reviewer"),
                        });
                    }
                    feature_config.approvals_reviewer = ApprovalsReviewer::User;
                    if previous_approvals_reviewer != ApprovalsReviewer::User {
                        permissions_history_label = Some("Default");
                    }
                }
                approvals_reviewer_override = Some(feature_config.approvals_reviewer);
            }
            if feature == Feature::GuardianApproval && effective_enabled {
                // The feature flag alone is not enough for the live session.
                // We also align approval policy + sandbox to the Guardian
                // Approvals preset so enabling the experiment immediately
                // makes guardian review observable in the current thread.
                if !self.try_set_approval_policy_on_config(
                    &mut feature_config,
                    guardian_approvals_preset.approval_policy,
                    "Failed to enable Guardian Approvals",
                    "failed to set guardian approvals approval policy on staged config",
                ) {
                    continue;
                }
                if !self.try_set_sandbox_policy_on_config(
                    &mut feature_config,
                    guardian_approvals_preset.sandbox_policy.clone(),
                    "Failed to enable Guardian Approvals",
                    "failed to set guardian approvals sandbox policy on staged config",
                ) {
                    continue;
                }
                feature_edits.extend([
                    ConfigEdit::SetPath {
                        segments: scoped_segments("approval_policy"),
                        value: "on-request".into(),
                    },
                    ConfigEdit::SetPath {
                        segments: scoped_segments("sandbox_mode"),
                        value: "workspace-write".into(),
                    },
                ]);
                approval_policy_override = Some(guardian_approvals_preset.approval_policy);
                sandbox_policy_override = Some(guardian_approvals_preset.sandbox_policy.clone());
            }
            next_config = feature_config;
            feature_updates_to_apply.push((feature, effective_enabled));
            builder = builder
                .with_edits(feature_edits)
                .set_feature_enabled(feature_key, effective_enabled);
        }

        // Persist first so the live session does not diverge from disk if the
        // config edit fails. Runtime/UI state is patched below only after the
        // durable config update succeeds.
        if let Err(err) = builder.apply().await {
            tracing::error!(error = %err, "failed to persist feature flags");
            self.chat_widget
                .add_error_message(format!("Failed to update experimental features: {err}"));
            return;
        }

        self.config = next_config;
        let show_memory_enable_notice = feature_updates_to_apply
            .iter()
            .any(|(feature, enabled)| *feature == Feature::MemoryTool && *enabled);
        for (feature, effective_enabled) in feature_updates_to_apply {
            self.chat_widget
                .set_feature_enabled(feature, effective_enabled);
        }
        if show_memory_enable_notice {
            self.chat_widget.add_memories_enable_notice();
        }
        if approvals_reviewer_override.is_some() {
            self.set_approvals_reviewer_in_app_and_widget(self.config.approvals_reviewer);
        }
        if approval_policy_override.is_some() {
            self.chat_widget
                .set_approval_policy(self.config.permissions.approval_policy.value());
        }
        if sandbox_policy_override.is_some()
            && let Err(err) = self
                .chat_widget
                .set_sandbox_policy(self.config.permissions.sandbox_policy.get().clone())
        {
            tracing::error!(
                error = %err,
                "failed to set guardian approvals sandbox policy on chat config"
            );
            self.chat_widget
                .add_error_message(format!("Failed to enable Guardian Approvals: {err}"));
        }

        if approval_policy_override.is_some()
            || approvals_reviewer_override.is_some()
            || sandbox_policy_override.is_some()
        {
            // This uses `OverrideTurnContext` intentionally: toggling the
            // experiment should update the active thread's effective approval
            // settings immediately, just like a `/approvals` selection. Without
            // this runtime patch, the config edit would only affect future
            // sessions or turns recreated from disk.
            let op = AppCommand::override_turn_context(
                /*cwd*/ None,
                approval_policy_override,
                approvals_reviewer_override,
                sandbox_policy_override,
                /*windows_sandbox_level*/ None,
                /*model*/ None,
                /*effort*/ None,
                /*summary*/ None,
                /*service_tier*/ None,
                /*collaboration_mode*/ None,
                /*personality*/ None,
            );
            let replay_state_op =
                ThreadEventStore::op_can_change_pending_replay_state(&op).then(|| op.clone());
            let submitted = self.chat_widget.submit_op(op);
            if submitted && let Some(op) = replay_state_op.as_ref() {
                self.note_active_thread_outbound_op(op).await;
                self.refresh_pending_thread_approvals().await;
            }
        }

        if windows_sandbox_changed {
            #[cfg(target_os = "windows")]
            {
                let windows_sandbox_level = WindowsSandboxLevel::from_config(&self.config);
                self.app_event_tx.send(AppEvent::CodexOp(
                    AppCommand::override_turn_context(
                        /*cwd*/ None,
                        /*approval_policy*/ None,
                        /*approvals_reviewer*/ None,
                        /*sandbox_policy*/ None,
                        #[cfg(target_os = "windows")]
                        Some(windows_sandbox_level),
                        /*model*/ None,
                        /*effort*/ None,
                        /*summary*/ None,
                        /*service_tier*/ None,
                        /*collaboration_mode*/ None,
                        /*personality*/ None,
                    )
                    .into_core(),
                ));
            }
        }

        if let Some(label) = permissions_history_label {
            self.chat_widget.add_info_message(
                format!("Permissions updated to {label}"),
                /*hint*/ None,
            );
        }
    }

    async fn update_memory_settings(
        &mut self,
        use_memories: bool,
        generate_memories: bool,
    ) -> bool {
        let active_profile = self.active_profile.clone();
        let scoped_memory_segments = |key: &str| {
            if let Some(profile) = active_profile.as_deref() {
                vec![
                    "profiles".to_string(),
                    profile.to_string(),
                    "memories".to_string(),
                    key.to_string(),
                ]
            } else {
                vec!["memories".to_string(), key.to_string()]
            }
        };
        let edits = [
            ConfigEdit::SetPath {
                segments: scoped_memory_segments("use_memories"),
                value: use_memories.into(),
            },
            ConfigEdit::SetPath {
                segments: scoped_memory_segments("generate_memories"),
                value: generate_memories.into(),
            },
        ];

        if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
            .with_edits(edits)
            .apply()
            .await
        {
            tracing::error!(error = %err, "failed to persist memory settings");
            self.chat_widget
                .add_error_message(format!("Failed to save memory settings: {err}"));
            return false;
        }

        self.config.memories.use_memories = use_memories;
        self.config.memories.generate_memories = generate_memories;
        self.chat_widget
            .set_memory_settings(use_memories, generate_memories);
        true
    }

    async fn update_memory_settings_with_app_server(
        &mut self,
        app_server: &mut AppServerSession,
        use_memories: bool,
        generate_memories: bool,
    ) {
        let previous_generate_memories = self.config.memories.generate_memories;
        if !self
            .update_memory_settings(use_memories, generate_memories)
            .await
        {
            return;
        }

        if previous_generate_memories == generate_memories {
            return;
        }

        let Some(thread_id) = self.current_displayed_thread_id() else {
            return;
        };

        let mode = if generate_memories {
            ThreadMemoryMode::Enabled
        } else {
            ThreadMemoryMode::Disabled
        };

        if let Err(err) = app_server.thread_memory_mode_set(thread_id, mode).await {
            tracing::error!(error = %err, %thread_id, "failed to update thread memory mode");
            self.chat_widget.add_error_message(format!(
                "Saved memory settings, but failed to update the current thread: {err}"
            ));
        }
    }

    async fn reset_memories_with_app_server(&mut self, app_server: &mut AppServerSession) {
        if let Err(err) = app_server.memory_reset().await {
            tracing::error!(error = %err, "failed to reset memories");
            self.chat_widget
                .add_error_message(format!("Failed to reset memories: {err}"));
            return;
        }

        self.chat_widget
            .add_info_message("Reset local memories.".to_string(), /*hint*/ None);
    }

    fn open_url_in_browser(&mut self, url: String) {
        if let Err(err) = webbrowser::open(&url) {
            self.chat_widget
                .add_error_message(format!("Failed to open browser for {url}: {err}"));
            return;
        }

        self.chat_widget
            .add_info_message(format!("Opened {url} in your browser."), /*hint*/ None);
    }

    fn clear_ui_header_lines_with_version(
        &self,
        width: u16,
        version: &'static str,
    ) -> Vec<Line<'static>> {
        history_cell::SessionHeaderHistoryCell::new(
            self.chat_widget.current_model().to_string(),
            self.chat_widget.current_reasoning_effort(),
            self.chat_widget.should_show_fast_status(
                self.chat_widget.current_model(),
                self.chat_widget.current_service_tier(),
            ),
            self.config.cwd.to_path_buf(),
            version,
        )
        .with_yolo_mode(history_cell::is_yolo_mode(&self.config))
        .display_lines(width)
    }

    fn clear_ui_header_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.clear_ui_header_lines_with_version(width, CODEX_CLI_VERSION)
    }

    fn queue_clear_ui_header(&mut self, tui: &mut tui::Tui) {
        let width = tui.terminal.last_known_screen_size.width;
        let header_lines = self.clear_ui_header_lines(width);
        if !header_lines.is_empty() {
            tui.insert_history_lines(header_lines);
            self.has_emitted_history_lines = true;
        }
    }

    fn clear_terminal_ui(&mut self, tui: &mut tui::Tui, redraw_header: bool) -> Result<()> {
        let is_alt_screen_active = tui.is_alt_screen_active();

        // Drop queued history insertions so stale transcript lines cannot be flushed after /clear.
        tui.clear_pending_history_lines();

        if is_alt_screen_active {
            tui.terminal.clear_visible_screen()?;
        } else {
            // Some terminals (Terminal.app, Warp) do not reliably drop scrollback when purge and
            // clear are emitted as separate backend commands. Prefer a single ANSI sequence.
            tui.terminal.clear_scrollback_and_visible_screen_ansi()?;
        }

        let mut area = tui.terminal.viewport_area;
        if area.y > 0 {
            // After a full clear, anchor the inline viewport at the top and redraw a fresh header
            // box. `insert_history_lines()` will shift the viewport down by the rendered height.
            area.y = 0;
            tui.terminal.set_viewport_area(area);
        }
        self.has_emitted_history_lines = false;

        if redraw_header {
            self.queue_clear_ui_header(tui);
        }
        Ok(())
    }

    fn reset_app_ui_state_after_clear(&mut self) {
        self.overlay = None;
        self.transcript_cells.clear();
        self.deferred_history_lines.clear();
        self.has_emitted_history_lines = false;
        self.backtrack = BacktrackState::default();
        self.backtrack_render_pending = false;
    }

    async fn shutdown_current_thread(&mut self, app_server: &mut AppServerSession) {
        if let Some(thread_id) = self.chat_widget.thread_id() {
            // Clear any in-flight rollback guard when switching threads.
            self.backtrack.pending_rollback = None;
            if let Err(err) = app_server.thread_unsubscribe(thread_id).await {
                tracing::warn!("failed to unsubscribe thread {thread_id}: {err}");
            }
            self.abort_thread_event_listener(thread_id);
        }
    }

    fn abort_thread_event_listener(&mut self, thread_id: ThreadId) {
        if let Some(handle) = self.thread_event_listener_tasks.remove(&thread_id) {
            handle.abort();
        }
    }

    fn abort_all_thread_event_listeners(&mut self) {
        for handle in self
            .thread_event_listener_tasks
            .drain()
            .map(|(_, handle)| handle)
        {
            handle.abort();
        }
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
        self.set_thread_active(thread_id, /*active*/ true).await;
        let receiver = if let Some(channel) = self.thread_event_channels.get_mut(&thread_id) {
            channel.receiver.take()
        } else {
            None
        };
        self.active_thread_id = Some(thread_id);
        self.active_thread_rx = receiver;
        self.refresh_pending_thread_approvals().await;
    }

    async fn store_active_thread_receiver(&mut self) {
        let Some(active_id) = self.active_thread_id else {
            return;
        };
        let input_state = self.chat_widget.capture_thread_input_state();
        if let Some(channel) = self.thread_event_channels.get_mut(&active_id) {
            let receiver = self.active_thread_rx.take();
            let mut store = channel.store.lock().await;
            store.active = false;
            store.input_state = input_state;
            if let Some(receiver) = receiver {
                channel.receiver = Some(receiver);
            }
        }
    }

    async fn activate_thread_for_replay(
        &mut self,
        thread_id: ThreadId,
    ) -> Option<(mpsc::Receiver<ThreadBufferedEvent>, ThreadEventSnapshot)> {
        let channel = self.thread_event_channels.get_mut(&thread_id)?;
        let receiver = channel.receiver.take()?;
        let mut store = channel.store.lock().await;
        store.active = true;
        let snapshot = store.snapshot();
        Some((receiver, snapshot))
    }

    async fn clear_active_thread(&mut self) {
        if let Some(active_id) = self.active_thread_id.take() {
            self.set_thread_active(active_id, /*active*/ false).await;
        }
        self.active_thread_rx = None;
        self.refresh_pending_thread_approvals().await;
    }

    async fn note_thread_outbound_op(&mut self, thread_id: ThreadId, op: &AppCommand) {
        let Some(channel) = self.thread_event_channels.get(&thread_id) else {
            return;
        };
        let mut store = channel.store.lock().await;
        store.note_outbound_op(op);
    }

    async fn note_active_thread_outbound_op(&mut self, op: &AppCommand) {
        if !ThreadEventStore::op_can_change_pending_replay_state(op) {
            return;
        }
        let Some(thread_id) = self.active_thread_id else {
            return;
        };
        self.note_thread_outbound_op(thread_id, op).await;
    }

    async fn active_turn_id_for_thread(&self, thread_id: ThreadId) -> Option<String> {
        let channel = self.thread_event_channels.get(&thread_id)?;
        let store = channel.store.lock().await;
        store.active_turn_id().map(ToOwned::to_owned)
    }

    fn thread_label(&self, thread_id: ThreadId) -> String {
        let is_primary = self.primary_thread_id == Some(thread_id);
        let fallback_label = if is_primary {
            "Main [default]".to_string()
        } else {
            let thread_id = thread_id.to_string();
            let short_id: String = thread_id.chars().take(8).collect();
            format!("Agent ({short_id})")
        };
        if let Some(entry) = self.agent_navigation.get(&thread_id) {
            let label = format_agent_picker_item_name(
                entry.agent_nickname.as_deref(),
                entry.agent_role.as_deref(),
                is_primary,
            );
            if label == "Agent" {
                let thread_id = thread_id.to_string();
                let short_id: String = thread_id.chars().take(8).collect();
                format!("{label} ({short_id})")
            } else {
                label
            }
        } else {
            fallback_label
        }
    }

    /// Returns the thread whose transcript is currently on screen.
    ///
    /// `active_thread_id` is the source of truth during steady state, but the widget can briefly
    /// lag behind thread bookkeeping during transitions. The footer label and adjacent-thread
    /// navigation both follow what the user is actually looking at, not whichever thread most
    /// recently began switching.
    fn current_displayed_thread_id(&self) -> Option<ThreadId> {
        self.active_thread_id.or(self.chat_widget.thread_id())
    }

    fn ignore_same_thread_resume(
        &mut self,
        target_session: &crate::resume_picker::SessionTarget,
    ) -> bool {
        if self.active_thread_id != Some(target_session.thread_id) {
            return false;
        };

        self.chat_widget.add_info_message(
            format!("Already viewing {}.", target_session.display_label()),
            /*hint*/ None,
        );
        true
    }

    /// Mirrors the visible thread into the contextual footer row.
    ///
    /// The footer sometimes shows ambient context instead of an instructional hint. In multi-agent
    /// sessions, that contextual row includes the currently viewed agent label. The label is
    /// intentionally hidden until there is more than one known thread so single-thread sessions do
    /// not spend footer space restating that the user is already on the main conversation.
    fn sync_active_agent_label(&mut self) {
        let label = self
            .agent_navigation
            .active_agent_label(self.current_displayed_thread_id(), self.primary_thread_id);
        self.chat_widget.set_active_agent_label(label);
    }

    async fn thread_cwd(&self, thread_id: ThreadId) -> Option<AbsolutePathBuf> {
        let channel = self.thread_event_channels.get(&thread_id)?;
        let store = channel.store.lock().await;
        store.session.as_ref().map(|session| session.cwd.clone())
    }

    async fn interactive_request_for_thread_request(
        &self,
        thread_id: ThreadId,
        request: &ServerRequest,
    ) -> Option<ThreadInteractiveRequest> {
        let thread_label = Some(self.thread_label(thread_id));
        match request {
            ServerRequest::CommandExecutionRequestApproval { params, .. } => {
                let network_approval_context = params
                    .network_approval_context
                    .clone()
                    .map(network_approval_context_to_core);
                let additional_permissions = params.additional_permissions.clone().map(Into::into);
                let proposed_execpolicy_amendment = params
                    .proposed_execpolicy_amendment
                    .clone()
                    .map(codex_app_server_protocol::ExecPolicyAmendment::into_core);
                let proposed_network_policy_amendments = params
                    .proposed_network_policy_amendments
                    .clone()
                    .map(|amendments| {
                        amendments
                            .into_iter()
                            .map(codex_app_server_protocol::NetworkPolicyAmendment::into_core)
                            .collect::<Vec<_>>()
                    });
                Some(ThreadInteractiveRequest::Approval(ApprovalRequest::Exec {
                    thread_id,
                    thread_label,
                    id: params
                        .approval_id
                        .clone()
                        .unwrap_or_else(|| params.item_id.clone()),
                    command: params
                        .command
                        .as_deref()
                        .map(split_command_string)
                        .unwrap_or_default(),
                    reason: params.reason.clone(),
                    available_decisions: params
                        .available_decisions
                        .clone()
                        .map(|decisions| {
                            decisions
                                .into_iter()
                                .map(command_execution_decision_to_review_decision)
                                .collect()
                        })
                        .unwrap_or_else(|| {
                            default_exec_approval_decisions(
                                network_approval_context.as_ref(),
                                proposed_execpolicy_amendment.as_ref(),
                                proposed_network_policy_amendments.as_deref(),
                                additional_permissions.as_ref(),
                            )
                        }),
                    network_approval_context,
                    additional_permissions,
                }))
            }
            ServerRequest::FileChangeRequestApproval { params, .. } => Some(
                ThreadInteractiveRequest::Approval(ApprovalRequest::ApplyPatch {
                    thread_id,
                    thread_label,
                    id: params.item_id.clone(),
                    reason: params.reason.clone(),
                    cwd: self
                        .thread_cwd(thread_id)
                        .await
                        .unwrap_or_else(|| self.config.cwd.clone()),
                    changes: HashMap::new(),
                }),
            ),
            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                if let Some(request) = McpServerElicitationFormRequest::from_app_server_request(
                    thread_id,
                    app_server_request_id_to_mcp_request_id(request_id),
                    params.clone(),
                ) {
                    Some(ThreadInteractiveRequest::McpServerElicitation(request))
                } else {
                    Some(ThreadInteractiveRequest::Approval(
                        ApprovalRequest::McpElicitation {
                            thread_id,
                            thread_label,
                            server_name: params.server_name.clone(),
                            request_id: app_server_request_id_to_mcp_request_id(request_id),
                            message: match &params.request {
                                codex_app_server_protocol::McpServerElicitationRequest::Form {
                                    message,
                                    ..
                                }
                                | codex_app_server_protocol::McpServerElicitationRequest::Url {
                                    message,
                                    ..
                                } => message.clone(),
                            },
                        },
                    ))
                }
            }
            ServerRequest::PermissionsRequestApproval { params, .. } => Some(
                ThreadInteractiveRequest::Approval(ApprovalRequest::Permissions {
                    thread_id,
                    thread_label,
                    call_id: params.item_id.clone(),
                    reason: params.reason.clone(),
                    permissions: params.permissions.clone().into(),
                }),
            ),
            _ => None,
        }
    }

    async fn submit_active_thread_op(
        &mut self,
        app_server: &mut AppServerSession,
        op: AppCommand,
    ) -> Result<()> {
        let Some(thread_id) = self.active_thread_id else {
            self.chat_widget
                .add_error_message("No active thread is available.".to_string());
            return Ok(());
        };

        self.submit_thread_op(app_server, thread_id, op).await
    }

    async fn submit_thread_op(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        op: AppCommand,
    ) -> Result<()> {
        crate::session_log::log_outbound_op(&op);

        if self.try_handle_local_history_op(thread_id, &op).await? {
            return Ok(());
        }

        if self
            .try_resolve_app_server_request(app_server, thread_id, &op)
            .await?
        {
            return Ok(());
        }

        if self
            .try_submit_active_thread_op_via_app_server(app_server, thread_id, &op)
            .await?
        {
            if ThreadEventStore::op_can_change_pending_replay_state(&op) {
                self.note_thread_outbound_op(thread_id, &op).await;
                self.refresh_pending_thread_approvals().await;
            }
            return Ok(());
        }

        self.chat_widget
            .add_error_message(format!("Not available in TUI yet for thread {thread_id}."));
        Ok(())
    }

    /// Spawn a background task that fetches MCP server status from the app-server
    /// via paginated RPCs, then delivers the result back through
    /// `AppEvent::McpInventoryLoaded`.
    ///
    /// The spawned task is fire-and-forget: no `JoinHandle` is stored, so a stale
    /// result may arrive after the user has moved on. We currently accept that
    /// tradeoff because the effect is limited to stale inventory output in history,
    /// while request-token invalidation would add cross-cutting async state for a
    /// low-severity path.
    fn fetch_mcp_inventory(&mut self, app_server: &AppServerSession) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = fetch_all_mcp_server_statuses(request_handle)
                .await
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::McpInventoryLoaded { result });
        });
    }

    /// Spawns a background task to fetch account rate limits and deliver the
    /// result as a `RateLimitsLoaded` event.
    ///
    /// The `origin` is forwarded to the completion handler so it can distinguish
    /// a startup prefetch (which only updates cached snapshots and schedules a
    /// frame) from a `/status`-triggered refresh (which must finalize the
    /// corresponding status card).
    fn refresh_rate_limits(
        &mut self,
        app_server: &AppServerSession,
        origin: RateLimitRefreshOrigin,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = fetch_account_rate_limits(request_handle)
                .await
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::RateLimitsLoaded { origin, result });
        });
    }

    fn fetch_plugins_list(&mut self, app_server: &AppServerSession, cwd: PathBuf) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = fetch_plugins_list(request_handle, cwd.clone())
                .await
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::PluginsLoaded { cwd, result });
        });
    }

    fn fetch_plugin_detail(
        &mut self,
        app_server: &AppServerSession,
        cwd: PathBuf,
        params: PluginReadParams,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = fetch_plugin_detail(request_handle, params)
                .await
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::PluginDetailLoaded { cwd, result });
        });
    }

    fn fetch_plugin_install(
        &mut self,
        app_server: &AppServerSession,
        cwd: PathBuf,
        marketplace_path: AbsolutePathBuf,
        plugin_name: String,
        plugin_display_name: String,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let cwd_for_event = cwd.clone();
            let marketplace_path_for_event = marketplace_path.clone();
            let plugin_name_for_event = plugin_name.clone();
            let result = fetch_plugin_install(request_handle, marketplace_path, plugin_name)
                .await
                .map_err(|err| format!("Failed to install plugin: {err}"));
            app_event_tx.send(AppEvent::PluginInstallLoaded {
                cwd: cwd_for_event,
                marketplace_path: marketplace_path_for_event,
                plugin_name: plugin_name_for_event,
                plugin_display_name,
                result,
            });
        });
    }

    fn fetch_plugin_uninstall(
        &mut self,
        app_server: &AppServerSession,
        cwd: PathBuf,
        plugin_id: String,
        plugin_display_name: String,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let cwd_for_event = cwd.clone();
            let plugin_id_for_event = plugin_id.clone();
            let result = fetch_plugin_uninstall(request_handle, plugin_id)
                .await
                .map_err(|err| format!("Failed to uninstall plugin: {err}"));
            app_event_tx.send(AppEvent::PluginUninstallLoaded {
                cwd: cwd_for_event,
                plugin_id: plugin_id_for_event,
                plugin_display_name,
                result,
            });
        });
    }

    fn refresh_plugin_mentions(&mut self) {
        let config = self.config.clone();
        let app_event_tx = self.app_event_tx.clone();
        if !config.features.enabled(Feature::Plugins) {
            app_event_tx.send(AppEvent::PluginMentionsLoaded { plugins: None });
            return;
        }

        tokio::spawn(async move {
            let plugins = PluginsManager::new(config.codex_home.to_path_buf())
                .plugins_for_config(&config)
                .await
                .capability_summaries()
                .to_vec();
            app_event_tx.send(AppEvent::PluginMentionsLoaded {
                plugins: Some(plugins),
            });
        });
    }

    fn submit_feedback(
        &mut self,
        app_server: &AppServerSession,
        category: FeedbackCategory,
        reason: Option<String>,
        turn_id: Option<String>,
        include_logs: bool,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        let origin_thread_id = self.chat_widget.thread_id();
        let rollout_path = if include_logs {
            self.chat_widget.rollout_path()
        } else {
            None
        };
        let params = build_feedback_upload_params(
            origin_thread_id,
            rollout_path,
            category,
            reason,
            turn_id,
            include_logs,
        );
        tokio::spawn(async move {
            let result = fetch_feedback_upload(request_handle, params)
                .await
                .map(|response| response.thread_id)
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::FeedbackSubmitted {
                origin_thread_id,
                category,
                include_logs,
                result,
            });
        });
    }

    fn handle_feedback_thread_event(&mut self, event: FeedbackThreadEvent) {
        match event.result {
            Ok(thread_id) => {
                self.chat_widget
                    .add_to_history(crate::bottom_pane::feedback_success_cell(
                        event.category,
                        event.include_logs,
                        &thread_id,
                        event.feedback_audience,
                    ))
            }
            Err(err) => self
                .chat_widget
                .add_to_history(history_cell::new_error_event(format!(
                    "Failed to upload feedback: {err}"
                ))),
        }
    }

    async fn enqueue_thread_feedback_event(
        &mut self,
        thread_id: ThreadId,
        event: FeedbackThreadEvent,
    ) {
        let (sender, store) = {
            let channel = self.ensure_thread_channel(thread_id);
            (channel.sender.clone(), Arc::clone(&channel.store))
        };

        let should_send = {
            let mut guard = store.lock().await;
            guard
                .buffer
                .push_back(ThreadBufferedEvent::FeedbackSubmission(event.clone()));
            if guard.buffer.len() > guard.capacity
                && let Some(removed) = guard.buffer.pop_front()
                && let ThreadBufferedEvent::Request(request) = &removed
            {
                guard
                    .pending_interactive_replay
                    .note_evicted_server_request(request);
            }
            guard.active
        };

        if should_send {
            match sender.try_send(ThreadBufferedEvent::FeedbackSubmission(event)) {
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
    }

    async fn handle_feedback_submitted(
        &mut self,
        origin_thread_id: Option<ThreadId>,
        category: FeedbackCategory,
        include_logs: bool,
        result: Result<String, String>,
    ) {
        let event = FeedbackThreadEvent {
            category,
            include_logs,
            feedback_audience: self.feedback_audience,
            result,
        };
        if let Some(thread_id) = origin_thread_id {
            self.enqueue_thread_feedback_event(thread_id, event).await;
        } else {
            self.handle_feedback_thread_event(event);
        }
    }

    /// Process the completed MCP inventory fetch: clear the loading spinner, then
    /// render either the full tool/resource listing or an error into chat history.
    ///
    /// When both the local config and the app-server report zero servers, a special
    /// "empty" cell is shown instead of the full table.
    fn handle_mcp_inventory_result(&mut self, result: Result<Vec<McpServerStatus>, String>) {
        let config = self.chat_widget.config_ref().clone();
        self.chat_widget.clear_mcp_inventory_loading();
        self.clear_committed_mcp_inventory_loading();

        let statuses = match result {
            Ok(statuses) => statuses,
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to load MCP inventory: {err}"));
                return;
            }
        };

        if config.mcp_servers.get().is_empty() && statuses.is_empty() {
            self.chat_widget
                .add_to_history(history_cell::empty_mcp_output());
            return;
        }

        self.chat_widget
            .add_to_history(history_cell::new_mcp_tools_output_from_statuses(
                &config,
                &statuses,
                McpServerStatusDetail::ToolsAndAuthOnly,
            ));
    }

    fn clear_committed_mcp_inventory_loading(&mut self) {
        let Some(index) = self
            .transcript_cells
            .iter()
            .rposition(|cell| cell.as_any().is::<history_cell::McpInventoryLoadingCell>())
        else {
            return;
        };

        self.transcript_cells.remove(index);
        if let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
            overlay.replace_cells(self.transcript_cells.clone());
        }
    }

    /// Intercept composer-history operations and handle them locally against
    /// `$CODEX_HOME/history.jsonl`, bypassing the app-server RPC layer.
    async fn try_handle_local_history_op(
        &mut self,
        thread_id: ThreadId,
        op: &AppCommand,
    ) -> Result<bool> {
        match op.view() {
            AppCommandView::Other(Op::AddToHistory { text }) => {
                let text = text.clone();
                let config = self.chat_widget.config_ref().clone();
                tokio::spawn(async move {
                    if let Err(err) = append_message_history_entry(&text, &thread_id, &config).await
                    {
                        tracing::warn!(
                            thread_id = %thread_id,
                            error = %err,
                            "failed to append to message history"
                        );
                    }
                });
                Ok(true)
            }
            AppCommandView::Other(Op::GetHistoryEntryRequest { offset, log_id }) => {
                let offset = *offset;
                let log_id = *log_id;
                let config = self.chat_widget.config_ref().clone();
                let app_event_tx = self.app_event_tx.clone();
                tokio::spawn(async move {
                    let entry_opt = tokio::task::spawn_blocking(move || {
                        lookup_message_history_entry(log_id, offset, &config)
                    })
                    .await
                    .unwrap_or_else(|err| {
                        tracing::warn!(error = %err, "history lookup task failed");
                        None
                    });

                    app_event_tx.send(AppEvent::ThreadHistoryEntryResponse {
                        thread_id,
                        event: GetHistoryEntryResponseEvent {
                            offset,
                            log_id,
                            entry: entry_opt.map(|entry| {
                                codex_protocol::message_history::HistoryEntry {
                                    conversation_id: entry.session_id,
                                    ts: entry.ts,
                                    text: entry.text,
                                }
                            }),
                        },
                    });
                });
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn try_submit_active_thread_op_via_app_server(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        op: &AppCommand,
    ) -> Result<bool> {
        match op.view() {
            AppCommandView::Interrupt => {
                if let Some(turn_id) = self.active_turn_id_for_thread(thread_id).await {
                    app_server.turn_interrupt(thread_id, turn_id).await?;
                } else {
                    app_server.startup_interrupt(thread_id).await?;
                }
                Ok(true)
            }
            AppCommandView::UserTurn {
                items,
                cwd,
                approval_policy,
                approvals_reviewer,
                sandbox_policy,
                model,
                effort,
                summary,
                service_tier,
                final_output_json_schema,
                collaboration_mode,
                personality,
            } => {
                let mut should_start_turn = true;
                if let Some(turn_id) = self.active_turn_id_for_thread(thread_id).await {
                    let mut steer_turn_id = turn_id;
                    let mut retried_after_turn_mismatch = false;
                    loop {
                        match app_server
                            .turn_steer(thread_id, steer_turn_id.clone(), items.to_vec())
                            .await
                        {
                            Ok(_) => return Ok(true),
                            Err(error) => {
                                if let Some(turn_error) =
                                    active_turn_not_steerable_turn_error(&error)
                                {
                                    if !self.chat_widget.enqueue_rejected_steer() {
                                        self.chat_widget.add_error_message(turn_error.message);
                                    }
                                    return Ok(true);
                                }
                                match active_turn_steer_race(&error) {
                                    Some(ActiveTurnSteerRace::Missing) => {
                                        if let Some(channel) =
                                            self.thread_event_channels.get(&thread_id)
                                        {
                                            let mut store = channel.store.lock().await;
                                            store.clear_active_turn_id();
                                        }
                                        should_start_turn = true;
                                        break;
                                    }
                                    Some(ActiveTurnSteerRace::ExpectedTurnMismatch {
                                        actual_turn_id,
                                    }) if !retried_after_turn_mismatch
                                        && actual_turn_id != steer_turn_id =>
                                    {
                                        // Review flows can swap the active turn before the TUI
                                        // processes the corresponding notification. Retry once with
                                        // the server-reported turn id so non-steerable review turns
                                        // still fall through to the existing queueing behavior.
                                        if let Some(channel) =
                                            self.thread_event_channels.get(&thread_id)
                                        {
                                            let mut store = channel.store.lock().await;
                                            store.active_turn_id = Some(actual_turn_id.clone());
                                        }
                                        steer_turn_id = actual_turn_id;
                                        retried_after_turn_mismatch = true;
                                    }
                                    Some(ActiveTurnSteerRace::ExpectedTurnMismatch {
                                        actual_turn_id,
                                    }) => {
                                        if let Some(channel) =
                                            self.thread_event_channels.get(&thread_id)
                                        {
                                            let mut store = channel.store.lock().await;
                                            store.active_turn_id = Some(actual_turn_id);
                                        }
                                        return Err(error.into());
                                    }
                                    None => return Err(error.into()),
                                }
                            }
                        }
                    }
                }
                if should_start_turn {
                    app_server
                        .turn_start(
                            thread_id,
                            items.to_vec(),
                            cwd.clone(),
                            approval_policy,
                            approvals_reviewer
                                .unwrap_or(self.chat_widget.config_ref().approvals_reviewer),
                            sandbox_policy.clone(),
                            model.to_string(),
                            effort,
                            *summary,
                            *service_tier,
                            collaboration_mode.clone(),
                            *personality,
                            final_output_json_schema.clone(),
                        )
                        .await?;
                }
                Ok(true)
            }
            AppCommandView::ListSkills { cwds, force_reload } => {
                let response = app_server
                    .skills_list(codex_app_server_protocol::SkillsListParams {
                        cwds: cwds.to_vec(),
                        force_reload,
                        per_cwd_extra_user_roots: None,
                    })
                    .await?;
                self.handle_skills_list_response(response);
                Ok(true)
            }
            AppCommandView::Compact => {
                app_server.thread_compact_start(thread_id).await?;
                Ok(true)
            }
            AppCommandView::SetThreadName { name } => {
                app_server
                    .thread_set_name(thread_id, name.to_string())
                    .await?;
                Ok(true)
            }
            AppCommandView::ThreadRollback { num_turns } => {
                let response = match app_server.thread_rollback(thread_id, num_turns).await {
                    Ok(response) => response,
                    Err(err) => {
                        self.handle_backtrack_rollback_failed();
                        return Err(err);
                    }
                };
                self.handle_thread_rollback_response(thread_id, num_turns, &response)
                    .await;
                Ok(true)
            }
            AppCommandView::Review { review_request } => {
                app_server
                    .review_start(thread_id, review_request.clone())
                    .await?;
                Ok(true)
            }
            AppCommandView::CleanBackgroundTerminals => {
                app_server
                    .thread_background_terminals_clean(thread_id)
                    .await?;
                Ok(true)
            }
            AppCommandView::RealtimeConversationStart(params) => {
                app_server
                    .thread_realtime_start(thread_id, params.clone())
                    .await?;
                Ok(true)
            }
            AppCommandView::RealtimeConversationAudio(params) => {
                app_server
                    .thread_realtime_audio(thread_id, params.clone())
                    .await?;
                Ok(true)
            }
            AppCommandView::RealtimeConversationText(params) => {
                app_server
                    .thread_realtime_text(thread_id, params.clone())
                    .await?;
                Ok(true)
            }
            AppCommandView::RealtimeConversationClose => {
                app_server.thread_realtime_stop(thread_id).await?;
                Ok(true)
            }
            AppCommandView::RunUserShellCommand { command } => {
                app_server
                    .thread_shell_command(thread_id, command.to_string())
                    .await?;
                Ok(true)
            }
            AppCommandView::ReloadUserConfig => {
                app_server.reload_user_config().await?;
                Ok(true)
            }
            AppCommandView::OverrideTurnContext { .. } => Ok(true),
            _ => Ok(false),
        }
    }

    async fn try_resolve_app_server_request(
        &mut self,
        app_server: &AppServerSession,
        thread_id: ThreadId,
        op: &AppCommand,
    ) -> Result<bool> {
        let Some(resolution) = self
            .pending_app_server_requests
            .take_resolution(op)
            .map_err(|err| color_eyre::eyre::eyre!(err))?
        else {
            return Ok(false);
        };

        match app_server
            .resolve_server_request(resolution.request_id, resolution.result)
            .await
        {
            Ok(()) => {
                if ThreadEventStore::op_can_change_pending_replay_state(op) {
                    self.note_thread_outbound_op(thread_id, op).await;
                    self.refresh_pending_thread_approvals().await;
                }
                Ok(true)
            }
            Err(err) => {
                self.chat_widget.add_error_message(format!(
                    "Failed to resolve app-server request for thread {thread_id}: {err}"
                ));
                Ok(false)
            }
        }
    }

    async fn refresh_pending_thread_approvals(&mut self) {
        let channels: Vec<(ThreadId, Arc<Mutex<ThreadEventStore>>)> = self
            .thread_event_channels
            .iter()
            .map(|(thread_id, channel)| (*thread_id, Arc::clone(&channel.store)))
            .collect();

        let mut pending_thread_ids = Vec::new();
        for (thread_id, store) in channels {
            if Some(thread_id) == self.active_thread_id {
                continue;
            }

            let store = store.lock().await;
            if store.has_pending_thread_approvals() {
                pending_thread_ids.push(thread_id);
            }
        }

        pending_thread_ids.sort_by_key(ThreadId::to_string);

        let threads = pending_thread_ids
            .into_iter()
            .map(|thread_id| self.thread_label(thread_id))
            .collect();

        self.chat_widget.set_pending_thread_approvals(threads);
    }

    async fn enqueue_thread_notification(
        &mut self,
        thread_id: ThreadId,
        notification: ServerNotification,
    ) -> Result<()> {
        let inferred_session = self
            .infer_session_for_thread_notification(thread_id, &notification)
            .await;
        let (sender, store) = {
            let channel = self.ensure_thread_channel(thread_id);
            (channel.sender.clone(), Arc::clone(&channel.store))
        };

        let should_send = {
            let mut guard = store.lock().await;
            if guard.session.is_none()
                && let Some(session) = inferred_session
            {
                guard.session = Some(session);
            }
            guard.push_notification(notification.clone());
            guard.active
        };

        if should_send {
            match sender.try_send(ThreadBufferedEvent::Notification(notification)) {
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
        self.refresh_pending_thread_approvals().await;
        Ok(())
    }

    /// Eagerly fetches nickname and role for receiver threads referenced by a collab notification.
    ///
    /// This runs on every buffered thread notification before it reaches rendering. For each
    /// receiver thread id that the navigation cache does not yet have metadata for, it issues a
    /// `thread/read` RPC and registers the result in both `AgentNavigationState` and the
    /// `ChatWidget` metadata map. Threads that already have a nickname or role cached are skipped,
    /// so the cost is at most one RPC per thread over the lifetime of a session.
    ///
    /// Failures are logged and silently ignored -- the worst outcome is that a rendered item shows
    /// a thread id instead of a human-readable name, which is the same behavior the TUI had before
    /// this change.
    async fn hydrate_collab_agent_metadata_for_notification(
        &mut self,
        app_server: &mut AppServerSession,
        notification: &ServerNotification,
    ) {
        let Some(receiver_thread_ids) = collab_receiver_thread_ids(notification) else {
            return;
        };

        for receiver_thread_id in receiver_thread_ids {
            let Ok(thread_id) = ThreadId::from_string(receiver_thread_id) else {
                tracing::warn!(
                    thread_id = receiver_thread_id,
                    "ignoring collab receiver with invalid thread id during metadata hydration"
                );
                continue;
            };

            if self
                .agent_navigation
                .get(&thread_id)
                .is_some_and(|entry| entry.agent_nickname.is_some() || entry.agent_role.is_some())
            {
                continue;
            }

            match app_server
                .thread_read(thread_id, /*include_turns*/ false)
                .await
            {
                Ok(thread) => {
                    self.upsert_agent_picker_thread(
                        thread_id,
                        thread.agent_nickname,
                        thread.agent_role,
                        /*is_closed*/ false,
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        thread_id = %thread_id,
                        error = %err,
                        "failed to hydrate collab receiver thread metadata"
                    );
                }
            }
        }
    }

    async fn infer_session_for_thread_notification(
        &mut self,
        thread_id: ThreadId,
        notification: &ServerNotification,
    ) -> Option<ThreadSessionState> {
        let ServerNotification::ThreadStarted(notification) = notification else {
            return None;
        };
        let mut session = self.primary_session_configured.clone()?;
        session.thread_id = thread_id;
        session.thread_name = notification.thread.name.clone();
        session.model_provider_id = notification.thread.model_provider.clone();
        session.cwd = notification.thread.cwd.clone();
        let rollout_path = notification.thread.path.clone();
        if let Some(model) =
            read_session_model(&self.config, thread_id, rollout_path.as_deref()).await
        {
            session.model = model;
        } else if rollout_path.is_some() {
            session.model.clear();
        }
        session.history_log_id = 0;
        session.history_entry_count = 0;
        session.rollout_path = rollout_path;
        self.upsert_agent_picker_thread(
            thread_id,
            notification.thread.agent_nickname.clone(),
            notification.thread.agent_role.clone(),
            /*is_closed*/ false,
        );
        Some(session)
    }

    async fn enqueue_thread_request(
        &mut self,
        thread_id: ThreadId,
        request: ServerRequest,
    ) -> Result<()> {
        let inactive_interactive_request = if self.active_thread_id != Some(thread_id) {
            self.interactive_request_for_thread_request(thread_id, &request)
                .await
        } else {
            None
        };
        let (sender, store) = {
            let channel = self.ensure_thread_channel(thread_id);
            (channel.sender.clone(), Arc::clone(&channel.store))
        };

        let should_send = {
            let mut guard = store.lock().await;
            guard.push_request(request.clone());
            guard.active
        };

        if should_send {
            match sender.try_send(ThreadBufferedEvent::Request(request)) {
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
        } else if let Some(request) = inactive_interactive_request {
            match request {
                ThreadInteractiveRequest::Approval(request) => {
                    self.chat_widget.push_approval_request(request);
                }
                ThreadInteractiveRequest::McpServerElicitation(request) => {
                    self.chat_widget
                        .push_mcp_server_elicitation_request(request);
                }
            }
        }
        self.refresh_pending_thread_approvals().await;
        Ok(())
    }

    async fn enqueue_thread_history_entry_response(
        &mut self,
        thread_id: ThreadId,
        event: GetHistoryEntryResponseEvent,
    ) -> Result<()> {
        let (sender, store) = {
            let channel = self.ensure_thread_channel(thread_id);
            (channel.sender.clone(), Arc::clone(&channel.store))
        };

        let should_send = {
            let mut guard = store.lock().await;
            guard
                .buffer
                .push_back(ThreadBufferedEvent::HistoryEntryResponse(event.clone()));
            if guard.buffer.len() > guard.capacity
                && let Some(removed) = guard.buffer.pop_front()
                && let ThreadBufferedEvent::Request(request) = &removed
            {
                guard
                    .pending_interactive_replay
                    .note_evicted_server_request(request);
            }
            guard.active
        };

        if should_send {
            match sender.try_send(ThreadBufferedEvent::HistoryEntryResponse(event)) {
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

    async fn enqueue_primary_thread_session(
        &mut self,
        session: ThreadSessionState,
        turns: Vec<Turn>,
    ) -> Result<()> {
        let thread_id = session.thread_id;
        self.primary_thread_id = Some(thread_id);
        self.primary_session_configured = Some(session.clone());
        self.upsert_agent_picker_thread(
            thread_id, /*agent_nickname*/ None, /*agent_role*/ None,
            /*is_closed*/ false,
        );
        let channel = self.ensure_thread_channel(thread_id);
        {
            let mut store = channel.store.lock().await;
            store.set_session(session.clone(), turns.clone());
        }
        self.activate_thread_channel(thread_id).await;
        self.chat_widget
            .set_initial_user_message_submit_suppressed(/*suppressed*/ true);
        self.chat_widget.handle_thread_session(session);
        self.chat_widget
            .replay_thread_turns(turns, ReplayKind::ResumeInitialMessages);
        let pending = std::mem::take(&mut self.pending_primary_events);
        for pending_event in pending {
            match pending_event {
                ThreadBufferedEvent::Notification(notification) => {
                    self.enqueue_thread_notification(thread_id, notification)
                        .await?;
                }
                ThreadBufferedEvent::Request(request) => {
                    self.enqueue_thread_request(thread_id, request).await?;
                }
                ThreadBufferedEvent::HistoryEntryResponse(event) => {
                    self.enqueue_thread_history_entry_response(thread_id, event)
                        .await?;
                }
                ThreadBufferedEvent::FeedbackSubmission(event) => {
                    self.enqueue_thread_feedback_event(thread_id, event).await;
                }
            }
        }
        self.chat_widget
            .set_initial_user_message_submit_suppressed(/*suppressed*/ false);
        self.chat_widget.submit_initial_user_message_if_pending();
        Ok(())
    }

    async fn enqueue_primary_thread_notification(
        &mut self,
        notification: ServerNotification,
    ) -> Result<()> {
        if let Some(thread_id) = self.primary_thread_id {
            return self
                .enqueue_thread_notification(thread_id, notification)
                .await;
        }
        self.pending_primary_events
            .push_back(ThreadBufferedEvent::Notification(notification));
        Ok(())
    }

    async fn enqueue_primary_thread_request(&mut self, request: ServerRequest) -> Result<()> {
        if let Some(thread_id) = self.primary_thread_id {
            return self.enqueue_thread_request(thread_id, request).await;
        }
        self.pending_primary_events
            .push_back(ThreadBufferedEvent::Request(request));
        Ok(())
    }

    async fn refresh_snapshot_session_if_needed(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        is_replay_only: bool,
        snapshot: &mut ThreadEventSnapshot,
    ) {
        let should_refresh = !is_replay_only
            && snapshot.session.as_ref().is_none_or(|session| {
                session.model.trim().is_empty() || session.rollout_path.is_none()
            });
        if !should_refresh {
            return;
        }

        match app_server
            .resume_thread(self.config.clone(), thread_id)
            .await
        {
            Ok(started) => {
                self.apply_refreshed_snapshot_thread(thread_id, started, snapshot)
                    .await
            }
            Err(err) => {
                tracing::warn!(
                    thread_id = %thread_id,
                    error = %err,
                    "failed to refresh inferred thread session before replay"
                );
            }
        }
    }

    async fn apply_refreshed_snapshot_thread(
        &mut self,
        thread_id: ThreadId,
        started: AppServerStartedThread,
        snapshot: &mut ThreadEventSnapshot,
    ) {
        let AppServerStartedThread { session, turns } = started;
        if let Some(channel) = self.thread_event_channels.get(&thread_id) {
            let mut store = channel.store.lock().await;
            store.set_session(session.clone(), turns.clone());
            store.rebase_buffer_after_session_refresh();
        }
        snapshot.session = Some(session);
        snapshot.turns = turns;
        snapshot
            .events
            .retain(ThreadEventStore::event_survives_session_refresh);
    }

    /// Opens the `/agent` picker after refreshing cached labels for known threads.
    ///
    /// The picker state is derived from long-lived thread channels plus best-effort metadata
    /// refreshes from the backend. Refresh failures are treated as "thread is only inspectable by
    /// historical id now" and converted into closed picker entries instead of deleting them, so
    /// the stable traversal order remains intact for review and keyboard navigation.
    async fn open_agent_picker(&mut self, app_server: &mut AppServerSession) {
        let mut thread_ids = self.agent_navigation.tracked_thread_ids();
        for thread_id in self.thread_event_channels.keys().copied() {
            if !thread_ids.contains(&thread_id) {
                thread_ids.push(thread_id);
            }
        }
        for thread_id in thread_ids {
            if !self
                .refresh_agent_picker_thread_liveness(app_server, thread_id)
                .await
            {
                continue;
            }
        }

        let has_non_primary_agent_thread = self
            .agent_navigation
            .has_non_primary_thread(self.primary_thread_id);
        if !self.config.features.enabled(Feature::Collab) && !has_non_primary_agent_thread {
            self.chat_widget.open_multi_agent_enable_prompt();
            return;
        }

        if self.agent_navigation.is_empty() {
            self.chat_widget
                .add_info_message("No agents available yet.".to_string(), /*hint*/ None);
            return;
        }

        let mut initial_selected_idx = None;
        let items: Vec<SelectionItem> = self
            .agent_navigation
            .ordered_threads()
            .iter()
            .enumerate()
            .map(|(idx, (thread_id, entry))| {
                if self.active_thread_id == Some(*thread_id) {
                    initial_selected_idx = Some(idx);
                }
                let id = *thread_id;
                let is_primary = self.primary_thread_id == Some(*thread_id);
                let name = format_agent_picker_item_name(
                    entry.agent_nickname.as_deref(),
                    entry.agent_role.as_deref(),
                    is_primary,
                );
                let uuid = thread_id.to_string();
                SelectionItem {
                    name: name.clone(),
                    name_prefix_spans: agent_picker_status_dot_spans(entry.is_closed),
                    description: Some(uuid.clone()),
                    is_current: self.active_thread_id == Some(*thread_id),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::SelectAgentThread(id));
                    })],
                    dismiss_on_select: true,
                    search_value: Some(format!("{name} {uuid}")),
                    ..Default::default()
                }
            })
            .collect();

        self.chat_widget.show_selection_view(SelectionViewParams {
            title: Some("Subagents".to_string()),
            subtitle: Some(AgentNavigationState::picker_subtitle()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            initial_selected_idx,
            ..Default::default()
        });
    }

    fn is_terminal_thread_read_error(err: &color_eyre::Report) -> bool {
        err.chain()
            .any(|cause| cause.to_string().contains("thread not loaded:"))
    }

    fn closed_state_for_thread_read_error(
        err: &color_eyre::Report,
        existing_is_closed: Option<bool>,
    ) -> bool {
        Self::is_terminal_thread_read_error(err) || existing_is_closed.unwrap_or(false)
    }

    fn can_fallback_from_include_turns_error(err: &color_eyre::Report) -> bool {
        err.chain().any(|cause| {
            let message = cause.to_string();
            message.contains("includeTurns is unavailable before first user message")
                || message.contains("ephemeral threads do not support includeTurns")
        })
    }

    /// Updates cached picker metadata and then mirrors any visible-label change into the footer.
    ///
    /// These two writes stay paired so the picker rows and contextual footer continue to describe
    /// the same displayed thread after nickname or role updates.
    fn upsert_agent_picker_thread(
        &mut self,
        thread_id: ThreadId,
        agent_nickname: Option<String>,
        agent_role: Option<String>,
        is_closed: bool,
    ) {
        self.chat_widget.set_collab_agent_metadata(
            thread_id,
            agent_nickname.clone(),
            agent_role.clone(),
        );
        self.agent_navigation
            .upsert(thread_id, agent_nickname, agent_role, is_closed);
        self.sync_active_agent_label();
    }

    /// Marks a cached picker thread closed and recomputes the contextual footer label.
    ///
    /// Closing a thread is not the same as removing it: users can still inspect finished agent
    /// transcripts, and the stable next/previous traversal order should not collapse around them.
    fn mark_agent_picker_thread_closed(&mut self, thread_id: ThreadId) {
        self.agent_navigation.mark_closed(thread_id);
        self.sync_active_agent_label();
    }

    async fn refresh_agent_picker_thread_liveness(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> bool {
        let existing_entry = self.agent_navigation.get(&thread_id).cloned();
        let has_replay_channel = self.thread_event_channels.contains_key(&thread_id);
        match app_server
            .thread_read(thread_id, /*include_turns*/ false)
            .await
        {
            Ok(thread) => {
                self.upsert_agent_picker_thread(
                    thread_id,
                    thread.agent_nickname.or_else(|| {
                        existing_entry
                            .as_ref()
                            .and_then(|entry| entry.agent_nickname.clone())
                    }),
                    thread.agent_role.or_else(|| {
                        existing_entry
                            .as_ref()
                            .and_then(|entry| entry.agent_role.clone())
                    }),
                    matches!(
                        thread.status,
                        codex_app_server_protocol::ThreadStatus::NotLoaded
                    ),
                );
                true
            }
            Err(err) => {
                if Self::is_terminal_thread_read_error(&err) && !has_replay_channel {
                    self.agent_navigation.remove(thread_id);
                    return false;
                }
                let is_closed = Self::closed_state_for_thread_read_error(
                    &err,
                    existing_entry.as_ref().map(|entry| entry.is_closed),
                );
                if let Some(entry) = existing_entry {
                    self.upsert_agent_picker_thread(
                        thread_id,
                        entry.agent_nickname,
                        entry.agent_role,
                        is_closed,
                    );
                } else {
                    self.upsert_agent_picker_thread(
                        thread_id, /*agent_nickname*/ None, /*agent_role*/ None,
                        is_closed,
                    );
                }
                true
            }
        }
    }

    async fn session_state_for_thread_read(
        &self,
        thread_id: ThreadId,
        thread: &codex_app_server_protocol::Thread,
    ) -> ThreadSessionState {
        let mut session = self
            .primary_session_configured
            .clone()
            .unwrap_or(ThreadSessionState {
                thread_id,
                forked_from_id: None,
                thread_name: None,
                model: self.chat_widget.current_model().to_string(),
                model_provider_id: self.config.model_provider_id.clone(),
                service_tier: self.chat_widget.current_service_tier(),
                approval_policy: self.config.permissions.approval_policy.value(),
                approvals_reviewer: self.config.approvals_reviewer,
                sandbox_policy: self.config.permissions.sandbox_policy.get().clone(),
                cwd: thread.cwd.clone(),
                instruction_source_paths: Vec::new(),
                reasoning_effort: self.chat_widget.current_reasoning_effort(),
                history_log_id: 0,
                history_entry_count: 0,
                network_proxy: None,
                rollout_path: thread.path.clone(),
            });
        session.thread_id = thread_id;
        session.thread_name = thread.name.clone();
        session.model_provider_id = thread.model_provider.clone();
        session.cwd = thread.cwd.clone();
        session.instruction_source_paths = Vec::new();
        session.rollout_path = thread.path.clone();
        if let Some(model) =
            read_session_model(&self.config, thread_id, thread.path.as_deref()).await
        {
            session.model = model;
        } else if thread.path.is_some() {
            session.model.clear();
        }
        session.history_log_id = 0;
        session.history_entry_count = 0;
        session
    }

    /// Materializes a live thread into local replay state when the picker knows about it but the
    /// TUI has not cached a local event channel yet.
    ///
    /// Resume-time backfill intentionally avoids creating empty placeholder channels, because those
    /// placeholders make stale `/agent` entries open blank transcripts. When a user later selects a
    /// still-live discovered thread, attach it on demand with a real resumed snapshot.
    async fn attach_live_thread_for_selection(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> Result<bool> {
        if self.thread_event_channels.contains_key(&thread_id) {
            return Ok(true);
        }

        let (session, turns, live_attached) = match app_server
            .resume_thread(self.config.clone(), thread_id)
            .await
        {
            Ok(started) => (started.session, started.turns, true),
            Err(resume_err) => {
                tracing::warn!(
                    thread_id = %thread_id,
                    error = %resume_err,
                    "failed to resume live thread for selection; falling back to thread/read"
                );
                let (thread, turns) = match app_server
                    .thread_read(thread_id, /*include_turns*/ true)
                    .await
                {
                    Ok(thread) => {
                        let turns = thread.turns.clone();
                        (thread, turns)
                    }
                    Err(err) if Self::can_fallback_from_include_turns_error(&err) => {
                        let thread = app_server
                            .thread_read(thread_id, /*include_turns*/ false)
                            .await?;
                        (thread, Vec::new())
                    }
                    Err(err) => return Err(err),
                };
                if turns.is_empty() {
                    // A `thread/read` fallback without turns would create a blank local replay
                    // channel with no live listener attached, which blocks later real re-attach.
                    return Err(color_eyre::eyre::eyre!(
                        "Agent thread {thread_id} is not yet available for replay or live attach."
                    ));
                }
                let mut session = self.session_state_for_thread_read(thread_id, &thread).await;
                // `thread/read` can seed replay state, but it does not attach the app-server
                // listener that `thread/resume` establishes, so treat this path as replay-only.
                session.model.clear();
                (session, turns, false)
            }
        };
        let channel = self.ensure_thread_channel(thread_id);
        let mut store = channel.store.lock().await;
        store.set_session(session, turns);
        Ok(live_attached)
    }

    /// Replaces the chat widget and re-seeds the new widget's collab metadata from the navigation
    /// cache.
    ///
    /// Thread switches reconstruct the `ChatWidget`, which loses the `collab_agent_metadata` map.
    /// This helper copies every known nickname/role from `AgentNavigationState` into the
    /// replacement widget so that replayed collab items render agent names immediately.
    fn replace_chat_widget(&mut self, mut chat_widget: ChatWidget) {
        // Transfer the last-written terminal title to the replacement widget
        // so it knows what OSC title is currently displayed. Without this, the
        // new widget would redundantly clear and rewrite the same title, causing
        // a visible flicker in some terminals.
        let previous_terminal_title = self.chat_widget.last_terminal_title.take();
        if chat_widget.last_terminal_title.is_none() {
            chat_widget.last_terminal_title = previous_terminal_title;
        }
        for (thread_id, entry) in self.agent_navigation.ordered_threads() {
            chat_widget.set_collab_agent_metadata(
                thread_id,
                entry.agent_nickname.clone(),
                entry.agent_role.clone(),
            );
        }
        self.chat_widget = chat_widget;
        self.sync_active_agent_label();
    }

    async fn select_agent_thread(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> Result<()> {
        if self.active_thread_id == Some(thread_id) {
            return Ok(());
        }

        if !self
            .refresh_agent_picker_thread_liveness(app_server, thread_id)
            .await
        {
            self.chat_widget
                .add_error_message(format!("Agent thread {thread_id} is no longer available."));
            return Ok(());
        }

        let mut is_replay_only = self
            .agent_navigation
            .get(&thread_id)
            .is_some_and(|entry| entry.is_closed);
        let mut attached_replay_only = false;
        if self.should_attach_live_thread_for_selection(thread_id) {
            match self
                .attach_live_thread_for_selection(app_server, thread_id)
                .await
            {
                Ok(live_attached) => {
                    attached_replay_only = !live_attached;
                    if attached_replay_only {
                        is_replay_only = true;
                    }
                }
                Err(err) => {
                    self.chat_widget.add_error_message(format!(
                        "Failed to attach to agent thread {thread_id}: {err}"
                    ));
                    return Ok(());
                }
            }
        } else if !self.thread_event_channels.contains_key(&thread_id) && is_replay_only {
            self.chat_widget
                .add_error_message(format!("Agent thread {thread_id} is no longer available."));
            return Ok(());
        }

        let previous_thread_id = self.active_thread_id;
        self.store_active_thread_receiver().await;
        self.active_thread_id = None;
        let Some((receiver, mut snapshot)) = self.activate_thread_for_replay(thread_id).await
        else {
            self.chat_widget
                .add_error_message(format!("Agent thread {thread_id} is already active."));
            if let Some(previous_thread_id) = previous_thread_id {
                self.activate_thread_channel(previous_thread_id).await;
            }
            return Ok(());
        };

        self.refresh_snapshot_session_if_needed(
            app_server,
            thread_id,
            is_replay_only,
            &mut snapshot,
        )
        .await;

        self.active_thread_id = Some(thread_id);
        self.active_thread_rx = Some(receiver);

        let init = self.chatwidget_init_for_forked_or_resumed_thread(tui, self.config.clone());
        self.replace_chat_widget(ChatWidget::new_with_app_event(init));

        self.reset_for_thread_switch(tui)?;
        self.replay_thread_snapshot(snapshot, !is_replay_only);
        if is_replay_only {
            let message = if attached_replay_only {
                format!(
                    "Agent thread {thread_id} could not be resumed live. Replaying saved transcript."
                )
            } else {
                format!("Agent thread {thread_id} is closed. Replaying saved transcript.")
            };
            self.chat_widget.add_info_message(message, /*hint*/ None);
        }
        self.drain_active_thread_events(tui).await?;
        self.refresh_pending_thread_approvals().await;

        Ok(())
    }

    fn should_attach_live_thread_for_selection(&self, thread_id: ThreadId) -> bool {
        !self.thread_event_channels.contains_key(&thread_id)
            && self
                .agent_navigation
                .get(&thread_id)
                .is_none_or(|entry| !entry.is_closed)
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
        self.abort_all_thread_event_listeners();
        self.thread_event_channels.clear();
        self.agent_navigation.clear();
        self.active_thread_id = None;
        self.active_thread_rx = None;
        self.primary_thread_id = None;
        self.last_subagent_backfill_attempt = None;
        self.primary_session_configured = None;
        self.pending_primary_events.clear();
        self.pending_app_server_requests.clear();
        self.chat_widget.set_pending_thread_approvals(Vec::new());
        self.sync_active_agent_label();
    }

    async fn start_fresh_session_with_summary_hint(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        session_start_source: Option<ThreadStartSource>,
    ) {
        // Start a fresh in-memory session while preserving resumability via persisted rollout
        // history.
        self.refresh_in_memory_config_from_disk_best_effort("starting a new thread")
            .await;
        let model = self.chat_widget.current_model().to_string();
        let config = self.fresh_session_config();
        let summary = session_summary(
            self.chat_widget.token_usage(),
            self.chat_widget.thread_id(),
            self.chat_widget.thread_name(),
        );
        self.shutdown_current_thread(app_server).await;
        let tracked_thread_ids: Vec<ThreadId> =
            self.thread_event_channels.keys().copied().collect();
        for thread_id in tracked_thread_ids {
            if let Err(err) = app_server.thread_unsubscribe(thread_id).await {
                tracing::warn!("failed to unsubscribe tracked thread {thread_id}: {err}");
            }
        }
        self.config = config.clone();
        match app_server
            .start_thread_with_session_start_source(&config, session_start_source)
            .await
        {
            Ok(started) => {
                if let Err(err) = self
                    .replace_chat_widget_with_app_server_thread(tui, app_server, started)
                    .await
                {
                    self.chat_widget.add_error_message(format!(
                        "Failed to attach to fresh app-server thread: {err}"
                    ));
                } else if let Some(summary) = summary {
                    let mut lines: Vec<Line<'static>> = Vec::new();
                    if let Some(usage_line) = summary.usage_line {
                        lines.push(usage_line.into());
                    }
                    if let Some(command) = summary.resume_command {
                        let spans = vec!["To continue this session, run ".into(), command.cyan()];
                        lines.push(spans.into());
                    }
                    self.chat_widget.add_plain_history_lines(lines);
                }
            }
            Err(err) => {
                self.chat_widget.add_error_message(format!(
                    "Failed to start a fresh session through the app server: {err}"
                ));
                self.config.model = Some(model);
            }
        }
        tui.frame_requester().schedule_frame();
    }

    async fn replace_chat_widget_with_app_server_thread(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        started: AppServerStartedThread,
    ) -> Result<()> {
        self.reset_thread_event_state();
        let init = self.chatwidget_init_for_forked_or_resumed_thread(tui, self.config.clone());
        self.replace_chat_widget(ChatWidget::new_with_app_event(init));
        self.enqueue_primary_thread_session(started.session, started.turns)
            .await?;
        self.backfill_loaded_subagent_threads(app_server).await;
        Ok(())
    }

    /// Fetches all loaded threads from the app server and registers descendants of the primary
    /// thread in the navigation cache and chat widget metadata.
    ///
    /// Called after `replace_chat_widget_with_app_server_thread` during resume, fork, and new
    /// thread creation so that the `/agent` picker and keyboard navigation are pre-populated even
    /// if the TUI did not witness the original spawn events.
    ///
    /// The loaded-thread list is fetched in full (no pagination) and the spawn tree is walked
    /// by `find_loaded_subagent_threads_for_primary`. Each discovered subagent is registered via
    /// `upsert_agent_picker_thread`, which writes to both `AgentNavigationState` and the
    /// `ChatWidget` metadata map.
    async fn backfill_loaded_subagent_threads(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> bool {
        let Some(primary_thread_id) = self.primary_thread_id else {
            return false;
        };

        let loaded_thread_ids = match app_server
            .thread_loaded_list(ThreadLoadedListParams {
                cursor: None,
                limit: None,
            })
            .await
        {
            Ok(response) => response.data,
            Err(err) => {
                tracing::warn!(%err, "failed to list loaded threads for subagent backfill");
                return false;
            }
        };

        let mut threads = Vec::new();
        let mut had_read_error = false;
        for thread_id in loaded_thread_ids {
            let Ok(thread_id) = ThreadId::from_string(&thread_id) else {
                tracing::warn!("ignoring loaded thread with invalid id during subagent backfill");
                continue;
            };

            if thread_id == primary_thread_id {
                continue;
            }

            match app_server
                .thread_read(thread_id, /*include_turns*/ false)
                .await
            {
                Ok(thread) => threads.push(thread),
                Err(err) => {
                    had_read_error = true;
                    tracing::warn!(thread_id = %thread_id, %err, "failed to read loaded thread");
                }
            }
        }

        for thread in find_loaded_subagent_threads_for_primary(threads, primary_thread_id) {
            self.upsert_agent_picker_thread(
                thread.thread_id,
                thread.agent_nickname,
                thread.agent_role,
                /*is_closed*/ false,
            );
        }

        !had_read_error
    }

    /// Returns the adjacent thread id for keyboard navigation, backfilling from the server if the
    /// local cache has no neighbor.
    ///
    /// Tries the fast path first: ask `AgentNavigationState` directly. If it returns `None` (no
    /// adjacent entry exists, typically because the cache was never populated with remote
    /// subagents), performs a full `backfill_loaded_subagent_threads` and retries. This ensures the
    /// first next/previous keypress in a resumed remote session discovers subagents on demand
    /// without requiring the user to wait for a proactive fetch.
    async fn adjacent_thread_id_with_backfill(
        &mut self,
        app_server: &mut AppServerSession,
        direction: AgentNavigationDirection,
    ) -> Option<ThreadId> {
        let current_thread = self.current_displayed_thread_id();
        if let Some(thread_id) = self
            .agent_navigation
            .adjacent_thread_id(current_thread, direction)
        {
            return Some(thread_id);
        }

        let primary_thread_id = self.primary_thread_id?;
        if self.last_subagent_backfill_attempt == Some(primary_thread_id) {
            return None;
        }

        if self.backfill_loaded_subagent_threads(app_server).await {
            self.last_subagent_backfill_attempt = Some(primary_thread_id);
        }
        self.agent_navigation
            .adjacent_thread_id(self.current_displayed_thread_id(), direction)
    }

    fn fresh_session_config(&self) -> Config {
        let mut config = self.config.clone();
        config.service_tier = self.chat_widget.current_service_tier();
        config
    }

    async fn drain_active_thread_events(&mut self, tui: &mut tui::Tui) -> Result<()> {
        let Some(mut rx) = self.active_thread_rx.take() else {
            return Ok(());
        };

        let mut disconnected = false;
        loop {
            match rx.try_recv() {
                Ok(event) => self.handle_thread_event_now(event),
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

    /// Returns `(closed_thread_id, primary_thread_id)` when a non-primary active
    /// thread has died and we should fail over to the primary thread.
    ///
    /// A user-requested shutdown (`ExitMode::ShutdownFirst`) sets
    /// `pending_shutdown_exit_thread_id`; matching shutdown completions are ignored
    /// here so Ctrl+C-like exits don't accidentally resurrect the main thread.
    ///
    /// Failover is only eligible when all of these are true:
    /// 1. the event is `thread/closed`;
    /// 2. the active thread differs from the primary thread;
    /// 3. the active thread is not the pending shutdown-exit thread.
    fn active_non_primary_shutdown_target(
        &self,
        notification: &ServerNotification,
    ) -> Option<(ThreadId, ThreadId)> {
        if !matches!(notification, ServerNotification::ThreadClosed(_)) {
            return None;
        }
        let active_thread_id = self.active_thread_id?;
        let primary_thread_id = self.primary_thread_id?;
        if self.pending_shutdown_exit_thread_id == Some(active_thread_id) {
            return None;
        }
        (active_thread_id != primary_thread_id).then_some((active_thread_id, primary_thread_id))
    }

    fn replay_thread_snapshot(
        &mut self,
        snapshot: ThreadEventSnapshot,
        resume_restored_queue: bool,
    ) {
        if let Some(session) = snapshot.session {
            self.chat_widget.handle_thread_session(session);
        }
        self.chat_widget
            .set_queue_autosend_suppressed(/*suppressed*/ true);
        self.chat_widget
            .restore_thread_input_state(snapshot.input_state);
        if !snapshot.turns.is_empty() {
            self.chat_widget
                .replay_thread_turns(snapshot.turns, ReplayKind::ThreadSnapshot);
        }
        for event in snapshot.events {
            self.handle_thread_event_replay(event);
        }
        self.chat_widget
            .set_queue_autosend_suppressed(/*suppressed*/ false);
        self.chat_widget
            .set_initial_user_message_submit_suppressed(/*suppressed*/ false);
        self.chat_widget.submit_initial_user_message_if_pending();
        if resume_restored_queue {
            self.chat_widget.maybe_send_next_queued_input();
        }
        self.refresh_status_line();
    }

    fn should_wait_for_initial_session(session_selection: &SessionSelection) -> bool {
        matches!(
            session_selection,
            SessionSelection::StartFresh | SessionSelection::Exit
        )
    }

    fn should_handle_active_thread_events(
        waiting_for_initial_session_configured: bool,
        has_active_thread_receiver: bool,
    ) -> bool {
        has_active_thread_receiver && !waiting_for_initial_session_configured
    }

    fn should_stop_waiting_for_initial_session(
        waiting_for_initial_session_configured: bool,
        primary_thread_id: Option<ThreadId>,
    ) -> bool {
        waiting_for_initial_session_configured && primary_thread_id.is_some()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        tui: &mut tui::Tui,
        mut app_server: AppServerSession,
        mut config: Config,
        cli_kv_overrides: Vec<(String, TomlValue)>,
        harness_overrides: ConfigOverrides,
        active_profile: Option<String>,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
        session_selection: SessionSelection,
        feedback: codex_feedback::CodexFeedback,
        is_first_run: bool,
        should_prompt_windows_sandbox_nux_at_startup: bool,
        remote_app_server_url: Option<String>,
        remote_app_server_auth_token: Option<String>,
        environment_manager: Arc<EnvironmentManager>,
    ) -> Result<AppExitInfo> {
        use tokio_stream::StreamExt;
        let (app_event_tx, mut app_event_rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(app_event_tx);
        emit_project_config_warnings(&app_event_tx, &config);
        emit_system_bwrap_warning(&app_event_tx, &config);
        tui.set_notification_settings(
            config.tui_notifications.method,
            config.tui_notifications.condition,
        );

        let harness_overrides =
            normalize_harness_overrides_for_cwd(harness_overrides, &config.cwd)?;
        let bootstrap = app_server.bootstrap(&config).await?;
        let mut model = bootstrap.default_model;
        let available_models = bootstrap.available_models;
        let exit_info = handle_model_migration_prompt_if_needed(
            tui,
            &mut config,
            model.as_str(),
            &app_event_tx,
            &available_models,
        )
        .await;
        if let Some(exit_info) = exit_info {
            app_server
                .shutdown()
                .await
                .inspect_err(|err| {
                    tracing::warn!("app-server shutdown failed: {err}");
                })
                .ok();
            return Ok(exit_info);
        }
        if let Some(updated_model) = config.model.clone() {
            model = updated_model;
        }
        let model_catalog = Arc::new(ModelCatalog::new(
            available_models.clone(),
            CollaborationModesConfig {
                default_mode_request_user_input: config
                    .features
                    .enabled(Feature::DefaultModeRequestUserInput),
            },
        ));
        let feedback_audience = bootstrap.feedback_audience;
        let auth_mode = bootstrap.auth_mode;
        let has_chatgpt_account = bootstrap.has_chatgpt_account;
        let requires_openai_auth = bootstrap.requires_openai_auth;
        let status_account_display = bootstrap.status_account_display.clone();
        let initial_plan_type = bootstrap.plan_type;
        let session_telemetry = SessionTelemetry::new(
            ThreadId::new(),
            model.as_str(),
            model.as_str(),
            /*account_id*/ None,
            bootstrap.account_email.clone(),
            auth_mode,
            codex_login::default_client::originator().value,
            config.otel.log_user_prompt,
            user_agent(),
            SessionSource::Cli,
        );
        if config
            .tui_status_line
            .as_ref()
            .is_some_and(|cmd| !cmd.is_empty())
        {
            session_telemetry.counter("codex.status_line", /*inc*/ 1, &[]);
        }

        let status_line_invalid_items_warned = Arc::new(AtomicBool::new(false));
        let terminal_title_invalid_items_warned = Arc::new(AtomicBool::new(false));

        let enhanced_keys_supported = tui.enhanced_keys_supported();
        let wait_for_initial_session_configured =
            Self::should_wait_for_initial_session(&session_selection);
        let (mut chat_widget, initial_started_thread) = match session_selection {
            SessionSelection::StartFresh | SessionSelection::Exit => {
                let started = app_server.start_thread(&config).await?;
                let startup_tooltip_override =
                    prepare_startup_tooltip_override(&mut config, &available_models, is_first_run)
                        .await;
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
                    has_chatgpt_account,
                    model_catalog: model_catalog.clone(),
                    feedback: feedback.clone(),
                    is_first_run,
                    status_account_display: status_account_display.clone(),
                    initial_plan_type,
                    model: Some(model.clone()),
                    startup_tooltip_override,
                    status_line_invalid_items_warned: status_line_invalid_items_warned.clone(),
                    terminal_title_invalid_items_warned: terminal_title_invalid_items_warned
                        .clone(),
                    session_telemetry: session_telemetry.clone(),
                };
                (ChatWidget::new_with_app_event(init), Some(started))
            }
            SessionSelection::Resume(target_session) => {
                let resumed = app_server
                    .resume_thread(config.clone(), target_session.thread_id)
                    .await
                    .wrap_err_with(|| {
                        let target_label = target_session.display_label();
                        format!("Failed to resume session from {target_label}")
                    })?;
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
                    has_chatgpt_account,
                    model_catalog: model_catalog.clone(),
                    feedback: feedback.clone(),
                    is_first_run,
                    status_account_display: status_account_display.clone(),
                    initial_plan_type,
                    model: config.model.clone(),
                    startup_tooltip_override: None,
                    status_line_invalid_items_warned: status_line_invalid_items_warned.clone(),
                    terminal_title_invalid_items_warned: terminal_title_invalid_items_warned
                        .clone(),
                    session_telemetry: session_telemetry.clone(),
                };
                (ChatWidget::new_with_app_event(init), Some(resumed))
            }
            SessionSelection::Fork(target_session) => {
                session_telemetry.counter(
                    "codex.thread.fork",
                    /*inc*/ 1,
                    &[("source", "cli_subcommand")],
                );
                let forked = app_server
                    .fork_thread(config.clone(), target_session.thread_id)
                    .await
                    .wrap_err_with(|| {
                        let target_label = target_session.display_label();
                        format!("Failed to fork session from {target_label}")
                    })?;
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
                    has_chatgpt_account,
                    model_catalog: model_catalog.clone(),
                    feedback: feedback.clone(),
                    is_first_run,
                    status_account_display: status_account_display.clone(),
                    initial_plan_type,
                    model: config.model.clone(),
                    startup_tooltip_override: None,
                    status_line_invalid_items_warned: status_line_invalid_items_warned.clone(),
                    terminal_title_invalid_items_warned: terminal_title_invalid_items_warned
                        .clone(),
                    session_telemetry: session_telemetry.clone(),
                };
                (ChatWidget::new_with_app_event(init), Some(forked))
            }
        };

        chat_widget
            .maybe_prompt_windows_sandbox_enable(should_prompt_windows_sandbox_nux_at_startup);

        let file_search = FileSearchManager::new(config.cwd.to_path_buf(), app_event_tx.clone());
        #[cfg(not(debug_assertions))]
        let upgrade_version = crate::updates::get_upgrade_version(&config);

        let mut app = Self {
            model_catalog,
            session_telemetry: session_telemetry.clone(),
            app_event_tx,
            chat_widget,
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
            status_line_invalid_items_warned: status_line_invalid_items_warned.clone(),
            terminal_title_invalid_items_warned: terminal_title_invalid_items_warned.clone(),
            backtrack: BacktrackState::default(),
            backtrack_render_pending: false,
            feedback: feedback.clone(),
            feedback_audience,
            environment_manager,
            remote_app_server_url,
            remote_app_server_auth_token,
            pending_update_action: None,
            pending_shutdown_exit_thread_id: None,
            windows_sandbox: WindowsSandboxState::default(),
            thread_event_channels: HashMap::new(),
            thread_event_listener_tasks: HashMap::new(),
            agent_navigation: AgentNavigationState::default(),
            active_thread_id: None,
            active_thread_rx: None,
            primary_thread_id: None,
            last_subagent_backfill_attempt: None,
            primary_session_configured: None,
            pending_primary_events: VecDeque::new(),
            pending_app_server_requests: PendingAppServerRequests::default(),
        };
        if let Some(started) = initial_started_thread {
            app.enqueue_primary_thread_session(started.session, started.turns)
                .await?;
        }
        match app_server
            .skills_list(codex_app_server_protocol::SkillsListParams {
                cwds: vec![app.config.cwd.to_path_buf()],
                force_reload: true,
                per_cwd_extra_user_roots: None,
            })
            .await
        {
            Ok(response) => app.handle_skills_list_response(response),
            Err(err) => tracing::warn!("failed to load skills on startup: {err:#}"),
        }

        // On startup, if Agent mode (workspace-write) or ReadOnly is active, warn about world-writable dirs on Windows.
        #[cfg(target_os = "windows")]
        {
            let should_check = WindowsSandboxLevel::from_config(&app.config)
                != WindowsSandboxLevel::Disabled
                && matches!(
                    app.config.permissions.sandbox_policy.get(),
                    codex_protocol::protocol::SandboxPolicy::WorkspaceWrite { .. }
                        | codex_protocol::protocol::SandboxPolicy::ReadOnly { .. }
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
                let sandbox_policy = app.config.permissions.sandbox_policy.get().clone();
                Self::spawn_world_writable_scan(cwd, env_map, logs_base_dir, sandbox_policy, tx);
            }
        }

        let tui_events = tui.event_stream();
        tokio::pin!(tui_events);

        tui.frame_requester().schedule_frame();
        // Kick off a non-blocking rate-limit prefetch so the first `/status`
        // already has data, without delaying the initial frame render.
        if requires_openai_auth && has_chatgpt_account {
            app.refresh_rate_limits(&app_server, RateLimitRefreshOrigin::StartupPrefetch);
        }

        let mut listen_for_app_server_events = true;
        let mut waiting_for_initial_session_configured = wait_for_initial_session_configured;

        #[cfg(not(debug_assertions))]
        let pre_loop_exit_reason = if let Some(latest_version) = upgrade_version {
            let control = app
                .handle_event(
                    tui,
                    &mut app_server,
                    AppEvent::InsertHistoryCell(Box::new(UpdateAvailableHistoryCell::new(
                        latest_version,
                        crate::update_action::get_update_action(),
                    ))),
                )
                .await?;
            match control {
                AppRunControl::Continue => None,
                AppRunControl::Exit(exit_reason) => Some(exit_reason),
            }
        } else {
            None
        };
        #[cfg(debug_assertions)]
        let pre_loop_exit_reason: Option<ExitReason> = None;

        let exit_reason_result = if let Some(exit_reason) = pre_loop_exit_reason {
            Ok(exit_reason)
        } else {
            loop {
                let control = select! {
                    Some(event) = app_event_rx.recv() => {
                        match app.handle_event(tui, &mut app_server, event).await {
                            Ok(control) => control,
                            Err(err) => break Err(err),
                        }
                    }
                    active = async {
                        if let Some(rx) = app.active_thread_rx.as_mut() {
                            rx.recv().await
                        } else {
                            None
                        }
                    }, if App::should_handle_active_thread_events(
                        waiting_for_initial_session_configured,
                        app.active_thread_rx.is_some()
                    ) => {
                        if let Some(event) = active {
                            if let Err(err) = app.handle_active_thread_event(tui, &mut app_server, event).await {
                                break Err(err);
                            }
                        } else {
                            app.clear_active_thread().await;
                        }
                        AppRunControl::Continue
                    }
                    event = tui_events.next() => {
                        if let Some(event) = event {
                            match app.handle_tui_event(tui, &mut app_server, event).await {
                                Ok(control) => control,
                                Err(err) => break Err(err),
                            }
                        } else {
                            tracing::warn!("terminal input stream closed; shutting down active thread");
                            app.handle_exit_mode(&mut app_server, ExitMode::ShutdownFirst).await
                        }
                    }
                    app_server_event = app_server.next_event(), if listen_for_app_server_events => {
                        match app_server_event {
                            Some(event) => app.handle_app_server_event(&app_server, event).await,
                            None => {
                                listen_for_app_server_events = false;
                                tracing::warn!("app-server event stream closed");
                            }
                        }
                        AppRunControl::Continue
                    }
                };
                if App::should_stop_waiting_for_initial_session(
                    waiting_for_initial_session_configured,
                    app.primary_thread_id,
                ) {
                    waiting_for_initial_session_configured = false;
                }
                match control {
                    AppRunControl::Continue => {}
                    AppRunControl::Exit(reason) => break Ok(reason),
                }
            }
        };
        if let Err(err) = app_server.shutdown().await {
            tracing::warn!(error = %err, "failed to shut down embedded app server");
        }
        let clear_result = tui.terminal.clear();
        let exit_reason = match exit_reason_result {
            Ok(exit_reason) => {
                clear_result?;
                exit_reason
            }
            Err(err) => {
                if let Err(clear_err) = clear_result {
                    tracing::warn!(error = %clear_err, "failed to clear terminal UI");
                }
                return Err(err);
            }
        };
        Ok(AppExitInfo {
            token_usage: app.token_usage(),
            thread_id: app.chat_widget.thread_id(),
            thread_name: app.chat_widget.thread_name(),
            update_action: app.pending_update_action,
            exit_reason,
        })
    }

    pub(crate) async fn handle_tui_event(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        event: TuiEvent,
    ) -> Result<AppRunControl> {
        if matches!(event, TuiEvent::Draw) {
            let size = tui.terminal.size()?;
            if size != tui.terminal.last_known_screen_size {
                self.refresh_status_line();
            }
        }

        if self.overlay.is_some() {
            let _ = self.handle_backtrack_overlay_event(tui, event).await?;
        } else {
            match event {
                TuiEvent::Key(key_event) => {
                    self.handle_key_event(tui, app_server, key_event).await;
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
                    // Allow widgets to process any pending timers before rendering.
                    self.chat_widget.pre_draw_tick();
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

    async fn resume_target_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        target_session: SessionTarget,
    ) -> Result<AppRunControl> {
        if self.ignore_same_thread_resume(&target_session) {
            tui.frame_requester().schedule_frame();
            return Ok(AppRunControl::Continue);
        }

        let current_cwd = self.config.cwd.to_path_buf();
        let resume_cwd = if self.remote_app_server_url.is_some() {
            current_cwd.clone()
        } else {
            match crate::resolve_cwd_for_resume_or_fork(
                tui,
                &self.config,
                &current_cwd,
                target_session.thread_id,
                target_session.path.as_deref(),
                CwdPromptAction::Resume,
                /*allow_prompt*/ true,
            )
            .await?
            {
                crate::ResolveCwdOutcome::Continue(Some(cwd)) => cwd,
                crate::ResolveCwdOutcome::Continue(None) => current_cwd.clone(),
                crate::ResolveCwdOutcome::Exit => {
                    return Ok(AppRunControl::Exit(ExitReason::UserRequested));
                }
            }
        };

        let mut resume_config = match self
            .rebuild_config_for_resume_or_fallback(&current_cwd, resume_cwd)
            .await
        {
            Ok(cfg) => cfg,
            Err(err) => {
                self.chat_widget.add_error_message(format!(
                    "Failed to rebuild configuration for resume: {err}"
                ));
                return Ok(AppRunControl::Continue);
            }
        };
        self.apply_runtime_policy_overrides(&mut resume_config);

        let summary = session_summary(
            self.chat_widget.token_usage(),
            self.chat_widget.thread_id(),
            self.chat_widget.thread_name(),
        );
        match app_server
            .resume_thread(resume_config.clone(), target_session.thread_id)
            .await
        {
            Ok(resumed) => {
                self.shutdown_current_thread(app_server).await;
                self.config = resume_config;
                tui.set_notification_settings(
                    self.config.tui_notifications.method,
                    self.config.tui_notifications.condition,
                );
                self.file_search
                    .update_search_dir(self.config.cwd.to_path_buf());
                match self
                    .replace_chat_widget_with_app_server_thread(tui, app_server, resumed)
                    .await
                {
                    Ok(()) => {
                        if let Some(summary) = summary {
                            let mut lines: Vec<Line<'static>> = Vec::new();
                            if let Some(usage_line) = summary.usage_line {
                                lines.push(usage_line.into());
                            }
                            if let Some(command) = summary.resume_command {
                                let spans =
                                    vec!["To continue this session, run ".into(), command.cyan()];
                                lines.push(spans.into());
                            }
                            self.chat_widget.add_plain_history_lines(lines);
                        }
                    }
                    Err(err) => {
                        self.chat_widget.add_error_message(format!(
                            "Failed to attach to resumed app-server thread: {err}"
                        ));
                    }
                }
            }
            Err(err) => {
                let path_display = target_session.display_label();
                self.chat_widget.add_error_message(format!(
                    "Failed to resume session from {path_display}: {err}"
                ));
            }
        }

        Ok(AppRunControl::Continue)
    }

    async fn handle_event(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        event: AppEvent,
    ) -> Result<AppRunControl> {
        match event {
            AppEvent::NewSession => {
                self.start_fresh_session_with_summary_hint(
                    tui, app_server, /*session_start_source*/ None,
                )
                .await;
            }
            AppEvent::ClearUi => {
                self.clear_terminal_ui(tui, /*redraw_header*/ false)?;
                self.reset_app_ui_state_after_clear();

                self.start_fresh_session_with_summary_hint(
                    tui,
                    app_server,
                    Some(ThreadStartSource::Clear),
                )
                .await;
            }
            AppEvent::OpenResumePicker => {
                let picker_app_server = match crate::start_app_server_for_picker(
                    &self.config,
                    &match self.remote_app_server_url.clone() {
                        Some(websocket_url) => crate::AppServerTarget::Remote {
                            websocket_url,
                            auth_token: self.remote_app_server_auth_token.clone(),
                        },
                        None => crate::AppServerTarget::Embedded,
                    },
                    self.environment_manager.clone(),
                )
                .await
                {
                    Ok(app_server) => app_server,
                    Err(err) => {
                        self.chat_widget.add_error_message(format!(
                            "Failed to start TUI session picker: {err}"
                        ));
                        return Ok(AppRunControl::Continue);
                    }
                };
                match crate::resume_picker::run_resume_picker_with_app_server(
                    tui,
                    &self.config,
                    /*show_all*/ false,
                    /*include_non_interactive*/ false,
                    picker_app_server,
                )
                .await?
                {
                    SessionSelection::Resume(target_session) => {
                        match self
                            .resume_target_session(tui, app_server, target_session)
                            .await?
                        {
                            AppRunControl::Continue => {}
                            AppRunControl::Exit(reason) => {
                                return Ok(AppRunControl::Exit(reason));
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
            AppEvent::ResumeSessionByIdOrName(id_or_name) => {
                match crate::lookup_session_target_with_app_server(
                    app_server,
                    self.config.codex_home.as_path(),
                    &id_or_name,
                )
                .await?
                {
                    Some(target_session) => {
                        return self
                            .resume_target_session(tui, app_server, target_session)
                            .await;
                    }
                    None => {
                        self.chat_widget.add_error_message(format!(
                            "No saved chat found matching '{id_or_name}'."
                        ));
                    }
                }
            }
            AppEvent::ForkCurrentSession => {
                self.session_telemetry.counter(
                    "codex.thread.fork",
                    /*inc*/ 1,
                    &[("source", "slash_command")],
                );
                let summary = session_summary(
                    self.chat_widget.token_usage(),
                    self.chat_widget.thread_id(),
                    self.chat_widget.thread_name(),
                );
                self.chat_widget
                    .add_plain_history_lines(vec!["/fork".magenta().into()]);
                if let Some(thread_id) = self.chat_widget.thread_id() {
                    self.refresh_in_memory_config_from_disk_best_effort("forking the thread")
                        .await;
                    match app_server.fork_thread(self.config.clone(), thread_id).await {
                        Ok(forked) => {
                            self.shutdown_current_thread(app_server).await;
                            match self
                                .replace_chat_widget_with_app_server_thread(tui, app_server, forked)
                                .await
                            {
                                Ok(()) => {
                                    if let Some(summary) = summary {
                                        let mut lines: Vec<Line<'static>> = Vec::new();
                                        if let Some(usage_line) = summary.usage_line {
                                            lines.push(usage_line.into());
                                        }
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
                                    self.chat_widget.add_error_message(format!(
                                        "Failed to attach to forked app-server thread: {err}"
                                    ));
                                }
                            }
                        }
                        Err(err) => {
                            self.chat_widget.add_error_message(format!(
                                "Failed to fork current session through the app server: {err}"
                            ));
                        }
                    }
                } else {
                    self.chat_widget.add_error_message(
                        "A thread must contain at least one turn before it can be forked."
                            .to_string(),
                    );
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
            AppEvent::ApplyThreadRollback { num_turns } => {
                if self.apply_non_pending_thread_rollback(num_turns) {
                    tui.frame_requester().schedule_frame();
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
                            thread::sleep(COMMIT_ANIMATION_TICK);
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
            AppEvent::Exit(mode) => {
                return Ok(self.handle_exit_mode(app_server, mode).await);
            }
            AppEvent::FatalExitRequest(message) => {
                return Ok(AppRunControl::Exit(ExitReason::Fatal(message)));
            }
            AppEvent::CodexOp(op) => {
                self.submit_active_thread_op(app_server, op.into()).await?;
            }
            AppEvent::SubmitThreadOp { thread_id, op } => {
                self.submit_thread_op(app_server, thread_id, op.into())
                    .await?;
            }
            AppEvent::ThreadHistoryEntryResponse { thread_id, event } => {
                self.enqueue_thread_history_entry_response(thread_id, event)
                    .await?;
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
                app_id,
                title,
                description,
                instructions,
                url,
                is_installed,
                is_enabled,
            } => {
                self.chat_widget
                    .open_app_link_view(crate::bottom_pane::AppLinkViewParams {
                        app_id,
                        title,
                        description,
                        instructions,
                        url,
                        is_installed,
                        is_enabled,
                        suggest_reason: None,
                        suggestion_type: None,
                        elicitation_target: None,
                    });
            }
            AppEvent::OpenUrlInBrowser { url } => {
                self.open_url_in_browser(url);
            }
            AppEvent::RefreshConnectors { force_refetch } => {
                self.chat_widget.refresh_connectors(force_refetch);
            }
            AppEvent::PluginInstallAuthAdvance { refresh_connectors } => {
                if refresh_connectors {
                    self.chat_widget.refresh_connectors(/*force_refetch*/ true);
                }
                self.chat_widget.advance_plugin_install_auth_flow();
            }
            AppEvent::PluginInstallAuthAbandon => {
                self.chat_widget.abandon_plugin_install_auth_flow();
            }
            AppEvent::FetchPluginsList { cwd } => {
                self.fetch_plugins_list(app_server, cwd);
            }
            AppEvent::OpenPluginDetailLoading {
                plugin_display_name,
            } => {
                self.chat_widget
                    .open_plugin_detail_loading_popup(&plugin_display_name);
            }
            AppEvent::OpenPluginInstallLoading {
                plugin_display_name,
            } => {
                self.chat_widget
                    .open_plugin_install_loading_popup(&plugin_display_name);
            }
            AppEvent::OpenPluginUninstallLoading {
                plugin_display_name,
            } => {
                self.chat_widget
                    .open_plugin_uninstall_loading_popup(&plugin_display_name);
            }
            AppEvent::PluginsLoaded { cwd, result } => {
                self.chat_widget.on_plugins_loaded(cwd, result);
            }
            AppEvent::FetchPluginDetail { cwd, params } => {
                self.fetch_plugin_detail(app_server, cwd, params);
            }
            AppEvent::PluginDetailLoaded { cwd, result } => {
                self.chat_widget.on_plugin_detail_loaded(cwd, result);
            }
            AppEvent::FetchPluginInstall {
                cwd,
                marketplace_path,
                plugin_name,
                plugin_display_name,
            } => {
                self.fetch_plugin_install(
                    app_server,
                    cwd,
                    marketplace_path,
                    plugin_name,
                    plugin_display_name,
                );
            }
            AppEvent::FetchPluginUninstall {
                cwd,
                plugin_id,
                plugin_display_name,
            } => {
                self.fetch_plugin_uninstall(app_server, cwd, plugin_id, plugin_display_name);
            }
            AppEvent::PluginInstallLoaded {
                cwd,
                marketplace_path,
                plugin_name,
                plugin_display_name,
                result,
            } => {
                let install_succeeded = result.is_ok();
                if install_succeeded {
                    if let Err(err) = self.refresh_in_memory_config_from_disk().await {
                        tracing::warn!(error = %err, "failed to refresh config after plugin install");
                    }
                    self.chat_widget.refresh_plugin_mentions();
                    self.chat_widget.submit_op(AppCommand::reload_user_config());
                }
                let should_refresh_plugin_detail = self.chat_widget.on_plugin_install_loaded(
                    cwd.clone(),
                    marketplace_path.clone(),
                    plugin_name.clone(),
                    plugin_display_name,
                    result,
                );
                if install_succeeded && self.chat_widget.config_ref().cwd.as_path() == cwd.as_path()
                {
                    self.fetch_plugins_list(app_server, cwd.clone());
                    if should_refresh_plugin_detail {
                        self.fetch_plugin_detail(
                            app_server,
                            cwd,
                            PluginReadParams {
                                marketplace_path,
                                plugin_name,
                            },
                        );
                    }
                }
            }
            AppEvent::FetchMcpInventory => {
                self.fetch_mcp_inventory(app_server);
            }
            AppEvent::McpInventoryLoaded { result } => {
                self.handle_mcp_inventory_result(result);
            }
            AppEvent::StartFileSearch(query) => {
                self.file_search.on_user_query(query);
            }
            AppEvent::FileSearchResult { query, matches } => {
                self.chat_widget.apply_file_search_result(query, matches);
            }
            AppEvent::RefreshRateLimits { origin } => {
                self.refresh_rate_limits(app_server, origin);
            }
            AppEvent::RateLimitsLoaded { origin, result } => match result {
                Ok(snapshots) => {
                    for snapshot in snapshots {
                        self.chat_widget.on_rate_limit_snapshot(Some(snapshot));
                    }
                    match origin {
                        RateLimitRefreshOrigin::StartupPrefetch => {
                            tui.frame_requester().schedule_frame();
                        }
                        RateLimitRefreshOrigin::StatusCommand { request_id } => {
                            self.chat_widget
                                .finish_status_rate_limit_refresh(request_id);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("account/rateLimits/read failed during TUI refresh: {err}");
                    if let RateLimitRefreshOrigin::StatusCommand { request_id } = origin {
                        self.chat_widget
                            .finish_status_rate_limit_refresh(request_id);
                    }
                }
            },
            AppEvent::ConnectorsLoaded { result, is_final } => {
                self.chat_widget.on_connectors_loaded(result, is_final);
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
            AppEvent::OpenRealtimeAudioDeviceSelection { kind } => {
                self.chat_widget.open_realtime_audio_device_selection(kind);
            }
            AppEvent::RealtimeWebrtcOfferCreated { result } => {
                self.chat_widget.on_realtime_webrtc_offer_created(result);
            }
            AppEvent::RealtimeWebrtcEvent(event) => {
                self.chat_widget.on_realtime_webrtc_event(event);
            }
            AppEvent::RealtimeWebrtcLocalAudioLevel(peak) => {
                self.chat_widget.on_realtime_webrtc_local_audio_level(peak);
            }
            AppEvent::OpenReasoningPopup { model } => {
                self.chat_widget.open_reasoning_popup(model);
            }
            AppEvent::OpenPlanReasoningScopePrompt { model, effort } => {
                self.chat_widget
                    .open_plan_reasoning_scope_prompt(model, effort);
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
            AppEvent::SubmitFeedback {
                category,
                reason,
                turn_id,
                include_logs,
            } => {
                self.submit_feedback(app_server, category, reason, turn_id, include_logs);
            }
            AppEvent::FeedbackSubmitted {
                origin_thread_id,
                category,
                include_logs,
                result,
            } => {
                self.handle_feedback_submitted(origin_thread_id, category, include_logs, result)
                    .await;
            }
            AppEvent::LaunchExternalEditor => {
                if self.chat_widget.external_editor_state() == ExternalEditorState::Active {
                    self.launch_external_editor(tui).await;
                }
            }
            AppEvent::OpenWindowsSandboxEnablePrompt { preset } => {
                self.chat_widget.open_windows_sandbox_enable_prompt(preset);
            }
            AppEvent::OpenWindowsSandboxFallbackPrompt { preset } => {
                self.session_telemetry.counter(
                    "codex.windows_sandbox.fallback_prompt_shown",
                    /*inc*/ 1,
                    &[],
                );
                self.chat_widget.clear_windows_sandbox_setup_status();
                if let Some(started_at) = self.windows_sandbox.setup_started_at.take() {
                    self.session_telemetry.record_duration(
                        "codex.windows_sandbox.elevated_setup_duration_ms",
                        started_at.elapsed(),
                        &[("result", "failure")],
                    );
                }
                self.chat_widget
                    .open_windows_sandbox_fallback_prompt(preset);
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
                    if crate::legacy_core::windows_sandbox::sandbox_setup_is_complete(
                        codex_home.as_path(),
                    ) {
                        tx.send(AppEvent::EnableWindowsSandboxForAgentMode {
                            preset,
                            mode: WindowsSandboxEnableMode::Elevated,
                        });
                        return Ok(AppRunControl::Continue);
                    }

                    self.chat_widget.show_windows_sandbox_setup_status();
                    self.windows_sandbox.setup_started_at = Some(Instant::now());
                    let session_telemetry = self.session_telemetry.clone();
                    tokio::task::spawn_blocking(move || {
                        let result = crate::legacy_core::windows_sandbox::run_elevated_setup(
                            &policy,
                            policy_cwd.as_path(),
                            command_cwd.as_path(),
                            &env_map,
                            codex_home.as_path(),
                        );
                        let event = match result {
                            Ok(()) => {
                                session_telemetry.counter(
                                    "codex.windows_sandbox.elevated_setup_success",
                                    /*inc*/ 1,
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
                                    crate::legacy_core::windows_sandbox::elevated_setup_failure_details(
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
                                session_telemetry.counter(
                                    crate::legacy_core::windows_sandbox::elevated_setup_failure_metric_name(
                                        &err,
                                    ),
                                    /*inc*/ 1,
                                    &tags,
                                );
                                tracing::error!(
                                    error = %err,
                                    "failed to run elevated Windows sandbox setup"
                                );
                                AppEvent::OpenWindowsSandboxFallbackPrompt { preset }
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
            AppEvent::BeginWindowsSandboxLegacySetup { preset } => {
                #[cfg(target_os = "windows")]
                {
                    let policy = preset.sandbox.clone();
                    let policy_cwd = self.config.cwd.clone();
                    let command_cwd = policy_cwd.clone();
                    let env_map: std::collections::HashMap<String, String> =
                        std::env::vars().collect();
                    let codex_home = self.config.codex_home.clone();
                    let tx = self.app_event_tx.clone();
                    let session_telemetry = self.session_telemetry.clone();

                    self.chat_widget.show_windows_sandbox_setup_status();
                    tokio::task::spawn_blocking(move || {
                        if let Err(err) =
                            crate::legacy_core::windows_sandbox::run_legacy_setup_preflight(
                                &policy,
                                policy_cwd.as_path(),
                                command_cwd.as_path(),
                                &env_map,
                                codex_home.as_path(),
                            )
                        {
                            session_telemetry.counter(
                                "codex.windows_sandbox.legacy_setup_preflight_failed",
                                /*inc*/ 1,
                                &[],
                            );
                            tracing::warn!(
                                error = %err,
                                "failed to preflight non-admin Windows sandbox setup"
                            );
                        }
                        tx.send(AppEvent::EnableWindowsSandboxForAgentMode {
                            preset,
                            mode: WindowsSandboxEnableMode::Legacy,
                        });
                    });
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = preset;
                }
            }
            AppEvent::BeginWindowsSandboxGrantReadRoot { path } => {
                #[cfg(target_os = "windows")]
                {
                    self.chat_widget
                        .add_to_history(history_cell::new_info_event(
                            format!("Granting sandbox read access to {path} ..."),
                            /*hint*/ None,
                        ));

                    let policy = self.config.permissions.sandbox_policy.get().clone();
                    let policy_cwd = self.config.cwd.clone();
                    let command_cwd = self.config.cwd.clone();
                    let env_map: std::collections::HashMap<String, String> =
                        std::env::vars().collect();
                    let codex_home = self.config.codex_home.clone();
                    let tx = self.app_event_tx.clone();

                    tokio::task::spawn_blocking(move || {
                        let requested_path = PathBuf::from(path);
                        let event = match crate::legacy_core::grant_read_root_non_elevated(
                            &policy,
                            policy_cwd.as_path(),
                            command_cwd.as_path(),
                            &env_map,
                            codex_home.as_path(),
                            requested_path.as_path(),
                        ) {
                            Ok(canonical_path) => AppEvent::WindowsSandboxGrantReadRootCompleted {
                                path: canonical_path,
                                error: None,
                            },
                            Err(err) => AppEvent::WindowsSandboxGrantReadRootCompleted {
                                path: requested_path,
                                error: Some(err.to_string()),
                            },
                        };
                        tx.send(event);
                    });
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = path;
                }
            }
            AppEvent::WindowsSandboxGrantReadRootCompleted { path, error } => match error {
                Some(err) => {
                    self.chat_widget
                        .add_to_history(history_cell::new_error_event(format!("Error: {err}")));
                }
                None => {
                    self.chat_widget
                        .add_to_history(history_cell::new_info_event(
                            format!("Sandbox read access granted for {}", path.display()),
                            /*hint*/ None,
                        ));
                }
            },
            AppEvent::EnableWindowsSandboxForAgentMode { preset, mode } => {
                #[cfg(target_os = "windows")]
                {
                    self.chat_widget.clear_windows_sandbox_setup_status();
                    if let Some(started_at) = self.windows_sandbox.setup_started_at.take() {
                        self.session_telemetry.record_duration(
                            "codex.windows_sandbox.elevated_setup_duration_ms",
                            started_at.elapsed(),
                            &[("result", "success")],
                        );
                    }
                    let profile = self.active_profile.as_deref();
                    let elevated_enabled = matches!(mode, WindowsSandboxEnableMode::Elevated);
                    let builder = ConfigEditsBuilder::new(&self.config.codex_home)
                        .with_profile(profile)
                        .set_windows_sandbox_mode(if elevated_enabled {
                            "elevated"
                        } else {
                            "unelevated"
                        })
                        .clear_legacy_windows_sandbox_keys();
                    match builder.apply().await {
                        Ok(()) => {
                            if elevated_enabled {
                                self.config.set_windows_sandbox_enabled(/*value*/ false);
                                self.config
                                    .set_windows_elevated_sandbox_enabled(/*value*/ true);
                            } else {
                                self.config.set_windows_sandbox_enabled(/*value*/ true);
                                self.config
                                    .set_windows_elevated_sandbox_enabled(/*value*/ false);
                            }
                            self.chat_widget.set_windows_sandbox_mode(
                                self.config.permissions.windows_sandbox_mode,
                            );
                            let windows_sandbox_level =
                                WindowsSandboxLevel::from_config(&self.config);
                            if let Some((sample_paths, extra_count, failed_scan)) =
                                self.chat_widget.world_writable_warning_details()
                            {
                                self.app_event_tx.send(AppEvent::CodexOp(
                                    AppCommand::override_turn_context(
                                        /*cwd*/ None,
                                        /*approval_policy*/ None,
                                        /*approvals_reviewer*/ None,
                                        /*sandbox_policy*/ None,
                                        #[cfg(target_os = "windows")]
                                        Some(windows_sandbox_level),
                                        /*model*/ None,
                                        /*effort*/ None,
                                        /*summary*/ None,
                                        /*service_tier*/ None,
                                        /*collaboration_mode*/ None,
                                        /*personality*/ None,
                                    )
                                    .into(),
                                ));
                                self.app_event_tx.send(
                                    AppEvent::OpenWorldWritableWarningConfirmation {
                                        preset: Some(preset.clone()),
                                        sample_paths,
                                        extra_count,
                                        failed_scan,
                                    },
                                );
                            } else {
                                self.app_event_tx.send(AppEvent::CodexOp(
                                    AppCommand::override_turn_context(
                                        /*cwd*/ None,
                                        Some(preset.approval),
                                        Some(self.config.approvals_reviewer),
                                        Some(preset.sandbox.clone()),
                                        #[cfg(target_os = "windows")]
                                        Some(windows_sandbox_level),
                                        /*model*/ None,
                                        /*effort*/ None,
                                        /*summary*/ None,
                                        /*service_tier*/ None,
                                        /*collaboration_mode*/ None,
                                        /*personality*/ None,
                                    )
                                    .into(),
                                ));
                                self.app_event_tx
                                    .send(AppEvent::UpdateAskForApprovalPolicy(preset.approval));
                                self.app_event_tx
                                    .send(AppEvent::UpdateSandboxPolicy(preset.sandbox.clone()));
                                let _ = mode;
                                self.chat_widget.add_plain_history_lines(vec![
                                    Line::from(vec!["• ".dim(), "Sandbox ready".into()]),
                                    Line::from(vec![
                                        "  ".into(),
                                        "Codex can now safely edit files and execute commands in your computer"
                                            .dark_gray(),
                                    ]),
                                ]);
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
                        let effort_label = effort
                            .map(|selected_effort| selected_effort.to_string())
                            .unwrap_or_else(|| "default".to_string());
                        tracing::info!("Selected model: {model}, Selected effort: {effort_label}");
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
                        self.chat_widget.add_info_message(message, /*hint*/ None);
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
            AppEvent::PluginUninstallLoaded {
                cwd,
                plugin_id: _plugin_id,
                plugin_display_name,
                result,
            } => {
                let uninstall_succeeded = result.is_ok();
                if uninstall_succeeded {
                    if let Err(err) = self.refresh_in_memory_config_from_disk().await {
                        tracing::warn!(
                            error = %err,
                            "failed to refresh config after plugin uninstall"
                        );
                    }
                    self.chat_widget.refresh_plugin_mentions();
                    self.chat_widget.submit_op(AppCommand::reload_user_config());
                }
                self.chat_widget.on_plugin_uninstall_loaded(
                    cwd.clone(),
                    plugin_display_name,
                    result,
                );
                if uninstall_succeeded
                    && self.chat_widget.config_ref().cwd.as_path() == cwd.as_path()
                {
                    self.fetch_plugins_list(app_server, cwd);
                }
            }
            AppEvent::RefreshPluginMentions => {
                self.refresh_plugin_mentions();
            }
            AppEvent::PluginMentionsLoaded { mut plugins } => {
                if !self.config.features.enabled(Feature::Plugins) {
                    plugins = None;
                }
                self.chat_widget.on_plugin_mentions_loaded(plugins);
            }
            AppEvent::PersistPersonalitySelection { personality } => {
                let profile = self.active_profile.as_deref();
                match ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_profile(profile)
                    .set_personality(Some(personality))
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
                        self.chat_widget.add_info_message(message, /*hint*/ None);
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
            AppEvent::PersistServiceTierSelection { service_tier } => {
                self.refresh_status_line();
                let profile = self.active_profile.as_deref();
                match ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_profile(profile)
                    .set_service_tier(service_tier)
                    .apply()
                    .await
                {
                    Ok(()) => {
                        let status = if service_tier.is_some() { "on" } else { "off" };
                        let mut message = format!("Fast mode set to {status}");
                        if let Some(profile) = profile {
                            message.push_str(" for ");
                            message.push_str(profile);
                            message.push_str(" profile");
                        }
                        self.chat_widget.add_info_message(message, /*hint*/ None);
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "failed to persist fast mode selection");
                        if let Some(profile) = profile {
                            self.chat_widget.add_error_message(format!(
                                "Failed to save Fast mode for profile `{profile}`: {err}"
                            ));
                        } else {
                            self.chat_widget.add_error_message(format!(
                                "Failed to save default Fast mode: {err}"
                            ));
                        }
                    }
                }
            }
            AppEvent::PersistRealtimeAudioDeviceSelection { kind, name } => {
                let builder = match kind {
                    RealtimeAudioDeviceKind::Microphone => {
                        ConfigEditsBuilder::new(&self.config.codex_home)
                            .set_realtime_microphone(name.as_deref())
                    }
                    RealtimeAudioDeviceKind::Speaker => {
                        ConfigEditsBuilder::new(&self.config.codex_home)
                            .set_realtime_speaker(name.as_deref())
                    }
                };

                match builder.apply().await {
                    Ok(()) => {
                        match kind {
                            RealtimeAudioDeviceKind::Microphone => {
                                self.config.realtime_audio.microphone = name.clone();
                            }
                            RealtimeAudioDeviceKind::Speaker => {
                                self.config.realtime_audio.speaker = name.clone();
                            }
                        }
                        self.chat_widget
                            .set_realtime_audio_device(kind, name.clone());

                        if self.chat_widget.realtime_conversation_is_live() {
                            self.chat_widget.open_realtime_audio_restart_prompt(kind);
                        } else {
                            let selection = name.unwrap_or_else(|| "System default".to_string());
                            self.chat_widget.add_info_message(
                                format!("Realtime {} set to {selection}", kind.noun()),
                                /*hint*/ None,
                            );
                        }
                    }
                    Err(err) => {
                        tracing::error!(
                            error = %err,
                            "failed to persist realtime audio selection"
                        );
                        self.chat_widget.add_error_message(format!(
                            "Failed to save realtime {}: {err}",
                            kind.noun()
                        ));
                    }
                }
            }
            AppEvent::RestartRealtimeAudioDevice { kind } => {
                self.chat_widget.restart_realtime_audio_device(kind);
            }
            AppEvent::UpdateAskForApprovalPolicy(policy) => {
                let mut config = self.config.clone();
                if !self.try_set_approval_policy_on_config(
                    &mut config,
                    policy,
                    "Failed to set approval policy",
                    "failed to set approval policy on app config",
                ) {
                    return Ok(AppRunControl::Continue);
                }
                self.config = config;
                self.runtime_approval_policy_override =
                    Some(self.config.permissions.approval_policy.value());
                self.chat_widget
                    .set_approval_policy(self.config.permissions.approval_policy.value());
            }
            AppEvent::UpdateSandboxPolicy(policy) => {
                #[cfg(target_os = "windows")]
                let policy_is_workspace_write_or_ro = matches!(
                    &policy,
                    codex_protocol::protocol::SandboxPolicy::WorkspaceWrite { .. }
                        | codex_protocol::protocol::SandboxPolicy::ReadOnly { .. }
                );
                let policy_for_chat = policy.clone();

                let mut config = self.config.clone();
                if !self.try_set_sandbox_policy_on_config(
                    &mut config,
                    policy,
                    "Failed to set sandbox policy",
                    "failed to set sandbox policy on app config",
                ) {
                    return Ok(AppRunControl::Continue);
                }
                self.config = config;
                if let Err(err) = self.chat_widget.set_sandbox_policy(policy_for_chat) {
                    tracing::warn!(%err, "failed to set sandbox policy on chat config");
                    self.chat_widget
                        .add_error_message(format!("Failed to set sandbox policy: {err}"));
                    return Ok(AppRunControl::Continue);
                }
                self.runtime_sandbox_policy_override =
                    Some(self.config.permissions.sandbox_policy.get().clone());

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
                        let sandbox_policy = self.config.permissions.sandbox_policy.get().clone();
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
            AppEvent::UpdateApprovalsReviewer(policy) => {
                self.config.approvals_reviewer = policy;
                self.chat_widget.set_approvals_reviewer(policy);
                let profile = self.active_profile.as_deref();
                let segments = if let Some(profile) = profile {
                    vec![
                        "profiles".to_string(),
                        profile.to_string(),
                        "approvals_reviewer".to_string(),
                    ]
                } else {
                    vec!["approvals_reviewer".to_string()]
                };
                if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_profile(profile)
                    .with_edits([ConfigEdit::SetPath {
                        segments,
                        value: policy.to_string().into(),
                    }])
                    .apply()
                    .await
                {
                    tracing::error!(
                        error = %err,
                        "failed to persist approvals reviewer update"
                    );
                    self.chat_widget
                        .add_error_message(format!("Failed to save approvals reviewer: {err}"));
                }
            }
            AppEvent::UpdateFeatureFlags { updates } => {
                self.update_feature_flags(updates).await;
            }
            AppEvent::UpdateMemorySettings {
                use_memories,
                generate_memories,
            } => {
                self.update_memory_settings_with_app_server(
                    app_server,
                    use_memories,
                    generate_memories,
                )
                .await;
            }
            AppEvent::ResetMemories => {
                self.reset_memories_with_app_server(app_server).await;
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
            AppEvent::UpdatePlanModeReasoningEffort(effort) => {
                self.config.plan_mode_reasoning_effort = effort;
                self.chat_widget.set_plan_mode_reasoning_effort(effort);
            }
            AppEvent::PersistFullAccessWarningAcknowledged => {
                if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
                    .set_hide_full_access_warning(/*acknowledged*/ true)
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
                    .set_hide_world_writable_warning(/*acknowledged*/ true)
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
                    .set_hide_rate_limit_model_nudge(/*acknowledged*/ true)
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
            AppEvent::PersistPlanModeReasoningEffort(effort) => {
                let profile = self.active_profile.as_deref();
                let segments = if let Some(profile) = profile {
                    vec![
                        "profiles".to_string(),
                        profile.to_string(),
                        "plan_mode_reasoning_effort".to_string(),
                    ]
                } else {
                    vec!["plan_mode_reasoning_effort".to_string()]
                };
                let edit = if let Some(effort) = effort {
                    ConfigEdit::SetPath {
                        segments,
                        value: effort.to_string().into(),
                    }
                } else {
                    ConfigEdit::ClearPath { segments }
                };
                if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_edits([edit])
                    .apply()
                    .await
                {
                    tracing::error!(
                        error = %err,
                        "failed to persist plan mode reasoning effort"
                    );
                    if let Some(profile) = profile {
                        self.chat_widget.add_error_message(format!(
                            "Failed to save Plan mode reasoning effort for profile `{profile}`: {err}"
                        ));
                    } else {
                        self.chat_widget.add_error_message(format!(
                            "Failed to save Plan mode reasoning effort: {err}"
                        ));
                    }
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
                self.open_agent_picker(app_server).await;
            }
            AppEvent::SelectAgentThread(thread_id) => {
                self.select_agent_thread(tui, app_server, thread_id).await?;
            }
            AppEvent::OpenSkillsList => {
                self.chat_widget.open_skills_list();
            }
            AppEvent::OpenManageSkillsPopup => {
                self.chat_widget.open_manage_skills_popup();
            }
            AppEvent::SetSkillEnabled { path, enabled } => {
                let edits = [ConfigEdit::SetSkillConfig {
                    path: path.to_path_buf(),
                    enabled,
                }];
                match ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_edits(edits)
                    .apply()
                    .await
                {
                    Ok(()) => {
                        self.chat_widget.update_skill_enabled(path, enabled);
                        if let Err(err) = self.refresh_in_memory_config_from_disk().await {
                            tracing::warn!(
                                error = %err,
                                "failed to refresh config after skill toggle"
                            );
                        }
                    }
                    Err(err) => {
                        let path_display = path.display();
                        self.chat_widget.add_error_message(format!(
                            "Failed to update skill config for {path_display}: {err}"
                        ));
                    }
                }
            }
            AppEvent::SetAppEnabled { id, enabled } => {
                let edits = if enabled {
                    vec![
                        ConfigEdit::ClearPath {
                            segments: vec!["apps".to_string(), id.clone(), "enabled".to_string()],
                        },
                        ConfigEdit::ClearPath {
                            segments: vec![
                                "apps".to_string(),
                                id.clone(),
                                "disabled_reason".to_string(),
                            ],
                        },
                    ]
                } else {
                    vec![
                        ConfigEdit::SetPath {
                            segments: vec!["apps".to_string(), id.clone(), "enabled".to_string()],
                            value: false.into(),
                        },
                        ConfigEdit::SetPath {
                            segments: vec![
                                "apps".to_string(),
                                id.clone(),
                                "disabled_reason".to_string(),
                            ],
                            value: "user".into(),
                        },
                    ]
                };
                match ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_edits(edits)
                    .apply()
                    .await
                {
                    Ok(()) => {
                        self.chat_widget.update_connector_enabled(&id, enabled);
                        if let Err(err) = self.refresh_in_memory_config_from_disk().await {
                            tracing::warn!(error = %err, "failed to refresh config after app toggle");
                        }
                        self.chat_widget.submit_op(AppCommand::reload_user_config());
                    }
                    Err(err) => {
                        self.chat_widget.add_error_message(format!(
                            "Failed to update app config for {id}: {err}"
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
                ApprovalRequest::Permissions {
                    permissions,
                    reason,
                    ..
                } => {
                    let _ = tui.enter_alt_screen();
                    let mut lines = Vec::new();
                    if let Some(reason) = reason {
                        lines.push(Line::from(vec!["Reason: ".into(), reason.italic()]));
                        lines.push(Line::from(""));
                    }
                    if let Some(rule_line) =
                        crate::bottom_pane::format_requested_permissions_rule(&permissions)
                    {
                        lines.push(Line::from(vec![
                            "Permission rule: ".into(),
                            rule_line.cyan(),
                        ]));
                    }
                    self.overlay = Some(Overlay::new_static_with_renderables(
                        vec![Box::new(Paragraph::new(lines).wrap(Wrap { trim: false }))],
                        "P E R M I S S I O N S".to_string(),
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
            #[cfg(not(target_os = "linux"))]
            AppEvent::UpdateRecordingMeter { id, text } => {
                // Update in place to preserve the element id for subsequent frames.
                let updated = self.chat_widget.update_recording_meter_in_place(&id, &text);
                if updated
                    || self
                        .chat_widget
                        .stop_realtime_conversation_for_deleted_meter(&id)
                {
                    tui.frame_requester().schedule_frame();
                }
            }
            AppEvent::StatusLineSetup { items } => {
                let ids = items.iter().map(ToString::to_string).collect::<Vec<_>>();
                let edit = crate::legacy_core::config::edit::status_line_items_edit(&ids);
                let apply_result = ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_edits([edit])
                    .apply()
                    .await;
                match apply_result {
                    Ok(()) => {
                        self.config.tui_status_line = Some(ids.clone());
                        self.chat_widget.setup_status_line(items);
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "failed to persist status line items; keeping previous selection");
                        self.chat_widget
                            .add_error_message(format!("Failed to save status line items: {err}"));
                    }
                }
            }
            AppEvent::StatusLineBranchUpdated { cwd, branch } => {
                self.chat_widget.set_status_line_branch(cwd, branch);
                self.refresh_status_line();
            }
            AppEvent::StatusLineSetupCancelled => {
                self.chat_widget.cancel_status_line_setup();
            }
            AppEvent::TerminalTitleSetup { items } => {
                let ids = items.iter().map(ToString::to_string).collect::<Vec<_>>();
                let edit = crate::legacy_core::config::edit::terminal_title_items_edit(&ids);
                let apply_result = ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_edits([edit])
                    .apply()
                    .await;
                match apply_result {
                    Ok(()) => {
                        self.config.tui_terminal_title = Some(ids.clone());
                        self.chat_widget.setup_terminal_title(items);
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "failed to persist terminal title items; keeping previous selection");
                        self.chat_widget.revert_terminal_title_setup_preview();
                        self.chat_widget.add_error_message(format!(
                            "Failed to save terminal title items: {err}"
                        ));
                    }
                }
            }
            AppEvent::TerminalTitleSetupPreview { items } => {
                self.chat_widget.preview_terminal_title(items);
            }
            AppEvent::TerminalTitleSetupCancelled => {
                self.chat_widget.cancel_terminal_title_setup();
            }
            AppEvent::SyntaxThemeSelected { name } => {
                let edit = crate::legacy_core::config::edit::syntax_theme_edit(&name);
                let apply_result = ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_edits([edit])
                    .apply()
                    .await;
                match apply_result {
                    Ok(()) => {
                        // Ensure the selected theme is active in the current
                        // session.  The preview callback covers arrow-key
                        // navigation, but if the user presses Enter without
                        // navigating, the runtime theme must still be applied.
                        if let Some(theme) = crate::render::highlight::resolve_theme_by_name(
                            &name,
                            Some(&self.config.codex_home),
                        ) {
                            crate::render::highlight::set_syntax_theme(theme);
                        }
                        self.sync_tui_theme_selection(name);
                    }
                    Err(err) => {
                        self.restore_runtime_theme_from_config();
                        tracing::error!(error = %err, "failed to persist theme selection");
                        self.chat_widget
                            .add_error_message(format!("Failed to save theme: {err}"));
                    }
                }
            }
        }
        Ok(AppRunControl::Continue)
    }

    async fn handle_exit_mode(
        &mut self,
        app_server: &mut AppServerSession,
        mode: ExitMode,
    ) -> AppRunControl {
        match mode {
            ExitMode::ShutdownFirst => {
                // Mark the thread we are explicitly shutting down for exit so
                // its shutdown completion does not trigger agent failover.
                self.pending_shutdown_exit_thread_id =
                    self.active_thread_id.or(self.chat_widget.thread_id());
                if self.pending_shutdown_exit_thread_id.is_some() {
                    self.shutdown_current_thread(app_server).await;
                }
                self.pending_shutdown_exit_thread_id = None;
                AppRunControl::Exit(ExitReason::UserRequested)
            }
            ExitMode::Immediate => {
                self.pending_shutdown_exit_thread_id = None;
                AppRunControl::Exit(ExitReason::UserRequested)
            }
        }
    }

    fn handle_skills_list_response(&mut self, response: SkillsListResponse) {
        let response = list_skills_response_to_core(response);
        let cwd = self.chat_widget.config_ref().cwd.clone();
        let errors = errors_for_cwd(&cwd, &response);
        emit_skill_load_warnings(&self.app_event_tx, &errors);
        self.chat_widget.handle_skills_list_response(response);
    }

    async fn handle_thread_rollback_response(
        &mut self,
        thread_id: ThreadId,
        num_turns: u32,
        response: &ThreadRollbackResponse,
    ) {
        if let Some(channel) = self.thread_event_channels.get(&thread_id) {
            let mut store = channel.store.lock().await;
            store.apply_thread_rollback(response);
        }
        if self.active_thread_id == Some(thread_id)
            && let Some(mut rx) = self.active_thread_rx.take()
        {
            let mut disconnected = false;
            loop {
                match rx.try_recv() {
                    Ok(_) => {}
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
        }
        self.handle_backtrack_rollback_succeeded(num_turns);
    }

    fn handle_thread_event_now(&mut self, event: ThreadBufferedEvent) {
        let needs_refresh = matches!(
            &event,
            ThreadBufferedEvent::Notification(ServerNotification::TurnStarted(_))
                | ThreadBufferedEvent::Notification(ServerNotification::ThreadTokenUsageUpdated(_))
        );
        match event {
            ThreadBufferedEvent::Notification(notification) => {
                self.chat_widget
                    .handle_server_notification(notification, /*replay_kind*/ None);
            }
            ThreadBufferedEvent::Request(request) => {
                if self
                    .pending_app_server_requests
                    .contains_server_request(&request)
                {
                    self.chat_widget
                        .handle_server_request(request, /*replay_kind*/ None);
                }
            }
            ThreadBufferedEvent::HistoryEntryResponse(event) => {
                self.chat_widget.handle_history_entry_response(event);
            }
            ThreadBufferedEvent::FeedbackSubmission(event) => {
                self.handle_feedback_thread_event(event);
            }
        }
        if needs_refresh {
            self.refresh_status_line();
        }
    }

    fn handle_thread_event_replay(&mut self, event: ThreadBufferedEvent) {
        match event {
            ThreadBufferedEvent::Notification(notification) => self
                .chat_widget
                .handle_server_notification(notification, Some(ReplayKind::ThreadSnapshot)),
            ThreadBufferedEvent::Request(request) => self
                .chat_widget
                .handle_server_request(request, Some(ReplayKind::ThreadSnapshot)),
            ThreadBufferedEvent::HistoryEntryResponse(event) => {
                self.chat_widget.handle_history_entry_response(event)
            }
            ThreadBufferedEvent::FeedbackSubmission(event) => {
                self.handle_feedback_thread_event(event);
            }
        }
    }

    /// Handles an event emitted by the currently active thread.
    ///
    /// This function enforces shutdown intent routing: unexpected non-primary
    /// thread shutdowns fail over to the primary thread, while user-requested
    /// app exits consume only the tracked shutdown completion and then proceed.
    async fn handle_active_thread_event(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        event: ThreadBufferedEvent,
    ) -> Result<()> {
        // Capture this before any potential thread switch: we only want to clear
        // the exit marker when the currently active thread acknowledges shutdown.
        let pending_shutdown_exit_completed = matches!(
            &event,
            ThreadBufferedEvent::Notification(ServerNotification::ThreadClosed(_))
        ) && self.pending_shutdown_exit_thread_id
            == self.active_thread_id;

        // Processing order matters:
        //
        // 1. handle unexpected non-primary shutdown failover first;
        // 2. clear pending exit marker for matching shutdown;
        // 3. forward the event through normal handling.
        //
        // This preserves the mental model that user-requested exits do not trigger
        // failover, while true sub-agent deaths still do.
        if let ThreadBufferedEvent::Notification(notification) = &event
            && let Some((closed_thread_id, primary_thread_id)) =
                self.active_non_primary_shutdown_target(notification)
        {
            self.mark_agent_picker_thread_closed(closed_thread_id);
            self.select_agent_thread(tui, app_server, primary_thread_id)
                .await?;
            if self.active_thread_id == Some(primary_thread_id) {
                self.chat_widget.add_info_message(
                    format!(
                        "Agent thread {closed_thread_id} closed. Switched back to main thread."
                    ),
                    /*hint*/ None,
                );
            } else {
                self.clear_active_thread().await;
                self.chat_widget.add_error_message(format!(
                    "Agent thread {closed_thread_id} closed. Failed to switch back to main thread {primary_thread_id}.",
                ));
            }
            return Ok(());
        }

        if pending_shutdown_exit_completed {
            // Clear only after seeing the shutdown completion for the tracked
            // thread, so unrelated shutdowns cannot consume this marker.
            self.pending_shutdown_exit_thread_id = None;
        }
        if let ThreadBufferedEvent::Notification(notification) = &event {
            self.hydrate_collab_agent_metadata_for_notification(app_server, notification)
                .await;
        }

        self.handle_thread_event_now(event);
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

    pub(crate) fn token_usage(&self) -> codex_protocol::protocol::TokenUsage {
        self.chat_widget.token_usage()
    }

    fn on_update_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        // TODO(aibrahim): Remove this and don't use config as a state object.
        // Instead, explicitly pass the stored collaboration mode's effort into new sessions.
        self.config.model_reasoning_effort = effort;
        self.chat_widget.set_reasoning_effort(effort);
    }

    fn on_update_personality(&mut self, personality: Personality) {
        self.config.personality = Some(personality);
        self.chat_widget.set_personality(personality);
    }

    fn sync_tui_theme_selection(&mut self, name: String) {
        self.config.tui_theme = Some(name.clone());
        self.chat_widget.set_tui_theme(Some(name));
    }

    fn restore_runtime_theme_from_config(&self) {
        if let Some(name) = self.config.tui_theme.as_deref()
            && let Some(theme) =
                crate::render::highlight::resolve_theme_by_name(name, Some(&self.config.codex_home))
        {
            crate::render::highlight::set_syntax_theme(theme);
            return;
        }

        let auto_theme_name = crate::render::highlight::adaptive_default_theme_name();
        if let Some(theme) = crate::render::highlight::resolve_theme_by_name(
            auto_theme_name,
            Some(&self.config.codex_home),
        ) {
            crate::render::highlight::set_syntax_theme(theme);
        }
    }

    fn personality_label(personality: Personality) -> &'static str {
        match personality {
            Personality::None => "None",
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
        self.chat_widget.set_footer_hint_override(/*items*/ None);
        tui.frame_requester().schedule_frame();
    }

    async fn handle_key_event(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        key_event: KeyEvent,
    ) {
        // Some terminals, especially on macOS, encode Option+Left/Right as Option+b/f unless
        // enhanced keyboard reporting is available. We only treat those word-motion fallbacks as
        // agent-switch shortcuts when the composer is empty so we never steal the expected
        // editing behavior for moving across words inside a draft.
        let allow_agent_word_motion_fallback = !self.enhanced_keys_supported
            && self.chat_widget.composer_text_with_pending().is_empty();
        if self.overlay.is_none()
            && self.chat_widget.no_modal_or_popup_active()
            // Alt+Left/Right are also natural word-motion keys in the composer. Keep agent
            // fast-switch available only once the draft is empty so editing behavior wins whenever
            // there is text on screen.
            && self.chat_widget.composer_text_with_pending().is_empty()
            && previous_agent_shortcut_matches(key_event, allow_agent_word_motion_fallback)
        {
            if let Some(thread_id) = self
                .adjacent_thread_id_with_backfill(app_server, AgentNavigationDirection::Previous)
                .await
            {
                let _ = self.select_agent_thread(tui, app_server, thread_id).await;
            }
            return;
        }
        if self.overlay.is_none()
            && self.chat_widget.no_modal_or_popup_active()
            // Mirror the previous-agent rule above: empty drafts may use these keys for thread
            // switching, but non-empty drafts keep them for expected word-wise cursor motion.
            && self.chat_widget.composer_text_with_pending().is_empty()
            && next_agent_shortcut_matches(key_event, allow_agent_word_motion_fallback)
        {
            if let Some(thread_id) = self
                .adjacent_thread_id_with_backfill(app_server, AgentNavigationDirection::Next)
                .await
            {
                let _ = self.select_agent_thread(tui, app_server, thread_id).await;
            }
            return;
        }

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
                code: KeyCode::Char('l'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } => {
                if !self.chat_widget.can_run_ctrl_l_clear_now() {
                    return;
                }
                if let Err(err) = self.clear_terminal_ui(tui, /*redraw_header*/ false) {
                    tracing::warn!(error = %err, "failed to clear terminal UI");
                    self.chat_widget
                        .add_error_message(format!("Failed to clear terminal UI: {err}"));
                } else {
                    self.reset_app_ui_state_after_clear();
                    self.queue_clear_ui_header(tui);
                    tui.frame_requester().schedule_frame();
                }
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
                self.chat_widget.handle_key_event(key_event);
            }
        };
    }

    fn refresh_status_line(&mut self) {
        self.chat_widget.refresh_status_line();
    }

    #[cfg(target_os = "windows")]
    fn spawn_world_writable_scan(
        cwd: AbsolutePathBuf,
        env_map: std::collections::HashMap<String, String>,
        logs_base_dir: AbsolutePathBuf,
        sandbox_policy: codex_protocol::protocol::SandboxPolicy,
        tx: AppEventSender,
    ) {
        tokio::task::spawn_blocking(move || {
            let logs_base_dir_path = logs_base_dir.as_path();
            let result = codex_windows_sandbox::apply_world_writable_scan_and_denies(
                logs_base_dir_path,
                cwd.as_path(),
                &env_map,
                &sandbox_policy,
                Some(logs_base_dir_path),
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

/// Collect every MCP server status needed for `/mcp` from the app-server by
/// walking the paginated `mcpServerStatus/list` RPC until no `next_cursor` is
/// returned.
///
/// All pages are eagerly gathered into a single `Vec` so the caller can render
/// the inventory atomically. Each page requests up to 100 entries.
async fn fetch_all_mcp_server_statuses(
    request_handle: AppServerRequestHandle,
) -> Result<Vec<McpServerStatus>> {
    let mut cursor = None;
    let mut statuses = Vec::new();

    loop {
        let request_id = RequestId::String(format!("mcp-inventory-{}", Uuid::new_v4()));
        let response: ListMcpServerStatusResponse = request_handle
            .request_typed(ClientRequest::McpServerStatusList {
                request_id,
                params: ListMcpServerStatusParams {
                    cursor: cursor.clone(),
                    limit: Some(100),
                    detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
                },
            })
            .await
            .wrap_err("mcpServerStatus/list failed in TUI")?;
        statuses.extend(response.data);
        if let Some(next_cursor) = response.next_cursor {
            cursor = Some(next_cursor);
        } else {
            break;
        }
    }

    Ok(statuses)
}

async fn fetch_account_rate_limits(
    request_handle: AppServerRequestHandle,
) -> Result<Vec<RateLimitSnapshot>> {
    let request_id = RequestId::String(format!("account-rate-limits-{}", Uuid::new_v4()));
    let response: GetAccountRateLimitsResponse = request_handle
        .request_typed(ClientRequest::GetAccountRateLimits {
            request_id,
            params: None,
        })
        .await
        .wrap_err("account/rateLimits/read failed in TUI")?;

    Ok(app_server_rate_limit_snapshots_to_core(response))
}

async fn fetch_plugins_list(
    request_handle: AppServerRequestHandle,
    cwd: PathBuf,
) -> Result<PluginListResponse> {
    let cwd = AbsolutePathBuf::try_from(cwd).wrap_err("plugin list cwd must be absolute")?;
    let request_id = RequestId::String(format!("plugin-list-{}", Uuid::new_v4()));
    let mut response = request_handle
        .request_typed(ClientRequest::PluginList {
            request_id,
            params: PluginListParams {
                cwds: Some(vec![cwd]),
                force_remote_sync: false,
            },
        })
        .await
        .wrap_err("plugin/list failed in TUI")?;
    hide_cli_only_plugin_marketplaces(&mut response);
    Ok(response)
}

const CLI_HIDDEN_PLUGIN_MARKETPLACES: &[&str] = &["openai-bundled"];

fn hide_cli_only_plugin_marketplaces(response: &mut PluginListResponse) {
    response
        .marketplaces
        .retain(|marketplace| !CLI_HIDDEN_PLUGIN_MARKETPLACES.contains(&marketplace.name.as_str()));
}

async fn fetch_plugin_detail(
    request_handle: AppServerRequestHandle,
    params: PluginReadParams,
) -> Result<PluginReadResponse> {
    let request_id = RequestId::String(format!("plugin-read-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::PluginRead { request_id, params })
        .await
        .wrap_err("plugin/read failed in TUI")
}

async fn fetch_plugin_install(
    request_handle: AppServerRequestHandle,
    marketplace_path: AbsolutePathBuf,
    plugin_name: String,
) -> Result<PluginInstallResponse> {
    let request_id = RequestId::String(format!("plugin-install-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::PluginInstall {
            request_id,
            params: PluginInstallParams {
                marketplace_path,
                plugin_name,
                force_remote_sync: false,
            },
        })
        .await
        .wrap_err("plugin/install failed in TUI")
}

async fn fetch_plugin_uninstall(
    request_handle: AppServerRequestHandle,
    plugin_id: String,
) -> Result<PluginUninstallResponse> {
    let request_id = RequestId::String(format!("plugin-uninstall-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::PluginUninstall {
            request_id,
            params: PluginUninstallParams {
                plugin_id,
                force_remote_sync: false,
            },
        })
        .await
        .wrap_err("plugin/uninstall failed in TUI")
}

fn build_feedback_upload_params(
    origin_thread_id: Option<ThreadId>,
    rollout_path: Option<PathBuf>,
    category: FeedbackCategory,
    reason: Option<String>,
    turn_id: Option<String>,
    include_logs: bool,
) -> FeedbackUploadParams {
    let extra_log_files = if include_logs {
        rollout_path.map(|rollout_path| vec![rollout_path])
    } else {
        None
    };
    let tags = turn_id.map(|turn_id| BTreeMap::from([(String::from("turn_id"), turn_id)]));
    FeedbackUploadParams {
        classification: crate::bottom_pane::feedback_classification(category).to_string(),
        reason,
        thread_id: origin_thread_id.map(|thread_id| thread_id.to_string()),
        include_logs,
        extra_log_files,
        tags,
    }
}

async fn fetch_feedback_upload(
    request_handle: AppServerRequestHandle,
    params: FeedbackUploadParams,
) -> Result<FeedbackUploadResponse> {
    let request_id = RequestId::String(format!("feedback-upload-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::FeedbackUpload { request_id, params })
        .await
        .wrap_err("feedback/upload failed in TUI")
}

/// Convert flat `McpServerStatus` responses into the per-server maps used by the
/// in-process MCP subsystem (tools keyed as `mcp__{server}__{tool}`, plus
/// per-server resource/template/auth maps). Test-only because the TUI
/// renders directly from `McpServerStatus` rather than these maps.
#[cfg(test)]
type McpInventoryMaps = (
    HashMap<String, codex_protocol::mcp::Tool>,
    HashMap<String, Vec<codex_protocol::mcp::Resource>>,
    HashMap<String, Vec<codex_protocol::mcp::ResourceTemplate>>,
    HashMap<String, McpAuthStatus>,
);

#[cfg(test)]
fn mcp_inventory_maps_from_statuses(statuses: Vec<McpServerStatus>) -> McpInventoryMaps {
    let mut tools = HashMap::new();
    let mut resources = HashMap::new();
    let mut resource_templates = HashMap::new();
    let mut auth_statuses = HashMap::new();

    for status in statuses {
        let server_name = status.name;
        auth_statuses.insert(
            server_name.clone(),
            match status.auth_status {
                codex_app_server_protocol::McpAuthStatus::Unsupported => McpAuthStatus::Unsupported,
                codex_app_server_protocol::McpAuthStatus::NotLoggedIn => McpAuthStatus::NotLoggedIn,
                codex_app_server_protocol::McpAuthStatus::BearerToken => McpAuthStatus::BearerToken,
                codex_app_server_protocol::McpAuthStatus::OAuth => McpAuthStatus::OAuth,
            },
        );
        resources.insert(server_name.clone(), status.resources);
        resource_templates.insert(server_name.clone(), status.resource_templates);
        for (tool_name, tool) in status.tools {
            tools.insert(format!("mcp__{server_name}__{tool_name}"), tool);
        }
    }

    (tools, resources, resource_templates, auth_statuses)
}

impl Drop for App {
    fn drop(&mut self) {
        if let Err(err) = self.chat_widget.clear_managed_terminal_title() {
            tracing::debug!(error = %err, "failed to clear terminal title on app drop");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_backtrack::BacktrackSelection;
    use crate::app_backtrack::BacktrackState;
    use crate::app_backtrack::user_count;

    use crate::chatwidget::ChatWidgetInit;
    use crate::chatwidget::create_initial_user_message;
    use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
    use crate::chatwidget::tests::set_chatgpt_auth;
    use crate::chatwidget::tests::set_fast_mode_test_catalog;
    use crate::file_search::FileSearchManager;
    use crate::history_cell::AgentMessageCell;
    use crate::history_cell::HistoryCell;
    use crate::history_cell::UserHistoryCell;
    use crate::history_cell::new_session_info;
    use crate::multi_agents::AgentPickerThreadEntry;
    use assert_matches::assert_matches;

    use crate::legacy_core::config::ConfigBuilder;
    use crate::legacy_core::config::ConfigOverrides;
    use codex_app_server_protocol::AdditionalFileSystemPermissions;
    use codex_app_server_protocol::AdditionalNetworkPermissions;
    use codex_app_server_protocol::AdditionalPermissionProfile;
    use codex_app_server_protocol::AgentMessageDeltaNotification;
    use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
    use codex_app_server_protocol::ConfigWarningNotification;
    use codex_app_server_protocol::HookCompletedNotification;
    use codex_app_server_protocol::HookEventName as AppServerHookEventName;
    use codex_app_server_protocol::HookExecutionMode as AppServerHookExecutionMode;
    use codex_app_server_protocol::HookHandlerType as AppServerHookHandlerType;
    use codex_app_server_protocol::HookOutputEntry as AppServerHookOutputEntry;
    use codex_app_server_protocol::HookOutputEntryKind as AppServerHookOutputEntryKind;
    use codex_app_server_protocol::HookRunStatus as AppServerHookRunStatus;
    use codex_app_server_protocol::HookRunSummary as AppServerHookRunSummary;
    use codex_app_server_protocol::HookScope as AppServerHookScope;
    use codex_app_server_protocol::HookStartedNotification;
    use codex_app_server_protocol::JSONRPCErrorError;
    use codex_app_server_protocol::NetworkApprovalContext as AppServerNetworkApprovalContext;
    use codex_app_server_protocol::NetworkApprovalProtocol as AppServerNetworkApprovalProtocol;
    use codex_app_server_protocol::NetworkPolicyAmendment as AppServerNetworkPolicyAmendment;
    use codex_app_server_protocol::NetworkPolicyRuleAction as AppServerNetworkPolicyRuleAction;
    use codex_app_server_protocol::NonSteerableTurnKind as AppServerNonSteerableTurnKind;
    use codex_app_server_protocol::PermissionsRequestApprovalParams;
    use codex_app_server_protocol::PluginMarketplaceEntry;
    use codex_app_server_protocol::RequestId as AppServerRequestId;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::ServerRequest;
    use codex_app_server_protocol::Thread;
    use codex_app_server_protocol::ThreadClosedNotification;
    use codex_app_server_protocol::ThreadItem;
    use codex_app_server_protocol::ThreadStartedNotification;
    use codex_app_server_protocol::ThreadTokenUsage;
    use codex_app_server_protocol::ThreadTokenUsageUpdatedNotification;
    use codex_app_server_protocol::TokenUsageBreakdown;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnCompletedNotification;
    use codex_app_server_protocol::TurnError as AppServerTurnError;
    use codex_app_server_protocol::TurnStartedNotification;
    use codex_app_server_protocol::TurnStatus;
    use codex_app_server_protocol::UserInput as AppServerUserInput;
    use codex_config::types::ModelAvailabilityNuxConfig;
    use codex_otel::SessionTelemetry;
    use codex_protocol::ThreadId;
    use codex_protocol::config_types::CollaborationMode;
    use codex_protocol::config_types::CollaborationModeMask;
    use codex_protocol::config_types::ModeKind;
    use codex_protocol::config_types::Settings;
    use codex_protocol::mcp::Tool;
    use codex_protocol::models::FileSystemPermissions;
    use codex_protocol::models::NetworkPermissions;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::openai_models::ModelAvailabilityNux;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::Event;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::McpAuthStatus;
    use codex_protocol::protocol::NetworkApprovalContext;
    use codex_protocol::protocol::NetworkApprovalProtocol;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::RolloutLine;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_protocol::protocol::SessionConfiguredEvent;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::TurnContextItem;
    use codex_protocol::request_permissions::RequestPermissionProfile;
    use codex_protocol::user_input::TextElement;
    use codex_protocol::user_input::UserInput;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::prelude::Line;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tempfile::tempdir;
    use tokio::time;

    fn test_absolute_path(path: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(PathBuf::from(path)).expect("absolute test path")
    }

    #[test]
    fn hide_cli_only_plugin_marketplaces_removes_openai_bundled() {
        let mut response = PluginListResponse {
            marketplaces: vec![
                PluginMarketplaceEntry {
                    name: "openai-bundled".to_string(),
                    path: test_absolute_path("/marketplaces/openai-bundled"),
                    interface: None,
                    plugins: Vec::new(),
                },
                PluginMarketplaceEntry {
                    name: "openai-curated".to_string(),
                    path: test_absolute_path("/marketplaces/openai-curated"),
                    interface: None,
                    plugins: Vec::new(),
                },
            ],
            marketplace_load_errors: Vec::new(),
            remote_sync_error: None,
            featured_plugin_ids: Vec::new(),
        };

        hide_cli_only_plugin_marketplaces(&mut response);

        assert_eq!(
            response.marketplaces,
            vec![PluginMarketplaceEntry {
                name: "openai-curated".to_string(),
                path: test_absolute_path("/marketplaces/openai-curated"),
                interface: None,
                plugins: Vec::new(),
            }]
        );
    }

    #[test]
    fn normalize_harness_overrides_resolves_relative_add_dirs() -> Result<()> {
        let temp_dir = tempdir()?;
        let base_cwd = temp_dir.path().join("base").abs();
        std::fs::create_dir_all(base_cwd.as_path())?;

        let overrides = ConfigOverrides {
            additional_writable_roots: vec![PathBuf::from("rel")],
            ..Default::default()
        };
        let normalized = normalize_harness_overrides_for_cwd(overrides, &base_cwd)?;

        assert_eq!(
            normalized.additional_writable_roots,
            vec![base_cwd.join("rel").into_path_buf()]
        );
        Ok(())
    }

    #[test]
    fn mcp_inventory_maps_prefix_tool_names_by_server() {
        let statuses = vec![
            McpServerStatus {
                name: "docs".to_string(),
                tools: HashMap::from([(
                    "list".to_string(),
                    Tool {
                        description: None,
                        name: "list".to_string(),
                        title: None,
                        input_schema: serde_json::json!({"type": "object"}),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                        meta: None,
                    },
                )]),
                resources: Vec::new(),
                resource_templates: Vec::new(),
                auth_status: codex_app_server_protocol::McpAuthStatus::Unsupported,
            },
            McpServerStatus {
                name: "disabled".to_string(),
                tools: HashMap::new(),
                resources: Vec::new(),
                resource_templates: Vec::new(),
                auth_status: codex_app_server_protocol::McpAuthStatus::Unsupported,
            },
        ];

        let (tools, resources, resource_templates, auth_statuses) =
            mcp_inventory_maps_from_statuses(statuses);
        let mut resource_names = resources.keys().cloned().collect::<Vec<_>>();
        resource_names.sort();
        let mut template_names = resource_templates.keys().cloned().collect::<Vec<_>>();
        template_names.sort();

        assert_eq!(
            tools.keys().cloned().collect::<Vec<_>>(),
            vec!["mcp__docs__list".to_string()]
        );
        assert_eq!(resource_names, vec!["disabled", "docs"]);
        assert_eq!(template_names, vec!["disabled", "docs"]);
        assert_eq!(
            auth_statuses.get("disabled"),
            Some(&McpAuthStatus::Unsupported)
        );
    }

    #[tokio::test]
    async fn handle_mcp_inventory_result_clears_committed_loading_cell() {
        let mut app = make_test_app().await;
        app.transcript_cells
            .push(Arc::new(history_cell::new_mcp_inventory_loading(
                /*animations_enabled*/ false,
            )));

        app.handle_mcp_inventory_result(Ok(vec![McpServerStatus {
            name: "docs".to_string(),
            tools: HashMap::new(),
            resources: Vec::new(),
            resource_templates: Vec::new(),
            auth_status: codex_app_server_protocol::McpAuthStatus::Unsupported,
        }]));

        assert_eq!(app.transcript_cells.len(), 0);
    }

    #[test]
    fn startup_waiting_gate_is_only_for_fresh_or_exit_session_selection() {
        assert_eq!(
            App::should_wait_for_initial_session(&SessionSelection::StartFresh),
            true
        );
        assert_eq!(
            App::should_wait_for_initial_session(&SessionSelection::Exit),
            true
        );
        assert_eq!(
            App::should_wait_for_initial_session(&SessionSelection::Resume(
                crate::resume_picker::SessionTarget {
                    path: Some(PathBuf::from("/tmp/restore")),
                    thread_id: ThreadId::new(),
                }
            )),
            false
        );
        assert_eq!(
            App::should_wait_for_initial_session(&SessionSelection::Fork(
                crate::resume_picker::SessionTarget {
                    path: Some(PathBuf::from("/tmp/fork")),
                    thread_id: ThreadId::new(),
                }
            )),
            false
        );
    }

    #[test]
    fn startup_waiting_gate_holds_active_thread_events_until_primary_thread_configured() {
        let mut wait_for_initial_session =
            App::should_wait_for_initial_session(&SessionSelection::StartFresh);
        assert_eq!(wait_for_initial_session, true);
        assert_eq!(
            App::should_handle_active_thread_events(
                wait_for_initial_session,
                /*has_active_thread_receiver*/ true
            ),
            false
        );

        assert_eq!(
            App::should_stop_waiting_for_initial_session(
                wait_for_initial_session,
                /*primary_thread_id*/ None
            ),
            false
        );
        if App::should_stop_waiting_for_initial_session(
            wait_for_initial_session,
            Some(ThreadId::new()),
        ) {
            wait_for_initial_session = false;
        }
        assert_eq!(wait_for_initial_session, false);

        assert_eq!(
            App::should_handle_active_thread_events(
                wait_for_initial_session,
                /*has_active_thread_receiver*/ true
            ),
            true
        );
    }

    #[test]
    fn startup_waiting_gate_not_applied_for_resume_or_fork_session_selection() {
        let wait_for_resume = App::should_wait_for_initial_session(&SessionSelection::Resume(
            crate::resume_picker::SessionTarget {
                path: Some(PathBuf::from("/tmp/restore")),
                thread_id: ThreadId::new(),
            },
        ));
        assert_eq!(
            App::should_handle_active_thread_events(
                wait_for_resume,
                /*has_active_thread_receiver*/ true
            ),
            true
        );
        let wait_for_fork = App::should_wait_for_initial_session(&SessionSelection::Fork(
            crate::resume_picker::SessionTarget {
                path: Some(PathBuf::from("/tmp/fork")),
                thread_id: ThreadId::new(),
            },
        ));
        assert_eq!(
            App::should_handle_active_thread_events(
                wait_for_fork,
                /*has_active_thread_receiver*/ true
            ),
            true
        );
    }

    #[tokio::test]
    async fn ignore_same_thread_resume_reports_noop_for_current_thread() {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.chat_widget.handle_thread_session(session.clone());
        app.thread_event_channels.insert(
            thread_id,
            ThreadEventChannel::new_with_session(
                THREAD_EVENT_CHANNEL_CAPACITY,
                session,
                Vec::new(),
            ),
        );
        app.activate_thread_channel(thread_id).await;
        while app_event_rx.try_recv().is_ok() {}

        let ignored = app.ignore_same_thread_resume(&crate::resume_picker::SessionTarget {
            path: Some(test_path_buf("/tmp/project")),
            thread_id,
        });

        assert!(ignored);
        let cell = match app_event_rx.try_recv() {
            Ok(AppEvent::InsertHistoryCell(cell)) => cell,
            other => panic!("expected info message after same-thread resume, saw {other:?}"),
        };
        let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 80));
        assert!(rendered.contains(&format!(
            "Already viewing {}.",
            test_path_display("/tmp/project")
        )));
    }

    #[tokio::test]
    async fn ignore_same_thread_resume_allows_reattaching_displayed_inactive_thread() {
        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.chat_widget.handle_thread_session(session);

        let ignored = app.ignore_same_thread_resume(&crate::resume_picker::SessionTarget {
            path: Some(test_path_buf("/tmp/project")),
            thread_id,
        });

        assert!(!ignored);
        assert!(app.transcript_cells.is_empty());
    }

    #[tokio::test]
    async fn enqueue_primary_thread_session_replays_buffered_approval_after_attach() -> Result<()> {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let approval_request =
            exec_approval_request(thread_id, "turn-1", "call-1", /*approval_id*/ None);

        assert_eq!(
            app.pending_app_server_requests
                .note_server_request(&approval_request),
            None
        );
        app.enqueue_primary_thread_request(approval_request).await?;
        app.enqueue_primary_thread_session(
            test_thread_session(thread_id, test_path_buf("/tmp/project")),
            Vec::new(),
        )
        .await?;

        let rx = app
            .active_thread_rx
            .as_mut()
            .expect("primary thread receiver should be active");
        let event = time::timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("timed out waiting for buffered approval event")
            .expect("channel closed unexpectedly");

        assert!(matches!(
            &event,
            ThreadBufferedEvent::Request(ServerRequest::CommandExecutionRequestApproval {
                params,
                ..
            }) if params.turn_id == "turn-1"
        ));

        app.handle_thread_event_now(event);
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

        while let Ok(app_event) = app_event_rx.try_recv() {
            if let AppEvent::SubmitThreadOp {
                thread_id: op_thread_id,
                ..
            } = app_event
            {
                assert_eq!(op_thread_id, thread_id);
                return Ok(());
            }
        }

        panic!("expected approval action to submit a thread-scoped op");
    }

    #[tokio::test]
    async fn resolved_buffered_approval_does_not_become_actionable_after_drain() -> Result<()> {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let approval_request =
            exec_approval_request(thread_id, "turn-1", "call-1", /*approval_id*/ None);

        app.enqueue_primary_thread_session(
            test_thread_session(thread_id, test_path_buf("/tmp/project")),
            Vec::new(),
        )
        .await?;
        while app_event_rx.try_recv().is_ok() {}

        assert_eq!(
            app.pending_app_server_requests
                .note_server_request(&approval_request),
            None
        );
        app.enqueue_thread_request(thread_id, approval_request)
            .await?;

        let resolved = app
            .pending_app_server_requests
            .resolve_notification(&AppServerRequestId::Integer(1))
            .expect("matching app-server request should resolve");
        app.chat_widget.dismiss_app_server_request(&resolved);
        while app_event_rx.try_recv().is_ok() {}

        let rx = app
            .active_thread_rx
            .as_mut()
            .expect("primary thread receiver should be active");
        let event = time::timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("timed out waiting for buffered approval event")
            .expect("channel closed unexpectedly");

        assert!(matches!(
            &event,
            ThreadBufferedEvent::Request(ServerRequest::CommandExecutionRequestApproval {
                params,
                ..
            }) if params.turn_id == "turn-1"
        ));

        app.handle_thread_event_now(event);
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

        while let Ok(app_event) = app_event_rx.try_recv() {
            assert!(
                !matches!(app_event, AppEvent::SubmitThreadOp { .. }),
                "resolved buffered approval should not become actionable"
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn enqueue_primary_thread_session_replays_turns_before_initial_prompt_submit()
    -> Result<()> {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let initial_prompt = "follow-up after replay".to_string();
        let config = app.config.clone();
        let model = crate::legacy_core::test_support::get_model_offline(config.model.as_deref());
        app.chat_widget = ChatWidget::new_with_app_event(ChatWidgetInit {
            config,
            frame_requester: crate::tui::FrameRequester::test_dummy(),
            app_event_tx: app.app_event_tx.clone(),
            initial_user_message: create_initial_user_message(
                Some(initial_prompt.clone()),
                Vec::new(),
                Vec::new(),
            ),
            enhanced_keys_supported: false,
            has_chatgpt_account: false,
            model_catalog: app.model_catalog.clone(),
            feedback: codex_feedback::CodexFeedback::new(),
            is_first_run: false,
            status_account_display: None,
            initial_plan_type: None,
            model: Some(model),
            startup_tooltip_override: None,
            status_line_invalid_items_warned: app.status_line_invalid_items_warned.clone(),
            terminal_title_invalid_items_warned: app.terminal_title_invalid_items_warned.clone(),
            session_telemetry: app.session_telemetry.clone(),
        });

        app.enqueue_primary_thread_session(
            test_thread_session(thread_id, test_path_buf("/tmp/project")),
            vec![test_turn(
                "turn-1",
                TurnStatus::Completed,
                vec![ThreadItem::UserMessage {
                    id: "user-1".to_string(),
                    content: vec![AppServerUserInput::Text {
                        text: "earlier prompt".to_string(),
                        text_elements: Vec::new(),
                    }],
                }],
            )],
        )
        .await?;

        let mut saw_replayed_answer = false;
        let mut submitted_items = None;
        while let Ok(event) = app_event_rx.try_recv() {
            match event {
                AppEvent::InsertHistoryCell(cell) => {
                    let transcript = lines_to_single_string(&cell.transcript_lines(/*width*/ 80));
                    saw_replayed_answer |= transcript.contains("earlier prompt");
                }
                AppEvent::SubmitThreadOp {
                    thread_id: op_thread_id,
                    op: Op::UserTurn { items, .. },
                } => {
                    assert_eq!(op_thread_id, thread_id);
                    submitted_items = Some(items);
                }
                AppEvent::CodexOp(Op::UserTurn { items, .. }) => {
                    submitted_items = Some(items);
                }
                _ => {}
            }
        }
        assert!(
            saw_replayed_answer,
            "expected replayed history before initial prompt submit"
        );
        assert_eq!(
            submitted_items,
            Some(vec![UserInput::Text {
                text: initial_prompt,
                text_elements: Vec::new(),
            }])
        );

        Ok(())
    }

    #[tokio::test]
    async fn reset_thread_event_state_aborts_listener_tasks() {
        struct NotifyOnDrop(Option<tokio::sync::oneshot::Sender<()>>);

        impl Drop for NotifyOnDrop {
            fn drop(&mut self) {
                if let Some(tx) = self.0.take() {
                    let _ = tx.send(());
                }
            }
        }

        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (dropped_tx, dropped_rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn(async move {
            let _notify_on_drop = NotifyOnDrop(Some(dropped_tx));
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        });
        app.thread_event_listener_tasks.insert(thread_id, handle);
        started_rx
            .await
            .expect("listener task should report it started");

        app.reset_thread_event_state();

        assert_eq!(app.thread_event_listener_tasks.is_empty(), true);
        time::timeout(Duration::from_millis(50), dropped_rx)
            .await
            .expect("timed out waiting for listener task abort")
            .expect("listener task drop notification should succeed");
    }

    #[tokio::test]
    async fn history_lookup_response_is_routed_to_requesting_thread() -> Result<()> {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();

        let handled = app
            .try_handle_local_history_op(
                thread_id,
                &Op::GetHistoryEntryRequest {
                    offset: 0,
                    log_id: 1,
                }
                .into(),
            )
            .await?;

        assert!(handled);

        let app_event = tokio::time::timeout(Duration::from_secs(1), app_event_rx.recv())
            .await
            .expect("history lookup should emit an app event")
            .expect("app event channel should stay open");

        let AppEvent::ThreadHistoryEntryResponse {
            thread_id: routed_thread_id,
            event,
        } = app_event
        else {
            panic!("expected thread-routed history response");
        };
        assert_eq!(routed_thread_id, thread_id);
        assert_eq!(event.offset, 0);
        assert_eq!(event.log_id, 1);
        assert!(event.entry.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn enqueue_thread_event_does_not_block_when_channel_full() -> Result<()> {
        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        app.thread_event_channels
            .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));
        app.set_thread_active(thread_id, /*active*/ true).await;

        let event = thread_closed_notification(thread_id);

        app.enqueue_thread_notification(thread_id, event.clone())
            .await?;
        time::timeout(
            Duration::from_millis(50),
            app.enqueue_thread_notification(thread_id, event),
        )
        .await
        .expect("enqueue_thread_notification blocked on a full channel")?;

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

    #[tokio::test]
    async fn replay_thread_snapshot_restores_draft_and_queued_input() {
        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.thread_event_channels.insert(
            thread_id,
            ThreadEventChannel::new_with_session(
                THREAD_EVENT_CHANNEL_CAPACITY,
                session.clone(),
                Vec::new(),
            ),
        );
        app.activate_thread_channel(thread_id).await;
        app.chat_widget.handle_thread_session(session.clone());

        app.chat_widget
            .apply_external_edit("draft prompt".to_string());
        app.chat_widget.submit_user_message_with_mode(
            "queued follow-up".to_string(),
            CollaborationModeMask {
                name: "Default".to_string(),
                mode: None,
                model: None,
                reasoning_effort: None,
                developer_instructions: None,
            },
        );
        let expected_input_state = app
            .chat_widget
            .capture_thread_input_state()
            .expect("expected thread input state");

        app.store_active_thread_receiver().await;

        let snapshot = {
            let channel = app
                .thread_event_channels
                .get(&thread_id)
                .expect("thread channel should exist");
            let store = channel.store.lock().await;
            assert_eq!(store.input_state, Some(expected_input_state));
            store.snapshot()
        };

        let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
            make_chatwidget_manual_with_sender().await;
        app.chat_widget = chat_widget;

        app.replay_thread_snapshot(snapshot, /*resume_restored_queue*/ true);

        assert_eq!(app.chat_widget.composer_text_with_pending(), "draft prompt");
        assert!(app.chat_widget.queued_user_message_texts().is_empty());
        while let Ok(op) = new_op_rx.try_recv() {
            assert!(
                !matches!(op, Op::UserTurn { .. }),
                "draft-only replay should not auto-submit queued input"
            );
        }
    }

    #[tokio::test]
    async fn active_turn_id_for_thread_uses_snapshot_turns() {
        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.thread_event_channels.insert(
            thread_id,
            ThreadEventChannel::new_with_session(
                THREAD_EVENT_CHANNEL_CAPACITY,
                session,
                vec![test_turn("turn-1", TurnStatus::InProgress, Vec::new())],
            ),
        );

        assert_eq!(
            app.active_turn_id_for_thread(thread_id).await,
            Some("turn-1".to_string())
        );
    }

    #[tokio::test]
    async fn replayed_turn_complete_submits_restored_queued_follow_up() {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.chat_widget.handle_thread_session(session.clone());
        app.chat_widget.handle_server_notification(
            turn_started_notification(thread_id, "turn-1"),
            /*replay_kind*/ None,
        );
        app.chat_widget.handle_server_notification(
            agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
            /*replay_kind*/ None,
        );
        app.chat_widget
            .apply_external_edit("queued follow-up".to_string());
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let input_state = app
            .chat_widget
            .capture_thread_input_state()
            .expect("expected queued follow-up state");

        let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
            make_chatwidget_manual_with_sender().await;
        app.chat_widget = chat_widget;
        app.chat_widget.handle_thread_session(session.clone());
        while new_op_rx.try_recv().is_ok() {}
        app.replay_thread_snapshot(
            ThreadEventSnapshot {
                session: None,
                turns: Vec::new(),
                events: vec![ThreadBufferedEvent::Notification(
                    turn_completed_notification(thread_id, "turn-1", TurnStatus::Completed),
                )],
                input_state: Some(input_state),
            },
            /*resume_restored_queue*/ true,
        );

        match next_user_turn_op(&mut new_op_rx) {
            Op::UserTurn { items, .. } => assert_eq!(
                items,
                vec![UserInput::Text {
                    text: "queued follow-up".to_string(),
                    text_elements: Vec::new(),
                }]
            ),
            other => panic!("expected queued follow-up submission, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn replay_only_thread_keeps_restored_queue_visible() {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.chat_widget.handle_thread_session(session.clone());
        app.chat_widget.handle_server_notification(
            turn_started_notification(thread_id, "turn-1"),
            /*replay_kind*/ None,
        );
        app.chat_widget.handle_server_notification(
            agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
            /*replay_kind*/ None,
        );
        app.chat_widget
            .apply_external_edit("queued follow-up".to_string());
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let input_state = app
            .chat_widget
            .capture_thread_input_state()
            .expect("expected queued follow-up state");

        let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
            make_chatwidget_manual_with_sender().await;
        app.chat_widget = chat_widget;
        app.chat_widget.handle_thread_session(session.clone());
        while new_op_rx.try_recv().is_ok() {}

        app.replay_thread_snapshot(
            ThreadEventSnapshot {
                session: None,
                turns: Vec::new(),
                events: vec![ThreadBufferedEvent::Notification(
                    turn_completed_notification(thread_id, "turn-1", TurnStatus::Completed),
                )],
                input_state: Some(input_state),
            },
            /*resume_restored_queue*/ false,
        );

        assert_eq!(
            app.chat_widget.queued_user_message_texts(),
            vec!["queued follow-up".to_string()]
        );
        assert!(
            new_op_rx.try_recv().is_err(),
            "replay-only threads should not auto-submit restored queue"
        );
    }

    #[tokio::test]
    async fn replay_thread_snapshot_keeps_queue_when_running_state_only_comes_from_snapshot() {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.chat_widget.handle_thread_session(session.clone());
        app.chat_widget.handle_server_notification(
            turn_started_notification(thread_id, "turn-1"),
            /*replay_kind*/ None,
        );
        app.chat_widget.handle_server_notification(
            agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
            /*replay_kind*/ None,
        );
        app.chat_widget
            .apply_external_edit("queued follow-up".to_string());
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let input_state = app
            .chat_widget
            .capture_thread_input_state()
            .expect("expected queued follow-up state");

        let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
            make_chatwidget_manual_with_sender().await;
        app.chat_widget = chat_widget;
        app.chat_widget.handle_thread_session(session.clone());
        while new_op_rx.try_recv().is_ok() {}

        app.replay_thread_snapshot(
            ThreadEventSnapshot {
                session: None,
                turns: Vec::new(),
                events: vec![],
                input_state: Some(input_state),
            },
            /*resume_restored_queue*/ true,
        );

        assert_eq!(
            app.chat_widget.queued_user_message_texts(),
            vec!["queued follow-up".to_string()]
        );
        assert!(
            new_op_rx.try_recv().is_err(),
            "restored queue should stay queued when replay did not prove the turn finished"
        );
    }

    #[tokio::test]
    async fn replay_thread_snapshot_in_progress_turn_restores_running_queue_state() {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.chat_widget.handle_thread_session(session.clone());
        app.chat_widget.handle_server_notification(
            turn_started_notification(thread_id, "turn-1"),
            /*replay_kind*/ None,
        );
        app.chat_widget.handle_server_notification(
            agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
            /*replay_kind*/ None,
        );
        app.chat_widget
            .apply_external_edit("queued follow-up".to_string());
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let input_state = app
            .chat_widget
            .capture_thread_input_state()
            .expect("expected queued follow-up state");

        let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
            make_chatwidget_manual_with_sender().await;
        app.chat_widget = chat_widget;
        app.chat_widget.handle_thread_session(session.clone());
        while new_op_rx.try_recv().is_ok() {}

        app.replay_thread_snapshot(
            ThreadEventSnapshot {
                session: None,
                turns: vec![test_turn("turn-1", TurnStatus::InProgress, Vec::new())],
                events: Vec::new(),
                input_state: Some(input_state),
            },
            /*resume_restored_queue*/ true,
        );

        assert_eq!(
            app.chat_widget.queued_user_message_texts(),
            vec!["queued follow-up".to_string()]
        );
        assert!(
            new_op_rx.try_recv().is_err(),
            "restored queue should stay queued while replayed turn is still running"
        );
    }

    #[tokio::test]
    async fn replay_thread_snapshot_in_progress_turn_restores_running_state_without_input_state() {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        let (chat_widget, _app_event_tx, _rx, _new_op_rx) =
            make_chatwidget_manual_with_sender().await;
        app.chat_widget = chat_widget;
        app.chat_widget.handle_thread_session(session);

        app.replay_thread_snapshot(
            ThreadEventSnapshot {
                session: None,
                turns: vec![test_turn("turn-1", TurnStatus::InProgress, Vec::new())],
                events: Vec::new(),
                input_state: None,
            },
            /*resume_restored_queue*/ false,
        );

        assert!(app.chat_widget.is_task_running_for_test());
    }

    #[tokio::test]
    async fn replay_thread_snapshot_does_not_submit_queue_before_replay_catches_up() {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.chat_widget.handle_thread_session(session.clone());
        app.chat_widget.handle_server_notification(
            turn_started_notification(thread_id, "turn-1"),
            /*replay_kind*/ None,
        );
        app.chat_widget.handle_server_notification(
            agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
            /*replay_kind*/ None,
        );
        app.chat_widget
            .apply_external_edit("queued follow-up".to_string());
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let input_state = app
            .chat_widget
            .capture_thread_input_state()
            .expect("expected queued follow-up state");

        let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
            make_chatwidget_manual_with_sender().await;
        app.chat_widget = chat_widget;
        app.chat_widget.handle_thread_session(session.clone());
        while new_op_rx.try_recv().is_ok() {}

        app.replay_thread_snapshot(
            ThreadEventSnapshot {
                session: None,
                turns: Vec::new(),
                events: vec![
                    ThreadBufferedEvent::Notification(turn_completed_notification(
                        thread_id,
                        "turn-0",
                        TurnStatus::Completed,
                    )),
                    ThreadBufferedEvent::Notification(turn_started_notification(
                        thread_id, "turn-1",
                    )),
                ],
                input_state: Some(input_state),
            },
            /*resume_restored_queue*/ true,
        );

        assert!(
            new_op_rx.try_recv().is_err(),
            "queued follow-up should stay queued until the latest turn completes"
        );
        assert_eq!(
            app.chat_widget.queued_user_message_texts(),
            vec!["queued follow-up".to_string()]
        );

        app.chat_widget.handle_server_notification(
            turn_completed_notification(thread_id, "turn-1", TurnStatus::Completed),
            /*replay_kind*/ None,
        );

        match next_user_turn_op(&mut new_op_rx) {
            Op::UserTurn { items, .. } => assert_eq!(
                items,
                vec![UserInput::Text {
                    text: "queued follow-up".to_string(),
                    text_elements: Vec::new(),
                }]
            ),
            other => panic!("expected queued follow-up submission, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn replay_thread_snapshot_restores_pending_pastes_for_submit() {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.thread_event_channels.insert(
            thread_id,
            ThreadEventChannel::new_with_session(
                THREAD_EVENT_CHANNEL_CAPACITY,
                session.clone(),
                Vec::new(),
            ),
        );
        app.activate_thread_channel(thread_id).await;
        app.chat_widget.handle_thread_session(session);

        let large = "x".repeat(1005);
        app.chat_widget.handle_paste(large.clone());
        let expected_input_state = app
            .chat_widget
            .capture_thread_input_state()
            .expect("expected thread input state");

        app.store_active_thread_receiver().await;

        let snapshot = {
            let channel = app
                .thread_event_channels
                .get(&thread_id)
                .expect("thread channel should exist");
            let store = channel.store.lock().await;
            assert_eq!(store.input_state, Some(expected_input_state));
            store.snapshot()
        };

        let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
            make_chatwidget_manual_with_sender().await;
        app.chat_widget = chat_widget;
        app.replay_thread_snapshot(snapshot, /*resume_restored_queue*/ true);

        assert_eq!(app.chat_widget.composer_text_with_pending(), large);

        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        match next_user_turn_op(&mut new_op_rx) {
            Op::UserTurn { items, .. } => assert_eq!(
                items,
                vec![UserInput::Text {
                    text: large,
                    text_elements: Vec::new(),
                }]
            ),
            other => panic!("expected restored paste submission, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn replay_thread_snapshot_restores_collaboration_mode_for_draft_submit() {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.chat_widget.handle_thread_session(session.clone());
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::High));
        app.chat_widget
            .set_collaboration_mask(CollaborationModeMask {
                name: "Plan".to_string(),
                mode: Some(ModeKind::Plan),
                model: Some("gpt-restored".to_string()),
                reasoning_effort: Some(Some(ReasoningEffortConfig::High)),
                developer_instructions: None,
            });
        app.chat_widget
            .apply_external_edit("draft prompt".to_string());
        let input_state = app
            .chat_widget
            .capture_thread_input_state()
            .expect("expected draft input state");

        let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
            make_chatwidget_manual_with_sender().await;
        app.chat_widget = chat_widget;
        app.chat_widget.handle_thread_session(session.clone());
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::Low));
        app.chat_widget
            .set_collaboration_mask(CollaborationModeMask {
                name: "Default".to_string(),
                mode: Some(ModeKind::Default),
                model: Some("gpt-replacement".to_string()),
                reasoning_effort: Some(Some(ReasoningEffortConfig::Low)),
                developer_instructions: None,
            });
        while new_op_rx.try_recv().is_ok() {}

        app.replay_thread_snapshot(
            ThreadEventSnapshot {
                session: None,
                turns: Vec::new(),
                events: vec![],
                input_state: Some(input_state),
            },
            /*resume_restored_queue*/ true,
        );
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        match next_user_turn_op(&mut new_op_rx) {
            Op::UserTurn {
                items,
                model,
                effort,
                collaboration_mode,
                ..
            } => {
                assert_eq!(
                    items,
                    vec![UserInput::Text {
                        text: "draft prompt".to_string(),
                        text_elements: Vec::new(),
                    }]
                );
                assert_eq!(model, "gpt-restored".to_string());
                assert_eq!(effort, Some(ReasoningEffortConfig::High));
                assert_eq!(
                    collaboration_mode,
                    Some(CollaborationMode {
                        mode: ModeKind::Plan,
                        settings: Settings {
                            model: "gpt-restored".to_string(),
                            reasoning_effort: Some(ReasoningEffortConfig::High),
                            developer_instructions: None,
                        },
                    })
                );
            }
            other => panic!("expected restored draft submission, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn replay_thread_snapshot_restores_collaboration_mode_without_input() {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.chat_widget.handle_thread_session(session.clone());
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::High));
        app.chat_widget
            .set_collaboration_mask(CollaborationModeMask {
                name: "Plan".to_string(),
                mode: Some(ModeKind::Plan),
                model: Some("gpt-restored".to_string()),
                reasoning_effort: Some(Some(ReasoningEffortConfig::High)),
                developer_instructions: None,
            });
        let input_state = app
            .chat_widget
            .capture_thread_input_state()
            .expect("expected collaboration-only input state");

        let (chat_widget, _app_event_tx, _rx, _new_op_rx) =
            make_chatwidget_manual_with_sender().await;
        app.chat_widget = chat_widget;
        app.chat_widget.handle_thread_session(session.clone());
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::Low));
        app.chat_widget
            .set_collaboration_mask(CollaborationModeMask {
                name: "Default".to_string(),
                mode: Some(ModeKind::Default),
                model: Some("gpt-replacement".to_string()),
                reasoning_effort: Some(Some(ReasoningEffortConfig::Low)),
                developer_instructions: None,
            });

        app.replay_thread_snapshot(
            ThreadEventSnapshot {
                session: None,
                turns: Vec::new(),
                events: vec![],
                input_state: Some(input_state),
            },
            /*resume_restored_queue*/ true,
        );

        assert_eq!(
            app.chat_widget.active_collaboration_mode_kind(),
            ModeKind::Plan
        );
        assert_eq!(app.chat_widget.current_model(), "gpt-restored");
        assert_eq!(
            app.chat_widget.current_reasoning_effort(),
            Some(ReasoningEffortConfig::High)
        );
    }

    #[tokio::test]
    async fn replayed_interrupted_turn_restores_queued_input_to_composer() {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        app.chat_widget.handle_thread_session(session.clone());
        app.chat_widget.handle_server_notification(
            turn_started_notification(thread_id, "turn-1"),
            /*replay_kind*/ None,
        );
        app.chat_widget.handle_server_notification(
            agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
            /*replay_kind*/ None,
        );
        app.chat_widget
            .apply_external_edit("queued follow-up".to_string());
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let input_state = app
            .chat_widget
            .capture_thread_input_state()
            .expect("expected queued follow-up state");

        let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
            make_chatwidget_manual_with_sender().await;
        app.chat_widget = chat_widget;
        app.chat_widget.handle_thread_session(session.clone());
        while new_op_rx.try_recv().is_ok() {}

        app.replay_thread_snapshot(
            ThreadEventSnapshot {
                session: None,
                turns: Vec::new(),
                events: vec![ThreadBufferedEvent::Notification(
                    turn_completed_notification(thread_id, "turn-1", TurnStatus::Interrupted),
                )],
                input_state: Some(input_state),
            },
            /*resume_restored_queue*/ true,
        );

        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "queued follow-up"
        );
        assert!(app.chat_widget.queued_user_message_texts().is_empty());
        assert!(
            new_op_rx.try_recv().is_err(),
            "replayed interrupted turns should restore queued input for editing, not submit it"
        );
    }

    #[tokio::test]
    async fn token_usage_update_refreshes_status_line_with_runtime_context_window() {
        let mut app = make_test_app().await;
        app.chat_widget
            .setup_status_line(vec![crate::bottom_pane::StatusLineItem::ContextWindowSize]);

        assert_eq!(app.chat_widget.status_line_text(), None);

        app.handle_thread_event_now(ThreadBufferedEvent::Notification(token_usage_notification(
            ThreadId::new(),
            "turn-1",
            Some(950_000),
        )));

        assert_eq!(
            app.chat_widget.status_line_text(),
            Some("950K window".into())
        );
    }

    #[tokio::test]
    async fn open_agent_picker_keeps_missing_threads_for_replay() -> Result<()> {
        let mut app = make_test_app().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let thread_id = ThreadId::new();
        app.thread_event_channels
            .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));

        app.open_agent_picker(&mut app_server).await;

        assert_eq!(app.thread_event_channels.contains_key(&thread_id), true);
        assert_eq!(
            app.agent_navigation.get(&thread_id),
            Some(&AgentPickerThreadEntry {
                agent_nickname: None,
                agent_role: None,
                is_closed: true,
            })
        );
        assert_eq!(app.agent_navigation.ordered_thread_ids(), vec![thread_id]);
        Ok(())
    }

    #[tokio::test]
    async fn open_agent_picker_preserves_cached_metadata_for_replay_threads() -> Result<()> {
        let mut app = make_test_app().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let thread_id = ThreadId::new();
        app.thread_event_channels
            .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));
        app.agent_navigation.upsert(
            thread_id,
            Some("Robie".to_string()),
            Some("explorer".to_string()),
            /*is_closed*/ true,
        );

        app.open_agent_picker(&mut app_server).await;

        assert_eq!(app.thread_event_channels.contains_key(&thread_id), true);
        assert_eq!(
            app.agent_navigation.get(&thread_id),
            Some(&AgentPickerThreadEntry {
                agent_nickname: Some("Robie".to_string()),
                agent_role: Some("explorer".to_string()),
                is_closed: true,
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn open_agent_picker_prunes_terminal_metadata_only_threads() -> Result<()> {
        let mut app = make_test_app().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let thread_id = ThreadId::new();
        app.agent_navigation.upsert(
            thread_id,
            Some("Ghost".to_string()),
            Some("worker".to_string()),
            /*is_closed*/ false,
        );

        app.open_agent_picker(&mut app_server).await;

        assert_eq!(app.agent_navigation.get(&thread_id), None);
        assert!(app.agent_navigation.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn open_agent_picker_marks_terminal_read_errors_closed() -> Result<()> {
        let mut app = make_test_app().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let thread_id = ThreadId::new();
        app.thread_event_channels
            .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));
        app.agent_navigation.upsert(
            thread_id,
            Some("Robie".to_string()),
            Some("explorer".to_string()),
            /*is_closed*/ false,
        );

        app.open_agent_picker(&mut app_server).await;

        assert_eq!(
            app.agent_navigation.get(&thread_id),
            Some(&AgentPickerThreadEntry {
                agent_nickname: Some("Robie".to_string()),
                agent_role: Some("explorer".to_string()),
                is_closed: true,
            })
        );
        Ok(())
    }

    #[test]
    fn terminal_thread_read_error_detection_matches_not_loaded_errors() {
        let err = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read failed: thread not loaded: thr_123"
        );

        assert!(App::is_terminal_thread_read_error(&err));
    }

    #[test]
    fn terminal_thread_read_error_detection_ignores_transient_failures() {
        let err = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read transport error: broken pipe"
        );

        assert!(!App::is_terminal_thread_read_error(&err));
    }

    #[test]
    fn closed_state_for_thread_read_error_preserves_live_state_without_cache_on_transient_error() {
        let err = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read transport error: broken pipe"
        );

        assert!(!App::closed_state_for_thread_read_error(
            &err, /*existing_is_closed*/ None
        ));
    }

    #[test]
    fn closed_state_for_thread_read_error_marks_terminal_uncached_threads_closed() {
        let err = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read failed: thread not loaded: thr_123"
        );

        assert!(App::closed_state_for_thread_read_error(
            &err, /*existing_is_closed*/ None
        ));
    }

    #[test]
    fn include_turns_fallback_detection_handles_unmaterialized_and_ephemeral_threads() {
        let unmaterialized = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read failed: thread thr_123 is not materialized yet; includeTurns is unavailable before first user message"
        );
        let ephemeral = color_eyre::eyre::eyre!(
            "thread/read failed during TUI session lookup: thread/read failed: ephemeral threads do not support includeTurns"
        );

        assert!(App::can_fallback_from_include_turns_error(&unmaterialized));
        assert!(App::can_fallback_from_include_turns_error(&ephemeral));
    }

    #[tokio::test]
    async fn open_agent_picker_marks_loaded_threads_open() -> Result<()> {
        let mut app = make_test_app().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let started = app_server
            .start_thread(app.chat_widget.config_ref())
            .await?;
        let thread_id = started.session.thread_id;
        app.thread_event_channels
            .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));

        app.open_agent_picker(&mut app_server).await;

        assert_eq!(
            app.agent_navigation.get(&thread_id),
            Some(&AgentPickerThreadEntry {
                agent_nickname: None,
                agent_role: None,
                is_closed: false,
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn attach_live_thread_for_selection_rejects_empty_non_ephemeral_fallback_threads()
    -> Result<()> {
        let mut app = make_test_app().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let started = app_server
            .start_thread(app.chat_widget.config_ref())
            .await?;
        let thread_id = started.session.thread_id;
        app.agent_navigation.upsert(
            thread_id,
            Some("Scout".to_string()),
            Some("worker".to_string()),
            /*is_closed*/ false,
        );

        let err = app
            .attach_live_thread_for_selection(&mut app_server, thread_id)
            .await
            .expect_err("empty fallback should not attach as a blank replay-only thread");

        assert_eq!(
            err.to_string(),
            format!("Agent thread {thread_id} is not yet available for replay or live attach.")
        );
        assert!(!app.thread_event_channels.contains_key(&thread_id));
        Ok(())
    }

    #[tokio::test]
    async fn attach_live_thread_for_selection_rejects_unmaterialized_fallback_threads() -> Result<()>
    {
        let mut app = make_test_app().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let mut ephemeral_config = app.chat_widget.config_ref().clone();
        ephemeral_config.ephemeral = true;
        let started = app_server.start_thread(&ephemeral_config).await?;
        let thread_id = started.session.thread_id;
        app.agent_navigation.upsert(
            thread_id,
            Some("Scout".to_string()),
            Some("worker".to_string()),
            /*is_closed*/ false,
        );

        let err = app
            .attach_live_thread_for_selection(&mut app_server, thread_id)
            .await
            .expect_err("ephemeral fallback should not attach as a blank live thread");

        assert_eq!(
            err.to_string(),
            format!("Agent thread {thread_id} is not yet available for replay or live attach.")
        );
        assert!(!app.thread_event_channels.contains_key(&thread_id));
        Ok(())
    }

    #[tokio::test]
    async fn should_attach_live_thread_for_selection_skips_closed_metadata_only_threads() {
        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        app.agent_navigation.upsert(
            thread_id,
            Some("Ghost".to_string()),
            Some("worker".to_string()),
            /*is_closed*/ true,
        );

        assert!(!app.should_attach_live_thread_for_selection(thread_id));

        app.agent_navigation.upsert(
            thread_id,
            Some("Ghost".to_string()),
            Some("worker".to_string()),
            /*is_closed*/ false,
        );
        assert!(app.should_attach_live_thread_for_selection(thread_id));

        app.thread_event_channels
            .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));
        assert!(!app.should_attach_live_thread_for_selection(thread_id));
    }

    #[tokio::test]
    async fn refresh_agent_picker_thread_liveness_prunes_closed_metadata_only_threads() -> Result<()>
    {
        let mut app = make_test_app().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let thread_id = ThreadId::new();
        app.agent_navigation.upsert(
            thread_id,
            Some("Ghost".to_string()),
            Some("worker".to_string()),
            /*is_closed*/ false,
        );

        let is_available = app
            .refresh_agent_picker_thread_liveness(&mut app_server, thread_id)
            .await;

        assert!(!is_available);
        assert_eq!(app.agent_navigation.get(&thread_id), None);
        assert!(!app.thread_event_channels.contains_key(&thread_id));
        Ok(())
    }

    #[tokio::test]
    async fn open_agent_picker_prompts_to_enable_multi_agent_when_disabled() -> Result<()> {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let _ = app.config.features.disable(Feature::Collab);

        app.open_agent_picker(&mut app_server).await;
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_matches!(
            app_event_rx.try_recv(),
            Ok(AppEvent::UpdateFeatureFlags { updates }) if updates == vec![(Feature::Collab, true)]
        );
        let cell = match app_event_rx.try_recv() {
            Ok(AppEvent::InsertHistoryCell(cell)) => cell,
            other => panic!("expected InsertHistoryCell event, got {other:?}"),
        };
        let rendered = cell
            .display_lines(/*width*/ 120)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Subagents will be enabled in the next session."));
        Ok(())
    }

    #[tokio::test]
    async fn update_memory_settings_persists_and_updates_widget_config() -> Result<()> {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

        app.update_memory_settings_with_app_server(
            &mut app_server,
            /*use_memories*/ false,
            /*generate_memories*/ false,
        )
        .await;

        assert!(!app.config.memories.use_memories);
        assert!(!app.config.memories.generate_memories);
        assert!(!app.chat_widget.config_ref().memories.use_memories);
        assert!(!app.chat_widget.config_ref().memories.generate_memories);

        let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
        let config_value = toml::from_str::<TomlValue>(&config)?;
        let memories = config_value
            .as_table()
            .and_then(|table| table.get("memories"))
            .and_then(TomlValue::as_table)
            .expect("memories table should exist");
        assert_eq!(
            memories.get("use_memories"),
            Some(&TomlValue::Boolean(false))
        );
        assert_eq!(
            memories.get("generate_memories"),
            Some(&TomlValue::Boolean(false))
        );
        assert!(
            !memories.contains_key("no_memories_if_mcp_or_web_search"),
            "the TUI menu should not write the MCP pollution setting"
        );
        app_server.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn update_memory_settings_updates_current_thread_memory_mode() -> Result<()> {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        // Seed the previous setting so this test exercises the thread-mode update path.
        app.config.memories.generate_memories = true;

        let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
        let started = app_server.start_thread(&app.config).await?;
        let thread_id = started.session.thread_id;
        app.active_thread_id = Some(thread_id);

        app.update_memory_settings_with_app_server(
            &mut app_server,
            /*use_memories*/ true,
            /*generate_memories*/ false,
        )
        .await;

        let state_db = codex_state::StateRuntime::init(
            codex_home.path().to_path_buf(),
            app.config.model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let memory_mode = state_db
            .get_thread_memory_mode(thread_id)
            .await
            .expect("thread memory mode should be readable");
        assert_eq!(memory_mode.as_deref(), Some("disabled"));

        app_server.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn reset_memories_clears_local_memory_directories() -> Result<()> {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();

        let memory_root = codex_home.path().join("memories");
        let extensions_root = codex_home.path().join("memories_extensions");
        std::fs::create_dir_all(memory_root.join("rollout_summaries"))?;
        std::fs::create_dir_all(&extensions_root)?;
        std::fs::write(memory_root.join("MEMORY.md"), "stale memory\n")?;
        std::fs::write(
            memory_root.join("rollout_summaries").join("stale.md"),
            "stale summary\n",
        )?;
        std::fs::write(extensions_root.join("stale.txt"), "stale extension\n")?;

        let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

        app.reset_memories_with_app_server(&mut app_server).await;

        assert_eq!(std::fs::read_dir(&memory_root)?.count(), 0);
        assert_eq!(std::fs::read_dir(&extensions_root)?.count(), 0);

        app_server.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn update_feature_flags_enabling_guardian_selects_guardian_approvals() -> Result<()> {
        let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        let guardian_approvals = guardian_approvals_mode();

        app.update_feature_flags(vec![(Feature::GuardianApproval, true)])
            .await;

        assert!(app.config.features.enabled(Feature::GuardianApproval));
        assert!(
            app.chat_widget
                .config_ref()
                .features
                .enabled(Feature::GuardianApproval)
        );
        assert_eq!(
            app.config.approvals_reviewer,
            guardian_approvals.approvals_reviewer
        );
        assert_eq!(
            app.config.permissions.approval_policy.value(),
            guardian_approvals.approval_policy
        );
        assert_eq!(
            app.chat_widget
                .config_ref()
                .permissions
                .approval_policy
                .value(),
            guardian_approvals.approval_policy
        );
        assert_eq!(
            app.chat_widget
                .config_ref()
                .permissions
                .sandbox_policy
                .get(),
            &guardian_approvals.sandbox_policy
        );
        assert_eq!(
            app.chat_widget.config_ref().approvals_reviewer,
            guardian_approvals.approvals_reviewer
        );
        assert_eq!(app.runtime_approval_policy_override, None);
        assert_eq!(app.runtime_sandbox_policy_override, None);
        assert_eq!(
            op_rx.try_recv(),
            Ok(Op::OverrideTurnContext {
                cwd: None,
                approval_policy: Some(guardian_approvals.approval_policy),
                approvals_reviewer: Some(guardian_approvals.approvals_reviewer),
                sandbox_policy: Some(guardian_approvals.sandbox_policy.clone()),
                windows_sandbox_level: None,
                model: None,
                effort: None,
                summary: None,
                service_tier: None,
                collaboration_mode: None,
                personality: None,
            })
        );
        let cell = match app_event_rx.try_recv() {
            Ok(AppEvent::InsertHistoryCell(cell)) => cell,
            other => panic!("expected InsertHistoryCell event, got {other:?}"),
        };
        let rendered = cell
            .display_lines(/*width*/ 120)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Permissions updated to Guardian Approvals"));

        let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
        assert!(config.contains("guardian_approval = true"));
        assert!(config.contains("approvals_reviewer = \"guardian_subagent\""));
        assert!(config.contains("approval_policy = \"on-request\""));
        assert!(config.contains("sandbox_mode = \"workspace-write\""));
        Ok(())
    }

    #[tokio::test]
    async fn update_feature_flags_disabling_guardian_clears_review_policy_and_restores_default()
    -> Result<()> {
        let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        let config_toml_path = codex_home.path().join("config.toml").abs();
        let config_toml = "approvals_reviewer = \"guardian_subagent\"\napproval_policy = \"on-request\"\nsandbox_mode = \"workspace-write\"\n\n[features]\nguardian_approval = true\n";
        std::fs::write(config_toml_path.as_path(), config_toml)?;
        let user_config = toml::from_str::<TomlValue>(config_toml)?;
        app.config.config_layer_stack = app
            .config
            .config_layer_stack
            .with_user_config(&config_toml_path, user_config);
        app.config
            .features
            .set_enabled(Feature::GuardianApproval, /*enabled*/ true)?;
        app.chat_widget
            .set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
        app.config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
        app.chat_widget
            .set_approvals_reviewer(ApprovalsReviewer::GuardianSubagent);
        app.config
            .permissions
            .approval_policy
            .set(AskForApproval::OnRequest)?;
        app.config
            .permissions
            .sandbox_policy
            .set(SandboxPolicy::new_workspace_write_policy())?;
        app.chat_widget
            .set_approval_policy(AskForApproval::OnRequest);
        app.chat_widget
            .set_sandbox_policy(SandboxPolicy::new_workspace_write_policy())?;

        app.update_feature_flags(vec![(Feature::GuardianApproval, false)])
            .await;

        assert!(!app.config.features.enabled(Feature::GuardianApproval));
        assert!(
            !app.chat_widget
                .config_ref()
                .features
                .enabled(Feature::GuardianApproval)
        );
        assert_eq!(app.config.approvals_reviewer, ApprovalsReviewer::User);
        assert_eq!(
            app.config.permissions.approval_policy.value(),
            AskForApproval::OnRequest
        );
        assert_eq!(
            app.chat_widget.config_ref().approvals_reviewer,
            ApprovalsReviewer::User
        );
        assert_eq!(app.runtime_approval_policy_override, None);
        assert_eq!(
            op_rx.try_recv(),
            Ok(Op::OverrideTurnContext {
                cwd: None,
                approval_policy: None,
                approvals_reviewer: Some(ApprovalsReviewer::User),
                sandbox_policy: None,
                windows_sandbox_level: None,
                model: None,
                effort: None,
                summary: None,
                service_tier: None,
                collaboration_mode: None,
                personality: None,
            })
        );
        let cell = match app_event_rx.try_recv() {
            Ok(AppEvent::InsertHistoryCell(cell)) => cell,
            other => panic!("expected InsertHistoryCell event, got {other:?}"),
        };
        let rendered = cell
            .display_lines(/*width*/ 120)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Permissions updated to Default"));

        let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
        assert!(!config.contains("guardian_approval = true"));
        assert!(!config.contains("approvals_reviewer ="));
        assert!(config.contains("approval_policy = \"on-request\""));
        assert!(config.contains("sandbox_mode = \"workspace-write\""));
        Ok(())
    }

    #[tokio::test]
    async fn update_feature_flags_enabling_guardian_overrides_explicit_manual_review_policy()
    -> Result<()> {
        let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        let guardian_approvals = guardian_approvals_mode();
        let config_toml_path = codex_home.path().join("config.toml").abs();
        let config_toml = "approvals_reviewer = \"user\"\n";
        std::fs::write(config_toml_path.as_path(), config_toml)?;
        let user_config = toml::from_str::<TomlValue>(config_toml)?;
        app.config.config_layer_stack = app
            .config
            .config_layer_stack
            .with_user_config(&config_toml_path, user_config);
        app.config.approvals_reviewer = ApprovalsReviewer::User;
        app.chat_widget
            .set_approvals_reviewer(ApprovalsReviewer::User);

        app.update_feature_flags(vec![(Feature::GuardianApproval, true)])
            .await;

        assert!(app.config.features.enabled(Feature::GuardianApproval));
        assert_eq!(
            app.config.approvals_reviewer,
            guardian_approvals.approvals_reviewer
        );
        assert_eq!(
            app.chat_widget.config_ref().approvals_reviewer,
            guardian_approvals.approvals_reviewer
        );
        assert_eq!(
            app.config.permissions.approval_policy.value(),
            guardian_approvals.approval_policy
        );
        assert_eq!(
            app.chat_widget
                .config_ref()
                .permissions
                .sandbox_policy
                .get(),
            &guardian_approvals.sandbox_policy
        );
        assert_eq!(
            op_rx.try_recv(),
            Ok(Op::OverrideTurnContext {
                cwd: None,
                approval_policy: Some(guardian_approvals.approval_policy),
                approvals_reviewer: Some(guardian_approvals.approvals_reviewer),
                sandbox_policy: Some(guardian_approvals.sandbox_policy.clone()),
                windows_sandbox_level: None,
                model: None,
                effort: None,
                summary: None,
                service_tier: None,
                collaboration_mode: None,
                personality: None,
            })
        );

        let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
        assert!(config.contains("approvals_reviewer = \"guardian_subagent\""));
        assert!(config.contains("guardian_approval = true"));
        assert!(config.contains("approval_policy = \"on-request\""));
        assert!(config.contains("sandbox_mode = \"workspace-write\""));
        Ok(())
    }

    #[tokio::test]
    async fn update_feature_flags_disabling_guardian_clears_manual_review_policy_without_history()
    -> Result<()> {
        let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        let config_toml_path = codex_home.path().join("config.toml").abs();
        let config_toml = "approvals_reviewer = \"user\"\napproval_policy = \"on-request\"\nsandbox_mode = \"workspace-write\"\n\n[features]\nguardian_approval = true\n";
        std::fs::write(config_toml_path.as_path(), config_toml)?;
        let user_config = toml::from_str::<TomlValue>(config_toml)?;
        app.config.config_layer_stack = app
            .config
            .config_layer_stack
            .with_user_config(&config_toml_path, user_config);
        app.config
            .features
            .set_enabled(Feature::GuardianApproval, /*enabled*/ true)?;
        app.chat_widget
            .set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
        app.config.approvals_reviewer = ApprovalsReviewer::User;
        app.chat_widget
            .set_approvals_reviewer(ApprovalsReviewer::User);

        app.update_feature_flags(vec![(Feature::GuardianApproval, false)])
            .await;

        assert!(!app.config.features.enabled(Feature::GuardianApproval));
        assert_eq!(app.config.approvals_reviewer, ApprovalsReviewer::User);
        assert_eq!(
            app.chat_widget.config_ref().approvals_reviewer,
            ApprovalsReviewer::User
        );
        assert_eq!(
            op_rx.try_recv(),
            Ok(Op::OverrideTurnContext {
                cwd: None,
                approval_policy: None,
                approvals_reviewer: Some(ApprovalsReviewer::User),
                sandbox_policy: None,
                windows_sandbox_level: None,
                model: None,
                effort: None,
                summary: None,
                service_tier: None,
                collaboration_mode: None,
                personality: None,
            })
        );
        assert!(
            app_event_rx.try_recv().is_err(),
            "manual review should not emit a permissions history update when the effective state stays default"
        );

        let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
        assert!(!config.contains("guardian_approval = true"));
        assert!(!config.contains("approvals_reviewer ="));
        Ok(())
    }

    #[tokio::test]
    async fn update_feature_flags_enabling_guardian_in_profile_sets_profile_auto_review_policy()
    -> Result<()> {
        let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        let guardian_approvals = guardian_approvals_mode();
        app.active_profile = Some("guardian".to_string());
        let config_toml_path = codex_home.path().join("config.toml").abs();
        let config_toml = "profile = \"guardian\"\napprovals_reviewer = \"user\"\n";
        std::fs::write(config_toml_path.as_path(), config_toml)?;
        let user_config = toml::from_str::<TomlValue>(config_toml)?;
        app.config.config_layer_stack = app
            .config
            .config_layer_stack
            .with_user_config(&config_toml_path, user_config);
        app.config.approvals_reviewer = ApprovalsReviewer::User;
        app.chat_widget
            .set_approvals_reviewer(ApprovalsReviewer::User);

        app.update_feature_flags(vec![(Feature::GuardianApproval, true)])
            .await;

        assert!(app.config.features.enabled(Feature::GuardianApproval));
        assert_eq!(
            app.config.approvals_reviewer,
            guardian_approvals.approvals_reviewer
        );
        assert_eq!(
            app.chat_widget.config_ref().approvals_reviewer,
            guardian_approvals.approvals_reviewer
        );
        assert_eq!(
            op_rx.try_recv(),
            Ok(Op::OverrideTurnContext {
                cwd: None,
                approval_policy: Some(guardian_approvals.approval_policy),
                approvals_reviewer: Some(guardian_approvals.approvals_reviewer),
                sandbox_policy: Some(guardian_approvals.sandbox_policy.clone()),
                windows_sandbox_level: None,
                model: None,
                effort: None,
                summary: None,
                service_tier: None,
                collaboration_mode: None,
                personality: None,
            })
        );

        let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
        let config_value = toml::from_str::<TomlValue>(&config)?;
        let profile_config = config_value
            .as_table()
            .and_then(|table| table.get("profiles"))
            .and_then(TomlValue::as_table)
            .and_then(|profiles| profiles.get("guardian"))
            .and_then(TomlValue::as_table)
            .expect("guardian profile should exist");
        assert_eq!(
            config_value
                .as_table()
                .and_then(|table| table.get("approvals_reviewer")),
            Some(&TomlValue::String("user".to_string()))
        );
        assert_eq!(
            profile_config.get("approvals_reviewer"),
            Some(&TomlValue::String("guardian_subagent".to_string()))
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_feature_flags_disabling_guardian_in_profile_allows_inherited_user_reviewer()
    -> Result<()> {
        let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.active_profile = Some("guardian".to_string());
        let config_toml_path = codex_home.path().join("config.toml").abs();
        let config_toml = r#"
profile = "guardian"
approvals_reviewer = "user"

[profiles.guardian]
approvals_reviewer = "guardian_subagent"

[profiles.guardian.features]
guardian_approval = true
"#;
        std::fs::write(config_toml_path.as_path(), config_toml)?;
        let user_config = toml::from_str::<TomlValue>(config_toml)?;
        app.config.config_layer_stack = app
            .config
            .config_layer_stack
            .with_user_config(&config_toml_path, user_config);
        app.config
            .features
            .set_enabled(Feature::GuardianApproval, /*enabled*/ true)?;
        app.chat_widget
            .set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
        app.config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
        app.chat_widget
            .set_approvals_reviewer(ApprovalsReviewer::GuardianSubagent);

        app.update_feature_flags(vec![(Feature::GuardianApproval, false)])
            .await;

        assert!(!app.config.features.enabled(Feature::GuardianApproval));
        assert!(
            !app.chat_widget
                .config_ref()
                .features
                .enabled(Feature::GuardianApproval)
        );
        assert_eq!(app.config.approvals_reviewer, ApprovalsReviewer::User);
        assert_eq!(
            app.chat_widget.config_ref().approvals_reviewer,
            ApprovalsReviewer::User
        );
        assert_eq!(
            op_rx.try_recv(),
            Ok(Op::OverrideTurnContext {
                cwd: None,
                approval_policy: None,
                approvals_reviewer: Some(ApprovalsReviewer::User),
                sandbox_policy: None,
                windows_sandbox_level: None,
                model: None,
                effort: None,
                summary: None,
                service_tier: None,
                collaboration_mode: None,
                personality: None,
            })
        );
        let cell = match app_event_rx.try_recv() {
            Ok(AppEvent::InsertHistoryCell(cell)) => cell,
            other => panic!("expected InsertHistoryCell event, got {other:?}"),
        };
        let rendered = cell
            .display_lines(/*width*/ 120)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Permissions updated to Default"));

        let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
        assert!(!config.contains("guardian_approval = true"));
        assert!(!config.contains("guardian_subagent"));
        assert_eq!(
            toml::from_str::<TomlValue>(&config)?
                .as_table()
                .and_then(|table| table.get("approvals_reviewer")),
            Some(&TomlValue::String("user".to_string()))
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_feature_flags_disabling_guardian_in_profile_keeps_inherited_non_user_reviewer_enabled()
    -> Result<()> {
        let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.active_profile = Some("guardian".to_string());
        let config_toml_path = codex_home.path().join("config.toml").abs();
        let config_toml = "profile = \"guardian\"\napprovals_reviewer = \"guardian_subagent\"\n\n[features]\nguardian_approval = true\n";
        std::fs::write(config_toml_path.as_path(), config_toml)?;
        let user_config = toml::from_str::<TomlValue>(config_toml)?;
        app.config.config_layer_stack = app
            .config
            .config_layer_stack
            .with_user_config(&config_toml_path, user_config);
        app.config
            .features
            .set_enabled(Feature::GuardianApproval, /*enabled*/ true)?;
        app.chat_widget
            .set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
        app.config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
        app.chat_widget
            .set_approvals_reviewer(ApprovalsReviewer::GuardianSubagent);

        app.update_feature_flags(vec![(Feature::GuardianApproval, false)])
            .await;

        assert!(app.config.features.enabled(Feature::GuardianApproval));
        assert!(
            app.chat_widget
                .config_ref()
                .features
                .enabled(Feature::GuardianApproval)
        );
        assert_eq!(
            app.config.approvals_reviewer,
            ApprovalsReviewer::GuardianSubagent
        );
        assert_eq!(
            app.chat_widget.config_ref().approvals_reviewer,
            ApprovalsReviewer::GuardianSubagent
        );
        assert!(
            op_rx.try_recv().is_err(),
            "disabling an inherited non-user reviewer should not patch the active session"
        );
        let app_events = std::iter::from_fn(|| app_event_rx.try_recv().ok()).collect::<Vec<_>>();
        assert!(
            !app_events.iter().any(|event| match event {
                AppEvent::InsertHistoryCell(cell) => cell
                    .display_lines(/*width*/ 120)
                    .iter()
                    .any(|line| line.to_string().contains("Permissions updated to")),
                _ => false,
            }),
            "blocking disable with inherited guardian review should not emit a permissions history update: {app_events:?}"
        );

        let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
        assert!(config.contains("guardian_approval = true"));
        assert_eq!(
            toml::from_str::<TomlValue>(&config)?
                .as_table()
                .and_then(|table| table.get("approvals_reviewer")),
            Some(&TomlValue::String("guardian_subagent".to_string()))
        );
        Ok(())
    }

    #[tokio::test]
    async fn open_agent_picker_allows_existing_agent_threads_when_feature_is_disabled() -> Result<()>
    {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let thread_id = ThreadId::new();
        app.thread_event_channels
            .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));

        app.open_agent_picker(&mut app_server).await;
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_matches!(
            app_event_rx.try_recv(),
            Ok(AppEvent::SelectAgentThread(selected_thread_id)) if selected_thread_id == thread_id
        );
        Ok(())
    }

    #[tokio::test]
    async fn refresh_pending_thread_approvals_only_lists_inactive_threads() {
        let mut app = make_test_app().await;
        let main_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000001").expect("valid thread");
        let agent_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000002").expect("valid thread");

        app.primary_thread_id = Some(main_thread_id);
        app.active_thread_id = Some(main_thread_id);
        app.thread_event_channels
            .insert(main_thread_id, ThreadEventChannel::new(/*capacity*/ 1));

        let agent_channel = ThreadEventChannel::new(/*capacity*/ 1);
        {
            let mut store = agent_channel.store.lock().await;
            store.push_request(exec_approval_request(
                agent_thread_id,
                "turn-1",
                "call-1",
                /*approval_id*/ None,
            ));
        }
        app.thread_event_channels
            .insert(agent_thread_id, agent_channel);
        app.agent_navigation.upsert(
            agent_thread_id,
            Some("Robie".to_string()),
            Some("explorer".to_string()),
            /*is_closed*/ false,
        );

        app.refresh_pending_thread_approvals().await;
        assert_eq!(
            app.chat_widget.pending_thread_approvals(),
            &["Robie [explorer]".to_string()]
        );

        app.active_thread_id = Some(agent_thread_id);
        app.refresh_pending_thread_approvals().await;
        assert!(app.chat_widget.pending_thread_approvals().is_empty());
    }

    #[tokio::test]
    async fn inactive_thread_approval_bubbles_into_active_view() -> Result<()> {
        let mut app = make_test_app().await;
        let main_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000011").expect("valid thread");
        let agent_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000022").expect("valid thread");

        app.primary_thread_id = Some(main_thread_id);
        app.active_thread_id = Some(main_thread_id);
        app.thread_event_channels
            .insert(main_thread_id, ThreadEventChannel::new(/*capacity*/ 1));
        app.thread_event_channels.insert(
            agent_thread_id,
            ThreadEventChannel::new_with_session(
                /*capacity*/ 1,
                ThreadSessionState {
                    approval_policy: AskForApproval::OnRequest,
                    sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
                    rollout_path: Some(test_path_buf("/tmp/agent-rollout.jsonl")),
                    ..test_thread_session(agent_thread_id, test_path_buf("/tmp/agent"))
                },
                Vec::new(),
            ),
        );
        app.agent_navigation.upsert(
            agent_thread_id,
            Some("Robie".to_string()),
            Some("explorer".to_string()),
            /*is_closed*/ false,
        );

        app.enqueue_thread_request(
            agent_thread_id,
            exec_approval_request(
                agent_thread_id,
                "turn-approval",
                "call-approval",
                /*approval_id*/ None,
            ),
        )
        .await?;

        assert_eq!(app.chat_widget.has_active_view(), true);
        assert_eq!(
            app.chat_widget.pending_thread_approvals(),
            &["Robie [explorer]".to_string()]
        );

        Ok(())
    }

    #[tokio::test]
    async fn inactive_thread_exec_approval_preserves_context() {
        let app = make_test_app().await;
        let thread_id = ThreadId::new();
        let mut request = exec_approval_request(
            thread_id,
            "turn-approval",
            "call-approval",
            /*approval_id*/ None,
        );
        let ServerRequest::CommandExecutionRequestApproval { params, .. } = &mut request else {
            panic!("expected exec approval request");
        };
        params.network_approval_context = Some(AppServerNetworkApprovalContext {
            host: "example.com".to_string(),
            protocol: AppServerNetworkApprovalProtocol::Socks5Tcp,
        });
        params.additional_permissions = Some(AdditionalPermissionProfile {
            network: Some(AdditionalNetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(AdditionalFileSystemPermissions {
                read: Some(vec![test_absolute_path("/tmp/read-only")]),
                write: Some(vec![test_absolute_path("/tmp/write")]),
            }),
        });
        params.proposed_network_policy_amendments = Some(vec![AppServerNetworkPolicyAmendment {
            host: "example.com".to_string(),
            action: AppServerNetworkPolicyRuleAction::Allow,
        }]);

        let Some(ThreadInteractiveRequest::Approval(ApprovalRequest::Exec {
            available_decisions,
            network_approval_context,
            additional_permissions,
            ..
        })) = app
            .interactive_request_for_thread_request(thread_id, &request)
            .await
        else {
            panic!("expected exec approval request");
        };

        assert_eq!(
            network_approval_context,
            Some(NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Socks5Tcp,
            })
        );
        assert_eq!(
            additional_permissions,
            Some(PermissionProfile {
                network: Some(NetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(FileSystemPermissions {
                    read: Some(vec![test_absolute_path("/tmp/read-only")]),
                    write: Some(vec![test_absolute_path("/tmp/write")]),
                }),
            })
        );
        assert_eq!(
            available_decisions,
            vec![
                codex_protocol::protocol::ReviewDecision::Approved,
                codex_protocol::protocol::ReviewDecision::ApprovedForSession,
                codex_protocol::protocol::ReviewDecision::NetworkPolicyAmendment {
                    network_policy_amendment: codex_protocol::approvals::NetworkPolicyAmendment {
                        host: "example.com".to_string(),
                        action: codex_protocol::approvals::NetworkPolicyRuleAction::Allow,
                    },
                },
                codex_protocol::protocol::ReviewDecision::Abort,
            ]
        );
    }

    #[tokio::test]
    async fn inactive_thread_exec_approval_splits_shell_wrapped_command() {
        let app = make_test_app().await;
        let thread_id = ThreadId::new();
        let script = r#"python3 -c 'print("Hello, world!")'"#;
        let mut request = exec_approval_request(
            thread_id,
            "turn-approval",
            "call-approval",
            /*approval_id*/ None,
        );
        let ServerRequest::CommandExecutionRequestApproval { params, .. } = &mut request else {
            panic!("expected exec approval request");
        };
        params.command = Some(
            shlex::try_join(["/bin/zsh", "-lc", script]).expect("round-trippable shell wrapper"),
        );

        let Some(ThreadInteractiveRequest::Approval(ApprovalRequest::Exec { command, .. })) = app
            .interactive_request_for_thread_request(thread_id, &request)
            .await
        else {
            panic!("expected exec approval request");
        };

        assert_eq!(
            command,
            vec![
                "/bin/zsh".to_string(),
                "-lc".to_string(),
                script.to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn inactive_thread_permissions_approval_preserves_file_system_permissions() {
        let app = make_test_app().await;
        let thread_id = ThreadId::new();
        let request = ServerRequest::PermissionsRequestApproval {
            request_id: AppServerRequestId::Integer(7),
            params: PermissionsRequestApprovalParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn-approval".to_string(),
                item_id: "call-approval".to_string(),
                reason: Some("Need access to .git".to_string()),
                permissions: codex_app_server_protocol::RequestPermissionProfile {
                    network: Some(AdditionalNetworkPermissions {
                        enabled: Some(true),
                    }),
                    file_system: Some(AdditionalFileSystemPermissions {
                        read: Some(vec![test_absolute_path("/tmp/read-only")]),
                        write: Some(vec![test_absolute_path("/tmp/write")]),
                    }),
                },
            },
        };

        let Some(ThreadInteractiveRequest::Approval(ApprovalRequest::Permissions {
            permissions,
            ..
        })) = app
            .interactive_request_for_thread_request(thread_id, &request)
            .await
        else {
            panic!("expected permissions approval request");
        };

        assert_eq!(
            permissions,
            RequestPermissionProfile {
                network: Some(NetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(FileSystemPermissions {
                    read: Some(vec![test_absolute_path("/tmp/read-only")]),
                    write: Some(vec![test_absolute_path("/tmp/write")]),
                }),
            }
        );
    }

    #[tokio::test]
    async fn inactive_thread_approval_badge_clears_after_turn_completion_notification() -> Result<()>
    {
        let mut app = make_test_app().await;
        let main_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000101").expect("valid thread");
        let agent_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000202").expect("valid thread");

        app.primary_thread_id = Some(main_thread_id);
        app.active_thread_id = Some(main_thread_id);
        app.thread_event_channels
            .insert(main_thread_id, ThreadEventChannel::new(/*capacity*/ 1));
        app.thread_event_channels.insert(
            agent_thread_id,
            ThreadEventChannel::new_with_session(
                /*capacity*/ 4,
                ThreadSessionState {
                    approval_policy: AskForApproval::OnRequest,
                    sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
                    rollout_path: Some(test_path_buf("/tmp/agent-rollout.jsonl")),
                    ..test_thread_session(agent_thread_id, test_path_buf("/tmp/agent"))
                },
                Vec::new(),
            ),
        );
        app.agent_navigation.upsert(
            agent_thread_id,
            Some("Robie".to_string()),
            Some("explorer".to_string()),
            /*is_closed*/ false,
        );

        app.enqueue_thread_request(
            agent_thread_id,
            exec_approval_request(
                agent_thread_id,
                "turn-approval",
                "call-approval",
                /*approval_id*/ None,
            ),
        )
        .await?;
        assert_eq!(
            app.chat_widget.pending_thread_approvals(),
            &["Robie [explorer]".to_string()]
        );

        app.enqueue_thread_notification(
            agent_thread_id,
            turn_completed_notification(agent_thread_id, "turn-approval", TurnStatus::Completed),
        )
        .await?;

        assert!(
            app.chat_widget.pending_thread_approvals().is_empty(),
            "turn completion should clear inactive-thread approval badge immediately"
        );

        Ok(())
    }

    #[tokio::test]
    async fn inactive_thread_started_notification_initializes_replay_session() -> Result<()> {
        let mut app = make_test_app().await;
        let temp_dir = tempdir()?;
        let main_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000101").expect("valid thread");
        let agent_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000202").expect("valid thread");
        let primary_session = ThreadSessionState {
            approval_policy: AskForApproval::OnRequest,
            sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
            ..test_thread_session(main_thread_id, test_path_buf("/tmp/main"))
        };

        app.primary_thread_id = Some(main_thread_id);
        app.active_thread_id = Some(main_thread_id);
        app.primary_session_configured = Some(primary_session.clone());
        app.thread_event_channels.insert(
            main_thread_id,
            ThreadEventChannel::new_with_session(
                /*capacity*/ 4,
                primary_session.clone(),
                Vec::new(),
            ),
        );

        let rollout_path = temp_dir.path().join("agent-rollout.jsonl");
        let turn_context = TurnContextItem {
            turn_id: None,
            trace_id: None,
            cwd: test_path_buf("/tmp/agent"),
            current_date: None,
            timezone: None,
            approval_policy: primary_session.approval_policy,
            sandbox_policy: primary_session.sandbox_policy.clone(),
            network: None,
            model: "gpt-agent".to_string(),
            personality: None,
            collaboration_mode: None,
            realtime_active: Some(false),
            effort: primary_session.reasoning_effort,
            summary: app.config.model_reasoning_summary.unwrap_or_default(),
            user_instructions: None,
            developer_instructions: None,
            final_output_json_schema: None,
            truncation_policy: None,
        };
        let rollout = RolloutLine {
            timestamp: "t0".to_string(),
            item: RolloutItem::TurnContext(turn_context),
        };
        std::fs::write(
            &rollout_path,
            format!("{}\n", serde_json::to_string(&rollout)?),
        )?;
        app.enqueue_thread_notification(
            agent_thread_id,
            ServerNotification::ThreadStarted(ThreadStartedNotification {
                thread: Thread {
                    id: agent_thread_id.to_string(),
                    forked_from_id: None,
                    preview: "agent thread".to_string(),
                    ephemeral: false,
                    model_provider: "agent-provider".to_string(),
                    created_at: 1,
                    updated_at: 2,
                    status: codex_app_server_protocol::ThreadStatus::Idle,
                    path: Some(rollout_path.clone()),
                    cwd: test_path_buf("/tmp/agent").abs(),
                    cli_version: "0.0.0".to_string(),
                    source: codex_app_server_protocol::SessionSource::Unknown,
                    agent_nickname: Some("Robie".to_string()),
                    agent_role: Some("explorer".to_string()),
                    git_info: None,
                    name: Some("agent thread".to_string()),
                    turns: Vec::new(),
                },
            }),
        )
        .await?;

        let store = app
            .thread_event_channels
            .get(&agent_thread_id)
            .expect("agent thread channel")
            .store
            .lock()
            .await;
        let session = store.session.clone().expect("inferred session");
        drop(store);

        assert_eq!(session.thread_id, agent_thread_id);
        assert_eq!(session.thread_name, Some("agent thread".to_string()));
        assert_eq!(session.model, "gpt-agent");
        assert_eq!(session.model_provider_id, "agent-provider");
        assert_eq!(session.approval_policy, primary_session.approval_policy);
        assert_eq!(session.cwd.as_path(), test_path_buf("/tmp/agent").as_path());
        assert_eq!(session.rollout_path, Some(rollout_path));
        assert_eq!(
            app.agent_navigation.get(&agent_thread_id),
            Some(&AgentPickerThreadEntry {
                agent_nickname: Some("Robie".to_string()),
                agent_role: Some("explorer".to_string()),
                is_closed: false,
            })
        );

        Ok(())
    }

    #[tokio::test]
    async fn inactive_thread_started_notification_preserves_primary_model_when_path_missing()
    -> Result<()> {
        let mut app = make_test_app().await;
        let main_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000301").expect("valid thread");
        let agent_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000302").expect("valid thread");
        let primary_session = ThreadSessionState {
            approval_policy: AskForApproval::OnRequest,
            sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
            ..test_thread_session(main_thread_id, test_path_buf("/tmp/main"))
        };

        app.primary_thread_id = Some(main_thread_id);
        app.active_thread_id = Some(main_thread_id);
        app.primary_session_configured = Some(primary_session.clone());
        app.thread_event_channels.insert(
            main_thread_id,
            ThreadEventChannel::new_with_session(
                /*capacity*/ 4,
                primary_session.clone(),
                Vec::new(),
            ),
        );

        app.enqueue_thread_notification(
            agent_thread_id,
            ServerNotification::ThreadStarted(ThreadStartedNotification {
                thread: Thread {
                    id: agent_thread_id.to_string(),
                    forked_from_id: None,
                    preview: "agent thread".to_string(),
                    ephemeral: false,
                    model_provider: "agent-provider".to_string(),
                    created_at: 1,
                    updated_at: 2,
                    status: codex_app_server_protocol::ThreadStatus::Idle,
                    path: None,
                    cwd: test_path_buf("/tmp/agent").abs(),
                    cli_version: "0.0.0".to_string(),
                    source: codex_app_server_protocol::SessionSource::Unknown,
                    agent_nickname: Some("Robie".to_string()),
                    agent_role: Some("explorer".to_string()),
                    git_info: None,
                    name: Some("agent thread".to_string()),
                    turns: Vec::new(),
                },
            }),
        )
        .await?;

        let store = app
            .thread_event_channels
            .get(&agent_thread_id)
            .expect("agent thread channel")
            .store
            .lock()
            .await;
        let session = store.session.clone().expect("inferred session");

        assert_eq!(session.model, primary_session.model);

        Ok(())
    }

    #[test]
    fn agent_picker_item_name_snapshot() {
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000123").expect("valid thread id");
        let snapshot = [
            format!(
                "{} | {}",
                format_agent_picker_item_name(
                    Some("Robie"),
                    Some("explorer"),
                    /*is_primary*/ true
                ),
                thread_id
            ),
            format!(
                "{} | {}",
                format_agent_picker_item_name(
                    Some("Robie"),
                    Some("explorer"),
                    /*is_primary*/ false
                ),
                thread_id
            ),
            format!(
                "{} | {}",
                format_agent_picker_item_name(
                    Some("Robie"),
                    /*agent_role*/ None,
                    /*is_primary*/ false
                ),
                thread_id
            ),
            format!(
                "{} | {}",
                format_agent_picker_item_name(
                    /*agent_nickname*/ None,
                    Some("explorer"),
                    /*is_primary*/ false
                ),
                thread_id
            ),
            format!(
                "{} | {}",
                format_agent_picker_item_name(
                    /*agent_nickname*/ None, /*agent_role*/ None,
                    /*is_primary*/ false
                ),
                thread_id
            ),
        ]
        .join("\n");
        assert_snapshot!("agent_picker_item_name", snapshot);
    }

    #[tokio::test]
    async fn active_non_primary_shutdown_target_returns_none_for_non_shutdown_event() -> Result<()>
    {
        let mut app = make_test_app().await;
        app.active_thread_id = Some(ThreadId::new());
        app.primary_thread_id = Some(ThreadId::new());

        assert_eq!(
            app.active_non_primary_shutdown_target(&ServerNotification::SkillsChanged(
                codex_app_server_protocol::SkillsChangedNotification {},
            )),
            None
        );
        Ok(())
    }

    #[tokio::test]
    async fn active_non_primary_shutdown_target_returns_none_for_primary_thread_shutdown()
    -> Result<()> {
        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        app.active_thread_id = Some(thread_id);
        app.primary_thread_id = Some(thread_id);

        assert_eq!(
            app.active_non_primary_shutdown_target(&thread_closed_notification(thread_id)),
            None
        );
        Ok(())
    }

    #[tokio::test]
    async fn active_non_primary_shutdown_target_returns_ids_for_non_primary_shutdown() -> Result<()>
    {
        let mut app = make_test_app().await;
        let active_thread_id = ThreadId::new();
        let primary_thread_id = ThreadId::new();
        app.active_thread_id = Some(active_thread_id);
        app.primary_thread_id = Some(primary_thread_id);

        assert_eq!(
            app.active_non_primary_shutdown_target(&thread_closed_notification(active_thread_id)),
            Some((active_thread_id, primary_thread_id))
        );
        Ok(())
    }

    #[tokio::test]
    async fn active_non_primary_shutdown_target_returns_none_when_shutdown_exit_is_pending()
    -> Result<()> {
        let mut app = make_test_app().await;
        let active_thread_id = ThreadId::new();
        let primary_thread_id = ThreadId::new();
        app.active_thread_id = Some(active_thread_id);
        app.primary_thread_id = Some(primary_thread_id);
        app.pending_shutdown_exit_thread_id = Some(active_thread_id);

        assert_eq!(
            app.active_non_primary_shutdown_target(&thread_closed_notification(active_thread_id)),
            None
        );
        Ok(())
    }

    #[tokio::test]
    async fn active_non_primary_shutdown_target_still_switches_for_other_pending_exit_thread()
    -> Result<()> {
        let mut app = make_test_app().await;
        let active_thread_id = ThreadId::new();
        let primary_thread_id = ThreadId::new();
        app.active_thread_id = Some(active_thread_id);
        app.primary_thread_id = Some(primary_thread_id);
        app.pending_shutdown_exit_thread_id = Some(ThreadId::new());

        assert_eq!(
            app.active_non_primary_shutdown_target(&thread_closed_notification(active_thread_id)),
            Some((active_thread_id, primary_thread_id))
        );
        Ok(())
    }

    async fn render_clear_ui_header_after_long_transcript_for_snapshot() -> String {
        let mut app = make_test_app().await;
        app.config.cwd = test_path_buf("/tmp/project").abs();
        app.chat_widget.set_model("gpt-test");
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::High));
        let story_part_one = "In the cliffside town of Bracken Ferry, the lighthouse had been dark for \
            nineteen years, and the children were told it was because the sea no longer wanted a \
            guide. Mara, who repaired clocks for a living, found that hard to believe. Every dawn she \
            heard the gulls circling the empty tower, and every dusk she watched ships hesitate at the \
            mouth of the bay as if listening for a signal that never came. When an old brass key fell \
            out of a cracked parcel in her workshop, tagged only with the words 'for the lamp room,' \
            she decided to climb the hill and see what the town had forgotten.";
        let story_part_two = "Inside the lighthouse she found gears wrapped in oilcloth, logbooks filled \
            with weather notes, and a lens shrouded beneath salt-stiff canvas. The mechanism was not \
            broken, only unfinished. Someone had removed the governor spring and hidden it in a false \
            drawer, along with a letter from the last keeper admitting he had darkened the light on \
            purpose after smugglers threatened his family. Mara spent the night rebuilding the clockwork \
            from spare watch parts, her fingers blackened with soot and grease, while a storm gathered \
            over the water and the harbor bells began to ring.";
        let story_part_three = "At midnight the first squall hit, and the fishing boats returned early, \
            blind in sheets of rain. Mara wound the mechanism, set the teeth by hand, and watched the \
            great lens begin to turn in slow, certain arcs. The beam swept across the bay, caught the \
            whitecaps, and reached the boats just as they were drifting toward the rocks below the \
            eastern cliffs. In the morning the town square was crowded with wet sailors, angry elders, \
            and wide-eyed children, but when the oldest captain placed the keeper's log on the fountain \
            and thanked Mara for relighting the coast, nobody argued. By sunset, Bracken Ferry had a \
            lighthouse again, and Mara had more clocks to mend than ever because everyone wanted \
            something in town to keep better time.";

        let user_cell = |text: &str| -> Arc<dyn HistoryCell> {
            Arc::new(UserHistoryCell {
                message: text.to_string(),
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
                remote_image_urls: Vec::new(),
            }) as Arc<dyn HistoryCell>
        };
        let agent_cell = |text: &str| -> Arc<dyn HistoryCell> {
            Arc::new(AgentMessageCell::new(
                vec![Line::from(text.to_string())],
                /*is_first_line*/ true,
            )) as Arc<dyn HistoryCell>
        };
        let make_header = |is_first| -> Arc<dyn HistoryCell> {
            let event = SessionConfiguredEvent {
                session_id: ThreadId::new(),
                forked_from_id: None,
                thread_name: None,
                model: "gpt-test".to_string(),
                model_provider_id: "test-provider".to_string(),
                service_tier: None,
                approval_policy: AskForApproval::Never,
                approvals_reviewer: ApprovalsReviewer::User,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                cwd: test_path_buf("/tmp/project").abs(),
                reasoning_effort: Some(ReasoningEffortConfig::High),
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                network_proxy: None,
                rollout_path: Some(PathBuf::new()),
            };
            Arc::new(new_session_info(
                app.chat_widget.config_ref(),
                app.chat_widget.current_model(),
                event,
                is_first,
                /*tooltip_override*/ None,
                /*auth_plan*/ None,
                /*show_fast_status*/ false,
            )) as Arc<dyn HistoryCell>
        };

        app.transcript_cells = vec![
            make_header(true),
            Arc::new(crate::history_cell::new_info_event(
                "startup tip that used to replay".to_string(),
                /*hint*/ None,
            )) as Arc<dyn HistoryCell>,
            user_cell("Tell me a long story about a town with a dark lighthouse."),
            agent_cell(story_part_one),
            user_cell("Continue the story and reveal why the light went out."),
            agent_cell(story_part_two),
            user_cell("Finish the story with a storm and a resolution."),
            agent_cell(story_part_three),
        ];
        app.has_emitted_history_lines = true;

        let rendered = app
            .clear_ui_header_lines_with_version(/*width*/ 80, "<VERSION>")
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            !rendered.contains("startup tip that used to replay"),
            "clear header should not replay startup notices"
        );
        assert!(
            !rendered.contains("Bracken Ferry"),
            "clear header should not replay prior conversation turns"
        );
        rendered
    }

    #[tokio::test]
    #[cfg_attr(
        target_os = "windows",
        ignore = "snapshot path rendering differs on Windows"
    )]
    async fn clear_ui_after_long_transcript_snapshots_fresh_header_only() {
        let rendered = render_clear_ui_header_after_long_transcript_for_snapshot().await;
        assert_snapshot!("clear_ui_after_long_transcript_fresh_header_only", rendered);
    }

    #[tokio::test]
    #[cfg_attr(
        target_os = "windows",
        ignore = "snapshot path rendering differs on Windows"
    )]
    async fn ctrl_l_clear_ui_after_long_transcript_reuses_clear_header_snapshot() {
        let rendered = render_clear_ui_header_after_long_transcript_for_snapshot().await;
        assert_snapshot!("clear_ui_after_long_transcript_fresh_header_only", rendered);
    }

    #[tokio::test]
    #[cfg_attr(
        target_os = "windows",
        ignore = "snapshot path rendering differs on Windows"
    )]
    async fn clear_ui_header_shows_fast_status_for_fast_capable_models() {
        let mut app = make_test_app().await;
        app.config.cwd = test_path_buf("/tmp/project").abs();
        app.chat_widget.set_model("gpt-5.4");
        set_fast_mode_test_catalog(&mut app.chat_widget);
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::XHigh));
        app.chat_widget
            .set_service_tier(Some(codex_protocol::config_types::ServiceTier::Fast));
        set_chatgpt_auth(&mut app.chat_widget);
        set_fast_mode_test_catalog(&mut app.chat_widget);

        let rendered = app
            .clear_ui_header_lines_with_version(/*width*/ 80, "<VERSION>")
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert_snapshot!("clear_ui_header_fast_status_fast_capable_models", rendered);
    }

    async fn make_test_app() -> App {
        let (chat_widget, app_event_tx, _rx, _op_rx) = make_chatwidget_manual_with_sender().await;
        let config = chat_widget.config_ref().clone();
        let file_search = FileSearchManager::new(config.cwd.to_path_buf(), app_event_tx.clone());
        let model = crate::legacy_core::test_support::get_model_offline(config.model.as_deref());
        let session_telemetry = test_session_telemetry(&config, model.as_str());

        App {
            model_catalog: chat_widget.model_catalog(),
            session_telemetry,
            app_event_tx,
            chat_widget,
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
            status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
            terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
            backtrack: BacktrackState::default(),
            backtrack_render_pending: false,
            feedback: codex_feedback::CodexFeedback::new(),
            feedback_audience: FeedbackAudience::External,
            environment_manager: Arc::new(EnvironmentManager::new(/*exec_server_url*/ None)),
            remote_app_server_url: None,
            remote_app_server_auth_token: None,
            pending_update_action: None,
            pending_shutdown_exit_thread_id: None,
            windows_sandbox: WindowsSandboxState::default(),
            thread_event_channels: HashMap::new(),
            thread_event_listener_tasks: HashMap::new(),
            agent_navigation: AgentNavigationState::default(),
            active_thread_id: None,
            active_thread_rx: None,
            primary_thread_id: None,
            last_subagent_backfill_attempt: None,
            primary_session_configured: None,
            pending_primary_events: VecDeque::new(),
            pending_app_server_requests: PendingAppServerRequests::default(),
        }
    }

    async fn make_test_app_with_channels() -> (
        App,
        tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
        tokio::sync::mpsc::UnboundedReceiver<Op>,
    ) {
        let (chat_widget, app_event_tx, rx, op_rx) = make_chatwidget_manual_with_sender().await;
        let config = chat_widget.config_ref().clone();
        let file_search = FileSearchManager::new(config.cwd.to_path_buf(), app_event_tx.clone());
        let model = crate::legacy_core::test_support::get_model_offline(config.model.as_deref());
        let session_telemetry = test_session_telemetry(&config, model.as_str());

        (
            App {
                model_catalog: chat_widget.model_catalog(),
                session_telemetry,
                app_event_tx,
                chat_widget,
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
                status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
                terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
                backtrack: BacktrackState::default(),
                backtrack_render_pending: false,
                feedback: codex_feedback::CodexFeedback::new(),
                feedback_audience: FeedbackAudience::External,
                environment_manager: Arc::new(EnvironmentManager::new(
                    /*exec_server_url*/ None,
                )),
                remote_app_server_url: None,
                remote_app_server_auth_token: None,
                pending_update_action: None,
                pending_shutdown_exit_thread_id: None,
                windows_sandbox: WindowsSandboxState::default(),
                thread_event_channels: HashMap::new(),
                thread_event_listener_tasks: HashMap::new(),
                agent_navigation: AgentNavigationState::default(),
                active_thread_id: None,
                active_thread_rx: None,
                primary_thread_id: None,
                last_subagent_backfill_attempt: None,
                primary_session_configured: None,
                pending_primary_events: VecDeque::new(),
                pending_app_server_requests: PendingAppServerRequests::default(),
            },
            rx,
            op_rx,
        )
    }

    fn test_thread_session(thread_id: ThreadId, cwd: PathBuf) -> ThreadSessionState {
        ThreadSessionState {
            thread_id,
            forked_from_id: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            cwd: cwd.abs(),
            instruction_source_paths: Vec::new(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        }
    }

    fn test_turn(turn_id: &str, status: TurnStatus, items: Vec<ThreadItem>) -> Turn {
        Turn {
            id: turn_id.to_string(),
            items,
            status,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }
    }

    fn turn_started_notification(thread_id: ThreadId, turn_id: &str) -> ServerNotification {
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: thread_id.to_string(),
            turn: Turn {
                started_at: Some(0),
                ..test_turn(turn_id, TurnStatus::InProgress, Vec::new())
            },
        })
    }

    fn turn_completed_notification(
        thread_id: ThreadId,
        turn_id: &str,
        status: TurnStatus,
    ) -> ServerNotification {
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: Turn {
                completed_at: Some(0),
                duration_ms: Some(1),
                ..test_turn(turn_id, status, Vec::new())
            },
        })
    }

    fn thread_closed_notification(thread_id: ThreadId) -> ServerNotification {
        ServerNotification::ThreadClosed(ThreadClosedNotification {
            thread_id: thread_id.to_string(),
        })
    }

    fn token_usage_notification(
        thread_id: ThreadId,
        turn_id: &str,
        model_context_window: Option<i64>,
    ) -> ServerNotification {
        ServerNotification::ThreadTokenUsageUpdated(ThreadTokenUsageUpdatedNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            token_usage: ThreadTokenUsage {
                total: TokenUsageBreakdown {
                    total_tokens: 10,
                    input_tokens: 4,
                    cached_input_tokens: 1,
                    output_tokens: 5,
                    reasoning_output_tokens: 0,
                },
                last: TokenUsageBreakdown {
                    total_tokens: 10,
                    input_tokens: 4,
                    cached_input_tokens: 1,
                    output_tokens: 5,
                    reasoning_output_tokens: 0,
                },
                model_context_window,
            },
        })
    }

    fn hook_started_notification(thread_id: ThreadId, turn_id: &str) -> ServerNotification {
        ServerNotification::HookStarted(HookStartedNotification {
            thread_id: thread_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            run: AppServerHookRunSummary {
                id: "user-prompt-submit:0:/tmp/hooks.json".to_string(),
                event_name: AppServerHookEventName::UserPromptSubmit,
                handler_type: AppServerHookHandlerType::Command,
                execution_mode: AppServerHookExecutionMode::Sync,
                scope: AppServerHookScope::Turn,
                source_path: test_path_buf("/tmp/hooks.json").abs(),
                display_order: 0,
                status: AppServerHookRunStatus::Running,
                status_message: Some("checking go-workflow input policy".to_string()),
                started_at: 1,
                completed_at: None,
                duration_ms: None,
                entries: Vec::new(),
            },
        })
    }

    fn hook_completed_notification(thread_id: ThreadId, turn_id: &str) -> ServerNotification {
        ServerNotification::HookCompleted(HookCompletedNotification {
            thread_id: thread_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            run: AppServerHookRunSummary {
                id: "user-prompt-submit:0:/tmp/hooks.json".to_string(),
                event_name: AppServerHookEventName::UserPromptSubmit,
                handler_type: AppServerHookHandlerType::Command,
                execution_mode: AppServerHookExecutionMode::Sync,
                scope: AppServerHookScope::Turn,
                source_path: test_path_buf("/tmp/hooks.json").abs(),
                display_order: 0,
                status: AppServerHookRunStatus::Stopped,
                status_message: Some("checking go-workflow input policy".to_string()),
                started_at: 1,
                completed_at: Some(11),
                duration_ms: Some(10),
                entries: vec![
                    AppServerHookOutputEntry {
                        kind: AppServerHookOutputEntryKind::Warning,
                        text: "go-workflow must start from PlanMode".to_string(),
                    },
                    AppServerHookOutputEntry {
                        kind: AppServerHookOutputEntryKind::Stop,
                        text: "prompt blocked".to_string(),
                    },
                ],
            },
        })
    }

    fn agent_message_delta_notification(
        thread_id: ThreadId,
        turn_id: &str,
        item_id: &str,
        delta: &str,
    ) -> ServerNotification {
        ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            item_id: item_id.to_string(),
            delta: delta.to_string(),
        })
    }

    fn exec_approval_request(
        thread_id: ThreadId,
        turn_id: &str,
        item_id: &str,
        approval_id: Option<&str>,
    ) -> ServerRequest {
        ServerRequest::CommandExecutionRequestApproval {
            request_id: AppServerRequestId::Integer(1),
            params: CommandExecutionRequestApprovalParams {
                thread_id: thread_id.to_string(),
                turn_id: turn_id.to_string(),
                item_id: item_id.to_string(),
                approval_id: approval_id.map(str::to_string),
                reason: Some("needs approval".to_string()),
                network_approval_context: None,
                command: Some("echo hello".to_string()),
                cwd: Some(test_path_buf("/tmp/project").abs()),
                command_actions: None,
                additional_permissions: None,
                proposed_execpolicy_amendment: None,
                proposed_network_policy_amendments: None,
                available_decisions: None,
            },
        }
    }

    #[test]
    fn thread_event_store_tracks_active_turn_lifecycle() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        assert_eq!(store.active_turn_id(), None);

        let thread_id = ThreadId::new();
        store.push_notification(turn_started_notification(thread_id, "turn-1"));
        assert_eq!(store.active_turn_id(), Some("turn-1"));

        store.push_notification(turn_completed_notification(
            thread_id,
            "turn-2",
            TurnStatus::Completed,
        ));
        assert_eq!(store.active_turn_id(), Some("turn-1"));

        store.push_notification(turn_completed_notification(
            thread_id,
            "turn-1",
            TurnStatus::Interrupted,
        ));
        assert_eq!(store.active_turn_id(), None);
    }

    #[test]
    fn thread_event_store_restores_active_turn_from_snapshot_turns() {
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        let turns = vec![
            test_turn("turn-1", TurnStatus::Completed, Vec::new()),
            test_turn("turn-2", TurnStatus::InProgress, Vec::new()),
        ];

        let store =
            ThreadEventStore::new_with_session(/*capacity*/ 8, session.clone(), turns.clone());
        assert_eq!(store.active_turn_id(), Some("turn-2"));

        let mut refreshed_store = ThreadEventStore::new(/*capacity*/ 8);
        refreshed_store.set_session(session, turns);
        assert_eq!(refreshed_store.active_turn_id(), Some("turn-2"));
    }

    #[test]
    fn thread_event_store_clear_active_turn_id_resets_cached_turn() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        let thread_id = ThreadId::new();
        store.push_notification(turn_started_notification(thread_id, "turn-1"));

        store.clear_active_turn_id();

        assert_eq!(store.active_turn_id(), None);
    }

    #[test]
    fn thread_event_store_rebase_preserves_resolved_request_state() {
        let thread_id = ThreadId::new();
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(exec_approval_request(
            thread_id,
            "turn-approval",
            "call-approval",
            /*approval_id*/ None,
        ));
        store.push_notification(ServerNotification::ServerRequestResolved(
            codex_app_server_protocol::ServerRequestResolvedNotification {
                request_id: AppServerRequestId::Integer(1),
                thread_id: thread_id.to_string(),
            },
        ));

        store.rebase_buffer_after_session_refresh();

        let snapshot = store.snapshot();
        assert!(snapshot.events.is_empty());
        assert_eq!(store.has_pending_thread_approvals(), false);
    }

    #[test]
    fn thread_event_store_rebase_preserves_hook_notifications() {
        let thread_id = ThreadId::new();
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_notification(hook_started_notification(thread_id, "turn-hook"));
        store.push_notification(hook_completed_notification(thread_id, "turn-hook"));

        store.rebase_buffer_after_session_refresh();

        let snapshot = store.snapshot();
        let hook_notifications = snapshot
            .events
            .into_iter()
            .map(|event| match event {
                ThreadBufferedEvent::Notification(notification) => {
                    serde_json::to_value(notification).expect("hook notification should serialize")
                }
                other => panic!("expected buffered hook notification, saw: {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            hook_notifications,
            vec![
                serde_json::to_value(hook_started_notification(thread_id, "turn-hook"))
                    .expect("hook notification should serialize"),
                serde_json::to_value(hook_completed_notification(thread_id, "turn-hook"))
                    .expect("hook notification should serialize"),
            ]
        );
    }

    #[test]
    fn build_feedback_upload_params_includes_thread_id_and_rollout_path() {
        let thread_id = ThreadId::new();
        let rollout_path = PathBuf::from("/tmp/rollout.jsonl");

        let params = build_feedback_upload_params(
            Some(thread_id),
            Some(rollout_path.clone()),
            FeedbackCategory::SafetyCheck,
            Some("needs follow-up".to_string()),
            Some("turn-123".to_string()),
            /*include_logs*/ true,
        );

        assert_eq!(params.classification, "safety_check");
        assert_eq!(params.reason, Some("needs follow-up".to_string()));
        assert_eq!(params.thread_id, Some(thread_id.to_string()));
        assert_eq!(
            params
                .tags
                .as_ref()
                .and_then(|tags| tags.get("turn_id"))
                .map(String::as_str),
            Some("turn-123")
        );
        assert_eq!(params.include_logs, true);
        assert_eq!(params.extra_log_files, Some(vec![rollout_path]));
    }

    #[test]
    fn build_feedback_upload_params_omits_rollout_path_without_logs() {
        let params = build_feedback_upload_params(
            /*origin_thread_id*/ None,
            Some(PathBuf::from("/tmp/rollout.jsonl")),
            FeedbackCategory::GoodResult,
            /*reason*/ None,
            /*turn_id*/ None,
            /*include_logs*/ false,
        );

        assert_eq!(params.classification, "good_result");
        assert_eq!(params.reason, None);
        assert_eq!(params.thread_id, None);
        assert_eq!(params.tags, None);
        assert_eq!(params.include_logs, false);
        assert_eq!(params.extra_log_files, None);
    }

    #[tokio::test]
    async fn feedback_submission_without_thread_emits_error_history_cell() {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;

        app.handle_feedback_submitted(
            /*origin_thread_id*/ None,
            FeedbackCategory::Bug,
            /*include_logs*/ true,
            Err("boom".to_string()),
        )
        .await;

        let cell = match app_event_rx.try_recv() {
            Ok(AppEvent::InsertHistoryCell(cell)) => cell,
            other => panic!("expected feedback error history cell, saw {other:?}"),
        };
        assert_eq!(
            lines_to_single_string(&cell.display_lines(/*width*/ 120)),
            "■ Failed to upload feedback: boom"
        );
    }

    #[tokio::test]
    async fn feedback_submission_for_inactive_thread_replays_into_origin_thread() {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let origin_thread_id = ThreadId::new();
        let active_thread_id = ThreadId::new();
        let origin_session = test_thread_session(origin_thread_id, test_path_buf("/tmp/origin"));
        let active_session = test_thread_session(active_thread_id, test_path_buf("/tmp/active"));
        app.thread_event_channels.insert(
            origin_thread_id,
            ThreadEventChannel::new_with_session(
                THREAD_EVENT_CHANNEL_CAPACITY,
                origin_session.clone(),
                Vec::new(),
            ),
        );
        app.thread_event_channels.insert(
            active_thread_id,
            ThreadEventChannel::new_with_session(
                THREAD_EVENT_CHANNEL_CAPACITY,
                active_session.clone(),
                Vec::new(),
            ),
        );
        app.activate_thread_channel(active_thread_id).await;
        app.chat_widget.handle_thread_session(active_session);
        while app_event_rx.try_recv().is_ok() {}

        app.handle_feedback_submitted(
            Some(origin_thread_id),
            FeedbackCategory::Bug,
            /*include_logs*/ true,
            Ok("uploaded-thread".to_string()),
        )
        .await;

        assert_matches!(
            app_event_rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        );

        let snapshot = {
            let channel = app
                .thread_event_channels
                .get(&origin_thread_id)
                .expect("origin thread channel should exist");
            let store = channel.store.lock().await;
            assert!(matches!(
                store.buffer.back(),
                Some(ThreadBufferedEvent::FeedbackSubmission(_))
            ));
            store.snapshot()
        };

        app.replay_thread_snapshot(snapshot, /*resume_restored_queue*/ false);

        let mut rendered_cells = Vec::new();
        while let Ok(event) = app_event_rx.try_recv() {
            if let AppEvent::InsertHistoryCell(cell) = event {
                rendered_cells.push(lines_to_single_string(&cell.display_lines(/*width*/ 120)));
            }
        }
        assert!(rendered_cells.iter().any(|cell| {
            cell.contains("• Feedback uploaded. Please open an issue using the following URL:")
                && cell.contains("uploaded-thread")
        }));
    }

    fn next_user_turn_op(op_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Op>) -> Op {
        let mut seen = Vec::new();
        while let Ok(op) = op_rx.try_recv() {
            if matches!(op, Op::UserTurn { .. }) {
                return op;
            }
            seen.push(format!("{op:?}"));
        }
        panic!("expected UserTurn op, saw: {seen:?}");
    }

    fn lines_to_single_string(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn test_session_telemetry(config: &Config, model: &str) -> SessionTelemetry {
        let model_info =
            crate::legacy_core::test_support::construct_model_info_offline(model, config);
        SessionTelemetry::new(
            ThreadId::new(),
            model,
            model_info.slug.as_str(),
            /*account_id*/ None,
            /*account_email*/ None,
            /*auth_mode*/ None,
            "test_originator".to_string(),
            /*log_user_prompts*/ false,
            "test".to_string(),
            SessionSource::Cli,
        )
    }

    fn app_enabled_in_effective_config(config: &Config, app_id: &str) -> Option<bool> {
        config
            .config_layer_stack
            .effective_config()
            .as_table()
            .and_then(|table| table.get("apps"))
            .and_then(TomlValue::as_table)
            .and_then(|apps| apps.get(app_id))
            .and_then(TomlValue::as_table)
            .and_then(|app| app.get("enabled"))
            .and_then(TomlValue::as_bool)
    }

    fn all_model_presets() -> Vec<ModelPreset> {
        crate::legacy_core::test_support::all_model_presets().clone()
    }

    fn model_availability_nux_config(shown_count: &[(&str, u32)]) -> ModelAvailabilityNuxConfig {
        ModelAvailabilityNuxConfig {
            shown_count: shown_count
                .iter()
                .map(|(model, count)| ((*model).to_string(), *count))
                .collect(),
        }
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
            "gpt-5.2-codex",
            &seen,
            &all_model_presets()
        ));
        assert!(should_show_model_migration_prompt(
            "gpt-5-codex",
            "gpt-5.2-codex",
            &seen,
            &all_model_presets()
        ));
        assert!(should_show_model_migration_prompt(
            "gpt-5-codex-mini",
            "gpt-5.2-codex",
            &seen,
            &all_model_presets()
        ));
        assert!(should_show_model_migration_prompt(
            "gpt-5.1-codex",
            "gpt-5.2-codex",
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

    #[test]
    fn select_model_availability_nux_picks_only_eligible_model() {
        let mut presets = all_model_presets();
        presets.iter_mut().for_each(|preset| {
            preset.availability_nux = None;
        });
        let target = presets
            .iter_mut()
            .find(|preset| preset.model == "gpt-5")
            .expect("target preset present");
        target.availability_nux = Some(ModelAvailabilityNux {
            message: "gpt-5 is available".to_string(),
        });

        let selected = select_model_availability_nux(&presets, &model_availability_nux_config(&[]));

        assert_eq!(
            selected,
            Some(StartupTooltipOverride {
                model_slug: "gpt-5".to_string(),
                message: "gpt-5 is available".to_string(),
            })
        );
    }

    #[test]
    fn select_model_availability_nux_skips_missing_and_exhausted_models() {
        let mut presets = all_model_presets();
        presets.iter_mut().for_each(|preset| {
            preset.availability_nux = None;
        });
        let gpt_5 = presets
            .iter_mut()
            .find(|preset| preset.model == "gpt-5")
            .expect("gpt-5 preset present");
        gpt_5.availability_nux = Some(ModelAvailabilityNux {
            message: "gpt-5 is available".to_string(),
        });
        let gpt_5_2 = presets
            .iter_mut()
            .find(|preset| preset.model == "gpt-5.2")
            .expect("gpt-5.2 preset present");
        gpt_5_2.availability_nux = Some(ModelAvailabilityNux {
            message: "gpt-5.2 is available".to_string(),
        });

        let selected = select_model_availability_nux(
            &presets,
            &model_availability_nux_config(&[("gpt-5", MODEL_AVAILABILITY_NUX_MAX_SHOW_COUNT)]),
        );

        assert_eq!(
            selected,
            Some(StartupTooltipOverride {
                model_slug: "gpt-5.2".to_string(),
                message: "gpt-5.2 is available".to_string(),
            })
        );
    }

    #[test]
    fn active_turn_not_steerable_turn_error_extracts_structured_server_error() {
        let turn_error = AppServerTurnError {
            message: "cannot steer a review turn".to_string(),
            codex_error_info: Some(AppServerCodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: AppServerNonSteerableTurnKind::Review,
            }),
            additional_details: None,
        };
        let error = TypedRequestError::Server {
            method: "turn/steer".to_string(),
            source: JSONRPCErrorError {
                code: -32602,
                message: turn_error.message.clone(),
                data: Some(serde_json::to_value(&turn_error).expect("turn error should serialize")),
            },
        };

        assert_eq!(
            active_turn_not_steerable_turn_error(&error),
            Some(turn_error)
        );
    }

    #[test]
    fn active_turn_steer_race_detects_missing_active_turn() {
        let error = TypedRequestError::Server {
            method: "turn/steer".to_string(),
            source: JSONRPCErrorError {
                code: -32602,
                message: "no active turn to steer".to_string(),
                data: None,
            },
        };

        assert_eq!(
            active_turn_steer_race(&error),
            Some(ActiveTurnSteerRace::Missing)
        );
        assert_eq!(active_turn_not_steerable_turn_error(&error), None);
    }

    #[test]
    fn active_turn_steer_race_extracts_actual_turn_id_from_mismatch() {
        let error = TypedRequestError::Server {
            method: "turn/steer".to_string(),
            source: JSONRPCErrorError {
                code: -32602,
                message: "expected active turn id `turn-expected` but found `turn-actual`"
                    .to_string(),
                data: None,
            },
        };

        assert_eq!(
            active_turn_steer_race(&error),
            Some(ActiveTurnSteerRace::ExpectedTurnMismatch {
                actual_turn_id: "turn-actual".to_string(),
            })
        );
    }

    #[test]
    fn select_model_availability_nux_uses_existing_model_order_as_priority() {
        let mut presets = all_model_presets();
        presets.iter_mut().for_each(|preset| {
            preset.availability_nux = None;
        });
        let first = presets
            .iter_mut()
            .find(|preset| preset.model == "gpt-5")
            .expect("gpt-5 preset present");
        first.availability_nux = Some(ModelAvailabilityNux {
            message: "first".to_string(),
        });
        let second = presets
            .iter_mut()
            .find(|preset| preset.model == "gpt-5.2")
            .expect("gpt-5.2 preset present");
        second.availability_nux = Some(ModelAvailabilityNux {
            message: "second".to_string(),
        });

        let selected = select_model_availability_nux(&presets, &model_availability_nux_config(&[]));

        assert_eq!(
            selected,
            Some(StartupTooltipOverride {
                model_slug: "gpt-5.2".to_string(),
                message: "second".to_string(),
            })
        );
    }

    #[test]
    fn select_model_availability_nux_returns_none_when_all_models_are_exhausted() {
        let mut presets = all_model_presets();
        presets.iter_mut().for_each(|preset| {
            preset.availability_nux = None;
        });
        let target = presets
            .iter_mut()
            .find(|preset| preset.model == "gpt-5")
            .expect("target preset present");
        target.availability_nux = Some(ModelAvailabilityNux {
            message: "gpt-5 is available".to_string(),
        });

        let selected = select_model_availability_nux(
            &presets,
            &model_availability_nux_config(&[("gpt-5", MODEL_AVAILABILITY_NUX_MAX_SHOW_COUNT)]),
        );

        assert_eq!(selected, None);
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
    async fn model_migration_prompt_skips_when_target_missing_or_hidden() {
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

        assert!(!should_show_model_migration_prompt(
            &current.model,
            "missing-target",
            &BTreeMap::new(),
            &available,
        ));

        assert!(target_preset_for_upgrade(&available, "missing-target").is_none());

        let mut with_hidden_target = all_model_presets();
        let target = with_hidden_target
            .iter_mut()
            .find(|preset| preset.model == "gpt-5.2-codex")
            .expect("target preset present");
        target.show_in_picker = false;

        assert!(!should_show_model_migration_prompt(
            "gpt-5-codex",
            "gpt-5.2-codex",
            &BTreeMap::new(),
            &with_hidden_target,
        ));
        assert!(target_preset_for_upgrade(&with_hidden_target, "gpt-5.2-codex").is_none());
    }

    #[tokio::test]
    async fn model_migration_prompt_shows_for_hidden_model() {
        let codex_home = tempdir().expect("temp codex home");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config");

        let mut available_models = all_model_presets();
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
        // Test "hidden current model still prompts" even if bundled
        // catalog data changes the target model's picker visibility.
        available_models
            .iter_mut()
            .find(|preset| preset.model == upgrade.id)
            .expect("upgrade target present")
            .show_in_picker = true;
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
    async fn refresh_in_memory_config_from_disk_loads_latest_apps_state() -> Result<()> {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        let app_id = "unit_test_refresh_in_memory_config_connector".to_string();

        assert_eq!(app_enabled_in_effective_config(&app.config, &app_id), None);

        ConfigEditsBuilder::new(&app.config.codex_home)
            .with_edits([
                ConfigEdit::SetPath {
                    segments: vec!["apps".to_string(), app_id.clone(), "enabled".to_string()],
                    value: false.into(),
                },
                ConfigEdit::SetPath {
                    segments: vec![
                        "apps".to_string(),
                        app_id.clone(),
                        "disabled_reason".to_string(),
                    ],
                    value: "user".into(),
                },
            ])
            .apply()
            .await
            .expect("persist app toggle");

        assert_eq!(app_enabled_in_effective_config(&app.config, &app_id), None);

        app.refresh_in_memory_config_from_disk().await?;

        assert_eq!(
            app_enabled_in_effective_config(&app.config, &app_id),
            Some(false)
        );
        Ok(())
    }

    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_best_effort_keeps_current_config_on_error()
    -> Result<()> {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        std::fs::write(codex_home.path().join("config.toml"), "[broken")?;
        let original_config = app.config.clone();

        app.refresh_in_memory_config_from_disk_best_effort("starting a new thread")
            .await;

        assert_eq!(app.config, original_config);
        Ok(())
    }

    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_uses_active_chat_widget_cwd() -> Result<()> {
        let mut app = make_test_app().await;
        let original_cwd = app.config.cwd.clone();
        let next_cwd_tmp = tempdir()?;
        let next_cwd = next_cwd_tmp.path().to_path_buf();

        app.chat_widget.handle_codex_event(Event {
            id: String::new(),
            msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                session_id: ThreadId::new(),
                forked_from_id: None,
                thread_name: None,
                model: "gpt-test".to_string(),
                model_provider_id: "test-provider".to_string(),
                service_tier: None,
                approval_policy: AskForApproval::Never,
                approvals_reviewer: ApprovalsReviewer::User,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                cwd: next_cwd.clone().abs(),
                reasoning_effort: None,
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                network_proxy: None,
                rollout_path: Some(PathBuf::new()),
            }),
        });

        assert_eq!(app.chat_widget.config_ref().cwd.to_path_buf(), next_cwd);
        assert_eq!(app.config.cwd, original_cwd);

        app.refresh_in_memory_config_from_disk().await?;

        assert_eq!(app.config.cwd, app.chat_widget.config_ref().cwd);
        Ok(())
    }

    #[tokio::test]
    async fn rebuild_config_for_resume_or_fallback_uses_current_config_on_same_cwd_error()
    -> Result<()> {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        std::fs::write(codex_home.path().join("config.toml"), "[broken")?;
        let current_config = app.config.clone();
        let current_cwd = current_config.cwd.clone();

        let resume_config = app
            .rebuild_config_for_resume_or_fallback(&current_cwd, current_cwd.to_path_buf())
            .await?;

        assert_eq!(resume_config, current_config);
        Ok(())
    }

    #[tokio::test]
    async fn rebuild_config_for_resume_or_fallback_errors_when_cwd_changes() -> Result<()> {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        std::fs::write(codex_home.path().join("config.toml"), "[broken")?;
        let current_cwd = app.config.cwd.clone();
        let next_cwd_tmp = tempdir()?;
        let next_cwd = next_cwd_tmp.path().to_path_buf();

        let result = app
            .rebuild_config_for_resume_or_fallback(&current_cwd, next_cwd)
            .await;

        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn sync_tui_theme_selection_updates_chat_widget_config_copy() {
        let mut app = make_test_app().await;

        app.sync_tui_theme_selection("dracula".to_string());

        assert_eq!(app.config.tui_theme.as_deref(), Some("dracula"));
        assert_eq!(
            app.chat_widget.config_ref().tui_theme.as_deref(),
            Some("dracula")
        );
    }

    #[tokio::test]
    async fn fresh_session_config_uses_current_service_tier() {
        let mut app = make_test_app().await;
        app.chat_widget
            .set_service_tier(Some(codex_protocol::config_types::ServiceTier::Fast));

        let config = app.fresh_session_config();

        assert_eq!(
            config.service_tier,
            Some(codex_protocol::config_types::ServiceTier::Fast)
        );
    }

    #[tokio::test]
    async fn backtrack_selection_with_duplicate_history_targets_unique_turn() {
        let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;

        let user_cell = |text: &str,
                         text_elements: Vec<TextElement>,
                         local_image_paths: Vec<PathBuf>,
                         remote_image_urls: Vec<String>|
         -> Arc<dyn HistoryCell> {
            Arc::new(UserHistoryCell {
                message: text.to_string(),
                text_elements,
                local_image_paths,
                remote_image_urls,
            }) as Arc<dyn HistoryCell>
        };
        let agent_cell = |text: &str| -> Arc<dyn HistoryCell> {
            Arc::new(AgentMessageCell::new(
                vec![Line::from(text.to_string())],
                /*is_first_line*/ true,
            )) as Arc<dyn HistoryCell>
        };

        let make_header = |is_first| {
            let event = SessionConfiguredEvent {
                session_id: ThreadId::new(),
                forked_from_id: None,
                thread_name: None,
                model: "gpt-test".to_string(),
                model_provider_id: "test-provider".to_string(),
                service_tier: None,
                approval_policy: AskForApproval::Never,
                approvals_reviewer: ApprovalsReviewer::User,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                cwd: test_path_buf("/home/user/project").abs(),
                reasoning_effort: None,
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                network_proxy: None,
                rollout_path: Some(PathBuf::new()),
            };
            Arc::new(new_session_info(
                app.chat_widget.config_ref(),
                app.chat_widget.current_model(),
                event,
                is_first,
                /*tooltip_override*/ None,
                /*auth_plan*/ None,
                /*show_fast_status*/ false,
            )) as Arc<dyn HistoryCell>
        };

        let placeholder = "[Image #1]";
        let edited_text = format!("follow-up (edited) {placeholder}");
        let edited_range = edited_text.len().saturating_sub(placeholder.len())..edited_text.len();
        let edited_text_elements = vec![TextElement::new(
            edited_range.into(),
            /*placeholder*/ None,
        )];
        let edited_local_image_paths = vec![PathBuf::from("/tmp/fake-image.png")];

        // Simulate a transcript with duplicated history (e.g., from prior backtracks)
        // and an edited turn appended after a session header boundary.
        app.transcript_cells = vec![
            make_header(true),
            user_cell("first question", Vec::new(), Vec::new(), Vec::new()),
            agent_cell("answer first"),
            user_cell("follow-up", Vec::new(), Vec::new(), Vec::new()),
            agent_cell("answer follow-up"),
            make_header(false),
            user_cell("first question", Vec::new(), Vec::new(), Vec::new()),
            agent_cell("answer first"),
            user_cell(
                &edited_text,
                edited_text_elements.clone(),
                edited_local_image_paths.clone(),
                vec!["https://example.com/backtrack.png".to_string()],
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
                thread_name: None,
                model: "gpt-test".to_string(),
                model_provider_id: "test-provider".to_string(),
                service_tier: None,
                approval_policy: AskForApproval::Never,
                approvals_reviewer: ApprovalsReviewer::User,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                cwd: test_path_buf("/home/user/project").abs(),
                reasoning_effort: None,
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                network_proxy: None,
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
        assert_eq!(
            selection.remote_image_urls,
            vec!["https://example.com/backtrack.png".to_string()]
        );

        app.apply_backtrack_rollback(selection);
        assert_eq!(
            app.chat_widget.remote_image_urls(),
            vec!["https://example.com/backtrack.png".to_string()]
        );

        let mut rollback_turns = None;
        while let Ok(op) = op_rx.try_recv() {
            if let Op::ThreadRollback { num_turns } = op {
                rollback_turns = Some(num_turns);
            }
        }

        assert_eq!(rollback_turns, Some(1));
    }

    #[tokio::test]
    async fn backtrack_remote_image_only_selection_clears_existing_composer_draft() {
        let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;

        app.transcript_cells = vec![Arc::new(UserHistoryCell {
            message: "original".to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }) as Arc<dyn HistoryCell>];
        app.chat_widget
            .set_composer_text("stale draft".to_string(), Vec::new(), Vec::new());

        let remote_image_url = "https://example.com/remote-only.png".to_string();
        app.apply_backtrack_rollback(BacktrackSelection {
            nth_user_message: 0,
            prefill: String::new(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: vec![remote_image_url.clone()],
        });

        assert_eq!(app.chat_widget.composer_text_with_pending(), "");
        assert_eq!(app.chat_widget.remote_image_urls(), vec![remote_image_url]);

        let mut rollback_turns = None;
        while let Ok(op) = op_rx.try_recv() {
            if let Op::ThreadRollback { num_turns } = op {
                rollback_turns = Some(num_turns);
            }
        }
        assert_eq!(rollback_turns, Some(1));
    }

    #[tokio::test]
    async fn backtrack_resubmit_preserves_data_image_urls_in_user_turn() {
        let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;

        let thread_id = ThreadId::new();
        app.chat_widget.handle_codex_event(Event {
            id: String::new(),
            msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                session_id: thread_id,
                forked_from_id: None,
                thread_name: None,
                model: "gpt-test".to_string(),
                model_provider_id: "test-provider".to_string(),
                service_tier: None,
                approval_policy: AskForApproval::Never,
                approvals_reviewer: ApprovalsReviewer::User,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                cwd: test_path_buf("/home/user/project").abs(),
                reasoning_effort: None,
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                network_proxy: None,
                rollout_path: Some(PathBuf::new()),
            }),
        });

        let data_image_url = "data:image/png;base64,abc123".to_string();
        app.transcript_cells = vec![Arc::new(UserHistoryCell {
            message: "please inspect this".to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: vec![data_image_url.clone()],
        }) as Arc<dyn HistoryCell>];

        app.apply_backtrack_rollback(BacktrackSelection {
            nth_user_message: 0,
            prefill: "please inspect this".to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: vec![data_image_url.clone()],
        });

        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let mut saw_rollback = false;
        let mut submitted_items: Option<Vec<UserInput>> = None;
        while let Ok(op) = op_rx.try_recv() {
            match op {
                Op::ThreadRollback { .. } => saw_rollback = true,
                Op::UserTurn { items, .. } => submitted_items = Some(items),
                _ => {}
            }
        }

        assert!(saw_rollback);
        let items = submitted_items.expect("expected user turn after backtrack resubmit");
        assert!(items.iter().any(|item| {
            matches!(
                item,
                UserInput::Image { image_url } if image_url == &data_image_url
            )
        }));
    }

    #[tokio::test]
    async fn replay_thread_snapshot_replays_turn_history_in_order() {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        app.replay_thread_snapshot(
            ThreadEventSnapshot {
                session: Some(test_thread_session(
                    thread_id,
                    test_path_buf("/home/user/project"),
                )),
                turns: vec![
                    Turn {
                        id: "turn-1".to_string(),
                        items: vec![ThreadItem::UserMessage {
                            id: "user-1".to_string(),
                            content: vec![AppServerUserInput::Text {
                                text: "first prompt".to_string(),
                                text_elements: Vec::new(),
                            }],
                        }],
                        status: TurnStatus::Completed,
                        error: None,
                        started_at: None,
                        completed_at: None,
                        duration_ms: None,
                    },
                    Turn {
                        id: "turn-2".to_string(),
                        items: vec![
                            ThreadItem::UserMessage {
                                id: "user-2".to_string(),
                                content: vec![AppServerUserInput::Text {
                                    text: "third prompt".to_string(),
                                    text_elements: Vec::new(),
                                }],
                            },
                            ThreadItem::AgentMessage {
                                id: "assistant-2".to_string(),
                                text: "done".to_string(),
                                phase: None,
                                memory_citation: None,
                            },
                        ],
                        status: TurnStatus::Completed,
                        error: None,
                        started_at: None,
                        completed_at: None,
                        duration_ms: None,
                    },
                ],
                events: Vec::new(),
                input_state: None,
            },
            /*resume_restored_queue*/ false,
        );

        while let Ok(event) = app_event_rx.try_recv() {
            if let AppEvent::InsertHistoryCell(cell) = event {
                let cell: Arc<dyn HistoryCell> = cell.into();
                app.transcript_cells.push(cell);
            }
        }

        let user_messages: Vec<String> = app
            .transcript_cells
            .iter()
            .filter_map(|cell| {
                cell.as_any()
                    .downcast_ref::<UserHistoryCell>()
                    .map(|cell| cell.message.clone())
            })
            .collect();
        assert_eq!(
            user_messages,
            vec!["first prompt".to_string(), "third prompt".to_string()]
        );
    }

    #[tokio::test]
    async fn replace_chat_widget_reseeds_collab_agent_metadata_for_replay() {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let receiver_thread_id =
            ThreadId::from_string("019cff70-2599-75e2-af72-b958ce5dc1cc").expect("valid thread");
        app.agent_navigation.upsert(
            receiver_thread_id,
            Some("Robie".to_string()),
            Some("explorer".to_string()),
            /*is_closed*/ false,
        );

        let replacement = ChatWidget::new_with_app_event(ChatWidgetInit {
            config: app.config.clone(),
            frame_requester: crate::tui::FrameRequester::test_dummy(),
            app_event_tx: app.app_event_tx.clone(),
            initial_user_message: None,
            enhanced_keys_supported: app.enhanced_keys_supported,
            has_chatgpt_account: app.chat_widget.has_chatgpt_account(),
            model_catalog: app.model_catalog.clone(),
            feedback: app.feedback.clone(),
            is_first_run: false,
            status_account_display: app.chat_widget.status_account_display().cloned(),
            initial_plan_type: app.chat_widget.current_plan_type(),
            model: Some(app.chat_widget.current_model().to_string()),
            startup_tooltip_override: None,
            status_line_invalid_items_warned: app.status_line_invalid_items_warned.clone(),
            terminal_title_invalid_items_warned: app.terminal_title_invalid_items_warned.clone(),
            session_telemetry: app.session_telemetry.clone(),
        });
        app.replace_chat_widget(replacement);

        app.replay_thread_snapshot(
            ThreadEventSnapshot {
                session: None,
                turns: Vec::new(),
                events: vec![ThreadBufferedEvent::Notification(
                    ServerNotification::ItemStarted(codex_app_server_protocol::ItemStartedNotification {
                        thread_id: "thread-1".to_string(),
                        turn_id: "turn-1".to_string(),
                        item: ThreadItem::CollabAgentToolCall {
                            id: "wait-1".to_string(),
                            tool: codex_app_server_protocol::CollabAgentTool::Wait,
                            status: codex_app_server_protocol::CollabAgentToolCallStatus::InProgress,
                            sender_thread_id: ThreadId::new().to_string(),
                            receiver_thread_ids: vec![receiver_thread_id.to_string()],
                            prompt: None,
                            model: None,
                            reasoning_effort: None,
                            agents_states: HashMap::new(),
                        },
                    }),
                )],
                input_state: None,
            },
            /*resume_restored_queue*/ false,
        );

        let mut saw_named_wait = false;
        while let Ok(event) = app_event_rx.try_recv() {
            if let AppEvent::InsertHistoryCell(cell) = event {
                let transcript = lines_to_single_string(&cell.transcript_lines(/*width*/ 80));
                saw_named_wait |= transcript.contains("Robie [explorer]");
            }
        }

        assert!(
            saw_named_wait,
            "expected replayed wait item to keep agent name"
        );
    }

    #[tokio::test]
    async fn refreshed_snapshot_session_persists_resumed_turns() {
        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        let initial_session = test_thread_session(thread_id, test_path_buf("/tmp/original"));
        app.thread_event_channels.insert(
            thread_id,
            ThreadEventChannel::new_with_session(
                /*capacity*/ 4,
                initial_session.clone(),
                Vec::new(),
            ),
        );

        let resumed_turns = vec![test_turn(
            "turn-1",
            TurnStatus::Completed,
            vec![ThreadItem::UserMessage {
                id: "user-1".to_string(),
                content: vec![AppServerUserInput::Text {
                    text: "restored prompt".to_string(),
                    text_elements: Vec::new(),
                }],
            }],
        )];
        let resumed_session = ThreadSessionState {
            cwd: test_path_buf("/tmp/refreshed").abs(),
            ..initial_session.clone()
        };
        let mut snapshot = ThreadEventSnapshot {
            session: Some(initial_session),
            turns: Vec::new(),
            events: Vec::new(),
            input_state: None,
        };

        app.apply_refreshed_snapshot_thread(
            thread_id,
            AppServerStartedThread {
                session: resumed_session.clone(),
                turns: resumed_turns.clone(),
            },
            &mut snapshot,
        )
        .await;

        assert_eq!(snapshot.session, Some(resumed_session.clone()));
        assert_eq!(snapshot.turns, resumed_turns);

        let store = app
            .thread_event_channels
            .get(&thread_id)
            .expect("thread channel")
            .store
            .lock()
            .await;
        let store_snapshot = store.snapshot();
        assert_eq!(store_snapshot.session, Some(resumed_session));
        assert_eq!(store_snapshot.turns, snapshot.turns);
    }

    #[tokio::test]
    async fn queued_rollback_syncs_overlay_and_clears_deferred_history() {
        let mut app = make_test_app().await;
        app.transcript_cells = vec![
            Arc::new(UserHistoryCell {
                message: "first".to_string(),
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
                remote_image_urls: Vec::new(),
            }) as Arc<dyn HistoryCell>,
            Arc::new(AgentMessageCell::new(
                vec![Line::from("after first")],
                /*is_first_line*/ false,
            )) as Arc<dyn HistoryCell>,
            Arc::new(UserHistoryCell {
                message: "second".to_string(),
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
                remote_image_urls: Vec::new(),
            }) as Arc<dyn HistoryCell>,
            Arc::new(AgentMessageCell::new(
                vec![Line::from("after second")],
                /*is_first_line*/ false,
            )) as Arc<dyn HistoryCell>,
        ];
        app.overlay = Some(Overlay::new_transcript(app.transcript_cells.clone()));
        app.deferred_history_lines = vec![Line::from("stale buffered line")];
        app.backtrack.overlay_preview_active = true;
        app.backtrack.nth_user_message = 1;

        let changed = app.apply_non_pending_thread_rollback(/*num_turns*/ 1);

        assert!(changed);
        assert!(app.backtrack_render_pending);
        assert!(app.deferred_history_lines.is_empty());
        assert_eq!(app.backtrack.nth_user_message, 0);
        let user_messages: Vec<String> = app
            .transcript_cells
            .iter()
            .filter_map(|cell| {
                cell.as_any()
                    .downcast_ref::<UserHistoryCell>()
                    .map(|cell| cell.message.clone())
            })
            .collect();
        assert_eq!(user_messages, vec!["first".to_string()]);
        let overlay_cell_count = match app.overlay.as_ref() {
            Some(Overlay::Transcript(t)) => t.committed_cell_count(),
            _ => panic!("expected transcript overlay"),
        };
        assert_eq!(overlay_cell_count, app.transcript_cells.len());
    }

    #[tokio::test]
    async fn thread_rollback_response_discards_queued_active_thread_events() {
        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        let (tx, rx) = mpsc::channel(8);
        app.active_thread_id = Some(thread_id);
        app.active_thread_rx = Some(rx);
        tx.send(ThreadBufferedEvent::Notification(
            ServerNotification::ConfigWarning(ConfigWarningNotification {
                summary: "stale warning".to_string(),
                details: None,
                path: None,
                range: None,
            }),
        ))
        .await
        .expect("event should queue");

        app.handle_thread_rollback_response(
            thread_id,
            /*num_turns*/ 1,
            &ThreadRollbackResponse {
                thread: Thread {
                    id: thread_id.to_string(),
                    forked_from_id: None,
                    preview: String::new(),
                    ephemeral: false,
                    model_provider: "openai".to_string(),
                    created_at: 0,
                    updated_at: 0,
                    status: codex_app_server_protocol::ThreadStatus::Idle,
                    path: None,
                    cwd: test_path_buf("/tmp/project").abs(),
                    cli_version: "0.0.0".to_string(),
                    source: SessionSource::Cli.into(),
                    agent_nickname: None,
                    agent_role: None,
                    git_info: None,
                    name: None,
                    turns: Vec::new(),
                },
            },
        )
        .await;

        let rx = app
            .active_thread_rx
            .as_mut()
            .expect("active receiver should remain attached");
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[tokio::test]
    async fn new_session_requests_shutdown_for_previous_conversation() {
        let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;

        let thread_id = ThreadId::new();
        let event = SessionConfiguredEvent {
            session_id: thread_id,
            forked_from_id: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            cwd: test_path_buf("/home/user/project").abs(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        };

        app.chat_widget.handle_codex_event(Event {
            id: String::new(),
            msg: EventMsg::SessionConfigured(event),
        });

        while app_event_rx.try_recv().is_ok() {}
        while op_rx.try_recv().is_ok() {}

        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        app.shutdown_current_thread(&mut app_server).await;

        assert!(
            op_rx.try_recv().is_err(),
            "shutdown should not submit Op::Shutdown"
        );
    }

    #[tokio::test]
    async fn shutdown_first_exit_returns_immediate_exit_when_shutdown_submit_fails() {
        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        app.active_thread_id = Some(thread_id);

        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let control = app
            .handle_exit_mode(&mut app_server, ExitMode::ShutdownFirst)
            .await;

        assert_eq!(app.pending_shutdown_exit_thread_id, None);
        assert!(matches!(
            control,
            AppRunControl::Exit(ExitReason::UserRequested)
        ));
    }

    #[tokio::test]
    async fn shutdown_first_exit_uses_app_server_shutdown_without_submitting_op() {
        let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        app.active_thread_id = Some(thread_id);

        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let control = app
            .handle_exit_mode(&mut app_server, ExitMode::ShutdownFirst)
            .await;

        assert_eq!(app.pending_shutdown_exit_thread_id, None);
        assert!(matches!(
            control,
            AppRunControl::Exit(ExitReason::UserRequested)
        ));
        assert!(
            op_rx.try_recv().is_err(),
            "shutdown should not submit Op::Shutdown"
        );
    }

    #[tokio::test]
    async fn interrupt_without_active_turn_is_treated_as_handled() {
        let mut app = make_test_app().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
                .await
                .expect("embedded app server");
        let started = app_server
            .start_thread(app.chat_widget.config_ref())
            .await
            .expect("thread/start should succeed");
        let thread_id = started.session.thread_id;
        app.enqueue_primary_thread_session(started.session, started.turns)
            .await
            .expect("primary thread should be registered");
        let op = AppCommand::interrupt();

        let handled = app
            .try_submit_active_thread_op_via_app_server(&mut app_server, thread_id, &op)
            .await
            .expect("interrupt submission should not fail");

        assert_eq!(handled, true);
    }

    #[tokio::test]
    async fn clear_only_ui_reset_preserves_chat_session_state() {
        let mut app = make_test_app().await;
        let thread_id = ThreadId::new();
        app.chat_widget.handle_codex_event(Event {
            id: String::new(),
            msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                session_id: thread_id,
                forked_from_id: None,
                thread_name: Some("keep me".to_string()),
                model: "gpt-test".to_string(),
                model_provider_id: "test-provider".to_string(),
                service_tier: None,
                approval_policy: AskForApproval::Never,
                approvals_reviewer: ApprovalsReviewer::User,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                cwd: test_path_buf("/tmp/project").abs(),
                reasoning_effort: None,
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                network_proxy: None,
                rollout_path: Some(PathBuf::new()),
            }),
        });
        app.chat_widget
            .apply_external_edit("draft prompt".to_string());
        app.transcript_cells = vec![Arc::new(UserHistoryCell {
            message: "old message".to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }) as Arc<dyn HistoryCell>];
        app.overlay = Some(Overlay::new_transcript(app.transcript_cells.clone()));
        app.deferred_history_lines = vec![Line::from("stale buffered line")];
        app.has_emitted_history_lines = true;
        app.backtrack.primed = true;
        app.backtrack.overlay_preview_active = true;
        app.backtrack.nth_user_message = 0;
        app.backtrack_render_pending = true;

        app.reset_app_ui_state_after_clear();

        assert!(app.overlay.is_none());
        assert!(app.transcript_cells.is_empty());
        assert!(app.deferred_history_lines.is_empty());
        assert!(!app.has_emitted_history_lines);
        assert!(!app.backtrack.primed);
        assert!(!app.backtrack.overlay_preview_active);
        assert!(app.backtrack.pending_rollback.is_none());
        assert!(!app.backtrack_render_pending);
        assert_eq!(app.chat_widget.thread_id(), Some(thread_id));
        assert_eq!(app.chat_widget.composer_text_with_pending(), "draft prompt");
    }

    #[tokio::test]
    async fn session_summary_skips_when_no_usage_or_resume_hint() {
        assert!(
            session_summary(
                TokenUsage::default(),
                /*thread_id*/ None,
                /*thread_name*/ None
            )
            .is_none()
        );
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

        let summary =
            session_summary(usage, Some(conversation), /*thread_name*/ None).expect("summary");
        assert_eq!(
            summary.usage_line,
            Some("Token usage: total=12 input=10 output=2".to_string())
        );
        assert_eq!(
            summary.resume_command,
            Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }

    #[tokio::test]
    async fn session_summary_prefers_name_over_id() {
        let usage = TokenUsage {
            input_tokens: 10,
            output_tokens: 2,
            total_tokens: 12,
            ..Default::default()
        };
        let conversation = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();

        let summary = session_summary(usage, Some(conversation), Some("my-session".to_string()))
            .expect("summary");
        assert_eq!(
            summary.resume_command,
            Some("codex resume my-session".to_string())
        );
    }
}
