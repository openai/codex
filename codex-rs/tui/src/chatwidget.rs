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
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use url::Url;

use self::realtime::PendingSteerCompareKey;
use crate::app_command::AppCommand;
use crate::app_event::RealtimeAudioDeviceKind;
use crate::app_server_approval_conversions::network_approval_context_to_core;
use crate::app_server_session::ThreadSessionState;
#[cfg(not(target_os = "linux"))]
use crate::audio_device::list_realtime_audio_device_names;
use crate::bottom_pane::StatusLineItem;
use crate::bottom_pane::StatusLinePreviewData;
use crate::bottom_pane::StatusLineSetupView;
use crate::bottom_pane::TerminalTitleItem;
use crate::bottom_pane::TerminalTitleSetupView;
use crate::legacy_core::DEFAULT_PROJECT_DOC_FILENAME;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::Constrained;
use crate::legacy_core::config::ConstraintResult;
use crate::legacy_core::config_loader::ConfigLayerStackOrdering;
use crate::legacy_core::find_thread_name_by_id;
use crate::legacy_core::plugins::PluginsManager;
use crate::legacy_core::skills::model::SkillMetadata;
#[cfg(target_os = "windows")]
use crate::legacy_core::windows_sandbox::WindowsSandboxLevelExt;
use crate::mention_codec::LinkedMention;
use crate::mention_codec::encode_history_mentions;
use crate::model_catalog::ModelCatalog;
use crate::multi_agents;
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
use crate::version::CODEX_CLI_VERSION;
use codex_app_server_protocol::AppSummary;
use codex_app_server_protocol::CodexErrorInfo as AppServerCodexErrorInfo;
use codex_app_server_protocol::CollabAgentState as AppServerCollabAgentState;
use codex_app_server_protocol::CollabAgentStatus as AppServerCollabAgentStatus;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
use codex_app_server_protocol::ConfigLayerSource;
use codex_app_server_protocol::ErrorNotification;
use codex_app_server_protocol::FileChangeRequestApprovalParams;
use codex_app_server_protocol::GuardianApprovalReviewAction;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::McpServerStartupState;
use codex_app_server_protocol::McpServerStatusUpdatedNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnPlanStepStatus;
use codex_app_server_protocol::TurnStatus;
use codex_chatgpt::connectors;
use codex_config::types::ApprovalsReviewer;
use codex_config::types::Notifications;
use codex_config::types::WindowsSandboxModeToml;
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
use codex_protocol::ThreadId;
use codex_protocol::account::PlanType;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::config_types::Settings;
#[cfg(target_os = "windows")]
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::local_image_label_text;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::plan_tool::PlanItemArg as UpdatePlanItemArg;
use codex_protocol::plan_tool::StepStatus as UpdatePlanItemStatus;
#[cfg(test)]
use codex_protocol::protocol::AgentMessageDeltaEvent;
#[cfg(test)]
use codex_protocol::protocol::AgentMessageEvent;
#[cfg(test)]
use codex_protocol::protocol::AgentReasoningDeltaEvent;
#[cfg(test)]
use codex_protocol::protocol::AgentReasoningEvent;
#[cfg(test)]
use codex_protocol::protocol::AgentReasoningRawContentDeltaEvent;
#[cfg(test)]
use codex_protocol::protocol::AgentReasoningRawContentEvent;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
#[cfg(test)]
use codex_protocol::protocol::BackgroundEventEvent;
#[cfg(test)]
use codex_protocol::protocol::CodexErrorInfo as CoreCodexErrorInfo;
use codex_protocol::protocol::CollabAgentRef;
#[cfg(test)]
use codex_protocol::protocol::CollabAgentSpawnBeginEvent;
use codex_protocol::protocol::CollabAgentStatusEntry;
use codex_protocol::protocol::CreditsSnapshot;
use codex_protocol::protocol::DeprecationNoticeEvent;
#[cfg(test)]
use codex_protocol::protocol::ErrorEvent;
#[cfg(test)]
use codex_protocol::protocol::Event;
#[cfg(test)]
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::ExecCommandOutputDeltaEvent;
use codex_protocol::protocol::ExecCommandSource;
#[cfg(test)]
use codex_protocol::protocol::ExitedReviewModeEvent;
use codex_protocol::protocol::GuardianAssessmentAction;
use codex_protocol::protocol::GuardianAssessmentDecisionSource;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::ImageGenerationBeginEvent;
use codex_protocol::protocol::ImageGenerationEndEvent;
use codex_protocol::protocol::ListSkillsResponseEvent;
#[cfg(test)]
use codex_protocol::protocol::McpListToolsResponseEvent;
#[cfg(test)]
use codex_protocol::protocol::McpStartupCompleteEvent;
use codex_protocol::protocol::McpStartupStatus;
#[cfg(test)]
use codex_protocol::protocol::McpStartupUpdateEvent;
use codex_protocol::protocol::McpToolCallBeginEvent;
use codex_protocol::protocol::McpToolCallEndEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::PatchApplyBeginEvent;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::ReviewTarget;
use codex_protocol::protocol::SkillMetadata as ProtocolSkillMetadata;
#[cfg(test)]
use codex_protocol::protocol::StreamErrorEvent;
use codex_protocol::protocol::TerminalInteractionEvent;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TurnAbortReason;
#[cfg(test)]
use codex_protocol::protocol::TurnCompleteEvent;
#[cfg(test)]
use codex_protocol::protocol::TurnDiffEvent;
#[cfg(test)]
use codex_protocol::protocol::UndoCompletedEvent;
#[cfg(test)]
use codex_protocol::protocol::UndoStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use codex_protocol::protocol::ViewImageToolCallEvent;
#[cfg(test)]
use codex_protocol::protocol::WarningEvent;
use codex_protocol::protocol::WebSearchBeginEvent;
use codex_protocol::protocol::WebSearchEndEvent;
use codex_protocol::request_permissions::RequestPermissionsEvent;
use codex_protocol::request_user_input::RequestUserInputEvent;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_protocol::user_input::TextElement;
use codex_protocol::user_input::UserInput;
use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_sleep_inhibitor::SleepInhibitor;
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
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;
use tracing::warn;

const DEFAULT_MODEL_DISPLAY_NAME: &str = "loading";
const PLAN_IMPLEMENTATION_TITLE: &str = "Implement this plan?";
const PLAN_IMPLEMENTATION_YES: &str = "Yes, implement this plan";
const PLAN_IMPLEMENTATION_NO: &str = "No, stay in Plan mode";
const PLAN_IMPLEMENTATION_CODING_MESSAGE: &str = "Implement the plan.";
const MULTI_AGENT_ENABLE_TITLE: &str = "Enable subagents?";
const MULTI_AGENT_ENABLE_YES: &str = "Yes, enable";
const MULTI_AGENT_ENABLE_NO: &str = "Not now";
const MULTI_AGENT_ENABLE_NOTICE: &str = "Subagents will be enabled in the next session.";
const PLAN_MODE_REASONING_SCOPE_TITLE: &str = "Apply reasoning change";
const PLAN_MODE_REASONING_SCOPE_PLAN_ONLY: &str = "Apply to Plan mode override";
const PLAN_MODE_REASONING_SCOPE_ALL_MODES: &str = "Apply to global default and Plan mode override";
const CONNECTORS_SELECTION_VIEW_ID: &str = "connectors-selection";
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

use crate::app_event::AppEvent;
use crate::app_event::ConnectorsSnapshot;
use crate::app_event::ExitMode;
use crate::app_event::RateLimitRefreshOrigin;
#[cfg(target_os = "windows")]
use crate::app_event::WindowsSandboxEnableMode;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ApprovalRequest;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BottomPaneParams;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::CollaborationModeIndicator;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::DOUBLE_PRESS_QUIT_SHORTCUT_ENABLED;
use crate::bottom_pane::ExperimentalFeatureItem;
use crate::bottom_pane::ExperimentalFeaturesView;
use crate::bottom_pane::InputResult;
use crate::bottom_pane::LocalImageAttachment;
use crate::bottom_pane::McpServerElicitationFormRequest;
use crate::bottom_pane::MentionBinding;
use crate::bottom_pane::QUIT_SHORTCUT_TIMEOUT;
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
#[cfg(test)]
use crate::history_cell::AgentMessageCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::HookCell;
use crate::history_cell::McpToolCallCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::WebSearchCell;
use crate::key_hint;
use crate::key_hint::KeyBinding;
#[cfg(test)]
use crate::markdown::append_markdown;
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
mod interrupts;
use self::interrupts::InterruptManager;
mod session_header;
use self::session_header::SessionHeader;
mod skills;
mod slash_dispatch;
use self::skills::collect_tool_mentions;
use self::skills::find_app_mentions;
use self::skills::find_skill_mentions_with_tool_mentions;
mod plugins;
use self::plugins::PluginsCacheState;
mod realtime;
use self::realtime::RealtimeConversationUiState;
use self::realtime::RenderedUserMessageEvent;
mod status_surfaces;
use self::status_surfaces::CachedProjectRootName;
use self::status_surfaces::TerminalTitleStatusKind;
use crate::streaming::chunking::AdaptiveChunkingPolicy;
use crate::streaming::commit_tick::CommitTickScope;
use crate::streaming::commit_tick::run_commit_tick;
use crate::streaming::controller::PlanStreamController;
use crate::streaming::controller::StreamController;

use chrono::Local;
use codex_file_search::FileMatch;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::plan_tool::UpdatePlanArgs;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_approval_presets::ApprovalPreset;
use codex_utils_approval_presets::builtin_approval_presets;
use strum::IntoEnumIterator;
use unicode_segmentation::UnicodeSegmentation;

const USER_SHELL_COMMAND_HELP_TITLE: &str = "Prefix a command with ! to run it locally";
const USER_SHELL_COMMAND_HELP_HINT: &str = "Example: !ls";
const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_STATUS_LINE_ITEMS: [&str; 2] = ["model-with-reasoning", "current-dir"];
// Track information about an in-flight exec command.
struct RunningCommand {
    command: Vec<String>,
    parsed_cmd: Vec<ParsedCommand>,
    source: ExecCommandSource,
}

struct UnifiedExecProcessSummary {
    key: String,
    call_id: String,
    command_display: String,
    recent_chunks: Vec<String>,
}

struct UnifiedExecWaitState {
    command_display: String,
}

impl UnifiedExecWaitState {
    fn new(command_display: String) -> Self {
        Self { command_display }
    }

    fn is_duplicate(&self, command_display: &str) -> bool {
        self.command_display == command_display
    }
}

#[derive(Clone, Debug)]
struct UnifiedExecWaitStreak {
    process_id: String,
    command_display: Option<String>,
}

impl UnifiedExecWaitStreak {
    fn new(process_id: String, command_display: Option<String>) -> Self {
        Self {
            process_id,
            command_display: command_display.filter(|display| !display.is_empty()),
        }
    }

    fn update_command_display(&mut self, command_display: Option<String>) {
        if self.command_display.is_some() {
            return;
        }
        self.command_display = command_display.filter(|display| !display.is_empty());
    }
}

fn is_unified_exec_source(source: ExecCommandSource) -> bool {
    matches!(
        source,
        ExecCommandSource::UnifiedExecStartup | ExecCommandSource::UnifiedExecInteraction
    )
}

fn is_standard_tool_call(parsed_cmd: &[ParsedCommand]) -> bool {
    !parsed_cmd.is_empty()
        && parsed_cmd
            .iter()
            .all(|parsed| !matches!(parsed, ParsedCommand::Unknown { .. }))
}

const RATE_LIMIT_WARNING_THRESHOLDS: [f64; 3] = [75.0, 90.0, 95.0];
const NUDGE_MODEL_SLUG: &str = "gpt-5.1-codex-mini";
const RATE_LIMIT_SWITCH_PROMPT_THRESHOLD: f64 = 90.0;

#[derive(Default)]
struct RateLimitWarningState {
    secondary_index: usize,
    primary_index: usize,
}

impl RateLimitWarningState {
    fn take_warnings(
        &mut self,
        secondary_used_percent: Option<f64>,
        secondary_window_minutes: Option<i64>,
        primary_used_percent: Option<f64>,
        primary_window_minutes: Option<i64>,
    ) -> Vec<String> {
        let reached_secondary_cap =
            matches!(secondary_used_percent, Some(percent) if percent == 100.0);
        let reached_primary_cap = matches!(primary_used_percent, Some(percent) if percent == 100.0);
        if reached_secondary_cap || reached_primary_cap {
            return Vec::new();
        }

        let mut warnings = Vec::new();

        if let Some(secondary_used_percent) = secondary_used_percent {
            let mut highest_secondary: Option<f64> = None;
            while self.secondary_index < RATE_LIMIT_WARNING_THRESHOLDS.len()
                && secondary_used_percent >= RATE_LIMIT_WARNING_THRESHOLDS[self.secondary_index]
            {
                highest_secondary = Some(RATE_LIMIT_WARNING_THRESHOLDS[self.secondary_index]);
                self.secondary_index += 1;
            }
            if let Some(threshold) = highest_secondary {
                let limit_label = secondary_window_minutes
                    .map(get_limits_duration)
                    .unwrap_or_else(|| "weekly".to_string());
                let remaining_percent = 100.0 - threshold;
                warnings.push(format!(
                    "Heads up, you have less than {remaining_percent:.0}% of your {limit_label} limit left. Run /status for a breakdown."
                ));
            }
        }

        if let Some(primary_used_percent) = primary_used_percent {
            let mut highest_primary: Option<f64> = None;
            while self.primary_index < RATE_LIMIT_WARNING_THRESHOLDS.len()
                && primary_used_percent >= RATE_LIMIT_WARNING_THRESHOLDS[self.primary_index]
            {
                highest_primary = Some(RATE_LIMIT_WARNING_THRESHOLDS[self.primary_index]);
                self.primary_index += 1;
            }
            if let Some(threshold) = highest_primary {
                let limit_label = primary_window_minutes
                    .map(get_limits_duration)
                    .unwrap_or_else(|| "5h".to_string());
                let remaining_percent = 100.0 - threshold;
                warnings.push(format!(
                    "Heads up, you have less than {remaining_percent:.0}% of your {limit_label} limit left. Run /status for a breakdown."
                ));
            }
        }

        warnings
    }
}

pub(crate) fn get_limits_duration(windows_minutes: i64) -> String {
    const MINUTES_PER_HOUR: i64 = 60;
    const MINUTES_PER_DAY: i64 = 24 * MINUTES_PER_HOUR;
    const MINUTES_PER_WEEK: i64 = 7 * MINUTES_PER_DAY;
    const MINUTES_PER_MONTH: i64 = 30 * MINUTES_PER_DAY;
    const ROUNDING_BIAS_MINUTES: i64 = 3;

    let windows_minutes = windows_minutes.max(0);

    if windows_minutes <= MINUTES_PER_DAY.saturating_add(ROUNDING_BIAS_MINUTES) {
        let adjusted = windows_minutes.saturating_add(ROUNDING_BIAS_MINUTES);
        let hours = std::cmp::max(1, adjusted / MINUTES_PER_HOUR);
        format!("{hours}h")
    } else if windows_minutes <= MINUTES_PER_WEEK.saturating_add(ROUNDING_BIAS_MINUTES) {
        "weekly".to_string()
    } else if windows_minutes <= MINUTES_PER_MONTH.saturating_add(ROUNDING_BIAS_MINUTES) {
        "monthly".to_string()
    } else {
        "annual".to_string()
    }
}

/// Common initialization parameters shared by all `ChatWidget` constructors.
pub(crate) struct ChatWidgetInit {
    pub(crate) config: Config,
    pub(crate) frame_requester: FrameRequester,
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) initial_user_message: Option<UserMessage>,
    pub(crate) enhanced_keys_supported: bool,
    pub(crate) has_chatgpt_account: bool,
    pub(crate) model_catalog: Arc<ModelCatalog>,
    pub(crate) feedback: codex_feedback::CodexFeedback,
    pub(crate) is_first_run: bool,
    pub(crate) status_account_display: Option<StatusAccountDisplay>,
    pub(crate) initial_plan_type: Option<PlanType>,
    pub(crate) model: Option<String>,
    pub(crate) startup_tooltip_override: Option<String>,
    // Shared latch so we only warn once about invalid status-line item IDs.
    pub(crate) status_line_invalid_items_warned: Arc<AtomicBool>,
    // Shared latch so we only warn once about invalid terminal-title item IDs.
    pub(crate) terminal_title_invalid_items_warned: Arc<AtomicBool>,
    pub(crate) session_telemetry: SessionTelemetry,
}

#[derive(Default)]
enum RateLimitSwitchPromptState {
    #[default]
    Idle,
    Pending,
    Shown,
}

#[derive(Debug, Clone, Default)]
enum ConnectorsCacheState {
    #[default]
    Uninitialized,
    Loading,
    Ready(ConnectorsSnapshot),
    Failed(String),
}

#[derive(Debug, Clone, Default)]
struct PluginListFetchState {
    cache_cwd: Option<PathBuf>,
    in_flight_cwd: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct PluginInstallAuthFlowState {
    plugin_display_name: String,
    next_app_index: usize,
}

#[derive(Debug)]
enum RateLimitErrorKind {
    ServerOverloaded,
    UsageLimit,
    Generic,
}

#[cfg(test)]
fn core_rate_limit_error_kind(info: &CoreCodexErrorInfo) -> Option<RateLimitErrorKind> {
    match info {
        CoreCodexErrorInfo::ServerOverloaded => Some(RateLimitErrorKind::ServerOverloaded),
        CoreCodexErrorInfo::UsageLimitExceeded => Some(RateLimitErrorKind::UsageLimit),
        CoreCodexErrorInfo::ResponseTooManyFailedAttempts {
            http_status_code: Some(429),
        } => Some(RateLimitErrorKind::Generic),
        _ => None,
    }
}

fn app_server_rate_limit_error_kind(info: &AppServerCodexErrorInfo) -> Option<RateLimitErrorKind> {
    match info {
        AppServerCodexErrorInfo::ServerOverloaded => Some(RateLimitErrorKind::ServerOverloaded),
        AppServerCodexErrorInfo::UsageLimitExceeded => Some(RateLimitErrorKind::UsageLimit),
        AppServerCodexErrorInfo::ResponseTooManyFailedAttempts {
            http_status_code: Some(429),
        } => Some(RateLimitErrorKind::Generic),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ExternalEditorState {
    #[default]
    Closed,
    Requested,
    Active,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StatusIndicatorState {
    header: String,
    details: Option<String>,
    details_max_lines: usize,
}

impl StatusIndicatorState {
    fn working() -> Self {
        Self {
            header: String::from("Working"),
            details: None,
            details_max_lines: STATUS_DETAILS_DEFAULT_MAX_LINES,
        }
    }

    fn is_guardian_review(&self) -> bool {
        self.header == "Reviewing approval request" || self.header.starts_with("Reviewing ")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PendingGuardianReviewStatus {
    entries: Vec<PendingGuardianReviewStatusEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingGuardianReviewStatusEntry {
    id: String,
    detail: String,
}

impl PendingGuardianReviewStatus {
    fn start_or_update(&mut self, id: String, detail: String) {
        if let Some(existing) = self.entries.iter_mut().find(|entry| entry.id == id) {
            existing.detail = detail;
        } else {
            self.entries
                .push(PendingGuardianReviewStatusEntry { id, detail });
        }
    }

    fn finish(&mut self, id: &str) -> bool {
        let original_len = self.entries.len();
        self.entries.retain(|entry| entry.id != id);
        self.entries.len() != original_len
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    // Guardian review status is derived from the full set of currently pending
    // review entries. The generic status cache on `ChatWidget` stores whichever
    // footer is currently rendered; this helper computes the guardian-specific
    // footer snapshot that should replace it while reviews remain in flight.
    fn status_indicator_state(&self) -> Option<StatusIndicatorState> {
        let details = if self.entries.len() == 1 {
            self.entries.first().map(|entry| entry.detail.clone())
        } else if self.entries.is_empty() {
            None
        } else {
            let mut lines = self
                .entries
                .iter()
                .take(3)
                .map(|entry| format!("• {}", entry.detail))
                .collect::<Vec<_>>();
            let remaining = self.entries.len().saturating_sub(3);
            if remaining > 0 {
                lines.push(format!("+{remaining} more"));
            }
            Some(lines.join("\n"))
        };
        let details = details?;
        let header = if self.entries.len() == 1 {
            String::from("Reviewing approval request")
        } else {
            format!("Reviewing {} approval requests", self.entries.len())
        };
        let details_max_lines = if self.entries.len() == 1 { 1 } else { 4 };
        Some(StatusIndicatorState {
            header,
            details: Some(details),
            details_max_lines,
        })
    }
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
    active_cell: Option<Box<dyn HistoryCell>>,
    /// Monotonic-ish counter used to invalidate transcript overlay caching.
    ///
    /// The transcript overlay appends a cached "live tail" for the current active cell. Most
    /// active-cell updates are mutations of the *existing* cell (not a replacement), so pointer
    /// identity alone is not a good cache key.
    ///
    /// Callers bump this whenever the active cell's transcript output could change without
    /// flushing. It is intentionally allowed to wrap, which implies a rare one-time cache collision
    /// where the overlay may briefly treat new tail content as already cached.
    active_cell_revision: u64,
    config: Config,
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
    token_info: Option<TokenUsageInfo>,
    rate_limit_snapshots_by_limit_id: BTreeMap<String, RateLimitSnapshotDisplay>,
    refreshing_status_outputs: Vec<(u64, StatusHistoryHandle)>,
    next_status_refresh_request_id: u64,
    plan_type: Option<PlanType>,
    rate_limit_warnings: RateLimitWarningState,
    rate_limit_switch_prompt: RateLimitSwitchPromptState,
    adaptive_chunking: AdaptiveChunkingPolicy,
    // Stream lifecycle controller
    stream_controller: Option<StreamController>,
    // Stream lifecycle controller for proposed plan output.
    plan_stream_controller: Option<PlanStreamController>,
    /// Holds the platform clipboard lease so copied text remains available while supported.
    clipboard_lease: Option<crate::clipboard_copy::ClipboardLease>,
    /// Raw markdown of the most recently completed agent response.
    ///
    /// This cache is intentionally best-effort: if the user rolls back the
    /// thread and then copies before a replacement response arrives, `/copy`
    /// may still return the response from before the rollback. Keeping this as
    /// a single cache avoids coupling copy state to the backtrack transcript.
    last_agent_markdown: Option<String>,
    /// Whether this turn already produced a copyable response.
    ///
    /// `TurnComplete.last_agent_message` is a fallback source: use it only when no earlier
    /// agent/plan/review item recorded copyable markdown for the turn. This gives item-level
    /// sources precedence and avoids duplicating the same final answer when both event shapes are
    /// emitted.
    saw_copy_source_this_turn: bool,
    running_commands: HashMap<String, RunningCommand>,
    collab_agent_metadata: HashMap<ThreadId, CollabAgentMetadata>,
    pending_collab_spawn_requests: HashMap<String, multi_agents::SpawnRequestSummary>,
    suppressed_exec_calls: HashSet<String>,
    skills_all: Vec<ProtocolSkillMetadata>,
    skills_initial_state: Option<HashMap<PathBuf, bool>>,
    last_unified_wait: Option<UnifiedExecWaitState>,
    unified_exec_wait_streak: Option<UnifiedExecWaitStreak>,
    turn_sleep_inhibitor: SleepInhibitor,
    task_complete_pending: bool,
    unified_exec_processes: Vec<UnifiedExecProcessSummary>,
    /// Tracks whether codex-core currently considers an agent turn to be in progress.
    ///
    /// This is kept separate from `mcp_startup_status` so that MCP startup progress (or completion)
    /// can update the status header without accidentally clearing the spinner for an active turn.
    agent_turn_running: bool,
    /// Tracks per-server MCP startup state while startup is in progress.
    ///
    /// The map is `Some(_)` from the first `McpStartupUpdate` until `McpStartupComplete`, and the
    /// bottom pane is treated as "running" while this is populated, even if no agent turn is
    /// currently executing.
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
    connectors_cache: ConnectorsCacheState,
    connectors_partial_snapshot: Option<ConnectorsSnapshot>,
    connectors_prefetch_in_flight: bool,
    connectors_force_refetch_pending: bool,
    plugins_cache: PluginsCacheState,
    plugins_fetch_state: PluginListFetchState,
    plugin_install_apps_needing_auth: Vec<AppSummary>,
    plugin_install_auth_flow: Option<PluginInstallAuthFlowState>,
    // Queue of interruptive UI events deferred during an active write cycle
    interrupts: InterruptManager,
    // Accumulates the current reasoning block text to extract a header
    reasoning_buffer: String,
    // Accumulates full reasoning content for transcript-only recording
    full_reasoning_buffer: String,
    // The currently rendered footer state. We keep the already-formatted
    // details here so transient stream interruptions can restore the footer
    // exactly as it was shown.
    current_status: StatusIndicatorState,
    // Guardian review keeps its own pending set so it can derive a single
    // footer summary from one or more in-flight review events.
    pending_guardian_review_status: PendingGuardianReviewStatus,
    // Active hook runs render in a dedicated live cell so they can run alongside tools.
    active_hook_cell: Option<HookCell>,
    // Semantic status used for terminal-title status rendering.
    terminal_title_status_kind: TerminalTitleStatusKind,
    // Previous status header to restore after a transient stream retry.
    retry_status_header: Option<String>,
    // Set when commentary output completes; once stream queues go idle we restore the status row.
    pending_status_indicator_restore: bool,
    suppress_queue_autosend: bool,
    thread_id: Option<ThreadId>,
    last_turn_id: Option<String>,
    thread_name: Option<String>,
    forked_from: Option<ThreadId>,
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
    // User messages queued while a turn is in progress
    queued_user_messages: VecDeque<UserMessage>,
    // User messages that tried to steer a non-regular turn and must be retried first.
    rejected_steers_queue: VecDeque<UserMessage>,
    // Steers already submitted to core but not yet committed into history.
    //
    // The bottom pane shows these above queued drafts until core records the
    // corresponding user message item.
    pending_steers: VecDeque<PendingSteer>,
    // When set, the next interrupt should resubmit all pending steers as one
    // fresh user turn instead of restoring them into the composer.
    submit_pending_steers_after_interrupt: bool,
    /// Terminal-appropriate keybinding for popping the most-recently queued
    /// message back into the composer.  Determined once at construction time via
    /// [`queued_message_edit_binding_for_terminal`] and propagated to
    /// `BottomPane` so the hint text matches the actual shortcut.
    queued_message_edit_binding: KeyBinding,
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
    // Simple review mode flag; used to adjust layout and banners.
    is_review_mode: bool,
    // Snapshot of token usage to restore after review mode exits.
    pre_review_token_info: Option<Option<TokenUsageInfo>>,
    // Whether the next streamed assistant content should be preceded by a final message separator.
    //
    // This is set whenever we insert a visible history cell that conceptually belongs to a turn.
    // The separator itself is only rendered if the turn recorded "work" activity (see
    // `had_work_activity`).
    needs_final_message_separator: bool,
    // Whether the current turn performed "work" (exec commands, MCP tool calls, patch applications).
    //
    // This gates rendering of the "Worked for …" separator so purely conversational turns don't
    // show an empty divider. It is reset when the separator is emitted.
    had_work_activity: bool,
    // Whether the current turn emitted a plan update.
    saw_plan_update_this_turn: bool,
    // Whether the current turn emitted a proposed plan item that has not been superseded by a
    // later steer. This is cleared when the user submits a steer so the plan popup only appears
    // if a newer proposed plan arrives afterward.
    saw_plan_item_this_turn: bool,
    // Latest `update_plan` checklist task counts for terminal-title rendering.
    last_plan_progress: Option<(usize, usize)>,
    // Incremental buffer for streamed plan content.
    plan_delta_buffer: String,
    // True while a plan item is streaming.
    plan_item_active: bool,
    // Status-indicator elapsed seconds captured at the last emitted final-message separator.
    //
    // This lets the separator show per-chunk work time (since the previous separator) rather than
    // the total task-running time reported by the status indicator.
    last_separator_elapsed_secs: Option<u64>,
    // Runtime metrics accumulated across delta snapshots for the active turn.
    turn_runtime_metrics: RuntimeMetricsSummary,
    last_rendered_width: std::cell::Cell<Option<usize>>,
    // Feedback sink for /feedback
    feedback: codex_feedback::CodexFeedback,
    // Current session rollout path (if known)
    current_rollout_path: Option<PathBuf>,
    // Current working directory (if known)
    current_cwd: Option<PathBuf>,
    // Runtime network proxy bind addresses from SessionConfigured.
    session_network_proxy: Option<codex_protocol::protocol::SessionNetworkProxyRuntime>,
    // Shared latch so we only warn once about invalid status-line item IDs.
    status_line_invalid_items_warned: Arc<AtomicBool>,
    // Shared latch so we only warn once about invalid terminal-title item IDs.
    terminal_title_invalid_items_warned: Arc<AtomicBool>,
    // Last terminal title emitted, to avoid writing duplicate OSC updates.
    pub(crate) last_terminal_title: Option<String>,
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
    external_editor_state: ExternalEditorState,
    realtime_conversation: RealtimeConversationUiState,
    last_rendered_user_message_event: Option<RenderedUserMessageEvent>,
    last_non_retry_error: Option<(String, String)>,
}

/// Cached nickname and role for a collab agent thread, used to attach human-readable labels to
/// rendered tool-call items.
///
/// Populated externally by `App` via `set_collab_agent_metadata` and consulted by the
/// notification-to-core-event conversion helpers. Defaults to empty so that missing metadata
/// degrades to the previous behavior of showing raw thread ids.
#[derive(Clone, Debug, Default)]
struct CollabAgentMetadata {
    agent_nickname: Option<String>,
    agent_role: Option<String>,
}

#[cfg_attr(not(test), allow(dead_code))]
enum CodexOpTarget {
    Direct(UnboundedSender<Op>),
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UserMessage {
    text: String,
    local_images: Vec<LocalImageAttachment>,
    /// Remote image attachments represented as URLs (for example data URLs)
    /// provided by app-server clients.
    ///
    /// Unlike `local_images`, these are not created by TUI image attach/paste
    /// flows. The TUI can restore and remove them while editing/backtracking.
    remote_image_urls: Vec<String>,
    text_elements: Vec<TextElement>,
    mention_bindings: Vec<MentionBinding>,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct ThreadComposerState {
    text: String,
    local_images: Vec<LocalImageAttachment>,
    remote_image_urls: Vec<String>,
    text_elements: Vec<TextElement>,
    mention_bindings: Vec<MentionBinding>,
    pending_pastes: Vec<(String, String)>,
}

impl ThreadComposerState {
    fn has_content(&self) -> bool {
        !self.text.is_empty()
            || !self.local_images.is_empty()
            || !self.remote_image_urls.is_empty()
            || !self.text_elements.is_empty()
            || !self.mention_bindings.is_empty()
            || !self.pending_pastes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ThreadInputState {
    composer: Option<ThreadComposerState>,
    pending_steers: VecDeque<UserMessage>,
    rejected_steers_queue: VecDeque<UserMessage>,
    queued_user_messages: VecDeque<UserMessage>,
    current_collaboration_mode: CollaborationMode,
    active_collaboration_mask: Option<CollaborationModeMask>,
    task_running: bool,
    agent_turn_running: bool,
}

impl From<String> for UserMessage {
    fn from(text: String) -> Self {
        Self {
            text,
            local_images: Vec::new(),
            remote_image_urls: Vec::new(),
            // Plain text conversion has no UI element ranges.
            text_elements: Vec::new(),
            mention_bindings: Vec::new(),
        }
    }
}

impl From<&str> for UserMessage {
    fn from(text: &str) -> Self {
        Self {
            text: text.to_string(),
            local_images: Vec::new(),
            remote_image_urls: Vec::new(),
            // Plain text conversion has no UI element ranges.
            text_elements: Vec::new(),
            mention_bindings: Vec::new(),
        }
    }
}

struct PendingSteer {
    user_message: UserMessage,
    compare_key: PendingSteerCompareKey,
}

pub(crate) fn create_initial_user_message(
    text: Option<String>,
    local_image_paths: Vec<PathBuf>,
    text_elements: Vec<TextElement>,
) -> Option<UserMessage> {
    let text = text.unwrap_or_default();
    if text.is_empty() && local_image_paths.is_empty() {
        None
    } else {
        let local_images = local_image_paths
            .into_iter()
            .enumerate()
            .map(|(idx, path)| LocalImageAttachment {
                placeholder: local_image_label_text(idx + 1),
                path,
            })
            .collect();
        Some(UserMessage {
            text,
            local_images,
            remote_image_urls: Vec::new(),
            text_elements,
            mention_bindings: Vec::new(),
        })
    }
}

fn append_text_with_rebased_elements(
    target_text: &mut String,
    target_text_elements: &mut Vec<TextElement>,
    text: &str,
    text_elements: impl IntoIterator<Item = TextElement>,
) {
    let offset = target_text.len();
    target_text.push_str(text);
    target_text_elements.extend(text_elements.into_iter().map(|mut element| {
        element.byte_range.start += offset;
        element.byte_range.end += offset;
        element
    }));
}

// When merging multiple queued drafts (e.g., after interrupt), each draft starts numbering
// its attachments at [Image #1]. Reassign placeholder labels based on the attachment list so
// the combined local_image_paths order matches the labels, even if placeholders were moved
// in the text (e.g., [Image #2] appearing before [Image #1]).
fn remap_placeholders_for_message(message: UserMessage, next_label: &mut usize) -> UserMessage {
    let UserMessage {
        text,
        text_elements,
        local_images,
        remote_image_urls,
        mention_bindings,
    } = message;
    if local_images.is_empty() {
        return UserMessage {
            text,
            text_elements,
            local_images,
            remote_image_urls,
            mention_bindings,
        };
    }

    let mut mapping: HashMap<String, String> = HashMap::new();
    let mut remapped_images = Vec::new();
    for attachment in local_images {
        let new_placeholder = local_image_label_text(*next_label);
        *next_label += 1;
        mapping.insert(attachment.placeholder.clone(), new_placeholder.clone());
        remapped_images.push(LocalImageAttachment {
            placeholder: new_placeholder,
            path: attachment.path,
        });
    }

    let mut elements = text_elements;
    elements.sort_by_key(|elem| elem.byte_range.start);

    let mut cursor = 0usize;
    let mut rebuilt = String::new();
    let mut rebuilt_elements = Vec::new();
    for mut elem in elements {
        let start = elem.byte_range.start.min(text.len());
        let end = elem.byte_range.end.min(text.len());
        if let Some(segment) = text.get(cursor..start) {
            rebuilt.push_str(segment);
        }

        let original = text.get(start..end).unwrap_or("");
        let placeholder = elem.placeholder(&text);
        let replacement = placeholder
            .and_then(|ph| mapping.get(ph))
            .map(String::as_str)
            .unwrap_or(original);

        let elem_start = rebuilt.len();
        rebuilt.push_str(replacement);
        let elem_end = rebuilt.len();

        if let Some(remapped) = placeholder.and_then(|ph| mapping.get(ph)) {
            elem.set_placeholder(Some(remapped.clone()));
        }
        elem.byte_range = (elem_start..elem_end).into();
        rebuilt_elements.push(elem);
        cursor = end;
    }
    if let Some(segment) = text.get(cursor..) {
        rebuilt.push_str(segment);
    }

    UserMessage {
        text: rebuilt,
        local_images: remapped_images,
        remote_image_urls,
        text_elements: rebuilt_elements,
        mention_bindings,
    }
}

fn merge_user_messages(messages: Vec<UserMessage>) -> UserMessage {
    let mut combined = UserMessage {
        text: String::new(),
        text_elements: Vec::new(),
        local_images: Vec::new(),
        remote_image_urls: Vec::new(),
        mention_bindings: Vec::new(),
    };
    let total_remote_images = messages
        .iter()
        .map(|message| message.remote_image_urls.len())
        .sum::<usize>();
    let mut next_image_label = total_remote_images + 1;

    for (idx, message) in messages.into_iter().enumerate() {
        if idx > 0 {
            combined.text.push('\n');
        }
        let UserMessage {
            text,
            text_elements,
            local_images,
            remote_image_urls,
            mention_bindings,
        } = remap_placeholders_for_message(message, &mut next_image_label);
        append_text_with_rebased_elements(
            &mut combined.text,
            &mut combined.text_elements,
            &text,
            text_elements,
        );
        combined.local_images.extend(local_images);
        combined.remote_image_urls.extend(remote_image_urls);
        combined.mention_bindings.extend(mention_bindings);
    }

    combined
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplayKind {
    ResumeInitialMessages,
    ThreadSnapshot,
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

fn thread_session_state_to_legacy_event(
    session: ThreadSessionState,
) -> codex_protocol::protocol::SessionConfiguredEvent {
    codex_protocol::protocol::SessionConfiguredEvent {
        session_id: session.thread_id,
        forked_from_id: session.forked_from_id,
        thread_name: session.thread_name,
        model: session.model,
        model_provider_id: session.model_provider_id,
        service_tier: session.service_tier,
        approval_policy: session.approval_policy,
        approvals_reviewer: session.approvals_reviewer,
        sandbox_policy: session.sandbox_policy,
        cwd: session.cwd,
        reasoning_effort: session.reasoning_effort,
        history_log_id: session.history_log_id,
        history_entry_count: usize::try_from(session.history_entry_count).unwrap_or(usize::MAX),
        initial_messages: None,
        network_proxy: session.network_proxy,
        rollout_path: session.rollout_path,
    }
}

fn hook_output_entry_from_notification(
    entry: codex_app_server_protocol::HookOutputEntry,
) -> codex_protocol::protocol::HookOutputEntry {
    codex_protocol::protocol::HookOutputEntry {
        kind: entry.kind.to_core(),
        text: entry.text,
    }
}

fn hook_run_summary_from_notification(
    run: codex_app_server_protocol::HookRunSummary,
) -> codex_protocol::protocol::HookRunSummary {
    codex_protocol::protocol::HookRunSummary {
        id: run.id,
        event_name: run.event_name.to_core(),
        handler_type: run.handler_type.to_core(),
        execution_mode: run.execution_mode.to_core(),
        scope: run.scope.to_core(),
        source_path: run.source_path,
        display_order: run.display_order,
        status: run.status.to_core(),
        status_message: run.status_message,
        started_at: run.started_at,
        completed_at: run.completed_at,
        duration_ms: run.duration_ms,
        entries: run
            .entries
            .into_iter()
            .map(hook_output_entry_from_notification)
            .collect(),
    }
}

fn hook_started_event_from_notification(
    notification: codex_app_server_protocol::HookStartedNotification,
) -> codex_protocol::protocol::HookStartedEvent {
    codex_protocol::protocol::HookStartedEvent {
        turn_id: notification.turn_id,
        run: hook_run_summary_from_notification(notification.run),
    }
}

fn hook_completed_event_from_notification(
    notification: codex_app_server_protocol::HookCompletedNotification,
) -> codex_protocol::protocol::HookCompletedEvent {
    codex_protocol::protocol::HookCompletedEvent {
        turn_id: notification.turn_id,
        run: hook_run_summary_from_notification(notification.run),
    }
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

fn exec_approval_request_from_params(
    params: CommandExecutionRequestApprovalParams,
) -> ExecApprovalRequestEvent {
    ExecApprovalRequestEvent {
        call_id: params.item_id,
        command: params
            .command
            .as_deref()
            .map(split_command_string)
            .unwrap_or_default(),
        cwd: params.cwd.unwrap_or_default(),
        reason: params.reason,
        network_approval_context: params
            .network_approval_context
            .map(network_approval_context_to_core),
        additional_permissions: params.additional_permissions.map(Into::into),
        turn_id: params.turn_id,
        approval_id: params.approval_id,
        proposed_execpolicy_amendment: params
            .proposed_execpolicy_amendment
            .map(codex_app_server_protocol::ExecPolicyAmendment::into_core),
        proposed_network_policy_amendments: params.proposed_network_policy_amendments.map(
            |amendments| {
                amendments
                    .into_iter()
                    .map(codex_app_server_protocol::NetworkPolicyAmendment::into_core)
                    .collect()
            },
        ),
        available_decisions: params.available_decisions.map(|decisions| {
            decisions
                .into_iter()
                .map(|decision| match decision {
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
                })
                .collect()
        }),
        parsed_cmd: params
            .command_actions
            .unwrap_or_default()
            .into_iter()
            .map(codex_app_server_protocol::CommandAction::into_core)
            .collect(),
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

fn app_server_patch_changes_to_core(
    changes: Vec<codex_app_server_protocol::FileUpdateChange>,
) -> HashMap<PathBuf, codex_protocol::protocol::FileChange> {
    changes
        .into_iter()
        .map(|change| {
            let path = PathBuf::from(change.path);
            let file_change = match change.kind {
                codex_app_server_protocol::PatchChangeKind::Add => {
                    codex_protocol::protocol::FileChange::Add {
                        content: change.diff,
                    }
                }
                codex_app_server_protocol::PatchChangeKind::Delete => {
                    codex_protocol::protocol::FileChange::Delete {
                        content: change.diff,
                    }
                }
                codex_app_server_protocol::PatchChangeKind::Update { move_path } => {
                    codex_protocol::protocol::FileChange::Update {
                        unified_diff: change.diff,
                        move_path,
                    }
                }
            };
            (path, file_change)
        })
        .collect()
}

fn app_server_collab_thread_id_to_core(thread_id: &str) -> Option<ThreadId> {
    match ThreadId::from_string(thread_id) {
        Ok(thread_id) => Some(thread_id),
        Err(err) => {
            warn!("ignoring collab tool-call item with invalid thread id {thread_id}: {err}");
            None
        }
    }
}

fn app_server_collab_state_to_core(state: &AppServerCollabAgentState) -> AgentStatus {
    match state.status {
        AppServerCollabAgentStatus::PendingInit => AgentStatus::PendingInit,
        AppServerCollabAgentStatus::Running => AgentStatus::Running,
        AppServerCollabAgentStatus::Interrupted => AgentStatus::Interrupted,
        AppServerCollabAgentStatus::Completed => AgentStatus::Completed(state.message.clone()),
        AppServerCollabAgentStatus::Errored => AgentStatus::Errored(
            state
                .message
                .clone()
                .unwrap_or_else(|| "Agent errored".into()),
        ),
        AppServerCollabAgentStatus::Shutdown => AgentStatus::Shutdown,
        AppServerCollabAgentStatus::NotFound => AgentStatus::NotFound,
    }
}

/// Converts app-server collab agent states into the core protocol representation, enriching each
/// entry with cached nickname and role metadata so rendered items show human-readable names.
fn app_server_collab_agent_statuses_to_core(
    receiver_thread_ids: &[String],
    agents_states: &HashMap<String, AppServerCollabAgentState>,
    collab_agent_metadata: &HashMap<ThreadId, CollabAgentMetadata>,
) -> (Vec<CollabAgentStatusEntry>, HashMap<ThreadId, AgentStatus>) {
    let mut agent_statuses = Vec::new();
    let mut statuses = HashMap::new();

    for receiver_thread_id in receiver_thread_ids {
        let Some(thread_id) = app_server_collab_thread_id_to_core(receiver_thread_id) else {
            continue;
        };
        let Some(agent_state) = agents_states.get(receiver_thread_id) else {
            continue;
        };
        let status = app_server_collab_state_to_core(agent_state);
        let metadata = collab_agent_metadata
            .get(&thread_id)
            .cloned()
            .unwrap_or_default();
        agent_statuses.push(CollabAgentStatusEntry {
            thread_id,
            agent_nickname: metadata.agent_nickname,
            agent_role: metadata.agent_role,
            status: status.clone(),
        });
        statuses.insert(thread_id, status);
    }

    (agent_statuses, statuses)
}

/// Builds `CollabAgentRef` entries for every valid receiver thread, attaching cached metadata.
///
/// Used when converting collab `Wait` tool-call items so the rendered waiting list shows agent
/// names instead of bare thread ids.
fn app_server_collab_receiver_agent_refs(
    receiver_thread_ids: &[String],
    collab_agent_metadata: &HashMap<ThreadId, CollabAgentMetadata>,
) -> Vec<CollabAgentRef> {
    receiver_thread_ids
        .iter()
        .filter_map(|thread_id| {
            let thread_id = app_server_collab_thread_id_to_core(thread_id)?;
            let metadata = collab_agent_metadata
                .get(&thread_id)
                .cloned()
                .unwrap_or_default();
            Some(CollabAgentRef {
                thread_id,
                agent_nickname: metadata.agent_nickname,
                agent_role: metadata.agent_role,
            })
        })
        .collect()
}

fn request_permissions_from_params(
    params: codex_app_server_protocol::PermissionsRequestApprovalParams,
) -> RequestPermissionsEvent {
    RequestPermissionsEvent {
        turn_id: params.turn_id,
        call_id: params.item_id,
        reason: params.reason,
        permissions: params.permissions.into(),
    }
}

fn request_user_input_from_params(params: ToolRequestUserInputParams) -> RequestUserInputEvent {
    RequestUserInputEvent {
        turn_id: params.turn_id,
        call_id: params.item_id,
        questions: params
            .questions
            .into_iter()
            .map(
                |question| codex_protocol::request_user_input::RequestUserInputQuestion {
                    id: question.id,
                    header: question.header,
                    question: question.question,
                    is_other: question.is_other,
                    is_secret: question.is_secret,
                    options: question.options.map(|options| {
                        options
                            .into_iter()
                            .map(|option| RequestUserInputQuestionOption {
                                label: option.label,
                                description: option.description,
                            })
                            .collect()
                    }),
                },
            )
            .collect(),
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

fn web_search_action_to_core(
    action: codex_app_server_protocol::WebSearchAction,
) -> codex_protocol::models::WebSearchAction {
    match action {
        codex_app_server_protocol::WebSearchAction::Search { query, queries } => {
            codex_protocol::models::WebSearchAction::Search { query, queries }
        }
        codex_app_server_protocol::WebSearchAction::OpenPage { url } => {
            codex_protocol::models::WebSearchAction::OpenPage { url }
        }
        codex_app_server_protocol::WebSearchAction::FindInPage { url, pattern } => {
            codex_protocol::models::WebSearchAction::FindInPage { url, pattern }
        }
        codex_app_server_protocol::WebSearchAction::Other => {
            codex_protocol::models::WebSearchAction::Other
        }
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
            CollabAgentMetadata {
                agent_nickname,
                agent_role,
            },
        );
    }

    /// Returns the cached metadata for a thread, defaulting to empty if none has been registered.
    fn collab_agent_metadata(&self, thread_id: ThreadId) -> CollabAgentMetadata {
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

    /// Synchronize the bottom-pane "task running" indicator with the current lifecycles.
    ///
    /// The bottom pane only has one running flag, but this module treats it as a derived state of
    /// both the agent turn lifecycle and MCP startup lifecycle.
    fn update_task_running_state(&mut self) {
        self.bottom_pane
            .set_task_running(self.agent_turn_running || self.mcp_startup_status.is_some());
        self.refresh_terminal_title();
    }

    fn restore_reasoning_status_header(&mut self) {
        if let Some(header) = extract_first_bold(&self.reasoning_buffer) {
            self.terminal_title_status_kind = TerminalTitleStatusKind::Thinking;
            self.set_status_header(header);
        } else if self.bottom_pane.is_task_running() {
            self.terminal_title_status_kind = TerminalTitleStatusKind::Working;
            self.set_status_header(String::from("Working"));
        }
    }

    fn flush_unified_exec_wait_streak(&mut self) {
        let Some(wait) = self.unified_exec_wait_streak.take() else {
            return;
        };
        self.needs_final_message_separator = true;
        let cell = history_cell::new_unified_exec_interaction(wait.command_display, String::new());
        self.app_event_tx
            .send(AppEvent::InsertHistoryCell(Box::new(cell)));
        self.restore_reasoning_status_header();
    }

    fn flush_answer_stream_with_separator(&mut self) {
        if let Some(mut controller) = self.stream_controller.take()
            && let Some(cell) = controller.finalize()
        {
            self.add_boxed_history(cell);
        }
        self.adaptive_chunking.reset();
    }

    fn stream_controllers_idle(&self) -> bool {
        self.stream_controller
            .as_ref()
            .map(|controller| controller.queued_lines() == 0)
            .unwrap_or(true)
            && self
                .plan_stream_controller
                .as_ref()
                .map(|controller| controller.queued_lines() == 0)
                .unwrap_or(true)
    }

    /// Restore the status indicator only after commentary completion is pending,
    /// the turn is still running, and all stream queues have drained.
    ///
    /// This gate prevents flicker while normal output is still actively
    /// streaming, but still restores a visible "working" affordance when a
    /// commentary block ends before the turn itself has completed.
    fn maybe_restore_status_indicator_after_stream_idle(&mut self) {
        if !self.pending_status_indicator_restore
            || !self.bottom_pane.is_task_running()
            || !self.stream_controllers_idle()
        {
            return;
        }

        self.bottom_pane.ensure_status_indicator();
        self.set_status(
            self.current_status.header.clone(),
            self.current_status.details.clone(),
            StatusDetailsCapitalization::Preserve,
            self.current_status.details_max_lines,
        );
        self.pending_status_indicator_restore = false;
    }

    /// Update the status indicator header and details.
    ///
    /// Passing `None` clears any existing details.
    fn set_status(
        &mut self,
        header: String,
        details: Option<String>,
        details_capitalization: StatusDetailsCapitalization,
        details_max_lines: usize,
    ) {
        let details = details
            .filter(|details| !details.is_empty())
            .map(|details| {
                let trimmed = details.trim_start();
                match details_capitalization {
                    StatusDetailsCapitalization::CapitalizeFirst => {
                        crate::text_formatting::capitalize_first(trimmed)
                    }
                    StatusDetailsCapitalization::Preserve => trimmed.to_string(),
                }
            });
        self.current_status = StatusIndicatorState {
            header: header.clone(),
            details: details.clone(),
            details_max_lines,
        };
        self.bottom_pane.update_status(
            header,
            details,
            StatusDetailsCapitalization::Preserve,
            details_max_lines,
        );
        let title_uses_status = self
            .config
            .tui_terminal_title
            .as_ref()
            .is_some_and(|items| items.iter().any(|item| item == "status"));
        let title_uses_spinner = self
            .config
            .tui_terminal_title
            .as_ref()
            .is_none_or(|items| items.iter().any(|item| item == "spinner"));
        if title_uses_status
            || (title_uses_spinner
                && self.terminal_title_status_kind == TerminalTitleStatusKind::Undoing)
        {
            self.refresh_terminal_title();
        }
    }

    /// Convenience wrapper around [`Self::set_status`];
    /// updates the status indicator header and clears any existing details.
    fn set_status_header(&mut self, header: String) {
        self.set_status(
            header,
            /*details*/ None,
            StatusDetailsCapitalization::CapitalizeFirst,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
    }

    /// Sets the currently rendered footer status-line value.
    pub(crate) fn set_status_line(&mut self, status_line: Option<Line<'static>>) {
        self.bottom_pane.set_status_line(status_line);
    }

    /// Forwards the contextual active-agent label into the bottom-pane footer pipeline.
    ///
    /// `ChatWidget` stays a pass-through here so `App` remains the owner of "which thread is the
    /// user actually looking at?" and the footer stack remains a pure renderer of that decision.
    pub(crate) fn set_active_agent_label(&mut self, active_agent_label: Option<String>) {
        self.bottom_pane.set_active_agent_label(active_agent_label);
    }

    /// Recomputes footer status-line content from config and current runtime state.
    ///
    /// This method is the status-line orchestrator: it parses configured item identifiers,
    /// warns once per session about invalid items, updates whether status-line mode is enabled,
    /// schedules async git-branch lookup when needed, and renders only values that are currently
    /// available.
    ///
    /// The omission behavior is intentional. If selected items are unavailable (for example before
    /// a session id exists or before branch lookup completes), those items are skipped without
    /// placeholders so the line remains compact and stable.
    pub(crate) fn refresh_status_line(&mut self) {
        self.refresh_status_surfaces();
    }

    /// Records that status-line setup was canceled.
    ///
    /// Cancellation is intentionally side-effect free for config state; the existing configuration
    /// remains active and no persistence is attempted.
    pub(crate) fn cancel_status_line_setup(&self) {
        tracing::info!("Status line setup canceled by user");
    }

    /// Applies status-line item selection from the setup view to in-memory config.
    ///
    /// An empty selection persists as an explicit empty list.
    pub(crate) fn setup_status_line(&mut self, items: Vec<StatusLineItem>) {
        tracing::info!("status line setup confirmed with items: {items:#?}");
        let ids = items.iter().map(ToString::to_string).collect::<Vec<_>>();
        self.config.tui_status_line = Some(ids);
        self.refresh_status_line();
    }

    /// Applies a temporary terminal-title selection while the setup UI is open.
    pub(crate) fn preview_terminal_title(&mut self, items: Vec<TerminalTitleItem>) {
        if self.terminal_title_setup_original_items.is_none() {
            self.terminal_title_setup_original_items = Some(self.config.tui_terminal_title.clone());
        }

        let ids = items.iter().map(ToString::to_string).collect::<Vec<_>>();
        self.config.tui_terminal_title = Some(ids);
        self.refresh_terminal_title();
    }

    /// Restores the terminal-title config that was active before the setup UI
    /// opened, undoing any preview changes. No-op if no setup session is active.
    pub(crate) fn revert_terminal_title_setup_preview(&mut self) {
        let Some(original_items) = self.terminal_title_setup_original_items.take() else {
            return;
        };

        self.config.tui_terminal_title = original_items;
        self.refresh_terminal_title();
    }

    /// Dismisses the terminal-title setup UI and reverts to the pre-setup config.
    pub(crate) fn cancel_terminal_title_setup(&mut self) {
        tracing::info!("Terminal title setup canceled by user");
        self.revert_terminal_title_setup_preview();
    }

    /// Commits a confirmed terminal-title selection, ending the setup session.
    ///
    /// After this call, `revert_terminal_title_setup_preview` becomes a no-op
    /// because the original config snapshot is discarded.
    pub(crate) fn setup_terminal_title(&mut self, items: Vec<TerminalTitleItem>) {
        tracing::info!("terminal title setup confirmed with items: {items:#?}");
        let ids = items.iter().map(ToString::to_string).collect::<Vec<_>>();
        self.terminal_title_setup_original_items = None;
        self.config.tui_terminal_title = Some(ids);
        self.refresh_terminal_title();
    }

    /// Stores async git-branch lookup results for the current status-line cwd.
    ///
    /// Results are dropped when they target an out-of-date cwd to avoid rendering stale branch
    /// names after directory changes.
    pub(crate) fn set_status_line_branch(&mut self, cwd: PathBuf, branch: Option<String>) {
        if self.status_line_branch_cwd.as_ref() != Some(&cwd) {
            self.status_line_branch_pending = false;
            return;
        }
        self.status_line_branch = branch;
        self.status_line_branch_pending = false;
        self.status_line_branch_lookup_complete = true;
        self.refresh_status_surfaces();
    }

    fn collect_runtime_metrics_delta(&mut self) {
        if let Some(delta) = self.session_telemetry.runtime_metrics_summary() {
            self.apply_runtime_metrics_delta(delta);
        }
    }

    fn apply_runtime_metrics_delta(&mut self, delta: RuntimeMetricsSummary) {
        let should_log_timing = has_websocket_timing_metrics(delta);
        self.turn_runtime_metrics.merge(delta);
        if should_log_timing {
            self.log_websocket_timing_totals(delta);
        }
    }

    fn log_websocket_timing_totals(&mut self, delta: RuntimeMetricsSummary) {
        if let Some(label) = history_cell::runtime_metrics_label(delta.responses_api_summary()) {
            self.add_plain_history_lines(vec![
                vec!["• ".dim(), format!("WebSocket timing: {label}").dark_gray()].into(),
            ]);
        }
    }

    fn refresh_runtime_metrics(&mut self) {
        self.collect_runtime_metrics_delta();
    }

    fn restore_retry_status_header_if_present(&mut self) {
        if let Some(header) = self.retry_status_header.take() {
            self.set_status_header(header);
        }
    }

    /// Record or update the raw markdown for the current agent turn.
    fn record_agent_markdown(&mut self, message: &str) {
        if message.is_empty() {
            return;
        }
        self.last_agent_markdown = Some(message.to_string());
        self.saw_copy_source_this_turn = true;
    }

    // --- Small event handlers ---
    fn on_session_configured(&mut self, event: codex_protocol::protocol::SessionConfiguredEvent) {
        self.last_agent_markdown = None;
        self.saw_copy_source_this_turn = false;
        self.bottom_pane
            .set_history_metadata(event.history_log_id, event.history_entry_count);
        self.set_skills(/*skills*/ None);
        self.session_network_proxy = event.network_proxy.clone();
        self.thread_id = Some(event.session_id);
        self.last_turn_id = None;
        self.thread_name = event.thread_name.clone();
        self.forked_from = event.forked_from_id;
        self.current_rollout_path = event.rollout_path.clone();
        self.current_cwd = Some(event.cwd.clone());
        match AbsolutePathBuf::try_from(event.cwd.clone()) {
            Ok(cwd) => self.config.cwd = cwd,
            Err(err) => {
                tracing::warn!(path = %event.cwd.display(), %err, "session cwd should be absolute");
            }
        }
        if let Err(err) = self
            .config
            .permissions
            .approval_policy
            .set(event.approval_policy)
        {
            tracing::warn!(%err, "failed to sync approval_policy from SessionConfigured");
            self.config.permissions.approval_policy =
                Constrained::allow_only(event.approval_policy);
        }
        if let Err(err) = self
            .config
            .permissions
            .sandbox_policy
            .set(event.sandbox_policy.clone())
        {
            tracing::warn!(%err, "failed to sync sandbox_policy from SessionConfigured");
            self.config.permissions.sandbox_policy =
                Constrained::allow_only(event.sandbox_policy.clone());
        }
        self.config.approvals_reviewer = event.approvals_reviewer;
        self.status_line_project_root_name_cache = None;
        let forked_from_id = event.forked_from_id;
        let model_for_header = event.model.clone();
        self.session_header.set_model(&model_for_header);
        self.current_collaboration_mode = self.current_collaboration_mode.with_updates(
            Some(model_for_header.clone()),
            Some(event.reasoning_effort),
            /*developer_instructions*/ None,
        );
        if let Some(mask) = self.active_collaboration_mask.as_mut() {
            mask.model = Some(model_for_header.clone());
            mask.reasoning_effort = Some(event.reasoning_effort);
        }
        self.refresh_model_display();
        self.refresh_status_surfaces();
        self.sync_fast_command_enabled();
        self.sync_personality_command_enabled();
        self.sync_plugins_command_enabled();
        self.refresh_plugin_mentions();
        let startup_tooltip_override = self.startup_tooltip_override.take();
        let show_fast_status = self.should_show_fast_status(&model_for_header, event.service_tier);
        #[cfg(test)]
        let initial_messages = event.initial_messages.clone();
        let session_info_cell = history_cell::new_session_info(
            &self.config,
            &model_for_header,
            event,
            self.show_welcome_banner,
            startup_tooltip_override,
            self.plan_type,
            show_fast_status,
        );
        self.apply_session_info_cell(session_info_cell);

        #[cfg(test)]
        if let Some(messages) = initial_messages {
            self.replay_initial_messages(messages);
        }
        self.saw_copy_source_this_turn = false;
        self.submit_op(AppCommand::list_skills(
            Vec::new(),
            /*force_reload*/ true,
        ));
        if self.connectors_enabled() {
            self.prefetch_connectors();
        }
        if let Some(user_message) = self.initial_user_message.take() {
            if self.suppress_initial_user_message_submit {
                self.initial_user_message = Some(user_message);
            } else {
                self.submit_user_message(user_message);
            }
        }
        if let Some(forked_from_id) = forked_from_id {
            self.emit_forked_thread_event(forked_from_id);
        }
        if !self.suppress_session_configured_redraw {
            self.request_redraw();
        }
    }

    pub(crate) fn set_initial_user_message_submit_suppressed(&mut self, suppressed: bool) {
        self.suppress_initial_user_message_submit = suppressed;
    }

    pub(crate) fn submit_initial_user_message_if_pending(&mut self) {
        if let Some(user_message) = self.initial_user_message.take() {
            self.submit_user_message(user_message);
        }
    }

    pub(crate) fn handle_thread_session(&mut self, session: ThreadSessionState) {
        self.on_session_configured(thread_session_state_to_legacy_event(session));
    }

    fn emit_forked_thread_event(&self, forked_from_id: ThreadId) {
        let app_event_tx = self.app_event_tx.clone();
        let codex_home = self.config.codex_home.clone();
        tokio::spawn(async move {
            let forked_from_id_text = forked_from_id.to_string();
            let send_name_and_id = |name: String| {
                let line: Line<'static> = vec![
                    "• ".dim(),
                    "Thread forked from ".into(),
                    name.cyan(),
                    " (".into(),
                    forked_from_id_text.clone().cyan(),
                    ")".into(),
                ]
                .into();
                app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                    PlainHistoryCell::new(vec![line]),
                )));
            };
            let send_id_only = || {
                let line: Line<'static> = vec![
                    "• ".dim(),
                    "Thread forked from ".into(),
                    forked_from_id_text.clone().cyan(),
                ]
                .into();
                app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                    PlainHistoryCell::new(vec![line]),
                )));
            };

            match find_thread_name_by_id(&codex_home, &forked_from_id).await {
                Ok(Some(name)) if !name.trim().is_empty() => {
                    send_name_and_id(name);
                }
                Ok(_) => send_id_only(),
                Err(err) => {
                    tracing::warn!("Failed to read forked thread name: {err}");
                    send_id_only();
                }
            }
        });
    }

    fn on_thread_name_updated(&mut self, event: codex_protocol::protocol::ThreadNameUpdatedEvent) {
        if self.thread_id == Some(event.thread_id) {
            if let Some(name) = event.thread_name.as_deref() {
                let cell = Self::rename_confirmation_cell(name, self.thread_id);
                self.add_boxed_history(Box::new(cell));
            }
            self.thread_name = event.thread_name;
            self.refresh_terminal_title();
            self.refresh_status_surfaces();
            self.request_redraw();
        }
    }

    fn set_skills(&mut self, skills: Option<Vec<SkillMetadata>>) {
        self.bottom_pane.set_skills(skills);
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
            self.last_turn_id.clone(),
            self.app_event_tx.clone(),
            include_logs,
        );
        self.bottom_pane.show_view(Box::new(view));
        self.request_redraw();
    }

    pub(crate) fn open_app_link_view(&mut self, params: crate::bottom_pane::AppLinkViewParams) {
        let view = crate::bottom_pane::AppLinkView::new(params, self.app_event_tx.clone());
        self.bottom_pane.show_view(Box::new(view));
        self.request_redraw();
    }

    pub(crate) fn open_feedback_consent(&mut self, category: crate::app_event::FeedbackCategory) {
        let snapshot = self.feedback.snapshot(self.thread_id);
        let params = crate::bottom_pane::feedback_upload_consent_params(
            self.app_event_tx.clone(),
            category,
            self.current_rollout_path.clone(),
            snapshot.feedback_diagnostics(),
        );
        self.bottom_pane.show_selection_view(params);
        self.request_redraw();
    }

    fn finalize_completed_assistant_message(&mut self, message: Option<&str>) {
        // If we have a stream_controller, the finalized message payload is redundant because the
        // visible content has already been accumulated through deltas.
        if self.stream_controller.is_none()
            && let Some(message) = message
            && !message.is_empty()
        {
            self.handle_streaming_delta(message.to_string());
        }
        self.flush_answer_stream_with_separator();
        self.handle_stream_finished();
        self.request_redraw();
    }

    fn on_agent_message(&mut self, message: String) {
        self.finalize_completed_assistant_message(Some(&message));
    }

    fn on_context_compacted(&mut self) {
        self.flush_answer_stream_with_separator();
        self.handle_stream_finished();
        self.add_to_history(history_cell::new_info_event(
            "Context compacted".to_owned(),
            /*hint*/ None,
        ));
        self.request_redraw();
    }

    fn on_agent_message_delta(&mut self, delta: String) {
        self.handle_streaming_delta(delta);
    }

    fn on_plan_delta(&mut self, delta: String) {
        if self.active_mode_kind() != ModeKind::Plan {
            return;
        }
        if !self.plan_item_active {
            self.plan_item_active = true;
            self.plan_delta_buffer.clear();
        }
        self.plan_delta_buffer.push_str(&delta);
        // Before streaming plan content, flush any active exec cell group.
        self.flush_unified_exec_wait_streak();
        self.flush_active_cell();

        if self.plan_stream_controller.is_none() {
            self.plan_stream_controller = Some(PlanStreamController::new(
                self.last_rendered_width.get().map(|w| w.saturating_sub(4)),
                &self.config.cwd,
            ));
        }
        if let Some(controller) = self.plan_stream_controller.as_mut()
            && controller.push(&delta)
        {
            self.app_event_tx.send(AppEvent::StartCommitAnimation);
            self.run_catch_up_commit_tick();
        }
        self.request_redraw();
    }

    fn on_plan_item_completed(&mut self, text: String) {
        let streamed_plan = self.plan_delta_buffer.trim().to_string();
        let plan_text = if text.trim().is_empty() {
            streamed_plan
        } else {
            text
        };
        if !plan_text.trim().is_empty() {
            self.record_agent_markdown(&plan_text);
        }
        // Plan commit ticks can hide the status row; remember whether we streamed plan output so
        // completion can restore it once stream queues are idle.
        let should_restore_after_stream = self.plan_stream_controller.is_some();
        self.plan_delta_buffer.clear();
        self.plan_item_active = false;
        self.saw_plan_item_this_turn = true;
        let finalized_streamed_cell =
            if let Some(mut controller) = self.plan_stream_controller.take() {
                controller.finalize()
            } else {
                None
            };
        if let Some(cell) = finalized_streamed_cell {
            self.add_boxed_history(cell);
            // TODO: Replace streamed output with the final plan item text if plan streaming is
            // removed or if we need to reconcile mismatches between streamed and final content.
        } else if !plan_text.is_empty() {
            self.add_to_history(history_cell::new_proposed_plan(plan_text, &self.config.cwd));
        }
        if should_restore_after_stream {
            self.pending_status_indicator_restore = true;
            self.maybe_restore_status_indicator_after_stream_idle();
        }
    }

    fn on_agent_reasoning_delta(&mut self, delta: String) {
        // For reasoning deltas, do not stream to history. Accumulate the
        // current reasoning block and extract the first bold element
        // (between **/**) as the chunk header. Show this header as status.
        self.reasoning_buffer.push_str(&delta);

        if self.unified_exec_wait_streak.is_some() {
            // Unified exec waiting should take precedence over reasoning-derived status headers.
            self.request_redraw();
            return;
        }

        if let Some(header) = extract_first_bold(&self.reasoning_buffer) {
            // Update the shimmer header to the extracted reasoning chunk header.
            self.terminal_title_status_kind = TerminalTitleStatusKind::Thinking;
            self.set_status_header(header);
        } else {
            // Fallback while we don't yet have a bold header: leave existing header as-is.
        }
        self.request_redraw();
    }

    fn on_agent_reasoning_final(&mut self) {
        // At the end of a reasoning block, record transcript-only content.
        self.full_reasoning_buffer.push_str(&self.reasoning_buffer);
        if !self.full_reasoning_buffer.is_empty() {
            let cell = history_cell::new_reasoning_summary_block(
                self.full_reasoning_buffer.clone(),
                &self.config.cwd,
            );
            self.add_boxed_history(cell);
        }
        self.reasoning_buffer.clear();
        self.full_reasoning_buffer.clear();
        self.request_redraw();
    }

    fn on_reasoning_section_break(&mut self) {
        // Start a new reasoning block for header extraction and accumulate transcript.
        self.full_reasoning_buffer.push_str(&self.reasoning_buffer);
        self.full_reasoning_buffer.push_str("\n\n");
        self.reasoning_buffer.clear();
    }

    // Raw reasoning uses the same flow as summarized reasoning

    fn on_task_started(&mut self) {
        self.agent_turn_running = true;
        self.turn_sleep_inhibitor
            .set_turn_running(/*turn_running*/ true);
        self.saw_copy_source_this_turn = false;
        self.saw_plan_update_this_turn = false;
        self.saw_plan_item_this_turn = false;
        self.last_plan_progress = None;
        self.plan_delta_buffer.clear();
        self.plan_item_active = false;
        self.adaptive_chunking.reset();
        self.plan_stream_controller = None;
        self.turn_runtime_metrics = RuntimeMetricsSummary::default();
        self.session_telemetry.reset_runtime_metrics();
        self.bottom_pane.clear_quit_shortcut_hint();
        self.quit_shortcut_expires_at = None;
        self.quit_shortcut_key = None;
        self.update_task_running_state();
        self.retry_status_header = None;
        if self.active_hook_cell.take().is_some() {
            self.bump_active_cell_revision();
        }
        self.pending_status_indicator_restore = false;
        self.bottom_pane
            .set_interrupt_hint_visible(/*visible*/ true);
        self.terminal_title_status_kind = TerminalTitleStatusKind::Working;
        self.set_status_header(String::from("Working"));
        self.full_reasoning_buffer.clear();
        self.reasoning_buffer.clear();
        self.request_redraw();
    }

    fn on_task_complete(&mut self, last_agent_message: Option<String>, from_replay: bool) {
        self.submit_pending_steers_after_interrupt = false;
        // Use `last_agent_message` from the turn-complete notification as the copy
        // source only when no earlier item-level event (AgentMessageItem, plan
        // commit, review output) already recorded markdown for this turn. This
        // prevents the final summary from overwriting a more specific source.
        if let Some(message) = last_agent_message
            .as_ref()
            .filter(|message| !message.is_empty())
            && !self.saw_copy_source_this_turn
        {
            self.record_agent_markdown(message);
        }
        // For desktop notifications: prefer the notification payload, fall back to
        // the item-level copy source if present, otherwise send an empty string.
        let notification_response = last_agent_message
            .as_ref()
            .filter(|message| !message.is_empty())
            .cloned()
            .or_else(|| {
                if self.saw_copy_source_this_turn {
                    self.last_agent_markdown.clone()
                } else {
                    None
                }
            })
            .unwrap_or_default();
        self.saw_copy_source_this_turn = false;
        // If a stream is currently active, finalize it.
        self.flush_answer_stream_with_separator();
        if let Some(mut controller) = self.plan_stream_controller.take()
            && let Some(cell) = controller.finalize()
        {
            self.add_boxed_history(cell);
        }
        self.flush_unified_exec_wait_streak();
        if !from_replay {
            self.collect_runtime_metrics_delta();
            let runtime_metrics =
                (!self.turn_runtime_metrics.is_empty()).then_some(self.turn_runtime_metrics);
            let show_work_separator = self.needs_final_message_separator && self.had_work_activity;
            if show_work_separator || runtime_metrics.is_some() {
                let elapsed_seconds = if show_work_separator {
                    self.bottom_pane
                        .status_widget()
                        .map(super::status_indicator_widget::StatusIndicatorWidget::elapsed_seconds)
                        .map(|current| self.worked_elapsed_from(current))
                } else {
                    None
                };
                self.add_to_history(history_cell::FinalMessageSeparator::new(
                    elapsed_seconds,
                    runtime_metrics,
                ));
            }
            self.turn_runtime_metrics = RuntimeMetricsSummary::default();
            self.needs_final_message_separator = false;
            self.had_work_activity = false;
            self.request_status_line_branch_refresh();
        }
        // Mark task stopped and request redraw now that all content is in history.
        self.pending_status_indicator_restore = false;
        self.agent_turn_running = false;
        self.turn_sleep_inhibitor
            .set_turn_running(/*turn_running*/ false);
        self.update_task_running_state();
        self.running_commands.clear();
        self.suppressed_exec_calls.clear();
        self.last_unified_wait = None;
        self.unified_exec_wait_streak = None;
        self.request_redraw();

        let had_pending_steers = !self.pending_steers.is_empty();
        self.refresh_pending_input_preview();

        if !from_replay && !self.has_queued_follow_up_messages() && !had_pending_steers {
            self.maybe_prompt_plan_implementation();
        }
        // Keep this flag for replayed completion events so a subsequent live TurnComplete can
        // still show the prompt once after thread switch replay.
        if !from_replay {
            self.saw_plan_item_this_turn = false;
        }
        // If there is a queued user message, send exactly one now to begin the next turn.
        self.maybe_send_next_queued_input();
        // Emit a notification when the turn completes (suppressed if focused).
        self.notify(Notification::AgentTurnComplete {
            response: notification_response,
        });

        self.maybe_show_pending_rate_limit_prompt();
    }

    fn maybe_prompt_plan_implementation(&mut self) {
        if !self.collaboration_modes_enabled() {
            return;
        }
        if self.has_queued_follow_up_messages() {
            return;
        }
        if self.active_mode_kind() != ModeKind::Plan {
            return;
        }
        if !self.saw_plan_item_this_turn {
            return;
        }
        if !self.bottom_pane.no_modal_or_popup_active() {
            return;
        }

        if matches!(
            self.rate_limit_switch_prompt,
            RateLimitSwitchPromptState::Pending
        ) {
            return;
        }

        self.open_plan_implementation_prompt();
    }

    fn open_plan_implementation_prompt(&mut self) {
        let default_mask = collaboration_modes::default_mode_mask(self.model_catalog.as_ref());
        let (implement_actions, implement_disabled_reason) = match default_mask {
            Some(mask) => {
                let user_text = PLAN_IMPLEMENTATION_CODING_MESSAGE.to_string();
                let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                    tx.send(AppEvent::SubmitUserMessageWithMode {
                        text: user_text.clone(),
                        collaboration_mode: mask.clone(),
                    });
                })];
                (actions, None)
            }
            None => (Vec::new(), Some("Default mode unavailable".to_string())),
        };
        let items = vec![
            SelectionItem {
                name: PLAN_IMPLEMENTATION_YES.to_string(),
                description: Some("Switch to Default and start coding.".to_string()),
                selected_description: None,
                is_current: false,
                actions: implement_actions,
                disabled_reason: implement_disabled_reason,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: PLAN_IMPLEMENTATION_NO.to_string(),
                description: Some("Continue planning with the model.".to_string()),
                selected_description: None,
                is_current: false,
                actions: Vec::new(),
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some(PLAN_IMPLEMENTATION_TITLE.to_string()),
            subtitle: None,
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
        self.notify(Notification::PlanModePrompt {
            title: PLAN_IMPLEMENTATION_TITLE.to_string(),
        });
    }

    fn has_queued_follow_up_messages(&self) -> bool {
        !self.rejected_steers_queue.is_empty() || !self.queued_user_messages.is_empty()
    }

    fn pop_next_queued_user_message(&mut self) -> Option<UserMessage> {
        if self.rejected_steers_queue.is_empty() {
            self.queued_user_messages.pop_front()
        } else {
            Some(merge_user_messages(
                self.rejected_steers_queue.drain(..).collect(),
            ))
        }
    }

    fn pop_latest_queued_user_message(&mut self) -> Option<UserMessage> {
        self.queued_user_messages
            .pop_back()
            .or_else(|| self.rejected_steers_queue.pop_back())
    }

    pub(crate) fn enqueue_rejected_steer(&mut self) -> bool {
        let Some(pending_steer) = self.pending_steers.pop_front() else {
            tracing::warn!(
                "received active-turn-not-steerable error without a matching pending steer"
            );
            return false;
        };
        self.rejected_steers_queue
            .push_back(pending_steer.user_message);
        self.refresh_pending_input_preview();
        true
    }

    #[cfg(test)]
    fn handle_steer_rejected_error(&mut self, codex_error_info: &CoreCodexErrorInfo) -> bool {
        matches!(
            codex_error_info,
            CoreCodexErrorInfo::ActiveTurnNotSteerable { .. }
        ) && self.enqueue_rejected_steer()
    }

    fn handle_app_server_steer_rejected_error(
        &mut self,
        codex_error_info: &AppServerCodexErrorInfo,
    ) -> bool {
        matches!(
            codex_error_info,
            AppServerCodexErrorInfo::ActiveTurnNotSteerable { .. }
        ) && self.enqueue_rejected_steer()
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

    #[cfg(test)]
    fn apply_turn_started_context_window(&mut self, model_context_window: Option<i64>) {
        let info = match self.token_info.take() {
            Some(mut info) => {
                info.model_context_window = model_context_window;
                info
            }
            None => {
                let Some(model_context_window) = model_context_window else {
                    return;
                };
                TokenUsageInfo {
                    total_token_usage: TokenUsage::default(),
                    last_token_usage: TokenUsage::default(),
                    model_context_window: Some(model_context_window),
                }
            }
        };

        self.apply_token_info(info);
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
        if let Some(saved) = self.pre_review_token_info.take() {
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

    pub(crate) fn on_rate_limit_snapshot(&mut self, snapshot: Option<RateLimitSnapshot>) {
        if let Some(mut snapshot) = snapshot {
            let limit_id = snapshot
                .limit_id
                .clone()
                .unwrap_or_else(|| "codex".to_string());
            let limit_label = snapshot
                .limit_name
                .clone()
                .unwrap_or_else(|| limit_id.clone());
            if snapshot.credits.is_none() {
                snapshot.credits = self
                    .rate_limit_snapshots_by_limit_id
                    .get(&limit_id)
                    .and_then(|display| display.credits.as_ref())
                    .map(|credits| CreditsSnapshot {
                        has_credits: credits.has_credits,
                        unlimited: credits.unlimited,
                        balance: credits.balance.clone(),
                    });
            }

            self.plan_type = snapshot.plan_type.or(self.plan_type);

            let is_codex_limit = limit_id.eq_ignore_ascii_case("codex");
            let warnings = if is_codex_limit {
                self.rate_limit_warnings.take_warnings(
                    snapshot
                        .secondary
                        .as_ref()
                        .map(|window| window.used_percent),
                    snapshot
                        .secondary
                        .as_ref()
                        .and_then(|window| window.window_minutes),
                    snapshot.primary.as_ref().map(|window| window.used_percent),
                    snapshot
                        .primary
                        .as_ref()
                        .and_then(|window| window.window_minutes),
                )
            } else {
                vec![]
            };

            let high_usage = is_codex_limit
                && (snapshot
                    .secondary
                    .as_ref()
                    .map(|w| w.used_percent >= RATE_LIMIT_SWITCH_PROMPT_THRESHOLD)
                    .unwrap_or(false)
                    || snapshot
                        .primary
                        .as_ref()
                        .map(|w| w.used_percent >= RATE_LIMIT_SWITCH_PROMPT_THRESHOLD)
                        .unwrap_or(false));

            let has_workspace_credits = snapshot
                .credits
                .as_ref()
                .map(|credits| credits.has_credits)
                .unwrap_or(false);

            if high_usage
                && !has_workspace_credits
                && !self.rate_limit_switch_prompt_hidden()
                && self.current_model() != NUDGE_MODEL_SLUG
                && !matches!(
                    self.rate_limit_switch_prompt,
                    RateLimitSwitchPromptState::Shown
                )
            {
                self.rate_limit_switch_prompt = RateLimitSwitchPromptState::Pending;
            }

            let display =
                rate_limit_snapshot_display_for_limit(&snapshot, limit_label, Local::now());
            self.rate_limit_snapshots_by_limit_id
                .insert(limit_id, display);

            if !warnings.is_empty() {
                for warning in warnings {
                    self.add_to_history(history_cell::new_warning_event(warning));
                }
                self.request_redraw();
            }
        } else {
            self.rate_limit_snapshots_by_limit_id.clear();
        }
        self.refresh_status_line();
    }
    /// Finalize any active exec as failed and stop/clear agent-turn UI state.
    ///
    /// This does not clear MCP startup tracking, because MCP startup can overlap with turn cleanup
    /// and should continue to drive the bottom-pane running indicator while it is in progress.
    fn finalize_turn(&mut self) {
        // Ensure any spinner is replaced by a red ✗ and flushed into history.
        self.finalize_active_cell_as_failed();
        // Reset running state and clear streaming buffers.
        self.agent_turn_running = false;
        self.turn_sleep_inhibitor
            .set_turn_running(/*turn_running*/ false);
        self.update_task_running_state();
        self.running_commands.clear();
        self.suppressed_exec_calls.clear();
        self.last_unified_wait = None;
        self.unified_exec_wait_streak = None;
        self.adaptive_chunking.reset();
        self.stream_controller = None;
        self.plan_stream_controller = None;
        self.pending_status_indicator_restore = false;
        self.request_status_line_branch_refresh();
        self.maybe_show_pending_rate_limit_prompt();
    }

    fn on_server_overloaded_error(&mut self, message: String) {
        self.submit_pending_steers_after_interrupt = false;
        self.finalize_turn();

        let message = if message.trim().is_empty() {
            "Codex is currently experiencing high load.".to_string()
        } else {
            message
        };

        self.add_to_history(history_cell::new_warning_event(message));
        self.request_redraw();
        self.maybe_send_next_queued_input();
    }

    fn on_error(&mut self, message: String) {
        self.submit_pending_steers_after_interrupt = false;
        self.finalize_turn();
        self.add_to_history(history_cell::new_error_event(message));
        self.request_redraw();

        // After an error ends the turn, try sending the next queued input.
        self.maybe_send_next_queued_input();
    }

    fn handle_non_retry_error(
        &mut self,
        message: String,
        codex_error_info: Option<AppServerCodexErrorInfo>,
    ) {
        if codex_error_info
            .as_ref()
            .is_some_and(|info| self.handle_app_server_steer_rejected_error(info))
        {
        } else if let Some(info) = codex_error_info
            .as_ref()
            .and_then(app_server_rate_limit_error_kind)
        {
            match info {
                RateLimitErrorKind::ServerOverloaded => self.on_server_overloaded_error(message),
                RateLimitErrorKind::UsageLimit | RateLimitErrorKind::Generic => {
                    self.on_error(message)
                }
            }
        } else {
            self.on_error(message);
        }
    }

    fn on_warning(&mut self, message: impl Into<String>) {
        self.add_to_history(history_cell::new_warning_event(message.into()));
        self.request_redraw();
    }

    /// Record one MCP startup update, promoting it into either the active startup
    /// round or a buffered "next" round.
    ///
    /// This path has to deal with lossy app-server delivery. After
    /// `finish_mcp_startup()` or `finish_mcp_startup_after_lag()`, we briefly
    /// ignore incoming updates so stale events from the just-finished round do not
    /// reopen startup. While that guard is active we buffer updates for a possible
    /// next round, and only reactivate once the buffered set is coherent enough to
    /// treat as a fresh startup round.
    fn update_mcp_startup_status(
        &mut self,
        server: String,
        status: McpStartupStatus,
        complete_when_settled: bool,
    ) {
        let mut activated_pending_round = false;
        let startup_status = if self.mcp_startup_ignore_updates_until_next_start {
            // Ignore-mode buffers the next plausible round so stale post-finish
            // updates cannot immediately reopen startup. A fresh `Starting`
            // update resets the buffer only if we have not already seen a
            // pending-round `Starting`; this preserves valid interleavings like
            // `alpha: Starting -> alpha: Ready -> beta: Starting`.
            if matches!(status, McpStartupStatus::Starting)
                && !self.mcp_startup_pending_next_round_saw_starting
            {
                self.mcp_startup_pending_next_round.clear();
                self.mcp_startup_allow_terminal_only_next_round = false;
            }
            self.mcp_startup_pending_next_round_saw_starting |=
                matches!(status, McpStartupStatus::Starting);
            self.mcp_startup_pending_next_round.insert(server, status);
            let Some(expected_servers) = &self.mcp_startup_expected_servers else {
                return;
            };
            let saw_full_round = expected_servers.is_empty()
                || expected_servers
                    .iter()
                    .all(|name| self.mcp_startup_pending_next_round.contains_key(name));
            let saw_starting = self
                .mcp_startup_pending_next_round
                .values()
                .any(|state| matches!(state, McpStartupStatus::Starting));
            if !(saw_full_round
                && (saw_starting || self.mcp_startup_allow_terminal_only_next_round))
            {
                return;
            }

            // The buffered map now looks like a complete next round, so promote it
            // to the active round and resume normal completion tracking.
            self.mcp_startup_ignore_updates_until_next_start = false;
            self.mcp_startup_allow_terminal_only_next_round = false;
            self.mcp_startup_pending_next_round_saw_starting = false;
            activated_pending_round = true;
            std::mem::take(&mut self.mcp_startup_pending_next_round)
        } else {
            // Normal path: fold the update into the active round and surface
            // per-server failures immediately.
            let mut startup_status = self.mcp_startup_status.take().unwrap_or_default();
            if let McpStartupStatus::Failed { error } = &status {
                self.on_warning(error);
            }
            startup_status.insert(server, status);
            startup_status
        };
        if activated_pending_round {
            // A promoted buffered round may already contain terminal failures.
            for state in startup_status.values() {
                if let McpStartupStatus::Failed { error } = state {
                    self.on_warning(error);
                }
            }
        }
        self.mcp_startup_status = Some(startup_status);
        self.update_task_running_state();

        // App-server-backed startup completes when every expected server has
        // reported a non-Starting status. Lag handling can force an earlier
        // settle via `finish_mcp_startup_after_lag()`.
        if complete_when_settled
            && let Some(current) = &self.mcp_startup_status
            && let Some(expected_servers) = &self.mcp_startup_expected_servers
            && !current.is_empty()
            && expected_servers
                .iter()
                .all(|name| current.contains_key(name))
            && current
                .values()
                .all(|state| !matches!(state, McpStartupStatus::Starting))
        {
            let mut failed = Vec::new();
            let mut cancelled = Vec::new();
            for (name, state) in current {
                match state {
                    McpStartupStatus::Ready => {}
                    McpStartupStatus::Failed { .. } => failed.push(name.clone()),
                    McpStartupStatus::Cancelled => cancelled.push(name.clone()),
                    McpStartupStatus::Starting => {}
                }
            }
            failed.sort();
            cancelled.sort();
            self.finish_mcp_startup(failed, cancelled);
            return;
        }
        if let Some(current) = &self.mcp_startup_status {
            // Otherwise keep the status header focused on the remaining
            // in-progress servers for the active round.
            let total = current.len();
            let mut starting: Vec<_> = current
                .iter()
                .filter_map(|(name, state)| {
                    if matches!(state, McpStartupStatus::Starting) {
                        Some(name)
                    } else {
                        None
                    }
                })
                .collect();
            starting.sort();
            if let Some(first) = starting.first() {
                let completed = total.saturating_sub(starting.len());
                let max_to_show = 3;
                let mut to_show: Vec<String> = starting
                    .iter()
                    .take(max_to_show)
                    .map(ToString::to_string)
                    .collect();
                if starting.len() > max_to_show {
                    to_show.push("…".to_string());
                }
                let header = if total > 1 {
                    format!(
                        "Starting MCP servers ({completed}/{total}): {}",
                        to_show.join(", ")
                    )
                } else {
                    format!("Booting MCP server: {first}")
                };
                self.set_status_header(header);
            }
        }
        self.request_redraw();
    }

    pub(crate) fn set_mcp_startup_expected_servers<I>(&mut self, server_names: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.mcp_startup_expected_servers = Some(server_names.into_iter().collect());
    }

    #[cfg(test)]
    fn on_mcp_startup_update(&mut self, ev: McpStartupUpdateEvent) {
        self.update_mcp_startup_status(ev.server, ev.status, /*complete_when_settled*/ false);
    }

    fn finish_mcp_startup(&mut self, failed: Vec<String>, cancelled: Vec<String>) {
        if !cancelled.is_empty() {
            self.on_warning(format!(
                "MCP startup interrupted. The following servers were not initialized: {}",
                cancelled.join(", ")
            ));
        }
        let mut parts = Vec::new();
        if !failed.is_empty() {
            parts.push(format!("failed: {}", failed.join(", ")));
        }
        if !parts.is_empty() {
            self.on_warning(format!("MCP startup incomplete ({})", parts.join("; ")));
        }

        self.mcp_startup_status = None;
        self.mcp_startup_ignore_updates_until_next_start = true;
        self.mcp_startup_allow_terminal_only_next_round = false;
        self.mcp_startup_pending_next_round.clear();
        self.mcp_startup_pending_next_round_saw_starting = false;
        self.update_task_running_state();
        self.maybe_send_next_queued_input();
        self.request_redraw();
    }

    pub(crate) fn finish_mcp_startup_after_lag(&mut self) {
        if self.mcp_startup_ignore_updates_until_next_start {
            if self.mcp_startup_pending_next_round.is_empty() {
                self.mcp_startup_pending_next_round_saw_starting = false;
            }
            self.mcp_startup_allow_terminal_only_next_round = true;
        }

        let Some(current) = &self.mcp_startup_status else {
            return;
        };

        let mut failed = Vec::new();
        let mut cancelled = Vec::new();

        let mut server_names: BTreeSet<String> = current.keys().cloned().collect();
        if let Some(expected_servers) = &self.mcp_startup_expected_servers {
            server_names.extend(expected_servers.iter().cloned());
        }

        for name in server_names {
            match current.get(&name) {
                Some(McpStartupStatus::Ready) => {}
                Some(McpStartupStatus::Failed { .. }) => failed.push(name),
                Some(McpStartupStatus::Cancelled | McpStartupStatus::Starting) | None => {
                    cancelled.push(name);
                }
            }
        }

        failed.sort();
        failed.dedup();
        cancelled.sort();
        cancelled.dedup();
        self.finish_mcp_startup(failed, cancelled);
    }

    #[cfg(test)]
    fn on_mcp_startup_complete(&mut self, ev: McpStartupCompleteEvent) {
        let failed = ev.failed.into_iter().map(|f| f.server).collect();
        self.finish_mcp_startup(failed, ev.cancelled);
    }

    fn on_mcp_server_status_updated(&mut self, notification: McpServerStatusUpdatedNotification) {
        let status = match notification.status {
            McpServerStartupState::Starting => McpStartupStatus::Starting,
            McpServerStartupState::Ready => McpStartupStatus::Ready,
            McpServerStartupState::Failed => McpStartupStatus::Failed {
                error: notification.error.unwrap_or_else(|| {
                    format!("MCP client for `{}` failed to start", notification.name)
                }),
            },
            McpServerStartupState::Cancelled => McpStartupStatus::Cancelled,
        };
        self.update_mcp_startup_status(
            notification.name,
            status,
            /*complete_when_settled*/ true,
        );
    }

    /// Handle a turn aborted due to user interrupt (Esc).
    /// When there are queued user messages, restore them into the composer
    /// separated by newlines rather than auto‑submitting the next one.
    fn on_interrupted_turn(&mut self, reason: TurnAbortReason) {
        // Finalize, log a gentle prompt, and clear running state.
        self.finalize_turn();
        let send_pending_steers_immediately = self.submit_pending_steers_after_interrupt;
        self.submit_pending_steers_after_interrupt = false;
        if reason != TurnAbortReason::ReviewEnded {
            if send_pending_steers_immediately {
                self.add_to_history(history_cell::new_info_event(
                    "Model interrupted to submit steer instructions.".to_owned(),
                    /*hint*/ None,
                ));
            } else {
                self.add_to_history(history_cell::new_error_event(
                    "Conversation interrupted - tell the model what to do differently. Something went wrong? Hit `/feedback` to report the issue.".to_owned(),
                ));
            }
        }

        // Core clears pending_input before emitting TurnAborted, so any unacknowledged steers
        // still tracked here must be restored locally instead of waiting for a later commit.
        if send_pending_steers_immediately {
            let pending_steers: Vec<UserMessage> = self
                .pending_steers
                .drain(..)
                .map(|pending| pending.user_message)
                .collect();
            if !pending_steers.is_empty() {
                self.submit_user_message(merge_user_messages(pending_steers));
            } else if let Some(combined) = self.drain_pending_messages_for_restore() {
                self.restore_user_message_to_composer(combined);
            }
        } else if let Some(combined) = self.drain_pending_messages_for_restore() {
            self.restore_user_message_to_composer(combined);
        }
        self.refresh_pending_input_preview();

        self.request_redraw();
    }

    /// Merge pending steers, queued drafts, and the current composer state into a single message.
    ///
    /// Each pending message numbers attachments from `[Image #1]` relative to its own remote
    /// images. When we concatenate multiple messages after interrupt, we must renumber local-image
    /// placeholders in a stable order and rebase text element byte ranges so the restored composer
    /// state stays aligned with the merged attachment list. Returns `None` when there is nothing to
    /// restore.
    fn drain_pending_messages_for_restore(&mut self) -> Option<UserMessage> {
        if self.pending_steers.is_empty() && !self.has_queued_follow_up_messages() {
            return None;
        }

        let existing_message = UserMessage {
            text: self.bottom_pane.composer_text(),
            text_elements: self.bottom_pane.composer_text_elements(),
            local_images: self.bottom_pane.composer_local_images(),
            remote_image_urls: self.bottom_pane.remote_image_urls(),
            mention_bindings: self.bottom_pane.composer_mention_bindings(),
        };

        let mut to_merge: Vec<UserMessage> = self.rejected_steers_queue.drain(..).collect();
        to_merge.extend(
            self.pending_steers
                .drain(..)
                .map(|steer| steer.user_message),
        );
        to_merge.extend(self.queued_user_messages.drain(..));
        if !existing_message.text.is_empty()
            || !existing_message.local_images.is_empty()
            || !existing_message.remote_image_urls.is_empty()
        {
            to_merge.push(existing_message);
        }

        Some(merge_user_messages(to_merge))
    }

    fn restore_user_message_to_composer(&mut self, user_message: UserMessage) {
        let UserMessage {
            text,
            local_images,
            remote_image_urls,
            text_elements,
            mention_bindings,
        } = user_message;
        let local_image_paths = local_images.into_iter().map(|img| img.path).collect();
        self.set_remote_image_urls(remote_image_urls);
        self.bottom_pane.set_composer_text_with_mention_bindings(
            text,
            text_elements,
            local_image_paths,
            mention_bindings,
        );
    }

    pub(crate) fn capture_thread_input_state(&self) -> Option<ThreadInputState> {
        let composer = ThreadComposerState {
            text: self.bottom_pane.composer_text(),
            text_elements: self.bottom_pane.composer_text_elements(),
            local_images: self.bottom_pane.composer_local_images(),
            remote_image_urls: self.bottom_pane.remote_image_urls(),
            mention_bindings: self.bottom_pane.composer_mention_bindings(),
            pending_pastes: self.bottom_pane.composer_pending_pastes(),
        };
        Some(ThreadInputState {
            composer: composer.has_content().then_some(composer),
            pending_steers: self
                .pending_steers
                .iter()
                .map(|pending| pending.user_message.clone())
                .collect(),
            rejected_steers_queue: self.rejected_steers_queue.clone(),
            queued_user_messages: self.queued_user_messages.clone(),
            current_collaboration_mode: self.current_collaboration_mode.clone(),
            active_collaboration_mask: self.active_collaboration_mask.clone(),
            task_running: self.bottom_pane.is_task_running(),
            agent_turn_running: self.agent_turn_running,
        })
    }

    pub(crate) fn restore_thread_input_state(&mut self, input_state: Option<ThreadInputState>) {
        let restored_task_running = input_state.as_ref().is_some_and(|state| state.task_running);
        if let Some(input_state) = input_state {
            self.current_collaboration_mode = input_state.current_collaboration_mode;
            self.active_collaboration_mask = input_state.active_collaboration_mask;
            self.agent_turn_running = input_state.agent_turn_running;
            self.update_collaboration_mode_indicator();
            self.refresh_model_dependent_surfaces();
            if let Some(composer) = input_state.composer {
                let local_image_paths = composer
                    .local_images
                    .into_iter()
                    .map(|img| img.path)
                    .collect();
                self.set_remote_image_urls(composer.remote_image_urls);
                self.bottom_pane.set_composer_text_with_mention_bindings(
                    composer.text,
                    composer.text_elements,
                    local_image_paths,
                    composer.mention_bindings,
                );
                self.bottom_pane
                    .set_composer_pending_pastes(composer.pending_pastes);
            } else {
                self.set_remote_image_urls(Vec::new());
                self.bottom_pane.set_composer_text_with_mention_bindings(
                    String::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                );
                self.bottom_pane.set_composer_pending_pastes(Vec::new());
            }
            self.pending_steers = input_state
                .pending_steers
                .into_iter()
                .map(|user_message| PendingSteer {
                    compare_key: PendingSteerCompareKey {
                        message: user_message.text.clone(),
                        image_count: user_message.local_images.len()
                            + user_message.remote_image_urls.len(),
                    },
                    user_message,
                })
                .collect();
            self.rejected_steers_queue = input_state.rejected_steers_queue;
            self.queued_user_messages = input_state.queued_user_messages;
        } else {
            self.agent_turn_running = false;
            self.pending_steers.clear();
            self.rejected_steers_queue.clear();
            self.set_remote_image_urls(Vec::new());
            self.bottom_pane.set_composer_text_with_mention_bindings(
                String::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
            self.bottom_pane.set_composer_pending_pastes(Vec::new());
            self.queued_user_messages.clear();
        }
        self.turn_sleep_inhibitor
            .set_turn_running(self.agent_turn_running);
        self.update_task_running_state();
        if restored_task_running && !self.bottom_pane.is_task_running() {
            self.bottom_pane.set_task_running(/*running*/ true);
            self.refresh_terminal_title();
        }
        self.refresh_pending_input_preview();
        self.request_redraw();
    }

    pub(crate) fn set_queue_autosend_suppressed(&mut self, suppressed: bool) {
        self.suppress_queue_autosend = suppressed;
    }

    fn on_plan_update(&mut self, update: UpdatePlanArgs) {
        self.saw_plan_update_this_turn = true;
        let total = update.plan.len();
        let completed = update
            .plan
            .iter()
            .filter(|item| match &item.status {
                StepStatus::Completed => true,
                StepStatus::Pending | StepStatus::InProgress => false,
            })
            .count();
        self.last_plan_progress = (total > 0).then_some((completed, total));
        self.refresh_terminal_title();
        self.add_to_history(history_cell::new_plan_update(update));
    }

    fn on_exec_approval_request(&mut self, _id: String, ev: ExecApprovalRequestEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_exec_approval(ev),
            |s| s.handle_exec_approval_now(ev2),
        );
    }

    fn on_apply_patch_approval_request(&mut self, _id: String, ev: ApplyPatchApprovalRequestEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_apply_patch_approval(ev),
            |s| s.handle_apply_patch_approval_now(ev2),
        );
    }

    /// Handle guardian review lifecycle events for the current thread.
    ///
    /// In-progress assessments temporarily own the live status footer so the
    /// user can see what is being reviewed, including parallel review
    /// aggregation. Terminal assessments clear or update that footer state and
    /// render the final approved/denied history cell when guardian returns a
    /// decision.
    fn on_guardian_assessment(&mut self, ev: GuardianAssessmentEvent) {
        let guardian_action_summary = |action: &GuardianAssessmentAction| match action {
            GuardianAssessmentAction::Command { command, .. } => Some(command.clone()),
            GuardianAssessmentAction::Execve { program, argv, .. } => {
                let command = if argv.is_empty() {
                    vec![program.clone()]
                } else {
                    argv.clone()
                };
                shlex::try_join(command.iter().map(String::as_str))
                    .ok()
                    .or_else(|| Some(command.join(" ")))
            }
            GuardianAssessmentAction::ApplyPatch { files, .. } => Some(if files.len() == 1 {
                format!("apply_patch touching {}", files[0].display())
            } else {
                format!("apply_patch touching {} files", files.len())
            }),
            GuardianAssessmentAction::NetworkAccess { target, .. } => {
                Some(format!("network access to {target}"))
            }
            GuardianAssessmentAction::McpToolCall {
                server,
                tool_name,
                connector_name,
                ..
            } => {
                let label = connector_name.as_deref().unwrap_or(server.as_str());
                Some(format!("MCP {tool_name} on {label}"))
            }
        };
        let guardian_command = |action: &GuardianAssessmentAction| match action {
            GuardianAssessmentAction::Command { command, .. } => shlex::split(command)
                .filter(|command| !command.is_empty())
                .or_else(|| Some(vec![command.clone()])),
            GuardianAssessmentAction::Execve { program, argv, .. } => Some(if argv.is_empty() {
                vec![program.clone()]
            } else {
                argv.clone()
            })
            .filter(|command| !command.is_empty()),
            GuardianAssessmentAction::ApplyPatch { .. }
            | GuardianAssessmentAction::NetworkAccess { .. }
            | GuardianAssessmentAction::McpToolCall { .. } => None,
        };

        if ev.status == GuardianAssessmentStatus::InProgress
            && let Some(detail) = guardian_action_summary(&ev.action)
        {
            // In-progress assessments own the live footer state while the
            // review is pending. Parallel reviews are aggregated into one
            // footer summary by `PendingGuardianReviewStatus`.
            self.bottom_pane.ensure_status_indicator();
            self.bottom_pane
                .set_interrupt_hint_visible(/*visible*/ true);
            self.pending_guardian_review_status
                .start_or_update(ev.id.clone(), detail);
            if let Some(status) = self.pending_guardian_review_status.status_indicator_state() {
                self.set_status(
                    status.header,
                    status.details,
                    StatusDetailsCapitalization::Preserve,
                    status.details_max_lines,
                );
            }
            self.request_redraw();
            return;
        }

        // Terminal assessments remove the matching pending footer entry first,
        // then render the final approved/denied history cell below.
        if self.pending_guardian_review_status.finish(&ev.id) {
            if let Some(status) = self.pending_guardian_review_status.status_indicator_state() {
                self.set_status(
                    status.header,
                    status.details,
                    StatusDetailsCapitalization::Preserve,
                    status.details_max_lines,
                );
            } else if self.current_status.is_guardian_review() {
                self.set_status_header(String::from("Working"));
            }
        } else if self.pending_guardian_review_status.is_empty()
            && self.current_status.is_guardian_review()
        {
            self.set_status_header(String::from("Working"));
        }

        if ev.status == GuardianAssessmentStatus::Approved {
            let cell = if let Some(command) = guardian_command(&ev.action) {
                history_cell::new_approval_decision_cell(
                    command,
                    codex_protocol::protocol::ReviewDecision::Approved,
                    history_cell::ApprovalDecisionActor::Guardian,
                )
            } else if let Some(summary) = guardian_action_summary(&ev.action) {
                history_cell::new_guardian_approved_action_request(summary)
            } else {
                let summary = serde_json::to_string(&ev.action)
                    .unwrap_or_else(|_| "<unrenderable guardian action>".to_string());
                history_cell::new_guardian_approved_action_request(summary)
            };

            self.add_boxed_history(cell);
            self.request_redraw();
            return;
        }

        if ev.status != GuardianAssessmentStatus::Denied {
            return;
        }
        let cell = if let Some(command) = guardian_command(&ev.action) {
            history_cell::new_approval_decision_cell(
                command,
                codex_protocol::protocol::ReviewDecision::Denied,
                history_cell::ApprovalDecisionActor::Guardian,
            )
        } else {
            match &ev.action {
                GuardianAssessmentAction::ApplyPatch { files, .. } => {
                    let files = files
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>();
                    history_cell::new_guardian_denied_patch_request(files)
                }
                GuardianAssessmentAction::McpToolCall {
                    server, tool_name, ..
                } => history_cell::new_guardian_denied_action_request(format!(
                    "codex to call MCP tool {server}.{tool_name}"
                )),
                GuardianAssessmentAction::NetworkAccess { target, .. } => {
                    history_cell::new_guardian_denied_action_request(format!(
                        "codex to access {target}"
                    ))
                }
                GuardianAssessmentAction::Command { .. } => unreachable!(),
                GuardianAssessmentAction::Execve { .. } => unreachable!(),
            }
        };

        self.add_boxed_history(cell);
        self.request_redraw();
    }

    fn on_elicitation_request(&mut self, ev: ElicitationRequestEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_elicitation(ev),
            |s| s.handle_elicitation_request_now(ev2),
        );
    }

    fn on_request_user_input(&mut self, ev: RequestUserInputEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_user_input(ev),
            |s| s.handle_request_user_input_now(ev2),
        );
    }

    fn on_request_permissions(&mut self, ev: RequestPermissionsEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_request_permissions(ev),
            |s| s.handle_request_permissions_now(ev2),
        );
    }

    fn on_exec_command_begin(&mut self, ev: ExecCommandBeginEvent) {
        self.flush_answer_stream_with_separator();
        if is_unified_exec_source(ev.source) {
            self.track_unified_exec_process_begin(&ev);
            if !self.bottom_pane.is_task_running() {
                return;
            }
            // Unified exec may be parsed as Unknown; keep the working indicator visible regardless.
            self.bottom_pane.ensure_status_indicator();
            if !is_standard_tool_call(&ev.parsed_cmd) {
                return;
            }
        }
        let ev2 = ev.clone();
        self.defer_or_handle(|q| q.push_exec_begin(ev), |s| s.handle_exec_begin_now(ev2));
    }

    fn on_exec_command_output_delta(&mut self, ev: ExecCommandOutputDeltaEvent) {
        self.track_unified_exec_output_chunk(&ev.call_id, &ev.chunk);
        if !self.bottom_pane.is_task_running() {
            return;
        }

        let Some(cell) = self
            .active_cell
            .as_mut()
            .and_then(|c| c.as_any_mut().downcast_mut::<ExecCell>())
        else {
            return;
        };

        if cell.append_output(&ev.call_id, std::str::from_utf8(&ev.chunk).unwrap_or("")) {
            self.bump_active_cell_revision();
            self.request_redraw();
        }
    }

    fn on_terminal_interaction(&mut self, ev: TerminalInteractionEvent) {
        if !self.bottom_pane.is_task_running() {
            return;
        }
        self.flush_answer_stream_with_separator();
        let command_display = self
            .unified_exec_processes
            .iter()
            .find(|process| process.key == ev.process_id)
            .map(|process| process.command_display.clone());
        if ev.stdin.is_empty() {
            // Empty stdin means we are polling for background output.
            // Surface this in the status indicator (single "waiting" surface) instead of
            // the transcript. Keep the header short so the interrupt hint remains visible.
            self.bottom_pane.ensure_status_indicator();
            self.bottom_pane
                .set_interrupt_hint_visible(/*visible*/ true);
            self.terminal_title_status_kind = TerminalTitleStatusKind::WaitingForBackgroundTerminal;
            self.set_status(
                "Waiting for background terminal".to_string(),
                command_display.clone(),
                StatusDetailsCapitalization::Preserve,
                /*details_max_lines*/ 1,
            );
            match &mut self.unified_exec_wait_streak {
                Some(wait) if wait.process_id == ev.process_id => {
                    wait.update_command_display(command_display);
                }
                Some(_) => {
                    self.flush_unified_exec_wait_streak();
                    self.unified_exec_wait_streak =
                        Some(UnifiedExecWaitStreak::new(ev.process_id, command_display));
                }
                None => {
                    self.unified_exec_wait_streak =
                        Some(UnifiedExecWaitStreak::new(ev.process_id, command_display));
                }
            }
            self.request_redraw();
        } else {
            if self
                .unified_exec_wait_streak
                .as_ref()
                .is_some_and(|wait| wait.process_id == ev.process_id)
            {
                self.flush_unified_exec_wait_streak();
            }
            self.add_to_history(history_cell::new_unified_exec_interaction(
                command_display,
                ev.stdin,
            ));
        }
    }

    fn on_patch_apply_begin(&mut self, event: PatchApplyBeginEvent) {
        self.add_to_history(history_cell::new_patch_event(
            event.changes,
            &self.config.cwd,
        ));
    }

    fn on_view_image_tool_call(&mut self, event: ViewImageToolCallEvent) {
        self.flush_answer_stream_with_separator();
        self.add_to_history(history_cell::new_view_image_tool_call(
            event.path,
            &self.config.cwd,
        ));
        self.request_redraw();
    }

    fn on_image_generation_begin(&mut self, _event: ImageGenerationBeginEvent) {
        self.flush_answer_stream_with_separator();
    }

    fn on_image_generation_end(&mut self, event: ImageGenerationEndEvent) {
        self.flush_answer_stream_with_separator();
        let saved_path = event.saved_path.map(|saved_path| {
            Url::from_file_path(Path::new(&saved_path))
                .map(|url| url.to_string())
                .unwrap_or(saved_path)
        });
        self.add_to_history(history_cell::new_image_generation_call(
            event.call_id,
            event.revised_prompt,
            saved_path,
        ));
        self.request_redraw();
    }

    fn on_patch_apply_end(&mut self, event: codex_protocol::protocol::PatchApplyEndEvent) {
        let ev2 = event.clone();
        self.defer_or_handle(
            |q| q.push_patch_end(event),
            |s| s.handle_patch_apply_end_now(ev2),
        );
    }

    fn on_exec_command_end(&mut self, ev: ExecCommandEndEvent) {
        if is_unified_exec_source(ev.source) {
            if let Some(process_id) = ev.process_id.as_deref()
                && self
                    .unified_exec_wait_streak
                    .as_ref()
                    .is_some_and(|wait| wait.process_id == process_id)
            {
                self.flush_unified_exec_wait_streak();
            }
            self.track_unified_exec_process_end(&ev);
            if !self.bottom_pane.is_task_running() {
                return;
            }
        }
        let ev2 = ev.clone();
        self.defer_or_handle(|q| q.push_exec_end(ev), |s| s.handle_exec_end_now(ev2));
    }

    fn track_unified_exec_process_begin(&mut self, ev: &ExecCommandBeginEvent) {
        if ev.source != ExecCommandSource::UnifiedExecStartup {
            return;
        }
        let key = ev.process_id.clone().unwrap_or(ev.call_id.to_string());
        let command_display = strip_bash_lc_and_escape(&ev.command);
        if let Some(existing) = self
            .unified_exec_processes
            .iter_mut()
            .find(|process| process.key == key)
        {
            existing.call_id = ev.call_id.clone();
            existing.command_display = command_display;
            existing.recent_chunks.clear();
        } else {
            self.unified_exec_processes.push(UnifiedExecProcessSummary {
                key,
                call_id: ev.call_id.clone(),
                command_display,
                recent_chunks: Vec::new(),
            });
        }
        self.sync_unified_exec_footer();
    }

    fn track_unified_exec_process_end(&mut self, ev: &ExecCommandEndEvent) {
        let key = ev.process_id.clone().unwrap_or(ev.call_id.to_string());
        let before = self.unified_exec_processes.len();
        self.unified_exec_processes
            .retain(|process| process.key != key);
        if self.unified_exec_processes.len() != before {
            self.sync_unified_exec_footer();
        }
    }

    fn sync_unified_exec_footer(&mut self) {
        let processes = self
            .unified_exec_processes
            .iter()
            .map(|process| process.command_display.clone())
            .collect();
        self.bottom_pane.set_unified_exec_processes(processes);
    }

    /// Record recent stdout/stderr lines for the unified exec footer.
    fn track_unified_exec_output_chunk(&mut self, call_id: &str, chunk: &[u8]) {
        let Some(process) = self
            .unified_exec_processes
            .iter_mut()
            .find(|process| process.call_id == call_id)
        else {
            return;
        };

        let text = String::from_utf8_lossy(chunk);
        for line in text
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.is_empty())
        {
            process.recent_chunks.push(line.to_string());
        }

        const MAX_RECENT_CHUNKS: usize = 3;
        if process.recent_chunks.len() > MAX_RECENT_CHUNKS {
            let drop_count = process.recent_chunks.len() - MAX_RECENT_CHUNKS;
            process.recent_chunks.drain(0..drop_count);
        }
    }

    fn on_mcp_tool_call_begin(&mut self, ev: McpToolCallBeginEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(|q| q.push_mcp_begin(ev), |s| s.handle_mcp_begin_now(ev2));
    }

    fn on_mcp_tool_call_end(&mut self, ev: McpToolCallEndEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(|q| q.push_mcp_end(ev), |s| s.handle_mcp_end_now(ev2));
    }

    fn on_web_search_begin(&mut self, ev: WebSearchBeginEvent) {
        self.flush_answer_stream_with_separator();
        self.flush_active_cell();
        self.active_cell = Some(Box::new(history_cell::new_active_web_search_call(
            ev.call_id,
            String::new(),
            self.config.animations,
        )));
        self.bump_active_cell_revision();
        self.request_redraw();
    }

    fn on_web_search_end(&mut self, ev: WebSearchEndEvent) {
        self.flush_answer_stream_with_separator();
        let WebSearchEndEvent {
            call_id,
            query,
            action,
        } = ev;
        let mut handled = false;
        if let Some(cell) = self
            .active_cell
            .as_mut()
            .and_then(|cell| cell.as_any_mut().downcast_mut::<WebSearchCell>())
            && cell.call_id() == call_id
        {
            cell.update(action.clone(), query.clone());
            cell.complete();
            self.bump_active_cell_revision();
            self.flush_active_cell();
            handled = true;
        }

        if !handled {
            self.add_to_history(history_cell::new_web_search_call(call_id, query, action));
        }
        self.had_work_activity = true;
    }

    fn on_collab_event(&mut self, cell: PlainHistoryCell) {
        self.flush_answer_stream_with_separator();
        self.add_to_history(cell);
        self.request_redraw();
    }

    fn on_collab_agent_tool_call(&mut self, item: ThreadItem) {
        let ThreadItem::CollabAgentToolCall {
            id,
            tool,
            status,
            sender_thread_id,
            receiver_thread_ids,
            prompt,
            model,
            reasoning_effort,
            agents_states,
        } = item
        else {
            return;
        };
        let sender_thread_id = app_server_collab_thread_id_to_core(&sender_thread_id)
            .or(self.thread_id)
            .unwrap_or_default();
        let first_receiver = receiver_thread_ids
            .first()
            .and_then(|thread_id| app_server_collab_thread_id_to_core(thread_id));
        let first_receiver_metadata =
            first_receiver.map(|thread_id| self.collab_agent_metadata(thread_id));

        match tool {
            CollabAgentTool::SpawnAgent => {
                if let (Some(model), Some(reasoning_effort)) = (model.clone(), reasoning_effort) {
                    self.pending_collab_spawn_requests.insert(
                        id.clone(),
                        multi_agents::SpawnRequestSummary {
                            model,
                            reasoning_effort,
                        },
                    );
                }

                if !matches!(status, CollabAgentToolCallStatus::InProgress) {
                    let spawn_request =
                        self.pending_collab_spawn_requests.remove(&id).or_else(|| {
                            model
                                .zip(reasoning_effort)
                                .map(|(model, reasoning_effort)| {
                                    multi_agents::SpawnRequestSummary {
                                        model,
                                        reasoning_effort,
                                    }
                                })
                        });
                    self.on_collab_event(multi_agents::spawn_end(
                        codex_protocol::protocol::CollabAgentSpawnEndEvent {
                            call_id: id,
                            sender_thread_id,
                            new_thread_id: first_receiver,
                            new_agent_nickname: first_receiver_metadata
                                .as_ref()
                                .and_then(|metadata| metadata.agent_nickname.clone()),
                            new_agent_role: first_receiver_metadata
                                .as_ref()
                                .and_then(|metadata| metadata.agent_role.clone()),
                            prompt: prompt.unwrap_or_default(),
                            model: String::new(),
                            reasoning_effort: ReasoningEffortConfig::Medium,
                            status: first_receiver
                                .as_ref()
                                .and_then(|thread_id| agents_states.get(&thread_id.to_string()))
                                .map(app_server_collab_state_to_core)
                                .unwrap_or_else(|| {
                                    AgentStatus::Errored("Agent spawn failed".into())
                                }),
                        },
                        spawn_request.as_ref(),
                    ));
                }
            }
            CollabAgentTool::SendInput => {
                if let Some(receiver_thread_id) = first_receiver
                    && !matches!(status, CollabAgentToolCallStatus::InProgress)
                {
                    self.on_collab_event(multi_agents::interaction_end(
                        codex_protocol::protocol::CollabAgentInteractionEndEvent {
                            call_id: id,
                            sender_thread_id,
                            receiver_thread_id,
                            receiver_agent_nickname: first_receiver_metadata
                                .as_ref()
                                .and_then(|metadata| metadata.agent_nickname.clone()),
                            receiver_agent_role: first_receiver_metadata
                                .as_ref()
                                .and_then(|metadata| metadata.agent_role.clone()),
                            prompt: prompt.unwrap_or_default(),
                            status: receiver_thread_ids
                                .iter()
                                .find_map(|thread_id| agents_states.get(thread_id))
                                .map(app_server_collab_state_to_core)
                                .unwrap_or_else(|| {
                                    AgentStatus::Errored("Agent interaction failed".into())
                                }),
                        },
                    ));
                }
            }
            CollabAgentTool::ResumeAgent => {
                if let Some(receiver_thread_id) = first_receiver {
                    if matches!(status, CollabAgentToolCallStatus::InProgress) {
                        self.on_collab_event(multi_agents::resume_begin(
                            codex_protocol::protocol::CollabResumeBeginEvent {
                                call_id: id,
                                sender_thread_id,
                                receiver_thread_id,
                                receiver_agent_nickname: first_receiver_metadata
                                    .as_ref()
                                    .and_then(|metadata| metadata.agent_nickname.clone()),
                                receiver_agent_role: first_receiver_metadata
                                    .as_ref()
                                    .and_then(|metadata| metadata.agent_role.clone()),
                            },
                        ));
                    } else {
                        self.on_collab_event(multi_agents::resume_end(
                            codex_protocol::protocol::CollabResumeEndEvent {
                                call_id: id,
                                sender_thread_id,
                                receiver_thread_id,
                                receiver_agent_nickname: first_receiver_metadata
                                    .as_ref()
                                    .and_then(|metadata| metadata.agent_nickname.clone()),
                                receiver_agent_role: first_receiver_metadata
                                    .as_ref()
                                    .and_then(|metadata| metadata.agent_role.clone()),
                                status: receiver_thread_ids
                                    .iter()
                                    .find_map(|thread_id| agents_states.get(thread_id))
                                    .map(app_server_collab_state_to_core)
                                    .unwrap_or_else(|| {
                                        AgentStatus::Errored("Agent resume failed".into())
                                    }),
                            },
                        ));
                    }
                }
            }
            CollabAgentTool::Wait => {
                if matches!(status, CollabAgentToolCallStatus::InProgress) {
                    self.on_collab_event(multi_agents::waiting_begin(
                        codex_protocol::protocol::CollabWaitingBeginEvent {
                            sender_thread_id,
                            receiver_thread_ids: receiver_thread_ids
                                .iter()
                                .filter_map(|thread_id| {
                                    app_server_collab_thread_id_to_core(thread_id)
                                })
                                .collect(),
                            receiver_agents: app_server_collab_receiver_agent_refs(
                                &receiver_thread_ids,
                                &self.collab_agent_metadata,
                            ),
                            call_id: id,
                        },
                    ));
                } else {
                    let (agent_statuses, statuses) = app_server_collab_agent_statuses_to_core(
                        &receiver_thread_ids,
                        &agents_states,
                        &self.collab_agent_metadata,
                    );
                    self.on_collab_event(multi_agents::waiting_end(
                        codex_protocol::protocol::CollabWaitingEndEvent {
                            sender_thread_id,
                            call_id: id,
                            agent_statuses,
                            statuses,
                        },
                    ));
                }
            }
            CollabAgentTool::CloseAgent => {
                if let Some(receiver_thread_id) = first_receiver
                    && !matches!(status, CollabAgentToolCallStatus::InProgress)
                {
                    self.on_collab_event(multi_agents::close_end(
                        codex_protocol::protocol::CollabCloseEndEvent {
                            call_id: id,
                            sender_thread_id,
                            receiver_thread_id,
                            receiver_agent_nickname: first_receiver_metadata
                                .as_ref()
                                .and_then(|metadata| metadata.agent_nickname.clone()),
                            receiver_agent_role: first_receiver_metadata
                                .as_ref()
                                .and_then(|metadata| metadata.agent_role.clone()),
                            status: receiver_thread_ids
                                .iter()
                                .find_map(|thread_id| agents_states.get(thread_id))
                                .map(app_server_collab_state_to_core)
                                .unwrap_or_else(|| {
                                    AgentStatus::Errored("Agent close failed".into())
                                }),
                        },
                    ));
                }
            }
        }
    }

    pub(crate) fn handle_history_entry_response(
        &mut self,
        event: codex_protocol::protocol::GetHistoryEntryResponseEvent,
    ) {
        let codex_protocol::protocol::GetHistoryEntryResponseEvent {
            offset,
            log_id,
            entry,
        } = event;
        self.bottom_pane
            .on_history_entry_response(log_id, offset, entry.map(|e| e.text));
    }

    fn on_shutdown_complete(&mut self) {
        self.request_immediate_exit();
    }

    fn on_turn_diff(&mut self, unified_diff: String) {
        debug!("TurnDiffEvent: {unified_diff}");
        self.refresh_status_line();
    }

    fn on_deprecation_notice(&mut self, event: DeprecationNoticeEvent) {
        let DeprecationNoticeEvent { summary, details } = event;
        self.add_to_history(history_cell::new_deprecation_notice(summary, details));
        self.request_redraw();
    }

    #[cfg(test)]
    fn on_background_event(&mut self, message: String) {
        debug!("BackgroundEvent: {message}");
        self.bottom_pane.ensure_status_indicator();
        self.bottom_pane
            .set_interrupt_hint_visible(/*visible*/ true);
        self.terminal_title_status_kind = TerminalTitleStatusKind::Thinking;
        self.set_status_header(message);
    }

    fn on_hook_started(&mut self, event: codex_protocol::protocol::HookStartedEvent) {
        self.flush_answer_stream_with_separator();
        self.flush_completed_hook_output();
        match self.active_hook_cell.as_mut() {
            Some(cell) => {
                cell.start_run(event.run);
                self.bump_active_cell_revision();
            }
            None => {
                self.active_hook_cell = Some(history_cell::new_active_hook_cell(
                    event.run,
                    self.config.animations,
                ));
                self.bump_active_cell_revision();
            }
        }
        self.request_redraw();
    }

    fn on_hook_completed(&mut self, event: codex_protocol::protocol::HookCompletedEvent) {
        let completed = event.run;
        let completed_existing_run = self
            .active_hook_cell
            .as_mut()
            .map(|cell| cell.complete_run(completed.clone()))
            .unwrap_or(false);
        if completed_existing_run {
            self.bump_active_cell_revision();
        } else {
            match self.active_hook_cell.as_mut() {
                Some(cell) => {
                    cell.add_completed_run(completed);
                    self.bump_active_cell_revision();
                }
                None => {
                    let cell =
                        history_cell::new_completed_hook_cell(completed, self.config.animations);
                    if !cell.is_empty() {
                        self.active_hook_cell = Some(cell);
                        self.bump_active_cell_revision();
                    }
                }
            }
        }
        self.flush_completed_hook_output();
        self.finish_active_hook_cell_if_idle();
        self.request_redraw();
    }

    fn flush_completed_hook_output(&mut self) {
        let Some(completed_cell) = self
            .active_hook_cell
            .as_mut()
            .and_then(HookCell::take_completed_persistent_runs)
        else {
            return;
        };
        let active_cell_is_empty = self
            .active_hook_cell
            .as_ref()
            .is_some_and(HookCell::is_empty);
        if active_cell_is_empty {
            self.active_hook_cell = None;
        }
        self.bump_active_cell_revision();
        self.needs_final_message_separator = true;
        self.app_event_tx
            .send(AppEvent::InsertHistoryCell(Box::new(completed_cell)));
    }

    fn finish_active_hook_cell_if_idle(&mut self) {
        let Some(cell) = self.active_hook_cell.as_ref() else {
            return;
        };
        if cell.is_empty() {
            self.active_hook_cell = None;
            self.bump_active_cell_revision();
            return;
        }
        if cell.should_flush()
            && let Some(cell) = self.active_hook_cell.take()
        {
            self.bump_active_cell_revision();
            self.needs_final_message_separator = true;
            self.app_event_tx
                .send(AppEvent::InsertHistoryCell(Box::new(cell)));
        }
    }

    fn update_due_hook_visibility(&mut self) {
        let Some(cell) = self.active_hook_cell.as_mut() else {
            return;
        };
        let now = Instant::now();
        if cell.advance_time(now) {
            self.bump_active_cell_revision();
        }
        self.finish_active_hook_cell_if_idle();
    }

    fn schedule_hook_timer_if_needed(&self) {
        if self.config.animations
            && self
                .active_hook_cell
                .as_ref()
                .is_some_and(HookCell::has_visible_running_run)
        {
            self.frame_requester
                .schedule_frame_in(Duration::from_millis(50));
        }

        let Some(deadline) = self
            .active_hook_cell
            .as_ref()
            .and_then(HookCell::next_timer_deadline)
        else {
            return;
        };
        let delay = deadline.saturating_duration_since(Instant::now());
        self.frame_requester.schedule_frame_in(delay);
    }

    #[cfg(test)]
    fn on_undo_started(&mut self, event: UndoStartedEvent) {
        self.bottom_pane.ensure_status_indicator();
        self.bottom_pane
            .set_interrupt_hint_visible(/*visible*/ false);
        let message = event
            .message
            .unwrap_or_else(|| "Undo in progress...".to_string());
        self.terminal_title_status_kind = TerminalTitleStatusKind::Undoing;
        self.set_status_header(message);
    }

    #[cfg(test)]
    fn on_undo_completed(&mut self, event: UndoCompletedEvent) {
        let UndoCompletedEvent { success, message } = event;
        self.bottom_pane.hide_status_indicator();
        self.terminal_title_status_kind = TerminalTitleStatusKind::Working;
        self.refresh_terminal_title();
        let message = message.unwrap_or_else(|| {
            if success {
                "Undo completed successfully.".to_string()
            } else {
                "Undo failed.".to_string()
            }
        });
        if success {
            self.add_info_message(message, /*hint*/ None);
        } else {
            self.add_error_message(message);
        }
    }

    fn on_stream_error(&mut self, message: String, additional_details: Option<String>) {
        if self.retry_status_header.is_none() {
            self.retry_status_header = Some(self.current_status.header.clone());
        }
        self.bottom_pane.ensure_status_indicator();
        self.terminal_title_status_kind = TerminalTitleStatusKind::Thinking;
        self.set_status(
            message,
            additional_details,
            StatusDetailsCapitalization::CapitalizeFirst,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
    }

    pub(crate) fn pre_draw_tick(&mut self) {
        self.update_due_hook_visibility();
        self.schedule_hook_timer_if_needed();
        self.bottom_pane.pre_draw_tick();
        if self.should_animate_terminal_title_spinner() {
            self.refresh_terminal_title();
        }
    }

    /// Handle completion of an `AgentMessage` turn item.
    ///
    /// Commentary completion sets a deferred restore flag so the status row
    /// returns once stream queues are idle. Final-answer completion (or absent
    /// phase for legacy models) clears the flag to preserve historical behavior.
    fn on_agent_message_item_completed(&mut self, item: AgentMessageItem) {
        let mut message = String::new();
        for content in &item.content {
            match content {
                AgentMessageContent::Text { text } => message.push_str(text),
            }
        }
        self.finalize_completed_assistant_message(
            (!message.is_empty()).then_some(message.as_str()),
        );
        if matches!(item.phase, Some(MessagePhase::FinalAnswer) | None) && !message.is_empty() {
            self.record_agent_markdown(&message);
        }
        self.pending_status_indicator_restore = match item.phase {
            // Models that don't support preambles only output AgentMessageItems on turn completion.
            Some(MessagePhase::FinalAnswer) | None => false,
            Some(MessagePhase::Commentary) => true,
        };
        self.maybe_restore_status_indicator_after_stream_idle();
    }

    /// Periodic tick for stream commits. In smooth mode this preserves one-line pacing, while
    /// catch-up mode drains larger batches to reduce queue lag.
    pub(crate) fn on_commit_tick(&mut self) {
        self.run_commit_tick();
    }

    /// Runs a regular periodic commit tick.
    fn run_commit_tick(&mut self) {
        self.run_commit_tick_with_scope(CommitTickScope::AnyMode);
    }

    /// Runs an opportunistic commit tick only if catch-up mode is active.
    fn run_catch_up_commit_tick(&mut self) {
        self.run_commit_tick_with_scope(CommitTickScope::CatchUpOnly);
    }

    /// Runs a commit tick for the current stream queue snapshot.
    ///
    /// `scope` controls whether this call may commit in smooth mode or only when catch-up
    /// is currently active. While lines are actively streaming we hide the status row to avoid
    /// duplicate "in progress" affordances. Restoration is gated separately so we only re-show
    /// the row after commentary completion once stream queues are idle.
    fn run_commit_tick_with_scope(&mut self, scope: CommitTickScope) {
        let now = Instant::now();
        let outcome = run_commit_tick(
            &mut self.adaptive_chunking,
            self.stream_controller.as_mut(),
            self.plan_stream_controller.as_mut(),
            scope,
            now,
        );
        for cell in outcome.cells {
            self.bottom_pane.hide_status_indicator();
            self.add_boxed_history(cell);
        }

        if outcome.has_controller && outcome.all_idle {
            self.maybe_restore_status_indicator_after_stream_idle();
            self.app_event_tx.send(AppEvent::StopCommitAnimation);
        }

        if self.agent_turn_running {
            self.refresh_runtime_metrics();
        }
    }

    fn flush_interrupt_queue(&mut self) {
        let mut mgr = std::mem::take(&mut self.interrupts);
        mgr.flush_all(self);
        self.interrupts = mgr;
    }

    #[inline]
    fn defer_or_handle(
        &mut self,
        push: impl FnOnce(&mut InterruptManager),
        handle: impl FnOnce(&mut Self),
    ) {
        // Preserve deterministic FIFO across queued interrupts: once anything
        // is queued due to an active write cycle, continue queueing until the
        // queue is flushed to avoid reordering (e.g., ExecEnd before ExecBegin).
        if self.stream_controller.is_some() || !self.interrupts.is_empty() {
            push(&mut self.interrupts);
        } else {
            handle(self);
        }
    }

    fn handle_stream_finished(&mut self) {
        if self.task_complete_pending {
            self.bottom_pane.hide_status_indicator();
            self.task_complete_pending = false;
        }
        // A completed stream indicates non-exec content was just inserted.
        self.flush_interrupt_queue();
    }

    #[inline]
    fn handle_streaming_delta(&mut self, delta: String) {
        // Before streaming agent content, flush any active exec cell group.
        self.flush_unified_exec_wait_streak();
        self.flush_active_cell();

        if self.stream_controller.is_none() {
            // If the previous turn inserted non-stream history (exec output, patch status, MCP
            // calls), render a separator before starting the next streamed assistant message.
            if self.needs_final_message_separator && self.had_work_activity {
                let elapsed_seconds = self
                    .bottom_pane
                    .status_widget()
                    .map(super::status_indicator_widget::StatusIndicatorWidget::elapsed_seconds)
                    .map(|current| self.worked_elapsed_from(current));
                self.add_to_history(history_cell::FinalMessageSeparator::new(
                    elapsed_seconds,
                    /*runtime_metrics*/ None,
                ));
                self.needs_final_message_separator = false;
                self.had_work_activity = false;
            } else if self.needs_final_message_separator {
                // Reset the flag even if we don't show separator (no work was done)
                self.needs_final_message_separator = false;
            }
            self.stream_controller = Some(StreamController::new(
                self.last_rendered_width.get().map(|w| w.saturating_sub(2)),
                &self.config.cwd,
            ));
        }
        if let Some(controller) = self.stream_controller.as_mut()
            && controller.push(&delta)
        {
            self.app_event_tx.send(AppEvent::StartCommitAnimation);
            self.run_catch_up_commit_tick();
        }
        self.request_redraw();
    }

    fn worked_elapsed_from(&mut self, current_elapsed: u64) -> u64 {
        let baseline = match self.last_separator_elapsed_secs {
            Some(last) if current_elapsed < last => 0,
            Some(last) => last,
            None => 0,
        };
        let elapsed = current_elapsed.saturating_sub(baseline);
        self.last_separator_elapsed_secs = Some(current_elapsed);
        elapsed
    }

    /// Finalizes an exec call while preserving the active exec cell grouping contract.
    ///
    /// Exec begin/end events usually pair through `running_commands`, but unified exec can emit an
    /// end event for a call that was never materialized as the current active `ExecCell` (for
    /// example, when another exploring group is still active). In that case we render the end as a
    /// standalone history entry instead of replacing or flushing the unrelated active exploring
    /// cell. If this method treated every unknown end as "complete the active cell", the UI could
    /// merge unrelated commands and hide still-running exploring work.
    pub(crate) fn handle_exec_end_now(&mut self, ev: ExecCommandEndEvent) {
        enum ExecEndTarget {
            // Normal case: the active exec cell already tracks this call id.
            ActiveTracked,
            // We have an active exec group, but it does not contain this call id. Render the end
            // as a standalone finalized history cell so the active group remains intact.
            OrphanHistoryWhileActiveExec,
            // No active exec cell can safely own this end; build a new cell from the end payload.
            NewCell,
        }

        let running = self.running_commands.remove(&ev.call_id);
        if self.suppressed_exec_calls.remove(&ev.call_id) {
            return;
        }
        let (command, parsed, source) = match running {
            Some(rc) => (rc.command, rc.parsed_cmd, rc.source),
            None => (ev.command.clone(), ev.parsed_cmd.clone(), ev.source),
        };
        let parsed = self.annotate_skill_reads_in_parsed_cmd(parsed);
        let is_unified_exec_interaction =
            matches!(source, ExecCommandSource::UnifiedExecInteraction);
        let end_target = match self.active_cell.as_ref() {
            Some(cell) => match cell.as_any().downcast_ref::<ExecCell>() {
                Some(exec_cell)
                    if exec_cell
                        .iter_calls()
                        .any(|call| call.call_id == ev.call_id) =>
                {
                    ExecEndTarget::ActiveTracked
                }
                Some(exec_cell) if exec_cell.is_active() => {
                    ExecEndTarget::OrphanHistoryWhileActiveExec
                }
                Some(_) | None => ExecEndTarget::NewCell,
            },
            None => ExecEndTarget::NewCell,
        };

        // Unified exec interaction rows intentionally hide command output text in the exec cell and
        // instead render the interaction-specific content elsewhere in the UI.
        let output = if is_unified_exec_interaction {
            CommandOutput {
                exit_code: ev.exit_code,
                formatted_output: String::new(),
                aggregated_output: String::new(),
            }
        } else {
            CommandOutput {
                exit_code: ev.exit_code,
                formatted_output: ev.formatted_output.clone(),
                aggregated_output: ev.aggregated_output.clone(),
            }
        };

        match end_target {
            ExecEndTarget::ActiveTracked => {
                if let Some(cell) = self
                    .active_cell
                    .as_mut()
                    .and_then(|c| c.as_any_mut().downcast_mut::<ExecCell>())
                {
                    let completed = cell.complete_call(&ev.call_id, output, ev.duration);
                    debug_assert!(completed, "active exec cell should contain {}", ev.call_id);
                    if cell.should_flush() {
                        self.flush_active_cell();
                    } else {
                        self.bump_active_cell_revision();
                        self.request_redraw();
                    }
                }
            }
            ExecEndTarget::OrphanHistoryWhileActiveExec => {
                let mut orphan = new_active_exec_command(
                    ev.call_id.clone(),
                    command,
                    parsed,
                    source,
                    ev.interaction_input.clone(),
                    self.config.animations,
                );
                let completed = orphan.complete_call(&ev.call_id, output, ev.duration);
                debug_assert!(
                    completed,
                    "new orphan exec cell should contain {}",
                    ev.call_id
                );
                self.needs_final_message_separator = true;
                self.app_event_tx
                    .send(AppEvent::InsertHistoryCell(Box::new(orphan)));
                self.request_redraw();
            }
            ExecEndTarget::NewCell => {
                self.flush_active_cell();
                let mut cell = new_active_exec_command(
                    ev.call_id.clone(),
                    command,
                    parsed,
                    source,
                    ev.interaction_input.clone(),
                    self.config.animations,
                );
                let completed = cell.complete_call(&ev.call_id, output, ev.duration);
                debug_assert!(completed, "new exec cell should contain {}", ev.call_id);
                if cell.should_flush() {
                    self.add_to_history(cell);
                } else {
                    self.active_cell = Some(Box::new(cell));
                    self.bump_active_cell_revision();
                    self.request_redraw();
                }
            }
        }
        // Mark that actual work was done (command executed)
        self.had_work_activity = true;
    }

    pub(crate) fn handle_patch_apply_end_now(
        &mut self,
        event: codex_protocol::protocol::PatchApplyEndEvent,
    ) {
        // If the patch was successful, just let the "Edited" block stand.
        // Otherwise, add a failure block.
        if !event.success {
            self.add_to_history(history_cell::new_patch_apply_failure(event.stderr));
        }
        // Mark that actual work was done (patch applied)
        self.had_work_activity = true;
    }

    pub(crate) fn handle_exec_approval_now(&mut self, ev: ExecApprovalRequestEvent) {
        self.flush_answer_stream_with_separator();
        let command = shlex::try_join(ev.command.iter().map(String::as_str))
            .unwrap_or_else(|_| ev.command.join(" "));
        self.notify(Notification::ExecApprovalRequested { command });

        let available_decisions = ev.effective_available_decisions();
        let request = ApprovalRequest::Exec {
            thread_id: self.thread_id.unwrap_or_default(),
            thread_label: None,
            id: ev.effective_approval_id(),
            command: ev.command,
            reason: ev.reason,
            available_decisions,
            network_approval_context: ev.network_approval_context,
            additional_permissions: ev.additional_permissions,
        };
        self.bottom_pane
            .push_approval_request(request, &self.config.features);
        self.request_redraw();
    }

    pub(crate) fn handle_apply_patch_approval_now(&mut self, ev: ApplyPatchApprovalRequestEvent) {
        self.flush_answer_stream_with_separator();

        let request = ApprovalRequest::ApplyPatch {
            thread_id: self.thread_id.unwrap_or_default(),
            thread_label: None,
            id: ev.call_id,
            reason: ev.reason,
            changes: ev.changes.clone(),
            cwd: self.config.cwd.to_path_buf(),
        };
        self.bottom_pane
            .push_approval_request(request, &self.config.features);
        self.request_redraw();
        self.notify(Notification::EditApprovalRequested {
            cwd: self.config.cwd.to_path_buf(),
            changes: ev.changes.keys().cloned().collect(),
        });
    }

    pub(crate) fn handle_elicitation_request_now(&mut self, ev: ElicitationRequestEvent) {
        self.flush_answer_stream_with_separator();

        self.notify(Notification::ElicitationRequested {
            server_name: ev.server_name.clone(),
        });

        let thread_id = self.thread_id.unwrap_or_default();
        if let Some(request) = McpServerElicitationFormRequest::from_event(thread_id, ev.clone()) {
            self.bottom_pane
                .push_mcp_server_elicitation_request(request);
        } else {
            let request = ApprovalRequest::McpElicitation {
                thread_id,
                thread_label: None,
                server_name: ev.server_name,
                request_id: ev.id,
                message: ev.request.message().to_string(),
            };
            self.bottom_pane
                .push_approval_request(request, &self.config.features);
        }
        self.request_redraw();
    }

    pub(crate) fn push_approval_request(&mut self, request: ApprovalRequest) {
        self.bottom_pane
            .push_approval_request(request, &self.config.features);
        self.request_redraw();
    }

    pub(crate) fn push_mcp_server_elicitation_request(
        &mut self,
        request: McpServerElicitationFormRequest,
    ) {
        self.bottom_pane
            .push_mcp_server_elicitation_request(request);
        self.request_redraw();
    }

    pub(crate) fn handle_request_user_input_now(&mut self, ev: RequestUserInputEvent) {
        self.flush_answer_stream_with_separator();
        self.notify(Notification::UserInputRequested {
            question_count: ev.questions.len(),
            summary: Notification::user_input_request_summary(&ev.questions),
        });
        self.bottom_pane.push_user_input_request(ev);
        self.request_redraw();
    }

    pub(crate) fn handle_request_permissions_now(&mut self, ev: RequestPermissionsEvent) {
        self.flush_answer_stream_with_separator();
        let request = ApprovalRequest::Permissions {
            thread_id: self.thread_id.unwrap_or_default(),
            thread_label: None,
            call_id: ev.call_id,
            reason: ev.reason,
            permissions: ev.permissions,
        };
        self.bottom_pane
            .push_approval_request(request, &self.config.features);
        self.request_redraw();
    }

    pub(crate) fn handle_exec_begin_now(&mut self, ev: ExecCommandBeginEvent) {
        // Ensure the status indicator is visible while the command runs.
        self.bottom_pane.ensure_status_indicator();
        let parsed_cmd = self.annotate_skill_reads_in_parsed_cmd(ev.parsed_cmd.clone());
        self.running_commands.insert(
            ev.call_id.clone(),
            RunningCommand {
                command: ev.command.clone(),
                parsed_cmd: parsed_cmd.clone(),
                source: ev.source,
            },
        );
        let is_wait_interaction = matches!(ev.source, ExecCommandSource::UnifiedExecInteraction)
            && ev
                .interaction_input
                .as_deref()
                .map(str::is_empty)
                .unwrap_or(true);
        let command_display = ev.command.join(" ");
        let should_suppress_unified_wait = is_wait_interaction
            && self
                .last_unified_wait
                .as_ref()
                .is_some_and(|wait| wait.is_duplicate(&command_display));
        if is_wait_interaction {
            self.last_unified_wait = Some(UnifiedExecWaitState::new(command_display));
        } else {
            self.last_unified_wait = None;
        }
        if should_suppress_unified_wait {
            self.suppressed_exec_calls.insert(ev.call_id);
            return;
        }
        let interaction_input = ev.interaction_input.clone();
        if let Some(cell) = self
            .active_cell
            .as_mut()
            .and_then(|c| c.as_any_mut().downcast_mut::<ExecCell>())
            && let Some(new_exec) = cell.with_added_call(
                ev.call_id.clone(),
                ev.command.clone(),
                parsed_cmd.clone(),
                ev.source,
                interaction_input.clone(),
            )
        {
            *cell = new_exec;
            self.bump_active_cell_revision();
        } else {
            self.flush_active_cell();

            self.active_cell = Some(Box::new(new_active_exec_command(
                ev.call_id.clone(),
                ev.command.clone(),
                parsed_cmd,
                ev.source,
                interaction_input,
                self.config.animations,
            )));
            self.bump_active_cell_revision();
        }

        self.request_redraw();
    }

    pub(crate) fn handle_mcp_begin_now(&mut self, ev: McpToolCallBeginEvent) {
        self.flush_answer_stream_with_separator();
        self.flush_active_cell();
        self.active_cell = Some(Box::new(history_cell::new_active_mcp_tool_call(
            ev.call_id,
            ev.invocation,
            self.config.animations,
        )));
        self.bump_active_cell_revision();
        self.request_redraw();
    }
    pub(crate) fn handle_mcp_end_now(&mut self, ev: McpToolCallEndEvent) {
        self.flush_answer_stream_with_separator();

        let McpToolCallEndEvent {
            call_id,
            invocation,
            duration,
            result,
        } = ev;

        let extra_cell = match self
            .active_cell
            .as_mut()
            .and_then(|cell| cell.as_any_mut().downcast_mut::<McpToolCallCell>())
        {
            Some(cell) if cell.call_id() == call_id => cell.complete(duration, result),
            _ => {
                self.flush_active_cell();
                let mut cell = history_cell::new_active_mcp_tool_call(
                    call_id,
                    invocation,
                    self.config.animations,
                );
                let extra_cell = cell.complete(duration, result);
                self.active_cell = Some(Box::new(cell));
                extra_cell
            }
        };

        self.flush_active_cell();
        if let Some(extra) = extra_cell {
            self.add_boxed_history(extra);
        }
        // Mark that actual work was done (MCP tool call)
        self.had_work_activity = true;
    }

    pub(crate) fn new_with_app_event(common: ChatWidgetInit) -> Self {
        Self::new_with_op_target(common, CodexOpTarget::AppEvent)
    }

    #[allow(dead_code)]
    pub(crate) fn new_with_op_sender(
        common: ChatWidgetInit,
        codex_op_tx: UnboundedSender<Op>,
    ) -> Self {
        Self::new_with_op_target(common, CodexOpTarget::Direct(codex_op_tx))
    }

    fn new_with_op_target(common: ChatWidgetInit, codex_op_target: CodexOpTarget) -> Self {
        let ChatWidgetInit {
            config,
            frame_requester,
            app_event_tx,
            initial_user_message,
            enhanced_keys_supported,
            has_chatgpt_account,
            model_catalog,
            feedback,
            is_first_run,
            status_account_display,
            initial_plan_type,
            model,
            startup_tooltip_override,
            status_line_invalid_items_warned,
            terminal_title_invalid_items_warned,
            session_telemetry,
        } = common;
        let model = model.filter(|m| !m.trim().is_empty());
        let mut config = config;
        config.model = model.clone();
        let prevent_idle_sleep = config.features.enabled(Feature::PreventIdleSleep);
        let mut rng = rand::rng();
        let placeholder = PLACEHOLDERS[rng.random_range(0..PLACEHOLDERS.len())].to_string();

        let model_override = model.as_deref();
        let model_for_header = model
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL_DISPLAY_NAME.to_string());
        let active_collaboration_mask =
            Self::initial_collaboration_mask(&config, model_catalog.as_ref(), model_override);
        let header_model = active_collaboration_mask
            .as_ref()
            .and_then(|mask| mask.model.clone())
            .unwrap_or_else(|| model_for_header.clone());
        let fallback_default = Settings {
            model: header_model.clone(),
            reasoning_effort: None,
            developer_instructions: None,
        };
        // Collaboration modes start in Default mode.
        let current_collaboration_mode = CollaborationMode {
            mode: ModeKind::Default,
            settings: fallback_default,
        };

        let active_cell = Some(Self::placeholder_session_header_cell(&config));

        let current_cwd = Some(config.cwd.to_path_buf());
        let queued_message_edit_binding = queued_message_edit_binding_for_terminal(terminal_info());
        let mut widget = Self {
            app_event_tx: app_event_tx.clone(),
            frame_requester: frame_requester.clone(),
            codex_op_target,
            bottom_pane: BottomPane::new(BottomPaneParams {
                frame_requester,
                app_event_tx,
                has_input_focus: true,
                enhanced_keys_supported,
                placeholder_text: placeholder,
                disable_paste_burst: config.disable_paste_burst,
                animations_enabled: config.animations,
                skills: None,
            }),
            active_cell,
            active_cell_revision: 0,
            config,
            skills_all: Vec::new(),
            skills_initial_state: None,
            current_collaboration_mode,
            active_collaboration_mask,
            has_chatgpt_account,
            model_catalog,
            session_telemetry,
            session_header: SessionHeader::new(header_model),
            initial_user_message,
            status_account_display,
            token_info: None,
            rate_limit_snapshots_by_limit_id: BTreeMap::new(),
            refreshing_status_outputs: Vec::new(),
            next_status_refresh_request_id: 0,
            plan_type: initial_plan_type,
            rate_limit_warnings: RateLimitWarningState::default(),
            rate_limit_switch_prompt: RateLimitSwitchPromptState::default(),
            adaptive_chunking: AdaptiveChunkingPolicy::default(),
            stream_controller: None,
            plan_stream_controller: None,
            clipboard_lease: None,
            running_commands: HashMap::new(),
            collab_agent_metadata: HashMap::new(),
            pending_collab_spawn_requests: HashMap::new(),
            suppressed_exec_calls: HashSet::new(),
            last_unified_wait: None,
            unified_exec_wait_streak: None,
            turn_sleep_inhibitor: SleepInhibitor::new(prevent_idle_sleep),
            task_complete_pending: false,
            unified_exec_processes: Vec::new(),
            agent_turn_running: false,
            mcp_startup_status: None,
            last_agent_markdown: None,
            saw_copy_source_this_turn: false,
            mcp_startup_expected_servers: None,
            mcp_startup_ignore_updates_until_next_start: false,
            mcp_startup_allow_terminal_only_next_round: false,
            mcp_startup_pending_next_round: HashMap::new(),
            mcp_startup_pending_next_round_saw_starting: false,
            connectors_cache: ConnectorsCacheState::default(),
            connectors_partial_snapshot: None,
            connectors_prefetch_in_flight: false,
            connectors_force_refetch_pending: false,
            plugins_cache: PluginsCacheState::default(),
            plugins_fetch_state: PluginListFetchState::default(),
            plugin_install_apps_needing_auth: Vec::new(),
            plugin_install_auth_flow: None,
            interrupts: InterruptManager::new(),
            reasoning_buffer: String::new(),
            full_reasoning_buffer: String::new(),
            current_status: StatusIndicatorState::working(),
            pending_guardian_review_status: PendingGuardianReviewStatus::default(),
            active_hook_cell: None,
            terminal_title_status_kind: TerminalTitleStatusKind::Working,
            retry_status_header: None,
            pending_status_indicator_restore: false,
            suppress_queue_autosend: false,
            thread_id: None,
            last_turn_id: None,
            thread_name: None,
            forked_from: None,
            queued_user_messages: VecDeque::new(),
            rejected_steers_queue: VecDeque::new(),
            pending_steers: VecDeque::new(),
            submit_pending_steers_after_interrupt: false,
            queued_message_edit_binding,
            show_welcome_banner: is_first_run,
            startup_tooltip_override,
            suppress_session_configured_redraw: false,
            suppress_initial_user_message_submit: false,
            pending_notification: None,
            quit_shortcut_expires_at: None,
            quit_shortcut_key: None,
            is_review_mode: false,
            pre_review_token_info: None,
            needs_final_message_separator: false,
            had_work_activity: false,
            saw_plan_update_this_turn: false,
            saw_plan_item_this_turn: false,
            last_plan_progress: None,
            plan_delta_buffer: String::new(),
            plan_item_active: false,
            last_separator_elapsed_secs: None,
            turn_runtime_metrics: RuntimeMetricsSummary::default(),
            last_rendered_width: std::cell::Cell::new(None),
            feedback,
            current_rollout_path: None,
            current_cwd,
            session_network_proxy: None,
            status_line_invalid_items_warned,
            terminal_title_invalid_items_warned,
            last_terminal_title: None,
            terminal_title_setup_original_items: None,
            terminal_title_animation_origin: Instant::now(),
            status_line_project_root_name_cache: None,
            status_line_branch: None,
            status_line_branch_cwd: None,
            status_line_branch_pending: false,
            status_line_branch_lookup_complete: false,
            external_editor_state: ExternalEditorState::Closed,
            realtime_conversation: RealtimeConversationUiState::default(),
            last_rendered_user_message_event: None,
            last_non_retry_error: None,
        };

        widget
            .bottom_pane
            .set_realtime_conversation_enabled(widget.realtime_conversation_enabled());
        widget
            .bottom_pane
            .set_audio_device_selection_enabled(widget.realtime_audio_device_selection_enabled());
        widget
            .bottom_pane
            .set_status_line_enabled(!widget.configured_status_line_items().is_empty());
        widget
            .bottom_pane
            .set_collaboration_modes_enabled(/*enabled*/ true);
        widget.sync_fast_command_enabled();
        widget.sync_personality_command_enabled();
        widget.sync_plugins_command_enabled();
        widget
            .bottom_pane
            .set_queued_message_edit_binding(widget.queued_message_edit_binding);
        #[cfg(target_os = "windows")]
        widget.bottom_pane.set_windows_degraded_sandbox_active(
            crate::legacy_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
                && matches!(
                    WindowsSandboxLevel::from_config(&widget.config),
                    WindowsSandboxLevel::RestrictedToken
                ),
        );
        widget.update_collaboration_mode_indicator();

        widget
            .bottom_pane
            .set_connectors_enabled(widget.connectors_enabled());
        widget.refresh_status_surfaces();

        widget
    }

    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            // Ctrl+O - copy last agent response from the main view.
            KeyEvent {
                code: KeyCode::Char('o'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } => {
                self.bottom_pane.clear_quit_shortcut_hint();
                self.quit_shortcut_expires_at = None;
                self.quit_shortcut_key = None;
                self.copy_last_agent_markdown();
                return;
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                kind: KeyEventKind::Press,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) && c.eq_ignore_ascii_case(&'c') => {
                self.on_ctrl_c();
                return;
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                kind: KeyEventKind::Press,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) && c.eq_ignore_ascii_case(&'d') => {
                if self.on_ctrl_d() {
                    return;
                }
                self.bottom_pane.clear_quit_shortcut_hint();
                self.quit_shortcut_expires_at = None;
                self.quit_shortcut_key = None;
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                kind: KeyEventKind::Press,
                ..
            } if modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                && c.eq_ignore_ascii_case(&'v') =>
            {
                match paste_image_to_temp_png() {
                    Ok((path, info)) => {
                        tracing::debug!(
                            "pasted image size={}x{} format={}",
                            info.width,
                            info.height,
                            info.encoded_format.label()
                        );
                        self.attach_image(path);
                    }
                    Err(err) => {
                        tracing::warn!("failed to paste image: {err}");
                        self.add_to_history(history_cell::new_error_event(format!(
                            "Failed to paste image: {err}",
                        )));
                    }
                }
                return;
            }
            other if other.kind == KeyEventKind::Press => {
                self.bottom_pane.clear_quit_shortcut_hint();
                self.quit_shortcut_expires_at = None;
                self.quit_shortcut_key = None;
            }
            _ => {}
        }

        if key_event.kind == KeyEventKind::Press
            && self.queued_message_edit_binding.is_press(key_event)
            && self.has_queued_follow_up_messages()
        {
            if let Some(user_message) = self.pop_latest_queued_user_message() {
                self.restore_user_message_to_composer(user_message);
                self.refresh_pending_input_preview();
                self.request_redraw();
            }
            return;
        }

        if matches!(key_event.code, KeyCode::Esc)
            && matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && !self.pending_steers.is_empty()
            && self.bottom_pane.is_task_running()
            && self.bottom_pane.no_modal_or_popup_active()
        {
            self.submit_pending_steers_after_interrupt = true;
            if !self.submit_op(AppCommand::interrupt()) {
                self.submit_pending_steers_after_interrupt = false;
            }
            return;
        }

        match key_event {
            KeyEvent {
                code: KeyCode::BackTab,
                kind: KeyEventKind::Press,
                ..
            } if self.collaboration_modes_enabled()
                && !self.bottom_pane.is_task_running()
                && self.bottom_pane.no_modal_or_popup_active() =>
            {
                self.cycle_collaboration_mode();
            }
            _ => match self.bottom_pane.handle_key_event(key_event) {
                InputResult::Submitted {
                    text,
                    text_elements,
                } => {
                    let local_images = self
                        .bottom_pane
                        .take_recent_submission_images_with_placeholders();
                    let remote_image_urls = self.take_remote_image_urls();
                    let user_message = UserMessage {
                        text,
                        local_images,
                        remote_image_urls,
                        text_elements,
                        mention_bindings: self
                            .bottom_pane
                            .take_recent_submission_mention_bindings(),
                    };
                    if user_message.text.is_empty()
                        && user_message.local_images.is_empty()
                        && user_message.remote_image_urls.is_empty()
                    {
                        return;
                    }
                    let Some(user_message) =
                        self.maybe_defer_user_message_for_realtime(user_message)
                    else {
                        return;
                    };
                    let should_submit_now =
                        self.is_session_configured() && !self.is_plan_streaming_in_tui();
                    if should_submit_now {
                        // Submitted is emitted when user submits.
                        // Reset any reasoning header only when we are actually submitting a turn.
                        self.reasoning_buffer.clear();
                        self.full_reasoning_buffer.clear();
                        self.set_status_header(String::from("Working"));
                        self.submit_user_message(user_message);
                    } else {
                        self.queue_user_message(user_message);
                    }
                }
                InputResult::Queued {
                    text,
                    text_elements,
                } => {
                    let local_images = self
                        .bottom_pane
                        .take_recent_submission_images_with_placeholders();
                    let remote_image_urls = self.take_remote_image_urls();
                    let user_message = UserMessage {
                        text,
                        local_images,
                        remote_image_urls,
                        text_elements,
                        mention_bindings: self
                            .bottom_pane
                            .take_recent_submission_mention_bindings(),
                    };
                    let Some(user_message) =
                        self.maybe_defer_user_message_for_realtime(user_message)
                    else {
                        return;
                    };
                    self.queue_user_message(user_message);
                }
                InputResult::Command(cmd) => {
                    self.handle_slash_command_dispatch(cmd);
                }
                InputResult::CommandWithArgs(cmd, args, text_elements) => {
                    self.handle_slash_command_with_args_dispatch(cmd, args, text_elements);
                }
                InputResult::None => {}
            },
        }
    }

    /// Attach a local image to the composer when the active model supports image inputs.
    ///
    /// When the model does not advertise image support, we keep the draft unchanged and surface a
    /// warning event so users can switch models or remove attachments.
    pub(crate) fn attach_image(&mut self, path: PathBuf) {
        if !self.current_model_supports_images() {
            self.add_to_history(history_cell::new_warning_event(
                self.image_inputs_not_supported_message(),
            ));
            self.request_redraw();
            return;
        }
        tracing::info!("attach_image path={path:?}");
        self.bottom_pane.attach_image(path);
        self.request_redraw();
    }

    pub(crate) fn composer_text_with_pending(&self) -> String {
        self.bottom_pane.composer_text_with_pending()
    }

    pub(crate) fn apply_external_edit(&mut self, text: String) {
        self.bottom_pane.apply_external_edit(text);
        self.request_redraw();
    }

    pub(crate) fn external_editor_state(&self) -> ExternalEditorState {
        self.external_editor_state
    }

    pub(crate) fn set_external_editor_state(&mut self, state: ExternalEditorState) {
        self.external_editor_state = state;
    }

    pub(crate) fn set_footer_hint_override(&mut self, items: Option<Vec<(String, String)>>) {
        self.bottom_pane.set_footer_hint_override(items);
    }

    pub(crate) fn show_selection_view(&mut self, params: SelectionViewParams) {
        self.bottom_pane.show_selection_view(params);
        self.request_redraw();
    }

    pub(crate) fn no_modal_or_popup_active(&self) -> bool {
        self.bottom_pane.no_modal_or_popup_active()
    }

    pub(crate) fn can_launch_external_editor(&self) -> bool {
        self.bottom_pane.can_launch_external_editor()
    }

    pub(crate) fn can_run_ctrl_l_clear_now(&mut self) -> bool {
        // Ctrl+L is not a slash command, but it follows /clear's current rule:
        // block while a task is running.
        if !self.bottom_pane.is_task_running() {
            return true;
        }

        let message = "Ctrl+L is disabled while a task is in progress.".to_string();
        self.add_to_history(history_cell::new_error_event(message));
        self.request_redraw();
        false
    }

    /// Copy the last agent response (raw markdown) to the system clipboard.
    pub(crate) fn copy_last_agent_markdown(&mut self) {
        self.copy_last_agent_markdown_with(crate::clipboard_copy::copy_to_clipboard);
    }

    /// Inner implementation with an injectable clipboard backend for testing.
    fn copy_last_agent_markdown_with(
        &mut self,
        copy_fn: impl FnOnce(&str) -> Result<Option<crate::clipboard_copy::ClipboardLease>, String>,
    ) {
        match self.last_agent_markdown.clone() {
            Some(markdown) if !markdown.is_empty() => match copy_fn(&markdown) {
                Ok(lease) => {
                    self.clipboard_lease = lease;
                    self.add_to_history(history_cell::new_info_event(
                        "Copied last message to clipboard".into(),
                        /*hint*/ None,
                    ));
                }
                Err(error) => self.add_to_history(history_cell::new_error_event(format!(
                    "Copy failed: {error}"
                ))),
            },
            _ => self.add_to_history(history_cell::new_error_event(
                "No agent response to copy".into(),
            )),
        }
        self.request_redraw();
    }

    #[cfg(test)]
    pub(crate) fn last_agent_markdown_text(&self) -> Option<&str> {
        self.last_agent_markdown.as_deref()
    }

    fn show_rename_prompt(&mut self) {
        let tx = self.app_event_tx.clone();
        let has_name = self
            .thread_name
            .as_ref()
            .is_some_and(|name| !name.is_empty());
        let title = if has_name {
            "Rename thread"
        } else {
            "Name thread"
        };
        let view = CustomPromptView::new(
            title.to_string(),
            "Type a name and press Enter".to_string(),
            /*context_label*/ None,
            Box::new(move |name: String| {
                let Some(name) = crate::legacy_core::util::normalize_thread_name(&name) else {
                    tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_error_event("Thread name cannot be empty.".to_string()),
                    )));
                    return;
                };
                tx.set_thread_name(name);
            }),
        );

        self.bottom_pane.show_view(Box::new(view));
    }

    pub(crate) fn handle_paste(&mut self, text: String) {
        self.bottom_pane.handle_paste(text);
    }

    // Returns true if caller should skip rendering this frame (a future frame is scheduled).
    pub(crate) fn handle_paste_burst_tick(&mut self, frame_requester: FrameRequester) -> bool {
        if self.bottom_pane.flush_paste_burst_if_due() {
            // A paste just flushed; request an immediate redraw and skip this frame.
            self.request_redraw();
            true
        } else if self.bottom_pane.is_in_paste_burst() {
            // While capturing a burst, schedule a follow-up tick and skip this frame
            // to avoid redundant renders between ticks.
            frame_requester.schedule_frame_in(
                crate::bottom_pane::ChatComposer::recommended_paste_flush_delay(),
            );
            true
        } else {
            false
        }
    }

    fn flush_active_cell(&mut self) {
        if let Some(active) = self.active_cell.take() {
            self.needs_final_message_separator = true;
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
                .active_cell
                .as_ref()
                .is_some_and(|c| c.as_any().is::<history_cell::SessionHeaderHistoryCell>());

        if !keep_placeholder_header_active && !cell.display_lines(u16::MAX).is_empty() {
            // Only break exec grouping if the cell renders visible lines.
            self.flush_active_cell();
            self.needs_final_message_separator = true;
        }
        self.app_event_tx.send(AppEvent::InsertHistoryCell(cell));
    }

    fn queue_user_message(&mut self, user_message: UserMessage) {
        if !self.is_session_configured() || self.bottom_pane.is_task_running() {
            self.queued_user_messages.push_back(user_message);
            self.refresh_pending_input_preview();
        } else {
            self.submit_user_message(user_message);
        }
    }

    fn submit_user_message(&mut self, user_message: UserMessage) {
        if !self.is_session_configured() {
            tracing::warn!("cannot submit user message before session is configured; queueing");
            self.queued_user_messages.push_front(user_message);
            self.refresh_pending_input_preview();
            return;
        }
        let UserMessage {
            text,
            local_images,
            remote_image_urls,
            text_elements,
            mention_bindings,
        } = user_message;
        if text.is_empty() && local_images.is_empty() && remote_image_urls.is_empty() {
            return;
        }
        if (!local_images.is_empty() || !remote_image_urls.is_empty())
            && !self.current_model_supports_images()
        {
            self.restore_blocked_image_submission(
                text,
                text_elements,
                local_images,
                mention_bindings,
                remote_image_urls,
            );
            return;
        }

        let render_in_history = !self.agent_turn_running;
        let mut items: Vec<UserInput> = Vec::new();

        // Special-case: "!cmd" executes a local shell command instead of sending to the model.
        if let Some(stripped) = text.strip_prefix('!') {
            let cmd = stripped.trim();
            if cmd.is_empty() {
                self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                    history_cell::new_info_event(
                        USER_SHELL_COMMAND_HELP_TITLE.to_string(),
                        Some(USER_SHELL_COMMAND_HELP_HINT.to_string()),
                    ),
                )));
                return;
            }
            self.submit_op(AppCommand::run_user_shell_command(cmd.to_string()));
            return;
        }

        for image_url in &remote_image_urls {
            items.push(UserInput::Image {
                image_url: image_url.clone(),
            });
        }

        for image in &local_images {
            items.push(UserInput::LocalImage {
                path: image.path.clone(),
            });
        }

        if !text.is_empty() {
            items.push(UserInput::Text {
                text: text.clone(),
                text_elements: text_elements.clone(),
            });
        }

        let mentions = collect_tool_mentions(&text, &HashMap::new());
        let bound_names: HashSet<String> = mention_bindings
            .iter()
            .map(|binding| binding.mention.clone())
            .collect();
        let mut skill_names_lower: HashSet<String> = HashSet::new();
        let mut selected_skill_paths: HashSet<PathBuf> = HashSet::new();
        let mut selected_plugin_ids: HashSet<String> = HashSet::new();

        if let Some(skills) = self.bottom_pane.skills() {
            skill_names_lower = skills
                .iter()
                .map(|skill| skill.name.to_ascii_lowercase())
                .collect();

            for binding in &mention_bindings {
                let path = binding
                    .path
                    .strip_prefix("skill://")
                    .unwrap_or(binding.path.as_str());
                let path = Path::new(path);
                if let Some(skill) = skills
                    .iter()
                    .find(|skill| skill.path_to_skills_md.as_path() == path)
                    && selected_skill_paths.insert(skill.path_to_skills_md.clone())
                {
                    items.push(UserInput::Skill {
                        name: skill.name.clone(),
                        path: skill.path_to_skills_md.clone(),
                    });
                }
            }

            let skill_mentions = find_skill_mentions_with_tool_mentions(&mentions, skills);
            for skill in skill_mentions {
                if bound_names.contains(skill.name.as_str())
                    || !selected_skill_paths.insert(skill.path_to_skills_md.clone())
                {
                    continue;
                }
                items.push(UserInput::Skill {
                    name: skill.name.clone(),
                    path: skill.path_to_skills_md.clone(),
                });
            }
        }

        if let Some(plugins) = self.plugins_for_mentions() {
            for binding in &mention_bindings {
                let Some(plugin_config_name) = binding
                    .path
                    .strip_prefix("plugin://")
                    .filter(|id| !id.is_empty())
                else {
                    continue;
                };
                if !selected_plugin_ids.insert(plugin_config_name.to_string()) {
                    continue;
                }
                if let Some(plugin) = plugins
                    .iter()
                    .find(|plugin| plugin.config_name == plugin_config_name)
                {
                    items.push(UserInput::Mention {
                        name: plugin.display_name.clone(),
                        path: binding.path.clone(),
                    });
                }
            }
        }

        let mut selected_app_ids: HashSet<String> = HashSet::new();
        if let Some(apps) = self.connectors_for_mentions() {
            for binding in &mention_bindings {
                let Some(app_id) = binding
                    .path
                    .strip_prefix("app://")
                    .filter(|id| !id.is_empty())
                else {
                    continue;
                };
                if !selected_app_ids.insert(app_id.to_string()) {
                    continue;
                }
                if let Some(app) = apps.iter().find(|app| app.id == app_id && app.is_enabled) {
                    items.push(UserInput::Mention {
                        name: app.name.clone(),
                        path: binding.path.clone(),
                    });
                }
            }

            let app_mentions = find_app_mentions(&mentions, apps, &skill_names_lower);
            for app in app_mentions {
                let slug = crate::legacy_core::connectors::connector_mention_slug(&app);
                if bound_names.contains(&slug) || !selected_app_ids.insert(app.id.clone()) {
                    continue;
                }
                let app_id = app.id.as_str();
                items.push(UserInput::Mention {
                    name: app.name.clone(),
                    path: format!("app://{app_id}"),
                });
            }
        }

        let effective_mode = self.effective_collaboration_mode();
        if effective_mode.model().trim().is_empty() {
            self.add_error_message(
                "Thread model is unavailable. Wait for the thread to finish syncing or choose a model before sending input.".to_string(),
            );
            return;
        }
        let collaboration_mode = if self.collaboration_modes_enabled() {
            self.active_collaboration_mask
                .as_ref()
                .map(|_| effective_mode.clone())
        } else {
            None
        };
        let pending_steer = (!render_in_history).then(|| PendingSteer {
            user_message: UserMessage {
                text: text.clone(),
                local_images: local_images.clone(),
                remote_image_urls: remote_image_urls.clone(),
                text_elements: text_elements.clone(),
                mention_bindings: mention_bindings.clone(),
            },
            compare_key: Self::pending_steer_compare_key_from_items(&items),
        });
        let personality = self
            .config
            .personality
            .filter(|_| self.config.features.enabled(Feature::Personality))
            .filter(|_| self.current_model_supports_personality());
        let service_tier = Some(self.config.service_tier);
        let op = AppCommand::user_turn(
            items,
            self.config.cwd.to_path_buf(),
            self.config.permissions.approval_policy.value(),
            self.config.permissions.sandbox_policy.get().clone(),
            effective_mode.model().to_string(),
            effective_mode.reasoning_effort(),
            /*summary*/ None,
            service_tier,
            /*final_output_json_schema*/ None,
            collaboration_mode,
            personality,
        );

        if !self.submit_op(op) {
            return;
        }

        // Persist the text to cross-session message history. Mentions are
        // encoded into placeholder syntax so recall can reconstruct the
        // mention bindings in a future session.
        if !text.is_empty() {
            let encoded_mentions = mention_bindings
                .iter()
                .map(|binding| LinkedMention {
                    mention: binding.mention.clone(),
                    path: binding.path.clone(),
                })
                .collect::<Vec<_>>();
            let history_text = encode_history_mentions(&text, &encoded_mentions);
            self.submit_op(Op::AddToHistory { text: history_text });
        }

        if let Some(pending_steer) = pending_steer {
            self.pending_steers.push_back(pending_steer);
            self.saw_plan_item_this_turn = false;
            self.refresh_pending_input_preview();
        }

        // Show replayable user content in conversation history.
        if render_in_history && !text.is_empty() {
            let local_image_paths = local_images
                .into_iter()
                .map(|img| img.path)
                .collect::<Vec<_>>();
            self.last_rendered_user_message_event =
                Some(Self::rendered_user_message_event_from_parts(
                    text.clone(),
                    text_elements.clone(),
                    local_image_paths.clone(),
                    remote_image_urls.clone(),
                ));
            self.add_to_history(history_cell::new_user_prompt(
                text,
                text_elements,
                local_image_paths,
                remote_image_urls,
            ));
        } else if render_in_history && !remote_image_urls.is_empty() {
            self.last_rendered_user_message_event =
                Some(Self::rendered_user_message_event_from_parts(
                    String::new(),
                    Vec::new(),
                    Vec::new(),
                    remote_image_urls.clone(),
                ));
            self.add_to_history(history_cell::new_user_prompt(
                String::new(),
                Vec::new(),
                Vec::new(),
                remote_image_urls,
            ));
        }

        self.needs_final_message_separator = false;
    }

    /// Restore the blocked submission draft without losing mention resolution state.
    ///
    /// The blocked-image path intentionally keeps the draft in the composer so
    /// users can remove attachments and retry. We must restore
    /// mention bindings alongside visible text; restoring only `$name` tokens
    /// makes the draft look correct while degrading mention resolution to
    /// name-only heuristics on retry.
    fn restore_blocked_image_submission(
        &mut self,
        text: String,
        text_elements: Vec<TextElement>,
        local_images: Vec<LocalImageAttachment>,
        mention_bindings: Vec<MentionBinding>,
        remote_image_urls: Vec<String>,
    ) {
        // Preserve the user's composed payload so they can retry after changing models.
        let local_image_paths = local_images.iter().map(|img| img.path.clone()).collect();
        self.set_remote_image_urls(remote_image_urls);
        self.bottom_pane.set_composer_text_with_mention_bindings(
            text,
            text_elements,
            local_image_paths,
            mention_bindings,
        );
        self.add_to_history(history_cell::new_warning_event(
            self.image_inputs_not_supported_message(),
        ));
        self.request_redraw();
    }

    /// Replay a subset of initial events into the UI to seed the transcript when
    /// resuming an existing session. This approximates the live event flow and
    /// is intentionally conservative: only safe-to-replay items are rendered to
    /// avoid triggering side effects. Event ids are passed as `None` to
    /// distinguish replayed events from live ones.
    pub(crate) fn replay_thread_turns(&mut self, turns: Vec<Turn>, replay_kind: ReplayKind) {
        for turn in turns {
            let Turn {
                id: turn_id,
                items,
                status,
                error,
                started_at,
                completed_at,
                duration_ms,
            } = turn;
            if matches!(status, TurnStatus::InProgress) {
                self.last_non_retry_error = None;
                self.on_task_started();
            }
            for item in items {
                self.replay_thread_item(item, turn_id.clone(), replay_kind);
            }
            if matches!(
                status,
                TurnStatus::Completed | TurnStatus::Interrupted | TurnStatus::Failed
            ) {
                self.handle_turn_completed_notification(
                    TurnCompletedNotification {
                        thread_id: self.thread_id.map(|id| id.to_string()).unwrap_or_default(),
                        turn: Turn {
                            id: turn_id,
                            items: Vec::new(),
                            status,
                            error,
                            started_at,
                            completed_at,
                            duration_ms,
                        },
                    },
                    Some(replay_kind),
                );
            }
        }
    }

    pub(crate) fn replay_thread_item(
        &mut self,
        item: ThreadItem,
        turn_id: String,
        replay_kind: ReplayKind,
    ) {
        self.handle_thread_item(item, turn_id, ThreadItemRenderSource::Replay(replay_kind));
    }

    fn handle_thread_item(
        &mut self,
        item: ThreadItem,
        turn_id: String,
        render_source: ThreadItemRenderSource,
    ) {
        let from_replay = render_source.is_replay();
        let replay_kind = render_source.replay_kind();
        match item {
            ThreadItem::UserMessage { id, content } => {
                let user_message = codex_protocol::items::UserMessageItem {
                    id,
                    content: content
                        .into_iter()
                        .map(codex_app_server_protocol::UserInput::into_core)
                        .collect(),
                };
                let codex_protocol::protocol::EventMsg::UserMessage(event) =
                    user_message.as_legacy_event()
                else {
                    unreachable!("user message item should convert to a user message event");
                };
                if from_replay {
                    self.on_user_message_event(event);
                } else {
                    let rendered = Self::rendered_user_message_event_from_event(&event);
                    let compare_key =
                        Self::pending_steer_compare_key_from_items(&user_message.content);
                    if self
                        .pending_steers
                        .front()
                        .is_some_and(|pending| pending.compare_key == compare_key)
                    {
                        if let Some(pending) = self.pending_steers.pop_front() {
                            self.refresh_pending_input_preview();
                            let pending_event = UserMessageEvent {
                                message: pending.user_message.text,
                                images: Some(pending.user_message.remote_image_urls),
                                local_images: pending
                                    .user_message
                                    .local_images
                                    .into_iter()
                                    .map(|image| image.path)
                                    .collect(),
                                text_elements: pending.user_message.text_elements,
                            };
                            self.on_user_message_event(pending_event);
                        } else if self.last_rendered_user_message_event.as_ref() != Some(&rendered)
                        {
                            tracing::warn!(
                                "pending steer matched compare key but queue was empty when rendering committed user message"
                            );
                            self.on_user_message_event(event);
                        }
                    } else if self.last_rendered_user_message_event.as_ref() != Some(&rendered) {
                        self.on_user_message_event(event);
                    }
                }
            }
            ThreadItem::AgentMessage {
                id,
                text,
                phase,
                memory_citation,
            } => {
                self.on_agent_message_item_completed(AgentMessageItem {
                    id,
                    content: vec![AgentMessageContent::Text { text }],
                    phase,
                    memory_citation: memory_citation.map(|citation| {
                        codex_protocol::memory_citation::MemoryCitation {
                            entries: citation
                                .entries
                                .into_iter()
                                .map(
                                    |entry| codex_protocol::memory_citation::MemoryCitationEntry {
                                        path: entry.path,
                                        line_start: entry.line_start,
                                        line_end: entry.line_end,
                                        note: entry.note,
                                    },
                                )
                                .collect(),
                            rollout_ids: citation.thread_ids,
                        }
                    }),
                });
            }
            ThreadItem::Plan { text, .. } => self.on_plan_item_completed(text),
            ThreadItem::Reasoning {
                summary, content, ..
            } => {
                if from_replay {
                    for delta in summary {
                        self.on_agent_reasoning_delta(delta);
                    }
                    if self.config.show_raw_agent_reasoning {
                        for delta in content {
                            self.on_agent_reasoning_delta(delta);
                        }
                    }
                }
                self.on_agent_reasoning_final();
            }
            ThreadItem::CommandExecution {
                id,
                command,
                cwd,
                process_id,
                source,
                status,
                command_actions,
                aggregated_output,
                exit_code,
                duration_ms,
            } => {
                if matches!(
                    status,
                    codex_app_server_protocol::CommandExecutionStatus::InProgress
                ) {
                    self.on_exec_command_begin(ExecCommandBeginEvent {
                        call_id: id,
                        process_id,
                        turn_id: turn_id.clone(),
                        command: split_command_string(&command),
                        cwd,
                        parsed_cmd: command_actions
                            .into_iter()
                            .map(codex_app_server_protocol::CommandAction::into_core)
                            .collect(),
                        source: source.to_core(),
                        interaction_input: None,
                    });
                } else {
                    let aggregated_output = aggregated_output.unwrap_or_default();
                    self.on_exec_command_end(ExecCommandEndEvent {
                        call_id: id,
                        process_id,
                        turn_id: turn_id.clone(),
                        command: split_command_string(&command),
                        cwd,
                        parsed_cmd: command_actions
                            .into_iter()
                            .map(codex_app_server_protocol::CommandAction::into_core)
                            .collect(),
                        source: source.to_core(),
                        interaction_input: None,
                        stdout: String::new(),
                        stderr: String::new(),
                        aggregated_output: aggregated_output.clone(),
                        exit_code: exit_code.unwrap_or_default(),
                        duration: Duration::from_millis(
                            duration_ms.unwrap_or_default().max(0) as u64
                        ),
                        formatted_output: aggregated_output,
                        status: match status {
                            codex_app_server_protocol::CommandExecutionStatus::Completed => {
                                codex_protocol::protocol::ExecCommandStatus::Completed
                            }
                            codex_app_server_protocol::CommandExecutionStatus::Failed => {
                                codex_protocol::protocol::ExecCommandStatus::Failed
                            }
                            codex_app_server_protocol::CommandExecutionStatus::Declined => {
                                codex_protocol::protocol::ExecCommandStatus::Declined
                            }
                            codex_app_server_protocol::CommandExecutionStatus::InProgress => {
                                codex_protocol::protocol::ExecCommandStatus::Failed
                            }
                        },
                    });
                }
            }
            ThreadItem::FileChange {
                id,
                changes,
                status,
            } => {
                if !matches!(
                    status,
                    codex_app_server_protocol::PatchApplyStatus::InProgress
                ) {
                    self.on_patch_apply_end(codex_protocol::protocol::PatchApplyEndEvent {
                        call_id: id,
                        turn_id: turn_id.clone(),
                        stdout: String::new(),
                        stderr: String::new(),
                        success: !matches!(
                            status,
                            codex_app_server_protocol::PatchApplyStatus::Failed
                        ),
                        changes: app_server_patch_changes_to_core(changes),
                        status: match status {
                            codex_app_server_protocol::PatchApplyStatus::Completed => {
                                codex_protocol::protocol::PatchApplyStatus::Completed
                            }
                            codex_app_server_protocol::PatchApplyStatus::Failed => {
                                codex_protocol::protocol::PatchApplyStatus::Failed
                            }
                            codex_app_server_protocol::PatchApplyStatus::Declined => {
                                codex_protocol::protocol::PatchApplyStatus::Declined
                            }
                            codex_app_server_protocol::PatchApplyStatus::InProgress => {
                                codex_protocol::protocol::PatchApplyStatus::Failed
                            }
                        },
                    });
                }
            }
            ThreadItem::McpToolCall {
                id,
                server,
                tool,
                arguments,
                result,
                error,
                duration_ms,
                ..
            } => {
                self.on_mcp_tool_call_end(codex_protocol::protocol::McpToolCallEndEvent {
                    call_id: id,
                    invocation: codex_protocol::protocol::McpInvocation {
                        server,
                        tool,
                        arguments: Some(arguments),
                    },
                    duration: Duration::from_millis(duration_ms.unwrap_or_default().max(0) as u64),
                    result: match (result, error) {
                        (_, Some(error)) => Err(error.message),
                        (Some(result), None) => Ok(codex_protocol::mcp::CallToolResult {
                            content: result.content,
                            structured_content: result.structured_content,
                            is_error: Some(false),
                            meta: None,
                        }),
                        (None, None) => Err("MCP tool call completed without a result".to_string()),
                    },
                });
            }
            ThreadItem::WebSearch { id, query, action } => {
                self.on_web_search_begin(WebSearchBeginEvent {
                    call_id: id.clone(),
                });
                self.on_web_search_end(WebSearchEndEvent {
                    call_id: id,
                    query,
                    action: action
                        .map(web_search_action_to_core)
                        .unwrap_or(codex_protocol::models::WebSearchAction::Other),
                });
            }
            ThreadItem::ImageView { id, path } => {
                self.on_view_image_tool_call(ViewImageToolCallEvent {
                    call_id: id,
                    path: path.into(),
                });
            }
            ThreadItem::ImageGeneration {
                id,
                status,
                revised_prompt,
                result,
                saved_path,
            } => {
                self.on_image_generation_end(ImageGenerationEndEvent {
                    call_id: id,
                    result,
                    revised_prompt,
                    status,
                    saved_path,
                });
            }
            ThreadItem::EnteredReviewMode { review, .. } => {
                if from_replay {
                    self.enter_review_mode_with_hint(review, /*from_replay*/ true);
                }
            }
            ThreadItem::ExitedReviewMode { .. } => {
                self.exit_review_mode_after_item();
            }
            ThreadItem::ContextCompaction { .. } => {
                self.on_agent_message("Context compacted".to_owned());
            }
            ThreadItem::HookPrompt { .. } => {}
            ThreadItem::CollabAgentToolCall {
                id,
                tool,
                status,
                sender_thread_id,
                receiver_thread_ids,
                prompt,
                model,
                reasoning_effort,
                agents_states,
            } => self.on_collab_agent_tool_call(ThreadItem::CollabAgentToolCall {
                id,
                tool,
                status,
                sender_thread_id,
                receiver_thread_ids,
                prompt,
                model,
                reasoning_effort,
                agents_states,
            }),
            ThreadItem::DynamicToolCall { .. } => {}
        }

        if matches!(replay_kind, Some(ReplayKind::ThreadSnapshot)) && turn_id.is_empty() {
            self.request_redraw();
        }
    }

    pub(crate) fn handle_server_request(
        &mut self,
        request: ServerRequest,
        replay_kind: Option<ReplayKind>,
    ) {
        let id = request.id().to_string();
        match request {
            ServerRequest::CommandExecutionRequestApproval { params, .. } => {
                self.on_exec_approval_request(id, exec_approval_request_from_params(params));
            }
            ServerRequest::FileChangeRequestApproval { params, .. } => {
                self.on_apply_patch_approval_request(
                    id,
                    patch_approval_request_from_params(params),
                );
            }
            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                self.on_mcp_server_elicitation_request(
                    app_server_request_id_to_mcp_request_id(&request_id),
                    params,
                );
            }
            ServerRequest::PermissionsRequestApproval { params, .. } => {
                self.on_request_permissions(request_permissions_from_params(params));
            }
            ServerRequest::ToolRequestUserInput { params, .. } => {
                self.on_request_user_input(request_user_input_from_params(params));
            }
            ServerRequest::DynamicToolCall { .. }
            | ServerRequest::ChatgptAuthTokensRefresh { .. }
            | ServerRequest::ApplyPatchApproval { .. }
            | ServerRequest::ExecCommandApproval { .. } => {
                if replay_kind.is_none() {
                    self.add_error_message(TUI_STUB_MESSAGE.to_string());
                }
            }
        }
    }

    pub(crate) fn handle_server_notification(
        &mut self,
        notification: ServerNotification,
        replay_kind: Option<ReplayKind>,
    ) {
        let from_replay = replay_kind.is_some();
        let is_resume_initial_replay =
            matches!(replay_kind, Some(ReplayKind::ResumeInitialMessages));
        let is_retry_error = matches!(
            &notification,
            ServerNotification::Error(ErrorNotification {
                will_retry: true,
                ..
            })
        );
        if !is_resume_initial_replay && !is_retry_error {
            self.restore_retry_status_header_if_present();
        }
        match notification {
            ServerNotification::ThreadTokenUsageUpdated(notification) => {
                self.set_token_info(Some(token_usage_info_from_app_server(
                    notification.token_usage,
                )));
            }
            ServerNotification::ThreadNameUpdated(notification) => {
                match ThreadId::from_string(&notification.thread_id) {
                    Ok(thread_id) => self.on_thread_name_updated(
                        codex_protocol::protocol::ThreadNameUpdatedEvent {
                            thread_id,
                            thread_name: notification.thread_name,
                        },
                    ),
                    Err(err) => {
                        tracing::warn!(
                            thread_id = notification.thread_id,
                            error = %err,
                            "ignoring app-server ThreadNameUpdated with invalid thread_id"
                        );
                    }
                }
            }
            ServerNotification::TurnStarted(notification) => {
                self.last_turn_id = Some(notification.turn.id);
                self.last_non_retry_error = None;
                if !matches!(replay_kind, Some(ReplayKind::ResumeInitialMessages)) {
                    self.on_task_started();
                }
            }
            ServerNotification::TurnCompleted(notification) => {
                self.handle_turn_completed_notification(notification, replay_kind);
            }
            ServerNotification::ItemStarted(notification) => {
                self.handle_item_started_notification(notification, replay_kind.is_some());
            }
            ServerNotification::ItemCompleted(notification) => {
                self.handle_item_completed_notification(notification, replay_kind);
            }
            ServerNotification::AgentMessageDelta(notification) => {
                self.on_agent_message_delta(notification.delta);
            }
            ServerNotification::PlanDelta(notification) => self.on_plan_delta(notification.delta),
            ServerNotification::ReasoningSummaryTextDelta(notification) => {
                self.on_agent_reasoning_delta(notification.delta);
            }
            ServerNotification::ReasoningTextDelta(notification) => {
                if self.config.show_raw_agent_reasoning {
                    self.on_agent_reasoning_delta(notification.delta);
                }
            }
            ServerNotification::ReasoningSummaryPartAdded(_) => self.on_reasoning_section_break(),
            ServerNotification::TerminalInteraction(notification) => {
                self.on_terminal_interaction(TerminalInteractionEvent {
                    call_id: notification.item_id,
                    process_id: notification.process_id,
                    stdin: notification.stdin,
                })
            }
            ServerNotification::CommandExecutionOutputDelta(notification) => {
                self.on_exec_command_output_delta(ExecCommandOutputDeltaEvent {
                    call_id: notification.item_id,
                    stream: codex_protocol::protocol::ExecOutputStream::Stdout,
                    chunk: notification.delta.into_bytes(),
                });
            }
            ServerNotification::FileChangeOutputDelta(notification) => {
                self.on_patch_apply_output_delta(notification.item_id, notification.delta);
            }
            ServerNotification::TurnDiffUpdated(notification) => {
                self.on_turn_diff(notification.diff)
            }
            ServerNotification::TurnPlanUpdated(notification) => {
                self.on_plan_update(UpdatePlanArgs {
                    explanation: notification.explanation,
                    plan: notification
                        .plan
                        .into_iter()
                        .map(|step| UpdatePlanItemArg {
                            step: step.step,
                            status: match step.status {
                                TurnPlanStepStatus::Pending => UpdatePlanItemStatus::Pending,
                                TurnPlanStepStatus::InProgress => UpdatePlanItemStatus::InProgress,
                                TurnPlanStepStatus::Completed => UpdatePlanItemStatus::Completed,
                            },
                        })
                        .collect(),
                })
            }
            ServerNotification::HookStarted(notification) => {
                self.on_hook_started(hook_started_event_from_notification(notification));
            }
            ServerNotification::HookCompleted(notification) => {
                self.on_hook_completed(hook_completed_event_from_notification(notification));
            }
            ServerNotification::Error(notification) => {
                if notification.will_retry {
                    if !from_replay {
                        self.on_stream_error(
                            notification.error.message,
                            notification.error.additional_details,
                        );
                    }
                } else {
                    self.last_non_retry_error = Some((
                        notification.turn_id.clone(),
                        notification.error.message.clone(),
                    ));
                    self.handle_non_retry_error(
                        notification.error.message,
                        notification.error.codex_error_info,
                    );
                }
            }
            ServerNotification::SkillsChanged(_) => {
                self.submit_op(AppCommand::list_skills(
                    Vec::new(),
                    /*force_reload*/ true,
                ));
            }
            ServerNotification::ModelRerouted(_) => {}
            ServerNotification::DeprecationNotice(notification) => {
                self.on_deprecation_notice(DeprecationNoticeEvent {
                    summary: notification.summary,
                    details: notification.details,
                })
            }
            ServerNotification::ConfigWarning(notification) => self.on_warning(
                notification
                    .details
                    .map(|details| format!("{}: {details}", notification.summary))
                    .unwrap_or(notification.summary),
            ),
            ServerNotification::McpServerStatusUpdated(notification) => {
                self.on_mcp_server_status_updated(notification)
            }
            ServerNotification::ItemGuardianApprovalReviewStarted(notification) => {
                self.on_guardian_review_notification(
                    notification.review_id,
                    notification.turn_id,
                    notification.review,
                    /*decision_source*/ None,
                    notification.action,
                );
            }
            ServerNotification::ItemGuardianApprovalReviewCompleted(notification) => {
                self.on_guardian_review_notification(
                    notification.review_id,
                    notification.turn_id,
                    notification.review,
                    Some(notification.decision_source),
                    notification.action,
                );
            }
            ServerNotification::ThreadClosed(_) => {
                if !from_replay {
                    self.on_shutdown_complete();
                }
            }
            ServerNotification::ThreadRealtimeStarted(notification) => {
                if !from_replay {
                    self.on_realtime_conversation_started(
                        codex_protocol::protocol::RealtimeConversationStartedEvent {
                            session_id: notification.session_id,
                            version: notification.version,
                        },
                    );
                }
            }
            ServerNotification::ThreadRealtimeItemAdded(notification) => {
                if !from_replay {
                    self.on_realtime_conversation_realtime(
                        codex_protocol::protocol::RealtimeConversationRealtimeEvent {
                            payload: codex_protocol::protocol::RealtimeEvent::ConversationItemAdded(
                                notification.item,
                            ),
                        },
                    );
                }
            }
            ServerNotification::ThreadRealtimeOutputAudioDelta(notification) => {
                if !from_replay {
                    self.on_realtime_conversation_realtime(
                        codex_protocol::protocol::RealtimeConversationRealtimeEvent {
                            payload: codex_protocol::protocol::RealtimeEvent::AudioOut(
                                notification.audio.into(),
                            ),
                        },
                    );
                }
            }
            ServerNotification::ThreadRealtimeError(notification) => {
                if !from_replay {
                    self.on_realtime_conversation_realtime(
                        codex_protocol::protocol::RealtimeConversationRealtimeEvent {
                            payload: codex_protocol::protocol::RealtimeEvent::Error(
                                notification.message,
                            ),
                        },
                    );
                }
            }
            ServerNotification::ThreadRealtimeClosed(notification) => {
                if !from_replay {
                    self.on_realtime_conversation_closed(
                        codex_protocol::protocol::RealtimeConversationClosedEvent {
                            reason: notification.reason,
                        },
                    );
                }
            }
            ServerNotification::ThreadRealtimeSdp(notification) => {
                if !from_replay {
                    self.on_realtime_conversation_sdp(notification.sdp);
                }
            }
            ServerNotification::ServerRequestResolved(_)
            | ServerNotification::AccountUpdated(_)
            | ServerNotification::AccountRateLimitsUpdated(_)
            | ServerNotification::ThreadStarted(_)
            | ServerNotification::ThreadStatusChanged(_)
            | ServerNotification::ThreadArchived(_)
            | ServerNotification::ThreadUnarchived(_)
            | ServerNotification::RawResponseItemCompleted(_)
            | ServerNotification::CommandExecOutputDelta(_)
            | ServerNotification::McpToolCallProgress(_)
            | ServerNotification::McpServerOauthLoginCompleted(_)
            | ServerNotification::AppListUpdated(_)
            | ServerNotification::FsChanged(_)
            | ServerNotification::FuzzyFileSearchSessionUpdated(_)
            | ServerNotification::FuzzyFileSearchSessionCompleted(_)
            | ServerNotification::ThreadRealtimeTranscriptUpdated(_)
            | ServerNotification::WindowsWorldWritableWarning(_)
            | ServerNotification::WindowsSandboxSetupCompleted(_)
            | ServerNotification::AccountLoginCompleted(_) => {}
            ServerNotification::ContextCompacted(_) => self.on_context_compacted(),
        }
    }

    pub(crate) fn handle_skills_list_response(&mut self, response: ListSkillsResponseEvent) {
        self.on_list_skills(response);
    }

    fn on_mcp_server_elicitation_request(
        &mut self,
        request_id: codex_protocol::mcp::RequestId,
        params: codex_app_server_protocol::McpServerElicitationRequestParams,
    ) {
        let request = codex_protocol::approvals::ElicitationRequestEvent {
            turn_id: params.turn_id,
            server_name: params.server_name,
            id: request_id,
            request: match params.request {
                codex_app_server_protocol::McpServerElicitationRequest::Form {
                    meta,
                    message,
                    requested_schema,
                } => codex_protocol::approvals::ElicitationRequest::Form {
                    meta,
                    message,
                    requested_schema: serde_json::to_value(requested_schema)
                        .unwrap_or(serde_json::Value::Null),
                },
                codex_app_server_protocol::McpServerElicitationRequest::Url {
                    meta,
                    message,
                    url,
                    elicitation_id,
                } => codex_protocol::approvals::ElicitationRequest::Url {
                    meta,
                    message,
                    url,
                    elicitation_id,
                },
            },
        };
        self.on_elicitation_request(request);
    }

    fn handle_turn_completed_notification(
        &mut self,
        notification: TurnCompletedNotification,
        replay_kind: Option<ReplayKind>,
    ) {
        match notification.turn.status {
            TurnStatus::Completed => {
                self.last_non_retry_error = None;
                self.on_task_complete(/*last_agent_message*/ None, replay_kind.is_some())
            }
            TurnStatus::Interrupted => {
                self.last_non_retry_error = None;
                self.on_interrupted_turn(TurnAbortReason::Interrupted);
            }
            TurnStatus::Failed => {
                if let Some(error) = notification.turn.error {
                    if self.last_non_retry_error.as_ref()
                        == Some(&(notification.turn.id.clone(), error.message.clone()))
                    {
                        self.last_non_retry_error = None;
                    } else {
                        self.handle_non_retry_error(error.message, error.codex_error_info);
                    }
                } else {
                    self.last_non_retry_error = None;
                    self.finalize_turn();
                    self.request_redraw();
                    self.maybe_send_next_queued_input();
                }
            }
            TurnStatus::InProgress => {}
        }
    }

    fn handle_item_started_notification(
        &mut self,
        notification: ItemStartedNotification,
        from_replay: bool,
    ) {
        match notification.item {
            ThreadItem::CommandExecution {
                id,
                command,
                cwd,
                process_id,
                source,
                command_actions,
                ..
            } => {
                self.on_exec_command_begin(ExecCommandBeginEvent {
                    call_id: id,
                    process_id,
                    turn_id: notification.turn_id,
                    command: split_command_string(&command),
                    cwd,
                    parsed_cmd: command_actions
                        .into_iter()
                        .map(codex_app_server_protocol::CommandAction::into_core)
                        .collect(),
                    source: source.to_core(),
                    interaction_input: None,
                });
            }
            ThreadItem::FileChange { id, changes, .. } => {
                self.on_patch_apply_begin(PatchApplyBeginEvent {
                    call_id: id,
                    turn_id: notification.turn_id,
                    auto_approved: false,
                    changes: app_server_patch_changes_to_core(changes),
                });
            }
            ThreadItem::McpToolCall {
                id,
                server,
                tool,
                arguments,
                ..
            } => {
                self.on_mcp_tool_call_begin(McpToolCallBeginEvent {
                    call_id: id,
                    invocation: codex_protocol::protocol::McpInvocation {
                        server,
                        tool,
                        arguments: Some(arguments),
                    },
                });
            }
            ThreadItem::WebSearch { id, .. } => {
                self.on_web_search_begin(WebSearchBeginEvent { call_id: id });
            }
            ThreadItem::ImageGeneration { id, .. } => {
                self.on_image_generation_begin(ImageGenerationBeginEvent { call_id: id });
            }
            ThreadItem::CollabAgentToolCall {
                id,
                tool,
                status,
                sender_thread_id,
                receiver_thread_ids,
                prompt,
                model,
                reasoning_effort,
                agents_states,
            } => self.on_collab_agent_tool_call(ThreadItem::CollabAgentToolCall {
                id,
                tool,
                status,
                sender_thread_id,
                receiver_thread_ids,
                prompt,
                model,
                reasoning_effort,
                agents_states,
            }),
            ThreadItem::EnteredReviewMode { review, .. } => {
                if !from_replay {
                    self.enter_review_mode_with_hint(review, /*from_replay*/ false);
                }
            }
            _ => {}
        }
    }

    fn handle_item_completed_notification(
        &mut self,
        notification: ItemCompletedNotification,
        replay_kind: Option<ReplayKind>,
    ) {
        self.handle_thread_item(
            notification.item,
            notification.turn_id,
            replay_kind.map_or(ThreadItemRenderSource::Live, ThreadItemRenderSource::Replay),
        );
    }

    fn on_patch_apply_output_delta(&mut self, _item_id: String, _delta: String) {}

    fn on_guardian_review_notification(
        &mut self,
        id: String,
        turn_id: String,
        review: codex_app_server_protocol::GuardianApprovalReview,
        decision_source: Option<codex_app_server_protocol::AutoReviewDecisionSource>,
        action: GuardianApprovalReviewAction,
    ) {
        self.on_guardian_assessment(GuardianAssessmentEvent {
            id,
            target_item_id: None,
            turn_id,
            status: match review.status {
                codex_app_server_protocol::GuardianApprovalReviewStatus::InProgress => {
                    GuardianAssessmentStatus::InProgress
                }
                codex_app_server_protocol::GuardianApprovalReviewStatus::Approved => {
                    GuardianAssessmentStatus::Approved
                }
                codex_app_server_protocol::GuardianApprovalReviewStatus::Denied => {
                    GuardianAssessmentStatus::Denied
                }
                codex_app_server_protocol::GuardianApprovalReviewStatus::TimedOut => {
                    GuardianAssessmentStatus::TimedOut
                }
                codex_app_server_protocol::GuardianApprovalReviewStatus::Aborted => {
                    GuardianAssessmentStatus::Aborted
                }
            },
            risk_level: review.risk_level.map(|risk_level| match risk_level {
                codex_app_server_protocol::GuardianRiskLevel::Low => {
                    codex_protocol::protocol::GuardianRiskLevel::Low
                }
                codex_app_server_protocol::GuardianRiskLevel::Medium => {
                    codex_protocol::protocol::GuardianRiskLevel::Medium
                }
                codex_app_server_protocol::GuardianRiskLevel::High => {
                    codex_protocol::protocol::GuardianRiskLevel::High
                }
                codex_app_server_protocol::GuardianRiskLevel::Critical => {
                    codex_protocol::protocol::GuardianRiskLevel::Critical
                }
            }),
            user_authorization: review.user_authorization.map(|user_authorization| {
                match user_authorization {
                    codex_app_server_protocol::GuardianUserAuthorization::Unknown => {
                        codex_protocol::protocol::GuardianUserAuthorization::Unknown
                    }
                    codex_app_server_protocol::GuardianUserAuthorization::Low => {
                        codex_protocol::protocol::GuardianUserAuthorization::Low
                    }
                    codex_app_server_protocol::GuardianUserAuthorization::Medium => {
                        codex_protocol::protocol::GuardianUserAuthorization::Medium
                    }
                    codex_app_server_protocol::GuardianUserAuthorization::High => {
                        codex_protocol::protocol::GuardianUserAuthorization::High
                    }
                }
            }),
            rationale: review.rationale,
            decision_source: decision_source.map(|source| match source {
                codex_app_server_protocol::AutoReviewDecisionSource::Agent => {
                    GuardianAssessmentDecisionSource::Agent
                }
            }),
            action: action.into(),
        });
    }

    #[cfg(test)]
    fn replay_initial_messages(&mut self, events: Vec<EventMsg>) {
        for msg in events {
            if matches!(
                msg,
                EventMsg::SessionConfigured(_) | EventMsg::ThreadNameUpdated(_)
            ) {
                continue;
            }
            // `id: None` indicates a synthetic/fake id coming from replay.
            self.dispatch_event_msg(
                /*id*/ None,
                msg,
                Some(ReplayKind::ResumeInitialMessages),
            );
        }
    }

    #[cfg(test)]
    pub(crate) fn handle_codex_event(&mut self, event: Event) {
        let Event { id, msg } = event;
        self.dispatch_event_msg(Some(id), msg, /*replay_kind*/ None);
    }

    #[cfg(test)]
    pub(crate) fn handle_codex_event_replay(&mut self, event: Event) {
        let Event { msg, .. } = event;
        if matches!(msg, EventMsg::ShutdownComplete) {
            return;
        }
        self.dispatch_event_msg(/*id*/ None, msg, Some(ReplayKind::ThreadSnapshot));
    }

    /// Dispatch a protocol `EventMsg` to the appropriate handler.
    ///
    /// `id` is `Some` for live events and `None` for replayed events from
    /// `replay_initial_messages()`. Callers should treat `None` as a "fake" id
    /// that must not be used to correlate follow-up actions.
    #[cfg(test)]
    fn dispatch_event_msg(
        &mut self,
        id: Option<String>,
        msg: EventMsg,
        replay_kind: Option<ReplayKind>,
    ) {
        let from_replay = replay_kind.is_some();
        let is_resume_initial_replay =
            matches!(replay_kind, Some(ReplayKind::ResumeInitialMessages));
        let is_stream_error = matches!(&msg, EventMsg::StreamError(_));
        if !is_resume_initial_replay && !is_stream_error {
            self.restore_retry_status_header_if_present();
        }

        match msg {
            EventMsg::AgentMessageDelta(_)
            | EventMsg::PlanDelta(_)
            | EventMsg::AgentReasoningDelta(_)
            | EventMsg::TerminalInteraction(_)
            | EventMsg::ExecCommandOutputDelta(_) => {}
            _ => {
                tracing::trace!("handle_codex_event: {:?}", msg);
            }
        }

        match msg {
            EventMsg::SessionConfigured(e) => self.on_session_configured(e),
            EventMsg::ThreadNameUpdated(e) => self.on_thread_name_updated(e),
            // NOTE: All three AgentMessage arms feed `record_agent_markdown` even
            // when the message is otherwise not rendered (thread-snapshot replay,
            // non-review live messages). This ensures the copy source stays
            // populated across replay, resume, and live paths.
            EventMsg::AgentMessage(AgentMessageEvent { message, .. })
                if matches!(replay_kind, Some(ReplayKind::ThreadSnapshot))
                    && !self.is_review_mode =>
            {
                if !message.is_empty() {
                    self.record_agent_markdown(&message);
                }
            }
            EventMsg::AgentMessage(AgentMessageEvent { message, .. })
                if from_replay || self.is_review_mode =>
            {
                if !message.is_empty() {
                    self.record_agent_markdown(&message);
                }
                // TODO(ccunningham): stop relying on legacy AgentMessage in review mode,
                // including thread-snapshot replay, and forward
                // ItemCompleted(TurnItem::AgentMessage(_)) instead.
                self.on_agent_message(message)
            }
            EventMsg::AgentMessage(AgentMessageEvent { message, .. }) => {
                if !message.is_empty() {
                    self.record_agent_markdown(&message);
                }
            }
            EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta }) => {
                self.on_agent_message_delta(delta)
            }
            EventMsg::PlanDelta(event) => self.on_plan_delta(event.delta),
            EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta })
            | EventMsg::AgentReasoningRawContentDelta(AgentReasoningRawContentDeltaEvent {
                delta,
            }) => self.on_agent_reasoning_delta(delta),
            EventMsg::AgentReasoning(AgentReasoningEvent { .. }) => self.on_agent_reasoning_final(),
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent { text }) => {
                self.on_agent_reasoning_delta(text);
                self.on_agent_reasoning_final();
            }
            EventMsg::AgentReasoningSectionBreak(_) => self.on_reasoning_section_break(),
            EventMsg::TurnStarted(event) => {
                let turn_id = event.turn_id;
                let model_context_window = event.model_context_window;
                self.last_turn_id = Some(turn_id);
                if !is_resume_initial_replay {
                    self.apply_turn_started_context_window(model_context_window);
                    self.on_task_started();
                }
            }
            EventMsg::TurnComplete(TurnCompleteEvent {
                last_agent_message, ..
            }) => {
                self.on_task_complete(last_agent_message, from_replay);
            }
            EventMsg::TokenCount(ev) => {
                self.set_token_info(ev.info);
                self.on_rate_limit_snapshot(ev.rate_limits);
            }
            EventMsg::Warning(WarningEvent { message }) => self.on_warning(message),
            EventMsg::GuardianAssessment(ev) => self.on_guardian_assessment(ev),
            EventMsg::ModelReroute(_) => {}
            EventMsg::Error(ErrorEvent {
                message,
                codex_error_info,
            }) => {
                if codex_error_info
                    .as_ref()
                    .is_some_and(|info| self.handle_steer_rejected_error(info))
                {
                } else if let Some(kind) = codex_error_info
                    .as_ref()
                    .and_then(core_rate_limit_error_kind)
                {
                    match kind {
                        RateLimitErrorKind::ServerOverloaded => {
                            self.on_server_overloaded_error(message)
                        }
                        RateLimitErrorKind::UsageLimit | RateLimitErrorKind::Generic => {
                            self.on_error(message)
                        }
                    }
                } else {
                    self.on_error(message);
                }
            }
            EventMsg::McpStartupUpdate(ev) => self.on_mcp_startup_update(ev),
            EventMsg::McpStartupComplete(ev) => self.on_mcp_startup_complete(ev),
            EventMsg::TurnAborted(ev) => match ev.reason {
                TurnAbortReason::Interrupted => {
                    self.on_interrupted_turn(ev.reason);
                }
                TurnAbortReason::Replaced => {
                    self.submit_pending_steers_after_interrupt = false;
                    self.pending_steers.clear();
                    self.refresh_pending_input_preview();
                    self.on_error("Turn aborted: replaced by a new task".to_owned())
                }
                TurnAbortReason::ReviewEnded => {
                    self.on_interrupted_turn(ev.reason);
                }
            },
            EventMsg::PlanUpdate(update) => self.on_plan_update(update),
            EventMsg::ExecApprovalRequest(ev) => {
                // For replayed events, synthesize an empty id (these should not occur).
                self.on_exec_approval_request(id.unwrap_or_default(), ev)
            }
            EventMsg::ApplyPatchApprovalRequest(ev) => {
                self.on_apply_patch_approval_request(id.unwrap_or_default(), ev)
            }
            EventMsg::ElicitationRequest(ev) => {
                self.on_elicitation_request(ev);
            }
            EventMsg::RequestUserInput(ev) => {
                self.on_request_user_input(ev);
            }
            EventMsg::RequestPermissions(ev) => {
                self.on_request_permissions(ev);
            }
            EventMsg::ExecCommandBegin(ev) => self.on_exec_command_begin(ev),
            EventMsg::TerminalInteraction(delta) => self.on_terminal_interaction(delta),
            EventMsg::ExecCommandOutputDelta(delta) => self.on_exec_command_output_delta(delta),
            EventMsg::PatchApplyBegin(ev) => self.on_patch_apply_begin(ev),
            EventMsg::PatchApplyEnd(ev) => self.on_patch_apply_end(ev),
            EventMsg::ExecCommandEnd(ev) => self.on_exec_command_end(ev),
            EventMsg::ViewImageToolCall(ev) => self.on_view_image_tool_call(ev),
            EventMsg::ImageGenerationBegin(ev) => self.on_image_generation_begin(ev),
            EventMsg::ImageGenerationEnd(ev) => self.on_image_generation_end(ev),
            EventMsg::McpToolCallBegin(ev) => self.on_mcp_tool_call_begin(ev),
            EventMsg::McpToolCallEnd(ev) => self.on_mcp_tool_call_end(ev),
            EventMsg::WebSearchBegin(ev) => self.on_web_search_begin(ev),
            EventMsg::WebSearchEnd(ev) => self.on_web_search_end(ev),
            EventMsg::GetHistoryEntryResponse(ev) => self.handle_history_entry_response(ev),
            EventMsg::McpListToolsResponse(ev) => self.on_list_mcp_tools(ev),
            EventMsg::ListSkillsResponse(ev) => self.on_list_skills(ev),
            EventMsg::SkillsUpdateAvailable => {
                self.submit_op(AppCommand::list_skills(
                    Vec::new(),
                    /*force_reload*/ true,
                ));
            }
            EventMsg::ShutdownComplete => self.on_shutdown_complete(),
            EventMsg::TurnDiff(TurnDiffEvent { unified_diff }) => self.on_turn_diff(unified_diff),
            EventMsg::DeprecationNotice(ev) => self.on_deprecation_notice(ev),
            EventMsg::BackgroundEvent(BackgroundEventEvent { message }) => {
                self.on_background_event(message)
            }
            EventMsg::UndoStarted(ev) => self.on_undo_started(ev),
            EventMsg::UndoCompleted(ev) => self.on_undo_completed(ev),
            EventMsg::StreamError(StreamErrorEvent {
                message,
                additional_details,
                ..
            }) => {
                if !is_resume_initial_replay {
                    self.on_stream_error(message, additional_details);
                }
            }
            EventMsg::UserMessage(ev) => {
                if from_replay || self.should_render_realtime_user_message_event(&ev) {
                    self.on_user_message_event(ev);
                }
            }
            EventMsg::EnteredReviewMode(review_request) => {
                self.on_entered_review_mode(review_request, from_replay)
            }
            EventMsg::ExitedReviewMode(review) => self.on_exited_review_mode(review),
            EventMsg::ContextCompacted(_) => self.on_context_compacted(),
            EventMsg::CollabAgentSpawnBegin(CollabAgentSpawnBeginEvent {
                call_id,
                model,
                reasoning_effort,
                ..
            }) => {
                self.pending_collab_spawn_requests.insert(
                    call_id,
                    multi_agents::SpawnRequestSummary {
                        model,
                        reasoning_effort,
                    },
                );
            }
            EventMsg::CollabAgentSpawnEnd(ev) => {
                let spawn_request = self.pending_collab_spawn_requests.remove(&ev.call_id);
                self.on_collab_event(multi_agents::spawn_end(ev, spawn_request.as_ref()));
            }
            EventMsg::CollabAgentInteractionBegin(_) => {}
            EventMsg::CollabAgentInteractionEnd(ev) => {
                self.on_collab_event(multi_agents::interaction_end(ev))
            }
            EventMsg::CollabWaitingBegin(ev) => {
                self.on_collab_event(multi_agents::waiting_begin(ev))
            }
            EventMsg::CollabWaitingEnd(ev) => self.on_collab_event(multi_agents::waiting_end(ev)),
            EventMsg::CollabCloseBegin(_) => {}
            EventMsg::CollabCloseEnd(ev) => self.on_collab_event(multi_agents::close_end(ev)),
            EventMsg::CollabResumeBegin(ev) => self.on_collab_event(multi_agents::resume_begin(ev)),
            EventMsg::CollabResumeEnd(ev) => self.on_collab_event(multi_agents::resume_end(ev)),
            EventMsg::ThreadRolledBack(rollback) => {
                if from_replay {
                    self.app_event_tx.send(AppEvent::ApplyThreadRollback {
                        num_turns: rollback.num_turns,
                    });
                }
            }
            EventMsg::RawResponseItem(_)
            | EventMsg::ItemStarted(_)
            | EventMsg::AgentMessageContentDelta(_)
            | EventMsg::ReasoningContentDelta(_)
            | EventMsg::ReasoningRawContentDelta(_)
            | EventMsg::DynamicToolCallRequest(_)
            | EventMsg::DynamicToolCallResponse(_)
            | EventMsg::RealtimeConversationListVoicesResponse(_) => {}
            EventMsg::HookStarted(event) => self.on_hook_started(event),
            EventMsg::HookCompleted(event) => self.on_hook_completed(event),
            EventMsg::RealtimeConversationStarted(ev) => {
                if !from_replay {
                    self.on_realtime_conversation_started(ev);
                }
            }
            EventMsg::RealtimeConversationSdp(ev) => {
                if !from_replay {
                    self.on_realtime_conversation_sdp(ev.sdp);
                }
            }
            EventMsg::RealtimeConversationRealtime(ev) => {
                if !from_replay {
                    self.on_realtime_conversation_realtime(ev);
                }
            }
            EventMsg::RealtimeConversationClosed(ev) => {
                if !from_replay {
                    self.on_realtime_conversation_closed(ev);
                }
            }
            EventMsg::ItemCompleted(event) => {
                let item = event.item;
                if !from_replay && let codex_protocol::items::TurnItem::UserMessage(item) = &item {
                    let EventMsg::UserMessage(event) = item.as_legacy_event() else {
                        unreachable!("user message item should convert to a legacy user message");
                    };
                    let rendered = Self::rendered_user_message_event_from_event(&event);
                    let compare_key = Self::pending_steer_compare_key_from_item(item);
                    if self
                        .pending_steers
                        .front()
                        .is_some_and(|pending| pending.compare_key == compare_key)
                    {
                        if let Some(pending) = self.pending_steers.pop_front() {
                            self.refresh_pending_input_preview();
                            let pending_event = UserMessageEvent {
                                message: pending.user_message.text,
                                images: Some(pending.user_message.remote_image_urls),
                                local_images: pending
                                    .user_message
                                    .local_images
                                    .into_iter()
                                    .map(|image| image.path)
                                    .collect(),
                                text_elements: pending.user_message.text_elements,
                            };
                            self.on_user_message_event(pending_event);
                        } else if self.last_rendered_user_message_event.as_ref() != Some(&rendered)
                        {
                            tracing::warn!(
                                "pending steer matched compare key but queue was empty when rendering committed user message"
                            );
                            self.on_user_message_event(event);
                        }
                    } else if self.last_rendered_user_message_event.as_ref() != Some(&rendered) {
                        self.on_user_message_event(event);
                    }
                }
                if let codex_protocol::items::TurnItem::Plan(plan_item) = &item {
                    self.on_plan_item_completed(plan_item.text.clone());
                }
                if let codex_protocol::items::TurnItem::AgentMessage(item) = item {
                    self.on_agent_message_item_completed(item);
                }
            }
        }

        if !from_replay && self.agent_turn_running {
            self.refresh_runtime_metrics();
        }
    }

    fn enter_review_mode_with_hint(&mut self, hint: String, from_replay: bool) {
        if self.pre_review_token_info.is_none() {
            self.pre_review_token_info = Some(self.token_info.clone());
        }
        if !from_replay && !self.bottom_pane.is_task_running() {
            self.bottom_pane.set_task_running(/*running*/ true);
        }
        self.is_review_mode = true;
        let banner = format!(">> Code review started: {hint} <<");
        self.add_to_history(history_cell::new_review_status_line(banner));
        self.request_redraw();
    }

    fn exit_review_mode_after_item(&mut self) {
        self.flush_answer_stream_with_separator();
        self.flush_interrupt_queue();
        self.flush_active_cell();
        self.is_review_mode = false;
        self.restore_pre_review_token_info();
        self.add_to_history(history_cell::new_review_status_line(
            "<< Code review finished >>".to_string(),
        ));
        self.request_redraw();
    }

    #[cfg(test)]
    fn on_entered_review_mode(&mut self, review: ReviewRequest, from_replay: bool) {
        let hint = review.user_facing_hint.unwrap_or_else(|| {
            crate::legacy_core::review_prompts::user_facing_hint(&review.target)
        });
        self.enter_review_mode_with_hint(hint, from_replay);
    }

    #[cfg(test)]
    fn on_exited_review_mode(&mut self, review: ExitedReviewModeEvent) {
        if let Some(output) = review.review_output {
            let review_markdown =
                crate::legacy_core::review_format::render_review_output_text(&output);
            self.record_agent_markdown(&review_markdown);
            self.flush_answer_stream_with_separator();
            self.flush_interrupt_queue();
            self.flush_active_cell();

            if output.findings.is_empty() {
                let explanation = output.overall_explanation.trim().to_string();
                if explanation.is_empty() {
                    tracing::error!("Reviewer failed to output a response.");
                    self.add_to_history(history_cell::new_error_event(
                        "Reviewer failed to output a response.".to_owned(),
                    ));
                } else {
                    // Show explanation when there are no structured findings.
                    let mut rendered: Vec<ratatui::text::Line<'static>> = vec!["".into()];
                    append_markdown(
                        &explanation,
                        /*width*/ None,
                        Some(self.config.cwd.as_path()),
                        &mut rendered,
                    );
                    let body_cell = AgentMessageCell::new(rendered, /*is_first_line*/ false);
                    self.app_event_tx
                        .send(AppEvent::InsertHistoryCell(Box::new(body_cell)));
                }
            }
            // Final message is rendered as part of the AgentMessage.
        }
        self.exit_review_mode_after_item();
    }

    fn on_user_message_event(&mut self, event: UserMessageEvent) {
        self.last_rendered_user_message_event =
            Some(Self::rendered_user_message_event_from_event(&event));
        let remote_image_urls = event.images.unwrap_or_default();
        if !event.message.trim().is_empty()
            || !event.text_elements.is_empty()
            || !remote_image_urls.is_empty()
        {
            self.add_to_history(history_cell::new_user_prompt(
                event.message,
                event.text_elements,
                event.local_images,
                remote_image_urls,
            ));
        }

        // User messages reset separator state so the next agent response doesn't add a stray break.
        self.needs_final_message_separator = false;
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
        // Wrapping avoids overflow; wraparound would require 2^64 bumps and at
        // worst causes a one-time cache-key collision.
        self.active_cell_revision = self.active_cell_revision.wrapping_add(1);
    }

    fn notify(&mut self, notification: Notification) {
        if !notification.allowed_for(&self.config.tui_notifications.notifications) {
            return;
        }
        if let Some(existing) = self.pending_notification.as_ref()
            && existing.priority() > notification.priority()
        {
            return;
        }
        self.pending_notification = Some(notification);
        self.request_redraw();
    }

    pub(crate) fn maybe_post_pending_notification(&mut self, tui: &mut crate::tui::Tui) {
        if let Some(notif) = self.pending_notification.take() {
            tui.notify(notif.display());
        }
    }

    /// Mark the active cell as failed (✗) and flush it into history.
    fn finalize_active_cell_as_failed(&mut self) {
        if let Some(mut cell) = self.active_cell.take() {
            // Insert finalized cell into history and keep grouping consistent.
            if let Some(exec) = cell.as_any_mut().downcast_mut::<ExecCell>() {
                exec.mark_failed();
            } else if let Some(tool) = cell.as_any_mut().downcast_mut::<McpToolCallCell>() {
                tool.mark_failed();
            }
            self.add_boxed_history(cell);
        }
    }

    // If idle and there are queued inputs, submit exactly one to start the next turn.
    pub(crate) fn maybe_send_next_queued_input(&mut self) {
        if self.suppress_queue_autosend {
            return;
        }
        if self.bottom_pane.is_task_running() {
            return;
        }
        if let Some(user_message) = self.pop_next_queued_user_message() {
            self.submit_user_message(user_message);
        }
        // Update the list to reflect the remaining queued messages (if any).
        self.refresh_pending_input_preview();
    }

    /// Rebuild and update the bottom-pane pending-input preview.
    fn refresh_pending_input_preview(&mut self) {
        let queued_messages: Vec<String> = self
            .queued_user_messages
            .iter()
            .map(|m| m.text.clone())
            .collect();
        let pending_steers: Vec<String> = self
            .pending_steers
            .iter()
            .map(|steer| steer.user_message.text.clone())
            .collect();
        let rejected_steers: Vec<String> = self
            .rejected_steers_queue
            .iter()
            .map(|message| message.text.clone())
            .collect();
        self.bottom_pane.set_pending_input_preview(
            queued_messages,
            pending_steers,
            rejected_steers,
        );
    }

    pub(crate) fn set_pending_thread_approvals(&mut self, threads: Vec<String>) {
        self.bottom_pane.set_pending_thread_approvals(threads);
    }

    pub(crate) fn add_diff_in_progress(&mut self) {
        self.request_redraw();
    }

    pub(crate) fn on_diff_complete(&mut self) {
        self.request_redraw();
    }

    pub(crate) fn add_status_output(
        &mut self,
        refreshing_rate_limits: bool,
        request_id: Option<u64>,
    ) {
        let default_usage = TokenUsage::default();
        let token_info = self.token_info.as_ref();
        let total_usage = token_info
            .map(|ti| &ti.total_token_usage)
            .unwrap_or(&default_usage);
        let collaboration_mode = self.collaboration_mode_label();
        let reasoning_effort_override = Some(self.effective_reasoning_effort());
        let rate_limit_snapshots: Vec<RateLimitSnapshotDisplay> = self
            .rate_limit_snapshots_by_limit_id
            .values()
            .cloned()
            .collect();
        let config = self.config.clone();
        let frame_requester = self.frame_requester.clone();
        let (cell, handle) = crate::status::new_status_output_with_rate_limits_handle(
            &self.config,
            self.status_account_display.as_ref(),
            token_info,
            total_usage,
            &self.thread_id,
            self.thread_name.clone(),
            self.forked_from,
            rate_limit_snapshots.as_slice(),
            self.plan_type,
            Local::now(),
            self.model_display_name(),
            collaboration_mode,
            reasoning_effort_override,
            "<none>".to_string(),
            refreshing_rate_limits,
        );
        let agents_summary_handle = handle.clone();
        tokio::spawn(async move {
            let agents_summary = match crate::status::discover_agents_summary(&config).await {
                Ok(summary) => summary,
                Err(err) => {
                    tracing::warn!(error = %err, "failed to discover project docs for /status");
                    "<none>".to_string()
                }
            };
            agents_summary_handle.finish_agents_summary_discovery(agents_summary);
            frame_requester.schedule_frame();
        });
        if let Some(request_id) = request_id {
            self.refreshing_status_outputs.push((request_id, handle));
        }
        self.add_to_history(cell);
    }

    pub(crate) fn finish_status_rate_limit_refresh(&mut self, request_id: u64) {
        if self.refreshing_status_outputs.is_empty() {
            return;
        }

        let rate_limit_snapshots: Vec<RateLimitSnapshotDisplay> = self
            .rate_limit_snapshots_by_limit_id
            .values()
            .cloned()
            .collect();
        let now = Local::now();
        let mut remaining = Vec::with_capacity(self.refreshing_status_outputs.len());
        let mut updated_any = false;
        for (pending_request_id, handle) in self.refreshing_status_outputs.drain(..) {
            if pending_request_id == request_id {
                updated_any = true;
                handle.finish_rate_limit_refresh(rate_limit_snapshots.as_slice(), now);
            } else {
                remaining.push((pending_request_id, handle));
            }
        }
        self.refreshing_status_outputs = remaining;
        if updated_any {
            self.request_redraw();
        }
    }

    pub(crate) fn add_debug_config_output(&mut self) {
        self.add_to_history(crate::debug_config::new_debug_config_output(
            &self.config,
            self.session_network_proxy.as_ref(),
        ));
    }

    fn open_status_line_setup(&mut self) {
        let configured_status_line_items = self.configured_status_line_items();
        let view = StatusLineSetupView::new(
            Some(configured_status_line_items.as_slice()),
            StatusLinePreviewData::from_iter(StatusLineItem::iter().filter_map(|item| {
                self.status_line_value_for_item(&item)
                    .map(|value| (item, value))
            })),
            self.app_event_tx.clone(),
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    fn open_terminal_title_setup(&mut self) {
        let configured_terminal_title_items = self.configured_terminal_title_items();
        self.terminal_title_setup_original_items = Some(self.config.tui_terminal_title.clone());
        let view = TerminalTitleSetupView::new(
            Some(configured_terminal_title_items.as_slice()),
            self.app_event_tx.clone(),
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    fn open_theme_picker(&mut self) {
        let codex_home = crate::legacy_core::config::find_codex_home().ok();
        let terminal_width = self
            .last_rendered_width
            .get()
            .and_then(|width| u16::try_from(width).ok());
        let params = crate::theme_picker::build_theme_picker_params(
            self.config.tui_theme.as_deref(),
            codex_home.as_deref(),
            terminal_width,
        );
        self.bottom_pane.show_selection_view(params);
    }

    fn status_line_context_window_size(&self) -> Option<i64> {
        self.token_info
            .as_ref()
            .and_then(|info| info.model_context_window)
            .or(self.config.model_context_window)
    }

    fn status_line_context_remaining_percent(&self) -> Option<i64> {
        let Some(context_window) = self.status_line_context_window_size() else {
            return Some(100);
        };
        let default_usage = TokenUsage::default();
        let usage = self
            .token_info
            .as_ref()
            .map(|info| &info.last_token_usage)
            .unwrap_or(&default_usage);
        Some(
            usage
                .percent_of_context_window_remaining(context_window)
                .clamp(0, 100),
        )
    }

    fn status_line_context_used_percent(&self) -> Option<i64> {
        let remaining = self.status_line_context_remaining_percent().unwrap_or(100);
        Some((100 - remaining).clamp(0, 100))
    }

    fn status_line_total_usage(&self) -> TokenUsage {
        self.token_info
            .as_ref()
            .map(|info| info.total_token_usage.clone())
            .unwrap_or_default()
    }

    fn status_line_limit_display(
        &self,
        window: Option<&RateLimitWindowDisplay>,
        label: &str,
    ) -> Option<String> {
        let window = window?;
        let remaining = (100.0f64 - window.used_percent).clamp(0.0f64, 100.0f64);
        Some(format!("{label} {remaining:.0}%"))
    }

    fn status_line_reasoning_effort_label(effort: Option<ReasoningEffortConfig>) -> &'static str {
        match effort {
            Some(ReasoningEffortConfig::Minimal) => "minimal",
            Some(ReasoningEffortConfig::Low) => "low",
            Some(ReasoningEffortConfig::Medium) => "medium",
            Some(ReasoningEffortConfig::High) => "high",
            Some(ReasoningEffortConfig::XHigh) => "xhigh",
            None | Some(ReasoningEffortConfig::None) => "default",
        }
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

    fn stop_rate_limit_poller(&mut self) {}

    pub(crate) fn refresh_connectors(&mut self, force_refetch: bool) {
        self.prefetch_connectors_with_options(force_refetch);
    }

    fn prefetch_connectors(&mut self) {
        self.prefetch_connectors_with_options(/*force_refetch*/ false);
    }

    fn prefetch_connectors_with_options(&mut self, force_refetch: bool) {
        if !self.connectors_enabled() {
            return;
        }
        if self.connectors_prefetch_in_flight {
            if force_refetch {
                self.connectors_force_refetch_pending = true;
            }
            return;
        }

        self.connectors_prefetch_in_flight = true;
        if !matches!(self.connectors_cache, ConnectorsCacheState::Ready(_)) {
            self.connectors_cache = ConnectorsCacheState::Loading;
        }

        let config = self.config.clone();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let accessible_result =
                match connectors::list_accessible_connectors_from_mcp_tools_with_options_and_status(
                    &config,
                    force_refetch,
                )
                .await
                {
                    Ok(connectors) => connectors,
                    Err(err) => {
                        app_event_tx.send(AppEvent::ConnectorsLoaded {
                            result: Err(format!("Failed to load apps: {err}")),
                            is_final: true,
                        });
                        return;
                    }
                };
            let should_schedule_force_refetch =
                !force_refetch && !accessible_result.codex_apps_ready;
            let accessible_connectors = accessible_result.connectors;

            app_event_tx.send(AppEvent::ConnectorsLoaded {
                result: Ok(ConnectorsSnapshot {
                    connectors: accessible_connectors.clone(),
                }),
                is_final: false,
            });

            let result: Result<ConnectorsSnapshot, String> = async {
                let all_connectors =
                    connectors::list_all_connectors_with_options(&config, force_refetch).await?;
                let connectors = connectors::merge_connectors_with_accessible(
                    all_connectors,
                    accessible_connectors,
                    /*all_connectors_loaded*/ true,
                );
                Ok(ConnectorsSnapshot { connectors })
            }
            .await
            .map_err(|err: anyhow::Error| format!("Failed to load apps: {err}"));

            app_event_tx.send(AppEvent::ConnectorsLoaded {
                result,
                is_final: true,
            });

            if should_schedule_force_refetch {
                app_event_tx.send(AppEvent::RefreshConnectors {
                    force_refetch: true,
                });
            }
        });
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn prefetch_rate_limits(&mut self) {
        self.stop_rate_limit_poller();
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn should_prefetch_rate_limits(&self) -> bool {
        self.config.model_provider.requires_openai_auth && self.has_chatgpt_account
    }

    fn lower_cost_preset(&self) -> Option<ModelPreset> {
        let models = self.model_catalog.try_list_models().ok()?;
        models
            .iter()
            .find(|preset| preset.show_in_picker && preset.model == NUDGE_MODEL_SLUG)
            .cloned()
    }

    fn rate_limit_switch_prompt_hidden(&self) -> bool {
        self.config
            .notices
            .hide_rate_limit_model_nudge
            .unwrap_or(false)
    }

    fn maybe_show_pending_rate_limit_prompt(&mut self) {
        if self.rate_limit_switch_prompt_hidden() {
            self.rate_limit_switch_prompt = RateLimitSwitchPromptState::Idle;
            return;
        }
        if !matches!(
            self.rate_limit_switch_prompt,
            RateLimitSwitchPromptState::Pending
        ) {
            return;
        }
        if let Some(preset) = self.lower_cost_preset() {
            self.open_rate_limit_switch_prompt(preset);
            self.rate_limit_switch_prompt = RateLimitSwitchPromptState::Shown;
        } else {
            self.rate_limit_switch_prompt = RateLimitSwitchPromptState::Idle;
        }
    }

    fn open_rate_limit_switch_prompt(&mut self, preset: ModelPreset) {
        let switch_model = preset.model;
        let switch_model_for_events = switch_model.clone();
        let default_effort: ReasoningEffortConfig = preset.default_reasoning_effort;

        let switch_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::CodexOp(
                AppCommand::override_turn_context(
                    /*cwd*/ None,
                    /*approval_policy*/ None,
                    /*approvals_reviewer*/ None,
                    /*sandbox_policy*/ None,
                    /*windows_sandbox_level*/ None,
                    Some(switch_model_for_events.clone()),
                    Some(Some(default_effort)),
                    /*summary*/ None,
                    /*service_tier*/ None,
                    /*collaboration_mode*/ None,
                    /*personality*/ None,
                )
                .into_core(),
            ));
            tx.send(AppEvent::UpdateModel(switch_model_for_events.clone()));
            tx.send(AppEvent::UpdateReasoningEffort(Some(default_effort)));
        })];

        let keep_actions: Vec<SelectionAction> = Vec::new();
        let never_actions: Vec<SelectionAction> = vec![Box::new(|tx| {
            tx.send(AppEvent::UpdateRateLimitSwitchPromptHidden(true));
            tx.send(AppEvent::PersistRateLimitSwitchPromptHidden);
        })];
        let description = if preset.description.is_empty() {
            Some("Uses fewer credits for upcoming turns.".to_string())
        } else {
            Some(preset.description)
        };

        let items = vec![
            SelectionItem {
                name: format!("Switch to {switch_model}"),
                description,
                selected_description: None,
                is_current: false,
                actions: switch_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Keep current model".to_string(),
                description: None,
                selected_description: None,
                is_current: false,
                actions: keep_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Keep current model (never show again)".to_string(),
                description: Some(
                    "Hide future rate limit reminders about switching models.".to_string(),
                ),
                selected_description: None,
                is_current: false,
                actions: never_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Approaching rate limits".to_string()),
            subtitle: Some(format!("Switch to {switch_model} for lower credit usage?")),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    /// Open a popup to choose a quick auto model. Selecting "All models"
    /// opens the full picker with every available preset.
    pub(crate) fn open_model_popup(&mut self) {
        if !self.is_session_configured() {
            self.add_info_message(
                "Model selection is disabled until startup completes.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let presets: Vec<ModelPreset> = match self.model_catalog.try_list_models() {
            Ok(models) => models,
            Err(_) => {
                self.add_info_message(
                    "Models are being updated; please try /model again in a moment.".to_string(),
                    /*hint*/ None,
                );
                return;
            }
        };
        self.open_model_popup_with_presets(presets);
    }

    pub(crate) fn open_personality_popup(&mut self) {
        if !self.is_session_configured() {
            self.add_info_message(
                "Personality selection is disabled until startup completes.".to_string(),
                /*hint*/ None,
            );
            return;
        }
        if !self.current_model_supports_personality() {
            let current_model = self.current_model();
            self.add_error_message(format!(
                "Current model ({current_model}) doesn't support personalities. Try /model to pick a different model."
            ));
            return;
        }
        self.open_personality_popup_for_current_model();
    }

    fn open_personality_popup_for_current_model(&mut self) {
        let current_personality = self.config.personality.unwrap_or(Personality::Friendly);
        let personalities = [Personality::Friendly, Personality::Pragmatic];
        let supports_personality = self.current_model_supports_personality();

        let items: Vec<SelectionItem> = personalities
            .into_iter()
            .map(|personality| {
                let name = Self::personality_label(personality).to_string();
                let description = Some(Self::personality_description(personality).to_string());
                let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                    tx.send(AppEvent::CodexOp(
                        AppCommand::override_turn_context(
                            /*cwd*/ None,
                            /*approval_policy*/ None,
                            /*approvals_reviewer*/ None,
                            /*sandbox_policy*/ None,
                            /*windows_sandbox_level*/ None,
                            /*model*/ None,
                            /*effort*/ None,
                            /*summary*/ None,
                            /*service_tier*/ None,
                            /*collaboration_mode*/ None,
                            Some(personality),
                        )
                        .into_core(),
                    ));
                    tx.send(AppEvent::UpdatePersonality(personality));
                    tx.send(AppEvent::PersistPersonalitySelection { personality });
                })];
                SelectionItem {
                    name,
                    description,
                    is_current: current_personality == personality,
                    is_disabled: !supports_personality,
                    actions,
                    dismiss_on_select: true,
                    ..Default::default()
                }
            })
            .collect();

        let mut header = ColumnRenderable::new();
        header.push(Line::from("Select Personality".bold()));
        header.push(Line::from("Choose a communication style for Codex.".dim()));

        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(header),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    pub(crate) fn open_realtime_audio_popup(&mut self) {
        let items = [
            RealtimeAudioDeviceKind::Microphone,
            RealtimeAudioDeviceKind::Speaker,
        ]
        .into_iter()
        .map(|kind| {
            let description = Some(format!(
                "Current: {}",
                self.current_realtime_audio_selection_label(kind)
            ));
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenRealtimeAudioDeviceSelection { kind });
            })];
            SelectionItem {
                name: kind.title().to_string(),
                description,
                actions,
                dismiss_on_select: true,
                ..Default::default()
            }
        })
        .collect();

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Settings".to_string()),
            subtitle: Some("Configure settings for Codex.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn open_realtime_audio_device_selection(&mut self, kind: RealtimeAudioDeviceKind) {
        match list_realtime_audio_device_names(kind) {
            Ok(device_names) => {
                self.open_realtime_audio_device_selection_with_names(kind, device_names);
            }
            Err(err) => {
                self.add_error_message(format!(
                    "Failed to load realtime {} devices: {err}",
                    kind.noun()
                ));
            }
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn open_realtime_audio_device_selection(&mut self, kind: RealtimeAudioDeviceKind) {
        let _ = kind;
    }

    #[cfg(not(target_os = "linux"))]
    fn open_realtime_audio_device_selection_with_names(
        &mut self,
        kind: RealtimeAudioDeviceKind,
        device_names: Vec<String>,
    ) {
        let current_selection = self.current_realtime_audio_device_name(kind);
        let current_available = current_selection
            .as_deref()
            .is_some_and(|name| device_names.iter().any(|device_name| device_name == name));
        let mut items = vec![SelectionItem {
            name: "System default".to_string(),
            description: Some("Use your operating system default device.".to_string()),
            is_current: current_selection.is_none(),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::PersistRealtimeAudioDeviceSelection { kind, name: None });
            })],
            dismiss_on_select: true,
            ..Default::default()
        }];

        if let Some(selection) = current_selection.as_deref()
            && !current_available
        {
            items.push(SelectionItem {
                name: format!("Unavailable: {selection}"),
                description: Some("Configured device is not currently available.".to_string()),
                is_current: true,
                is_disabled: true,
                disabled_reason: Some("Reconnect the device or choose another one.".to_string()),
                ..Default::default()
            });
        }

        items.extend(device_names.into_iter().map(|device_name| {
            let persisted_name = device_name.clone();
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::PersistRealtimeAudioDeviceSelection {
                    kind,
                    name: Some(persisted_name.clone()),
                });
            })];
            SelectionItem {
                is_current: current_selection.as_deref() == Some(device_name.as_str()),
                name: device_name,
                actions,
                dismiss_on_select: true,
                ..Default::default()
            }
        }));

        let mut header = ColumnRenderable::new();
        header.push(Line::from(format!("Select {}", kind.title()).bold()));
        header.push(Line::from(
            "Saved devices apply to realtime voice only.".dim(),
        ));

        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(header),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    pub(crate) fn open_realtime_audio_restart_prompt(&mut self, kind: RealtimeAudioDeviceKind) {
        let restart_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::RestartRealtimeAudioDevice { kind });
        })];
        let items = vec![
            SelectionItem {
                name: "Restart now".to_string(),
                description: Some(format!("Restart local {} audio now.", kind.noun())),
                actions: restart_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Apply later".to_string(),
                description: Some(format!(
                    "Keep the current {} until local audio starts again.",
                    kind.noun()
                )),
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        let mut header = ColumnRenderable::new();
        header.push(Line::from(format!("Restart {} now?", kind.title()).bold()));
        header.push(Line::from(
            "Configuration is saved. Restart local audio to use it immediately.".dim(),
        ));

        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(header),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    fn model_menu_header(&self, title: &str, subtitle: &str) -> Box<dyn Renderable> {
        let title = title.to_string();
        let subtitle = subtitle.to_string();
        let mut header = ColumnRenderable::new();
        header.push(Line::from(title.bold()));
        header.push(Line::from(subtitle.dim()));
        if let Some(warning) = self.model_menu_warning_line() {
            header.push(warning);
        }
        Box::new(header)
    }

    fn model_menu_warning_line(&self) -> Option<Line<'static>> {
        let base_url = self.custom_openai_base_url()?;
        let warning = format!(
            "Warning: OpenAI base URL is overridden to {base_url}. Selecting models may not be supported or work properly."
        );
        Some(Line::from(warning.red()))
    }

    fn custom_openai_base_url(&self) -> Option<String> {
        if !self.config.model_provider.is_openai() {
            return None;
        }

        let base_url = self.config.model_provider.base_url.as_ref()?;
        let trimmed = base_url.trim();
        if trimmed.is_empty() {
            return None;
        }

        let normalized = trimmed.trim_end_matches('/');
        if normalized == DEFAULT_OPENAI_BASE_URL {
            return None;
        }

        Some(trimmed.to_string())
    }

    pub(crate) fn open_model_popup_with_presets(&mut self, presets: Vec<ModelPreset>) {
        let presets: Vec<ModelPreset> = presets
            .into_iter()
            .filter(|preset| preset.show_in_picker)
            .collect();

        let current_model = self.current_model();
        let current_label = presets
            .iter()
            .find(|preset| preset.model.as_str() == current_model)
            .map(|preset| preset.model.to_string())
            .unwrap_or_else(|| self.model_display_name().to_string());

        let (mut auto_presets, other_presets): (Vec<ModelPreset>, Vec<ModelPreset>) = presets
            .into_iter()
            .partition(|preset| Self::is_auto_model(&preset.model));

        if auto_presets.is_empty() {
            self.open_all_models_popup(other_presets);
            return;
        }

        auto_presets.sort_by_key(|preset| Self::auto_model_order(&preset.model));
        let mut items: Vec<SelectionItem> = auto_presets
            .into_iter()
            .map(|preset| {
                let description =
                    (!preset.description.is_empty()).then_some(preset.description.clone());
                let model = preset.model.clone();
                let should_prompt_plan_mode_scope = self.should_prompt_plan_mode_reasoning_scope(
                    model.as_str(),
                    Some(preset.default_reasoning_effort),
                );
                let actions = Self::model_selection_actions(
                    model.clone(),
                    Some(preset.default_reasoning_effort),
                    should_prompt_plan_mode_scope,
                );
                SelectionItem {
                    name: model.clone(),
                    description,
                    is_current: model.as_str() == current_model,
                    is_default: preset.is_default,
                    actions,
                    dismiss_on_select: true,
                    ..Default::default()
                }
            })
            .collect();

        if !other_presets.is_empty() {
            let all_models = other_presets;
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenAllModelsPopup {
                    models: all_models.clone(),
                });
            })];

            let is_current = !items.iter().any(|item| item.is_current);
            let description = Some(format!(
                "Choose a specific model and reasoning level (current: {current_label})"
            ));

            items.push(SelectionItem {
                name: "All models".to_string(),
                description,
                is_current,
                actions,
                dismiss_on_select: true,
                ..Default::default()
            });
        }

        let header = self.model_menu_header(
            "Select Model",
            "Pick a quick auto mode or browse all models.",
        );
        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header,
            ..Default::default()
        });
    }

    fn is_auto_model(model: &str) -> bool {
        model.starts_with("codex-auto-")
    }

    fn auto_model_order(model: &str) -> usize {
        match model {
            "codex-auto-fast" => 0,
            "codex-auto-balanced" => 1,
            "codex-auto-thorough" => 2,
            _ => 3,
        }
    }

    pub(crate) fn open_all_models_popup(&mut self, presets: Vec<ModelPreset>) {
        if presets.is_empty() {
            self.add_info_message(
                "No additional models are available right now.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let mut items: Vec<SelectionItem> = Vec::new();
        for preset in presets.into_iter() {
            let description =
                (!preset.description.is_empty()).then_some(preset.description.to_string());
            let is_current = preset.model.as_str() == self.current_model();
            let single_supported_effort = preset.supported_reasoning_efforts.len() == 1;
            let preset_for_action = preset.clone();
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                let preset_for_event = preset_for_action.clone();
                tx.send(AppEvent::OpenReasoningPopup {
                    model: preset_for_event,
                });
            })];
            items.push(SelectionItem {
                name: preset.model.clone(),
                description,
                is_current,
                is_default: preset.is_default,
                actions,
                dismiss_on_select: single_supported_effort,
                ..Default::default()
            });
        }

        let header = self.model_menu_header(
            "Select Model and Effort",
            "Access legacy models by running codex -m <model_name> or in your config.toml",
        );
        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some("Press enter to select reasoning effort, or esc to dismiss.".into()),
            items,
            header,
            ..Default::default()
        });
    }

    pub(crate) fn open_collaboration_modes_popup(&mut self) {
        let presets = collaboration_modes::presets_for_tui(self.model_catalog.as_ref());
        if presets.is_empty() {
            self.add_info_message(
                "No collaboration modes are available right now.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let current_kind = self
            .active_collaboration_mask
            .as_ref()
            .and_then(|mask| mask.mode)
            .or_else(|| {
                collaboration_modes::default_mask(self.model_catalog.as_ref())
                    .and_then(|mask| mask.mode)
            });
        let items: Vec<SelectionItem> = presets
            .into_iter()
            .map(|mask| {
                let name = mask.name.clone();
                let is_current = current_kind == mask.mode;
                let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                    tx.send(AppEvent::UpdateCollaborationMode(mask.clone()));
                })];
                SelectionItem {
                    name,
                    is_current,
                    actions,
                    dismiss_on_select: true,
                    ..Default::default()
                }
            })
            .collect();

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Select Collaboration Mode".to_string()),
            subtitle: Some("Pick a collaboration preset.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    fn model_selection_actions(
        model_for_action: String,
        effort_for_action: Option<ReasoningEffortConfig>,
        should_prompt_plan_mode_scope: bool,
    ) -> Vec<SelectionAction> {
        vec![Box::new(move |tx| {
            if should_prompt_plan_mode_scope {
                tx.send(AppEvent::OpenPlanReasoningScopePrompt {
                    model: model_for_action.clone(),
                    effort: effort_for_action,
                });
                return;
            }

            tx.send(AppEvent::UpdateModel(model_for_action.clone()));
            tx.send(AppEvent::UpdateReasoningEffort(effort_for_action));
            tx.send(AppEvent::PersistModelSelection {
                model: model_for_action.clone(),
                effort: effort_for_action,
            });
        })]
    }

    fn should_prompt_plan_mode_reasoning_scope(
        &self,
        selected_model: &str,
        selected_effort: Option<ReasoningEffortConfig>,
    ) -> bool {
        if !self.collaboration_modes_enabled()
            || self.active_mode_kind() != ModeKind::Plan
            || selected_model != self.current_model()
        {
            return false;
        }

        // Prompt whenever the selection is not a true no-op for both:
        // 1) the active Plan-mode effective reasoning, and
        // 2) the stored global defaults that would be updated by the fallback path.
        selected_effort != self.effective_reasoning_effort()
            || selected_model != self.current_collaboration_mode.model()
            || selected_effort != self.current_collaboration_mode.reasoning_effort()
    }

    pub(crate) fn open_plan_reasoning_scope_prompt(
        &mut self,
        model: String,
        effort: Option<ReasoningEffortConfig>,
    ) {
        let reasoning_phrase = match effort {
            Some(ReasoningEffortConfig::None) => "no reasoning".to_string(),
            Some(selected_effort) => {
                format!(
                    "{} reasoning",
                    Self::reasoning_effort_label(selected_effort).to_lowercase()
                )
            }
            None => "the selected reasoning".to_string(),
        };
        let plan_only_description = format!("Always use {reasoning_phrase} in Plan mode.");
        let plan_reasoning_source = if let Some(plan_override) =
            self.config.plan_mode_reasoning_effort
        {
            format!(
                "user-chosen Plan override ({})",
                Self::reasoning_effort_label(plan_override).to_lowercase()
            )
        } else if let Some(plan_mask) = collaboration_modes::plan_mask(self.model_catalog.as_ref())
        {
            match plan_mask.reasoning_effort.flatten() {
                Some(plan_effort) => format!(
                    "built-in Plan default ({})",
                    Self::reasoning_effort_label(plan_effort).to_lowercase()
                ),
                None => "built-in Plan default (no reasoning)".to_string(),
            }
        } else {
            "built-in Plan default".to_string()
        };
        let all_modes_description = format!(
            "Set the global default reasoning level and the Plan mode override. This replaces the current {plan_reasoning_source}."
        );
        let subtitle = format!("Choose where to apply {reasoning_phrase}.");

        let plan_only_actions: Vec<SelectionAction> = vec![Box::new({
            let model = model.clone();
            move |tx| {
                tx.send(AppEvent::UpdateModel(model.clone()));
                tx.send(AppEvent::UpdatePlanModeReasoningEffort(effort));
                tx.send(AppEvent::PersistPlanModeReasoningEffort(effort));
            }
        })];
        let all_modes_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::UpdateModel(model.clone()));
            tx.send(AppEvent::UpdateReasoningEffort(effort));
            tx.send(AppEvent::UpdatePlanModeReasoningEffort(effort));
            tx.send(AppEvent::PersistPlanModeReasoningEffort(effort));
            tx.send(AppEvent::PersistModelSelection {
                model: model.clone(),
                effort,
            });
        })];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some(PLAN_MODE_REASONING_SCOPE_TITLE.to_string()),
            subtitle: Some(subtitle),
            footer_hint: Some(standard_popup_hint_line()),
            items: vec![
                SelectionItem {
                    name: PLAN_MODE_REASONING_SCOPE_PLAN_ONLY.to_string(),
                    description: Some(plan_only_description),
                    actions: plan_only_actions,
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: PLAN_MODE_REASONING_SCOPE_ALL_MODES.to_string(),
                    description: Some(all_modes_description),
                    actions: all_modes_actions,
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        self.notify(Notification::PlanModePrompt {
            title: PLAN_MODE_REASONING_SCOPE_TITLE.to_string(),
        });
    }

    /// Open a popup to choose the reasoning effort (stage 2) for the given model.
    pub(crate) fn open_reasoning_popup(&mut self, preset: ModelPreset) {
        let default_effort: ReasoningEffortConfig = preset.default_reasoning_effort;
        let supported = preset.supported_reasoning_efforts;
        let in_plan_mode =
            self.collaboration_modes_enabled() && self.active_mode_kind() == ModeKind::Plan;

        let warn_effort = if supported
            .iter()
            .any(|option| option.effort == ReasoningEffortConfig::XHigh)
        {
            Some(ReasoningEffortConfig::XHigh)
        } else if supported
            .iter()
            .any(|option| option.effort == ReasoningEffortConfig::High)
        {
            Some(ReasoningEffortConfig::High)
        } else {
            None
        };
        let warning_text = warn_effort.map(|effort| {
            let effort_label = Self::reasoning_effort_label(effort);
            format!("⚠ {effort_label} reasoning effort can quickly consume Plus plan rate limits.")
        });
        let warn_for_model = preset.model.starts_with("gpt-5.1-codex")
            || preset.model.starts_with("gpt-5.1-codex-max")
            || preset.model.starts_with("gpt-5.2");

        struct EffortChoice {
            stored: Option<ReasoningEffortConfig>,
            display: ReasoningEffortConfig,
        }
        let mut choices: Vec<EffortChoice> = Vec::new();
        for effort in ReasoningEffortConfig::iter() {
            if supported.iter().any(|option| option.effort == effort) {
                choices.push(EffortChoice {
                    stored: Some(effort),
                    display: effort,
                });
            }
        }
        if choices.is_empty() {
            choices.push(EffortChoice {
                stored: Some(default_effort),
                display: default_effort,
            });
        }

        if choices.len() == 1 {
            let selected_effort = choices.first().and_then(|c| c.stored);
            let selected_model = preset.model;
            if self.should_prompt_plan_mode_reasoning_scope(&selected_model, selected_effort) {
                self.app_event_tx
                    .send(AppEvent::OpenPlanReasoningScopePrompt {
                        model: selected_model,
                        effort: selected_effort,
                    });
            } else {
                self.apply_model_and_effort(selected_model, selected_effort);
            }
            return;
        }

        let default_choice: Option<ReasoningEffortConfig> = choices
            .iter()
            .any(|choice| choice.stored == Some(default_effort))
            .then_some(Some(default_effort))
            .flatten()
            .or_else(|| choices.iter().find_map(|choice| choice.stored))
            .or(Some(default_effort));

        let model_slug = preset.model.to_string();
        let is_current_model = self.current_model() == preset.model.as_str();
        let highlight_choice = if is_current_model {
            if in_plan_mode {
                self.config
                    .plan_mode_reasoning_effort
                    .or(self.effective_reasoning_effort())
            } else {
                self.effective_reasoning_effort()
            }
        } else {
            default_choice
        };
        let selection_choice = highlight_choice.or(default_choice);
        let initial_selected_idx = choices
            .iter()
            .position(|choice| choice.stored == selection_choice)
            .or_else(|| {
                selection_choice
                    .and_then(|effort| choices.iter().position(|choice| choice.display == effort))
            });
        let mut items: Vec<SelectionItem> = Vec::new();
        for choice in choices.iter() {
            let effort = choice.display;
            let mut effort_label = Self::reasoning_effort_label(effort).to_string();
            if choice.stored == default_choice {
                effort_label.push_str(" (default)");
            }

            let description = choice
                .stored
                .and_then(|effort| {
                    supported
                        .iter()
                        .find(|option| option.effort == effort)
                        .map(|option| option.description.to_string())
                })
                .filter(|text| !text.is_empty());

            let show_warning = warn_for_model && warn_effort == Some(effort);
            let selected_description = if show_warning {
                warning_text.as_ref().map(|warning_message| {
                    description.as_ref().map_or_else(
                        || warning_message.clone(),
                        |d| format!("{d}\n{warning_message}"),
                    )
                })
            } else {
                None
            };

            let model_for_action = model_slug.clone();
            let choice_effort = choice.stored;
            let should_prompt_plan_mode_scope =
                self.should_prompt_plan_mode_reasoning_scope(model_slug.as_str(), choice_effort);
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                if should_prompt_plan_mode_scope {
                    tx.send(AppEvent::OpenPlanReasoningScopePrompt {
                        model: model_for_action.clone(),
                        effort: choice_effort,
                    });
                } else {
                    tx.send(AppEvent::UpdateModel(model_for_action.clone()));
                    tx.send(AppEvent::UpdateReasoningEffort(choice_effort));
                    tx.send(AppEvent::PersistModelSelection {
                        model: model_for_action.clone(),
                        effort: choice_effort,
                    });
                }
            })];

            items.push(SelectionItem {
                name: effort_label,
                description,
                selected_description,
                is_current: is_current_model && choice.stored == highlight_choice,
                actions,
                dismiss_on_select: true,
                ..Default::default()
            });
        }

        let mut header = ColumnRenderable::new();
        header.push(Line::from(
            format!("Select Reasoning Level for {model_slug}").bold(),
        ));

        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(header),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            initial_selected_idx,
            ..Default::default()
        });
    }

    fn reasoning_effort_label(effort: ReasoningEffortConfig) -> &'static str {
        match effort {
            ReasoningEffortConfig::None => "None",
            ReasoningEffortConfig::Minimal => "Minimal",
            ReasoningEffortConfig::Low => "Low",
            ReasoningEffortConfig::Medium => "Medium",
            ReasoningEffortConfig::High => "High",
            ReasoningEffortConfig::XHigh => "Extra high",
        }
    }

    fn apply_model_and_effort_without_persist(
        &self,
        model: String,
        effort: Option<ReasoningEffortConfig>,
    ) {
        self.app_event_tx.send(AppEvent::UpdateModel(model));
        self.app_event_tx
            .send(AppEvent::UpdateReasoningEffort(effort));
    }

    fn apply_model_and_effort(&self, model: String, effort: Option<ReasoningEffortConfig>) {
        self.apply_model_and_effort_without_persist(model.clone(), effort);
        self.app_event_tx
            .send(AppEvent::PersistModelSelection { model, effort });
    }

    /// Open the permissions popup (alias for /permissions).
    pub(crate) fn open_approvals_popup(&mut self) {
        self.open_permissions_popup();
    }

    /// Open a popup to choose the permissions mode (approval policy + sandbox policy).
    pub(crate) fn open_permissions_popup(&mut self) {
        let include_read_only = cfg!(target_os = "windows");
        let current_approval = self.config.permissions.approval_policy.value();
        let current_sandbox = self.config.permissions.sandbox_policy.get();
        let guardian_approval_enabled = self.config.features.enabled(Feature::GuardianApproval);
        let current_review_policy = self.config.approvals_reviewer;
        let mut items: Vec<SelectionItem> = Vec::new();
        let presets: Vec<ApprovalPreset> = builtin_approval_presets();

        #[cfg(target_os = "windows")]
        let windows_sandbox_level = WindowsSandboxLevel::from_config(&self.config);
        #[cfg(target_os = "windows")]
        let windows_degraded_sandbox_enabled =
            matches!(windows_sandbox_level, WindowsSandboxLevel::RestrictedToken);
        #[cfg(not(target_os = "windows"))]
        let windows_degraded_sandbox_enabled = false;

        let show_elevate_sandbox_hint =
            crate::legacy_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
                && windows_degraded_sandbox_enabled
                && presets.iter().any(|preset| preset.id == "auto");

        let guardian_disabled_reason = |enabled: bool| {
            let mut next_features = self.config.features.get().clone();
            next_features.set_enabled(Feature::GuardianApproval, enabled);
            self.config
                .features
                .can_set(&next_features)
                .err()
                .map(|err| err.to_string())
        };

        for preset in presets.into_iter() {
            if !include_read_only && preset.id == "read-only" {
                continue;
            }
            let base_name = if preset.id == "auto" && windows_degraded_sandbox_enabled {
                "Default (non-admin sandbox)".to_string()
            } else {
                preset.label.to_string()
            };
            let base_description =
                Some(preset.description.replace(" (Identical to Agent mode)", ""));
            let approval_disabled_reason = match self
                .config
                .permissions
                .approval_policy
                .can_set(&preset.approval)
            {
                Ok(()) => None,
                Err(err) => Some(err.to_string()),
            };
            let default_disabled_reason = approval_disabled_reason
                .clone()
                .or_else(|| guardian_disabled_reason(false));
            let requires_confirmation = preset.id == "full-access"
                && !self
                    .config
                    .notices
                    .hide_full_access_warning
                    .unwrap_or(false);
            let default_actions: Vec<SelectionAction> = if requires_confirmation {
                let preset_clone = preset.clone();
                vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenFullAccessConfirmation {
                        preset: preset_clone.clone(),
                        return_to_permissions: !include_read_only,
                    });
                })]
            } else if preset.id == "auto" {
                #[cfg(target_os = "windows")]
                {
                    if WindowsSandboxLevel::from_config(&self.config)
                        == WindowsSandboxLevel::Disabled
                    {
                        let preset_clone = preset.clone();
                        if crate::legacy_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
                            && crate::legacy_core::windows_sandbox::sandbox_setup_is_complete(
                                self.config.codex_home.as_path(),
                            )
                        {
                            vec![Box::new(move |tx| {
                                tx.send(AppEvent::EnableWindowsSandboxForAgentMode {
                                    preset: preset_clone.clone(),
                                    mode: WindowsSandboxEnableMode::Elevated,
                                });
                            })]
                        } else {
                            vec![Box::new(move |tx| {
                                tx.send(AppEvent::OpenWindowsSandboxEnablePrompt {
                                    preset: preset_clone.clone(),
                                });
                            })]
                        }
                    } else if let Some((sample_paths, extra_count, failed_scan)) =
                        self.world_writable_warning_details()
                    {
                        let preset_clone = preset.clone();
                        vec![Box::new(move |tx| {
                            tx.send(AppEvent::OpenWorldWritableWarningConfirmation {
                                preset: Some(preset_clone.clone()),
                                sample_paths: sample_paths.clone(),
                                extra_count,
                                failed_scan,
                            });
                        })]
                    } else {
                        Self::approval_preset_actions(
                            preset.approval,
                            preset.sandbox.clone(),
                            base_name.clone(),
                            ApprovalsReviewer::User,
                        )
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Self::approval_preset_actions(
                        preset.approval,
                        preset.sandbox.clone(),
                        base_name.clone(),
                        ApprovalsReviewer::User,
                    )
                }
            } else {
                Self::approval_preset_actions(
                    preset.approval,
                    preset.sandbox.clone(),
                    base_name.clone(),
                    ApprovalsReviewer::User,
                )
            };
            if preset.id == "auto" {
                items.push(SelectionItem {
                    name: base_name.clone(),
                    description: base_description.clone(),
                    is_current: current_review_policy == ApprovalsReviewer::User
                        && Self::preset_matches_current(current_approval, current_sandbox, &preset),
                    actions: default_actions,
                    dismiss_on_select: true,
                    disabled_reason: default_disabled_reason,
                    ..Default::default()
                });

                if guardian_approval_enabled {
                    items.push(SelectionItem {
                        name: "Guardian Approvals".to_string(),
                        description: Some(
                            "Same workspace-write permissions as Default, but eligible `on-request` approvals are routed through the guardian reviewer subagent."
                                .to_string(),
                        ),
                        is_current: current_review_policy == ApprovalsReviewer::GuardianSubagent
                            && Self::preset_matches_current(
                                current_approval,
                                current_sandbox,
                                &preset,
                            ),
                        actions: Self::approval_preset_actions(
                            preset.approval,
                            preset.sandbox.clone(),
                            "Guardian Approvals".to_string(),
                            ApprovalsReviewer::GuardianSubagent,
                        ),
                        dismiss_on_select: true,
                        disabled_reason: approval_disabled_reason
                            .or_else(|| guardian_disabled_reason(true)),
                        ..Default::default()
                    });
                }
            } else {
                items.push(SelectionItem {
                    name: base_name,
                    description: base_description,
                    is_current: Self::preset_matches_current(
                        current_approval,
                        current_sandbox,
                        &preset,
                    ),
                    actions: default_actions,
                    dismiss_on_select: true,
                    disabled_reason: default_disabled_reason,
                    ..Default::default()
                });
            }
        }

        let footer_note = show_elevate_sandbox_hint.then(|| {
            vec![
                "The non-admin sandbox protects your files and prevents network access under most circumstances. However, it carries greater risk if prompt injected. To upgrade to the default sandbox, run ".dim(),
                "/setup-default-sandbox".cyan(),
                ".".dim(),
            ]
            .into()
        });

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Update Model Permissions".to_string()),
            footer_note,
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(()),
            ..Default::default()
        });
    }

    pub(crate) fn open_experimental_popup(&mut self) {
        let features: Vec<ExperimentalFeatureItem> = FEATURES
            .iter()
            .filter_map(|spec| {
                let name = spec.stage.experimental_menu_name()?;
                let description = spec.stage.experimental_menu_description()?;
                Some(ExperimentalFeatureItem {
                    feature: spec.id,
                    name: name.to_string(),
                    description: description.to_string(),
                    enabled: self.config.features.enabled(spec.id),
                })
            })
            .collect();

        let view = ExperimentalFeaturesView::new(features, self.app_event_tx.clone());
        self.bottom_pane.show_view(Box::new(view));
    }

    fn approval_preset_actions(
        approval: AskForApproval,
        sandbox: SandboxPolicy,
        label: String,
        approvals_reviewer: ApprovalsReviewer,
    ) -> Vec<SelectionAction> {
        vec![Box::new(move |tx| {
            let sandbox_clone = sandbox.clone();
            tx.send(AppEvent::CodexOp(
                AppCommand::override_turn_context(
                    /*cwd*/ None,
                    Some(approval),
                    Some(approvals_reviewer),
                    Some(sandbox_clone.clone()),
                    /*windows_sandbox_level*/ None,
                    /*model*/ None,
                    /*effort*/ None,
                    /*summary*/ None,
                    /*service_tier*/ None,
                    /*collaboration_mode*/ None,
                    /*personality*/ None,
                )
                .into_core(),
            ));
            tx.send(AppEvent::UpdateAskForApprovalPolicy(approval));
            tx.send(AppEvent::UpdateSandboxPolicy(sandbox_clone));
            tx.send(AppEvent::UpdateApprovalsReviewer(approvals_reviewer));
            tx.send(AppEvent::InsertHistoryCell(Box::new(
                history_cell::new_info_event(
                    format!("Permissions updated to {label}"),
                    /*hint*/ None,
                ),
            )));
        })]
    }

    fn preset_matches_current(
        current_approval: AskForApproval,
        current_sandbox: &SandboxPolicy,
        preset: &ApprovalPreset,
    ) -> bool {
        if current_approval != preset.approval {
            return false;
        }

        match (current_sandbox, &preset.sandbox) {
            (SandboxPolicy::DangerFullAccess, SandboxPolicy::DangerFullAccess) => true,
            (
                SandboxPolicy::ReadOnly {
                    network_access: current_network_access,
                    ..
                },
                SandboxPolicy::ReadOnly {
                    network_access: preset_network_access,
                    ..
                },
            ) => current_network_access == preset_network_access,
            (
                SandboxPolicy::WorkspaceWrite {
                    network_access: current_network_access,
                    ..
                },
                SandboxPolicy::WorkspaceWrite {
                    network_access: preset_network_access,
                    ..
                },
            ) => current_network_access == preset_network_access,
            _ => false,
        }
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn world_writable_warning_details(&self) -> Option<(Vec<String>, usize, bool)> {
        if self
            .config
            .notices
            .hide_world_writable_warning
            .unwrap_or(false)
        {
            return None;
        }
        let cwd = self.config.cwd.clone();
        let env_map: std::collections::HashMap<String, String> = std::env::vars().collect();
        match codex_windows_sandbox::apply_world_writable_scan_and_denies(
            self.config.codex_home.as_path(),
            cwd.as_path(),
            &env_map,
            self.config.permissions.sandbox_policy.get(),
            Some(self.config.codex_home.as_path()),
        ) {
            Ok(_) => None,
            Err(_) => Some((Vec::new(), 0, true)),
        }
    }

    #[cfg(not(target_os = "windows"))]
    #[allow(dead_code)]
    pub(crate) fn world_writable_warning_details(&self) -> Option<(Vec<String>, usize, bool)> {
        None
    }

    pub(crate) fn open_full_access_confirmation(
        &mut self,
        preset: ApprovalPreset,
        return_to_permissions: bool,
    ) {
        let selected_name = preset.label.to_string();
        let approval = preset.approval;
        let sandbox = preset.sandbox;
        let mut header_children: Vec<Box<dyn Renderable>> = Vec::new();
        let title_line = Line::from("Enable full access?").bold();
        let info_line = Line::from(vec![
            "When Codex runs with full access, it can edit any file on your computer and run commands with network, without your approval. "
                .into(),
            "Exercise caution when enabling full access. This significantly increases the risk of data loss, leaks, or unexpected behavior."
                .fg(Color::Red),
        ]);
        header_children.push(Box::new(title_line));
        header_children.push(Box::new(
            Paragraph::new(vec![info_line]).wrap(Wrap { trim: false }),
        ));
        let header = ColumnRenderable::with(header_children);

        let mut accept_actions = Self::approval_preset_actions(
            approval,
            sandbox.clone(),
            selected_name.clone(),
            ApprovalsReviewer::User,
        );
        accept_actions.push(Box::new(|tx| {
            tx.send(AppEvent::UpdateFullAccessWarningAcknowledged(true));
        }));

        let mut accept_and_remember_actions = Self::approval_preset_actions(
            approval,
            sandbox,
            selected_name,
            ApprovalsReviewer::User,
        );
        accept_and_remember_actions.push(Box::new(|tx| {
            tx.send(AppEvent::UpdateFullAccessWarningAcknowledged(true));
            tx.send(AppEvent::PersistFullAccessWarningAcknowledged);
        }));

        let deny_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            if return_to_permissions {
                tx.send(AppEvent::OpenPermissionsPopup);
            } else {
                tx.send(AppEvent::OpenApprovalsPopup);
            }
        })];

        let items = vec![
            SelectionItem {
                name: "Yes, continue anyway".to_string(),
                description: Some("Apply full access for this session".to_string()),
                actions: accept_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Yes, and don't ask again".to_string(),
                description: Some("Enable full access and remember this choice".to_string()),
                actions: accept_and_remember_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Cancel".to_string(),
                description: Some("Go back without enabling full access".to_string()),
                actions: deny_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(header),
            ..Default::default()
        });
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn open_world_writable_warning_confirmation(
        &mut self,
        preset: Option<ApprovalPreset>,
        sample_paths: Vec<String>,
        extra_count: usize,
        failed_scan: bool,
    ) {
        let (approval, sandbox) = match &preset {
            Some(p) => (Some(p.approval), Some(p.sandbox.clone())),
            None => (None, None),
        };
        let mut header_children: Vec<Box<dyn Renderable>> = Vec::new();
        let describe_policy = |policy: &SandboxPolicy| match policy {
            SandboxPolicy::WorkspaceWrite { .. } => "Agent mode",
            SandboxPolicy::ReadOnly { .. } => "Read-Only mode",
            _ => "Agent mode",
        };
        let mode_label = preset
            .as_ref()
            .map(|p| describe_policy(&p.sandbox))
            .unwrap_or_else(|| describe_policy(self.config.permissions.sandbox_policy.get()));
        let info_line = if failed_scan {
            Line::from(vec![
                "We couldn't complete the world-writable scan, so protections cannot be verified. "
                    .into(),
                format!("The Windows sandbox cannot guarantee protection in {mode_label}.")
                    .fg(Color::Red),
            ])
        } else {
            Line::from(vec![
                "The Windows sandbox cannot protect writes to folders that are writable by Everyone.".into(),
                " Consider removing write access for Everyone from the following folders:".into(),
            ])
        };
        header_children.push(Box::new(
            Paragraph::new(vec![info_line]).wrap(Wrap { trim: false }),
        ));

        if !sample_paths.is_empty() {
            // Show up to three examples and optionally an "and X more" line.
            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from(""));
            for p in &sample_paths {
                lines.push(Line::from(format!("  - {p}")));
            }
            if extra_count > 0 {
                lines.push(Line::from(format!("and {extra_count} more")));
            }
            header_children.push(Box::new(Paragraph::new(lines).wrap(Wrap { trim: false })));
        }
        let header = ColumnRenderable::with(header_children);

        // Build actions ensuring acknowledgement happens before applying the new sandbox policy,
        // so downstream policy-change hooks don't re-trigger the warning.
        let mut accept_actions: Vec<SelectionAction> = Vec::new();
        // Suppress the immediate re-scan only when a preset will be applied (i.e., via /approvals or
        // /permissions), to avoid duplicate warnings from the ensuing policy change.
        if preset.is_some() {
            accept_actions.push(Box::new(|tx| {
                tx.send(AppEvent::SkipNextWorldWritableScan);
            }));
        }
        if let (Some(approval), Some(sandbox)) = (approval, sandbox.clone()) {
            accept_actions.extend(Self::approval_preset_actions(
                approval,
                sandbox,
                mode_label.to_string(),
                ApprovalsReviewer::User,
            ));
        }

        let mut accept_and_remember_actions: Vec<SelectionAction> = Vec::new();
        accept_and_remember_actions.push(Box::new(|tx| {
            tx.send(AppEvent::UpdateWorldWritableWarningAcknowledged(true));
            tx.send(AppEvent::PersistWorldWritableWarningAcknowledged);
        }));
        if let (Some(approval), Some(sandbox)) = (approval, sandbox) {
            accept_and_remember_actions.extend(Self::approval_preset_actions(
                approval,
                sandbox,
                mode_label.to_string(),
                ApprovalsReviewer::User,
            ));
        }

        let items = vec![
            SelectionItem {
                name: "Continue".to_string(),
                description: Some(format!("Apply {mode_label} for this session")),
                actions: accept_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Continue and don't warn again".to_string(),
                description: Some(format!("Enable {mode_label} and remember this choice")),
                actions: accept_and_remember_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(header),
            ..Default::default()
        });
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn open_world_writable_warning_confirmation(
        &mut self,
        _preset: Option<ApprovalPreset>,
        _sample_paths: Vec<String>,
        _extra_count: usize,
        _failed_scan: bool,
    ) {
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn open_windows_sandbox_enable_prompt(&mut self, preset: ApprovalPreset) {
        use ratatui_macros::line;

        if !crate::legacy_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED {
            // Legacy flow (pre-NUX): explain the experimental sandbox and let the user enable it
            // directly (no elevation prompts).
            let mut header = ColumnRenderable::new();
            header.push(*Box::new(
                Paragraph::new(vec![
                    line!["Agent mode on Windows uses an experimental sandbox to limit network and filesystem access.".bold()],
                    line!["Learn more: https://developers.openai.com/codex/windows"],
                ])
                .wrap(Wrap { trim: false }),
            ));

            let preset_clone = preset;
            let items = vec![
                SelectionItem {
                    name: "Enable experimental sandbox".to_string(),
                    description: None,
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::EnableWindowsSandboxForAgentMode {
                            preset: preset_clone.clone(),
                            mode: WindowsSandboxEnableMode::Legacy,
                        });
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Go back".to_string(),
                    description: None,
                    actions: vec![Box::new(|tx| {
                        tx.send(AppEvent::OpenApprovalsPopup);
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ];

            self.bottom_pane.show_selection_view(SelectionViewParams {
                title: None,
                footer_hint: Some(standard_popup_hint_line()),
                items,
                header: Box::new(header),
                ..Default::default()
            });
            return;
        }

        self.session_telemetry.counter(
            "codex.windows_sandbox.elevated_prompt_shown",
            /*inc*/ 1,
            &[],
        );

        let mut header = ColumnRenderable::new();
        header.push(*Box::new(
            Paragraph::new(vec![
                line!["Set up the Codex agent sandbox to protect your files and control network access. Learn more <https://developers.openai.com/codex/windows>"],
            ])
            .wrap(Wrap { trim: false }),
        ));

        let accept_otel = self.session_telemetry.clone();
        let legacy_otel = self.session_telemetry.clone();
        let legacy_preset = preset.clone();
        let quit_otel = self.session_telemetry.clone();
        let items = vec![
            SelectionItem {
                name: "Set up default sandbox (requires Administrator permissions)".to_string(),
                description: None,
                actions: vec![Box::new(move |tx| {
                    accept_otel.counter(
                        "codex.windows_sandbox.elevated_prompt_accept",
                        /*inc*/ 1,
                        &[],
                    );
                    tx.send(AppEvent::BeginWindowsSandboxElevatedSetup {
                        preset: preset.clone(),
                    });
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Use non-admin sandbox (higher risk if prompt injected)".to_string(),
                description: None,
                actions: vec![Box::new(move |tx| {
                    legacy_otel.counter(
                        "codex.windows_sandbox.elevated_prompt_use_legacy",
                        /*inc*/ 1,
                        &[],
                    );
                    tx.send(AppEvent::BeginWindowsSandboxLegacySetup {
                        preset: legacy_preset.clone(),
                    });
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Quit".to_string(),
                description: None,
                actions: vec![Box::new(move |tx| {
                    quit_otel.counter(
                        "codex.windows_sandbox.elevated_prompt_quit",
                        /*inc*/ 1,
                        &[],
                    );
                    tx.send(AppEvent::Exit(ExitMode::ShutdownFirst));
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: None,
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(header),
            ..Default::default()
        });
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn open_windows_sandbox_enable_prompt(&mut self, _preset: ApprovalPreset) {}

    #[cfg(target_os = "windows")]
    pub(crate) fn open_windows_sandbox_fallback_prompt(&mut self, preset: ApprovalPreset) {
        use ratatui_macros::line;

        let mut lines = Vec::new();
        lines.push(line![
            "Couldn't set up your sandbox with Administrator permissions".bold()
        ]);
        lines.push(line![""]);
        lines.push(line![
            "You can still use Codex in a non-admin sandbox. It carries greater risk if prompt injected."
        ]);
        lines.push(line![
            "Learn more <https://developers.openai.com/codex/windows>"
        ]);

        let mut header = ColumnRenderable::new();
        header.push(*Box::new(Paragraph::new(lines).wrap(Wrap { trim: false })));

        let elevated_preset = preset.clone();
        let legacy_preset = preset;
        let quit_otel = self.session_telemetry.clone();
        let items = vec![
            SelectionItem {
                name: "Try setting up admin sandbox again".to_string(),
                description: None,
                actions: vec![Box::new({
                    let otel = self.session_telemetry.clone();
                    let preset = elevated_preset;
                    move |tx| {
                        otel.counter(
                            "codex.windows_sandbox.fallback_retry_elevated",
                            /*inc*/ 1,
                            &[],
                        );
                        tx.send(AppEvent::BeginWindowsSandboxElevatedSetup {
                            preset: preset.clone(),
                        });
                    }
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Use Codex with non-admin sandbox".to_string(),
                description: None,
                actions: vec![Box::new({
                    let otel = self.session_telemetry.clone();
                    let preset = legacy_preset;
                    move |tx| {
                        otel.counter(
                            "codex.windows_sandbox.fallback_use_legacy",
                            /*inc*/ 1,
                            &[],
                        );
                        tx.send(AppEvent::BeginWindowsSandboxLegacySetup {
                            preset: preset.clone(),
                        });
                    }
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Quit".to_string(),
                description: None,
                actions: vec![Box::new(move |tx| {
                    quit_otel.counter(
                        "codex.windows_sandbox.fallback_prompt_quit",
                        /*inc*/ 1,
                        &[],
                    );
                    tx.send(AppEvent::Exit(ExitMode::ShutdownFirst));
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: None,
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(header),
            ..Default::default()
        });
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn open_windows_sandbox_fallback_prompt(&mut self, _preset: ApprovalPreset) {}

    #[cfg(target_os = "windows")]
    pub(crate) fn maybe_prompt_windows_sandbox_enable(&mut self, show_now: bool) {
        if show_now
            && WindowsSandboxLevel::from_config(&self.config) == WindowsSandboxLevel::Disabled
            && let Some(preset) = builtin_approval_presets()
                .into_iter()
                .find(|preset| preset.id == "auto")
        {
            self.open_windows_sandbox_enable_prompt(preset);
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn maybe_prompt_windows_sandbox_enable(&mut self, _show_now: bool) {}

    #[cfg(target_os = "windows")]
    pub(crate) fn show_windows_sandbox_setup_status(&mut self) {
        // While elevated sandbox setup runs, prevent typing so the user doesn't
        // accidentally queue messages that will run under an unexpected mode.
        self.bottom_pane.set_composer_input_enabled(
            /*enabled*/ false,
            Some("Input disabled until setup completes.".to_string()),
        );
        self.bottom_pane.ensure_status_indicator();
        self.bottom_pane
            .set_interrupt_hint_visible(/*visible*/ false);
        self.set_status(
            "Setting up sandbox...".to_string(),
            Some("Hang tight, this may take a few minutes".to_string()),
            StatusDetailsCapitalization::CapitalizeFirst,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
        self.request_redraw();
    }

    #[cfg(not(target_os = "windows"))]
    #[allow(dead_code)]
    pub(crate) fn show_windows_sandbox_setup_status(&mut self) {}

    #[cfg(target_os = "windows")]
    pub(crate) fn clear_windows_sandbox_setup_status(&mut self) {
        self.bottom_pane
            .set_composer_input_enabled(/*enabled*/ true, /*placeholder*/ None);
        self.bottom_pane.hide_status_indicator();
        self.request_redraw();
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn clear_windows_sandbox_setup_status(&mut self) {}

    /// Set the approval policy in the widget's config copy.
    pub(crate) fn set_approval_policy(&mut self, policy: AskForApproval) {
        if let Err(err) = self.config.permissions.approval_policy.set(policy) {
            tracing::warn!(%err, "failed to set approval_policy on chat config");
        }
    }

    /// Set the sandbox policy in the widget's config copy.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn set_sandbox_policy(&mut self, policy: SandboxPolicy) -> ConstraintResult<()> {
        self.config.permissions.sandbox_policy.set(policy)?;
        Ok(())
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn set_windows_sandbox_mode(&mut self, mode: Option<WindowsSandboxModeToml>) {
        self.config.permissions.windows_sandbox_mode = mode;
        #[cfg(target_os = "windows")]
        self.bottom_pane.set_windows_degraded_sandbox_active(
            crate::legacy_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
                && matches!(
                    WindowsSandboxLevel::from_config(&self.config),
                    WindowsSandboxLevel::RestrictedToken
                ),
        );
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn set_feature_enabled(&mut self, feature: Feature, enabled: bool) -> bool {
        if let Err(err) = self.config.features.set_enabled(feature, enabled) {
            tracing::warn!(
                error = %err,
                feature = feature.key(),
                "failed to update constrained chat widget feature state"
            );
        }
        let enabled = self.config.features.enabled(feature);
        if feature == Feature::RealtimeConversation {
            let realtime_conversation_enabled = self.realtime_conversation_enabled();
            self.bottom_pane
                .set_realtime_conversation_enabled(realtime_conversation_enabled);
            self.bottom_pane
                .set_audio_device_selection_enabled(self.realtime_audio_device_selection_enabled());
            if !realtime_conversation_enabled && self.realtime_conversation.is_live() {
                self.request_realtime_conversation_close(Some(
                    "Realtime voice mode was closed because the feature was disabled.".to_string(),
                ));
            }
        }
        if feature == Feature::FastMode {
            self.sync_fast_command_enabled();
        }
        if feature == Feature::Personality {
            self.sync_personality_command_enabled();
        }
        if feature == Feature::Plugins {
            self.sync_plugins_command_enabled();
            self.refresh_plugin_mentions();
        }
        if feature == Feature::PreventIdleSleep {
            self.turn_sleep_inhibitor = SleepInhibitor::new(enabled);
            self.turn_sleep_inhibitor
                .set_turn_running(self.agent_turn_running);
        }
        #[cfg(target_os = "windows")]
        if matches!(
            feature,
            Feature::WindowsSandbox | Feature::WindowsSandboxElevated
        ) {
            self.bottom_pane.set_windows_degraded_sandbox_active(
                crate::legacy_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
                    && matches!(
                        WindowsSandboxLevel::from_config(&self.config),
                        WindowsSandboxLevel::RestrictedToken
                    ),
            );
        }
        enabled
    }

    pub(crate) fn set_approvals_reviewer(&mut self, policy: ApprovalsReviewer) {
        self.config.approvals_reviewer = policy;
    }

    pub(crate) fn set_full_access_warning_acknowledged(&mut self, acknowledged: bool) {
        self.config.notices.hide_full_access_warning = Some(acknowledged);
    }

    pub(crate) fn set_world_writable_warning_acknowledged(&mut self, acknowledged: bool) {
        self.config.notices.hide_world_writable_warning = Some(acknowledged);
    }

    pub(crate) fn set_rate_limit_switch_prompt_hidden(&mut self, hidden: bool) {
        self.config.notices.hide_rate_limit_model_nudge = Some(hidden);
        if hidden {
            self.rate_limit_switch_prompt = RateLimitSwitchPromptState::Idle;
        }
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn world_writable_warning_hidden(&self) -> bool {
        self.config
            .notices
            .hide_world_writable_warning
            .unwrap_or(false)
    }

    /// Override the reasoning effort used when Plan mode is active.
    ///
    /// When the active mask is already Plan, the override is applied immediately
    /// so the footer reflects it without waiting for the next mode switch.
    /// Passing `None` resets to the Plan-mode preset default.
    pub(crate) fn set_plan_mode_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        self.config.plan_mode_reasoning_effort = effort;
        if self.collaboration_modes_enabled()
            && let Some(mask) = self.active_collaboration_mask.as_mut()
            && mask.mode == Some(ModeKind::Plan)
        {
            if let Some(effort) = effort {
                mask.reasoning_effort = Some(Some(effort));
            } else if let Some(plan_mask) =
                collaboration_modes::plan_mask(self.model_catalog.as_ref())
            {
                mask.reasoning_effort = plan_mask.reasoning_effort;
            }
        }
        self.refresh_model_dependent_surfaces();
    }

    /// Set the reasoning effort for the non-Plan collaboration mode.
    ///
    /// Does not touch the active Plan mask — Plan reasoning is controlled
    /// exclusively by the Plan preset and `set_plan_mode_reasoning_effort`.
    pub(crate) fn set_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        self.current_collaboration_mode = self.current_collaboration_mode.with_updates(
            /*model*/ None,
            Some(effort),
            /*developer_instructions*/ None,
        );
        if self.collaboration_modes_enabled()
            && let Some(mask) = self.active_collaboration_mask.as_mut()
            && mask.mode != Some(ModeKind::Plan)
        {
            // Generic "global default" updates should not mutate the active Plan mask.
            // Plan reasoning is controlled by the Plan preset and Plan-only override updates.
            mask.reasoning_effort = Some(effort);
        }
        self.refresh_model_dependent_surfaces();
    }

    /// Set the personality in the widget's config copy.
    pub(crate) fn set_personality(&mut self, personality: Personality) {
        self.config.personality = Some(personality);
    }

    /// Set Fast mode in the widget's config copy.
    pub(crate) fn set_service_tier(&mut self, service_tier: Option<ServiceTier>) {
        self.config.service_tier = service_tier;
    }

    pub(crate) fn current_service_tier(&self) -> Option<ServiceTier> {
        self.config.service_tier
    }

    pub(crate) fn status_account_display(&self) -> Option<&StatusAccountDisplay> {
        self.status_account_display.as_ref()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn model_catalog(&self) -> Arc<ModelCatalog> {
        self.model_catalog.clone()
    }

    pub(crate) fn current_plan_type(&self) -> Option<PlanType> {
        self.plan_type
    }

    pub(crate) fn has_chatgpt_account(&self) -> bool {
        self.has_chatgpt_account
    }

    pub(crate) fn update_account_state(
        &mut self,
        status_account_display: Option<StatusAccountDisplay>,
        plan_type: Option<PlanType>,
        has_chatgpt_account: bool,
    ) {
        self.status_account_display = status_account_display;
        self.plan_type = plan_type;
        self.has_chatgpt_account = has_chatgpt_account;
        self.bottom_pane
            .set_connectors_enabled(self.connectors_enabled());
    }

    pub(crate) fn should_show_fast_status(
        &self,
        model: &str,
        service_tier: Option<ServiceTier>,
    ) -> bool {
        self.model_supports_fast_mode(model)
            && matches!(service_tier, Some(ServiceTier::Fast))
            && self.has_chatgpt_account
    }

    fn fast_mode_enabled(&self) -> bool {
        self.config.features.enabled(Feature::FastMode)
    }

    pub(crate) fn set_realtime_audio_device(
        &mut self,
        kind: RealtimeAudioDeviceKind,
        name: Option<String>,
    ) {
        match kind {
            RealtimeAudioDeviceKind::Microphone => self.config.realtime_audio.microphone = name,
            RealtimeAudioDeviceKind::Speaker => self.config.realtime_audio.speaker = name,
        }
    }

    /// Set the syntax theme override in the widget's config copy.
    pub(crate) fn set_tui_theme(&mut self, theme: Option<String>) {
        self.config.tui_theme = theme;
    }

    /// Set the model in the widget's config copy and stored collaboration mode.
    pub(crate) fn set_model(&mut self, model: &str) {
        self.current_collaboration_mode = self.current_collaboration_mode.with_updates(
            Some(model.to_string()),
            /*effort*/ None,
            /*developer_instructions*/ None,
        );
        if self.collaboration_modes_enabled()
            && let Some(mask) = self.active_collaboration_mask.as_mut()
        {
            mask.model = Some(model.to_string());
        }
        self.refresh_model_dependent_surfaces();
    }

    fn set_service_tier_selection(&mut self, service_tier: Option<ServiceTier>) {
        self.set_service_tier(service_tier);
        self.app_event_tx.send(AppEvent::CodexOp(
            AppCommand::override_turn_context(
                /*cwd*/ None,
                /*approval_policy*/ None,
                /*approvals_reviewer*/ None,
                /*sandbox_policy*/ None,
                /*windows_sandbox_level*/ None,
                /*model*/ None,
                /*effort*/ None,
                /*summary*/ None,
                Some(service_tier),
                /*collaboration_mode*/ None,
                /*personality*/ None,
            )
            .into_core(),
        ));
        self.app_event_tx
            .send(AppEvent::PersistServiceTierSelection { service_tier });
    }

    pub(crate) fn current_model(&self) -> &str {
        if !self.collaboration_modes_enabled() {
            return self.current_collaboration_mode.model();
        }
        self.active_collaboration_mask
            .as_ref()
            .and_then(|mask| mask.model.as_deref())
            .unwrap_or_else(|| self.current_collaboration_mode.model())
    }

    pub(crate) fn realtime_conversation_is_live(&self) -> bool {
        self.realtime_conversation.is_live()
    }

    fn current_realtime_audio_device_name(&self, kind: RealtimeAudioDeviceKind) -> Option<String> {
        match kind {
            RealtimeAudioDeviceKind::Microphone => self.config.realtime_audio.microphone.clone(),
            RealtimeAudioDeviceKind::Speaker => self.config.realtime_audio.speaker.clone(),
        }
    }

    fn current_realtime_audio_selection_label(&self, kind: RealtimeAudioDeviceKind) -> String {
        self.current_realtime_audio_device_name(kind)
            .unwrap_or_else(|| "System default".to_string())
    }

    fn sync_fast_command_enabled(&mut self) {
        self.bottom_pane
            .set_fast_command_enabled(self.fast_mode_enabled());
    }

    fn sync_personality_command_enabled(&mut self) {
        self.bottom_pane
            .set_personality_command_enabled(self.config.features.enabled(Feature::Personality));
    }

    fn sync_plugins_command_enabled(&mut self) {
        self.bottom_pane
            .set_plugins_command_enabled(self.config.features.enabled(Feature::Plugins));
    }

    fn current_model_supports_personality(&self) -> bool {
        let model = self.current_model();
        self.model_catalog
            .try_list_models()
            .ok()
            .and_then(|models| {
                models
                    .into_iter()
                    .find(|preset| preset.model == model)
                    .map(|preset| preset.supports_personality)
            })
            .unwrap_or(false)
    }

    fn model_supports_fast_mode(&self, model: &str) -> bool {
        self.model_catalog
            .try_list_models()
            .ok()
            .and_then(|models| {
                models
                    .into_iter()
                    .find(|preset| preset.model == model)
                    .map(|preset| preset.supports_fast_mode())
            })
            .unwrap_or(false)
    }

    /// Return whether the effective model currently advertises image-input support.
    ///
    /// We intentionally default to `true` when model metadata cannot be read so transient catalog
    /// failures do not hard-block user input in the UI.
    fn current_model_supports_images(&self) -> bool {
        let model = self.current_model();
        self.model_catalog
            .try_list_models()
            .ok()
            .and_then(|models| {
                models
                    .into_iter()
                    .find(|preset| preset.model == model)
                    .map(|preset| preset.input_modalities.contains(&InputModality::Image))
            })
            .unwrap_or(true)
    }

    fn sync_image_paste_enabled(&mut self) {
        let enabled = self.current_model_supports_images();
        self.bottom_pane.set_image_paste_enabled(enabled);
    }

    fn image_inputs_not_supported_message(&self) -> String {
        format!(
            "Model {} does not support image inputs. Remove images or switch models.",
            self.current_model()
        )
    }

    #[allow(dead_code)] // Used in tests
    pub(crate) fn current_collaboration_mode(&self) -> &CollaborationMode {
        &self.current_collaboration_mode
    }

    pub(crate) fn current_reasoning_effort(&self) -> Option<ReasoningEffortConfig> {
        self.effective_reasoning_effort()
    }

    #[cfg(test)]
    pub(crate) fn active_collaboration_mode_kind(&self) -> ModeKind {
        self.active_mode_kind()
    }

    fn is_session_configured(&self) -> bool {
        self.thread_id.is_some()
    }

    fn collaboration_modes_enabled(&self) -> bool {
        true
    }

    fn initial_collaboration_mask(
        _config: &Config,
        model_catalog: &ModelCatalog,
        model_override: Option<&str>,
    ) -> Option<CollaborationModeMask> {
        let mut mask = collaboration_modes::default_mask(model_catalog)?;
        if let Some(model_override) = model_override {
            mask.model = Some(model_override.to_string());
        }
        Some(mask)
    }

    fn active_mode_kind(&self) -> ModeKind {
        self.active_collaboration_mask
            .as_ref()
            .and_then(|mask| mask.mode)
            .unwrap_or(ModeKind::Default)
    }

    fn effective_reasoning_effort(&self) -> Option<ReasoningEffortConfig> {
        if !self.collaboration_modes_enabled() {
            return self.current_collaboration_mode.reasoning_effort();
        }
        let current_effort = self.current_collaboration_mode.reasoning_effort();
        self.active_collaboration_mask
            .as_ref()
            .and_then(|mask| mask.reasoning_effort)
            .unwrap_or(current_effort)
    }

    fn effective_collaboration_mode(&self) -> CollaborationMode {
        if !self.collaboration_modes_enabled() {
            return self.current_collaboration_mode.clone();
        }
        self.active_collaboration_mask.as_ref().map_or_else(
            || self.current_collaboration_mode.clone(),
            |mask| self.current_collaboration_mode.apply_mask(mask),
        )
    }

    fn refresh_model_display(&mut self) {
        let effective = self.effective_collaboration_mode();
        self.session_header.set_model(effective.model());
        // Keep composer paste affordances aligned with the currently effective model.
        self.sync_image_paste_enabled();
        self.refresh_terminal_title();
    }

    /// Refresh every UI surface that depends on the effective model, reasoning
    /// effort, or collaboration mode.
    ///
    /// Call this at the end of any setter that mutates `current_collaboration_mode`,
    /// `active_collaboration_mask`, or per-mode reasoning-effort overrides.
    /// Consolidating both refreshes here prevents the bug where callers update the
    /// header/title (`refresh_model_display`) but forget the footer status line
    /// (`refresh_status_line`).
    fn refresh_model_dependent_surfaces(&mut self) {
        self.refresh_model_display();
        self.refresh_status_line();
    }

    fn model_display_name(&self) -> &str {
        let model = self.current_model();
        if model.is_empty() {
            DEFAULT_MODEL_DISPLAY_NAME
        } else {
            model
        }
    }

    /// Get the label for the current collaboration mode.
    fn collaboration_mode_label(&self) -> Option<&'static str> {
        if !self.collaboration_modes_enabled() {
            return None;
        }
        let active_mode = self.active_mode_kind();
        active_mode
            .is_tui_visible()
            .then_some(active_mode.display_name())
    }

    fn collaboration_mode_indicator(&self) -> Option<CollaborationModeIndicator> {
        if !self.collaboration_modes_enabled() {
            return None;
        }
        match self.active_mode_kind() {
            ModeKind::Plan => Some(CollaborationModeIndicator::Plan),
            ModeKind::Default | ModeKind::PairProgramming | ModeKind::Execute => None,
        }
    }

    fn update_collaboration_mode_indicator(&mut self) {
        let indicator = self.collaboration_mode_indicator();
        self.bottom_pane.set_collaboration_mode_indicator(indicator);
    }

    fn personality_label(personality: Personality) -> &'static str {
        match personality {
            Personality::None => "None",
            Personality::Friendly => "Friendly",
            Personality::Pragmatic => "Pragmatic",
        }
    }

    fn personality_description(personality: Personality) -> &'static str {
        match personality {
            Personality::None => "No personality instructions.",
            Personality::Friendly => "Warm, collaborative, and helpful.",
            Personality::Pragmatic => "Concise, task-focused, and direct.",
        }
    }

    /// Cycle to the next collaboration mode variant (Plan -> Default -> Plan).
    fn cycle_collaboration_mode(&mut self) {
        if !self.collaboration_modes_enabled() {
            return;
        }

        if let Some(next_mask) = collaboration_modes::next_mask(
            self.model_catalog.as_ref(),
            self.active_collaboration_mask.as_ref(),
        ) {
            self.set_collaboration_mask(next_mask);
        }
    }

    /// Update the active collaboration mask.
    ///
    /// When collaboration modes are enabled and a preset is selected,
    /// the current mode is attached to submissions as `Op::UserTurn { collaboration_mode: Some(...) }`.
    pub(crate) fn set_collaboration_mask(&mut self, mut mask: CollaborationModeMask) {
        if !self.collaboration_modes_enabled() {
            return;
        }
        let previous_mode = self.active_mode_kind();
        let previous_model = self.current_model().to_string();
        let previous_effort = self.effective_reasoning_effort();
        if mask.mode == Some(ModeKind::Plan)
            && let Some(effort) = self.config.plan_mode_reasoning_effort
        {
            mask.reasoning_effort = Some(Some(effort));
        }
        self.active_collaboration_mask = Some(mask);
        self.update_collaboration_mode_indicator();
        self.refresh_model_dependent_surfaces();
        let next_mode = self.active_mode_kind();
        let next_model = self.current_model();
        let next_effort = self.effective_reasoning_effort();
        if previous_mode != next_mode
            && (previous_model != next_model || previous_effort != next_effort)
        {
            let mut message = format!("Model changed to {next_model}");
            if !next_model.starts_with("codex-auto-") {
                let reasoning_label = match next_effort {
                    Some(ReasoningEffortConfig::Minimal) => "minimal",
                    Some(ReasoningEffortConfig::Low) => "low",
                    Some(ReasoningEffortConfig::Medium) => "medium",
                    Some(ReasoningEffortConfig::High) => "high",
                    Some(ReasoningEffortConfig::XHigh) => "xhigh",
                    None | Some(ReasoningEffortConfig::None) => "default",
                };
                message.push(' ');
                message.push_str(reasoning_label);
            }
            message.push_str(" for ");
            message.push_str(next_mode.display_name());
            message.push_str(" mode.");
            self.add_info_message(message, /*hint*/ None);
        }
        self.request_redraw();
    }

    fn connectors_enabled(&self) -> bool {
        self.config.features.enabled(Feature::Apps) && self.has_chatgpt_account
    }

    fn connectors_for_mentions(&self) -> Option<&[connectors::AppInfo]> {
        if !self.connectors_enabled() {
            return None;
        }

        if let Some(snapshot) = &self.connectors_partial_snapshot {
            return Some(snapshot.connectors.as_slice());
        }

        match &self.connectors_cache {
            ConnectorsCacheState::Ready(snapshot) => Some(snapshot.connectors.as_slice()),
            _ => None,
        }
    }

    fn plugins_for_mentions(
        &self,
    ) -> Option<&[crate::legacy_core::plugins::PluginCapabilitySummary]> {
        if !self.config.features.enabled(Feature::Plugins) {
            return None;
        }

        self.bottom_pane.plugins().map(Vec::as_slice)
    }

    /// Build a placeholder header cell while the session is configuring.
    fn placeholder_session_header_cell(config: &Config) -> Box<dyn HistoryCell> {
        let placeholder_style = Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC);
        Box::new(history_cell::SessionHeaderHistoryCell::new_with_style(
            DEFAULT_MODEL_DISPLAY_NAME.to_string(),
            placeholder_style,
            /*reasoning_effort*/ None,
            /*show_fast_status*/ false,
            config.cwd.to_path_buf(),
            CODEX_CLI_VERSION,
        ))
    }

    /// Merge the real session info cell with any placeholder header to avoid double boxes.
    fn apply_session_info_cell(&mut self, cell: history_cell::SessionInfoCell) {
        let mut session_info_cell = Some(Box::new(cell) as Box<dyn HistoryCell>);
        let merged_header = if let Some(active) = self.active_cell.take() {
            if active
                .as_any()
                .is::<history_cell::SessionHeaderHistoryCell>()
            {
                // Reuse the existing placeholder header to avoid rendering two boxes.
                if let Some(cell) = session_info_cell.take() {
                    self.active_cell = Some(cell);
                }
                true
            } else {
                self.active_cell = Some(active);
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

    pub(crate) fn add_plain_history_lines(&mut self, lines: Vec<Line<'static>>) {
        self.add_boxed_history(Box::new(PlainHistoryCell::new(lines)));
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
    pub(crate) fn add_mcp_output(&mut self) {
        self.flush_answer_stream_with_separator();
        self.flush_active_cell();
        self.active_cell = Some(Box::new(history_cell::new_mcp_inventory_loading(
            self.config.animations,
        )));
        self.bump_active_cell_revision();
        self.request_redraw();
        self.app_event_tx.send(AppEvent::FetchMcpInventory);
    }

    /// Remove the MCP loading spinner if it is still the active cell.
    ///
    /// Uses `Any`-based type checking so that a late-arriving inventory result
    /// does not accidentally clear an unrelated cell that was set in the meantime.
    pub(crate) fn clear_mcp_inventory_loading(&mut self) {
        let Some(active) = self.active_cell.as_ref() else {
            return;
        };
        if !active
            .as_any()
            .is::<history_cell::McpInventoryLoadingCell>()
        {
            return;
        }
        self.active_cell = None;
        self.bump_active_cell_revision();
        self.request_redraw();
    }

    pub(crate) fn add_connectors_output(&mut self) {
        if !self.connectors_enabled() {
            self.add_info_message(
                "Apps are disabled.".to_string(),
                Some("Enable the apps feature to use $ or /apps.".to_string()),
            );
            return;
        }

        let connectors_cache = self.connectors_cache.clone();
        let should_force_refetch = !self.connectors_prefetch_in_flight
            || matches!(connectors_cache, ConnectorsCacheState::Ready(_));
        self.prefetch_connectors_with_options(should_force_refetch);

        match connectors_cache {
            ConnectorsCacheState::Ready(snapshot) => {
                if snapshot.connectors.is_empty() {
                    self.add_info_message("No apps available.".to_string(), /*hint*/ None);
                } else {
                    self.open_connectors_popup(&snapshot.connectors);
                }
            }
            ConnectorsCacheState::Failed(err) => {
                self.add_to_history(history_cell::new_error_event(err));
            }
            ConnectorsCacheState::Loading | ConnectorsCacheState::Uninitialized => {
                self.open_connectors_loading_popup();
            }
        }
        self.request_redraw();
    }

    fn open_connectors_loading_popup(&mut self) {
        if !self.bottom_pane.replace_selection_view_if_active(
            CONNECTORS_SELECTION_VIEW_ID,
            self.connectors_loading_popup_params(),
        ) {
            self.bottom_pane
                .show_selection_view(self.connectors_loading_popup_params());
        }
    }

    fn open_connectors_popup(&mut self, connectors: &[connectors::AppInfo]) {
        self.bottom_pane.show_selection_view(
            self.connectors_popup_params(connectors, /*selected_connector_id*/ None),
        );
    }

    fn connectors_loading_popup_params(&self) -> SelectionViewParams {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Apps".bold()));
        header.push(Line::from("Loading installed and available apps...".dim()));

        SelectionViewParams {
            view_id: Some(CONNECTORS_SELECTION_VIEW_ID),
            header: Box::new(header),
            items: vec![SelectionItem {
                name: "Loading apps...".to_string(),
                description: Some("This updates when the full list is ready.".to_string()),
                is_disabled: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn connectors_popup_params(
        &self,
        connectors: &[connectors::AppInfo],
        selected_connector_id: Option<&str>,
    ) -> SelectionViewParams {
        let total = connectors.len();
        let installed = connectors
            .iter()
            .filter(|connector| connector.is_accessible)
            .count();
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Apps".bold()));
        header.push(Line::from(
            "Use $ to insert an installed app into your prompt.".dim(),
        ));
        header.push(Line::from(
            format!("Installed {installed} of {total} available apps.").dim(),
        ));
        let initial_selected_idx = selected_connector_id.and_then(|selected_connector_id| {
            connectors
                .iter()
                .position(|connector| connector.id == selected_connector_id)
        });
        let mut items: Vec<SelectionItem> = Vec::with_capacity(connectors.len());
        for connector in connectors {
            let connector_label = connectors::connector_display_label(connector);
            let connector_title = connector_label.clone();
            let link_description = Self::connector_description(connector);
            let description = Self::connector_brief_description(connector);
            let status_label = Self::connector_status_label(connector);
            let search_value = format!("{connector_label} {}", connector.id);
            let mut item = SelectionItem {
                name: connector_label,
                description: Some(description),
                search_value: Some(search_value),
                ..Default::default()
            };
            let is_installed = connector.is_accessible;
            let selected_label = if is_installed {
                format!(
                    "{status_label}. Press Enter to open the app page to install, manage, or enable/disable this app."
                )
            } else {
                format!("{status_label}. Press Enter to open the app page to install this app.")
            };
            let missing_label = format!("{status_label}. App link unavailable.");
            let instructions = if connector.is_accessible {
                "Manage this app in your browser."
            } else {
                "Install this app in your browser, then reload Codex."
            };
            if let Some(install_url) = connector.install_url.clone() {
                let app_id = connector.id.clone();
                let is_enabled = connector.is_enabled;
                let title = connector_title.clone();
                let instructions = instructions.to_string();
                let description = link_description.clone();
                item.actions = vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenAppLink {
                        app_id: app_id.clone(),
                        title: title.clone(),
                        description: description.clone(),
                        instructions: instructions.clone(),
                        url: install_url.clone(),
                        is_installed,
                        is_enabled,
                    });
                })];
                item.dismiss_on_select = true;
                item.selected_description = Some(selected_label);
            } else {
                let missing_label_for_action = missing_label.clone();
                item.actions = vec![Box::new(move |tx| {
                    tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_info_event(
                            missing_label_for_action.clone(),
                            /*hint*/ None,
                        ),
                    )));
                })];
                item.dismiss_on_select = true;
                item.selected_description = Some(missing_label);
            }
            items.push(item);
        }

        SelectionViewParams {
            view_id: Some(CONNECTORS_SELECTION_VIEW_ID),
            header: Box::new(header),
            footer_hint: Some(Self::connectors_popup_hint_line()),
            items,
            is_searchable: true,
            search_placeholder: Some("Type to search apps".to_string()),
            col_width_mode: ColumnWidthMode::AutoAllRows,
            initial_selected_idx,
            ..Default::default()
        }
    }

    fn refresh_connectors_popup_if_open(&mut self, connectors: &[connectors::AppInfo]) {
        let selected_connector_id =
            if let (Some(selected_index), ConnectorsCacheState::Ready(snapshot)) = (
                self.bottom_pane
                    .selected_index_for_active_view(CONNECTORS_SELECTION_VIEW_ID),
                &self.connectors_cache,
            ) {
                snapshot
                    .connectors
                    .get(selected_index)
                    .map(|connector| connector.id.as_str())
            } else {
                None
            };
        let _ = self.bottom_pane.replace_selection_view_if_active(
            CONNECTORS_SELECTION_VIEW_ID,
            self.connectors_popup_params(connectors, selected_connector_id),
        );
    }

    fn connectors_popup_hint_line() -> Line<'static> {
        Line::from(vec![
            "Press ".into(),
            key_hint::plain(KeyCode::Esc).into(),
            " to close.".into(),
        ])
    }

    fn connector_brief_description(connector: &connectors::AppInfo) -> String {
        let status_label = Self::connector_status_label(connector);
        match Self::connector_description(connector) {
            Some(description) => format!("{status_label} · {description}"),
            None => status_label.to_string(),
        }
    }

    fn connector_status_label(connector: &connectors::AppInfo) -> &'static str {
        if connector.is_accessible {
            if connector.is_enabled {
                "Installed"
            } else {
                "Installed · Disabled"
            }
        } else {
            "Can be installed"
        }
    }

    fn connector_description(connector: &connectors::AppInfo) -> Option<String> {
        connector
            .description
            .as_deref()
            .map(str::trim)
            .filter(|description| !description.is_empty())
            .map(str::to_string)
    }

    /// Forward file-search results to the bottom pane.
    pub(crate) fn apply_file_search_result(&mut self, query: String, matches: Vec<FileMatch>) {
        self.bottom_pane.on_file_search_result(query, matches);
    }

    /// Handles a Ctrl+C press at the chat-widget layer.
    ///
    /// The first press arms a time-bounded quit shortcut and shows a footer hint via the bottom
    /// pane. If cancellable work is active, Ctrl+C also submits `Op::Interrupt` after the shortcut
    /// is armed.
    ///
    /// Active realtime conversations take precedence over bottom-pane Ctrl+C handling so the
    /// first press always stops live voice, even when the composer contains the recording meter.
    ///
    /// If the same quit shortcut is pressed again before expiry, this requests a shutdown-first
    /// quit.
    fn on_ctrl_c(&mut self) {
        let key = key_hint::ctrl(KeyCode::Char('c'));
        if self.realtime_conversation.is_live() {
            self.bottom_pane.clear_quit_shortcut_hint();
            self.quit_shortcut_expires_at = None;
            self.quit_shortcut_key = None;
            self.stop_realtime_conversation_from_ui();
            return;
        }
        let modal_or_popup_active = !self.bottom_pane.no_modal_or_popup_active();
        if self.bottom_pane.on_ctrl_c() == CancellationEvent::Handled {
            if DOUBLE_PRESS_QUIT_SHORTCUT_ENABLED {
                if modal_or_popup_active {
                    self.quit_shortcut_expires_at = None;
                    self.quit_shortcut_key = None;
                    self.bottom_pane.clear_quit_shortcut_hint();
                } else {
                    self.arm_quit_shortcut(key);
                }
            }
            return;
        }

        if !DOUBLE_PRESS_QUIT_SHORTCUT_ENABLED {
            if self.is_cancellable_work_active() {
                self.submit_op(AppCommand::interrupt());
            } else {
                self.request_quit_without_confirmation();
            }
            return;
        }

        if self.quit_shortcut_active_for(key) {
            self.quit_shortcut_expires_at = None;
            self.quit_shortcut_key = None;
            self.request_quit_without_confirmation();
            return;
        }

        self.arm_quit_shortcut(key);

        if self.is_cancellable_work_active() {
            self.submit_op(AppCommand::interrupt());
        }
    }

    /// Handles a Ctrl+D press at the chat-widget layer.
    ///
    /// Ctrl-D only participates in quit when the composer is empty and no modal/popup is active.
    /// Otherwise it should be routed to the active view and not attempt to quit.
    fn on_ctrl_d(&mut self) -> bool {
        let key = key_hint::ctrl(KeyCode::Char('d'));
        if !DOUBLE_PRESS_QUIT_SHORTCUT_ENABLED {
            if !self.bottom_pane.composer_is_empty() || !self.bottom_pane.no_modal_or_popup_active()
            {
                return false;
            }

            self.request_quit_without_confirmation();
            return true;
        }

        if self.quit_shortcut_active_for(key) {
            self.quit_shortcut_expires_at = None;
            self.quit_shortcut_key = None;
            self.request_quit_without_confirmation();
            return true;
        }

        if !self.bottom_pane.composer_is_empty() || !self.bottom_pane.no_modal_or_popup_active() {
            return false;
        }

        self.arm_quit_shortcut(key);
        true
    }

    /// True if `key` matches the armed quit shortcut and the window has not expired.
    fn quit_shortcut_active_for(&self, key: KeyBinding) -> bool {
        self.quit_shortcut_key == Some(key)
            && self
                .quit_shortcut_expires_at
                .is_some_and(|expires_at| Instant::now() < expires_at)
    }

    /// Arm the double-press quit shortcut and show the footer hint.
    ///
    /// This keeps the state machine (`quit_shortcut_*`) in `ChatWidget`, since
    /// it is the component that interprets Ctrl+C vs Ctrl+D and decides whether
    /// quitting is currently allowed, while delegating rendering to `BottomPane`.
    fn arm_quit_shortcut(&mut self, key: KeyBinding) {
        self.quit_shortcut_expires_at = Instant::now()
            .checked_add(QUIT_SHORTCUT_TIMEOUT)
            .or_else(|| Some(Instant::now()));
        self.quit_shortcut_key = Some(key);
        self.bottom_pane.show_quit_shortcut_hint(key);
    }

    // Review mode counts as cancellable work so Ctrl+C interrupts instead of quitting.
    fn is_cancellable_work_active(&self) -> bool {
        self.bottom_pane.is_task_running() || self.is_review_mode
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

    pub(crate) fn submit_user_message_with_mode(
        &mut self,
        text: String,
        mut collaboration_mode: CollaborationModeMask,
    ) {
        if collaboration_mode.mode == Some(ModeKind::Plan)
            && let Some(effort) = self.config.plan_mode_reasoning_effort
        {
            collaboration_mode.reasoning_effort = Some(Some(effort));
        }
        if self.agent_turn_running
            && self.active_collaboration_mask.as_ref() != Some(&collaboration_mode)
        {
            self.add_error_message(
                "Cannot switch collaboration mode while a turn is running.".to_string(),
            );
            return;
        }
        self.set_collaboration_mask(collaboration_mode);
        let should_queue = self.is_plan_streaming_in_tui();
        let user_message = UserMessage {
            text,
            local_images: Vec::new(),
            remote_image_urls: Vec::new(),
            text_elements: Vec::new(),
            mention_bindings: Vec::new(),
        };
        if should_queue {
            self.queue_user_message(user_message);
        } else {
            self.submit_user_message(user_message);
        }
    }

    /// True when the UI is in the regular composer state with no running task,
    /// no modal overlay (e.g. approvals or status indicator), and no composer popups.
    /// In this state Esc-Esc backtracking is enabled.
    pub(crate) fn is_normal_backtrack_mode(&self) -> bool {
        self.bottom_pane.is_normal_backtrack_mode()
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
    pub(crate) fn queued_user_message_texts(&self) -> Vec<String> {
        self.rejected_steers_queue
            .iter()
            .map(|message| message.text.clone())
            .chain(
                self.queued_user_messages
                    .iter()
                    .map(|message| message.text.clone()),
            )
            .collect()
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
    /// Forward a command directly to codex.
    pub(crate) fn submit_op<T>(&mut self, op: T) -> bool
    where
        T: Into<AppCommand>,
    {
        let op: AppCommand = op.into();
        if op.is_review() && !self.bottom_pane.is_task_running() {
            self.bottom_pane.set_task_running(/*running*/ true);
        }
        match &self.codex_op_target {
            CodexOpTarget::Direct(codex_op_tx) => {
                crate::session_log::log_outbound_op(&op);
                if let Err(e) = codex_op_tx.send(op.into_core()) {
                    tracing::error!("failed to submit op: {e}");
                    return false;
                }
            }
            CodexOpTarget::AppEvent => {
                self.app_event_tx.send(AppEvent::CodexOp(op.into()));
            }
        }
        true
    }

    #[cfg(test)]
    fn on_list_mcp_tools(&mut self, ev: McpListToolsResponseEvent) {
        self.add_to_history(history_cell::new_mcp_tools_output(
            &self.config,
            ev.tools,
            ev.resources,
            ev.resource_templates,
            &ev.auth_statuses,
        ));
    }

    fn on_list_skills(&mut self, ev: ListSkillsResponseEvent) {
        self.set_skills_from_response(&ev);
        self.refresh_plugin_mentions();
    }

    pub(crate) fn on_connectors_loaded(
        &mut self,
        result: Result<ConnectorsSnapshot, String>,
        is_final: bool,
    ) {
        let mut trigger_pending_force_refetch = false;
        if is_final {
            self.connectors_prefetch_in_flight = false;
            if self.connectors_force_refetch_pending {
                self.connectors_force_refetch_pending = false;
                trigger_pending_force_refetch = true;
            }
        }

        match result {
            Ok(mut snapshot) => {
                if !is_final {
                    snapshot.connectors = connectors::merge_connectors_with_accessible(
                        Vec::new(),
                        snapshot.connectors,
                        /*all_connectors_loaded*/ false,
                    );
                }
                snapshot.connectors =
                    connectors::with_app_enabled_state(snapshot.connectors, &self.config);
                if let ConnectorsCacheState::Ready(existing_snapshot) = &self.connectors_cache {
                    let enabled_by_id: HashMap<&str, bool> = existing_snapshot
                        .connectors
                        .iter()
                        .map(|connector| (connector.id.as_str(), connector.is_enabled))
                        .collect();
                    for connector in &mut snapshot.connectors {
                        if let Some(is_enabled) = enabled_by_id.get(connector.id.as_str()) {
                            connector.is_enabled = *is_enabled;
                        }
                    }
                }
                if is_final {
                    self.connectors_partial_snapshot = None;
                    self.refresh_connectors_popup_if_open(&snapshot.connectors);
                    self.connectors_cache = ConnectorsCacheState::Ready(snapshot.clone());
                } else {
                    self.connectors_partial_snapshot = Some(snapshot.clone());
                }
                self.bottom_pane.set_connectors_snapshot(Some(snapshot));
            }
            Err(err) => {
                let partial_snapshot = self.connectors_partial_snapshot.take();
                if let ConnectorsCacheState::Ready(snapshot) = &self.connectors_cache {
                    warn!("failed to refresh apps list; retaining current apps snapshot: {err}");
                    self.bottom_pane
                        .set_connectors_snapshot(Some(snapshot.clone()));
                } else if let Some(snapshot) = partial_snapshot {
                    warn!(
                        "failed to load full apps list; falling back to installed apps snapshot: {err}"
                    );
                    self.refresh_connectors_popup_if_open(&snapshot.connectors);
                    self.connectors_cache = ConnectorsCacheState::Ready(snapshot.clone());
                    self.bottom_pane.set_connectors_snapshot(Some(snapshot));
                } else {
                    self.connectors_cache = ConnectorsCacheState::Failed(err);
                    self.bottom_pane.set_connectors_snapshot(/*snapshot*/ None);
                }
            }
        }

        if trigger_pending_force_refetch {
            self.prefetch_connectors_with_options(/*force_refetch*/ true);
        }
    }

    pub(crate) fn update_connector_enabled(&mut self, connector_id: &str, enabled: bool) {
        let ConnectorsCacheState::Ready(mut snapshot) = self.connectors_cache.clone() else {
            return;
        };

        let mut changed = false;
        for connector in &mut snapshot.connectors {
            if connector.id == connector_id {
                changed = connector.is_enabled != enabled;
                connector.is_enabled = enabled;
                break;
            }
        }

        if !changed {
            return;
        }

        self.refresh_connectors_popup_if_open(&snapshot.connectors);
        self.connectors_cache = ConnectorsCacheState::Ready(snapshot.clone());
        self.bottom_pane.set_connectors_snapshot(Some(snapshot));
    }

    pub(crate) fn refresh_plugin_mentions(&mut self) {
        if !self.config.features.enabled(Feature::Plugins) {
            self.bottom_pane.set_plugin_mentions(/*plugins*/ None);
            return;
        }

        let plugins = PluginsManager::new(self.config.codex_home.clone())
            .plugins_for_config(&self.config)
            .capability_summaries()
            .to_vec();
        self.bottom_pane.set_plugin_mentions(Some(plugins));
    }

    pub(crate) fn sync_plugin_mentions_config(&mut self, config: &Config) {
        self.config.features = config.features.clone();
        self.config.config_layer_stack = config.config_layer_stack.clone();
        self.config.realtime = config.realtime.clone();
    }

    pub(crate) fn open_review_popup(&mut self) {
        let mut items: Vec<SelectionItem> = Vec::new();

        items.push(SelectionItem {
            name: "Review against a base branch".to_string(),
            description: Some("(PR Style)".into()),
            actions: vec![Box::new({
                let cwd = self.config.cwd.to_path_buf();
                move |tx| {
                    tx.send(AppEvent::OpenReviewBranchPicker(cwd.clone()));
                }
            })],
            dismiss_on_select: false,
            ..Default::default()
        });

        items.push(SelectionItem {
            name: "Review uncommitted changes".to_string(),
            actions: vec![Box::new(move |tx: &AppEventSender| {
                tx.review(ReviewRequest {
                    target: ReviewTarget::UncommittedChanges,
                    user_facing_hint: None,
                });
            })],
            dismiss_on_select: true,
            ..Default::default()
        });

        // New: Review a specific commit (opens commit picker)
        items.push(SelectionItem {
            name: "Review a commit".to_string(),
            actions: vec![Box::new({
                let cwd = self.config.cwd.to_path_buf();
                move |tx| {
                    tx.send(AppEvent::OpenReviewCommitPicker(cwd.clone()));
                }
            })],
            dismiss_on_select: false,
            ..Default::default()
        });

        items.push(SelectionItem {
            name: "Custom review instructions".to_string(),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenReviewCustomPrompt);
            })],
            dismiss_on_select: false,
            ..Default::default()
        });

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Select a review preset".into()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    pub(crate) async fn show_review_branch_picker(&mut self, cwd: &Path) {
        let branches = local_git_branches(cwd).await;
        let current_branch = current_branch_name(cwd)
            .await
            .unwrap_or_else(|| "(detached HEAD)".to_string());
        let mut items: Vec<SelectionItem> = Vec::with_capacity(branches.len());

        for option in branches {
            let branch = option.clone();
            items.push(SelectionItem {
                name: format!("{current_branch} -> {branch}"),
                actions: vec![Box::new(move |tx3: &AppEventSender| {
                    tx3.review(ReviewRequest {
                        target: ReviewTarget::BaseBranch {
                            branch: branch.clone(),
                        },
                        user_facing_hint: None,
                    });
                })],
                dismiss_on_select: true,
                search_value: Some(option),
                ..Default::default()
            });
        }

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Select a base branch".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            is_searchable: true,
            search_placeholder: Some("Type to search branches".to_string()),
            ..Default::default()
        });
    }

    pub(crate) async fn show_review_commit_picker(&mut self, cwd: &Path) {
        let commits = recent_commits(cwd, /*limit*/ 100).await;

        let mut items: Vec<SelectionItem> = Vec::with_capacity(commits.len());
        for entry in commits {
            let subject = entry.subject.clone();
            let sha = entry.sha.clone();
            let search_val = format!("{subject} {sha}");

            items.push(SelectionItem {
                name: subject.clone(),
                actions: vec![Box::new(move |tx3: &AppEventSender| {
                    tx3.review(ReviewRequest {
                        target: ReviewTarget::Commit {
                            sha: sha.clone(),
                            title: Some(subject.clone()),
                        },
                        user_facing_hint: None,
                    });
                })],
                dismiss_on_select: true,
                search_value: Some(search_val),
                ..Default::default()
            });
        }

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Select a commit to review".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            is_searchable: true,
            search_placeholder: Some("Type to search commits".to_string()),
            ..Default::default()
        });
    }

    pub(crate) fn show_review_custom_prompt(&mut self) {
        let tx = self.app_event_tx.clone();
        let view = CustomPromptView::new(
            "Custom review instructions".to_string(),
            "Type instructions and press Enter".to_string(),
            /*context_label*/ None,
            Box::new(move |prompt: String| {
                let trimmed = prompt.trim().to_string();
                if trimmed.is_empty() {
                    return;
                }
                tx.review(ReviewRequest {
                    target: ReviewTarget::Custom {
                        instructions: trimmed,
                    },
                    user_facing_hint: None,
                });
            }),
        );
        self.bottom_pane.show_view(Box::new(view));
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
        let cell = self.active_cell.as_ref();
        let hook_cell = self.active_hook_cell.as_ref();
        if cell.is_none() && hook_cell.is_none() {
            return None;
        }
        Some(ActiveCellTranscriptKey {
            revision: self.active_cell_revision,
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
        if let Some(cell) = self.active_cell.as_ref() {
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

    fn as_renderable(&self) -> RenderableItem<'_> {
        let active_cell_renderable = match &self.active_cell {
            Some(cell) => RenderableItem::Borrowed(cell).inset(Insets::tlbr(
                /*top*/ 1, /*left*/ 0, /*bottom*/ 0, /*right*/ 0,
            )),
            None => RenderableItem::Owned(Box::new(())),
        };
        let active_hook_cell_renderable = match &self.active_hook_cell {
            Some(cell) if cell.should_render() => {
                RenderableItem::Borrowed(cell).inset(Insets::tlbr(
                    /*top*/ 1, /*left*/ 0, /*bottom*/ 0, /*right*/ 0,
                ))
            }
            _ => RenderableItem::Owned(Box::new(())),
        };
        let mut flex = FlexRenderable::new();
        flex.push(/*flex*/ 1, active_cell_renderable);
        flex.push(/*flex*/ 0, active_hook_cell_renderable);
        flex.push(
            /*flex*/ 0,
            RenderableItem::Borrowed(&self.bottom_pane).inset(Insets::tlbr(
                /*top*/ 1, /*left*/ 0, /*bottom*/ 0, /*right*/ 0,
            )),
        );
        RenderableItem::Owned(Box::new(flex))
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

impl Renderable for ChatWidget {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.as_renderable().render(area, buf);
        self.last_rendered_width.set(Some(area.width as usize));
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.as_renderable().desired_height(width)
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.as_renderable().cursor_pos(area)
    }
}

#[derive(Debug)]
enum Notification {
    AgentTurnComplete {
        response: String,
    },
    ExecApprovalRequested {
        command: String,
    },
    EditApprovalRequested {
        cwd: PathBuf,
        changes: Vec<PathBuf>,
    },
    ElicitationRequested {
        server_name: String,
    },
    PlanModePrompt {
        title: String,
    },
    UserInputRequested {
        question_count: usize,
        summary: Option<String>,
    },
}

impl Notification {
    fn display(&self) -> String {
        match self {
            Notification::AgentTurnComplete { response } => {
                Notification::agent_turn_preview(response)
                    .unwrap_or_else(|| "Agent turn complete".to_string())
            }
            Notification::ExecApprovalRequested { command } => {
                format!(
                    "Approval requested: {}",
                    truncate_text(command, /*max_graphemes*/ 30)
                )
            }
            Notification::EditApprovalRequested { cwd, changes } => {
                format!(
                    "Codex wants to edit {}",
                    if changes.len() == 1 {
                        #[allow(clippy::unwrap_used)]
                        display_path_for(changes.first().unwrap(), cwd)
                    } else {
                        format!("{} files", changes.len())
                    }
                )
            }
            Notification::ElicitationRequested { server_name } => {
                format!("Approval requested by {server_name}")
            }
            Notification::PlanModePrompt { title } => {
                format!("Plan mode prompt: {title}")
            }
            Notification::UserInputRequested {
                question_count,
                summary,
            } => match (*question_count, summary.as_deref()) {
                (1, Some(summary)) => format!("Question requested: {summary}"),
                (1, None) => "Question requested".to_string(),
                (count, _) => format!("Questions requested: {count}"),
            },
        }
    }

    fn type_name(&self) -> &str {
        match self {
            Notification::AgentTurnComplete { .. } => "agent-turn-complete",
            Notification::ExecApprovalRequested { .. }
            | Notification::EditApprovalRequested { .. }
            | Notification::ElicitationRequested { .. } => "approval-requested",
            Notification::PlanModePrompt { .. } => "plan-mode-prompt",
            Notification::UserInputRequested { .. } => "user-input-requested",
        }
    }

    fn priority(&self) -> u8 {
        match self {
            Notification::AgentTurnComplete { .. } => 0,
            Notification::ExecApprovalRequested { .. }
            | Notification::EditApprovalRequested { .. }
            | Notification::ElicitationRequested { .. }
            | Notification::PlanModePrompt { .. }
            | Notification::UserInputRequested { .. } => 1,
        }
    }

    fn allowed_for(&self, settings: &Notifications) -> bool {
        match settings {
            Notifications::Enabled(enabled) => *enabled,
            Notifications::Custom(allowed) => allowed.iter().any(|a| a == self.type_name()),
        }
    }

    fn agent_turn_preview(response: &str) -> Option<String> {
        let mut normalized = String::new();
        for part in response.split_whitespace() {
            if !normalized.is_empty() {
                normalized.push(' ');
            }
            normalized.push_str(part);
        }
        let trimmed = normalized.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(truncate_text(trimmed, AGENT_NOTIFICATION_PREVIEW_GRAPHEMES))
        }
    }

    fn user_input_request_summary(
        questions: &[codex_protocol::request_user_input::RequestUserInputQuestion],
    ) -> Option<String> {
        let first_question = questions.first()?;
        let summary = if first_question.header.trim().is_empty() {
            first_question.question.trim()
        } else {
            first_question.header.trim()
        };
        if summary.is_empty() {
            None
        } else {
            Some(truncate_text(summary, /*max_graphemes*/ 30))
        }
    }
}

const AGENT_NOTIFICATION_PREVIEW_GRAPHEMES: usize = 200;

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
pub(crate) fn show_review_commit_picker_with_entries(
    chat: &mut ChatWidget,
    entries: Vec<CommitLogEntry>,
) {
    let mut items: Vec<SelectionItem> = Vec::with_capacity(entries.len());
    for entry in entries {
        let subject = entry.subject.clone();
        let sha = entry.sha.clone();
        let search_val = format!("{subject} {sha}");

        items.push(SelectionItem {
            name: subject.clone(),
            actions: vec![Box::new(move |tx3: &AppEventSender| {
                tx3.review(ReviewRequest {
                    target: ReviewTarget::Commit {
                        sha: sha.clone(),
                        title: Some(subject.clone()),
                    },
                    user_facing_hint: None,
                });
            })],
            dismiss_on_select: true,
            search_value: Some(search_val),
            ..Default::default()
        });
    }

    chat.bottom_pane.show_selection_view(SelectionViewParams {
        title: Some("Select a commit to review".to_string()),
        footer_hint: Some(standard_popup_hint_line()),
        items,
        is_searchable: true,
        search_placeholder: Some("Type to search commits".to_string()),
        ..Default::default()
    });
}

#[cfg(test)]
pub(crate) mod tests;
