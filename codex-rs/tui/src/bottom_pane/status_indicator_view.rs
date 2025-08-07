use ratatui::buffer::Buffer;
use ratatui::widgets::WidgetRef;

use crate::app_event_sender::AppEventSender;
use crate::status_indicator_widget::StatusIndicatorWidget;
use codex_core::protocol::TokenUsage;

use super::BottomPaneView;
use super::bottom_pane_view::ConditionalUpdate;

pub(crate) struct StatusIndicatorView {
    view: StatusIndicatorWidget,
}

impl StatusIndicatorView {
    pub fn new(app_event_tx: AppEventSender) -> Self {
        Self {
            view: StatusIndicatorWidget::new(app_event_tx),
        }
    }

    pub fn update_text(&mut self, text: String) {
        self.view.update_text(text);
    }

    pub fn set_token_usage(&mut self, usage: TokenUsage) {
        self.view.set_token_usage(usage);
    }
}

impl BottomPaneView<'_> for StatusIndicatorView {
    fn update_status_text(&mut self, text: String) -> ConditionalUpdate {
        self.update_text(text);
        ConditionalUpdate::NeedsRedraw
    }

    fn should_hide_when_task_is_done(&mut self) -> bool {
        true
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.view.desired_height(width)
    }

    fn render(&self, area: ratatui::layout::Rect, buf: &mut Buffer) {
        self.view.render_ref(area, buf);
    }

    fn update_token_usage(&mut self, token_usage: TokenUsage) -> ConditionalUpdate {
        self.set_token_usage(token_usage);
        ConditionalUpdate::NeedsRedraw
    }
}
