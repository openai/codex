//! The main Codex TUI chat surface.
//!
//! `ChatWidget` consumes protocol events, builds and updates history cells, and drives rendering
//! for both the main viewport and overlay UIs.
//!
//! The UI has both committed transcript cells (finalized `HistoryCell`s) and an in-flight active
//! cell (`ChatWidget.active_cell`) that can mutate in place while streaming (often representing a
//! coalesced exec/tool group). The transcript overlay (`Ctrl+T`) renders committed cells plus a
//! cached, render-only live tail derived from the current active cell so in-flight tool calls are
//! visible immediately.
//!
//! The transcript overlay is kept in sync by `App::overlay_forward_event`, which syncs a live tail
//! during draws using `active_cell_transcript_key()` and `active_cell_transcript_lines()`. The
//! cache key is designed to change when the active cell mutates in place or when its transcript
//! output is time-dependent so the overlay can refresh its cached tail without rebuilding it on
//! every draw.
//!
//! The bottom pane exposes a single "task running" indicator that drives the spinner and interrupt
//! hints. This module treats that indicator as derived UI-busy state: it is set while an agent turn
//! is in progress and while MCP server startup is in progress. Those lifecycles are tracked
//! independently (`agent_turn_running` and `mcp_startup_status`) and synchronized via
//! `update_task_running_state`.
//!
//! For preamble-capable models, assistant output may include commentary before
//! the final answer. During streaming we hide the status row to avoid duplicate
//! progress indicators; once commentary completes and stream queues drain, we
//! re-show it so users still see turn-in-progress state between output bursts.
//!
//! Slash-command parsing lives in the bottom-pane composer, but slash-command acceptance lives
//! here. That split lets the composer stage a recall entry before clearing input while this module
//! records the attempted slash command after dispatch just like ordinary submitted text.
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crate::app::app_server_requests::ResolvedAppServerRequest;
use crate::app_command::AppCommand;
use crate::app_event::HistoryLookupResponse;
use crate::app_event::RealtimeAudioDeviceKind;
use crate::app_server_approval_conversions::file_update_changes_to_display;
use crate::approval_events::ApplyPatchApprovalRequestEvent;
use crate::approval_events::ExecApprovalRequestEvent;
#[cfg(not(target_os = "linux"))]
use crate::audio_device::list_realtime_audio_device_names;
use crate::bottom_pane::StatusLineItem;
use crate::bottom_pane::StatusLineSetupView;
use crate::bottom_pane::StatusSurfacePreviewData;
use crate::bottom_pane::StatusSurfacePreviewItem;
use crate::bottom_pane::TerminalTitleItem;
use crate::bottom_pane::TerminalTitleSetupView;
use crate::diff_model::FileChange;
use crate::git_action_directives::parse_assistant_markdown;
use crate::legacy_core::DEFAULT_AGENTS_MD_FILENAME;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::Constrained;
use crate::legacy_core::config::ConstraintResult;
#[cfg(target_os = "windows")]
use crate::legacy_core::windows_sandbox::WindowsSandboxLevelExt;
use crate::mention_codec::LinkedMention;
use crate::mention_codec::encode_history_mentions;
use crate::model_catalog::ModelCatalog;
use crate::multi_agents;
use crate::multi_agents::AgentMetadata;
use crate::session_state::SessionNetworkProxyRuntime;
use crate::session_state::ThreadSessionState;
use crate::status::RateLimitWindowDisplay;
use crate::status::StatusAccountDisplay;
use crate::status::StatusHistoryHandle;
use crate::status::format_directory_display;
use crate::status::format_tokens_compact;
use crate::status::rate_limit_snapshot_display_for_limit;
use crate::terminal_title::SetTerminalTitleResult;
use crate::terminal_title::clear_terminal_title;
use crate::terminal_title::set_terminal_title;
use crate::text_formatting::proper_join;
use crate::token_usage::TokenUsage;
use crate::token_usage::TokenUsageInfo;
use crate::version::CODEX_CLI_VERSION;
use codex_app_server_protocol::AddCreditsNudgeCreditType;
use codex_app_server_protocol::AddCreditsNudgeEmailStatus;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppSummary;
use codex_app_server_protocol::CodexErrorInfo as AppServerCodexErrorInfo;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
use codex_app_server_protocol::CommandExecutionSource as ExecCommandSource;
use codex_app_server_protocol::ConfigLayerSource;
use codex_app_server_protocol::CreditsSnapshot;
use codex_app_server_protocol::ErrorNotification;
use codex_app_server_protocol::FileChangeRequestApprovalParams;
use codex_app_server_protocol::GuardianApprovalReviewAction;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::ModelVerification as AppServerModelVerification;
use codex_app_server_protocol::RateLimitReachedType;
use codex_app_server_protocol::RateLimitSnapshot;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ReviewTarget;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SkillMetadata as ProtocolSkillMetadata;
use codex_app_server_protocol::SkillsListResponse;
use codex_app_server_protocol::ThreadGoal as AppThreadGoal;
use codex_app_server_protocol::ThreadGoalStatus as AppThreadGoalStatus;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnPlanStepStatus;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_chatgpt::connectors as chatgpt_connectors;
use codex_config::ConfigLayerStackOrdering;
use codex_config::types::ApprovalsReviewer;
use codex_config::types::Notifications;
use codex_config::types::WindowsSandboxModeToml;
use codex_core_skills::model::SkillMetadata;
use codex_exec_server::EnvironmentManager;
use codex_features::FEATURES;
use codex_features::Feature;
#[cfg(test)]
use codex_git_utils::CommitLogEntry;
use codex_git_utils::current_branch_name;
use codex_git_utils::get_git_repo_root;
use codex_git_utils::local_git_branches;
use codex_git_utils::recent_commits;
use codex_otel::RuntimeMetricsSummary;
use codex_otel::SessionTelemetry;
use codex_plugin::PluginCapabilitySummary;
use codex_protocol::ThreadId;
use codex_protocol::account::PlanType;
use codex_protocol::approvals::GuardianAssessmentAction;
use codex_protocol::approvals::GuardianAssessmentDecisionSource;
use codex_protocol::approvals::GuardianAssessmentEvent;
use codex_protocol::approvals::GuardianAssessmentStatus;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::Settings;
#[cfg(target_os = "windows")]
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::plan_tool::PlanItemArg as UpdatePlanItemArg;
use codex_protocol::plan_tool::StepStatus as UpdatePlanItemStatus;
use codex_protocol::request_permissions::RequestPermissionsEvent;
use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;
use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use codex_utils_absolute_path::AbsolutePathBuf;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use rand::Rng;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;
use tracing::warn;

const DEFAULT_MODEL_DISPLAY_NAME: &str = "loading";
const MULTI_AGENT_ENABLE_TITLE: &str = "Enable subagents?";
const MULTI_AGENT_ENABLE_YES: &str = "Yes, enable";
const MULTI_AGENT_ENABLE_NO: &str = "Not now";
const MULTI_AGENT_ENABLE_NOTICE: &str = "Subagents will be enabled in the next session.";
const TRUSTED_ACCESS_FOR_CYBER_VERIFICATION_WARNING: &str = "Your conversations have multiple flags for possible cybersecurity risk. Responses may take longer because extra safety checks are on. To get authorized for security work, join the Trusted Access for Cyber program: https://chatgpt.com/cyber";
const MEMORIES_DOC_URL: &str = "https://developers.openai.com/codex/memories";
const MEMORIES_ENABLE_TITLE: &str = "Enable memories?";
const MEMORIES_ENABLE_YES: &str = "Yes, enable";
const MEMORIES_ENABLE_NO: &str = "Not now";
const MEMORIES_ENABLE_NOTICE: &str = "Memories will be enabled in the next session.";
const PLAN_MODE_REASONING_SCOPE_TITLE: &str = "Apply reasoning change";
const PLAN_MODE_REASONING_SCOPE_PLAN_ONLY: &str = "Apply to Plan mode override";
const PLAN_MODE_REASONING_SCOPE_ALL_MODES: &str = "Apply to global default and Plan mode override";
const CONNECTORS_SELECTION_VIEW_ID: &str = "connectors-selection";
const PET_SELECTION_LOADING_VIEW_ID: &str = "pet-selection-loading";
const AMBIENT_PET_WRAP_GAP_COLUMNS: u16 = 2;
const TUI_STUB_MESSAGE: &str = "Not available in TUI yet.";

/// Choose the keybinding used to edit the most-recently queued message.
///
/// Apple Terminal, Warp, and VSCode integrated terminals intercept or silently
/// swallow Alt+Up, and tmux does not reliably pass that chord through. We fall
/// back to Shift+Left for those environments while keeping the more discoverable
/// Alt+Up everywhere else.
///
/// The match is exhaustive so that adding a new `TerminalName` variant forces
/// an explicit decision about which binding that terminal should use.
fn queued_message_edit_binding_for_terminal(terminal_info: TerminalInfo) -> KeyBinding {
    if matches!(
        terminal_info.multiplexer.as_ref(),
        Some(Multiplexer::Tmux { .. })
    ) {
        return key_hint::shift(KeyCode::Left);
    }

    match terminal_info.name {
        TerminalName::AppleTerminal | TerminalName::WarpTerminal | TerminalName::VsCode => {
            key_hint::shift(KeyCode::Left)
        }
        TerminalName::Ghostty
        | TerminalName::Iterm2
        | TerminalName::WezTerm
        | TerminalName::Kitty
        | TerminalName::Alacritty
        | TerminalName::Konsole
        | TerminalName::GnomeTerminal
        | TerminalName::Vte
        | TerminalName::WindowsTerminal
        | TerminalName::Dumb
        | TerminalName::Unknown => key_hint::alt(KeyCode::Up),
    }
}

fn queued_message_edit_hint_binding(
    bindings: &[KeyBinding],
    terminal_info: TerminalInfo,
) -> Option<KeyBinding> {
    let terminal_binding = queued_message_edit_binding_for_terminal(terminal_info);
    bindings
        .contains(&terminal_binding)
        .then_some(terminal_binding)
        .or_else(|| bindings.first().copied())
}

use crate::app_event::AppEvent;
use crate::app_event::ExitMode;
use crate::app_event::RateLimitRefreshOrigin;
#[cfg(target_os = "windows")]
use crate::app_event::WindowsSandboxEnableMode;
use crate::app_event_sender::AppEventSender;
use crate::auto_review_denials;
use crate::auto_review_denials::RecentAutoReviewDenials;
use crate::bottom_pane::ApprovalRequest;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BottomPaneParams;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::CollaborationModeIndicator;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::DOUBLE_PRESS_QUIT_SHORTCUT_ENABLED;
use crate::bottom_pane::ExperimentalFeatureItem;
use crate::bottom_pane::ExperimentalFeaturesView;
use crate::bottom_pane::GoalStatusIndicator;
use crate::bottom_pane::InputResult;
use crate::bottom_pane::LocalImageAttachment;
use crate::bottom_pane::McpServerElicitationFormRequest;
use crate::bottom_pane::MemoriesSettingsView;
use crate::bottom_pane::MentionBinding;
use crate::bottom_pane::QUIT_SHORTCUT_TIMEOUT;
use crate::bottom_pane::QueuedInputAction;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::custom_prompt_view::CustomPromptView;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::clipboard_paste::paste_image_to_temp_png;
use crate::collaboration_modes;
use crate::diff_render::display_path_for;
use crate::exec_cell::CommandOutput;
use crate::exec_cell::ExecCell;
use crate::exec_cell::new_active_exec_command;
use crate::exec_command::split_command_string;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::get_git_diff::get_git_diff;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use crate::history_cell::HistoryRenderMode;
use crate::history_cell::HookCell;
use crate::history_cell::McpInvocation;
use crate::history_cell::McpToolCallCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::WebSearchCell;
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::ChatKeymap;
use crate::keymap::RuntimeKeymap;
use crate::render::Insets;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::FlexRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt;
use crate::render::renderable::RenderableItem;
use crate::slash_command::SlashCommand;
use crate::status::RateLimitSnapshotDisplay;
use crate::status_indicator_widget::STATUS_DETAILS_DEFAULT_MAX_LINES;
use crate::status_indicator_widget::StatusDetailsCapitalization;
use crate::text_formatting::truncate_text;
use crate::tui::FrameRequester;
mod command_lifecycle;
mod connectors;
mod constructor;
use self::connectors::ConnectorsState;
mod exec_state;
use self::exec_state::RunningCommand;
use self::exec_state::UnifiedExecProcessSummary;
use self::exec_state::UnifiedExecWaitState;
use self::exec_state::UnifiedExecWaitStreak;
use self::exec_state::command_execution_command_and_parsed;
use self::exec_state::is_standard_tool_call;
use self::exec_state::is_unified_exec_source;
mod goal_status;
use self::goal_status::GoalStatusState;
#[cfg(test)]
use self::goal_status::goal_status_indicator_from_app_goal;
mod goal_menu;
mod goal_validation;
mod ide_context;
use self::ide_context::IdeContextState;
mod input_queue;
use self::input_queue::InputQueueState;
mod input_flow;
mod input_restore;
mod input_submission;
mod interrupts;
use self::interrupts::InterruptManager;
mod keymap_picker;
mod mcp_startup;
use self::mcp_startup::McpStartupStatus;
mod pets;
mod session_flow;
mod session_header;
use self::session_header::SessionHeader;
mod hook_lifecycle;
mod hooks;
mod interaction;
mod skills;
mod slash_dispatch;
use self::skills::collect_tool_mentions;
use self::skills::find_app_mentions;
use self::skills::find_skill_mentions_with_tool_mentions;
mod plugins;
use self::plugins::PluginInstallAuthFlowState;
use self::plugins::PluginListFetchState;
use self::plugins::PluginsCacheState;
mod plan_implementation;
use self::plan_implementation::PLAN_IMPLEMENTATION_TITLE;
mod model_popups;
mod notifications;
use self::notifications::Notification;
mod permission_popups;
mod protocol;
mod protocol_requests;
mod rate_limits;
use self::rate_limits::RateLimitErrorKind;
use self::rate_limits::RateLimitSwitchPromptState;
use self::rate_limits::RateLimitWarningState;
use self::rate_limits::app_server_rate_limit_error_kind;
pub(crate) use self::rate_limits::get_limits_duration;
use self::rate_limits::is_app_server_cyber_policy_error;
mod realtime;
mod rendering;
mod replay;
use self::realtime::RealtimeConversationUiState;
mod reasoning_shortcuts;
mod review;
mod review_popups;
use self::review::ReviewState;
#[cfg(test)]
pub(crate) use self::review_popups::show_review_commit_picker_with_entries;
mod service_tiers;
mod settings;
mod settings_popups;
mod side;
mod status_state;
mod windows_sandbox_prompts;
use self::status_state::StatusIndicatorState;
use self::status_state::StatusState;
use self::status_state::TerminalTitleStatusKind;
mod status_controls;
mod status_surfaces;
mod streaming;
use self::status_surfaces::CachedProjectRootName;
mod tool_lifecycle;
mod tool_requests;
mod transcript;
use self::transcript::TranscriptState;
mod turn_lifecycle;
mod turn_runtime;
use self::turn_lifecycle::TurnLifecycleState;
mod user_messages;
use self::user_messages::PendingSteer;
use self::user_messages::PendingSteerCompareKey;
use self::user_messages::QueueDrain;
use self::user_messages::QueuedUserMessage;
use self::user_messages::ShellEscapePolicy;
use self::user_messages::ThreadComposerState;
pub(crate) use self::user_messages::ThreadInputState;
pub(crate) use self::user_messages::UserMessage;
use self::user_messages::UserMessageDisplay;
#[cfg(test)]
use self::user_messages::UserMessageHistoryOverride;
use self::user_messages::UserMessageHistoryRecord;
use self::user_messages::app_server_text_elements;
pub(crate) use self::user_messages::create_initial_user_message;
use self::user_messages::merge_user_messages;
use self::user_messages::merge_user_messages_with_history_record;
#[cfg(test)]
use self::user_messages::remap_placeholders_for_message;
use self::user_messages::user_message_display_for_history;
use self::user_messages::user_message_for_restore;
use self::user_messages::user_message_preview_text;
mod warnings;
use self::warnings::WarningDisplayState;
pub(crate) use crate::branch_summary::StatusLineGitSummary;
use crate::streaming::chunking::AdaptiveChunkingPolicy;
use crate::streaming::commit_tick::CommitTickScope;
use crate::streaming::commit_tick::run_commit_tick;
use crate::streaming::controller::PlanStreamController;
use crate::streaming::controller::StreamController;
use crate::workspace_command::WorkspaceCommandRunner;

use chrono::Local;
use codex_app_server_protocol::AskForApproval;
use codex_file_search::FileMatch;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::plan_tool::UpdatePlanArgs;
use codex_utils_approval_presets::ApprovalPreset;
use codex_utils_approval_presets::builtin_approval_presets;
use strum::IntoEnumIterator;
use unicode_segmentation::UnicodeSegmentation;

const USER_SHELL_COMMAND_HELP_TITLE: &str = "Prefix a command with ! to run it locally";
const USER_SHELL_COMMAND_HELP_HINT: &str = "Example: !ls";
const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_STATUS_LINE_ITEMS: [&str; 2] = ["model-with-reasoning", "current-dir"];
const MAX_AGENT_COPY_HISTORY: usize = 32;

/// Common initialization parameters shared by all `ChatWidget` constructors.
pub(crate) struct ChatWidgetInit {
    pub(crate) config: Config,
    pub(crate) environment_manager: Arc<EnvironmentManager>,
    pub(crate) frame_requester: FrameRequester,
    pub(crate) app_event_tx: AppEventSender,
    /// App-server-backed runner used by status surfaces for workspace metadata probes.
    ///
    /// Tests that do not exercise git status-line refreshes may leave this unset. Production TUI
    /// construction provides a runner for the active app-server session.
    pub(crate) workspace_command_runner: Option<WorkspaceCommandRunner>,
    pub(crate) initial_user_message: Option<UserMessage>,
    pub(crate) enhanced_keys_supported: bool,
    pub(crate) has_chatgpt_account: bool,
    pub(crate) model_catalog: Arc<ModelCatalog>,
    pub(crate) feedback: codex_feedback::CodexFeedback,
    pub(crate) is_first_run: bool,
    pub(crate) status_account_display: Option<StatusAccountDisplay>,
    pub(crate) runtime_model_provider_base_url: Option<String>,
    pub(crate) initial_plan_type: Option<PlanType>,
    pub(crate) model: Option<String>,
    pub(crate) startup_tooltip_override: Option<String>,
    // Shared latch so we only warn once about invalid status-line item IDs.
    pub(crate) status_line_invalid_items_warned: Arc<AtomicBool>,
    // Shared latch so we only warn once about invalid terminal-title item IDs.
    pub(crate) terminal_title_invalid_items_warned: Arc<AtomicBool>,
    pub(crate) session_telemetry: SessionTelemetry,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ExternalEditorState {
    #[default]
    Closed,
    Requested,
    Active,
}

/// Maintains the per-session UI state and interaction state machines for the chat screen.
///
/// `ChatWidget` owns the state derived from the protocol event stream (history cells, streaming
/// buffers, bottom-pane overlays, and transient status text) and turns key presses into user
/// intent (`Op` submissions and `AppEvent` requests).
///
/// It is not responsible for running the agent itself; it reflects progress by updating UI state
/// and by sending requests back to codex-core.
///
/// Quit/interrupt behavior intentionally spans layers: the bottom pane owns local input routing
/// (which view gets Ctrl+C), while `ChatWidget` owns process-level decisions such as interrupting
/// active work, arming the double-press quit shortcut, and requesting shutdown-first exit.
pub(crate) struct ChatWidget {
    app_event_tx: AppEventSender,
    codex_op_target: CodexOpTarget,
    bottom_pane: BottomPane,
    transcript: TranscriptState,
    config: Config,
    environment_manager: Arc<EnvironmentManager>,
    raw_output_mode: bool,
    /// Runtime value resolved by core. `config.service_tier` remains the explicit user choice.
    effective_service_tier: Option<String>,
    /// The unmasked collaboration mode settings (always Default mode).
    ///
    /// Masks are applied on top of this base mode to derive the effective mode.
    current_collaboration_mode: CollaborationMode,
    /// The currently active collaboration mask, if any.
    active_collaboration_mask: Option<CollaborationModeMask>,
    has_chatgpt_account: bool,
    model_catalog: Arc<ModelCatalog>,
    session_telemetry: SessionTelemetry,
    session_header: SessionHeader,
    initial_user_message: Option<UserMessage>,
    status_account_display: Option<StatusAccountDisplay>,
    runtime_model_provider_base_url: Option<String>,
    token_info: Option<TokenUsageInfo>,
    rate_limit_snapshots_by_limit_id: BTreeMap<String, RateLimitSnapshotDisplay>,
    refreshing_status_outputs: Vec<(u64, StatusHistoryHandle)>,
    next_status_refresh_request_id: u64,
    plan_type: Option<PlanType>,
    codex_rate_limit_reached_type: Option<RateLimitReachedType>,
    rate_limit_warnings: RateLimitWarningState,
    warning_display_state: WarningDisplayState,
    rate_limit_switch_prompt: RateLimitSwitchPromptState,
    add_credits_nudge_email_in_flight: Option<AddCreditsNudgeCreditType>,
    adaptive_chunking: AdaptiveChunkingPolicy,
    // Stream lifecycle controller
    stream_controller: Option<StreamController>,
    // Stream lifecycle controller for proposed plan output.
    plan_stream_controller: Option<PlanStreamController>,
    /// Holds the platform clipboard lease so copied text remains available while supported.
    clipboard_lease: Option<crate::clipboard_copy::ClipboardLease>,
    copy_last_response_binding: Vec<KeyBinding>,
    running_commands: HashMap<String, RunningCommand>,
    collab_agent_metadata: HashMap<ThreadId, AgentMetadata>,
    pending_collab_spawn_requests: HashMap<String, multi_agents::SpawnRequestSummary>,
    suppressed_exec_calls: HashSet<String>,
    skills_all: Vec<ProtocolSkillMetadata>,
    skills_initial_state: Option<HashMap<AbsolutePathBuf, bool>>,
    last_unified_wait: Option<UnifiedExecWaitState>,
    unified_exec_wait_streak: Option<UnifiedExecWaitStreak>,
    turn_lifecycle: TurnLifecycleState,
    task_complete_pending: bool,
    unified_exec_processes: Vec<UnifiedExecProcessSummary>,
    /// Tracks per-server MCP startup state while startup is in progress.
    ///
    /// The map is `Some(_)` from the first startup status update until the
    /// app-server-backed startup round settles, and the bottom pane is treated
    /// as "running" while this is populated, even if no agent turn is currently
    /// executing.
    mcp_startup_status: Option<HashMap<String, McpStartupStatus>>,
    /// Expected MCP servers for the current startup round, seeded from enabled local config.
    mcp_startup_expected_servers: Option<HashSet<String>>,
    /// After startup settles, ignore stale updates until enough notifications confirm a new round.
    mcp_startup_ignore_updates_until_next_start: bool,
    /// A lag signal for the next round means terminal-only updates are enough to settle it.
    mcp_startup_allow_terminal_only_next_round: bool,
    /// Buffers post-settle MCP startup updates until they cover a full fresh round.
    mcp_startup_pending_next_round: HashMap<String, McpStartupStatus>,
    /// Tracks whether the buffered next round has seen any `Starting` update yet.
    mcp_startup_pending_next_round_saw_starting: bool,
    connectors: ConnectorsState,
    ide_context: IdeContextState,
    plugins_cache: PluginsCacheState,
    plugins_fetch_state: PluginListFetchState,
    plugin_install_apps_needing_auth: Vec<AppSummary>,
    plugin_install_auth_flow: Option<PluginInstallAuthFlowState>,
    plugins_active_tab_id: Option<String>,
    newly_installed_marketplace_tab_id: Option<String>,
    // Queue of interruptive UI events deferred during an active write cycle
    interrupts: InterruptManager,
    // Accumulates the current reasoning block text to extract a header
    reasoning_buffer: String,
    // Accumulates full reasoning content for transcript-only recording
    full_reasoning_buffer: String,
    status_state: StatusState,
    review: ReviewState,
    // Active hook runs render in a dedicated live cell so they can run alongside tools.
    active_hook_cell: Option<HookCell>,
    // Ambient companion rendered over the transcript area, never inside the footer rows.
    ambient_pet: Option<crate::pets::AmbientPet>,
    pet_picker_preview_state: crate::pets::PetPickerPreviewState,
    pet_picker_preview_pet: Option<crate::pets::AmbientPet>,
    pet_picker_preview_request_id: u64,
    pet_picker_preview_image_visible: std::cell::Cell<bool>,
    pet_selection_load_request_id: u64,
    #[cfg(test)]
    pet_image_support_override: Option<crate::pets::PetImageSupport>,
    thread_id: Option<ThreadId>,
    /// Nudge dismissals that should survive draft edits within the current thread scope.
    ///
    /// The nudge is only a discovery aid, so once a user dismisses it or enters Plan mode we keep it
    /// hidden for that thread instead of resurfacing it on every matching draft.
    dismissed_plan_mode_nudge_scopes: HashSet<PlanModeNudgeScope>,
    thread_name: Option<String>,
    thread_rename_block_message: Option<String>,
    active_side_conversation: bool,
    normal_placeholder_text: String,
    side_placeholder_text: String,
    forked_from: Option<ThreadId>,
    interrupted_turn_notice_mode: InterruptedTurnNoticeMode,
    frame_requester: FrameRequester,
    // Whether to include the initial welcome banner on session configured
    show_welcome_banner: bool,
    // One-shot tooltip override for the primary startup session.
    startup_tooltip_override: Option<String>,
    // When resuming an existing session (selected via resume picker), avoid an
    // immediate redraw on SessionConfigured to prevent a gratuitous UI flicker.
    suppress_session_configured_redraw: bool,
    // During snapshot restore, defer startup prompt submission until replayed
    // history has been rendered so resumed/forked prompts keep chronological
    // order.
    suppress_initial_user_message_submit: bool,
    input_queue: InputQueueState,
    /// Main chat-surface bindings resolved from `tui.keymap.chat`.
    chat_keymap: ChatKeymap,
    /// Keybinding to show for popping the most-recently queued message back
    /// into the composer. This may differ from the first configured binding
    /// when the default set includes a terminal-specific fallback.
    queued_message_edit_hint_binding: Option<KeyBinding>,
    // Pending notification to show when unfocused on next Draw
    pending_notification: Option<Notification>,
    /// When `Some`, the user has pressed a quit shortcut and the second press
    /// must occur before `quit_shortcut_expires_at`.
    quit_shortcut_expires_at: Option<Instant>,
    /// Tracks which quit shortcut key was pressed first.
    ///
    /// We require the second press to match this key so `Ctrl+C` followed by
    /// `Ctrl+D` (or vice versa) doesn't quit accidentally.
    quit_shortcut_key: Option<KeyBinding>,
    // Runtime metrics accumulated across delta snapshots for the active turn.
    turn_runtime_metrics: RuntimeMetricsSummary,
    last_rendered_width: std::cell::Cell<Option<usize>>,
    // Feedback sink for /feedback
    feedback: codex_feedback::CodexFeedback,
    // Current session rollout path (if known)
    current_rollout_path: Option<PathBuf>,
    // Current working directory (if known)
    current_cwd: Option<PathBuf>,
    // App-server-backed command runner for status-line workspace metadata lookups.
    workspace_command_runner: Option<WorkspaceCommandRunner>,
    // Instruction source files loaded for the current session, supplied by app-server.
    instruction_source_paths: Vec<AbsolutePathBuf>,
    // Runtime network proxy bind addresses from SessionConfigured.
    session_network_proxy: Option<SessionNetworkProxyRuntime>,
    // Shared latch so we only warn once about invalid status-line item IDs.
    status_line_invalid_items_warned: Arc<AtomicBool>,
    // Shared latch so we only warn once about invalid terminal-title item IDs.
    terminal_title_invalid_items_warned: Arc<AtomicBool>,
    // Last terminal title emitted, to avoid writing duplicate OSC updates.
    pub(crate) last_terminal_title: Option<String>,
    // Last visible "action required" state observed by the terminal-title renderer.
    last_terminal_title_requires_action: bool,
    // Original terminal-title config captured when the setup UI opens.
    //
    // The outer `Option` tracks whether a setup session is active (`Some`)
    // or not (`None`). The inner `Option<Vec<String>>` mirrors the shape
    // of `config.tui_terminal_title` (which is `None` when using defaults).
    // On cancel or persist-failure the inner value is restored to config;
    // on confirm the outer is set to `None` to end the session.
    terminal_title_setup_original_items: Option<Option<Vec<String>>>,
    // Baseline instant used to animate spinner-prefixed title statuses.
    terminal_title_animation_origin: Instant,
    // Cached project-root display name keyed by cwd for status/title rendering.
    status_line_project_root_name_cache: Option<CachedProjectRootName>,
    // Cached git branch name for the status line (None if unknown).
    status_line_branch: Option<String>,
    // CWD used to resolve the cached branch; change resets branch state.
    status_line_branch_cwd: Option<PathBuf>,
    // True while an async branch lookup is in flight.
    status_line_branch_pending: bool,
    // True once we've attempted a branch lookup for the current CWD.
    status_line_branch_lookup_complete: bool,
    // Cached PR and branch-change summary for the active status-line cwd.
    status_line_git_summary: Option<StatusLineGitSummary>,
    // CWD used to resolve the cached Git summary; change resets summary state.
    status_line_git_summary_cwd: Option<PathBuf>,
    // True while an async Git summary lookup is in flight.
    status_line_git_summary_pending: bool,
    // True once we've attempted a Git summary lookup for the current CWD.
    status_line_git_summary_lookup_complete: bool,
    // Current thread-goal status shown in the status line when plan mode is inactive.
    current_goal_status_indicator: Option<GoalStatusIndicator>,
    current_goal_status: Option<GoalStatusState>,
    external_editor_state: ExternalEditorState,
    realtime_conversation: RealtimeConversationUiState,
    last_rendered_user_message_display: Option<UserMessageDisplay>,
    last_non_retry_error: Option<(String, String)>,
}

#[cfg_attr(not(test), allow(dead_code))]
enum CodexOpTarget {
    Direct(UnboundedSender<AppCommand>),
    AppEvent,
}

/// Snapshot of active-cell state that affects transcript overlay rendering.
///
/// The overlay keeps a cached "live tail" for the in-flight cell; this key lets
/// it cheaply decide when to recompute that tail as the active cell evolves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ActiveCellTranscriptKey {
    /// Cache-busting revision for in-place updates.
    ///
    /// Many active cells are updated incrementally while streaming (for example when exec groups
    /// add output or change status), and the transcript overlay caches its live tail, so this
    /// revision gives a cheap way to say "same active cell, but its transcript output is different
    /// now". Callers bump it on any mutation that can affect `HistoryCell::transcript_lines`.
    pub(crate) revision: u64,
    /// Whether the active cell continues the prior stream, which affects
    /// spacing between transcript blocks.
    pub(crate) is_stream_continuation: bool,
    /// Optional animation tick for time-dependent transcript output.
    ///
    /// When this changes, the overlay recomputes the cached tail even if the revision and width
    /// are unchanged, which is how shimmer/spinner visuals can animate in the overlay without any
    /// underlying data change.
    pub(crate) animation_tick: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum InterruptedTurnNoticeMode {
    #[default]
    Default,
    Suppress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplayKind {
    ResumeInitialMessages,
    ThreadSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionConfiguredDisplay {
    Normal,
    /// Apply session state without emitting the session info cell.
    Quiet,
    SideConversation,
}

/// Scope used to keep Plan-mode nudge dismissal local to one conversation context.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum PlanModeNudgeScope {
    /// Drafts entered before the server has assigned a thread id.
    NewThread,
    /// Drafts associated with one configured thread.
    Thread(ThreadId),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TurnAbortReason {
    Interrupted,
    BudgetLimited,
}

/// Returns whether `text` contains the standalone word `plan`.
///
/// This intentionally mirrors the App suggestion heuristic instead of trying to infer broader
/// planning intent from substrings such as `planning`. Slash and shell drafts still match here so
/// callers can keep lexical matching separate from presentation policy.
fn contains_plan_keyword(text: &str) -> bool {
    text.split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .any(|word| word.eq_ignore_ascii_case("plan"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThreadItemRenderSource {
    Live,
    Replay(ReplayKind),
}

impl ThreadItemRenderSource {
    fn is_replay(self) -> bool {
        matches!(self, Self::Replay(_))
    }

    fn replay_kind(self) -> Option<ReplayKind> {
        match self {
            Self::Live => None,
            Self::Replay(replay_kind) => Some(replay_kind),
        }
    }
}

fn exec_approval_request_from_params(
    params: CommandExecutionRequestApprovalParams,
    fallback_cwd: &AbsolutePathBuf,
) -> ExecApprovalRequestEvent {
    ExecApprovalRequestEvent {
        call_id: params.item_id,
        command: params
            .command
            .as_deref()
            .map(split_command_string)
            .unwrap_or_default(),
        cwd: params.cwd.unwrap_or_else(|| fallback_cwd.clone()),
        reason: params.reason,
        network_approval_context: params.network_approval_context,
        additional_permissions: params.additional_permissions,
        turn_id: params.turn_id,
        approval_id: params.approval_id,
        proposed_execpolicy_amendment: params.proposed_execpolicy_amendment,
        proposed_network_policy_amendments: params.proposed_network_policy_amendments,
        available_decisions: params.available_decisions,
    }
}

fn patch_approval_request_from_params(
    params: FileChangeRequestApprovalParams,
) -> ApplyPatchApprovalRequestEvent {
    ApplyPatchApprovalRequestEvent {
        call_id: params.item_id,
        turn_id: params.turn_id,
        changes: HashMap::new(),
        reason: params.reason,
        grant_root: params.grant_root,
    }
}

fn request_permissions_from_params(
    params: codex_app_server_protocol::PermissionsRequestApprovalParams,
) -> RequestPermissionsEvent {
    RequestPermissionsEvent {
        turn_id: params.turn_id,
        call_id: params.item_id,
        started_at_ms: params.started_at_ms,
        reason: params.reason,
        permissions: params.permissions.into(),
        cwd: Some(params.cwd),
    }
}

fn token_usage_info_from_app_server(token_usage: ThreadTokenUsage) -> TokenUsageInfo {
    TokenUsageInfo {
        total_token_usage: TokenUsage {
            total_tokens: token_usage.total.total_tokens,
            input_tokens: token_usage.total.input_tokens,
            cached_input_tokens: token_usage.total.cached_input_tokens,
            output_tokens: token_usage.total.output_tokens,
            reasoning_output_tokens: token_usage.total.reasoning_output_tokens,
        },
        last_token_usage: TokenUsage {
            total_tokens: token_usage.last.total_tokens,
            input_tokens: token_usage.last.input_tokens,
            cached_input_tokens: token_usage.last.cached_input_tokens,
            output_tokens: token_usage.last.output_tokens,
            reasoning_output_tokens: token_usage.last.reasoning_output_tokens,
        },
        model_context_window: token_usage.model_context_window,
    }
}

impl ChatWidget {
    /// Stores or overwrites the cached nickname and role for a collab agent thread.
    ///
    /// Called by `App::upsert_agent_picker_thread` and `App::replace_chat_widget` to keep the
    /// rendering metadata in sync with the navigation cache. Must be called before any
    /// notification referencing this thread is processed, otherwise the rendered item will fall
    /// back to showing the raw thread id.
    pub(crate) fn set_collab_agent_metadata(
        &mut self,
        thread_id: ThreadId,
        agent_nickname: Option<String>,
        agent_role: Option<String>,
    ) {
        self.collab_agent_metadata.insert(
            thread_id,
            AgentMetadata {
                agent_nickname,
                agent_role,
            },
        );
    }

    /// Returns the cached metadata for a thread, defaulting to empty if none has been registered.
    fn collab_agent_metadata(&self, thread_id: ThreadId) -> AgentMetadata {
        self.collab_agent_metadata
            .get(&thread_id)
            .cloned()
            .unwrap_or_default()
    }

    fn realtime_conversation_enabled(&self) -> bool {
        self.config.features.enabled(Feature::RealtimeConversation)
            && cfg!(not(target_os = "linux"))
    }

    fn realtime_audio_device_selection_enabled(&self) -> bool {
        self.realtime_conversation_enabled()
    }

    fn restore_retry_status_header_if_present(&mut self) {
        if let Some(header) = self.status_state.take_retry_status_header() {
            self.set_status_header(header);
        }
    }

    /// Record or update the raw markdown for the current agent turn.
    fn record_agent_markdown(&mut self, message: &str) {
        if !message.is_empty() {
            self.transcript.record_agent_markdown(message.to_string());
        }
    }

    fn record_visible_user_turn_for_copy(&mut self) {
        self.transcript.record_visible_user_turn();
    }

    pub(crate) fn open_feedback_note(
        &mut self,
        category: crate::app_event::FeedbackCategory,
        include_logs: bool,
    ) {
        self.show_feedback_note(category, include_logs);
    }

    fn show_feedback_note(
        &mut self,
        category: crate::app_event::FeedbackCategory,
        include_logs: bool,
    ) {
        let view = crate::bottom_pane::FeedbackNoteView::new(
            category,
            self.turn_lifecycle.last_turn_id.clone(),
            self.app_event_tx.clone(),
            include_logs,
        );
        self.bottom_pane.show_view(Box::new(view));
        self.request_redraw();
    }

    pub(crate) fn open_app_link_view(&mut self, params: crate::bottom_pane::AppLinkViewParams) {
        let view = crate::bottom_pane::AppLinkView::new_with_keymap(
            params,
            self.app_event_tx.clone(),
            self.bottom_pane.list_keymap(),
        );
        self.bottom_pane.show_view(Box::new(view));
        self.request_redraw();
    }

    pub(crate) fn dismiss_app_server_request(&mut self, request: &ResolvedAppServerRequest) {
        // A remotely resolved request must not remain user-actionable. It may be
        // materialized in the bottom pane or still deferred behind active streaming.
        let removed_deferred = self.interrupts.remove_resolved_prompt(request);
        let removed_visible = self.bottom_pane.dismiss_app_server_request(request);
        if removed_deferred || removed_visible {
            self.request_redraw();
        }
    }

    pub(crate) fn open_feedback_consent(&mut self, category: crate::app_event::FeedbackCategory) {
        let snapshot = self.feedback.snapshot(self.thread_id);
        let params = crate::bottom_pane::feedback_upload_consent_params(
            self.app_event_tx.clone(),
            category,
            self.current_rollout_path.clone(),
            self.thread_id
                .map(|thread_id| format!("auto-review-rollout-{thread_id}.jsonl")),
            snapshot.feedback_diagnostics(),
        );
        self.bottom_pane.show_selection_view(params);
        self.request_redraw();
    }

    pub(crate) fn open_multi_agent_enable_prompt(&mut self) {
        let items = vec![
            SelectionItem {
                name: MULTI_AGENT_ENABLE_YES.to_string(),
                description: Some(
                    "Save the setting now. You will need a new session to use it.".to_string(),
                ),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::UpdateFeatureFlags {
                        updates: vec![(Feature::Collab, true)],
                    });
                    tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_warning_event(MULTI_AGENT_ENABLE_NOTICE.to_string()),
                    )));
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: MULTI_AGENT_ENABLE_NO.to_string(),
                description: Some("Keep subagents disabled.".to_string()),
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some(MULTI_AGENT_ENABLE_TITLE.to_string()),
            subtitle: Some("Subagents are currently disabled in your config.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    pub(crate) fn open_memories_popup(&mut self) {
        if !self.config.features.enabled(Feature::MemoryTool) {
            self.open_memories_enable_prompt();
            return;
        }

        let view = MemoriesSettingsView::new(
            self.config.memories.use_memories,
            self.config.memories.generate_memories,
            self.app_event_tx.clone(),
            self.bottom_pane.list_keymap(),
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    pub(crate) fn open_memories_enable_prompt(&mut self) {
        let items = vec![
            SelectionItem {
                name: MEMORIES_ENABLE_YES.to_string(),
                description: Some(
                    "Save the setting now. You will need a new session to use it.".to_string(),
                ),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::UpdateFeatureFlags {
                        updates: vec![(Feature::MemoryTool, true)],
                    });
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: MEMORIES_ENABLE_NO.to_string(),
                description: Some("Keep memories disabled.".to_string()),
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some(MEMORIES_ENABLE_TITLE.to_string()),
            subtitle: Some("Memories are currently disabled in your config.".to_string()),
            footer_note: Some(Line::from(vec![
                "Learn more: ".dim(),
                MEMORIES_DOC_URL.cyan().underlined(),
            ])),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    pub(crate) fn set_memory_settings(&mut self, use_memories: bool, generate_memories: bool) {
        self.config.memories.use_memories = use_memories;
        self.config.memories.generate_memories = generate_memories;
    }

    pub(crate) fn set_token_info(&mut self, info: Option<TokenUsageInfo>) {
        match info {
            Some(info) => self.apply_token_info(info),
            None => {
                self.bottom_pane
                    .set_context_window(/*percent*/ None, /*used_tokens*/ None);
                self.token_info = None;
            }
        }
    }

    fn apply_token_info(&mut self, info: TokenUsageInfo) {
        let percent = self.context_remaining_percent(&info);
        let used_tokens = self.context_used_tokens(&info, percent.is_some());
        self.bottom_pane.set_context_window(percent, used_tokens);
        self.token_info = Some(info);
    }

    fn context_remaining_percent(&self, info: &TokenUsageInfo) -> Option<i64> {
        info.model_context_window.map(|window| {
            info.last_token_usage
                .percent_of_context_window_remaining(window)
        })
    }

    fn context_used_tokens(&self, info: &TokenUsageInfo, percent_known: bool) -> Option<i64> {
        if percent_known {
            return None;
        }

        Some(info.total_token_usage.tokens_in_context_window())
    }

    fn restore_pre_review_token_info(&mut self) {
        if let Some(saved) = self.review.pre_review_token_info.take() {
            match saved {
                Some(info) => self.apply_token_info(info),
                None => {
                    self.bottom_pane
                        .set_context_window(/*percent*/ None, /*used_tokens*/ None);
                    self.token_info = None;
                }
            }
        }
    }

    pub(crate) fn handle_history_entry_response(&mut self, event: HistoryLookupResponse) {
        let HistoryLookupResponse {
            offset,
            log_id,
            entry,
        } = event;
        self.bottom_pane
            .on_history_entry_response(log_id, offset, entry);
    }

    pub(crate) fn pre_draw_tick(&mut self) {
        self.update_due_hook_visibility();
        self.schedule_hook_timer_if_needed();
        self.bottom_pane.pre_draw_tick();
        if let Some(pet) = self.ambient_pet.as_ref() {
            pet.schedule_next_frame();
        }
        self.refresh_plan_mode_nudge();
        self.refresh_goal_status_indicator_for_time_tick();
        if self.terminal_title_shows_action_required() != self.last_terminal_title_requires_action {
            self.refresh_terminal_title();
        }
        if self.should_animate_terminal_title_spinner()
            || self.should_animate_terminal_title_action_required()
        {
            self.refresh_terminal_title();
        }
    }

    fn flush_active_cell(&mut self) {
        if let Some(active) = self.transcript.active_cell.take() {
            self.transcript.needs_final_message_separator = true;
            self.app_event_tx.send(AppEvent::InsertHistoryCell(active));
        }
    }

    pub(crate) fn add_to_history(&mut self, cell: impl HistoryCell + 'static) {
        self.add_boxed_history(Box::new(cell));
    }

    fn add_boxed_history(&mut self, cell: Box<dyn HistoryCell>) {
        // Keep the placeholder session header as the active cell until real session info arrives,
        // so we can merge headers instead of committing a duplicate box to history.
        let keep_placeholder_header_active = !self.is_session_configured()
            && self
                .transcript
                .active_cell
                .as_ref()
                .is_some_and(|c| c.as_any().is::<history_cell::SessionHeaderHistoryCell>());

        if !keep_placeholder_header_active && !cell.display_lines(u16::MAX).is_empty() {
            // Only break exec grouping if the cell renders visible lines.
            if !self.has_active_stream_tail() {
                self.flush_active_cell();
            }
            self.transcript.needs_final_message_separator = true;
        }
        self.app_event_tx.send(AppEvent::InsertHistoryCell(cell));
    }

    fn enter_review_mode_with_hint(&mut self, hint: String, from_replay: bool) {
        if self.review.pre_review_token_info.is_none() {
            self.review.pre_review_token_info = Some(self.token_info.clone());
        }
        if !from_replay && !self.bottom_pane.is_task_running() {
            self.bottom_pane.set_task_running(/*running*/ true);
        }
        self.review.is_review_mode = true;
        let banner = format!(">> Code review started: {hint} <<");
        self.add_to_history(history_cell::new_review_status_line(banner));
        self.request_redraw();
    }

    fn exit_review_mode_after_item(&mut self) {
        self.flush_answer_stream_with_separator();
        self.flush_interrupt_queue();
        self.flush_active_cell();
        self.review.is_review_mode = false;
        self.restore_pre_review_token_info();
        self.add_to_history(history_cell::new_review_status_line(
            "<< Code review finished >>".to_string(),
        ));
        self.request_redraw();
    }

    fn on_committed_user_message(&mut self, items: &[UserInput], from_replay: bool) {
        let display = Self::user_message_display_from_inputs(items);
        if from_replay {
            if !self.review.is_review_mode {
                self.on_user_message_display(display);
            }
            return;
        }

        let compare_key = Self::pending_steer_compare_key_from_items(items);
        if self
            .input_queue
            .pending_steers
            .front()
            .is_some_and(|pending| pending.compare_key == compare_key)
        {
            if let Some(pending) = self.input_queue.pending_steers.pop_front() {
                self.refresh_pending_input_preview();
                let pending_display =
                    user_message_display_for_history(pending.user_message, &pending.history_record);
                self.on_user_message_display(pending_display);
            } else if self.last_rendered_user_message_display.as_ref() != Some(&display) {
                tracing::warn!(
                    "pending steer matched compare key but queue was empty when rendering committed user message"
                );
                self.on_user_message_display(display);
            }
        } else if !self.review.is_review_mode
            && self.last_rendered_user_message_display.as_ref() != Some(&display)
        {
            self.on_user_message_display(display);
        }
    }

    fn on_user_message_display(&mut self, display: UserMessageDisplay) {
        self.last_rendered_user_message_display = Some(display.clone());
        if !display.message.trim().is_empty()
            || !display.text_elements.is_empty()
            || !display.local_images.is_empty()
            || !display.remote_image_urls.is_empty()
        {
            self.record_visible_user_turn_for_copy();
            self.add_to_history(history_cell::new_user_prompt(
                display.message,
                display.text_elements,
                display.local_images,
                display.remote_image_urls,
            ));
        }

        // User messages reset separator state so the next agent response doesn't add a stray break.
        self.transcript.needs_final_message_separator = false;
    }

    /// Exit the UI immediately without waiting for shutdown.
    ///
    /// Prefer [`Self::request_quit_without_confirmation`] for user-initiated exits;
    /// this is mainly a fallback for shutdown completion or emergency exits.
    fn request_immediate_exit(&self) {
        self.app_event_tx.send(AppEvent::Exit(ExitMode::Immediate));
    }

    /// Request a shutdown-first quit.
    ///
    /// This is used for explicit quit commands (`/quit`, `/exit`, `/logout`) and for
    /// the double-press Ctrl+C/Ctrl+D quit shortcut.
    fn request_quit_without_confirmation(&self) {
        self.app_event_tx
            .send(AppEvent::Exit(ExitMode::ShutdownFirst));
    }

    fn request_redraw(&mut self) {
        self.frame_requester.schedule_frame();
    }

    fn bump_active_cell_revision(&mut self) {
        self.transcript.bump_active_cell_revision();
    }

    /// Mark the active cell as failed (✗) and flush it into history.
    fn finalize_active_cell_as_failed(&mut self) {
        if let Some(mut cell) = self.transcript.active_cell.take() {
            // Insert finalized cell into history and keep grouping consistent.
            if let Some(exec) = cell.as_any_mut().downcast_mut::<ExecCell>() {
                exec.mark_failed();
            } else if let Some(tool) = cell.as_any_mut().downcast_mut::<McpToolCallCell>() {
                tool.mark_failed();
            }
            self.add_boxed_history(cell);
        }
    }

    pub(crate) fn set_pending_thread_approvals(&mut self, threads: Vec<String>) {
        self.bottom_pane.set_pending_thread_approvals(threads);
    }

    pub(crate) fn clear_thread_rename_block(&mut self) {
        self.thread_rename_block_message = None;
    }

    pub(crate) fn set_thread_rename_block_message(&mut self, message: impl Into<String>) {
        self.thread_rename_block_message = Some(message.into());
    }

    pub(crate) fn set_interrupted_turn_notice_mode(&mut self, mode: InterruptedTurnNoticeMode) {
        self.interrupted_turn_notice_mode = mode;
    }

    pub(crate) fn add_diff_in_progress(&mut self) {
        self.request_redraw();
    }

    pub(crate) fn on_diff_complete(&mut self) {
        self.request_redraw();
    }

    pub(crate) fn add_debug_config_output(&mut self) {
        self.add_to_history(crate::debug_config::new_debug_config_output(
            &self.config,
            self.session_network_proxy.as_ref(),
        ));
    }

    pub(crate) fn add_ps_output(&mut self) {
        let processes = self
            .unified_exec_processes
            .iter()
            .map(|process| history_cell::UnifiedExecProcessDetails {
                command_display: process.command_display.clone(),
                recent_chunks: process.recent_chunks.clone(),
            })
            .collect();
        self.add_to_history(history_cell::new_unified_exec_processes_output(processes));
    }

    fn clean_background_terminals(&mut self) {
        self.submit_op(AppCommand::clean_background_terminals());
        self.unified_exec_processes.clear();
        self.sync_unified_exec_footer();
        self.add_info_message(
            "Stopping all background terminals.".to_string(),
            /*hint*/ None,
        );
    }

    fn plugins_for_mentions(&self) -> Option<&[PluginCapabilitySummary]> {
        if !self.config.features.enabled(Feature::Plugins) {
            return None;
        }

        self.bottom_pane.plugins().map(Vec::as_slice)
    }

    /// Build a placeholder header cell while the session is configuring.
    fn placeholder_session_header_cell(config: &Config) -> Box<dyn HistoryCell> {
        let placeholder_style = Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC);
        Box::new(
            history_cell::SessionHeaderHistoryCell::new_with_style(
                DEFAULT_MODEL_DISPLAY_NAME.to_string(),
                placeholder_style,
                /*reasoning_effort*/ None,
                /*show_fast_status*/ false,
                config.cwd.to_path_buf(),
                CODEX_CLI_VERSION,
            )
            .with_yolo_mode(history_cell::is_yolo_mode(config)),
        )
    }

    /// Merge the real session info cell with any placeholder header to avoid double boxes.
    fn apply_session_info_cell(&mut self, cell: history_cell::SessionInfoCell) {
        let mut session_info_cell = Some(Box::new(cell) as Box<dyn HistoryCell>);
        let merged_header = if let Some(active) = self.transcript.active_cell.take() {
            if active
                .as_any()
                .is::<history_cell::SessionHeaderHistoryCell>()
            {
                // Reuse the existing placeholder header to avoid rendering two boxes.
                if let Some(cell) = session_info_cell.take() {
                    self.transcript.active_cell = Some(cell);
                }
                true
            } else {
                self.transcript.active_cell = Some(active);
                false
            }
        } else {
            false
        };

        self.flush_active_cell();

        if !merged_header && let Some(cell) = session_info_cell {
            self.add_boxed_history(cell);
        }
    }

    pub(crate) fn add_info_message(&mut self, message: String, hint: Option<String>) {
        self.add_to_history(history_cell::new_info_event(message, hint));
        self.request_redraw();
    }

    pub(crate) fn add_memories_enable_notice(&mut self) {
        self.add_to_history(history_cell::new_warning_event(
            MEMORIES_ENABLE_NOTICE.to_string(),
        ));
        self.request_redraw();
    }

    pub(crate) fn add_plain_history_lines(&mut self, lines: Vec<Line<'static>>) {
        self.add_boxed_history(Box::new(PlainHistoryCell::new(lines)));
        self.request_redraw();
    }

    pub(crate) fn add_warning_message(&mut self, message: String) {
        self.add_to_history(history_cell::new_warning_event(message));
        self.request_redraw();
    }

    pub(crate) fn add_error_message(&mut self, message: String) {
        self.add_to_history(history_cell::new_error_event(message));
        self.request_redraw();
    }

    fn add_app_server_stub_message(&mut self, feature: &str) {
        warn!(feature, "stubbed unsupported TUI feature");
        self.add_error_message(format!("{feature}: {TUI_STUB_MESSAGE}"));
    }

    fn rename_confirmation_cell(name: &str, thread_id: Option<ThreadId>) -> PlainHistoryCell {
        let resume_cmd = crate::legacy_core::util::resume_command(Some(name), thread_id)
            .unwrap_or_else(|| format!("codex resume {name}"));
        let name = name.to_string();
        let line = vec![
            "• ".into(),
            "Thread renamed to ".into(),
            name.cyan(),
            ", to resume this thread run ".into(),
            resume_cmd.cyan(),
        ];
        PlainHistoryCell::new(vec![line.into()])
    }

    /// Begin the asynchronous MCP inventory flow: show a loading spinner and
    /// request the app-server fetch via `AppEvent::FetchMcpInventory`.
    ///
    /// The spinner lives in `active_cell` and is cleared by
    /// [`clear_mcp_inventory_loading`] once the result arrives.
    pub(crate) fn add_mcp_output(&mut self, detail: McpServerStatusDetail) {
        self.flush_answer_stream_with_separator();
        self.flush_active_cell();
        self.transcript.active_cell = Some(Box::new(history_cell::new_mcp_inventory_loading(
            self.config.animations,
        )));
        self.bump_active_cell_revision();
        self.request_redraw();
        self.app_event_tx
            .send(AppEvent::FetchMcpInventory { detail });
    }

    /// Remove the MCP loading spinner if it is still the active cell.
    ///
    /// Uses `Any`-based type checking so that a late-arriving inventory result
    /// does not accidentally clear an unrelated cell that was set in the meantime.
    pub(crate) fn clear_mcp_inventory_loading(&mut self) {
        let Some(active) = self.transcript.active_cell.as_ref() else {
            return;
        };
        if !active
            .as_any()
            .is::<history_cell::McpInventoryLoadingCell>()
        {
            return;
        }
        self.transcript.active_cell = None;
        self.bump_active_cell_revision();
        self.request_redraw();
    }

    /// Forward file-search results to the bottom pane.
    pub(crate) fn apply_file_search_result(&mut self, query: String, matches: Vec<FileMatch>) {
        self.bottom_pane.on_file_search_result(query, matches);
    }

    /// Return the markdown body width available to an active stream.
    ///
    /// Streaming controllers render only the message body, while history cells add bullets,
    /// gutters, or plan padding around that body. Callers pass the reserved columns for that
    /// wrapper so live output uses the same width that finalized cells will use during reflow.
    fn current_stream_width(&self, reserved_cols: usize) -> Option<usize> {
        self.last_rendered_width.get().and_then(|width| {
            if width == 0 {
                None
            } else {
                let width = u16::try_from(width).unwrap_or(u16::MAX);
                let width = usize::from(self.history_wrap_width(width));
                Some(crate::width::usable_content_width(width, reserved_cols).unwrap_or(1))
            }
        })
    }

    pub(crate) fn raw_output_mode(&self) -> bool {
        self.raw_output_mode
    }

    pub(crate) fn history_render_mode(&self) -> HistoryRenderMode {
        if self.raw_output_mode {
            HistoryRenderMode::Raw
        } else {
            HistoryRenderMode::Rich
        }
    }

    pub(crate) fn set_raw_output_mode(&mut self, enabled: bool) {
        self.raw_output_mode = enabled;
        self.config.tui_raw_output_mode = enabled;
        let render_mode = self.history_render_mode();
        if let Some(controller) = self.stream_controller.as_mut() {
            controller.set_render_mode(render_mode);
        }
        if let Some(controller) = self.plan_stream_controller.as_mut() {
            controller.set_render_mode(render_mode);
        }
        self.refresh_status_surfaces();
    }

    pub(crate) fn raw_output_mode_notice(enabled: bool) -> &'static str {
        if enabled {
            "Raw output mode on: transcript text is shown for clean terminal selection."
        } else {
            "Raw output mode off: rich transcript rendering restored."
        }
    }

    pub(crate) fn set_raw_output_mode_and_notify(&mut self, enabled: bool) {
        self.set_raw_output_mode(enabled);
        self.add_info_message(
            Self::raw_output_mode_notice(enabled).to_string(),
            /*hint*/ None,
        );
    }

    pub(crate) fn toggle_raw_output_mode_and_notify(&mut self) -> bool {
        let enabled = !self.raw_output_mode;
        self.set_raw_output_mode_and_notify(enabled);
        enabled
    }

    /// Update resize-sensitive chat widget state after the terminal width changes.
    ///
    /// The app calls this even when terminal resize reflow is disabled so live stream wrapping
    /// remains consistent with the current viewport. Finalized transcript rebuilding stays gated at
    /// the app layer.
    pub(crate) fn on_terminal_resize(&mut self, width: u16) {
        let had_rendered_width = self.last_rendered_width.get().is_some();
        self.last_rendered_width.set(Some(width as usize));
        let stream_width = self.current_stream_width(/*reserved_cols*/ 2);
        let plan_stream_width = self.current_stream_width(/*reserved_cols*/ 4);
        if let Some(controller) = self.stream_controller.as_mut() {
            controller.set_width(stream_width);
        }
        if let Some(controller) = self.plan_stream_controller.as_mut() {
            controller.set_width(plan_stream_width);
        }
        self.sync_active_stream_tail();
        if !had_rendered_width {
            self.request_redraw();
        }
    }

    /// Whether an agent message stream is active (not a plan stream).
    pub(crate) fn has_active_agent_stream(&self) -> bool {
        self.stream_controller.is_some()
    }

    /// Whether a proposed-plan stream is active.
    pub(crate) fn has_active_plan_stream(&self) -> bool {
        self.plan_stream_controller.is_some()
    }

    fn is_plan_streaming_in_tui(&self) -> bool {
        self.plan_stream_controller.is_some()
    }

    pub(crate) fn composer_is_empty(&self) -> bool {
        self.bottom_pane.composer_is_empty()
    }

    #[cfg(test)]
    pub(crate) fn is_task_running_for_test(&self) -> bool {
        self.bottom_pane.is_task_running()
    }

    pub(crate) fn toggle_vim_mode_and_notify(&mut self) {
        let enabled = self.bottom_pane.toggle_vim_enabled();
        let message = if enabled {
            "Vim mode enabled."
        } else {
            "Vim mode disabled."
        };
        self.add_info_message(message.to_string(), /*hint*/ None);
    }

    /// True when the UI is in the regular composer state with no running task,
    /// no modal overlay (e.g. approvals or status indicator), and no composer popups.
    /// In this state Esc-Esc backtracking is enabled.
    pub(crate) fn is_normal_backtrack_mode(&self) -> bool {
        self.bottom_pane.is_normal_backtrack_mode()
    }

    pub(crate) fn should_handle_vim_insert_escape(&self, key_event: KeyEvent) -> bool {
        self.bottom_pane
            .composer_should_handle_vim_insert_escape(key_event)
    }

    pub(crate) fn insert_str(&mut self, text: &str) {
        self.bottom_pane.insert_str(text);
    }

    /// Replace the composer content with the provided text and reset cursor.
    pub(crate) fn set_composer_text(
        &mut self,
        text: String,
        text_elements: Vec<TextElement>,
        local_image_paths: Vec<PathBuf>,
    ) {
        self.bottom_pane
            .set_composer_text(text, text_elements, local_image_paths);
        self.refresh_plan_mode_nudge();
    }

    pub(crate) fn set_remote_image_urls(&mut self, remote_image_urls: Vec<String>) {
        self.bottom_pane.set_remote_image_urls(remote_image_urls);
    }

    fn take_remote_image_urls(&mut self) -> Vec<String> {
        self.bottom_pane.take_remote_image_urls()
    }

    #[cfg(test)]
    pub(crate) fn remote_image_urls(&self) -> Vec<String> {
        self.bottom_pane.remote_image_urls()
    }

    #[cfg(test)]
    pub(crate) fn pending_thread_approvals(&self) -> &[String] {
        self.bottom_pane.pending_thread_approvals()
    }

    #[cfg(test)]
    pub(crate) fn has_active_view(&self) -> bool {
        self.bottom_pane.has_active_view()
    }

    pub(crate) fn show_esc_backtrack_hint(&mut self) {
        self.bottom_pane.show_esc_backtrack_hint();
    }

    pub(crate) fn clear_esc_backtrack_hint(&mut self) {
        self.bottom_pane.clear_esc_backtrack_hint();
    }

    fn refresh_skills_for_current_cwd(&mut self, force_reload: bool) {
        self.submit_op(AppCommand::list_skills(
            vec![self.config.cwd.to_path_buf()],
            force_reload,
        ));
    }

    /// Forward a command directly to codex.
    pub(crate) fn submit_op<T>(&mut self, op: T) -> bool
    where
        T: Into<AppCommand>,
    {
        let op: AppCommand = op.into();
        self.prepare_local_op_submission(&op);
        if op.is_review() && !self.bottom_pane.is_task_running() {
            self.bottom_pane.set_task_running(/*running*/ true);
        }
        match &self.codex_op_target {
            CodexOpTarget::Direct(codex_op_tx) => {
                crate::session_log::log_outbound_op(&op);
                if let Err(e) = codex_op_tx.send(op) {
                    tracing::error!("failed to submit op: {e}");
                    return false;
                }
            }
            CodexOpTarget::AppEvent => {
                self.app_event_tx.send(AppEvent::CodexOp(op));
            }
        }
        true
    }

    fn append_message_history_entry(&self, text: String) {
        let Some(thread_id) = self.thread_id else {
            tracing::warn!("failed to append to message history: no active thread id");
            return;
        };
        self.app_event_tx
            .send(AppEvent::AppendMessageHistoryEntry { thread_id, text });
    }

    pub(crate) fn prepare_local_op_submission(&mut self, op: &AppCommand) {
        if matches!(op, AppCommand::Interrupt) && self.turn_lifecycle.agent_turn_running {
            if let Some(controller) = self.stream_controller.as_mut() {
                controller.clear_queue();
            }
            if let Some(controller) = self.plan_stream_controller.as_mut() {
                controller.clear_queue();
            }
            self.clear_active_stream_tail();
            self.request_redraw();
        }
    }

    fn on_list_skills(&mut self, ev: SkillsListResponse) {
        self.set_skills_from_response(&ev);
        self.refresh_plugin_mentions();
    }

    pub(crate) fn refresh_plugin_mentions(&mut self) {
        if !self.config.features.enabled(Feature::Plugins) {
            self.bottom_pane.set_plugin_mentions(/*plugins*/ None);
            return;
        }

        self.app_event_tx.send(AppEvent::RefreshPluginMentions);
    }

    pub(crate) fn on_plugin_mentions_loaded(
        &mut self,
        plugins: Option<Vec<PluginCapabilitySummary>>,
    ) {
        if self.bottom_pane.plugins() == plugins.as_ref() {
            return;
        }
        self.bottom_pane.set_plugin_mentions(plugins);
    }

    pub(crate) fn sync_plugin_mentions_config(&mut self, config: &Config) {
        self.config.features = config.features.clone();
        self.config.config_layer_stack = config.config_layer_stack.clone();
        self.config.realtime = config.realtime.clone();
        self.config.memories = config.memories.clone();
        self.config.terminal_resize_reflow = config.terminal_resize_reflow;
        self.sync_mentions_v2_enabled();
    }

    pub(crate) fn token_usage(&self) -> TokenUsage {
        self.token_info
            .as_ref()
            .map(|ti| ti.total_token_usage.clone())
            .unwrap_or_default()
    }

    pub(crate) fn thread_id(&self) -> Option<ThreadId> {
        self.thread_id
    }

    pub(crate) fn thread_name(&self) -> Option<String> {
        self.thread_name.clone()
    }

    /// Returns the current thread's precomputed rollout path.
    ///
    /// For fresh non-ephemeral threads this path may exist before the file is
    /// materialized; rollout persistence is deferred until the first user
    /// message is recorded.
    pub(crate) fn rollout_path(&self) -> Option<PathBuf> {
        self.current_rollout_path.clone()
    }

    /// Returns a cache key describing the current in-flight active cell for the transcript overlay.
    ///
    /// `Ctrl+T` renders committed transcript cells plus a render-only live tail derived from the
    /// current active cell, and the overlay caches that tail; this key is what it uses to decide
    /// whether it must recompute. When there is no active cell, this returns `None` so the overlay
    /// can drop the tail entirely.
    ///
    /// If callers mutate the active cell's transcript output without bumping the revision (or
    /// providing an appropriate animation tick), the overlay will keep showing a stale tail while
    /// the main viewport updates.
    pub(crate) fn active_cell_transcript_key(&self) -> Option<ActiveCellTranscriptKey> {
        let cell = self.transcript.active_cell.as_ref();
        let hook_cell = self.active_hook_cell.as_ref();
        if cell.is_none() && hook_cell.is_none() {
            return None;
        }
        Some(ActiveCellTranscriptKey {
            revision: self.transcript.active_cell_revision,
            is_stream_continuation: cell
                .map(|cell| cell.is_stream_continuation())
                .unwrap_or(false),
            animation_tick: cell
                .and_then(|cell| cell.transcript_animation_tick())
                .or_else(|| {
                    hook_cell.and_then(super::history_cell::HistoryCell::transcript_animation_tick)
                }),
        })
    }

    /// Returns the active cell's transcript lines for a given terminal width.
    ///
    /// This is a convenience for the transcript overlay live-tail path, and it intentionally
    /// filters out empty results so the overlay can treat "nothing to render" as "no tail". Callers
    /// should pass the same width the overlay uses; using a different width will cause wrapping
    /// mismatches between the main viewport and the transcript overlay.
    pub(crate) fn active_cell_transcript_lines(&self, width: u16) -> Option<Vec<Line<'static>>> {
        let mut lines = Vec::new();
        if let Some(cell) = self.transcript.active_cell.as_ref() {
            lines.extend(cell.transcript_lines(width));
        }
        if let Some(hook_cell) = self.active_hook_cell.as_ref() {
            // Compute hook lines first so hidden hooks do not add a separator.
            let hook_lines = hook_cell.transcript_lines(width);
            if !hook_lines.is_empty() && !lines.is_empty() {
                lines.push("".into());
            }
            lines.extend(hook_lines);
        }
        (!lines.is_empty()).then_some(lines)
    }

    /// Return a reference to the widget's current config (includes any
    /// runtime overrides applied via TUI, e.g., model or approval policy).
    pub(crate) fn config_ref(&self) -> &Config {
        &self.config
    }

    #[cfg(test)]
    pub(crate) fn status_line_text(&self) -> Option<String> {
        self.bottom_pane.status_line_text()
    }

    pub(crate) fn clear_token_usage(&mut self) {
        self.token_info = None;
    }
}

#[cfg(not(target_os = "linux"))]
impl ChatWidget {
    pub(crate) fn update_recording_meter_in_place(&mut self, id: &str, text: &str) -> bool {
        let updated = self.bottom_pane.update_recording_meter_in_place(id, text);
        if updated {
            self.request_redraw();
        }
        updated
    }

    pub(crate) fn remove_recording_meter_placeholder(&mut self, id: &str) {
        self.bottom_pane.remove_recording_meter_placeholder(id);
        // Ensure the UI redraws to reflect placeholder removal.
        self.request_redraw();
    }
}

fn has_websocket_timing_metrics(summary: RuntimeMetricsSummary) -> bool {
    summary.responses_api_overhead_ms > 0
        || summary.responses_api_inference_time_ms > 0
        || summary.responses_api_engine_iapi_ttft_ms > 0
        || summary.responses_api_engine_service_ttft_ms > 0
        || summary.responses_api_engine_iapi_tbt_ms > 0
        || summary.responses_api_engine_service_tbt_ms > 0
}

impl Drop for ChatWidget {
    fn drop(&mut self) {
        self.reset_realtime_conversation_state();
        self.stop_rate_limit_poller();
    }
}

const PLACEHOLDERS: [&str; 8] = [
    "Explain this codebase",
    "Summarize recent commits",
    "Implement {feature}",
    "Find and fix a bug in @filename",
    "Write tests for @filename",
    "Improve documentation in @filename",
    "Run /review on my current changes",
    "Use /skills to list available skills",
];

const SIDE_PLACEHOLDERS: [&str; 3] = [
    "Check recently modified functions for compatibility",
    "How many files have been modified?",
    "Will this algorithm scale well?",
];

// Extract the first bold (Markdown) element in the form **...** from `s`.
// Returns the inner text if found; otherwise `None`.
fn extract_first_bold(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'*' {
            let start = i + 2;
            let mut j = start;
            while j + 1 < bytes.len() {
                if bytes[j] == b'*' && bytes[j + 1] == b'*' {
                    // Found closing **
                    let inner = &s[start..j];
                    let trimmed = inner.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    } else {
                        return None;
                    }
                }
                j += 1;
            }
            // No closing; stop searching (wait for more deltas)
            return None;
        }
        i += 1;
    }
    None
}

#[cfg(test)]
pub(crate) mod tests;
