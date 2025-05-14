use std::sync::mpsc::SendError;
use std::sync::mpsc::Sender;

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use crate::app_event::AppEvent;
use crate::status_indicator_widget::StatusIndicatorWidget;

use super::BottomPane;
use super::BottomPaneView;
use super::bottom_pane_view::ConditionalUpdate;

pub(crate) struct StatusIndicatorView {
    view: StatusIndicatorWidget,
}

impl StatusIndicatorView {
    pub fn new(app_event_tx: Sender<AppEvent>, height: u16) -> Self {
        Self {
            view: StatusIndicatorWidget::new(app_event_tx, height),
        }
    }

    pub fn update_text(&mut self, text: String) {
        self.view.update_text(text);
    }
}

impl<'a> BottomPaneView<'a> for StatusIndicatorView {
    fn handle_key_event(
        &mut self,
        _pane: &mut BottomPane<'a>,
        _key_event: KeyEvent,
    ) -> Result<(), SendError<AppEvent>> {
        Ok(())
    }

    fn update_status_text(&mut self, text: String) -> ConditionalUpdate {
        self.update_text(text);
        ConditionalUpdate::NeedsRedraw
    }

    fn should_hide_when_task_is_done(&mut self) -> bool {
        true
    }

    fn calculate_required_height(&self, _area: &Rect) -> u16 {
        self.view.get_height()
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.view.render_ref(area, buf);
    }
}
