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
//! - Model information (name, reasoning level)
//! - Directory paths (current dir, project root)
//! - Git information (branch name)
//! - Context usage (remaining %, used %, window size)
//! - Usage limits (5-hour, weekly)
//! - Session info (ID, tokens used)
//! - Application version

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use std::collections::HashSet;
use strum::IntoEnumIterator;
use strum_macros::Display;
use strum_macros::EnumIter;
use strum_macros::EnumString;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::multi_select_picker::MultiSelectItem;
use crate::bottom_pane::multi_select_picker::MultiSelectPicker;
use crate::render::renderable::Renderable;

/// Available items that can be displayed in the status line.
///
/// Each variant represents a piece of information that can be shown at the
/// bottom of the TUI. Items are serialized to kebab-case for configuration
/// storage (e.g., `ModelWithReasoning` becomes `model-with-reasoning`).
///
/// Some items are conditionally displayed based on availability:
/// - Git-related items only show when in a git repository
/// - Context/limit items only show when data is available from the API
/// - Session ID only shows after a session has started
#[derive(EnumIter, EnumString, Display, Debug, Clone, Eq, PartialEq)]
#[strum(serialize_all = "kebab_case")]
pub(crate) enum StatusLineItem {
    /// The current model name.
    ModelName,

    /// Model name with reasoning level suffix.
    ModelWithReasoning,

    /// Current working directory path.
    CurrentDir,

    /// Project root directory (if detected).
    ProjectRoot,

    /// Current git branch name (if in a repository).
    GitBranch,

    /// Percentage of context window remaining.
    ContextRemaining,

    /// Percentage of context window used.
    ContextUsed,

    /// Remaining usage on the 5-hour rate limit.
    FiveHourLimit,

    /// Remaining usage on the weekly rate limit.
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

    /// Full session UUID.
    SessionId,
}

impl StatusLineItem {
    /// User-visible description shown in the popup.
    pub(crate) fn description(&self) -> &'static str {
        match self {
            StatusLineItem::ModelName => "Current model name",
            StatusLineItem::ModelWithReasoning => "Current model name with reasoning level",
            StatusLineItem::CurrentDir => "Current working directory",
            StatusLineItem::ProjectRoot => "Project root directory (omitted when unavailable)",
            StatusLineItem::GitBranch => "Current Git branch (omitted when unavailable)",
            StatusLineItem::ContextRemaining => {
                "Percentage of context window remaining (omitted when unknown)"
            }
            StatusLineItem::ContextUsed => {
                "Percentage of context window used (omitted when unknown)"
            }
            StatusLineItem::FiveHourLimit => {
                "Remaining usage on 5-hour usage limit (omitted when unavailable)"
            }
            StatusLineItem::WeeklyLimit => {
                "Remaining usage on weekly usage limit (omitted when unavailable)"
            }
            StatusLineItem::CodexVersion => "Codex application version",
            StatusLineItem::ContextWindowSize => {
                "Total context window size in tokens (omitted when unknown)"
            }
            StatusLineItem::UsedTokens => "Total tokens used in session (omitted when zero)",
            StatusLineItem::TotalInputTokens => "Total input tokens used in session",
            StatusLineItem::TotalOutputTokens => "Total output tokens used in session",
            StatusLineItem::SessionId => {
                "Current session identifier (omitted until session starts)"
            }
        }
    }

    /// Returns an example rendering of this item for the preview.
    ///
    /// These are placeholder values used to show users what each item looks
    /// like in the status line before they confirm their selection.
    pub(crate) fn render(&self) -> &'static str {
        match self {
            StatusLineItem::ModelName => "gpt-5.2-codex",
            StatusLineItem::ModelWithReasoning => "gpt-5.2-codex medium",
            StatusLineItem::CurrentDir => "~/project/path",
            StatusLineItem::ProjectRoot => "~/project",
            StatusLineItem::GitBranch => "feat/awesome-feature",
            StatusLineItem::ContextRemaining => "18% left",
            StatusLineItem::ContextUsed => "82% used",
            StatusLineItem::FiveHourLimit => "5h 100%",
            StatusLineItem::WeeklyLimit => "weekly 98%",
            StatusLineItem::CodexVersion => "v0.93.0",
            StatusLineItem::ContextWindowSize => "258K window",
            StatusLineItem::UsedTokens => "27.3K used",
            StatusLineItem::TotalInputTokens => "17,588 in",
            StatusLineItem::TotalOutputTokens => "265 out",
            StatusLineItem::SessionId => "019c19bd-ceb6-73b0-adc8-8ec0397b85cf",
        }
    }
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
    /// Creates a new status line setup view.
    ///
    /// # Arguments
    ///
    /// * `status_line_items` - Currently configured item IDs (in display order),
    ///   or `None` to start with all items disabled
    /// * `app_event_tx` - Event sender for dispatching configuration changes
    ///
    /// Items from `status_line_items` are shown first (in order) and marked as
    /// enabled. Remaining items are appended and marked as disabled.
    pub(crate) fn new(status_line_items: Option<&[String]>, app_event_tx: AppEventSender) -> Self {
        let mut used_ids = HashSet::new();
        let mut items = Vec::new();

        if let Some(selected_items) = status_line_items.as_ref() {
            for id in *selected_items {
                let Ok(item) = id.parse::<StatusLineItem>() else {
                    continue;
                };
                let item_id = item.to_string();
                if !used_ids.insert(item_id.clone()) {
                    continue;
                }
                items.push(Self::status_line_select_item(item, true));
            }
        }

        for item in StatusLineItem::iter() {
            let item_id = item.to_string();
            if used_ids.contains(&item_id) {
                continue;
            }
            items.push(Self::status_line_select_item(item, false));
        }

        Self {
            picker: MultiSelectPicker::builder(
                "Configure Status Line".to_string(),
                Some("Select which items to display in the status line.".to_string()),
                app_event_tx,
            )
            .instructions(vec![
                "Use ↑↓ to navigate, ←→ to move, space to select, enter to confirm, esc to cancel."
                    .into(),
            ])
            .items(items)
            .enable_ordering()
            .on_preview(|items| {
                let preview = items
                    .iter()
                    .filter(|item| item.enabled)
                    .filter_map(|item| item.id.parse::<StatusLineItem>().ok())
                    .map(|item| item.render())
                    .collect::<Vec<_>>()
                    .join(" · ");
                if preview.is_empty() {
                    None
                } else {
                    Some(Line::from(preview))
                }
            })
            .on_confirm(|ids, app_event| {
                let items = ids
                    .iter()
                    .map(|id| id.parse::<StatusLineItem>())
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap_or_default();
                app_event.send(AppEvent::StatusLineSetup { items });
            })
            .on_cancel(|app_event| {
                app_event.send(AppEvent::StatusLineSetupCancelled);
            })
            .build(),
        }
    }

    /// Converts a [`StatusLineItem`] into a [`MultiSelectItem`] for the picker.
    fn status_line_select_item(item: StatusLineItem, enabled: bool) -> MultiSelectItem {
        MultiSelectItem {
            id: item.to_string(),
            name: item.to_string(),
            description: Some(item.description().to_string()),
            enabled,
        }
    }
}

impl BottomPaneView for StatusLineSetupView {
    fn handle_key_event(&mut self, key_event: crossterm::event::KeyEvent) {
        self.picker.handle_key_event(key_event);
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
