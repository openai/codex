use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::widgets::WidgetRef;

use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::status_indicator_widget::StatusIndicatorWidget;

use super::BottomPaneView;

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
}

impl BottomPaneView<'_> for StatusIndicatorView {
    fn should_hide_when_task_is_done(&mut self) -> bool {
        true
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.view.desired_height(width)
    }

    fn render(&self, area: ratatui::layout::Rect, buf: &mut Buffer) {
        self.view.render_ref(area, buf);
    }

    fn handle_key_event(&mut self, _pane: &mut BottomPane<'_>, key_event: KeyEvent) {
        if key_event.code == KeyCode::Esc {
            self.view.interrupt();
        }
    }
}
