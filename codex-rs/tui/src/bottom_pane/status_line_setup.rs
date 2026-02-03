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

#[derive(EnumIter, EnumString, Display, Debug, Clone, Eq, PartialEq)]
#[strum(serialize_all = "kebab_case")]
pub(crate) enum StatusLineItem {
    ModelName,
    ModelWithReasoning,
    CurrentDir,
    ProjectRoot,
    GitBranch,
    ContextRemaining,
    ContextUsed,
    GitLines,
    FiveDayLimit,
    WeeklyLimit,
    CodexVersion,
    ContextWindowSize,
    TotalInputTokens,
    TotalOutputTokens,
    SessionId,
    SessionIdPrefix,
}

impl StatusLineItem {
    /// User-visible description shown in the popup.
    pub(crate) fn description(&self) -> &'static str {
        match self {
            StatusLineItem::ModelName => "Current model name",
            StatusLineItem::ModelWithReasoning => "Current model name with reasoning level",
            StatusLineItem::CurrentDir => "Current working directory",
            StatusLineItem::ProjectRoot => "Project root directory",
            StatusLineItem::GitBranch => "Current Git branch",
            StatusLineItem::ContextRemaining => "Percentage of context window remaining",
            StatusLineItem::ContextUsed => "Percentage of context window used",
            StatusLineItem::GitLines => "Total lines added to and removed from Git in session",
            StatusLineItem::FiveDayLimit => "Remaining usage on 5-day usage limit",
            StatusLineItem::WeeklyLimit => "Remaining usage on weekly usage limit",
            StatusLineItem::CodexVersion => "Codex application version",
            StatusLineItem::ContextWindowSize => "Total context window size in tokens",
            StatusLineItem::TotalInputTokens => "Total input tokens used in session",
            StatusLineItem::TotalOutputTokens => "Total output tokens used in session",
            StatusLineItem::SessionId => "Current session identifier",
            StatusLineItem::SessionIdPrefix => "Current session identifier (shortened)",
        }
    }

    pub(crate) fn render(&self) -> &'static str {
        match self {
            StatusLineItem::ModelName => "gpt-5.2-codex",
            StatusLineItem::ModelWithReasoning => "gpt-5.2-codex (medium)",
            StatusLineItem::CurrentDir => "~/project/path",
            StatusLineItem::ProjectRoot => "~/project",
            StatusLineItem::GitBranch => "feat/awesome-feature",
            StatusLineItem::ContextRemaining => "18% left",
            StatusLineItem::ContextUsed => "82% used",
            StatusLineItem::FiveDayLimit => "5h 100%",
            StatusLineItem::WeeklyLimit => "7d 98%",
            StatusLineItem::GitLines => "+123/-45",
            StatusLineItem::CodexVersion => "v0.93.0",
            StatusLineItem::ContextWindowSize => "258,400",
            StatusLineItem::TotalInputTokens => "17,588",
            StatusLineItem::TotalOutputTokens => "265",
            StatusLineItem::SessionId => "019c19bd-ceb6-73b0-adc8-8ec0397b85cf",
            StatusLineItem::SessionIdPrefix => "019c19bd",
        }
    }
}

pub(crate) struct StatusLineSetupView {
    picker: MultiSelectPicker,
    app_event_tx: AppEventSender,
}

impl StatusLineSetupView {
    pub(crate) fn new(status_line_items: Option<&[String]>, app_event_tx: AppEventSender) -> Self {
        let enabled_ids: HashSet<String> = status_line_items
            .as_ref()
            .map(|items| items.iter().cloned().collect())
            .unwrap_or_default();
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
            let enabled = enabled_ids.contains(&item_id);
            items.push(Self::status_line_select_item(item, enabled));
        }

        Self {
            picker: MultiSelectPicker::new(
                "Configure Status Line".to_string(),
                Some("Select which items to display in the status line.".to_string()),
                app_event_tx.clone(),
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
            app_event_tx,
        }
    }

    fn status_line_select_item(item: StatusLineItem, enabled: bool) -> MultiSelectItem {
        MultiSelectItem {
            id: item.to_string(),
            name: item.to_string(),
            description: Some(item.description().to_string()),
            enabled,
            ..Default::default()
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
        self.app_event_tx.send(AppEvent::StatusLineSetupCancelled);
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
