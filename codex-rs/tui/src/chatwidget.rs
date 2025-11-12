use std::collections::HashMap;
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use codex_core::config::Config;
use codex_core::config::types::Notifications;
use codex_core::git_info::current_branch_name;
use codex_core::git_info::local_git_branches;
use codex_core::project_doc::DEFAULT_PROJECT_DOC_FILENAME;
use codex_core::protocol::AgentMessageDeltaEvent;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::AgentReasoningDeltaEvent;
use codex_core::protocol::AgentReasoningEvent;
use codex_core::protocol::AgentReasoningRawContentDeltaEvent;
use codex_core::protocol::AgentReasoningRawContentEvent;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::BackgroundEventEvent;
use codex_core::protocol::DeprecationNoticeEvent;
use codex_core::protocol::ErrorEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::ExitedReviewModeEvent;
use codex_core::protocol::ListCustomPromptsResponseEvent;
use codex_core::protocol::McpListToolsResponseEvent;
use codex_core::protocol::McpToolCallBeginEvent;
use codex_core::protocol::McpToolCallEndEvent;
use codex_core::protocol::Op;
use codex_core::protocol::PatchApplyBeginEvent;
use codex_core::protocol::RateLimitSnapshot;
use codex_core::protocol::ReviewRequest;
use codex_core::protocol::StreamErrorEvent;
use codex_core::protocol::TaskCompleteEvent;
use codex_core::protocol::TokenUsage;
use codex_core::protocol::TokenUsageInfo;
use codex_core::protocol::TurnAbortReason;
use codex_core::protocol::TurnDiffEvent;
use codex_core::protocol::UndoCompletedEvent;
use codex_core::protocol::UndoStartedEvent;
use codex_core::protocol::UserMessageEvent;
use codex_core::protocol::ViewImageToolCallEvent;
use codex_core::protocol::WarningEvent;
use codex_core::protocol::WebSearchBeginEvent;
use codex_core::protocol::WebSearchEndEvent;
use codex_protocol::ConversationId;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::user_input::UserInput;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use dirs::home_dir;
use rand::Rng;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::debug;

use crate::app_event::AppEvent;
use crate::app_event::SecurityReviewAutoScopeSelection;
use crate::app_event::SecurityReviewCommandState;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ApprovalRequest;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BottomPaneParams;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::InputResult;
use crate::bottom_pane::SecurityReviewScopeConfirmView;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::StatusSnapshot;
use crate::bottom_pane::custom_prompt_view::CustomPromptView;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::clipboard_paste::paste_image_to_temp_png;
use crate::diff_render::display_path_for;
use crate::exec_cell::CommandOutput;
use crate::exec_cell::ExecCell;
use crate::exec_cell::new_active_exec_command;
use crate::get_git_diff::get_git_diff;
use crate::history_cell;
use crate::history_cell::AgentMessageCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::McpToolCallCell;
use crate::history_cell::PlainHistoryCell;
use crate::markdown::append_markdown;
#[cfg(target_os = "windows")]
use crate::onboarding::WSL_INSTRUCTIONS;
use crate::render::Insets;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::FlexRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt;
use crate::render::renderable::RenderableItem;
use crate::security_review::SECURITY_REVIEW_FOLLOW_UP_MARKER;
use crate::security_review::SecurityReviewFailure;
use crate::security_review::SecurityReviewMetadata;
use crate::security_review::SecurityReviewMode;
use crate::security_review::SecurityReviewRequest;
use crate::security_review::SecurityReviewResult;
use crate::security_review::build_follow_up_user_prompt;
use crate::security_review::read_security_review_metadata;
use crate::security_review::run_security_review;
use crate::slash_command::SlashCommand;
use crate::status::RateLimitSnapshotDisplay;
use crate::status_indicator_widget::fmt_elapsed_compact;
use crate::text_formatting::truncate_text;
use crate::tui::FrameRequester;
mod interrupts;
use self::interrupts::InterruptManager;
mod agent;
use self::agent::spawn_agent;
use self::agent::spawn_agent_from_existing;
mod session_header;
use self::session_header::SessionHeader;
use crate::streaming::controller::StreamController;

use chrono::Local;
use codex_common::approval_presets::ApprovalPreset;
use codex_common::approval_presets::builtin_approval_presets;
use codex_common::model_presets::ModelPreset;
use codex_common::model_presets::builtin_model_presets;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_file_search::FileMatch;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::plan_tool::UpdatePlanArgs;
use strum::IntoEnumIterator;

const MAX_TRACKED_GHOST_COMMITS: usize = 20;
const MAX_STATUS_THINKING_LINES: usize = 3;
const MAX_STATUS_TOOL_CALLS: usize = 4;
const USER_SHELL_COMMAND_HELP_TITLE: &str = "Prefix a command with ! to run it locally";
const USER_SHELL_COMMAND_HELP_HINT: &str = "Example: !ls";
// Track information about an in-flight exec command.
struct RunningCommand {
    command: Vec<String>,
    parsed_cmd: Vec<ParsedCommand>,
    is_user_shell_command: bool,
}

const RATE_LIMIT_WARNING_THRESHOLDS: [f64; 3] = [75.0, 90.0, 95.0];
const NUDGE_MODEL_SLUG: &str = "gpt-5-codex-mini";
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
                warnings.push(format!(
                    "Heads up, you've used over {threshold:.0}% of your {limit_label} limit. Run /status for a breakdown."
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
                warnings.push(format!(
                    "Heads up, you've used over {threshold:.0}% of your {limit_label} limit. Run /status for a breakdown."
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
    pub(crate) initial_prompt: Option<String>,
    pub(crate) initial_images: Vec<PathBuf>,
    pub(crate) enhanced_keys_supported: bool,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) feedback: codex_feedback::CodexFeedback,
}

#[derive(Default)]
enum RateLimitSwitchPromptState {
    #[default]
    Idle,
    Pending,
    Shown,
}

pub(crate) struct ChatWidget {
    app_event_tx: AppEventSender,
    codex_op_tx: UnboundedSender<Op>,
    bottom_pane: BottomPane,
    active_cell: Option<Box<dyn HistoryCell>>,
    config: Config,
    auth_manager: Arc<AuthManager>,
    session_header: SessionHeader,
    initial_user_message: Option<UserMessage>,
    token_info: Option<TokenUsageInfo>,
    rate_limit_snapshot: Option<RateLimitSnapshotDisplay>,
    rate_limit_warnings: RateLimitWarningState,
    rate_limit_switch_prompt: RateLimitSwitchPromptState,
    // Stream lifecycle controller
    stream_controller: Option<StreamController>,
    running_commands: HashMap<String, RunningCommand>,
    task_complete_pending: bool,
    // Queue of interruptive UI events deferred during an active write cycle
    interrupts: InterruptManager,
    // Accumulates the current reasoning block text to extract a header
    reasoning_buffer: String,
    // Accumulates full reasoning content for transcript-only recording
    full_reasoning_buffer: String,
    // Current status header shown in the status indicator.
    current_status_header: String,
    status_progress: Option<f32>,
    status_thinking_lines: Vec<String>,
    status_tool_calls: Vec<(String, String)>,
    // Previous status header to restore after a transient stream retry.
    retry_status_header: Option<String>,
    conversation_id: Option<ConversationId>,
    frame_requester: FrameRequester,
    default_placeholder: String,
    // Whether to include the initial welcome banner on session configured
    show_welcome_banner: bool,
    // When resuming an existing session (selected via resume picker), avoid an
    // immediate redraw on SessionConfigured to prevent a gratuitous UI flicker.
    suppress_session_configured_redraw: bool,
    // User messages queued while a turn is in progress
    queued_user_messages: VecDeque<UserMessage>,
    // Pending notification to show when unfocused on next Draw
    pending_notification: Option<Notification>,
    // Simple review mode flag; used to adjust layout and banners.
    is_review_mode: bool,
    // Whether to add a final message separator after the last message
    needs_final_message_separator: bool,

    security_review_task: Option<JoinHandle<()>>,
    security_review_context: Option<SecurityReviewContext>,
    security_review_artifacts: Option<SecurityReviewArtifactsState>,
    security_review_follow_up: Option<SecurityReviewFollowUpState>,

    last_rendered_width: std::cell::Cell<Option<usize>>,
    // Feedback sink for /feedback
    feedback: codex_feedback::CodexFeedback,
    // Current session rollout path (if known)
    current_rollout_path: Option<PathBuf>,
}

struct UserMessage {
    text: String,
    image_paths: Vec<PathBuf>,
}

impl From<String> for UserMessage {
    fn from(text: String) -> Self {
        Self {
            text,
            image_paths: Vec::new(),
        }
    }
}

impl From<&str> for UserMessage {
    fn from(text: &str) -> Self {
        Self {
            text: text.to_string(),
            image_paths: Vec::new(),
        }
    }
}

fn create_initial_user_message(text: String, image_paths: Vec<PathBuf>) -> Option<UserMessage> {
    if text.is_empty() && image_paths.is_empty() {
        None
    } else {
        Some(UserMessage { text, image_paths })
    }
}

fn annotate_scope_prompt(prompt: &str) -> String {
    let abbreviations: Vec<&str> = prompt
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| {
            let len = token.chars().count();
            len >= 2 && token.chars().all(|c| c.is_ascii_uppercase())
        })
        .collect();

    if abbreviations.is_empty() {
        return prompt.to_string();
    }

    let abbreviations_list = abbreviations.join(", ");
    format!(
        "{prompt}\n\nAbbreviations detected: {abbreviations_list}. If you can infer their meaning from the repository context, state your assumption; otherwise highlight that clarification is needed."
    )
}

fn strip_progress_prefix(text: &str) -> &str {
    let trimmed = text.trim_start();
    let mut parts = trimmed.splitn(3, ' ');
    let first = parts.next().unwrap_or_default();
    let second = parts.next();
    let rest = parts.next();

    if let (Some(second), Some(rest)) = (second, rest)
        && let Some(digits) = first.strip_suffix('%')
        && !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
        && second.chars().all(|c| c == '█' || c == '░')
    {
        return rest.trim_start();
    }

    text
}

fn parse_progress_suffix(text: &str) -> Option<(u8, &str)> {
    let trimmed = text.trim();
    let trimmed = trimmed.strip_suffix('.').unwrap_or(trimmed);
    let idx = trimmed.rfind(" - ")?;
    let tail = trimmed[idx + 3..].trim();
    let percent_str = tail.strip_suffix('%')?;
    if percent_str.is_empty() || !percent_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let percent = percent_str.parse::<u16>().ok()?.min(100) as u8;
    let core = trimmed[..idx].trim_end();
    Some((percent, core))
}

fn build_percent_prefix(percent: u8) -> String {
    let pct = percent.min(100);
    let width = 10usize;
    let filled = usize::from(pct) * width / 100;
    let mut bar = String::with_capacity(width);
    if filled > 0 {
        bar.push_str(&"█".repeat(filled));
    }
    if width > filled {
        bar.push_str(&"░".repeat(width - filled));
    }
    format!("{pct}% {bar} ")
}

#[derive(Clone)]
struct SecurityReviewResumeCandidate {
    folder_name: String,
    output_root: PathBuf,
    metadata: SecurityReviewMetadata,
}

fn sanitize_repo_slug(repo_path: &Path) -> String {
    let raw = repo_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| repo_path.to_string_lossy().into_owned());
    let mut slug = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else if matches!(ch, '-' | '_') {
            '-'
        } else {
            '-'
        };
        if mapped == '-' {
            if !slug.ends_with('-') {
                slug.push(mapped);
            }
        } else {
            slug.push(mapped);
        }
    }
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "repository".to_string()
    } else {
        trimmed
    }
}

fn security_review_storage_root(repo_path: &Path) -> PathBuf {
    let base = env::var_os("CODEXHOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|dir| dir.join(".codex")))
        .unwrap_or_else(|| repo_path.to_path_buf());
    base.join("appsec_review")
        .join(sanitize_repo_slug(repo_path))
}

fn latest_security_review_candidate(storage_root: &Path) -> Option<SecurityReviewResumeCandidate> {
    let entries = fs::read_dir(storage_root).ok()?;
    let mut candidates: Vec<(String, SecurityReviewResumeCandidate)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let metadata_path = path.join("metadata.json");
        let snapshot_path = path.join("context").join("bugs_snapshot.json");
        if !metadata_path.exists() || !snapshot_path.exists() {
            continue;
        }
        let metadata = match read_security_review_metadata(&metadata_path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let folder_name = entry.file_name().into_string().unwrap_or_else(|_| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("latest")
                .to_string()
        });
        candidates.push((
            folder_name.clone(),
            SecurityReviewResumeCandidate {
                folder_name,
                output_root: path,
                metadata,
            },
        ));
    }
    if candidates.is_empty() {
        return None;
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    let mut candidate = candidates.into_iter().next().map(|(_, c)| c)?;
    if candidate.folder_name.is_empty() {
        candidate.folder_name = candidate
            .output_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("latest")
            .to_string();
    }
    Some(candidate)
}

struct SecurityReviewContext {
    mode: SecurityReviewMode,
    include_paths: Vec<String>,
    output_root: PathBuf,
    repo_path: PathBuf,
    model: String,
    provider_name: String,
    started_at: Instant,
    last_log: Option<String>,
    thinking_lines: Vec<String>,
    log_lines: Vec<String>,
    progress_percent: Option<u8>,
}

struct SecurityReviewFollowUpState {
    repo_root: PathBuf,
    scope_paths: Vec<String>,
    mode: SecurityReviewMode,
    follow_up_path: PathBuf,
    follow_up_label: String,
}

#[allow(dead_code)]
#[derive(Clone)]
struct SecurityReviewArtifactsState {
    repo_root: PathBuf,
    snapshot_path: PathBuf,
    bugs_path: PathBuf,
    report_path: Option<PathBuf>,
    report_html_path: Option<PathBuf>,
    metadata_path: PathBuf,
    api_overview_path: Option<PathBuf>,
    classification_json_path: Option<PathBuf>,
    classification_table_path: Option<PathBuf>,
}

impl ChatWidget {
    fn model_description_for(slug: &str) -> Option<&'static str> {
        if slug.starts_with("gpt-5-codex") {
            Some("Optimized for coding tasks with many tools.")
        } else if slug.starts_with("gpt-5") {
            Some("Broad world knowledge with strong general reasoning.")
        } else {
            None
        }
    }

    fn prompt_security_review_resume(
        &mut self,
        mode: SecurityReviewMode,
        include_paths: Vec<String>,
        scope_prompt: Option<String>,
        candidate: SecurityReviewResumeCandidate,
    ) {
        let repo_path = self.config.cwd.clone();
        let display_path = display_path_for(&candidate.output_root, &repo_path);
        let scope_summary = if candidate.metadata.scope_paths.is_empty() {
            "entire repository".to_string()
        } else {
            candidate.metadata.scope_paths.join(", ")
        };

        let mut items: Vec<SelectionItem> = Vec::new();
        let resume_candidate = candidate;
        items.push(SelectionItem {
            name: format!("Resume latest review ({})", resume_candidate.folder_name),
            description: Some(format!(
                "Mode: {} • Scope: {}",
                resume_candidate.metadata.mode.as_str(),
                scope_summary
            )),
            actions: vec![Box::new(move |tx: &AppEventSender| {
                tx.send(AppEvent::ResumeSecurityReview {
                    output_root: resume_candidate.output_root.clone(),
                    metadata: resume_candidate.metadata.clone(),
                });
            })],
            dismiss_on_select: true,
            search_value: Some(display_path.clone()),
            ..Default::default()
        });

        let include_paths_for_retry = include_paths;
        let scope_prompt_for_retry = scope_prompt;
        items.push(SelectionItem {
            name: "Start a new security review".to_string(),
            description: Some("Create a fresh report".to_string()),
            actions: vec![Box::new(move |tx: &AppEventSender| {
                tx.send(AppEvent::StartSecurityReview {
                    mode,
                    include_paths: include_paths_for_retry.clone(),
                    scope_prompt: scope_prompt_for_retry.clone(),
                    force_new: true,
                });
            })],
            dismiss_on_select: true,
            ..Default::default()
        });

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Existing security review found".to_string()),
            subtitle: Some(format!("Latest output at {display_path}")),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    pub(crate) fn resume_security_review(
        &mut self,
        output_root: PathBuf,
        metadata: SecurityReviewMetadata,
    ) {
        if self.bottom_pane.is_task_running() || self.security_review_context.is_some() {
            self.add_error_message(
                "Cannot resume while a security review is running. Wait for it to finish first."
                    .to_string(),
            );
            return;
        }

        let repo_path = self.config.cwd.clone();
        if !output_root.exists() {
            self.add_error_message(format!(
                "Security review output path {} no longer exists.",
                output_root.display()
            ));
            return;
        }

        let context_dir = output_root.join("context");
        let bugs_path = context_dir.join("bugs.md");
        let snapshot_path = context_dir.join("bugs_snapshot.json");
        let metadata_path = output_root.join("metadata.json");

        if !bugs_path.exists() {
            self.add_error_message(format!(
                "Cannot resume; expected bugs markdown missing at {}.",
                bugs_path.display()
            ));
            return;
        }
        if !snapshot_path.exists() {
            self.add_error_message(format!(
                "Cannot resume; expected snapshot missing at {}.",
                snapshot_path.display()
            ));
            return;
        }
        if !metadata_path.exists() {
            self.add_error_message(format!(
                "Cannot resume; expected metadata missing at {}.",
                metadata_path.display()
            ));
            return;
        }

        let report_path = output_root.join("report.md");
        let report_html_path = output_root.join("report.html");
        let report_path = report_path.exists().then_some(report_path);
        let report_html_path = report_html_path.exists().then_some(report_html_path);
        let api_candidate = context_dir.join("apis.md");
        let api_overview_path = api_candidate.exists().then_some(api_candidate);
        let classification_json_candidate = context_dir.join("classification.jsonl");
        let classification_json_path = classification_json_candidate
            .exists()
            .then_some(classification_json_candidate);
        let classification_table_candidate = context_dir.join("classification.md");
        let classification_table_path = classification_table_candidate
            .exists()
            .then_some(classification_table_candidate);

        self.clear_security_review_follow_up();
        self.security_review_task = None;
        self.bottom_pane.set_task_running(false);
        self.security_review_artifacts = Some(SecurityReviewArtifactsState {
            repo_root: repo_path.clone(),
            snapshot_path,
            bugs_path: bugs_path.clone(),
            report_path: report_path.clone(),
            report_html_path: report_html_path.clone(),
            metadata_path,
            api_overview_path,
            classification_json_path,
            classification_table_path,
        });

        let follow_up_path = match metadata.mode {
            SecurityReviewMode::Full => report_path
                .clone()
                .or(report_html_path.clone())
                .unwrap_or(bugs_path),
            SecurityReviewMode::Bugs => bugs_path,
        };
        let has_report = report_path.is_some() || report_html_path.is_some();
        let follow_up_label = if metadata.mode == SecurityReviewMode::Full && has_report {
            "Report".to_string()
        } else {
            "Bugs".to_string()
        };

        self.security_review_follow_up = Some(SecurityReviewFollowUpState {
            repo_root: repo_path.clone(),
            scope_paths: metadata.scope_paths.clone(),
            mode: metadata.mode,
            follow_up_path: follow_up_path.clone(),
            follow_up_label,
        });
        self.bottom_pane
            .set_placeholder_text("Ask a security review follow-up question".to_string());

        let follow_up_display = display_path_for(&follow_up_path, &repo_path);
        self.add_info_message(
            format!(
                "Resumed security review (mode: {}) — follow-up context loaded from {}.",
                metadata.mode.as_str(),
                follow_up_display
            ),
            None,
        );
    }
    fn flush_answer_stream_with_separator(&mut self) {
        if let Some(mut controller) = self.stream_controller.take()
            && let Some(cell) = controller.finalize()
        {
            self.add_boxed_history(cell);
        }
    }

    fn set_status_header(&mut self, header: String) {
        self.current_status_header = header.clone();
        if self.security_review_context.is_some() {
            self.bottom_pane.update_status_header(header);
        }
    }

    fn clear_status_tracking(&mut self) {
        self.status_progress = None;
        self.status_thinking_lines.clear();
        self.status_tool_calls.clear();
    }

    fn push_status_snapshot(&mut self) {
        if self.security_review_context.is_some() {
            return;
        }

        let tool_calls: Vec<String> = if self.status_tool_calls.len() <= MAX_STATUS_TOOL_CALLS {
            self.status_tool_calls
                .iter()
                .map(|(_, label)| label.clone())
                .collect()
        } else {
            self.status_tool_calls[self.status_tool_calls.len() - MAX_STATUS_TOOL_CALLS..]
                .iter()
                .map(|(_, label)| label.clone())
                .collect()
        };

        let snapshot = StatusSnapshot {
            header: self.current_status_header.clone(),
            progress: self.status_progress,
            thinking: self.status_thinking_lines.clone(),
            tool_calls,
            logs: Vec::new(),
        };
        self.bottom_pane.update_status_snapshot(snapshot);
    }

    fn update_thinking_lines_from_reasoning(&mut self) {
        if self.security_review_context.is_some() {
            return;
        }

        let mut lines: Vec<String> = self
            .reasoning_buffer
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| truncate_text(line, 160))
            .collect();

        if lines.len() > MAX_STATUS_THINKING_LINES {
            let start = lines.len() - MAX_STATUS_THINKING_LINES;
            lines = lines.split_off(start);
        }

        self.status_thinking_lines = lines;
    }

    fn add_tool_call(&mut self, key: String, label: String) {
        if self.security_review_context.is_some() {
            return;
        }
        self.status_tool_calls
            .retain(|(existing_key, _)| existing_key != &key);
        self.status_tool_calls.push((key, label));
        self.push_status_snapshot();
    }

    fn remove_tool_call(&mut self, key: &str) {
        if self.security_review_context.is_some() {
            return;
        }
        let original_len = self.status_tool_calls.len();
        self.status_tool_calls
            .retain(|(existing_key, _)| existing_key != key);
        if self.status_tool_calls.len() != original_len {
            self.push_status_snapshot();
        }
    }

    fn format_exec_command(command: &[String]) -> String {
        shlex::try_join(command.iter().map(String::as_str)).unwrap_or_else(|_| command.join(" "))
    }

    // --- Small event handlers ---
    fn on_session_configured(&mut self, event: codex_core::protocol::SessionConfiguredEvent) {
        self.bottom_pane
            .set_history_metadata(event.history_log_id, event.history_entry_count);
        self.conversation_id = Some(event.session_id);
        self.current_rollout_path = Some(event.rollout_path.clone());
        let initial_messages = event.initial_messages.clone();
        let model_for_header = event.model.clone();
        self.session_header.set_model(&model_for_header);
        self.add_to_history(history_cell::new_session_info(
            &self.config,
            event,
            self.show_welcome_banner,
        ));
        if let Some(messages) = initial_messages {
            self.replay_initial_messages(messages);
        }
        // Ask codex-core to enumerate custom prompts for this session.
        self.submit_op(Op::ListCustomPrompts);
        if let Some(user_message) = self.initial_user_message.take() {
            self.submit_user_message(user_message);
        }
        if !self.suppress_session_configured_redraw {
            self.request_redraw();
        }
    }

    pub(crate) fn open_feedback_note(
        &mut self,
        category: crate::app_event::FeedbackCategory,
        include_logs: bool,
    ) {
        // Build a fresh snapshot at the time of opening the note overlay.
        let snapshot = self.feedback.snapshot(self.conversation_id);
        let rollout = if include_logs {
            self.current_rollout_path.clone()
        } else {
            None
        };
        let view = crate::bottom_pane::FeedbackNoteView::new(
            category,
            snapshot,
            rollout,
            self.app_event_tx.clone(),
            include_logs,
        );
        self.bottom_pane.show_view(Box::new(view));
        self.request_redraw();
    }

    pub(crate) fn open_feedback_consent(&mut self, category: crate::app_event::FeedbackCategory) {
        let params = crate::bottom_pane::feedback_upload_consent_params(
            self.app_event_tx.clone(),
            category,
            self.current_rollout_path.clone(),
        );
        self.bottom_pane.show_selection_view(params);
        self.request_redraw();
    }

    fn on_agent_message(&mut self, message: String) {
        // If we have a stream_controller, then the final agent message is redundant and will be a
        // duplicate of what has already been streamed.
        if self.stream_controller.is_none() {
            self.handle_streaming_delta(message);
        }
        self.flush_answer_stream_with_separator();
        self.handle_stream_finished();
        self.request_redraw();
    }

    fn on_agent_message_delta(&mut self, delta: String) {
        self.handle_streaming_delta(delta);
    }

    fn on_agent_reasoning_delta(&mut self, delta: String) {
        // For reasoning deltas, do not stream to history. Accumulate the
        // current reasoning block and extract the first bold element
        // (between **/**) as the chunk header. Show this header as status.
        self.reasoning_buffer.push_str(&delta);
        self.update_thinking_lines_from_reasoning();

        if let Some(header) = extract_first_bold(&self.reasoning_buffer) {
            // Update the shimmer header to the extracted reasoning chunk header.
            self.set_status_header(header);
        }
        self.push_status_snapshot();
        self.request_redraw();
    }

    fn on_agent_reasoning_final(&mut self) {
        // At the end of a reasoning block, record transcript-only content.
        self.full_reasoning_buffer.push_str(&self.reasoning_buffer);
        if !self.full_reasoning_buffer.is_empty() {
            let cell = history_cell::new_reasoning_summary_block(
                self.full_reasoning_buffer.clone(),
                &self.config,
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
        self.bottom_pane.clear_ctrl_c_quit_hint();
        self.bottom_pane.set_task_running(true);
        self.retry_status_header = None;
        self.clear_status_tracking();
        self.bottom_pane.set_interrupt_hint_visible(true);
        self.set_status_header(String::from("Working"));
        self.full_reasoning_buffer.clear();
        self.reasoning_buffer.clear();
        self.push_status_snapshot();
        self.request_redraw();
    }

    fn on_task_complete(&mut self, last_agent_message: Option<String>) {
        // If a stream is currently active, finalize it.
        self.flush_answer_stream_with_separator();
        self.clear_status_tracking();
        self.push_status_snapshot();
        // Mark task stopped and request redraw now that all content is in history.
        self.bottom_pane.set_task_running(false);
        self.running_commands.clear();
        self.request_redraw();

        // If there is a queued user message, send exactly one now to begin the next turn.
        self.maybe_send_next_queued_input();
        // Emit a notification when the turn completes (suppressed if focused).
        self.notify(Notification::AgentTurnComplete {
            response: last_agent_message.unwrap_or_default(),
        });

        self.maybe_show_pending_rate_limit_prompt();
    }

    pub(crate) fn set_token_info(&mut self, info: Option<TokenUsageInfo>) {
        if let Some(info) = info {
            let context_window = info
                .model_context_window
                .or(self.config.model_context_window);
            let percent = context_window.map(|window| {
                info.last_token_usage
                    .percent_of_context_window_remaining(window)
            });
            self.bottom_pane.set_context_window_percent(percent);
            self.token_info = Some(info);
        }
    }

    fn on_rate_limit_snapshot(&mut self, snapshot: Option<RateLimitSnapshot>) {
        if let Some(snapshot) = snapshot {
            let warnings = self.rate_limit_warnings.take_warnings(
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
            );

            let high_usage = snapshot
                .secondary
                .as_ref()
                .map(|w| w.used_percent >= RATE_LIMIT_SWITCH_PROMPT_THRESHOLD)
                .unwrap_or(false)
                || snapshot
                    .primary
                    .as_ref()
                    .map(|w| w.used_percent >= RATE_LIMIT_SWITCH_PROMPT_THRESHOLD)
                    .unwrap_or(false);

            if high_usage
                && !self.rate_limit_switch_prompt_hidden()
                && self.config.model != NUDGE_MODEL_SLUG
                && !matches!(
                    self.rate_limit_switch_prompt,
                    RateLimitSwitchPromptState::Shown
                )
            {
                self.rate_limit_switch_prompt = RateLimitSwitchPromptState::Pending;
            }

            let display = crate::status::rate_limit_snapshot_display(&snapshot, Local::now());
            self.rate_limit_snapshot = Some(display);

            if !warnings.is_empty() {
                for warning in warnings {
                    self.add_to_history(history_cell::new_warning_event(warning));
                }
                self.request_redraw();
            }
        } else {
            self.rate_limit_snapshot = None;
        }
    }
    /// Finalize any active exec as failed and stop/clear running UI state.
    fn finalize_turn(&mut self) {
        // Ensure any spinner is replaced by a red ✗ and flushed into history.
        self.finalize_active_cell_as_failed();
        // Reset running state and clear streaming buffers.
        self.bottom_pane.set_task_running(false);
        self.running_commands.clear();
        self.stream_controller = None;
        self.maybe_show_pending_rate_limit_prompt();
    }

    fn on_error(&mut self, message: String) {
        self.finalize_turn();
        self.add_to_history(history_cell::new_error_event(message));
        self.request_redraw();

        // After an error ends the turn, try sending the next queued input.
        self.maybe_send_next_queued_input();
    }

    fn on_warning(&mut self, message: String) {
        self.add_to_history(history_cell::new_warning_event(message));
        self.request_redraw();
    }

    /// Handle a turn aborted due to user interrupt (Esc).
    /// When there are queued user messages, restore them into the composer
    /// separated by newlines rather than auto‑submitting the next one.
    fn on_interrupted_turn(&mut self, reason: TurnAbortReason) {
        // Finalize, log a gentle prompt, and clear running state.
        self.finalize_turn();

        if reason != TurnAbortReason::ReviewEnded {
            self.add_to_history(history_cell::new_error_event(
                "Conversation interrupted - tell the model what to do differently. Something went wrong? Hit `/feedback` to report the issue.".to_owned(),
            ));
        }

        // If any messages were queued during the task, restore them into the composer.
        if !self.queued_user_messages.is_empty() {
            let queued_text = self
                .queued_user_messages
                .iter()
                .map(|m| m.text.clone())
                .collect::<Vec<_>>()
                .join("\n");
            let existing_text = self.bottom_pane.composer_text();
            let combined = if existing_text.is_empty() {
                queued_text
            } else if queued_text.is_empty() {
                existing_text
            } else {
                format!("{queued_text}\n{existing_text}")
            };
            self.bottom_pane.set_composer_text(combined);
            // Clear the queue and update the status indicator list.
            self.queued_user_messages.clear();
            self.refresh_queued_user_messages();
        }

        self.request_redraw();
    }

    fn on_plan_update(&mut self, update: UpdatePlanArgs) {
        if self.security_review_context.is_none() {
            if update.plan.is_empty() {
                self.status_progress = None;
            } else {
                let total = update.plan.len() as f32;
                let completed = update
                    .plan
                    .iter()
                    .filter(|item| matches!(item.status, StepStatus::Completed))
                    .count() as f32;
                let in_progress = update
                    .plan
                    .iter()
                    .filter(|item| matches!(item.status, StepStatus::InProgress))
                    .count() as f32;
                let progress = ((completed + in_progress * 0.5) / total).clamp(0.0, 1.0);
                self.status_progress = Some(progress);
            }
            self.push_status_snapshot();
        }
        self.add_to_history(history_cell::new_plan_update(update));
    }

    fn on_exec_approval_request(&mut self, id: String, ev: ExecApprovalRequestEvent) {
        let id2 = id.clone();
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_exec_approval(id, ev),
            |s| s.handle_exec_approval_now(id2, ev2),
        );
    }

    fn on_apply_patch_approval_request(&mut self, id: String, ev: ApplyPatchApprovalRequestEvent) {
        let id2 = id.clone();
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_apply_patch_approval(id, ev),
            |s| s.handle_apply_patch_approval_now(id2, ev2),
        );
    }

    fn on_exec_command_begin(&mut self, ev: ExecCommandBeginEvent) {
        self.flush_answer_stream_with_separator();
        let ev2 = ev.clone();
        self.defer_or_handle(|q| q.push_exec_begin(ev), |s| s.handle_exec_begin_now(ev2));
    }

    fn on_exec_command_output_delta(
        &mut self,
        _ev: codex_core::protocol::ExecCommandOutputDeltaEvent,
    ) {
        // TODO: Handle streaming exec output if/when implemented
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

    fn on_patch_apply_end(&mut self, event: codex_core::protocol::PatchApplyEndEvent) {
        let ev2 = event.clone();
        self.defer_or_handle(
            |q| q.push_patch_end(event),
            |s| s.handle_patch_apply_end_now(ev2),
        );
    }

    fn on_exec_command_end(&mut self, ev: ExecCommandEndEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(|q| q.push_exec_end(ev), |s| s.handle_exec_end_now(ev2));
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
        let key = format!("web:{}", ev.call_id.as_str());
        self.add_tool_call(key, "Searching web".to_string());
    }

    fn on_web_search_end(&mut self, ev: WebSearchEndEvent) {
        self.flush_answer_stream_with_separator();
        self.remove_tool_call(&format!("web:{}", ev.call_id.as_str()));
        self.add_to_history(history_cell::new_web_search_call(format!(
            "Searched: {}",
            ev.query
        )));
    }

    fn on_get_history_entry_response(
        &mut self,
        event: codex_core::protocol::GetHistoryEntryResponseEvent,
    ) {
        let codex_core::protocol::GetHistoryEntryResponseEvent {
            offset,
            log_id,
            entry,
        } = event;
        self.bottom_pane
            .on_history_entry_response(log_id, offset, entry.map(|e| e.text));
    }

    fn on_shutdown_complete(&mut self) {
        self.request_exit();
    }

    fn on_turn_diff(&mut self, unified_diff: String) {
        debug!("TurnDiffEvent: {unified_diff}");
    }

    fn on_deprecation_notice(&mut self, event: DeprecationNoticeEvent) {
        let DeprecationNoticeEvent { summary, details } = event;
        self.add_to_history(history_cell::new_deprecation_notice(summary, details));
        self.request_redraw();
    }

    fn on_background_event(&mut self, message: String) {
        debug!("BackgroundEvent: {message}");
    }

    fn on_undo_started(&mut self, event: UndoStartedEvent) {
        self.bottom_pane.ensure_status_indicator();
        self.bottom_pane.set_interrupt_hint_visible(false);
        let message = event
            .message
            .unwrap_or_else(|| "Undo in progress...".to_string());
        self.set_status_header(message);
    }

    fn on_undo_completed(&mut self, event: UndoCompletedEvent) {
        let UndoCompletedEvent { success, message } = event;
        self.bottom_pane.hide_status_indicator();
        let message = message.unwrap_or_else(|| {
            if success {
                "Undo completed successfully.".to_string()
            } else {
                "Undo failed.".to_string()
            }
        });
        if success {
            self.add_info_message(message, None);
        } else {
            self.add_error_message(message);
        }
    }

    fn on_stream_error(&mut self, message: String) {
        if self.retry_status_header.is_none() {
            self.retry_status_header = Some(self.current_status_header.clone());
        }
        self.set_status_header(message);
        self.push_status_snapshot();
    }

    /// Periodic tick to commit at most one queued line to history with a small delay,
    /// animating the output.
    pub(crate) fn on_commit_tick(&mut self) {
        if let Some(controller) = self.stream_controller.as_mut() {
            let (cell, is_idle) = controller.on_commit_tick();
            if let Some(cell) = cell {
                self.bottom_pane.hide_status_indicator();
                self.add_boxed_history(cell);
            }
            if is_idle {
                self.app_event_tx.send(AppEvent::StopCommitAnimation);
            }
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
        self.flush_active_cell();

        if self.stream_controller.is_none() {
            if self.needs_final_message_separator {
                let elapsed_seconds = self
                    .bottom_pane
                    .status_widget()
                    .map(super::status_indicator_widget::StatusIndicatorWidget::elapsed_seconds);
                self.add_to_history(history_cell::FinalMessageSeparator::new(elapsed_seconds));
                self.needs_final_message_separator = false;
            }
            self.stream_controller = Some(StreamController::new(
                self.last_rendered_width.get().map(|w| w.saturating_sub(2)),
            ));
        }
        if let Some(controller) = self.stream_controller.as_mut()
            && controller.push(&delta)
        {
            self.app_event_tx.send(AppEvent::StartCommitAnimation);
        }
        self.request_redraw();
    }

    pub(crate) fn handle_exec_end_now(&mut self, ev: ExecCommandEndEvent) {
        self.remove_tool_call(&format!("exec:{}", ev.call_id.as_str()));
        let running = self.running_commands.remove(&ev.call_id);
        let (command, parsed, is_user_shell_command) = match running {
            Some(rc) => (rc.command, rc.parsed_cmd, rc.is_user_shell_command),
            None => (vec![ev.call_id.clone()], Vec::new(), false),
        };

        let needs_new = self
            .active_cell
            .as_ref()
            .map(|cell| cell.as_any().downcast_ref::<ExecCell>().is_none())
            .unwrap_or(true);
        if needs_new {
            self.flush_active_cell();
            self.active_cell = Some(Box::new(new_active_exec_command(
                ev.call_id.clone(),
                command,
                parsed,
                is_user_shell_command,
            )));
        }

        if let Some(cell) = self
            .active_cell
            .as_mut()
            .and_then(|c| c.as_any_mut().downcast_mut::<ExecCell>())
        {
            cell.complete_call(
                &ev.call_id,
                CommandOutput {
                    exit_code: ev.exit_code,
                    formatted_output: ev.formatted_output.clone(),
                    aggregated_output: ev.aggregated_output.clone(),
                },
                ev.duration,
            );
            if cell.should_flush() {
                self.flush_active_cell();
            }
        }
    }

    pub(crate) fn handle_patch_apply_end_now(
        &mut self,
        event: codex_core::protocol::PatchApplyEndEvent,
    ) {
        // If the patch was successful, just let the "Edited" block stand.
        // Otherwise, add a failure block.
        if !event.success {
            self.add_to_history(history_cell::new_patch_apply_failure(event.stderr));
        }
    }

    pub(crate) fn handle_exec_approval_now(&mut self, id: String, ev: ExecApprovalRequestEvent) {
        self.flush_answer_stream_with_separator();
        let command = shlex::try_join(ev.command.iter().map(String::as_str))
            .unwrap_or_else(|_| ev.command.join(" "));
        self.notify(Notification::ExecApprovalRequested { command });

        let request = ApprovalRequest::Exec {
            id,
            command: ev.command,
            reason: ev.reason,
            risk: ev.risk,
        };
        self.bottom_pane.push_approval_request(request);
        self.request_redraw();
    }

    pub(crate) fn handle_apply_patch_approval_now(
        &mut self,
        id: String,
        ev: ApplyPatchApprovalRequestEvent,
    ) {
        self.flush_answer_stream_with_separator();

        let request = ApprovalRequest::ApplyPatch {
            id,
            reason: ev.reason,
            changes: ev.changes.clone(),
            cwd: self.config.cwd.clone(),
        };
        self.bottom_pane.push_approval_request(request);
        self.request_redraw();
        self.notify(Notification::EditApprovalRequested {
            cwd: self.config.cwd.clone(),
            changes: ev.changes.keys().cloned().collect(),
        });
    }

    pub(crate) fn handle_exec_begin_now(&mut self, ev: ExecCommandBeginEvent) {
        // Ensure the status indicator is visible while the command runs.
        let command_display = Self::format_exec_command(&ev.command);
        let command_display = truncate_text(&command_display, 120);
        let label = format!("Running {command_display}");
        let key = format!("exec:{}", ev.call_id.as_str());
        self.add_tool_call(key, label);
        self.running_commands.insert(
            ev.call_id.clone(),
            RunningCommand {
                command: ev.command.clone(),
                parsed_cmd: ev.parsed_cmd.clone(),
                is_user_shell_command: ev.is_user_shell_command,
            },
        );
        if let Some(cell) = self
            .active_cell
            .as_mut()
            .and_then(|c| c.as_any_mut().downcast_mut::<ExecCell>())
            && let Some(new_exec) = cell.with_added_call(
                ev.call_id.clone(),
                ev.command.clone(),
                ev.parsed_cmd.clone(),
                ev.is_user_shell_command,
            )
        {
            *cell = new_exec;
        } else {
            self.flush_active_cell();

            self.active_cell = Some(Box::new(new_active_exec_command(
                ev.call_id.clone(),
                ev.command.clone(),
                ev.parsed_cmd,
                ev.is_user_shell_command,
            )));
        }

        self.request_redraw();
    }

    pub(crate) fn handle_mcp_begin_now(&mut self, ev: McpToolCallBeginEvent) {
        let label_text = {
            let invocation = &ev.invocation;
            format!("Calling {}::{}", invocation.server, invocation.tool)
        };
        let label_text = truncate_text(&label_text, 120);
        let key = format!("mcp:{}", ev.call_id.as_str());
        self.add_tool_call(key, label_text);
        self.flush_answer_stream_with_separator();
        self.flush_active_cell();
        self.active_cell = Some(Box::new(history_cell::new_active_mcp_tool_call(
            ev.call_id,
            ev.invocation,
        )));
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
        self.remove_tool_call(&format!("mcp:{}", call_id.as_str()));

        let extra_cell = match self
            .active_cell
            .as_mut()
            .and_then(|cell| cell.as_any_mut().downcast_mut::<McpToolCallCell>())
        {
            Some(cell) if cell.call_id() == call_id => cell.complete(duration, result),
            _ => {
                self.flush_active_cell();
                let mut cell = history_cell::new_active_mcp_tool_call(call_id, invocation);
                let extra_cell = cell.complete(duration, result);
                self.active_cell = Some(Box::new(cell));
                extra_cell
            }
        };

        self.flush_active_cell();
        if let Some(extra) = extra_cell {
            self.add_boxed_history(extra);
        }
    }

    pub(crate) fn new(
        common: ChatWidgetInit,
        conversation_manager: Arc<ConversationManager>,
    ) -> Self {
        let ChatWidgetInit {
            config,
            frame_requester,
            app_event_tx,
            initial_prompt,
            initial_images,
            enhanced_keys_supported,
            auth_manager,
            feedback,
        } = common;
        let mut rng = rand::rng();
        let placeholder = EXAMPLE_PROMPTS[rng.random_range(0..EXAMPLE_PROMPTS.len())].to_string();
        let default_placeholder = placeholder.clone();
        let codex_op_tx = spawn_agent(config.clone(), app_event_tx.clone(), conversation_manager);

        Self {
            app_event_tx: app_event_tx.clone(),
            frame_requester: frame_requester.clone(),
            codex_op_tx,
            bottom_pane: BottomPane::new(BottomPaneParams {
                frame_requester,
                app_event_tx,
                has_input_focus: true,
                enhanced_keys_supported,
                placeholder_text: placeholder,
                disable_paste_burst: config.disable_paste_burst,
            }),
            active_cell: None,
            config: config.clone(),
            auth_manager,
            session_header: SessionHeader::new(config.model),
            initial_user_message: create_initial_user_message(
                initial_prompt.unwrap_or_default(),
                initial_images,
            ),
            token_info: None,
            rate_limit_snapshot: None,
            rate_limit_warnings: RateLimitWarningState::default(),
            rate_limit_switch_prompt: RateLimitSwitchPromptState::default(),
            stream_controller: None,
            running_commands: HashMap::new(),
            task_complete_pending: false,
            interrupts: InterruptManager::new(),
            reasoning_buffer: String::new(),
            full_reasoning_buffer: String::new(),
            current_status_header: String::from("Working"),
            status_progress: None,
            status_thinking_lines: Vec::new(),
            status_tool_calls: Vec::new(),
            retry_status_header: None,
            conversation_id: None,
            queued_user_messages: VecDeque::new(),
            show_welcome_banner: true,
            suppress_session_configured_redraw: false,
            pending_notification: None,
            is_review_mode: false,
            needs_final_message_separator: false,
            security_review_task: None,
            security_review_context: None,
            security_review_artifacts: None,
            security_review_follow_up: None,
            default_placeholder,
            last_rendered_width: std::cell::Cell::new(None),
            feedback,
            current_rollout_path: None,
        }
    }

    /// Create a ChatWidget attached to an existing conversation (e.g., a fork).
    pub(crate) fn new_from_existing(
        common: ChatWidgetInit,
        conversation: std::sync::Arc<codex_core::CodexConversation>,
        session_configured: codex_core::protocol::SessionConfiguredEvent,
    ) -> Self {
        let ChatWidgetInit {
            config,
            frame_requester,
            app_event_tx,
            initial_prompt,
            initial_images,
            enhanced_keys_supported,
            auth_manager,
            feedback,
        } = common;
        let mut rng = rand::rng();
        let placeholder = EXAMPLE_PROMPTS[rng.random_range(0..EXAMPLE_PROMPTS.len())].to_string();
        let default_placeholder = placeholder.clone();

        let codex_op_tx =
            spawn_agent_from_existing(conversation, session_configured, app_event_tx.clone());

        Self {
            app_event_tx: app_event_tx.clone(),
            frame_requester: frame_requester.clone(),
            codex_op_tx,
            bottom_pane: BottomPane::new(BottomPaneParams {
                frame_requester,
                app_event_tx,
                has_input_focus: true,
                enhanced_keys_supported,
                placeholder_text: placeholder,
                disable_paste_burst: config.disable_paste_burst,
            }),
            active_cell: None,
            config: config.clone(),
            auth_manager,
            session_header: SessionHeader::new(config.model),
            initial_user_message: create_initial_user_message(
                initial_prompt.unwrap_or_default(),
                initial_images,
            ),
            token_info: None,
            rate_limit_snapshot: None,
            rate_limit_warnings: RateLimitWarningState::default(),
            rate_limit_switch_prompt: RateLimitSwitchPromptState::default(),
            stream_controller: None,
            running_commands: HashMap::new(),
            task_complete_pending: false,
            interrupts: InterruptManager::new(),
            reasoning_buffer: String::new(),
            full_reasoning_buffer: String::new(),
            current_status_header: String::from("Working"),
            status_progress: None,
            status_thinking_lines: Vec::new(),
            status_tool_calls: Vec::new(),
            retry_status_header: None,
            conversation_id: None,
            queued_user_messages: VecDeque::new(),
            show_welcome_banner: true,
            suppress_session_configured_redraw: true,
            pending_notification: None,
            is_review_mode: false,
            needs_final_message_separator: false,
            security_review_task: None,
            security_review_context: None,
            security_review_artifacts: None,
            security_review_follow_up: None,
            default_placeholder,
            last_rendered_width: std::cell::Cell::new(None),
            feedback,
            current_rollout_path: None,
        }
    }

    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
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
            } if modifiers.contains(KeyModifiers::CONTROL) && c.eq_ignore_ascii_case(&'v') => {
                match paste_image_to_temp_png() {
                    Ok((path, info)) => {
                        self.attach_image(
                            path,
                            info.width,
                            info.height,
                            info.encoded_format.label(),
                        );
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
                self.bottom_pane.clear_ctrl_c_quit_hint();
            }
            _ => {}
        }

        match key_event {
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::ALT,
                kind: KeyEventKind::Press,
                ..
            } if !self.queued_user_messages.is_empty() => {
                // Prefer the most recently queued item.
                if let Some(user_message) = self.queued_user_messages.pop_back() {
                    self.bottom_pane.set_composer_text(user_message.text);
                    self.refresh_queued_user_messages();
                    self.request_redraw();
                }
            }
            _ => {
                match self.bottom_pane.handle_key_event(key_event) {
                    InputResult::Submitted(text) => {
                        // If a task is running, queue the user input to be sent after the turn completes.
                        let user_message = UserMessage {
                            text,
                            image_paths: self.bottom_pane.take_recent_submission_images(),
                        };
                        self.queue_user_message(user_message);
                    }
                    InputResult::Command(cmd) => {
                        self.dispatch_command(cmd);
                    }
                    InputResult::None => {}
                }
            }
        }
    }

    pub(crate) fn attach_image(
        &mut self,
        path: PathBuf,
        width: u32,
        height: u32,
        format_label: &str,
    ) {
        tracing::info!(
            "attach_image path={path:?} width={width} height={height} format={format_label}",
        );
        self.bottom_pane
            .attach_image(path, width, height, format_label);
        self.request_redraw();
    }

    fn dispatch_command(&mut self, cmd: SlashCommand) {
        if !cmd.available_during_task() && self.bottom_pane.is_task_running() {
            let message = format!(
                "'/{}' is disabled while a task is in progress.",
                cmd.command()
            );
            self.add_to_history(history_cell::new_error_event(message));
            self.request_redraw();
            return;
        }
        match cmd {
            SlashCommand::Feedback => {
                // Step 1: pick a category (UI built in feedback_view)
                let params =
                    crate::bottom_pane::feedback_selection_params(self.app_event_tx.clone());
                self.bottom_pane.show_selection_view(params);
                self.request_redraw();
            }
            SlashCommand::New => {
                self.app_event_tx.send(AppEvent::NewSession);
            }
            SlashCommand::Init => {
                let init_target = self.config.cwd.join(DEFAULT_PROJECT_DOC_FILENAME);
                if init_target.exists() {
                    let message = format!(
                        "{DEFAULT_PROJECT_DOC_FILENAME} already exists here. Skipping /init to avoid overwriting it."
                    );
                    self.add_info_message(message, None);
                    return;
                }
                const INIT_PROMPT: &str = include_str!("../prompt_for_init_command.md");
                self.submit_user_message(INIT_PROMPT.to_string().into());
            }
            SlashCommand::Compact => {
                self.clear_token_usage();
                self.app_event_tx.send(AppEvent::CodexOp(Op::Compact));
            }
            SlashCommand::Review => {
                self.open_review_popup();
            }
            SlashCommand::SecReview => {
                self.open_security_review_popup();
            }
            SlashCommand::Validate => {
                // Web/API validation for high-risk findings from the last review
                if self.bottom_pane.is_task_running() || self.security_review_context.is_some() {
                    self.add_error_message(
                        "Cannot run /validate while a task is in progress.".to_string(),
                    );
                    self.request_redraw();
                    return;
                }
                let Some(artifacts) = self.security_review_artifacts.clone() else {
                    self.add_error_message(
                        "No security review results to validate. Run /secreview first.".to_string(),
                    );
                    self.request_redraw();
                    return;
                };

                self.bottom_pane.set_task_running(true);
                self.bottom_pane
                    .update_status_header("Validating findings — preparing".to_string());

                let provider = self.config.model_provider.clone();
                let auth = self.auth_manager.auth();
                let model = self.config.model.clone();
                let tx = self.app_event_tx.clone();
                let repo_path = self.config.cwd.clone();
                tokio::spawn(async move {
                    use crate::security_review::run_web_validation;
                    match run_web_validation(
                        repo_path,
                        artifacts.snapshot_path.clone(),
                        artifacts.bugs_path.clone(),
                        artifacts.report_path.clone(),
                        artifacts.report_html_path.clone(),
                        provider,
                        auth,
                        model,
                        Some(tx.clone()),
                    )
                    .await
                    {
                        Ok(_) => {
                            tx.send(AppEvent::SecurityReviewLog(
                                "Validation complete; report updated.".to_string(),
                            ));
                        }
                        Err(err) => {
                            tx.send(AppEvent::SecurityReviewLog(format!(
                                "Validation failed: {}",
                                err.message
                            )));
                        }
                    }
                    // Clear the in-progress flag regardless of outcome.
                    tx.send(AppEvent::SecurityReviewComplete {
                        result: crate::security_review::SecurityReviewResult {
                            findings_summary: String::new(),
                            bug_summary_table: None,
                            bugs: Vec::new(),
                            bugs_path: artifacts.bugs_path,
                            report_path: artifacts.report_path,
                            report_html_path: artifacts.report_html_path,
                            snapshot_path: artifacts.snapshot_path,
                            metadata_path: artifacts.metadata_path,
                            api_overview_path: artifacts.api_overview_path,
                            classification_json_path: artifacts.classification_json_path,
                            classification_table_path: artifacts.classification_table_path,
                            logs: vec![],
                            token_usage: codex_core::protocol::TokenUsage::default(),
                        },
                    });
                });
            }
            SlashCommand::Model => {
                self.open_model_popup();
            }
            SlashCommand::Approvals => {
                self.open_approvals_popup();
            }
            SlashCommand::Quit | SlashCommand::Exit => {
                self.request_exit();
            }
            SlashCommand::Logout => {
                if let Err(e) = codex_core::auth::logout(
                    &self.config.codex_home,
                    self.config.cli_auth_credentials_store_mode,
                ) {
                    tracing::error!("failed to logout: {e}");
                }
                self.request_exit();
            }
            SlashCommand::Undo => {
                self.app_event_tx.send(AppEvent::CodexOp(Op::Undo));
            }
            SlashCommand::Diff => {
                self.add_diff_in_progress();
                let tx = self.app_event_tx.clone();
                tokio::spawn(async move {
                    let text = match get_git_diff().await {
                        Ok((is_git_repo, diff_text)) => {
                            if is_git_repo {
                                diff_text
                            } else {
                                "`/diff` — _not inside a git repository_".to_string()
                            }
                        }
                        Err(e) => format!("Failed to compute diff: {e}"),
                    };
                    tx.send(AppEvent::DiffResult(text));
                });
            }
            SlashCommand::Mention => {
                self.insert_str("@");
            }
            SlashCommand::Status => {
                self.add_status_output();
            }
            SlashCommand::Mcp => {
                self.add_mcp_output();
            }
            SlashCommand::Rollout => {
                if let Some(path) = self.rollout_path() {
                    self.add_info_message(
                        format!("Current rollout path: {}", path.display()),
                        None,
                    );
                } else {
                    self.add_info_message("Rollout path is not available yet.".to_string(), None);
                }
            }
            SlashCommand::TestApproval => {
                use codex_core::protocol::EventMsg;
                use std::collections::HashMap;

                use codex_core::protocol::ApplyPatchApprovalRequestEvent;
                use codex_core::protocol::FileChange;

                self.app_event_tx.send(AppEvent::CodexEvent(Event {
                    id: "1".to_string(),
                    // msg: EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                    //     call_id: "1".to_string(),
                    //     command: vec!["git".into(), "apply".into()],
                    //     cwd: self.config.cwd.clone(),
                    //     reason: Some("test".to_string()),
                    // }),
                    msg: EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                        call_id: "1".to_string(),
                        changes: HashMap::from([
                            (
                                PathBuf::from("/tmp/test.txt"),
                                FileChange::Add {
                                    content: "test".to_string(),
                                },
                            ),
                            (
                                PathBuf::from("/tmp/test2.txt"),
                                FileChange::Update {
                                    unified_diff: "+test\n-test2".to_string(),
                                    move_path: None,
                                },
                            ),
                        ]),
                        reason: None,
                        grant_root: Some(PathBuf::from("/tmp")),
                    }),
                }));
            }
        }
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

    fn add_to_history(&mut self, cell: impl HistoryCell + 'static) {
        self.add_boxed_history(Box::new(cell));
    }

    fn add_boxed_history(&mut self, cell: Box<dyn HistoryCell>) {
        if !cell.display_lines(u16::MAX).is_empty() {
            // Only break exec grouping if the cell renders visible lines.
            self.flush_active_cell();
            self.needs_final_message_separator = true;
        }
        self.app_event_tx.send(AppEvent::InsertHistoryCell(cell));
    }

    fn queue_user_message(&mut self, user_message: UserMessage) {
        if self.bottom_pane.is_task_running() {
            self.queued_user_messages.push_back(user_message);
            self.refresh_queued_user_messages();
        } else {
            self.submit_user_message(user_message);
        }
    }

    fn submit_user_message(&mut self, user_message: UserMessage) {
        let UserMessage { text, image_paths } = user_message;
        if text.is_empty() && image_paths.is_empty() {
            return;
        }

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
            self.submit_op(Op::RunUserShellCommand {
                command: cmd.to_string(),
            });
            return;
        }

        if !text.is_empty() {
            let outbound = self
                .security_review_follow_up_prompt(&text)
                .unwrap_or_else(|| text.clone());
            items.push(UserInput::Text { text: outbound });
        }

        for path in image_paths {
            items.push(UserInput::LocalImage { path });
        }

        self.codex_op_tx
            .send(Op::UserInput { items })
            .unwrap_or_else(|e| {
                tracing::error!("failed to send message: {e}");
            });

        // Persist the text to cross-session message history.
        if !text.is_empty() {
            self.codex_op_tx
                .send(Op::AddToHistory { text: text.clone() })
                .unwrap_or_else(|e| {
                    tracing::error!("failed to send AddHistory op: {e}");
                });
        }

        // Only show the text portion in conversation history.
        if !text.is_empty() {
            self.add_to_history(history_cell::new_user_prompt(text));
        }
        self.needs_final_message_separator = false;
    }

    /// Replay a subset of initial events into the UI to seed the transcript when
    /// resuming an existing session. This approximates the live event flow and
    /// is intentionally conservative: only safe-to-replay items are rendered to
    /// avoid triggering side effects. Event ids are passed as `None` to
    /// distinguish replayed events from live ones.
    fn replay_initial_messages(&mut self, events: Vec<EventMsg>) {
        for msg in events {
            if matches!(msg, EventMsg::SessionConfigured(_)) {
                continue;
            }
            // `id: None` indicates a synthetic/fake id coming from replay.
            self.dispatch_event_msg(None, msg, true);
        }
    }

    pub(crate) fn handle_codex_event(&mut self, event: Event) {
        let Event { id, msg } = event;
        self.dispatch_event_msg(Some(id), msg, false);
    }

    /// Dispatch a protocol `EventMsg` to the appropriate handler.
    ///
    /// `id` is `Some` for live events and `None` for replayed events from
    /// `replay_initial_messages()`. Callers should treat `None` as a "fake" id
    /// that must not be used to correlate follow-up actions.
    fn dispatch_event_msg(&mut self, id: Option<String>, msg: EventMsg, from_replay: bool) {
        match msg {
            EventMsg::AgentMessageDelta(_)
            | EventMsg::AgentReasoningDelta(_)
            | EventMsg::ExecCommandOutputDelta(_) => {}
            _ => {
                tracing::trace!("handle_codex_event: {:?}", msg);
            }
        }

        match msg {
            EventMsg::SessionConfigured(e) => self.on_session_configured(e),
            EventMsg::AgentMessage(AgentMessageEvent { message }) => self.on_agent_message(message),
            EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta }) => {
                self.on_agent_message_delta(delta)
            }
            EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta })
            | EventMsg::AgentReasoningRawContentDelta(AgentReasoningRawContentDeltaEvent {
                delta,
            }) => self.on_agent_reasoning_delta(delta),
            EventMsg::AgentReasoning(AgentReasoningEvent { .. }) => self.on_agent_reasoning_final(),
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent { text }) => {
                self.on_agent_reasoning_delta(text);
                self.on_agent_reasoning_final()
            }
            EventMsg::AgentReasoningSectionBreak(_) => self.on_reasoning_section_break(),
            EventMsg::TaskStarted(_) => self.on_task_started(),
            EventMsg::TaskComplete(TaskCompleteEvent { last_agent_message }) => {
                self.on_task_complete(last_agent_message)
            }
            EventMsg::TokenCount(ev) => {
                self.set_token_info(ev.info);
                self.on_rate_limit_snapshot(ev.rate_limits);
            }
            EventMsg::Warning(WarningEvent { message }) => self.on_warning(message),
            EventMsg::Error(ErrorEvent { message }) => self.on_error(message),
            EventMsg::TurnAborted(ev) => match ev.reason {
                TurnAbortReason::Interrupted => {
                    self.on_interrupted_turn(ev.reason);
                }
                TurnAbortReason::Replaced => {
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
            EventMsg::ExecCommandBegin(ev) => self.on_exec_command_begin(ev),
            EventMsg::ExecCommandOutputDelta(delta) => self.on_exec_command_output_delta(delta),
            EventMsg::PatchApplyBegin(ev) => self.on_patch_apply_begin(ev),
            EventMsg::PatchApplyEnd(ev) => self.on_patch_apply_end(ev),
            EventMsg::ExecCommandEnd(ev) => self.on_exec_command_end(ev),
            EventMsg::ViewImageToolCall(ev) => self.on_view_image_tool_call(ev),
            EventMsg::McpToolCallBegin(ev) => self.on_mcp_tool_call_begin(ev),
            EventMsg::McpToolCallEnd(ev) => self.on_mcp_tool_call_end(ev),
            EventMsg::WebSearchBegin(ev) => self.on_web_search_begin(ev),
            EventMsg::WebSearchEnd(ev) => self.on_web_search_end(ev),
            EventMsg::GetHistoryEntryResponse(ev) => self.on_get_history_entry_response(ev),
            EventMsg::McpListToolsResponse(ev) => self.on_list_mcp_tools(ev),
            EventMsg::ListCustomPromptsResponse(ev) => self.on_list_custom_prompts(ev),
            EventMsg::ShutdownComplete => self.on_shutdown_complete(),
            EventMsg::TurnDiff(TurnDiffEvent { unified_diff }) => self.on_turn_diff(unified_diff),
            EventMsg::DeprecationNotice(ev) => self.on_deprecation_notice(ev),
            EventMsg::BackgroundEvent(BackgroundEventEvent { message }) => {
                self.on_background_event(message)
            }
            EventMsg::UndoStarted(ev) => self.on_undo_started(ev),
            EventMsg::UndoCompleted(ev) => self.on_undo_completed(ev),
            EventMsg::StreamError(StreamErrorEvent { message }) => self.on_stream_error(message),
            EventMsg::UserMessage(ev) => {
                if from_replay {
                    self.on_user_message_event(ev);
                }
            }
            EventMsg::EnteredReviewMode(review_request) => {
                self.on_entered_review_mode(review_request)
            }
            EventMsg::ExitedReviewMode(review) => self.on_exited_review_mode(review),
            EventMsg::RawResponseItem(_)
            | EventMsg::ItemStarted(_)
            | EventMsg::ItemCompleted(_)
            | EventMsg::AgentMessageContentDelta(_)
            | EventMsg::ReasoningContentDelta(_)
            | EventMsg::ReasoningRawContentDelta(_) => {}
        }
    }

    fn on_entered_review_mode(&mut self, review: ReviewRequest) {
        // Enter review mode and emit a concise banner
        self.is_review_mode = true;
        let banner = format!(">> Code review started: {} <<", review.user_facing_hint);
        self.add_to_history(history_cell::new_review_status_line(banner));
        self.request_redraw();
    }

    fn on_exited_review_mode(&mut self, review: ExitedReviewModeEvent) {
        // Leave review mode; if output is present, flush pending stream + show results.
        if let Some(output) = review.review_output {
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
                    append_markdown(&explanation, None, &mut rendered);
                    let body_cell = AgentMessageCell::new(rendered, false);
                    self.app_event_tx
                        .send(AppEvent::InsertHistoryCell(Box::new(body_cell)));
                }
            } else {
                let message_text =
                    codex_core::review_format::format_review_findings_block(&output.findings, None);
                let mut message_lines: Vec<ratatui::text::Line<'static>> = Vec::new();
                append_markdown(&message_text, None, &mut message_lines);
                let body_cell = AgentMessageCell::new(message_lines, true);
                self.app_event_tx
                    .send(AppEvent::InsertHistoryCell(Box::new(body_cell)));
            }
        }

        self.is_review_mode = false;
        // Append a finishing banner at the end of this turn.
        self.add_to_history(history_cell::new_review_status_line(
            "<< Code review finished >>".to_string(),
        ));
        self.request_redraw();
    }

    fn on_user_message_event(&mut self, event: UserMessageEvent) {
        let message = event.message.trim();
        if !message.is_empty() {
            self.add_to_history(history_cell::new_user_prompt(message.to_string()));
        }
    }

    fn request_exit(&self) {
        self.app_event_tx.send(AppEvent::ExitRequest);
    }

    fn request_redraw(&mut self) {
        self.frame_requester.schedule_frame();
    }

    fn notify(&mut self, notification: Notification) {
        if !notification.allowed_for(&self.config.tui_notifications) {
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
    fn maybe_send_next_queued_input(&mut self) {
        if self.bottom_pane.is_task_running() {
            return;
        }
        if let Some(user_message) = self.queued_user_messages.pop_front() {
            self.submit_user_message(user_message);
        }
        // Update the list to reflect the remaining queued messages (if any).
        self.refresh_queued_user_messages();
    }

    /// Rebuild and update the queued user messages from the current queue.
    fn refresh_queued_user_messages(&mut self) {
        let messages: Vec<String> = self
            .queued_user_messages
            .iter()
            .map(|m| m.text.clone())
            .collect();
        self.bottom_pane.set_queued_user_messages(messages);
    }

    pub(crate) fn add_diff_in_progress(&mut self) {
        self.request_redraw();
    }

    pub(crate) fn on_diff_complete(&mut self) {
        self.request_redraw();
    }

    pub(crate) fn add_status_output(&mut self) {
        let default_usage = TokenUsage::default();
        let (total_usage, context_usage) = if let Some(ti) = &self.token_info {
            (&ti.total_token_usage, Some(&ti.last_token_usage))
        } else {
            (&default_usage, Some(&default_usage))
        };
        self.add_to_history(crate::status::new_status_output(
            &self.config,
            self.auth_manager.as_ref(),
            total_usage,
            context_usage,
            &self.conversation_id,
            self.rate_limit_snapshot.as_ref(),
            Local::now(),
        ));
    }

    fn lower_cost_preset(&self) -> Option<ModelPreset> {
        let auth_mode = self.auth_manager.auth().map(|auth| auth.mode);
        builtin_model_presets(auth_mode)
            .into_iter()
            .find(|preset| preset.model == NUDGE_MODEL_SLUG)
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
        let switch_model = preset.model.to_string();
        let display_name = preset.display_name.to_string();
        let default_effort: ReasoningEffortConfig = preset.default_reasoning_effort;

        let switch_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::CodexOp(Op::OverrideTurnContext {
                cwd: None,
                approval_policy: None,
                sandbox_policy: None,
                model: Some(switch_model.clone()),
                effort: Some(Some(default_effort)),
                summary: None,
            }));
            tx.send(AppEvent::UpdateModel(switch_model.clone()));
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
            Some(preset.description.to_string())
        };

        let items = vec![
            SelectionItem {
                name: format!("Switch to {display_name}"),
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
            subtitle: Some(format!("Switch to {display_name} for lower credit usage?")),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    /// Open a popup to choose the model (stage 1). After selecting a model,
    /// a second popup is shown to choose the reasoning effort.
    pub(crate) fn open_model_popup(&mut self) {
        let current_model = self.config.model.clone();
        let auth_mode = self.auth_manager.auth().map(|auth| auth.mode);
        let presets: Vec<ModelPreset> = builtin_model_presets(auth_mode);

        let mut items: Vec<SelectionItem> = Vec::new();
        for preset in presets.into_iter() {
            let description = if preset.description.is_empty() {
                None
            } else {
                Some(preset.description.to_string())
            };
            let is_current = preset.model == current_model;
            let preset_for_action = preset;
            let single_supported_effort = preset_for_action.supported_reasoning_efforts.len() == 1;
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenReasoningPopup {
                    model: preset_for_action,
                });
            })];
            items.push(SelectionItem {
                name: preset.display_name.to_string(),
                description,
                is_current,
                actions,
                dismiss_on_select: single_supported_effort,
                ..Default::default()
            });
        }

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Select Model and Effort".to_string()),
            subtitle: Some("Switch the model for this and future Codex CLI sessions".to_string()),
            footer_hint: Some("Press enter to select reasoning effort, or esc to dismiss.".into()),
            items,
            ..Default::default()
        });
    }

    /// Open a popup to choose the reasoning effort (stage 2) for the given model.
    pub(crate) fn open_reasoning_popup(&mut self, preset: ModelPreset) {
        let default_effort: ReasoningEffortConfig = preset.default_reasoning_effort;
        let supported = preset.supported_reasoning_efforts;

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
            if let Some(effort) = choices.first().and_then(|c| c.stored) {
                self.apply_model_and_effort(preset.model.to_string(), Some(effort));
            } else {
                self.apply_model_and_effort(preset.model.to_string(), None);
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
        let is_current_model = self.config.model == preset.model;
        let highlight_choice = if is_current_model {
            self.config.model_reasoning_effort
        } else {
            default_choice
        };
        let mut items: Vec<SelectionItem> = Vec::new();
        for choice in choices.iter() {
            let effort = choice.display;
            let mut effort_label = effort.to_string();
            if let Some(first) = effort_label.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
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

            let warning = "⚠ High reasoning effort can quickly consume Plus plan rate limits.";
            let show_warning =
                preset.model.starts_with("gpt-5-codex") && effort == ReasoningEffortConfig::High;
            let selected_description = show_warning.then(|| {
                description
                    .as_ref()
                    .map_or(warning.to_string(), |d| format!("{d}\n{warning}"))
            });

            let model_for_action = model_slug.clone();
            let effort_for_action = choice.stored;
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::CodexOp(Op::OverrideTurnContext {
                    cwd: None,
                    approval_policy: None,
                    sandbox_policy: None,
                    model: Some(model_for_action.clone()),
                    effort: Some(effort_for_action),
                    summary: None,
                }));
                tx.send(AppEvent::UpdateModel(model_for_action.clone()));
                tx.send(AppEvent::UpdateReasoningEffort(effort_for_action));
                tx.send(AppEvent::PersistModelSelection {
                    model: model_for_action.clone(),
                    effort: effort_for_action,
                });
                tracing::info!(
                    "Selected model: {}, Selected effort: {}",
                    model_for_action,
                    effort_for_action
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "default".to_string())
                );
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
            ..Default::default()
        });
    }

    fn apply_model_and_effort(&self, model: String, effort: Option<ReasoningEffortConfig>) {
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::OverrideTurnContext {
                cwd: None,
                approval_policy: None,
                sandbox_policy: None,
                model: Some(model.clone()),
                effort: Some(effort),
                summary: None,
            }));
        self.app_event_tx.send(AppEvent::UpdateModel(model.clone()));
        self.app_event_tx
            .send(AppEvent::UpdateReasoningEffort(effort));
        self.app_event_tx.send(AppEvent::PersistModelSelection {
            model: model.clone(),
            effort,
        });
        tracing::info!(
            "Selected model: {}, Selected effort: {}",
            model,
            effort
                .map(|e| e.to_string())
                .unwrap_or_else(|| "default".to_string())
        );
    }

    /// Open a popup to choose the approvals mode (ask for approval policy + sandbox policy).
    pub(crate) fn open_approvals_popup(&mut self) {
        let current_approval = self.config.approval_policy;
        let current_sandbox = self.config.sandbox_policy.clone();
        let mut items: Vec<SelectionItem> = Vec::new();
        let presets: Vec<ApprovalPreset> = builtin_approval_presets();
        #[cfg(target_os = "windows")]
        let header_renderable: Box<dyn Renderable> = if self
            .config
            .forced_auto_mode_downgraded_on_windows
        {
            use ratatui_macros::line;

            let mut header = ColumnRenderable::new();
            header.push(line![
                "Codex forced your settings back to Read Only on this Windows machine.".bold()
            ]);
            header.push(line![
                "To re-enable Auto mode, run Codex inside Windows Subsystem for Linux (WSL) or enable Full Access manually.".dim()
                ]);
            Box::new(header)
        } else {
            Box::new(())
        };
        #[cfg(not(target_os = "windows"))]
        let header_renderable: Box<dyn Renderable> = Box::new(());
        for preset in presets.into_iter() {
            let is_current =
                current_approval == preset.approval && current_sandbox == preset.sandbox;
            let name = preset.label.to_string();
            let description_text = preset.description;
            let description = if cfg!(target_os = "windows")
                && preset.id == "auto"
                && codex_core::get_platform_sandbox().is_none()
            {
                Some(format!(
                    "{description_text}\nRequires Windows Subsystem for Linux (WSL). Show installation instructions..."
                ))
            } else {
                Some(description_text.to_string())
            };
            let requires_confirmation = preset.id == "full-access"
                && !self
                    .config
                    .notices
                    .hide_full_access_warning
                    .unwrap_or(false);
            let actions: Vec<SelectionAction> = if requires_confirmation {
                let preset_clone = preset.clone();
                vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenFullAccessConfirmation {
                        preset: preset_clone.clone(),
                    });
                })]
            } else if preset.id == "auto" {
                #[cfg(target_os = "windows")]
                {
                    if codex_core::get_platform_sandbox().is_none() {
                        vec![Box::new(|tx| {
                            tx.send(AppEvent::ShowWindowsAutoModeInstructions);
                        })]
                    } else if !self
                        .config
                        .notices
                        .hide_world_writable_warning
                        .unwrap_or(false)
                        && self.windows_world_writable_flagged()
                    {
                        let preset_clone = preset.clone();
                        // Compute sample paths for the warning popup.
                        let mut env_map: std::collections::HashMap<String, String> =
                            std::collections::HashMap::new();
                        for (k, v) in std::env::vars() {
                            env_map.insert(k, v);
                        }
                        let (sample_paths, extra_count, failed_scan) =
                            match codex_windows_sandbox::preflight_audit_everyone_writable(
                                &self.config.cwd,
                                &env_map,
                                Some(self.config.codex_home.as_path()),
                            ) {
                                Ok(paths) if !paths.is_empty() => {
                                    fn normalize_windows_path_for_display(
                                        p: &std::path::Path,
                                    ) -> String {
                                        let canon = dunce::canonicalize(p)
                                            .unwrap_or_else(|_| p.to_path_buf());
                                        canon.display().to_string().replace('/', "\\")
                                    }
                                    let as_strings: Vec<String> = paths
                                        .iter()
                                        .map(|p| normalize_windows_path_for_display(p))
                                        .collect();
                                    let samples: Vec<String> =
                                        as_strings.iter().take(3).cloned().collect();
                                    let extra = if as_strings.len() > samples.len() {
                                        as_strings.len() - samples.len()
                                    } else {
                                        0
                                    };
                                    (samples, extra, false)
                                }
                                Err(_) => (Vec::new(), 0, true),
                                _ => (Vec::new(), 0, false),
                            };
                        vec![Box::new(move |tx| {
                            tx.send(AppEvent::OpenWorldWritableWarningConfirmation {
                                preset: Some(preset_clone.clone()),
                                sample_paths: sample_paths.clone(),
                                extra_count,
                                failed_scan,
                            });
                        })]
                    } else {
                        Self::approval_preset_actions(preset.approval, preset.sandbox.clone())
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Self::approval_preset_actions(preset.approval, preset.sandbox.clone())
                }
            } else {
                Self::approval_preset_actions(preset.approval, preset.sandbox.clone())
            };
            items.push(SelectionItem {
                name,
                description,
                is_current,
                actions,
                dismiss_on_select: true,
                ..Default::default()
            });
        }

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Select Approval Mode".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: header_renderable,
            ..Default::default()
        });
    }

    fn approval_preset_actions(
        approval: AskForApproval,
        sandbox: SandboxPolicy,
    ) -> Vec<SelectionAction> {
        vec![Box::new(move |tx| {
            let sandbox_clone = sandbox.clone();
            tx.send(AppEvent::CodexOp(Op::OverrideTurnContext {
                cwd: None,
                approval_policy: Some(approval),
                sandbox_policy: Some(sandbox_clone.clone()),
                model: None,
                effort: None,
                summary: None,
            }));
            tx.send(AppEvent::UpdateAskForApprovalPolicy(approval));
            tx.send(AppEvent::UpdateSandboxPolicy(sandbox_clone));
        })]
    }

    #[cfg(target_os = "windows")]
    fn windows_world_writable_flagged(&self) -> bool {
        use std::collections::HashMap;
        let mut env_map: HashMap<String, String> = HashMap::new();
        for (k, v) in std::env::vars() {
            env_map.insert(k, v);
        }
        match codex_windows_sandbox::preflight_audit_everyone_writable(
            &self.config.cwd,
            &env_map,
            Some(self.config.codex_home.as_path()),
        ) {
            Ok(paths) => !paths.is_empty(),
            Err(_) => true,
        }
    }

    pub(crate) fn open_full_access_confirmation(&mut self, preset: ApprovalPreset) {
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

        let mut accept_actions = Self::approval_preset_actions(approval, sandbox.clone());
        accept_actions.push(Box::new(|tx| {
            tx.send(AppEvent::UpdateFullAccessWarningAcknowledged(true));
        }));

        let mut accept_and_remember_actions = Self::approval_preset_actions(approval, sandbox);
        accept_and_remember_actions.push(Box::new(|tx| {
            tx.send(AppEvent::UpdateFullAccessWarningAcknowledged(true));
            tx.send(AppEvent::PersistFullAccessWarningAcknowledged);
        }));

        let deny_actions: Vec<SelectionAction> = vec![Box::new(|tx| {
            tx.send(AppEvent::OpenApprovalsPopup);
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
        let mode_label = match self.config.sandbox_policy {
            SandboxPolicy::WorkspaceWrite { .. } => "Auto mode",
            SandboxPolicy::ReadOnly => "Read-Only mode",
            _ => "Auto mode",
        };
        let title_line = Line::from("Unprotected directories found").bold();
        let info_line = if failed_scan {
            Line::from(vec![
                "We couldn't complete the world-writable scan, so protections cannot be verified. "
                    .into(),
                format!("The Windows sandbox cannot guarantee protection in {mode_label}.")
                    .fg(Color::Red),
            ])
        } else {
            Line::from(vec![
                "Some important directories on this system are world-writable. ".into(),
                format!(
                    "The Windows sandbox cannot protect writes to these locations in {mode_label}."
                )
                .fg(Color::Red),
            ])
        };
        header_children.push(Box::new(title_line));
        header_children.push(Box::new(
            Paragraph::new(vec![info_line]).wrap(Wrap { trim: false }),
        ));

        if !sample_paths.is_empty() {
            // Show up to three examples and optionally an "and X more" line.
            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from("Examples:").bold());
            for p in &sample_paths {
                lines.push(Line::from(format!(" - {p}")));
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
        // Suppress the immediate re-scan only when a preset will be applied (i.e., via /approvals),
        // to avoid duplicate warnings from the ensuing policy change.
        if preset.is_some() {
            accept_actions.push(Box::new(|tx| {
                tx.send(AppEvent::SkipNextWorldWritableScan);
            }));
        }
        if let (Some(approval), Some(sandbox)) = (approval, sandbox.clone()) {
            accept_actions.extend(Self::approval_preset_actions(approval, sandbox));
        }

        let mut accept_and_remember_actions: Vec<SelectionAction> = Vec::new();
        accept_and_remember_actions.push(Box::new(|tx| {
            tx.send(AppEvent::UpdateWorldWritableWarningAcknowledged(true));
            tx.send(AppEvent::PersistWorldWritableWarningAcknowledged);
        }));
        if let (Some(approval), Some(sandbox)) = (approval, sandbox) {
            accept_and_remember_actions.extend(Self::approval_preset_actions(approval, sandbox));
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
    pub(crate) fn open_windows_auto_mode_instructions(&mut self) {
        use ratatui_macros::line;

        let mut header = ColumnRenderable::new();
        header.push(line![
            "Auto mode requires Windows Subsystem for Linux (WSL2).".bold()
        ]);
        header.push(line!["Run Codex inside WSL to enable sandboxed commands."]);
        header.push(line![""]);
        header.push(Paragraph::new(WSL_INSTRUCTIONS).wrap(Wrap { trim: false }));

        let items = vec![SelectionItem {
            name: "Back".to_string(),
            description: Some(
                "Return to the approval mode list. Auto mode stays disabled outside WSL."
                    .to_string(),
            ),
            actions: vec![Box::new(|tx| {
                tx.send(AppEvent::OpenApprovalsPopup);
            })],
            dismiss_on_select: true,
            ..Default::default()
        }];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: None,
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(header),
            ..Default::default()
        });
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn open_windows_auto_mode_instructions(&mut self) {}

    /// Set the approval policy in the widget's config copy.
    pub(crate) fn set_approval_policy(&mut self, policy: AskForApproval) {
        self.config.approval_policy = policy;
    }

    /// Set the sandbox policy in the widget's config copy.
    pub(crate) fn set_sandbox_policy(&mut self, policy: SandboxPolicy) {
        self.config.sandbox_policy = policy;
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

    /// Set the reasoning effort in the widget's config copy.
    pub(crate) fn set_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        self.config.model_reasoning_effort = effort;
    }

    /// Set the model in the widget's config copy.
    pub(crate) fn set_model(&mut self, model: &str) {
        self.session_header.set_model(model);
        self.config.model = model.to_string();
    }

    fn clear_security_review_follow_up(&mut self) {
        if self.security_review_follow_up.take().is_some() {
            self.bottom_pane
                .set_placeholder_text(self.default_placeholder.clone());
        }
    }

    fn security_review_follow_up_prompt(&self, text: &str) -> Option<String> {
        let state = self.security_review_follow_up.as_ref()?;
        if text.starts_with(SECURITY_REVIEW_FOLLOW_UP_MARKER) {
            return None;
        }
        let question = text.trim();
        if question.is_empty() {
            return None;
        }
        Some(build_follow_up_user_prompt(
            state.mode,
            &state.scope_paths,
            state.follow_up_path.as_path(),
            &state.repo_root,
            state.follow_up_label.as_str(),
            question,
        ))
    }

    pub(crate) fn add_info_message(&mut self, message: String, hint: Option<String>) {
        self.add_to_history(history_cell::new_info_event(message, hint));
        self.request_redraw();
    }

    pub(crate) fn add_error_message(&mut self, message: String) {
        self.add_to_history(history_cell::new_error_event(message));
        self.request_redraw();
    }

    pub(crate) fn add_mcp_output(&mut self) {
        if self.config.mcp_servers.is_empty() {
            self.add_to_history(history_cell::empty_mcp_output());
        } else {
            self.submit_op(Op::ListMcpTools);
        }
    }

    /// Forward file-search results to the bottom pane.
    pub(crate) fn apply_file_search_result(&mut self, query: String, matches: Vec<FileMatch>) {
        self.bottom_pane.on_file_search_result(query, matches);
    }

    /// Handle Ctrl-C key press.
    fn on_ctrl_c(&mut self) {
        if self.bottom_pane.on_ctrl_c() == CancellationEvent::Handled {
            return;
        }

        if self.bottom_pane.is_task_running() {
            self.bottom_pane.show_ctrl_c_quit_hint();
            if !self.cancel_security_review() {
                self.submit_op(Op::Interrupt);
            }
            return;
        }

        self.submit_op(Op::Shutdown);
    }

    fn cancel_security_review(&mut self) -> bool {
        if let Some(handle) = self.security_review_task.take() {
            handle.abort();
            self.bottom_pane.set_task_running(false);
            self.bottom_pane
                .update_status_header(String::from("Working"));
            self.security_review_context = None;
            self.clear_security_review_follow_up();
            self.add_info_message("Security review cancelled.".to_string(), None);
            return true;
        }
        false
    }

    pub(crate) fn composer_is_empty(&self) -> bool {
        self.bottom_pane.composer_is_empty()
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
    pub(crate) fn set_composer_text(&mut self, text: String) {
        self.bottom_pane.set_composer_text(text);
    }

    pub(crate) fn show_esc_backtrack_hint(&mut self) {
        self.bottom_pane.show_esc_backtrack_hint();
    }

    pub(crate) fn clear_esc_backtrack_hint(&mut self) {
        self.bottom_pane.clear_esc_backtrack_hint();
    }
    /// Forward an `Op` directly to codex.
    pub(crate) fn submit_op(&self, op: Op) {
        // Record outbound operation for session replay fidelity.
        crate::session_log::log_outbound_op(&op);
        if let Err(e) = self.codex_op_tx.send(op) {
            tracing::error!("failed to submit op: {e}");
        }
    }

    fn on_list_mcp_tools(&mut self, ev: McpListToolsResponseEvent) {
        self.add_to_history(history_cell::new_mcp_tools_output(
            &self.config,
            ev.tools,
            ev.resources,
            ev.resource_templates,
            &ev.auth_statuses,
        ));
    }

    fn on_list_custom_prompts(&mut self, ev: ListCustomPromptsResponseEvent) {
        let len = ev.custom_prompts.len();
        debug!("received {len} custom prompts");
        // Forward to bottom pane so the slash popup can show them now.
        self.bottom_pane.set_custom_prompts(ev.custom_prompts);
    }

    pub(crate) fn open_review_popup(&mut self) {
        let mut items: Vec<SelectionItem> = Vec::new();

        items.push(SelectionItem {
            name: "Review against a base branch".to_string(),
            description: Some("(PR Style)".into()),
            actions: vec![Box::new({
                let cwd = self.config.cwd.clone();
                move |tx| {
                    tx.send(AppEvent::OpenReviewBranchPicker(cwd.clone()));
                }
            })],
            dismiss_on_select: false,
            ..Default::default()
        });

        items.push(SelectionItem {
            name: "Review uncommitted changes".to_string(),
            actions: vec![Box::new(
                move |tx: &AppEventSender| {
                    tx.send(AppEvent::CodexOp(Op::Review {
                        review_request: ReviewRequest {
                            prompt: "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings.".to_string(),
                            user_facing_hint: "current changes".to_string(),
                        },
                    }));
                },
            )],
            dismiss_on_select: true,
            ..Default::default()
        });

        // New: Review a specific commit (opens commit picker)
        items.push(SelectionItem {
            name: "Review a commit".to_string(),
            actions: vec![Box::new({
                let cwd = self.config.cwd.clone();
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

    pub(crate) fn open_security_review_popup(&mut self) {
        let mut items: Vec<SelectionItem> = Vec::new();

        items.push(SelectionItem {
            name: "Full security review".to_string(),
            description: Some("(bugs + full report)".into()),
            actions: vec![Box::new(|tx: &AppEventSender| {
                tx.send(AppEvent::StartSecurityReview {
                    mode: SecurityReviewMode::Full,
                    include_paths: Vec::new(),
                    scope_prompt: None,
                    force_new: false,
                });
            })],
            dismiss_on_select: true,
            ..Default::default()
        });

        items.push(SelectionItem {
            name: "Quick bug sweep".to_string(),
            description: Some("(bugs only)".into()),
            actions: vec![Box::new(|tx: &AppEventSender| {
                tx.send(AppEvent::StartSecurityReview {
                    mode: SecurityReviewMode::Bugs,
                    include_paths: Vec::new(),
                    scope_prompt: None,
                    force_new: false,
                });
            })],
            dismiss_on_select: true,
            ..Default::default()
        });

        items.push(SelectionItem {
            name: "Full review for specific paths".to_string(),
            description: Some("Enter relative paths to scope the run".into()),
            actions: vec![Box::new(|tx: &AppEventSender| {
                tx.send(AppEvent::OpenSecurityReviewPathPrompt(
                    SecurityReviewMode::Full,
                ));
            })],
            dismiss_on_select: true,
            ..Default::default()
        });

        items.push(SelectionItem {
            name: "Bug sweep for specific paths".to_string(),
            actions: vec![Box::new(|tx: &AppEventSender| {
                tx.send(AppEvent::OpenSecurityReviewPathPrompt(
                    SecurityReviewMode::Bugs,
                ));
            })],
            dismiss_on_select: true,
            ..Default::default()
        });

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Security review options".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    pub(crate) fn start_security_review(
        &mut self,
        mode: SecurityReviewMode,
        include_paths: Vec<String>,
        scope_prompt: Option<String>,
        force_new: bool,
    ) {
        if self.bottom_pane.is_task_running() || self.security_review_context.is_some() {
            self.add_error_message(
                "A security review is already running. Wait for it to finish before starting another."
                    .to_string(),
            );
            return;
        }

        let repo_path = self.config.cwd.clone();
        if !repo_path.exists() {
            self.add_error_message(format!(
                "Repository path {} does not exist.",
                repo_path.display()
            ));
            return;
        }

        let storage_root = security_review_storage_root(&repo_path);
        if !force_new && let Some(candidate) = latest_security_review_candidate(&storage_root) {
            self.prompt_security_review_resume(mode, include_paths, scope_prompt, candidate);
            return;
        }

        self.clear_security_review_follow_up();

        let mut resolved_paths: Vec<PathBuf> = Vec::new();
        let mut display_paths: Vec<String> = Vec::new();

        if !include_paths.is_empty() {
            for raw in include_paths {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let candidate = if Path::new(trimmed).is_absolute() {
                    PathBuf::from(trimmed)
                } else {
                    repo_path.join(trimmed)
                };

                if !candidate.exists() {
                    self.add_error_message(format!(
                        "Path `{}` was not found within {}.",
                        trimmed,
                        repo_path.display()
                    ));
                    return;
                }

                let canonical = candidate.canonicalize().unwrap_or(candidate.clone());
                resolved_paths.push(canonical.clone());
                display_paths.push(display_path_for(&canonical, &repo_path));
            }

            if resolved_paths.is_empty() {
                self.add_error_message(
                    "No valid paths were provided for the security review.".to_string(),
                );
                return;
            }
        }

        let skip_auto_scope_confirmation = false;

        let context_paths = display_paths.clone();
        // Do not echo the auto-scope prompt into the scope list; keep the header concise.

        if let Err(err) = fs::create_dir_all(&storage_root) {
            self.add_error_message(format!(
                "Failed to prepare security review output directory {}: {err}",
                storage_root.display()
            ));
            return;
        }

        let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let mut output_root = storage_root.join(&timestamp);
        let mut collision_counter: usize = 1;
        while output_root.exists() {
            let candidate = format!("{timestamp}-{collision_counter:02}");
            output_root = storage_root.join(candidate);
            collision_counter = collision_counter.saturating_add(1);
        }
        if let Err(err) = fs::create_dir_all(&output_root) {
            self.add_error_message(format!(
                "Failed to create security review output directory {}: {err}",
                output_root.display()
            ));
            return;
        }

        self.bottom_pane.set_task_running(true);
        self.bottom_pane
            .update_status_header(format!("Security review ({}) — preparing", mode.as_str()));

        let scope_text = if context_paths.is_empty() {
            "entire repository".to_string()
        } else {
            context_paths.join(", ")
        };

        self.add_info_message(
            if context_paths.is_empty() {
                format!(">> Security review started (mode: {}) <<", mode.as_str())
            } else {
                format!(
                    ">> Security review started (mode: {}, scope: {}) <<",
                    mode.as_str(),
                    scope_text
                )
            },
            None,
        );

        self.security_review_context = Some(SecurityReviewContext {
            mode,
            include_paths: context_paths.clone(),
            output_root: output_root.clone(),
            repo_path: repo_path.clone(),
            model: self.config.model.clone(),
            provider_name: self.config.model_provider.name.clone(),
            started_at: Instant::now(),
            last_log: None,
            thinking_lines: Vec::new(),
            log_lines: Vec::new(),
            progress_percent: Some(0),
        });

        // Enable auto-scope when a scope prompt is provided (options 3 and 4).
        // We annotate the prompt for both Full and Bugs modes.
        let annotated_scope_prompt = scope_prompt
            .as_ref()
            .map(|prompt| annotate_scope_prompt(prompt.as_str()));

        let request = SecurityReviewRequest {
            repo_path,
            include_paths: resolved_paths,
            scope_display_paths: context_paths,
            output_root,
            mode,
            include_spec_in_bug_analysis: true,
            triage_model: self.config.review_model.clone(),
            model: self.config.model.clone(),
            provider: self.config.model_provider.clone(),
            auth: self.auth_manager.auth(),
            progress_sender: Some(self.app_event_tx.clone()),
            skip_auto_scope_confirmation,
            auto_scope_prompt: annotated_scope_prompt,
        };

        let tx = self.app_event_tx.clone();
        let handle = tokio::spawn(async move {
            match run_security_review(request).await {
                Ok(result) => {
                    tx.send(AppEvent::SecurityReviewComplete { result });
                }
                Err(error) => {
                    tx.send(AppEvent::SecurityReviewFailed { error });
                }
            }
        });
        self.security_review_task = Some(handle);
    }

    pub(crate) fn show_security_review_path_prompt(&mut self, mode: SecurityReviewMode) {
        let tx = self.app_event_tx.clone();
        let repo_root = self.config.cwd.clone();
        let title = match mode {
            SecurityReviewMode::Full => "Full security review (scoped)".to_string(),
            SecurityReviewMode::Bugs => "Bug sweep (scoped)".to_string(),
        };
        let hint = match mode {
            SecurityReviewMode::Full => "Type relative paths (space-separated) to include, then press Enter to run the full review".to_string(),
            SecurityReviewMode::Bugs => "Type relative paths (space-separated) to include, then press Enter to run the bug sweep".to_string(),
        };
        let view = CustomPromptView::new(
            title,
            hint,
            None,
            Box::new(move |input: String| {
                let trimmed = input.trim();
                let (include_paths, scope_prompt_override): (Vec<String>, Option<String>) =
                    if trimmed.is_empty() {
                        let prompt = match mode {
                            SecurityReviewMode::Full => "No user scope provided. Choose the 3-8 directories that best represent the production attack surface (core services, externally exposed APIs, authz/authn flows, critical infrastructure). Skip tests, vendor archives, docs, and generated code.".to_string(),
                            SecurityReviewMode::Bugs => "No user scope provided. Pick the smallest set of directories most likely to contain critical or high-risk code paths (externally reachable services, request parsing, auth, secret handling). Ignore tests, vendor archives, docs, and generated code.".to_string(),
                        };
                        (Vec::new(), Some(prompt))
                    } else {
                        let mut collected: Vec<String> = Vec::new();
                        let mut all_valid = true;

                        for segment in trimmed.split_whitespace() {
                            let candidate = if Path::new(segment).is_absolute() {
                                PathBuf::from(segment)
                            } else {
                                repo_root.join(segment)
                            };
                            if candidate.exists() {
                                collected.push(segment.to_string());
                            } else {
                                all_valid = false;
                                break;
                            }
                        }

                        if all_valid && !collected.is_empty() {
                            (collected, None)
                        } else {
                            (Vec::new(), Some(trimmed.to_string()))
                        }
                    };

                tx.send(AppEvent::StartSecurityReview {
                    mode,
                    include_paths,
                    scope_prompt: scope_prompt_override,
                    force_new: false,
                });
            }),
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    pub(crate) fn show_security_review_scope_confirmation(
        &mut self,
        mode: SecurityReviewMode,
        prompt: String,
        selections: Vec<SecurityReviewAutoScopeSelection>,
        responder: oneshot::Sender<bool>,
    ) {
        let view = SecurityReviewScopeConfirmView::new(mode, prompt, selections, responder);
        self.bottom_pane.show_view(Box::new(view));
    }

    pub(crate) fn on_security_review_scope_resolved(&mut self, paths: Vec<String>) {
        if let Some(ctx) = self.security_review_context.as_mut() {
            ctx.include_paths = if paths.is_empty() { Vec::new() } else { paths };
        }
    }

    pub(crate) fn show_registration_prompt(
        &mut self,
        url: Option<String>,
        responder: tokio::sync::oneshot::Sender<Option<String>>,
    ) {
        if let Some(link) = url.as_ref() {
            // Try to open the URL in the default browser.
            let link_clone = link.clone();
            tokio::spawn(async move {
                #[cfg(target_os = "macos")]
                {
                    let _ = tokio::process::Command::new("open")
                        .arg(&link_clone)
                        .status()
                        .await;
                }
                #[cfg(all(unix, not(target_os = "macos")))]
                {
                    let _ = tokio::process::Command::new("xdg-open")
                        .arg(&link_clone)
                        .status()
                        .await;
                }
                #[cfg(target_os = "windows")]
                {
                    let _ = tokio::process::Command::new("rundll32.exe")
                        .arg("url.dll,FileProtocolHandler")
                        .arg(&link_clone)
                        .status()
                        .await;
                }
            });
        }

        let hint = if let Some(link) = url.as_ref() {
            format!(
                "We attempted to auto-register accounts but failed. A login/signup page was opened: {link}\n\nRegister at least two test accounts, then paste credentials as `user:pass, user2:pass2` and press Enter."
            )
        } else {
            "We attempted to auto-register accounts but failed. Register at least two test accounts, then paste credentials as `user:pass, user2:pass2` and press Enter.".to_string()
        };

        use std::sync::Arc;
        use std::sync::Mutex;
        let responder_cell = Arc::new(Mutex::new(Some(responder)));
        let responder_cell_clone = responder_cell;
        let view = CustomPromptView::new(
            "Register test accounts".to_string(),
            hint,
            None,
            Box::new(move |input: String| {
                if let Some(tx) = responder_cell_clone.lock().ok().and_then(|mut g| g.take()) {
                    let _ = tx.send(Some(input));
                }
            }),
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    pub(crate) fn on_security_review_log(&mut self, message: String) {
        if let Some(ctx) = self.security_review_context.as_mut() {
            // Drop overly verbose heartbeat for bug analysis; header already shows progress.
            if message.starts_with("Still waiting for bug analysis response from model") {
                return;
            }
            let previous_percent = ctx.progress_percent;
            // Extract trailing percent in the form " - NN%" and move it to the front.
            // Enhance with a small 10-slot progress bar.
            let mut percent_prefix = String::new();
            let mut core = message.as_str();
            let progress_changed;
            if let Some((percent, trimmed_core)) = parse_progress_suffix(message.as_str()) {
                ctx.progress_percent = Some(percent);
                percent_prefix = build_percent_prefix(percent);
                core = trimmed_core;
                progress_changed = previous_percent != Some(percent);
            } else {
                ctx.progress_percent = None;
                progress_changed = previous_percent.is_some();
            }

            let mut added_to_log = false;
            if !message.starts_with("Model reasoning:") {
                let trimmed_message = strip_progress_prefix(message.as_str()).trim();
                let is_explicit_progress = trimmed_message.starts_with("File triage progress:")
                    || trimmed_message.starts_with("Bug analysis progress:");
                if !trimmed_message.is_empty() && !is_explicit_progress {
                    ctx.log_lines.push(truncate_text(trimmed_message, 160));
                    if ctx.log_lines.len() > 5 {
                        let excess = ctx.log_lines.len() - 5;
                        ctx.log_lines.drain(0..excess);
                    }
                    added_to_log = true;
                }
            }

            ctx.last_log = Some(message.clone());
            // Compact known progress messages: show counts succinctly.
            let mut display_core = strip_progress_prefix(core).trim();
            if let Some(rest) = display_core
                .strip_prefix("File triage progress:")
                .or_else(|| display_core.strip_prefix("Bug analysis progress:"))
            {
                let tail = rest.trim();
                if let Some(slash_pos) = tail.find('/') {
                    // Keep "N/M" and append " files" for clarity.
                    let (a, b) = tail.split_at(slash_pos);
                    let _ = a.trim();
                    let _ = b.trim();
                    // Use the original tail (N/M) if it looks correct.
                    if tail.chars().any(|c| c == '/') {
                        display_core = Box::leak(format!("{tail} files").into_boxed_str());
                    }
                }
            }

            let truncated = truncate_text(display_core, 96);
            let header = format!(
                "Security review ({}) - {}{}",
                ctx.mode.as_str(),
                percent_prefix,
                truncated
            );

            if message.starts_with("Model reasoning:") {
                let reason = message.trim_start_matches("Model reasoning:").trim();
                let line = truncate_text(reason, 160);
                ctx.thinking_lines.push(line);
                if ctx.thinking_lines.len() > 4 {
                    let start = ctx.thinking_lines.len() - 4;
                    ctx.thinking_lines = ctx.thinking_lines.split_off(start);
                }
                self.bottom_pane.update_status_snapshot(
                    crate::status_indicator_widget::StatusSnapshot {
                        header,
                        progress: None,
                        thinking: ctx.thinking_lines.clone(),
                        tool_calls: Vec::new(),
                        logs: ctx.log_lines.clone(),
                    },
                );
            } else {
                self.bottom_pane.update_status_logs(ctx.log_lines.clone());
                if !added_to_log || !percent_prefix.is_empty() || progress_changed {
                    self.bottom_pane.update_status_header(header);
                }
            }
        }
    }

    pub(crate) fn on_security_review_command_status(
        &mut self,
        _id: u64,
        summary: String,
        state: SecurityReviewCommandState,
        preview: Vec<String>,
    ) {
        if let Some(ctx) = self.security_review_context.as_mut() {
            let state_label = match state {
                SecurityReviewCommandState::Running => "running",
                SecurityReviewCommandState::Matches => "matches",
                SecurityReviewCommandState::NoMatches => "no matches",
                SecurityReviewCommandState::Error => "error",
            };
            let mut header_text = summary.clone();
            if let Some(first) = preview.first().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                header_text = format!("{header_text} — {first}");
            }
            let truncated = truncate_text(&header_text, 96);
            // Also surface a sub-section under the status with the tool call and a few preview lines.
            let mut tool_calls: Vec<String> = Vec::new();
            tool_calls.push(format!("• {summary}"));
            for line in &preview {
                tool_calls.push(format!("  {line}"));
            }
            self.bottom_pane.update_status_snapshot(
                crate::status_indicator_widget::StatusSnapshot {
                    header: format!(
                        "Security review ({}) — [{state_label}] {truncated}",
                        ctx.mode.as_str()
                    ),
                    progress: None,
                    thinking: ctx.thinking_lines.clone(),
                    tool_calls,
                    logs: ctx.log_lines.clone(),
                },
            );
            ctx.last_log = Some(header_text);
        }
    }

    pub(crate) fn on_security_review_complete(&mut self, result: SecurityReviewResult) {
        self.bottom_pane.set_task_running(false);
        self.bottom_pane
            .update_status_header(String::from("Working"));
        self.security_review_task = None;

        // Merge security review token usage into the session total so the
        // CLI exit summary reflects it, matching regular Codex sessions.
        if !result.token_usage.is_zero() {
            if let Some(info) = self.token_info.as_mut() {
                info.append_last_usage(&result.token_usage);
            } else {
                self.token_info = codex_core::protocol::TokenUsageInfo::new_or_append(
                    &None,
                    &Some(result.token_usage.clone()),
                    self.config.model_context_window,
                );
            }
        }

        let context = self.security_review_context.take();
        let (mode, scope_paths, _output_root, repo_path, model, provider, started_at, last_log) =
            if let Some(ctx) = context {
                (
                    ctx.mode,
                    ctx.include_paths,
                    ctx.output_root,
                    ctx.repo_path,
                    ctx.model,
                    ctx.provider_name,
                    ctx.started_at,
                    ctx.last_log,
                )
            } else {
                (
                    SecurityReviewMode::Full,
                    Vec::new(),
                    PathBuf::new(),
                    self.config.cwd.clone(),
                    self.config.model.clone(),
                    self.config.model_provider.name.clone(),
                    Instant::now(),
                    None,
                )
            };

        let duration = started_at.elapsed();
        let duration_display = fmt_elapsed_compact(duration.as_secs());
        let scope_text = if scope_paths.is_empty() {
            "entire repository".to_string()
        } else {
            scope_paths.join(", ")
        };
        let findings_summary_text = result.findings_summary.clone();
        let bugs_display = display_path_for(&result.bugs_path, &repo_path);
        let report_markdown_display = result
            .report_path
            .as_ref()
            .map(|path| display_path_for(path, &repo_path));
        let report_html_display = result
            .report_html_path
            .as_ref()
            .map(|path| display_path_for(path, &repo_path));

        let mut summary_lines: Vec<Line<'static>> = Vec::new();
        summary_lines.push(vec!["• ".dim(), format!("Mode: {}", mode.as_str()).into()].into());
        summary_lines.push(vec!["  • ".into(), format!("Scope: {scope_text}").into()].into());
        summary_lines.push(
            vec![
                "  • ".into(),
                format!("Duration: {duration_display}").into(),
            ]
            .into(),
        );
        summary_lines.push(
            vec![
                "  • ".into(),
                format!("Model: {model} via {provider}").into(),
            ]
            .into(),
        );
        if !result.token_usage.is_zero() {
            let usage_line = format!(
                "{}",
                codex_core::protocol::FinalOutput::from(result.token_usage.clone())
            );
            summary_lines.push(vec!["  • ".into(), usage_line.into()].into());
        }
        summary_lines.push(
            vec![
                "  • ".into(),
                format!("Findings: {}", findings_summary_text.as_str()).into(),
            ]
            .into(),
        );
        let mut artifact_lines: Vec<Line<'static>> = Vec::new();
        if let Some(md) = report_markdown_display.as_ref() {
            artifact_lines
                .push(vec!["  • ".into(), format!("Report (markdown): {md}").into()].into());
        }
        if let Some(html) = report_html_display.as_ref() {
            artifact_lines
                .push(vec!["  • ".into(), format!("Report (html): {html}").into()].into());
        }
        if artifact_lines.is_empty() {
            artifact_lines.push(
                vec![
                    "  • ".into(),
                    format!("Bugs: {}", bugs_display.as_str()).into(),
                ]
                .into(),
            );
        }
        summary_lines.extend(artifact_lines);
        if let Some(api_path) = result
            .api_overview_path
            .as_ref()
            .map(|path| display_path_for(path, &repo_path))
        {
            summary_lines.push(
                vec![
                    "  • ".into(),
                    format!("API entry points: {}", api_path.as_str()).into(),
                ]
                .into(),
            );
        }
        if let Some(class_json) = result
            .classification_json_path
            .as_ref()
            .map(|path| display_path_for(path, &repo_path))
        {
            summary_lines.push(
                vec![
                    "  • ".into(),
                    format!("Data classification (JSONL): {}", class_json.as_str()).into(),
                ]
                .into(),
            );
        }
        if let Some(class_table) = result
            .classification_table_path
            .as_ref()
            .map(|path| display_path_for(path, &repo_path))
        {
            summary_lines.push(
                vec![
                    "  • ".into(),
                    format!("Data classification (markdown): {}", class_table.as_str()).into(),
                ]
                .into(),
            );
        }
        if let Some(last_log) = last_log {
            summary_lines
                .push(vec!["  • ".into(), format!("Last update: {last_log}").dim()].into());
        }
        summary_lines.push(
            vec![
                "  • ".into(),
                format!("Log entries: {}", result.logs.len()).dim(),
            ]
            .into(),
        );

        self.add_to_history(PlainHistoryCell::new(summary_lines));

        if !result.logs.is_empty() {
            let mut log_lines: Vec<Line<'static>> = Vec::new();
            log_lines.push(vec!["Logs".bold()].into());
            for entry in &result.logs {
                let prefix = security_review_log_prefix(entry);
                log_lines.push(vec![prefix.dim(), entry.clone().into()].into());
            }
            self.add_to_history(PlainHistoryCell::new(log_lines));
        }

        self.security_review_artifacts = Some(SecurityReviewArtifactsState {
            repo_root: repo_path.clone(),
            snapshot_path: result.snapshot_path.clone(),
            bugs_path: result.bugs_path.clone(),
            report_path: result.report_path.clone(),
            report_html_path: result.report_html_path.clone(),
            metadata_path: result.metadata_path.clone(),
            api_overview_path: result.api_overview_path.clone(),
            classification_json_path: result.classification_json_path.clone(),
            classification_table_path: result.classification_table_path.clone(),
        });

        let follow_up_path = match mode {
            SecurityReviewMode::Full => result
                .report_path
                .clone()
                .or_else(|| result.report_html_path.clone())
                .unwrap_or_else(|| result.bugs_path.clone()),
            SecurityReviewMode::Bugs => result.bugs_path.clone(),
        };
        let has_report = result.report_path.is_some() || result.report_html_path.is_some();
        let follow_up_label = if mode == SecurityReviewMode::Full && has_report {
            "Report".to_string()
        } else {
            "Bugs".to_string()
        };
        let follow_up_display = display_path_for(&follow_up_path, &repo_path);

        // Show bug summary table at the end, just before the follow-up line.
        if let Some(table) = result.bug_summary_table.as_ref() {
            let mut table_lines: Vec<Line<'static>> = Vec::new();
            table_lines.push("Bug summary table".bold().into());
            for row in table.lines() {
                table_lines.push(Line::from(row.to_string()));
            }
            self.add_to_history(PlainHistoryCell::new(table_lines));
        }
        self.add_info_message(
            format!(
                "Security review follow-up ready — questions will include context from {follow_up_label} ({follow_up_display})."
            ),
            None,
        );
        self.bottom_pane
            .set_placeholder_text("Ask a security review follow-up question".to_string());

        self.security_review_follow_up = Some(SecurityReviewFollowUpState {
            repo_root: repo_path,
            scope_paths,
            mode,
            follow_up_path,
            follow_up_label,
        });
    }

    pub(crate) fn on_security_review_failed(&mut self, error: SecurityReviewFailure) {
        self.bottom_pane.set_task_running(false);
        self.bottom_pane
            .update_status_header(String::from("Working"));
        self.security_review_task = None;
        self.security_review_context = None;
        self.clear_security_review_follow_up();

        self.add_error_message(format!("Security review failed: {}", error.message));

        if !error.logs.is_empty() {
            let mut log_lines: Vec<Line<'static>> = Vec::new();
            log_lines.push(vec!["Logs".bold()].into());
            for entry in error.logs {
                let prefix = security_review_log_prefix(&entry);
                log_lines.push(vec![prefix.dim(), entry.into()].into());
            }
            self.add_to_history(PlainHistoryCell::new(log_lines));
        }
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
                    tx3.send(AppEvent::CodexOp(Op::Review {
                        review_request: ReviewRequest {
                            prompt: format!(
                                "Review the code changes against the base branch '{branch}'. Start by finding the merge diff between the current branch and {branch}'s upstream e.g. (`git merge-base HEAD \"$(git rev-parse --abbrev-ref \"{branch}@{{upstream}}\")\"`), then run `git diff` against that SHA to see what changes we would merge into the {branch} branch. Provide prioritized, actionable findings."
                            ),
                            user_facing_hint: format!("changes against '{branch}'"),
                        },
                    }));
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
        let commits = codex_core::git_info::recent_commits(cwd, 100).await;

        let mut items: Vec<SelectionItem> = Vec::with_capacity(commits.len());
        for entry in commits {
            let subject = entry.subject.clone();
            let sha = entry.sha.clone();
            let short = sha.chars().take(7).collect::<String>();
            let search_val = format!("{subject} {sha}");

            items.push(SelectionItem {
                name: subject.clone(),
                actions: vec![Box::new(move |tx3: &AppEventSender| {
                    let hint = format!("commit {short}");
                    let prompt = format!(
                        "Review the code changes introduced by commit {sha} (\"{subject}\"). Provide prioritized, actionable findings."
                    );
                    tx3.send(AppEvent::CodexOp(Op::Review {
                        review_request: ReviewRequest {
                            prompt,
                            user_facing_hint: hint,
                        },
                    }));
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
            None,
            Box::new(move |prompt: String| {
                let trimmed = prompt.trim().to_string();
                if trimmed.is_empty() {
                    return;
                }
                tx.send(AppEvent::CodexOp(Op::Review {
                    review_request: ReviewRequest {
                        prompt: trimmed.clone(),
                        user_facing_hint: trimmed,
                    },
                }));
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

    pub(crate) fn conversation_id(&self) -> Option<ConversationId> {
        self.conversation_id
    }

    pub(crate) fn rollout_path(&self) -> Option<PathBuf> {
        self.current_rollout_path.clone()
    }

    /// Return a reference to the widget's current config (includes any
    /// runtime overrides applied via TUI, e.g., model or approval policy).
    pub(crate) fn config_ref(&self) -> &Config {
        &self.config
    }

    pub(crate) fn clear_token_usage(&mut self) {
        self.token_info = None;
    }

    fn as_renderable(&self) -> RenderableItem<'_> {
        let active_cell_renderable = match &self.active_cell {
            Some(cell) => RenderableItem::Borrowed(cell).inset(Insets::tlbr(1, 0, 0, 0)),
            None => RenderableItem::Owned(Box::new(())),
        };
        let mut flex = FlexRenderable::new();
        flex.push(1, active_cell_renderable);
        flex.push(
            0,
            RenderableItem::Borrowed(&self.bottom_pane).inset(Insets::tlbr(1, 0, 0, 0)),
        );
        RenderableItem::Owned(Box::new(flex))
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

enum Notification {
    AgentTurnComplete { response: String },
    ExecApprovalRequested { command: String },
    EditApprovalRequested { cwd: PathBuf, changes: Vec<PathBuf> },
}

impl Notification {
    fn display(&self) -> String {
        match self {
            Notification::AgentTurnComplete { response } => {
                Notification::agent_turn_preview(response)
                    .unwrap_or_else(|| "Agent turn complete".to_string())
            }
            Notification::ExecApprovalRequested { command } => {
                format!("Approval requested: {}", truncate_text(command, 30))
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
        }
    }

    fn type_name(&self) -> &str {
        match self {
            Notification::AgentTurnComplete { .. } => "agent-turn-complete",
            Notification::ExecApprovalRequested { .. }
            | Notification::EditApprovalRequested { .. } => "approval-requested",
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
}

const AGENT_NOTIFICATION_PREVIEW_GRAPHEMES: usize = 200;

const EXAMPLE_PROMPTS: [&str; 6] = [
    "Explain this codebase",
    "Summarize recent commits",
    "Implement {feature}",
    "Find and fix a bug in @filename",
    "Write tests for @filename",
    "Improve documentation in @filename",
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
    entries: Vec<codex_core::git_info::CommitLogEntry>,
) {
    let mut items: Vec<SelectionItem> = Vec::with_capacity(entries.len());
    for entry in entries {
        let subject = entry.subject.clone();
        let sha = entry.sha.clone();
        let short = sha.chars().take(7).collect::<String>();
        let search_val = format!("{subject} {sha}");

        items.push(SelectionItem {
            name: subject.clone(),
            actions: vec![Box::new(move |tx3: &AppEventSender| {
                let hint = format!("commit {short}");
                let prompt = format!(
                    "Review the code changes introduced by commit {sha} (\"{subject}\"). Provide prioritized, actionable findings."
                );
                tx3.send(AppEvent::CodexOp(Op::Review {
                    review_request: ReviewRequest {
                        prompt,
                        user_facing_hint: hint,
                    },
                }));
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

fn security_review_log_prefix(entry: &str) -> &'static str {
    if is_security_review_tool_log(entry) {
        "    ↳ "
    } else {
        "  ↳ "
    }
}

fn is_security_review_tool_log(entry: &str) -> bool {
    const TOOL_PREFIXES: &[&str] = &[
        "Search `",
        "No content matches found for `",
        "Ripgrep content search for `",
        "Stopping search commands for ",
        "grep_files for `",
        "No files matched `",
        "Auto scope content search `",
        "Auto scope grep_files search",
        "Auto scope read `",
    ];
    if TOOL_PREFIXES.iter().any(|prefix| entry.starts_with(prefix)) {
        return true;
    }

    if entry.starts_with("Auto scope ") && (entry.contains(" search ") || entry.contains(" read "))
    {
        return true;
    }

    if entry.contains(" verification for ") {
        return true;
    }

    const TOOL_FRAGMENTS: &[&str] = &[
        ": curl",
        ": failed to run curl",
        ": python",
        ": failed to run python",
    ];
    TOOL_FRAGMENTS
        .iter()
        .any(|fragment| entry.contains(fragment))
}

#[cfg(test)]
pub(crate) mod tests;
