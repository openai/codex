//! Terminal title configuration view for customizing the terminal window/tab title.
//!
//! This module provides an interactive picker for selecting which items appear
//! in the terminal title. Users can:
//!
//! - Select items
//! - Reorder items
//! - Preview the rendered title

use std::collections::HashSet;

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
use crate::render::renderable::Renderable;

/// Available items that can be displayed in the terminal title.
#[derive(EnumIter, EnumString, Display, Debug, Clone, Eq, PartialEq)]
#[strum(serialize_all = "kebab_case")]
pub(crate) enum TerminalTitleItem {
    /// Project root name, or a compact cwd fallback.
    Project,
    /// Compact runtime status (Ready, Working, Thinking, ...).
    Status,
    /// Current thread title (if available).
    Thread,
    /// Current git branch (if available).
    GitBranch,
    /// Current model name.
    Model,
    /// Latest checklist task progress from `update_plan` (if available).
    #[strum(to_string = "task-progress")]
    TaskProgress,
}

impl TerminalTitleItem {
    pub(crate) fn description(&self) -> &'static str {
        match self {
            TerminalTitleItem::Project => "Project name (falls back to current directory name)",
            TerminalTitleItem::Status => "Compact session status (Ready, Working, Thinking, ...)",
            TerminalTitleItem::Thread => "Current thread title (omitted until available)",
            TerminalTitleItem::GitBranch => "Current Git branch (omitted when unavailable)",
            TerminalTitleItem::Model => "Current model name",
            TerminalTitleItem::TaskProgress => {
                "Latest task progress from update_plan (omitted until available)"
            }
        }
    }

    pub(crate) fn render(&self) -> &'static str {
        match self {
            TerminalTitleItem::Project => "codex",
            TerminalTitleItem::Status => "Working",
            TerminalTitleItem::Thread => "Fix terminal title command",
            TerminalTitleItem::GitBranch => "feat/title-command",
            TerminalTitleItem::Model => "gpt-5.2-codex",
            TerminalTitleItem::TaskProgress => "Tasks 2/5",
        }
    }
}

/// Interactive view for configuring terminal-title items.
pub(crate) struct TerminalTitleSetupView {
    picker: MultiSelectPicker,
}

impl TerminalTitleSetupView {
    pub(crate) fn new(title_items: Option<&[String]>, app_event_tx: AppEventSender) -> Self {
        let mut used_ids = HashSet::new();
        let mut items = Vec::new();

        if let Some(selected_items) = title_items.as_ref() {
            for id in *selected_items {
                let Ok(item) = id.parse::<TerminalTitleItem>() else {
                    continue;
                };
                let item_id = item.to_string();
                if !used_ids.insert(item_id.clone()) {
                    continue;
                }
                items.push(Self::title_select_item(item, true));
            }
        }

        for item in TerminalTitleItem::iter() {
            let item_id = item.to_string();
            if used_ids.contains(&item_id) {
                continue;
            }
            items.push(Self::title_select_item(item, false));
        }

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
            .on_preview(|items| {
                let preview = items
                    .iter()
                    .filter(|item| item.enabled)
                    .filter_map(|item| item.id.parse::<TerminalTitleItem>().ok())
                    .map(|item| item.render())
                    .collect::<Vec<_>>()
                    .join(" | ");
                if preview.is_empty() {
                    None
                } else {
                    Some(Line::from(preview))
                }
            })
            .on_change(|items, app_event| {
                let items = items
                    .iter()
                    .filter(|item| item.enabled)
                    .filter_map(|item| item.id.parse::<TerminalTitleItem>().ok())
                    .collect::<Vec<_>>();
                app_event.send(AppEvent::TerminalTitleSetupPreview { items });
            })
            .on_confirm(|ids, app_event| {
                let items = ids
                    .iter()
                    .map(|id| id.parse::<TerminalTitleItem>())
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap_or_default();
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
    use insta::assert_snapshot;
    use tokio::sync::mpsc::unbounded_channel;

    fn render_lines(view: &TerminalTitleSetupView, width: u16) -> String {
        let height = view.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        let lines: Vec<String> = (0..area.height)
            .map(|row| {
                let mut line = String::new();
                for col in 0..area.width {
                    let symbol = buf[(area.x + col, area.y + row)].symbol();
                    if symbol.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(symbol);
                    }
                }
                line
            })
            .collect();
        lines.join("\n")
    }

    #[test]
    fn renders_title_setup_popup() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let selected = [
            "project".to_string(),
            "status".to_string(),
            "thread".to_string(),
        ];
        let view = TerminalTitleSetupView::new(Some(&selected), tx);
        assert_snapshot!("terminal_title_setup_basic", render_lines(&view, 84));
    }
}
