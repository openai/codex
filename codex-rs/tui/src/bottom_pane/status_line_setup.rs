use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use strum_macros::EnumString;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::multi_select_picker::MultiSelectItem;
use crate::bottom_pane::multi_select_picker::MultiSelectPicker;
use crate::render::renderable::Renderable;

#[derive(EnumIter, EnumString, Debug, Clone, Eq, PartialEq)]
pub(crate) enum StatusLineItem {
    ModelName,
    Cwd,
    GitBranch,
    ContextUsedPct,
    ContextRemainingPct,
    CodexVersion,
    ContextWindowSize,
    TotalInputTokens,
    TotalOutputTokens,
    GitLinesAdded,
    GitLinesRemoved,
    SessionId,
}

impl StatusLineItem {
    /// User-visible description shown in the popup.
    pub(crate) fn description(&self) -> &'static str {
        match self {
            StatusLineItem::ModelName => "Current model name",
            StatusLineItem::Cwd => "Current working directory",
            StatusLineItem::GitBranch => "Current Git branch",
            StatusLineItem::ContextUsedPct => "Percentage of context window used",

            StatusLineItem::CodexVersion => "Codex application version",
            StatusLineItem::ContextRemainingPct => "Percentage of context window remaining",
            StatusLineItem::ContextWindowSize => "Total context window size in tokens",
            StatusLineItem::TotalInputTokens => "Total input tokens used in session",
            StatusLineItem::TotalOutputTokens => "Total output tokens used in session",
            StatusLineItem::GitLinesAdded => "Total lines added to Git in session",
            StatusLineItem::GitLinesRemoved => "Total lines removed from Git in session",
            StatusLineItem::SessionId => "Current session identifier",
        }
    }

    pub(crate) fn short_label(&self) -> &'static str {
        match self {
            StatusLineItem::ModelName => "Model",
            StatusLineItem::Cwd => "Cwd",
            StatusLineItem::GitBranch => "Git",
            StatusLineItem::ContextUsedPct => "Used",
            StatusLineItem::ContextRemainingPct => "Remaining",
            StatusLineItem::CodexVersion => "Version",
            StatusLineItem::ContextWindowSize => "Ctx",
            StatusLineItem::TotalInputTokens => "In",
            StatusLineItem::TotalOutputTokens => "Out",
            StatusLineItem::GitLinesAdded => "+Lines",
            StatusLineItem::GitLinesRemoved => "-Lines",
            StatusLineItem::SessionId => "Session",
        }
    }
}

pub(crate) struct StatusLineSetupView {
    picker: MultiSelectPicker,
    app_event_tx: AppEventSender,
}

impl StatusLineSetupView {
    pub(crate) fn new(app_event_tx: AppEventSender) -> Self {
        let items = StatusLineItem::iter()
            .map(|item| MultiSelectItem {
                id: format!("{:?}", item),
                name: format!("{:?}", item),
                description: Some(item.description().to_string()),
                ..Default::default()
            })
            .collect::<Vec<_>>();

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
            .preview(|items| {
                let preview = items
                    .iter()
                    .filter(|item| item.enabled)
                    .filter_map(|item| item.id.parse::<StatusLineItem>().ok())
                    .map(|item| item.short_label())
                    .collect::<Vec<_>>()
                    .join(" · ");
                if preview.is_empty() {
                    None
                } else {
                    Some(Line::from(preview))
                }
            })
            .on_change(|items, app_event| {
                let items = items
                    .iter()
                    .filter(|multi_select_item| multi_select_item.enabled)
                    .filter_map(|multi_select_item| multi_select_item.id.parse().ok())
                    .collect();
                app_event.send(AppEvent::StatusLinePreview { items });
            })
            .on_confirm(|items, app_event| {
                app_event.send(AppEvent::StatusLinePreview { items: Vec::new() });
                let items = items.into();
                app_event.send(AppEvent::StatusLineSetup { items });
            })
            .on_cancel(|app_event| {
                app_event.send(AppEvent::StatusLinePreview { items: Vec::new() });
                app_event.send(AppEvent::StatusLineSetupCancelled);
            })
            .build(),
            app_event_tx,
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
        self.app_event_tx
            .send(AppEvent::StatusLinePreview { items: Vec::new() });
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
