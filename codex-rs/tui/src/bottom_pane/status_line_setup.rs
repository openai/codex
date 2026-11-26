//! Status line configuration view for customizing the TUI status bar.
//!
//! This module provides an interactive picker for selecting which items appear
//! in the status line at the bottom of the terminal. Users can:
//!
//! - **Select items**: Toggle which information is displayed
//! - **Reorder items**: Use left/right arrows to change display order
//! - **Preview changes**: See a live preview of the configured status line
//!
//! # Available Status Line Items
//!
//! - Model information (name, reasoning level, provider)
//! - Named configuration profile
//! - Directory paths (current dir, project root)
//! - Machine hostname
//! - Git information (branch name)
//! - Permissions profile
//! - Approval mode
//! - Context usage (remaining %, used %, window size)
//! - Usage limits (primary, secondary)
//! - Session info (thread title, thread ID, tokens used)
//! - Application version

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use strum_macros::Display;
use strum_macros::EnumIter;
use strum_macros::EnumString;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::backend_id::BackendId;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::multi_select_picker::MultiSelectItem;
use crate::bottom_pane::multi_select_picker::MultiSelectPicker;
use crate::bottom_pane::status_surface_preview::StatusSurfacePreviewData;
use crate::bottom_pane::status_surface_preview::StatusSurfacePreviewItem;
use crate::key_hint::KeyBinding;
use crate::keymap::ListKeymap;
use crate::render::renderable::Renderable;

const STATUS_LINE_USE_THEME_COLORS_ITEM_ID: &str = "status-line-use-theme-colors";

/// Available items that can be displayed in the status line.
///
/// Each variant represents a piece of information that can be shown at the
/// bottom of the TUI. Items are serialized to kebab-case for configuration
/// storage (e.g., `ModelWithReasoning` becomes `model-with-reasoning`).
///
/// Some items are conditionally displayed based on availability:
/// - Git-related items only show when in a git repository
/// - Context/limit items only show when data is available from the API
/// - Thread ID only shows after a session has started
#[derive(EnumIter, EnumString, Display, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[strum(serialize_all = "kebab_case")]
pub(crate) enum StatusLineItem {
    /// The current model name.
    #[strum(to_string = "model", serialize = "model-name")]
    ModelName,

    /// Model name with reasoning level and processing tier.
    ModelWithReasoning,

    /// Current reasoning level.
    Reasoning,

    /// Current working directory path.
    CurrentDir,

    /// Project root directory (if detected).
    #[strum(
        to_string = "project-name",
        serialize = "project",
        serialize = "project-root"
    )]
    ProjectRoot,

    /// Hostname of the machine running Codex.
    Hostname,

    /// Current git branch name (if in a repository).
    GitBranch,

    /// Open pull request number for the current branch.
    PullRequestNumber,

    /// Committed branch diff stats relative to the default branch.
    BranchChanges,

    /// Compact runtime run-state text.
    #[strum(to_string = "run-state", serialize = "status")]
    Status,

    /// Active permission profile or sandbox summary.
    Permissions,

    /// Active command approval mode.
    #[strum(to_string = "approval-mode", serialize = "approval")]
    ApprovalMode,

    /// Percentage of context window remaining.
    ContextRemaining,

    /// Percentage of context window used.
    ///
    /// Also accepts the legacy `context-usage` config value.
    #[strum(to_string = "context-used", serialize = "context-usage")]
    ContextUsed,

    /// Remaining usage on the primary rate limit.
    FiveHourLimit,

    /// Remaining usage on the secondary rate limit.
    WeeklyLimit,

    /// Codex application version.
    CodexVersion,

    /// Total context window size in tokens.
    ContextWindowSize,

    /// Total tokens used in the current session.
    UsedTokens,

    /// Total input tokens consumed.
    TotalInputTokens,

    /// Total output tokens generated.
    TotalOutputTokens,

    /// Estimated credits attributed directly to the current enterprise thread.
    ThreadCredits,

    /// Estimated dollar cost attributed directly to the current enterprise thread.
    EstimatedThreadCost,

    /// Full thread UUID.
    #[strum(to_string = "thread-id", serialize = "session-id")]
    SessionId,

    /// Effective processing tier for the current model.
    FastMode,

    /// Whether Daybreak is enabled for this thread.
    Daybreak,

    /// Whether raw scrollback mode is currently active.
    RawOutput,

    /// Current thread name, omitted when unnamed.
    ThreadName,

    /// Current thread title, falling back to its identifier when unnamed.
    ThreadTitle,

    /// Current workspace notification headline.
    WorkspaceHeadline,

    /// Latest checklist task progress from `update_plan` (if available).
    TaskProgress,

    /// Named config profile selected for this invocation.
    Profile,

    /// Model provider ID for the active thread.
    #[strum(to_string = "provider", serialize = "model-provider")]
    Provider,
}

impl serde::Serialize for StatusLineItem {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Display emits the canonical config ID even when the item was parsed from an alias.
        serializer.collect_str(self)
    }
}

impl StatusLineItem {
    /// User-visible description shown in the popup.
    pub(crate) fn description(self) -> &'static str {
        match self {
            StatusLineItem::ModelName => "Current model name",
            StatusLineItem::ModelWithReasoning => "Current model name with reasoning level",
            StatusLineItem::Reasoning => "Current reasoning level",
            StatusLineItem::Profile => {
                "Config profile selected with --profile (omitted when unset)"
            }
            StatusLineItem::Provider => "Model provider for the active thread",
            StatusLineItem::CurrentDir => "Current working directory",
            StatusLineItem::ProjectRoot => "Project name (omitted when unavailable)",
            StatusLineItem::Hostname => "Current machine hostname (omitted when unavailable)",
            StatusLineItem::GitBranch => "Current Git branch (omitted when unavailable)",
            StatusLineItem::PullRequestNumber => {
                "Open pull request number for the current branch (omitted when unavailable)"
            }
            StatusLineItem::BranchChanges => {
                "Committed branch changes against the default branch (omitted when unavailable)"
            }
            StatusLineItem::Status => "Compact session run-state text (Ready, Working, Thinking)",
            StatusLineItem::Permissions => "Active permission profile or sandbox mode",
            StatusLineItem::ApprovalMode => "Active command approval mode",
            StatusLineItem::ContextRemaining => {
                "Percentage of context window remaining (omitted when unknown)"
            }
            StatusLineItem::ContextUsed => {
                "Percentage of context window used (omitted when unknown)"
            }
            StatusLineItem::FiveHourLimit => {
                "Remaining usage on the primary usage limit (omitted when unavailable)"
            }
            StatusLineItem::WeeklyLimit => {
                "Remaining usage on the secondary usage limit (omitted when unavailable)"
            }
            StatusLineItem::CodexVersion => "Codex application version",
            StatusLineItem::ContextWindowSize => {
                "Total context window size in tokens (omitted when unknown)"
            }
            StatusLineItem::UsedTokens => "Total tokens used in session (omitted when zero)",
            StatusLineItem::TotalInputTokens => "Total input tokens used in session",
            StatusLineItem::TotalOutputTokens => "Total output tokens used in session",
            StatusLineItem::ThreadCredits => {
                "Estimated current-thread credits (Enterprise workspaces only; omitted when unavailable)"
            }
            StatusLineItem::EstimatedThreadCost => {
                "Estimated current-thread cost in USD (Enterprise workspaces only; omitted when unavailable)"
            }
            StatusLineItem::SessionId => "Current thread identifier (omitted until thread starts)",
            StatusLineItem::FastMode => "Effective processing tier for the current model",
            StatusLineItem::Daybreak => "Whether Daybreak is enabled for this thread",
            StatusLineItem::RawOutput => "Whether raw scrollback mode is active",
            StatusLineItem::ThreadName => "Current thread name (omitted when unnamed)",
            StatusLineItem::ThreadTitle => {
                "Current thread title, or thread identifier when unnamed"
            }
            StatusLineItem::WorkspaceHeadline => {
                "Workspace notification headline (Enterprise workspaces only; omitted when unavailable)"
            }
            StatusLineItem::TaskProgress => {
                "Latest task progress from update_plan (omitted until available)"
            }
        }
    }

    pub(crate) fn preview_item(self) -> StatusSurfacePreviewItem {
        match self {
            StatusLineItem::ModelName => StatusSurfacePreviewItem::Model,
            StatusLineItem::ModelWithReasoning => StatusSurfacePreviewItem::ModelWithReasoning,
            StatusLineItem::Reasoning => StatusSurfacePreviewItem::Reasoning,
            StatusLineItem::Profile => StatusSurfacePreviewItem::Profile,
            StatusLineItem::Provider => StatusSurfacePreviewItem::Provider,
            StatusLineItem::CurrentDir => StatusSurfacePreviewItem::CurrentDir,
            StatusLineItem::ProjectRoot => StatusSurfacePreviewItem::ProjectRoot,
            StatusLineItem::Hostname => StatusSurfacePreviewItem::Hostname,
            StatusLineItem::GitBranch => StatusSurfacePreviewItem::GitBranch,
            StatusLineItem::PullRequestNumber => StatusSurfacePreviewItem::PullRequestNumber,
            StatusLineItem::BranchChanges => StatusSurfacePreviewItem::BranchChanges,
            StatusLineItem::Status => StatusSurfacePreviewItem::Status,
            StatusLineItem::Permissions => StatusSurfacePreviewItem::Permissions,
            StatusLineItem::ApprovalMode => StatusSurfacePreviewItem::ApprovalMode,
            StatusLineItem::ContextRemaining => StatusSurfacePreviewItem::ContextRemaining,
            StatusLineItem::ContextUsed => StatusSurfacePreviewItem::ContextUsed,
            StatusLineItem::FiveHourLimit => StatusSurfacePreviewItem::FiveHourLimit,
            StatusLineItem::WeeklyLimit => StatusSurfacePreviewItem::WeeklyLimit,
            StatusLineItem::CodexVersion => StatusSurfacePreviewItem::CodexVersion,
            StatusLineItem::ContextWindowSize => StatusSurfacePreviewItem::ContextWindowSize,
            StatusLineItem::UsedTokens => StatusSurfacePreviewItem::UsedTokens,
            StatusLineItem::TotalInputTokens => StatusSurfacePreviewItem::TotalInputTokens,
            StatusLineItem::TotalOutputTokens => StatusSurfacePreviewItem::TotalOutputTokens,
            StatusLineItem::ThreadCredits => StatusSurfacePreviewItem::ThreadCredits,
            StatusLineItem::EstimatedThreadCost => StatusSurfacePreviewItem::EstimatedThreadCost,
            StatusLineItem::SessionId => StatusSurfacePreviewItem::SessionId,
            StatusLineItem::FastMode => StatusSurfacePreviewItem::FastMode,
            StatusLineItem::Daybreak => StatusSurfacePreviewItem::Daybreak,
            StatusLineItem::RawOutput => StatusSurfacePreviewItem::RawOutput,
            StatusLineItem::ThreadName => StatusSurfacePreviewItem::ThreadName,
            StatusLineItem::ThreadTitle => StatusSurfacePreviewItem::ThreadTitle,
            StatusLineItem::WorkspaceHeadline => StatusSurfacePreviewItem::WorkspaceHeadline,
            StatusLineItem::TaskProgress => StatusSurfacePreviewItem::TaskProgress,
        }
    }
}

pub(crate) enum StatuslineProviderActions {
    Hidden,
    Choose,
    CreateAndChoose {
        backend: Option<BackendId>,
        thread_id: Option<codex_protocol::ThreadId>,
    },
}

/// Interactive view for configuring which items appear in the status line.
///
/// Wraps a [`MultiSelectPicker`] with status-line-specific behavior:
/// - Pre-populates items from current configuration
/// - Shows a live preview of the configured status line
/// - Emits [`AppEvent::StatusLineSetup`] on confirmation
/// - Emits [`AppEvent::StatusLineSetupCancelled`] on cancellation
pub(crate) struct StatusLineSetupView {
    /// The underlying multi-select picker widget.
    picker: MultiSelectPicker,
}

impl StatusLineSetupView {
    pub(crate) const VIEW_ID: &'static str = "status-line-setup";

    /// Creates a new status line setup view.
    ///
    /// # Arguments
    ///
    /// * `status_line_items` - Currently configured item IDs (in display order),
    ///   or `None` to start with all items disabled
    /// * `use_theme_colors` - Whether the preview and saved status line use colors from
    ///   the active theme
    /// * `app_event_tx` - Event sender for dispatching configuration changes
    ///
    /// Items from `status_line_items` are shown first (in order) and marked as
    /// enabled. Remaining items are appended and marked as disabled.
    pub(crate) fn new(
        status_line_items: Option<&[String]>,
        use_theme_colors: bool,
        providers: StatuslineProviderActions,
        preview_data: StatusSurfacePreviewData,
        app_event_tx: AppEventSender,
        list_keymap: ListKeymap,
    ) -> Self {
        let mut items = Vec::new();
        if !matches!(providers, StatuslineProviderActions::Hidden) {
            items.push(MultiSelectItem {
                name: "Choose a status line provider".into(),
                orderable: false,
                section_break_after: matches!(providers, StatuslineProviderActions::Choose),
                on_activate: Some(Box::new(|tx, key| {
                    tx.send(AppEvent::OpenStatuslinePicker(key))
                })),
                ..Default::default()
            });
        }
        if let StatuslineProviderActions::CreateAndChoose { backend, thread_id } = providers {
            items.push(MultiSelectItem {
                name: "Create a custom status line".into(),
                orderable: false,
                section_break_after: true,
                on_activate: Some(Box::new(move |tx, key| {
                    tx.send(AppEvent::CreateCustomStatusline {
                        backend,
                        thread_id,
                        key,
                    })
                })),
                ..Default::default()
            });
        }
        items.push(MultiSelectItem {
            id: STATUS_LINE_USE_THEME_COLORS_ITEM_ID.to_string(),
            name: "Use theme colors".to_string(),
            description: Some("Apply colors from the active /theme".to_string()),
            enabled: use_theme_colors,
            orderable: false,
            section_break_after: true,
            on_activate: None,
        });

        items.extend(preview_data.picker_items(
            status_line_items,
            StatusLineItem::description,
            |item| Some(item.preview_item()),
        ));

        Self {
            picker: MultiSelectPicker::builder(
                "Configure Status Line".to_string(),
                Some("Select which items to display in the status line.".to_string()),
                app_event_tx,
            )
            .list_keymap(list_keymap)
            .items(items)
            .enable_ordering()
            .on_preview_content(move |items| {
                let use_theme_colors = items
                    .iter()
                    .any(|item| item.id == STATUS_LINE_USE_THEME_COLORS_ITEM_ID && item.enabled);
                preview_data
                    .status_line_for_items(
                        items
                            .iter()
                            .filter(|item| item.enabled)
                            .filter_map(|item| item.id.parse::<StatusLineItem>().ok()),
                        use_theme_colors,
                    )
                    .map(Into::into)
            })
            .on_confirm(|ids, app_event| {
                let use_theme_colors = ids
                    .iter()
                    .any(|id| id == STATUS_LINE_USE_THEME_COLORS_ITEM_ID);
                let items = ids
                    .iter()
                    .filter_map(|id| id.parse::<StatusLineItem>().ok())
                    .collect::<Vec<_>>();
                app_event.send(AppEvent::StatusLineSetup {
                    items,
                    use_theme_colors,
                });
            })
            .on_cancel(|app_event| {
                app_event.send(AppEvent::StatusLineSetupCancelled);
            })
            .build(),
        }
    }
}

impl BottomPaneView for StatusLineSetupView {
    fn view_id(&self) -> Option<&'static str> {
        Some(Self::VIEW_ID)
    }

    fn keymap_contexts(&self) -> crate::keymap::KeymapContextSet {
        crate::keymap::KeymapContextSet::new(crate::keymap::KeymapContext::List)
    }

    fn accepts_input_when_disconnected(&self) -> bool {
        true
    }

    fn handle_key_event(&mut self, key_event: crossterm::event::KeyEvent) {
        self.picker.handle_key_event(key_event);
    }

    fn handle_key_event_with_activation_key(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        activation_key: Option<KeyBinding>,
    ) {
        self.picker
            .handle_key_event_with_activation_key(key_event, activation_key);
    }

    fn is_complete(&self) -> bool {
        self.picker.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.picker.close();
        CancellationEvent::Handled
    }
}

impl Renderable for StatusLineSetupView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.picker.render(area, buf)
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.picker.desired_height(width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event_sender::AppEventSender;
    use crate::bottom_pane::multi_select_picker::tests::render_lines;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    use crate::app_event::AppEvent;

    #[test]
    fn configured_aliases_keep_their_first_position() {
        let configured = ["project", "model-name", "project-name", "invalid"].map(str::to_string);
        let rows = StatusSurfacePreviewData::default().picker_items(
            Some(&configured),
            StatusLineItem::description,
            |item| Some(item.preview_item()),
        );
        let enabled = rows
            .iter()
            .filter(|row| row.enabled)
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(enabled, ["project-name", "model"]);
        assert_eq!(
            rows.iter().filter(|row| row.id == "project-name").count(),
            1
        );
    }

    #[test]
    fn context_used_accepts_context_usage_legacy_id() {
        assert_eq!(StatusLineItem::ContextUsed.to_string(), "context-used");
        assert_eq!(
            "context-used".parse::<StatusLineItem>(),
            Ok(StatusLineItem::ContextUsed)
        );
        assert_eq!(
            "context-usage".parse::<StatusLineItem>(),
            Ok(StatusLineItem::ContextUsed)
        );
    }

    #[test]
    fn context_remaining_is_selectable_id() {
        assert_eq!(
            "context-remaining".parse::<StatusLineItem>(),
            Ok(StatusLineItem::ContextRemaining)
        );
        assert_eq!(
            StatusLineItem::ContextRemaining.to_string(),
            "context-remaining"
        );
    }

    #[test]
    fn thread_usage_items_are_independently_selectable() {
        assert_eq!(
            "thread-credits".parse::<StatusLineItem>(),
            Ok(StatusLineItem::ThreadCredits)
        );
        assert_eq!(
            "estimated-thread-cost".parse::<StatusLineItem>(),
            Ok(StatusLineItem::EstimatedThreadCost)
        );
    }

    #[test]
    fn project_name_is_canonical_and_accepts_legacy_ids() {
        assert_eq!(StatusLineItem::ProjectRoot.to_string(), "project-name");
        assert_eq!(
            "project-name".parse::<StatusLineItem>(),
            Ok(StatusLineItem::ProjectRoot)
        );
        assert_eq!(
            "project".parse::<StatusLineItem>(),
            Ok(StatusLineItem::ProjectRoot)
        );
        assert_eq!(
            "project-root".parse::<StatusLineItem>(),
            Ok(StatusLineItem::ProjectRoot)
        );
    }

    #[test]
    fn model_is_canonical_and_accepts_model_name_legacy_id() {
        assert_eq!(StatusLineItem::ModelName.to_string(), "model");
        assert_eq!(
            "model".parse::<StatusLineItem>(),
            Ok(StatusLineItem::ModelName)
        );
        assert_eq!(
            "model-name".parse::<StatusLineItem>(),
            Ok(StatusLineItem::ModelName)
        );
    }

    #[test]
    fn reasoning_is_selectable_id() {
        assert_eq!(StatusLineItem::Reasoning.to_string(), "reasoning");
        assert_eq!(
            "reasoning".parse::<StatusLineItem>(),
            Ok(StatusLineItem::Reasoning)
        );
    }

    #[test]
    fn run_state_is_canonical_and_accepts_status_legacy_id() {
        assert_eq!(StatusLineItem::Status.to_string(), "run-state");
        assert_eq!(
            "run-state".parse::<StatusLineItem>(),
            Ok(StatusLineItem::Status)
        );
        assert_eq!(
            "status".parse::<StatusLineItem>(),
            Ok(StatusLineItem::Status)
        );
    }

    #[test]
    fn git_summary_items_are_selectable_ids() {
        assert_eq!(
            "pull-request-number".parse::<StatusLineItem>(),
            Ok(StatusLineItem::PullRequestNumber)
        );
        assert_eq!(
            "branch-changes".parse::<StatusLineItem>(),
            Ok(StatusLineItem::BranchChanges)
        );
    }

    #[test]
    fn parse_status_line_items_accepts_title_only_variants() {
        let items = ["run-state", "task-progress"]
            .into_iter()
            .map(str::parse::<StatusLineItem>)
            .collect::<Result<Vec<_>, _>>();
        assert_eq!(
            items,
            Ok(vec![StatusLineItem::Status, StatusLineItem::TaskProgress,])
        );
    }

    #[test]
    fn preview_fills_missing_runtime_values_with_placeholders() {
        let preview_data = StatusSurfacePreviewData::from_iter([
            (
                StatusLineItem::ModelName.preview_item(),
                "gpt-5".to_string(),
            ),
            (
                StatusLineItem::ThreadTitle.preview_item(),
                "Roadmap cleanup".to_string(),
            ),
        ]);
        let preview = preview_data.status_line_for_items(
            [
                StatusLineItem::ModelName,
                StatusLineItem::GitBranch,
                StatusLineItem::ThreadTitle,
            ],
            /*use_theme_colors*/ true,
        );
        assert_eq!(
            preview.unwrap().line.to_string(),
            "gpt-5 · feat/awesome-feature · Roadmap cleanup"
        );
    }

    #[test]
    fn picker_previews_the_pr_link_and_preserves_the_color_preference() {
        let url = "https://example.com/pr/42";
        for use_theme_colors in [false, true] {
            let (tx, mut rx) = unbounded_channel();
            let mut preview = StatusSurfacePreviewData::from_iter([(
                StatusLineItem::PullRequestNumber.preview_item(),
                "PR #42",
            )]);
            preview.pull_request_url = Some(url.to_owned());
            let mut view = StatusLineSetupView::new(
                Some(&[StatusLineItem::PullRequestNumber.to_string()]),
                use_theme_colors,
                StatuslineProviderActions::Hidden,
                preview,
                AppEventSender::new(tx),
                crate::keymap::RuntimeKeymap::defaults().list,
            );
            let output = render_lines(&view, /*width*/ 80);
            assert!(output.contains(&crate::terminal_hyperlinks::osc8_hyperlink(url, "P")));
            view.handle_key_event(crossterm::event::KeyCode::Enter.into());
            assert!(matches!(
                rx.try_recv(),
                Ok(AppEvent::StatusLineSetup { use_theme_colors: saved, .. }) if saved == use_theme_colors
            ));
        }
    }

    #[test]
    fn setup_view_snapshot_uses_runtime_preview_values() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let view = StatusLineSetupView::new(
            Some(&[
                StatusLineItem::ModelName.to_string(),
                StatusLineItem::CurrentDir.to_string(),
                StatusLineItem::GitBranch.to_string(),
            ]),
            /*use_theme_colors*/ true,
            StatuslineProviderActions::Hidden,
            StatusSurfacePreviewData::from_iter([
                (
                    StatusLineItem::ModelName.preview_item(),
                    "gpt-5-codex".to_string(),
                ),
                (
                    StatusLineItem::CurrentDir.preview_item(),
                    "~/codex-rs".to_string(),
                ),
                (
                    StatusLineItem::GitBranch.preview_item(),
                    "jif/statusline-preview".to_string(),
                ),
                (
                    StatusLineItem::WeeklyLimit.preview_item(),
                    "weekly 82% left".to_string(),
                ),
            ]),
            AppEventSender::new(tx_raw),
            crate::keymap::RuntimeKeymap::defaults().list,
        );

        assert_snapshot!(render_lines(&view, /*width*/ 72));
    }

    #[test]
    fn setup_view_snapshot_includes_thread_usage_items() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let view = StatusLineSetupView::new(
            Some(&[
                StatusLineItem::ThreadCredits.to_string(),
                StatusLineItem::EstimatedThreadCost.to_string(),
            ]),
            /*use_theme_colors*/ true,
            StatuslineProviderActions::Hidden,
            StatusSurfacePreviewData::from_iter([
                (
                    StatusLineItem::ThreadCredits.preview_item(),
                    "5.2 credits".to_string(),
                ),
                (
                    StatusLineItem::EstimatedThreadCost.preview_item(),
                    "~$1.82".to_string(),
                ),
            ]),
            AppEventSender::new(tx_raw),
            crate::keymap::RuntimeKeymap::defaults().list,
        );

        assert_snapshot!(render_lines(&view, /*width*/ 100));
    }

    #[test]
    fn setup_view_snapshot_includes_hostname() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let view = StatusLineSetupView::new(
            Some(&[
                StatusLineItem::Hostname.to_string(),
                StatusLineItem::CurrentDir.to_string(),
            ]),
            /*use_theme_colors*/ true,
            StatuslineProviderActions::Hidden,
            StatusSurfacePreviewData::from_iter([
                (
                    StatusLineItem::Hostname.preview_item(),
                    "ssh-build-01.example.com".to_string(),
                ),
                (
                    StatusLineItem::CurrentDir.preview_item(),
                    "~/codex-rs".to_string(),
                ),
            ]),
            AppEventSender::new(tx_raw),
            crate::keymap::RuntimeKeymap::defaults().list,
        );

        assert_snapshot!(
            render_lines(&view, /*width*/ 100)
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
