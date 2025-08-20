use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::widgets::WidgetRef;

use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::status_indicator_widget::StatusIndicatorWidget;
use crate::tui::FrameRequester;

use super::BottomPaneView;

pub(crate) struct StatusIndicatorView {
    view: StatusIndicatorWidget,
}

impl StatusIndicatorView {
    pub fn new(app_event_tx: AppEventSender, frame_requester: FrameRequester) -> Self {
        Self {
            view: StatusIndicatorWidget::new(app_event_tx, frame_requester),
        }
    }

    pub fn update_text(&mut self, text: String) {
        self.view.update_text(text);
    }

    pub fn update_header(&mut self, header: String) {
        self.view.update_header(header);
    }
}

impl BottomPaneView for StatusIndicatorView {
    fn update_status_header(&mut self, header: String) {
        self.update_header(header);
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

    fn handle_key_event(&mut self, _pane: &mut BottomPane, key_event: KeyEvent) {
        if key_event.code == KeyCode::Esc {
            self.view.interrupt();
        }
    }

    fn update_status_text(&mut self, text: String) {
        self.update_text(text);
    }
}
