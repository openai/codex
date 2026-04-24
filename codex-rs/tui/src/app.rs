//! Top-level TUI application state and runtime wiring.
//!
//! This module owns the `App` struct, shared imports, and the high-level run loop that coordinates
//! the focused app submodules.

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
use crate::external_agent_config_migration_startup::ExternalAgentConfigMigrationStartupOutcome;
use crate::external_agent_config_migration_startup::handle_external_agent_config_migration_prompt_if_needed;
use crate::external_editor;
use crate::file_search::FileSearchManager;
use crate::history_cell;
use crate::history_cell::HistoryCell;
#[cfg(not(debug_assertions))]
use crate::history_cell::UpdateAvailableHistoryCell;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::RuntimeKeymap;
use crate::legacy_core::append_message_history_entry;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use crate::legacy_core::config::edit::ConfigEdit;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
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
use codex_app_server_protocol::AddCreditsNudgeCreditType;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CodexErrorInfo as AppServerCodexErrorInfo;
use codex_app_server_protocol::ConfigLayerSource;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::FeedbackUploadParams;
use codex_app_server_protocol::FeedbackUploadResponse;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::PluginInstallParams;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginReadResponse;
use codex_app_server_protocol::PluginUninstallParams;
use codex_app_server_protocol::PluginUninstallResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SendAddCreditsNudgeEmailParams;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::SkillsListResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadMemoryMode;
use codex_app_server_protocol::ThreadRollbackResponse;
use codex_app_server_protocol::ThreadStartSource;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnError as AppServerTurnError;
use codex_app_server_protocol::TurnStatus;
use codex_config::ConfigLayerStackOrdering;
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
use crossterm::event::KeyModifiers;
use ratatui::backend::Backend;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::Write;
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
mod background_requests;
mod config_persistence;
mod event_dispatch;
mod history_ui;
mod input;
mod loaded_threads;
mod pending_interactive_replay;
mod platform_actions;
mod replay_filter;
mod session_lifecycle;
mod side;
mod startup_prompts;
mod thread_events;
mod thread_routing;
mod thread_session_state;

use self::agent_navigation::AgentNavigationDirection;
use self::agent_navigation::AgentNavigationState;
use self::app_server_requests::PendingAppServerRequests;
use self::loaded_threads::find_loaded_subagent_threads_for_primary;
use self::pending_interactive_replay::PendingInteractiveReplayState;
use self::platform_actions::*;
use self::side::SideParentStatus;
use self::side::SideParentStatusChange;
use self::side::SideThreadState;
use self::startup_prompts::*;
use self::thread_events::*;

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
    additional_permissions: Option<&codex_protocol::models::AdditionalPermissionProfile>,
) -> Vec<codex_protocol::protocol::ReviewDecision> {
    ExecApprovalRequestEvent::default_available_decisions(
        network_approval_context,
        proposed_execpolicy_amendment,
        proposed_network_policy_amendments,
        additional_permissions,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AutoReviewMode {
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    sandbox_policy: SandboxPolicy,
}

/// Enabling the Auto-review experiment in the TUI should also switch the
/// current `/approvals` settings to the matching Auto-review mode. Users
/// can still change `/approvals` afterward; this just assumes that opting into
/// the experiment means they want Auto-review enabled immediately.
fn auto_review_mode() -> AutoReviewMode {
    AutoReviewMode {
        approval_policy: AskForApproval::OnRequest,
        approvals_reviewer: ApprovalsReviewer::AutoReview,
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
    rollout_path: Option<&Path>,
) -> Option<SessionSummary> {
    let usage_line = (!token_usage.is_zero()).then(|| FinalOutput::from(token_usage).to_string());
    let thread_id =
        resumable_thread(thread_id, thread_name, rollout_path).map(|thread| thread.thread_id);
    let resume_command =
        crate::legacy_core::util::resume_command(/*thread_name*/ None, thread_id);

    if usage_line.is_none() && resume_command.is_none() {
        return None;
    }

    Some(SessionSummary {
        usage_line,
        resume_command,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResumableThread {
    thread_id: ThreadId,
    thread_name: Option<String>,
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

fn rollout_path_is_resumable(rollout_path: &Path) -> bool {
    std::fs::metadata(rollout_path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionSummary {
    usage_line: Option<String>,
    resume_command: Option<String>,
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
    pub(crate) keymap: RuntimeKeymap,

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
    side_threads: HashMap<ThreadId, SideThreadState>,
    active_thread_id: Option<ThreadId>,
    active_thread_rx: Option<mpsc::Receiver<ThreadBufferedEvent>>,
    primary_thread_id: Option<ThreadId>,
    last_subagent_backfill_attempt: Option<ThreadId>,
    primary_session_configured: Option<ThreadSessionState>,
    pending_primary_events: VecDeque<ThreadBufferedEvent>,
    pending_app_server_requests: PendingAppServerRequests,
    // Serialize plugin enablement writes per plugin so stale completions cannot
    // overwrite a newer toggle, even if the plugin is toggled from different
    // cwd contexts.
    pending_plugin_enabled_writes: HashMap<String, Option<bool>>,
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
        initial_user_message: Option<crate::chatwidget::UserMessage>,
    ) -> crate::chatwidget::ChatWidgetInit {
        crate::chatwidget::ChatWidgetInit {
            config: cfg,
            frame_requester: tui.frame_requester(),
            app_event_tx: self.app_event_tx.clone(),
            initial_user_message,
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
        entered_trust_nux: bool,
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
        let external_agent_config_migration_outcome =
            handle_external_agent_config_migration_prompt_if_needed(
                tui,
                &mut app_server,
                &mut config,
                &cli_kv_overrides,
                &harness_overrides,
                entered_trust_nux,
            )
            .await?;
        let external_agent_config_migration_message = match external_agent_config_migration_outcome
        {
            ExternalAgentConfigMigrationStartupOutcome::Continue { success_message } => {
                success_message
            }
            ExternalAgentConfigMigrationStartupOutcome::ExitRequested => {
                app_server
                    .shutdown()
                    .await
                    .inspect_err(|err| {
                        tracing::warn!("app-server shutdown failed: {err}");
                    })
                    .ok();
                return Ok(AppExitInfo {
                    token_usage: TokenUsage::default(),
                    thread_id: None,
                    thread_name: None,
                    update_action: None,
                    exit_reason: ExitReason::UserRequested,
                });
            }
        };
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
        if let Some(message) = external_agent_config_migration_message {
            chat_widget.add_info_message(message, /*hint*/ None);
        }

        chat_widget
            .maybe_prompt_windows_sandbox_enable(should_prompt_windows_sandbox_nux_at_startup);

        let file_search = FileSearchManager::new(config.cwd.to_path_buf(), app_event_tx.clone());
        let runtime_keymap = RuntimeKeymap::from_config(&config.tui_keymap).map_err(|err| {
            color_eyre::eyre::eyre!(
                "Invalid `tui.keymap` configuration: {err}\n\
Fix the config and retry.\n\
See the Codex keymap documentation for supported actions and examples."
            )
        })?;
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
            keymap: runtime_keymap,
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
            side_threads: HashMap::new(),
            active_thread_id: None,
            active_thread_rx: None,
            primary_thread_id: None,
            last_subagent_backfill_attempt: None,
            primary_session_configured: None,
            pending_primary_events: VecDeque::new(),
            pending_app_server_requests: PendingAppServerRequests::default(),
            pending_plugin_enabled_writes: HashMap::new(),
        };
        if let Some(started) = initial_started_thread {
            app.enqueue_primary_thread_session(started.session, started.turns)
                .await?;
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
        app.refresh_startup_skills(&app_server);
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
        let resumable_thread = resumable_thread(
            app.chat_widget.thread_id(),
            app.chat_widget.thread_name(),
            app.chat_widget.rollout_path().as_deref(),
        );
        Ok(AppExitInfo {
            token_usage: app.token_usage(),
            thread_id: resumable_thread.as_ref().map(|thread| thread.thread_id),
            thread_name: resumable_thread.and_then(|thread| thread.thread_name),
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
