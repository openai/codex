//! Terminal title configuration view for customizing the terminal window/tab title.
//!
//! This module provides an interactive picker for selecting which items appear
//! in the terminal title. Users can:
//!
//! - Select items
//! - Reorder items
//! - Preview the rendered title

use itertools::Itertools;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
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
use crate::bottom_pane::status_surface_preview::StatusSurfacePreviewData;
use crate::bottom_pane::status_surface_preview::StatusSurfacePreviewItem;
use crate::render::renderable::Renderable;

/// Available items that can be displayed in the terminal title.
///
/// Variants serialize to kebab-case identifiers (e.g. `AppName` -> `"app-name"`)
/// via strum. These identifiers are persisted in user config files, so renaming
/// or removing a variant is a breaking config change.
#[derive(EnumIter, EnumString, Display, Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[strum(serialize_all = "kebab-case")]
pub(crate) enum TerminalTitleItem {
    /// Codex app name.
    AppName,
    /// Project root name, or a compact cwd fallback.
    Project,
    /// Animated task spinner while active.
    Spinner,
    /// Compact runtime status text.
    Status,
    /// Current thread title (if available).
    Thread,
    /// Current git branch (if available).
    GitBranch,
    /// Current model name.
    Model,
    /// Latest checklist task progress from `update_plan` (if available).
    TaskProgress,
}

impl TerminalTitleItem {
    pub(crate) fn description(self) -> &'static str {
        match self {
            TerminalTitleItem::AppName => "Codex app name",
            TerminalTitleItem::Project => "Project name (falls back to current directory name)",
            TerminalTitleItem::Spinner => {
                "Animated task spinner (omitted while idle or when animations are off)"
            }
            TerminalTitleItem::Status => "Compact session status text (Ready, Working, Thinking)",
            TerminalTitleItem::Thread => "Current thread title (omitted until available)",
            TerminalTitleItem::GitBranch => "Current Git branch (omitted when unavailable)",
            TerminalTitleItem::Model => "Current model name",
            TerminalTitleItem::TaskProgress => {
                "Latest task progress from update_plan (omitted until available)"
            }
        }
    }

    pub(crate) fn preview_item(self) -> Option<StatusSurfacePreviewItem> {
        match self {
            TerminalTitleItem::AppName => Some(StatusSurfacePreviewItem::AppName),
            TerminalTitleItem::Project => Some(StatusSurfacePreviewItem::ProjectName),
            TerminalTitleItem::Spinner => None,
            TerminalTitleItem::Status => Some(StatusSurfacePreviewItem::Status),
            TerminalTitleItem::Thread => Some(StatusSurfacePreviewItem::ThreadTitle),
            TerminalTitleItem::GitBranch => Some(StatusSurfacePreviewItem::GitBranch),
            TerminalTitleItem::Model => Some(StatusSurfacePreviewItem::Model),
            TerminalTitleItem::TaskProgress => Some(StatusSurfacePreviewItem::TaskProgress),
        }
    }

    /// Returns the separator to place before this item in a rendered title.
    ///
    /// The spinner gets a plain space on either side so it reads as
    /// `my-project <spinner> Working` rather than `my-project | <spinner> | Working`.
    /// All other adjacent items are joined with ` | `.
    pub(crate) fn separator_from_previous(self, previous: Option<Self>) -> &'static str {
        match previous {
            None => "",
            Some(previous)
                if previous == TerminalTitleItem::Spinner || self == TerminalTitleItem::Spinner =>
            {
                " "
            }
            Some(_) => " | ",
        }
    }
}

pub(crate) fn preview_line_for_title_items(
    items: &[TerminalTitleItem],
    preview_data: &StatusSurfacePreviewData,
) -> Option<Line<'static>> {
    let mut previous = None;
    let preview = items
        .iter()
        .copied()
        .fold(String::new(), |mut preview, item| {
            if item == TerminalTitleItem::Spinner {
                preview.push_str(item.separator_from_previous(previous));
                preview.push('⠋');
                previous = Some(item);
                return preview;
            }
            let Some(value) = item
                .preview_item()
                .and_then(|preview_item| preview_data.value_for(preview_item))
            else {
                return preview;
            };
            preview.push_str(item.separator_from_previous(previous));
            preview.push_str(value);
            previous = Some(item);
            preview
        });
    if preview.is_empty() {
        None
    } else {
        Some(Line::from(preview))
    }
}

fn parse_terminal_title_items<T>(ids: impl Iterator<Item = T>) -> Option<Vec<TerminalTitleItem>>
where
    T: AsRef<str>,
{
    // Treat parsing as all-or-nothing so preview/confirm callbacks never emit
    // a partially interpreted ordering. Invalid ids are ignored when building
    // the picker, but once the user is interacting with the picker we only want
    // to persist or preview a fully valid selection.
    ids.map(|id| id.as_ref().parse::<TerminalTitleItem>())
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

/// Interactive view for configuring terminal-title items.
pub(crate) struct TerminalTitleSetupView {
    picker: MultiSelectPicker,
}

impl TerminalTitleSetupView {
    /// Creates the terminal-title picker, preserving the configured item order first.
    ///
    /// Unknown configured ids are skipped here instead of surfaced inline. The
    /// main TUI still warns about them when rendering the actual title, but the
    /// picker itself only exposes the selectable items it can meaningfully
    /// preview and persist.
    pub(crate) fn new(
        title_items: Option<&[String]>,
        preview_data: StatusSurfacePreviewData,
        app_event_tx: AppEventSender,
    ) -> Self {
        let selected_items = title_items
            .into_iter()
            .flatten()
            .filter_map(|id| id.parse::<TerminalTitleItem>().ok())
            .unique()
            .collect_vec();
        let selected_set = selected_items
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let items = selected_items
            .into_iter()
            .map(|item| Self::title_select_item(item, /*enabled*/ true))
            .chain(
                TerminalTitleItem::iter()
                    .filter(|item| !selected_set.contains(item))
                    .map(|item| Self::title_select_item(item, /*enabled*/ false)),
            )
            .collect();

        Self {
            picker: MultiSelectPicker::builder(
                "Configure Terminal Title".to_string(),
                Some("Select which items to display in the terminal title.".to_string()),
                app_event_tx,
            )
            .instructions(vec![
                "Use ↑↓ to navigate, ←→ to move, space to select, enter to confirm, esc to cancel."
                    .into(),
            ])
            .items(items)
            .enable_ordering()
            .on_preview(move |items| {
                let items = parse_terminal_title_items(
                    items
                        .iter()
                        .filter(|item| item.enabled)
                        .map(|item| item.id.as_str()),
                )?;
                preview_line_for_title_items(&items, &preview_data)
            })
            .on_change(|items, app_event| {
                let Some(items) = parse_terminal_title_items(
                    items
                        .iter()
                        .filter(|item| item.enabled)
                        .map(|item| item.id.as_str()),
                ) else {
                    return;
                };
                app_event.send(AppEvent::TerminalTitleSetupPreview { items });
            })
            .on_confirm(|ids, app_event| {
                let Some(items) = parse_terminal_title_items(ids.iter().map(String::as_str)) else {
                    return;
                };
                app_event.send(AppEvent::TerminalTitleSetup { items });
            })
            .on_cancel(|app_event| {
                app_event.send(AppEvent::TerminalTitleSetupCancelled);
            })
            .build(),
        }
    }

    fn title_select_item(item: TerminalTitleItem, enabled: bool) -> MultiSelectItem {
        MultiSelectItem {
            id: item.to_string(),
            name: item.to_string(),
            description: Some(item.description().to_string()),
            enabled,
        }
    }
}

impl BottomPaneView for TerminalTitleSetupView {
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

impl Renderable for TerminalTitleSetupView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.picker.render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.picker.desired_height(width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_terminal_title_items_preserves_order() {
        let items =
            parse_terminal_title_items(["project", "spinner", "status", "thread"].into_iter());
        assert_eq!(
            items,
            Some(vec![
                TerminalTitleItem::Project,
                TerminalTitleItem::Spinner,
                TerminalTitleItem::Status,
                TerminalTitleItem::Thread,
            ])
        );
    }

    #[test]
    fn parse_terminal_title_items_rejects_invalid_ids() {
        let items = parse_terminal_title_items(["project", "not-a-title-item"].into_iter());
        assert_eq!(items, None);
    }

    #[test]
    fn parse_terminal_title_items_accepts_kebab_case_variants() {
        let items = parse_terminal_title_items(["app-name", "git-branch"].into_iter());
        assert_eq!(
            items,
            Some(vec![
                TerminalTitleItem::AppName,
                TerminalTitleItem::GitBranch,
            ])
        );
    }
}
