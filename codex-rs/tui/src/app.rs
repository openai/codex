//! Top-level TUI application state and runtime wiring.
//!
//! This module owns the `App` struct, shared imports, and the high-level run loop that coordinates
//! the focused app submodules.

use crate::AppServerTarget;
use crate::app_backtrack::BacktrackState;
use crate::app_command::AppCommand;
use crate::app_event::AppEvent;
use crate::app_event::ExitMode;
use crate::app_event::FeedbackCategory;
use crate::app_event::HistoryLookupResponse;
use crate::app_event::PermissionProfileSelection;
use crate::app_event::PluginLocation;
use crate::app_event::PluginRemoteSectionError;
use crate::app_event::RateLimitRefreshOrigin;
use crate::app_event::RunningTaskExitAction;
use crate::app_event::ThreadTitleDestination;
#[cfg(target_os = "windows")]
use crate::app_event::WindowsSandboxEnableMode;
use crate::app_event_sender::AppEventSender;
use crate::app_server_session::AppServerBootstrap;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::AppServerStartedThread;
use crate::app_server_session::TurnPermissionsOverride;
use crate::app_server_session::app_server_rate_limit_snapshots;
use crate::bottom_pane::AppLinkViewParams;
use crate::bottom_pane::ApplyPatchApprovalRequest;
use crate::bottom_pane::ApprovalRequest;
use crate::bottom_pane::ExecApprovalRequest;
use crate::bottom_pane::FeedbackAudience;
use crate::bottom_pane::McpElicitationApprovalRequest;
use crate::bottom_pane::McpServerElicitationFormRequest;
use crate::bottom_pane::PermissionsApprovalRequest;
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
use crate::hooks_rpc::HookTrustUpdate;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::KeyChordMatcher;
use crate::keymap::RuntimeKeymap;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use crate::legacy_core::config::PermissionProfileSnapshot;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use crate::managed_new_thread_defaults::apply_managed_new_thread_defaults;
use crate::model_catalog::ModelCatalog;
use crate::model_migration::ModelMigrationOutcome;
use crate::model_migration::migration_copy_for_models;
use crate::model_migration::run_model_migration_prompt;
use crate::multi_agents::agent_picker_status_dot_spans;
use crate::multi_agents::format_agent_picker_item_name;
use crate::multi_agents::next_agent_shortcut_matches;
use crate::multi_agents::previous_agent_shortcut_matches;
use crate::multi_agents::sub_agent_activity_display;
use crate::pager_overlay::Overlay;
use crate::render::highlight::highlight_bash_to_lines;
use crate::render::renderable::Renderable;
use crate::resume_picker::SessionSelection;
use crate::resume_picker::SessionTarget;
use crate::session_state::ThreadSessionState;
use crate::startup_draft::StartupDraftPump;
#[cfg(test)]
use crate::test_support::PathBufExt;
#[cfg(test)]
use crate::test_support::test_path_buf;
#[cfg(test)]
use crate::test_support::test_path_display;
use crate::token_usage::TokenUsage;
use crate::transcript_reflow::TranscriptReflowState;
use crate::tui;
use crate::tui::TuiEvent;
use crate::update_action::UpdateAction;
use crate::version::CODEX_CLI_VERSION;
use crate::workspace_command::AppServerWorkspaceCommandRunner;
use crate::workspace_command::WorkspaceCommandRunner;
use codex_ansi_escape::ansi_escape_line;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::AddCreditsNudgeCreditType;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CodexErrorInfo as AppServerCodexErrorInfo;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::FeedbackUploadParams;
use codex_app_server_protocol::FeedbackUploadResponse;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::HooksListEntry;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
#[cfg(test)]
use codex_app_server_protocol::McpAuthStatus;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::PluginInstallParams;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::PluginListMarketplaceKind;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginMarketplaceEntry;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginReadResponse;
use codex_app_server_protocol::PluginUninstallParams;
use codex_app_server_protocol::PluginUninstallResponse;
use codex_app_server_protocol::SandboxMode as AppServerSandboxMode;
use codex_app_server_protocol::SendAddCreditsNudgeEmailParams;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SkillErrorInfo;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::SkillsListResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadMemoryMode;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use codex_app_server_protocol::ThreadStartSource;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnError as AppServerTurnError;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::WriteStatus;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::types::ApprovalsReviewer;
use codex_config::types::MemoriesToml;
use codex_config::types::ModelAvailabilityNuxConfig;
#[cfg(target_os = "windows")]
use codex_config::types::WindowsToml;
use codex_exec_server::EnvironmentManager;
use codex_features::Feature;
use codex_features::FeaturesToml;
use codex_models_manager::model_presets::HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG;
use codex_models_manager::model_presets::HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG;
use codex_otel::SessionTelemetry;
use codex_otel::TelemetryAuthMode;
use codex_protocol::ThreadId;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ModelAvailabilityNux;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelUpgrade;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_rollout::StateDbHandle;
use codex_terminal_detection::user_agent;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_approval_presets::builtin_permission_profile_for_active_permission_profile;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
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
mod agent_message_consolidation;
mod agent_navigation;
mod agent_picker;
mod agent_status_feed;
mod agents_overview;
mod agents_overview_actions;
mod agents_overview_details;
mod agents_overview_threads;
mod agents_overview_usage;
mod agents_overview_view;
pub(crate) use agents_overview::AGENTS_OVERVIEW_VIEW_ID;
mod app_server_event_targets;
mod app_server_events;
pub(crate) mod app_server_requests;
mod backend_banner_fallback;
mod background_requests;
mod config_persistence;
mod connector_mentions;
mod event_dispatch;
mod exit_summary;
mod experimental_features;
mod file_change_approvals;
mod history_pagination;
mod history_ui;
mod input;
mod loaded_threads;
mod managed_worktree_creation;
mod misalignment_policy;
mod model_defaults;
mod new_session;
pub(crate) use new_session::has_launch_setting;
mod pending_interactive_replay;
mod permission_shortcuts;
mod pets;
mod platform_actions;
mod plugin_mentions;
mod rate_limit_refresh;
mod realtime_delivery;
mod realtime_settings;
mod reasoning_replay;
mod recap;
mod reconnect;
mod replay_filter;
mod resize_reflow;
mod resume_config;
mod safety_buffering;
mod server_version_notice;
mod session_lifecycle;
mod session_picker;
mod side;
mod startup;
mod startup_prompts;
mod startup_warnings;
mod thread_event_buffer;
mod thread_events;
mod thread_goal_actions;
mod thread_routing;
mod thread_session_state;
mod thread_settings;
mod thread_title;
mod transcript_export;
mod user_verification;
mod user_verification_errors;
mod user_verification_requests;
mod working_directory;

use self::agent_navigation::AgentNavigationDirection;
use self::agent_navigation::AgentNavigationState;
use self::app_server_requests::PendingAppServerRequests;
use self::loaded_threads::find_loaded_subagent_threads_for_primary;
use self::pending_interactive_replay::PendingInteractiveReplayState;
pub(crate) use self::platform_actions::WindowsSandboxHost;
use self::platform_actions::*;
use self::side::SideParentStatus;
use self::side::SideParentStatusChange;
use self::side::SideThreadState;
use self::startup_prompts::*;
use self::thread_events::*;

const EXTERNAL_EDITOR_HINT: &str = "Save and close external editor to continue.";
const THREAD_EVENT_CHANNEL_CAPACITY: usize = 32768;

enum ThreadInteractiveRequest {
    AppLink(AppLinkViewParams),
    Approval(ApprovalRequest),
    McpServerElicitation(McpServerElicitationFormRequest),
    UserVerification {
        thread_id: ThreadId,
        request: crate::bottom_pane::user_verification::UserVerificationRequest,
    },
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

fn sub_agent_activity_item(notification: &ServerNotification) -> Option<&ThreadItem> {
    match notification {
        ServerNotification::ItemStarted(notification) => match &notification.item {
            ThreadItem::SubAgentActivity { .. } => Some(&notification.item),
            _ => None,
        },
        ServerNotification::ItemCompleted(notification) => match &notification.item {
            ThreadItem::SubAgentActivity { .. } => Some(&notification.item),
            _ => None,
        },
        _ => None,
    }
}

fn collab_receiver_is_not_found(
    notification: &ServerNotification,
    receiver_thread_id: &str,
) -> bool {
    match notification {
        ServerNotification::ItemCompleted(notification) => match &notification.item {
            ThreadItem::CollabAgentToolCall { agents_states, .. } => {
                agents_states.get(receiver_thread_id).is_some_and(|state| {
                    matches!(
                        &state.status,
                        codex_app_server_protocol::CollabAgentStatus::NotFound
                    )
                })
            }
            _ => false,
        },
        _ => false,
    }
}

fn default_exec_approval_decisions(
    network_approval_context: Option<&codex_app_server_protocol::NetworkApprovalContext>,
    proposed_execpolicy_amendment: Option<&codex_app_server_protocol::ExecPolicyAmendment>,
    proposed_network_policy_amendments: Option<
        &[codex_app_server_protocol::NetworkPolicyAmendment],
    >,
    additional_permissions: Option<&codex_app_server_protocol::AdditionalPermissionProfile>,
) -> Vec<codex_app_server_protocol::CommandExecutionApprovalDecision> {
    use codex_app_server_protocol::CommandExecutionApprovalDecision;
    use codex_app_server_protocol::NetworkPolicyRuleAction;

    if network_approval_context.is_some() {
        let mut decisions = vec![
            CommandExecutionApprovalDecision::Accept,
            CommandExecutionApprovalDecision::AcceptForSession,
        ];
        if let Some(amendment) = proposed_network_policy_amendments.and_then(|amendments| {
            amendments
                .iter()
                .find(|amendment| amendment.action == NetworkPolicyRuleAction::Allow)
        }) {
            decisions.push(
                CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
                    network_policy_amendment: amendment.clone(),
                },
            );
        }
        decisions.push(CommandExecutionApprovalDecision::Cancel);
        return decisions;
    }

    if additional_permissions.is_some() {
        return vec![
            CommandExecutionApprovalDecision::Accept,
            CommandExecutionApprovalDecision::Cancel,
        ];
    }

    let mut decisions = vec![CommandExecutionApprovalDecision::Accept];
    if let Some(execpolicy_amendment) = proposed_execpolicy_amendment {
        decisions.push(
            CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
                execpolicy_amendment: execpolicy_amendment.clone(),
            },
        );
    }
    decisions.push(CommandExecutionApprovalDecision::Cancel);
    decisions
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AutoReviewMode {
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    active_permission_profile: ActivePermissionProfile,
}

/// Enabling the Auto-review experiment in the TUI should also switch the
/// current `/permissions` settings to the matching Auto-review mode. Users
/// can still change `/permissions` afterward; this just assumes that opting into
/// the experiment means they want Auto-review enabled immediately.
fn auto_review_mode() -> AutoReviewMode {
    AutoReviewMode {
        approval_policy: AskForApproval::OnRequest,
        approvals_reviewer: ApprovalsReviewer::AutoReview,
        active_permission_profile: ActivePermissionProfile::new(
            BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
        ),
    }
}

#[cfg(test)]
impl AutoReviewMode {
    fn permission_profile(&self) -> PermissionProfile {
        builtin_permission_profile_for_active_permission_profile(&self.active_permission_profile)
            .expect("auto-review mode should use a built-in permission profile")
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
    pub resume_hint: Option<ResumableThread>,
    pub disconnect_info: Option<DisconnectInfo>,
    pub update_action: Option<UpdateAction>,
    pub exit_reason: ExitReason,
}

impl AppExitInfo {
    pub fn fatal(message: impl Into<String>) -> Self {
        Self {
            token_usage: TokenUsage::default(),
            thread_id: None,
            resume_hint: None,
            disconnect_info: None,
            update_action: None,
            exit_reason: ExitReason::Fatal(message.into()),
        }
    }
}

pub use exit_summary::DisconnectInfo;
pub use exit_summary::ResumableThread;

#[derive(Debug)]
pub(crate) enum AppRunControl {
    Continue,
    Exit(ExitReason),
}

#[derive(Debug, Clone)]
pub enum ExitReason {
    UserRequested,
    Archived(ThreadId),
    TurnInterrupted,
    /// The current thread was deleted, rather than disconnected.
    ThreadRemoved,
    Fatal(String),
}

fn session_summary(
    token_usage: TokenUsage,
    thread_id: Option<ThreadId>,
    thread_name: Option<String>,
    rollout_path: Option<&Path>,
) -> Option<SessionSummary> {
    let usage_line = (!token_usage.is_zero()).then(|| token_usage.to_string());
    let resume_hint = resume_hint_for_resumable_thread(thread_id, thread_name, rollout_path);

    if usage_line.is_none() && resume_hint.is_none() {
        return None;
    }

    Some(SessionSummary {
        usage_line,
        resume_hint,
    })
}

fn resumable_thread(
    thread_id: Option<ThreadId>,
    thread_name: Option<String>,
    rollout_path: Option<&Path>,
) -> Option<ResumableThread> {
    let thread_id = thread_id?;
    let rollout_path = rollout_path?;
    rollout_path_is_resumable(rollout_path).then_some(ResumableThread {
        thread_id,
        thread_name,
    })
}

fn resume_hint_for_resumable_thread(
    thread_id: Option<ThreadId>,
    thread_name: Option<String>,
    rollout_path: Option<&Path>,
) -> Option<String> {
    let thread = resumable_thread(thread_id, thread_name, rollout_path)?;
    codex_utils_cli::resume_hint(thread.thread_name.as_deref(), Some(thread.thread_id))
}

fn rollout_path_is_resumable(rollout_path: &Path) -> bool {
    std::fs::metadata(rollout_path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

fn errors_for_cwd(cwd: &Path, response: &SkillsListResponse) -> Vec<SkillErrorInfo> {
    response
        .data
        .iter()
        .find(|entry| entry.cwd.as_path() == cwd)
        .map(|entry| entry.errors.clone())
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionSummary {
    usage_line: Option<String>,
    resume_hint: Option<String>,
}

#[derive(Debug, Default)]
struct InitialHistoryReplayBuffer {
    retained_lines: VecDeque<crate::terminal_hyperlinks::HyperlinkLine>,
    render_from_transcript_tail: bool,
    was_truncated: bool,
}

pub(crate) struct App {
    feature_write_lock: Arc<tokio::sync::Mutex<()>>,
    model_catalog: Arc<ModelCatalog>,
    pub(crate) session_telemetry: SessionTelemetry,
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) chat_widget: ChatWidget,
    workspace_command_runner: Option<WorkspaceCommandRunner>,
    /// Legacy bootstrap and server-setting inputs; local preferences live in `local_settings`.
    pub(crate) config: Config,
    pub(crate) local_settings: crate::local_settings::LocalSettings,
    launch_cwd: PathBuf,
    /// Resume anchor selected by `/cd`; ordinary resumes retain the immutable launch cwd.
    runtime_working_directory_override: Option<PathBuf>,
    pub(crate) state_db: Option<StateDbHandle>,
    cli_kv_overrides: Vec<(String, TomlValue)>,
    harness_overrides: ConfigOverrides,
    loader_overrides: LoaderOverrides,
    cloud_config_bundle: CloudConfigBundleLoader,
    runtime_approval_policy_override: Option<RuntimeApprovalPolicyOverride>,
    runtime_permission_profile_override: Option<RuntimePermissionProfileOverride>,
    /// In-flight remote selections; confirmed settings live in each task's server snapshot.
    pending_server_profiles: HashMap<ThreadId, PermissionProfileSelection>,

    pub(crate) file_search: FileSearchManager,

    pub(crate) transcript_cells: Vec<Arc<dyn HistoryCell>>,
    last_rendered_history_tail: Option<history_ui::RenderedHistoryTail>,
    last_thread_usage_status_cell: Option<history_ui::ThreadUsageStatusHistory>,
    pub(crate) pending_thread_usage_history_refresh: bool,

    // Pager overlay state (Transcript or Static like Diff)
    pub(crate) overlay: Option<Overlay>,
    pub(crate) deferred_history_lines: Vec<crate::terminal_hyperlinks::HyperlinkLine>,
    has_emitted_history_lines: bool,
    transcript_reflow: TranscriptReflowState,
    initial_history_replay_buffer: Option<InitialHistoryReplayBuffer>,
    pending_thread_switch_resets: usize,
    pub(crate) scrollback_has_older_history: bool,

    pub(crate) enhanced_keys_supported: bool,
    pub(crate) keymap: RuntimeKeymap,
    pub(crate) key_chord_matcher: KeyChordMatcher,

    /// The foreground loop owns stream pacing; stopped animations have no timer.
    pub(crate) commit_animation: Option<tokio::time::Interval>,
    // Shared across ChatWidget instances so invalid status-line config warnings only emit once.
    status_line_invalid_items_warned: Arc<AtomicBool>,
    // Shared across ChatWidget instances so invalid terminal-title config warnings only emit once.
    terminal_title_invalid_items_warned: Arc<AtomicBool>,
    // Tracks active skill-load warnings so refreshes do not duplicate history cells.
    skill_load_warnings: SkillLoadWarningState,

    // Esc-backtracking state grouped
    pub(crate) backtrack: crate::app_backtrack::BacktrackState,
    /// When set, the next draw rebuilds terminal scrollback from the retained transcript cells.
    ///
    /// This keeps scrollback consistent with the retained transcript after backtracking.
    pub(crate) backtrack_render_pending: bool,
    pub(crate) feedback: codex_feedback::CodexFeedback,
    feedback_audience: FeedbackAudience,
    environment_manager: Arc<EnvironmentManager>,
    app_server_target: AppServerTarget,
    reconnect: reconnect::ReconnectState,
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
    pending_realtime_speech_replay: HashMap<ThreadId, Vec<(String, ThreadItem)>>,
    pending_realtime_transcript_replay:
        HashMap<ThreadId, VecDeque<crate::chatwidget::RealtimeTranscriptRecord>>,
    realtime_replay_order: VecDeque<ThreadId>,
    temporary_structured_requests: HashMap<ThreadId, mpsc::UnboundedSender<ServerNotification>>,
    /// Track title generation across thread switches and deduplicate automatic requests.
    pending_thread_titles: HashSet<(ThreadId, ThreadTitleDestination)>,
    thread_event_listener_tasks: HashMap<ThreadId, JoinHandle<()>>,
    agent_navigation: AgentNavigationState,
    agents_overview: agents_overview::AgentsOverviewState,
    side_threads: HashMap<ThreadId, SideThreadState>,
    abandoned_side_threads: HashSet<ThreadId>,
    active_thread_id: Option<ThreadId>,
    active_thread_rx: Option<mpsc::Receiver<ThreadBufferedEvent>>,
    primary_thread_id: Option<ThreadId>,
    last_subagent_backfill_attempt: Option<ThreadId>,
    primary_session_configured: Option<ThreadSessionState>,
    pending_primary_events: VecDeque<ThreadBufferedEvent>,
    pending_app_server_requests: PendingAppServerRequests,
    dynamic_tool_status_updates:
        tokio::sync::broadcast::Sender<codex_app_server_protocol::ThreadStatusChangedNotification>,
    dynamic_tool_tasks: HashMap<codex_app_server_protocol::RequestId, (String, JoinHandle<()>)>,
    pending_startup_thread_start: bool,
    pending_server_version_notice: Option<crate::status::remote_connection::ServerVersionNotice>,
    /// Opens the session picker after event dispatch returns, with a fresh stack.
    pending_open_resume_picker: bool,
    /// Runs a requested /cd after event dispatch returns, with a fresh stack.
    pending_working_directory_change: Option<working_directory::PendingWorkingDirectoryChange>,
    /// Starts worktree setup after the event handler returns, with a fresh stack.
    pending_start_managed_worktree: Option<(crate::app_event::ManagedWorktreeMode, Option<String>)>,
    pending_managed_worktree_creation: bool,
    /// Defers checkout completion and config loading until the event handler returns.
    pending_managed_worktree_created: Option<Box<crate::app_event::ManagedWorktreeCreated>>,
    /// Defers the saved-history fork until the event handler has returned.
    pending_managed_worktree_transition: Option<Box<crate::app_event::ManagedWorktreeTransition>>,
    /// Holds notifications until the new widget is attached on a fresh loop iteration.
    pending_managed_worktree_attach: Option<Box<working_directory::ManagedWorktreeAttach>>,
    /// Keeps protected screens quarantined until initialized chat receives genuine user input.
    startup_protected_input_boundary: bool,
    /// Keeps that boundary armed while a startup approval waits for the typing-idle timer.
    startup_pending_protected_request: bool,
    /// Invalidates in-flight full rate-limit reads when a newer rolling hard stop arrives.
    rate_limit_hard_stop_generation: u64,
    rate_limit_refresh_state: rate_limit_refresh::RateLimitRefreshState,
    // Serialize plugin enablement writes per plugin so stale completions cannot
    // overwrite a newer toggle, even if the plugin is toggled from different
    // cwd contexts.
    pending_plugin_enabled_writes: HashMap<String, Option<bool>>,
    // Serialize hook enablement writes per hook so stale completions cannot
    // persist an older toggle after a newer one.
    pending_hook_enabled_writes: HashMap<String, Option<bool>>,
    recap: recap::RecapState,
}

#[derive(Debug, Clone, PartialEq)]
struct RuntimePermissionProfileOverride {
    permission_profile: PermissionProfile,
    active_permission_profile: Option<ActivePermissionProfile>,
    network: Option<crate::legacy_core::config::NetworkProxySpec>,
    approvals_reviewer: ApprovalsReviewer,
    turn_override: RuntimePermissionProfileTurnOverride,
}

/// Separates user choices from settings inherited when attaching to another task.
#[derive(Debug, Clone, Copy, PartialEq)]
enum RuntimeApprovalPolicyOverride {
    Explicit(AskForApproval),
    Restored(AskForApproval),
}

impl RuntimeApprovalPolicyOverride {
    fn policy(self) -> AskForApproval {
        match self {
            Self::Explicit(policy) | Self::Restored(policy) => policy,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimePolicyOverrideScope {
    All,
    ExplicitOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimePermissionProfileTurnOverride {
    Preserve,
    LegacySandbox,
}

impl RuntimePermissionProfileOverride {
    fn from_config(config: &Config) -> Self {
        Self {
            permission_profile: config.permissions.permission_profile().clone(),
            active_permission_profile: config.permissions.active_permission_profile(),
            network: config.permissions.network.clone(),
            approvals_reviewer: config.approvals_reviewer,
            turn_override: RuntimePermissionProfileTurnOverride::LegacySandbox,
        }
    }

    fn from_restored_config(config: &Config) -> Self {
        Self {
            turn_override: RuntimePermissionProfileTurnOverride::Preserve,
            ..Self::from_config(config)
        }
    }

    fn matches_config(&self, config: &Config) -> bool {
        self.permission_profile == *config.permissions.permission_profile()
            && self.active_permission_profile == config.permissions.active_permission_profile()
            && self.network == config.permissions.network
            && self.approvals_reviewer == config.approvals_reviewer
    }

    fn turn_permission_profile(&self) -> Option<&PermissionProfile> {
        matches!(
            self.turn_override,
            RuntimePermissionProfileTurnOverride::LegacySandbox
        )
        .then_some(&self.permission_profile)
    }
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

fn active_turn_interrupt_race(error: &TypedRequestError) -> Option<String> {
    let TypedRequestError::Server { method, source } = error else {
        return None;
    };
    if method != "turn/interrupt" {
        return None;
    }
    let mismatch_prefix = "expected active turn id ";
    let mismatch_separator = " but found ";
    Some(
        source
            .message
            .strip_prefix(mismatch_prefix)?
            .split_once(mismatch_separator)?
            .1
            .to_string(),
    )
}

impl App {
    pub fn chatwidget_init_for_forked_or_resumed_thread(
        &self,
        tui: &mut tui::Tui,
        cfg: crate::legacy_core::config::Config,
        initial_user_message: Option<crate::chatwidget::UserMessage>,
    ) -> crate::chatwidget::ChatWidgetInit {
        crate::chatwidget::ChatWidgetInit {
            local_settings: self.local_settings.clone(),
            config: cfg,
            frame_requester: tui.frame_requester(),
            app_event_tx: self.app_event_tx.clone(),
            workspace_command_runner: self.workspace_command_runner.clone(),
            initial_user_message,
            enhanced_keys_supported: self.enhanced_keys_supported,
            has_chatgpt_account: self.chat_widget.has_chatgpt_account(),
            requires_openai_auth: self.chat_widget.requires_openai_auth,
            has_codex_backend_auth: self.chat_widget.has_codex_backend_auth(),
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

    pub(crate) async fn handle_tui_event(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        event: TuiEvent,
    ) -> Result<AppRunControl> {
        if self.reconnect.offline
            && let TuiEvent::Key(key) = &event
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && let KeyCode::Char(character) = key.code
            && (character.eq_ignore_ascii_case(&'c')
                || (character.eq_ignore_ascii_case(&'d')
                    && self.chat_widget.composer_is_empty()
                    && self.chat_widget.no_modal_or_popup_active()))
        {
            return Ok(AppRunControl::Exit(ExitReason::UserRequested));
        }
        let screen_size = tui.screen_size_for_event(&event)?;
        if !matches!(
            &event,
            TuiEvent::Key(_) | TuiEvent::Paste(_) | TuiEvent::FocusLost
        ) {
            self.expire_pending_key_chord();
            self.handle_draw_pre_render(tui, screen_size)?;
        }

        let event = if let TuiEvent::Key(mut key_event) = event {
            let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
            if self.should_recover_vim_insert_escape(key_event) {
                // Restore both strokes before chords or global shortcuts can consume them.
                if let Some(escape) = self.route_key_chord_event(tui, escape) {
                    self.handle_key_event(tui, app_server, escape).await;
                }
                key_event.modifiers.remove(KeyModifiers::ALT);
            }

            let Some(key_event) = self.route_key_chord_event(tui, key_event) else {
                return Ok(AppRunControl::Continue);
            };
            TuiEvent::Key(key_event)
        } else {
            event
        };

        if self.reconnect.offline
            && let TuiEvent::Key(key) = &event
        {
            if self.reconnect.presentation == reconnect::ReconnectPresentation::Overview {
                self.chat_widget.handle_disconnected_view_key(*key);
            } else {
                self.chat_widget.handle_disconnected_key(*key);
            }
            return Ok(AppRunControl::Continue);
        }

        match &event {
            TuiEvent::FocusLost => {
                self.chat_widget
                    .set_sparkle_terminal_focus(/*focused*/ false);
                let now = Instant::now();
                let thread_id = self.current_displayed_thread_id();

                self.recap.note_focus_lost(now);

                if let Some(thread_id) = thread_id {
                    self.schedule_recap_check(thread_id, now);
                }
            }
            TuiEvent::FocusGained => {
                self.recap.note_focus_gained();
            }
            _ => {}
        }

        if self.overlay.is_some() {
            let _ = self
                .handle_backtrack_overlay_event(tui, app_server, event)
                .await?;
        } else {
            match event {
                TuiEvent::Key(key_event) => {
                    self.handle_key_event(tui, app_server, key_event).await;
                }
                TuiEvent::Paste(pasted) => {
                    // Pasted text may contain CRLF pairs or bare CRs (e.g., from iTerm2),
                    // but tui-textarea expects LF. Normalize CRLF pairs before bare CRs so
                    // each pasted line break becomes one LF and existing LFs stay unchanged.
                    // [tui-textarea]: https://github.com/rhysd/tui-textarea/blob/4d18622eeac13b309e0ff6a55a46ac6706da68cf/src/textarea.rs#L782-L783
                    // [iTerm2]: https://github.com/gnachman/iTerm2/blob/5d0c0d9f68523cbd0494dad5422998964a2ecd8d/sources/iTermPasteHelper.m#L206-L216
                    let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
                    self.chat_widget.handle_paste(pasted);
                    if self.reconnect.offline
                        && self.reconnect.presentation
                            == reconnect::ReconnectPresentation::Conversation
                    {
                        self.chat_widget.handle_disconnected_key(KeyEvent::new(
                            KeyCode::Null,
                            KeyModifiers::NONE,
                        ));
                    }
                }
                TuiEvent::Draw | TuiEvent::Resume | TuiEvent::Resize(_) | TuiEvent::FocusGained => {
                    if self.backtrack_render_pending {
                        self.rebuild_transcript_after_backtrack(tui, screen_size.into())?;
                        self.backtrack_render_pending = false;
                    }
                    self.chat_widget.maybe_post_pending_notification(tui);
                    if self
                        .chat_widget
                        .handle_paste_burst_tick(tui.frame_requester())
                    {
                        tui.defer_screen_size(screen_size);
                        return Ok(AppRunControl::Continue);
                    }
                    // Allow widgets to process any pending timers before rendering.
                    let had_active_view = self.chat_widget.has_active_view();
                    self.chat_widget.pre_draw_tick();
                    self.refresh_agents_overview_usage(app_server, tui.frame_requester());
                    let rendered_area = self.render_chat_widget_frame(tui, screen_size)?;
                    if !had_active_view
                        && self.chat_widget.has_active_view()
                        && self.startup_protected_input_boundary
                    {
                        tui.discard_pending_input_before_interactive_screen()?;
                        self.startup_pending_protected_request = false;
                    }
                    if self.chat_widget.ambient_pet_image_enabled() {
                        let ambient_pet_area = Rect::new(
                            /*x*/ 0,
                            /*y*/ 0,
                            screen_size.width,
                            screen_size.height,
                        );
                        if let Err(err) = tui.draw_ambient_pet_image(
                            self.chat_widget
                                .ambient_pet_draw(ambient_pet_area, rendered_area.bottom()),
                        ) {
                            self.handle_ambient_pet_image_render_error(tui, err)?;
                        }
                    }
                    if let Some(request) = self.chat_widget.pet_picker_preview_draw() {
                        if let Err(err) = tui.draw_pet_picker_preview_image(Some(request)) {
                            self.handle_pet_picker_preview_image_render_error(tui, err)?;
                        }
                    } else if self.chat_widget.should_clear_pet_picker_preview_image()
                        && let Err(err) = tui.draw_pet_picker_preview_image(/*request*/ None)
                    {
                        self.handle_pet_picker_preview_image_render_error(tui, err)?;
                    }
                    if self.chat_widget.external_editor_state() == ExternalEditorState::Requested {
                        self.chat_widget
                            .set_external_editor_state(ExternalEditorState::Active);
                        self.app_event_tx.send(AppEvent::LaunchExternalEditor);
                    }
                }
                TuiEvent::FocusLost => {}
            }
        }
        Ok(AppRunControl::Continue)
    }

    pub(super) fn show_shutdown_feedback(&mut self, tui: &mut tui::Tui) -> Result<()> {
        self.disable_ambient_pet_before_shutdown(tui)?;
        self.chat_widget.show_shutdown_in_progress();
        let screen_size = tui.terminal.last_known_screen_size;
        self.handle_draw_pre_render(tui, screen_size)?;
        self.chat_widget.pre_draw_tick();
        self.render_chat_widget_frame(tui, screen_size)?;
        Ok(())
    }

    fn render_chat_widget_frame(&mut self, tui: &mut tui::Tui, screen_size: Size) -> Result<Rect> {
        self.sync_thread_title_progress();
        self.chat_widget
            .set_sparkle_terminal_focus(tui.is_terminal_focused());
        let dashboard_visible = self
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_some();
        let dashboard_was_visible = std::mem::replace(
            &mut self.agents_overview.rendered_full_screen,
            dashboard_visible,
        );
        // Full-height inline overlays scroll history off screen without a terminal resize.
        // Rebuild it once when returning to a content-height chat viewport.
        let restoring_inline_viewport = !tui.is_alt_screen_active()
            && tui.terminal.viewport_area.height == screen_size.height
            && self.with_chat_widget_frame(screen_size.width, |height, _| height)
                < screen_size.height;
        if !dashboard_visible && (dashboard_was_visible || restoring_inline_viewport) {
            self.schedule_immediate_resize_reflow(tui);
            self.maybe_run_resize_reflow(tui, screen_size)?;
        }
        self.with_chat_widget_frame(screen_size.width, |desired_height, chat_widget| {
            let desired_height = if dashboard_visible {
                screen_size.height
            } else {
                desired_height
            };
            let mut rendered_area = Rect::default();
            tui.draw_with_resize_reflow(desired_height, screen_size, |frame| {
                let area = frame.area();
                rendered_area = area;
                chat_widget.render(area, frame.buffer);
                self.chat_widget.note_rendered_width(area.width);
                if let Some((x, y)) = chat_widget.cursor_pos(area) {
                    frame.set_cursor_style(chat_widget.cursor_style(area));
                    frame.set_cursor_position((x, y));
                }
            })?;
            Ok(rendered_area)
        })
    }

    fn with_chat_widget_frame<T>(
        &self,
        width: u16,
        render: impl FnOnce(u16, &dyn Renderable) -> T,
    ) -> T {
        let chat_widget = self.chat_widget.as_renderable();
        render(chat_widget.desired_height(width), &chat_widget)
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Err(err) = self.chat_widget.clear_managed_terminal_title() {
            tracing::debug!(error = %err, "failed to clear terminal title on app drop");
        }
    }
}

#[cfg(test)]
pub(super) mod test_support;
#[cfg(test)]
mod tests;
