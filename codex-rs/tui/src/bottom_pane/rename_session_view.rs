use std::cell::RefCell;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::WidgetRef;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::bottom_pane::textarea::TextArea;
use crate::bottom_pane::textarea::TextAreaState;

/// Simple bottom-pane view to rename the current session.
pub(crate) struct RenameSessionView {
    tx: AppEventSender,
    input: TextArea,
    input_state: RefCell<TextAreaState>,
    done: bool,
}

impl RenameSessionView {
    pub(crate) fn new(tx: AppEventSender, initial: &str) -> Self {
        let mut input = TextArea::new();
        input.set_text(initial);
        input.set_cursor(initial.len());
        Self {
            tx,
            input,
            input_state: RefCell::new(TextAreaState::default()),
            done: false,
        }
    }
}

impl crate::render::renderable::Renderable for RenameSessionView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        // Two rows layout: title and input + hint
        let [title, input, hint] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);

        let title_line: Line = vec!["Rename session".bold().cyan()].into();
        WidgetRef::render_ref(&title_line, title, buf);

        // Render single-line input prefixed with label
        let label = "Name: ";
        let label_width = label.len() as u16;
        // Draw label
        WidgetRef::render_ref(&Line::from(Span::from(label).dim()), input, buf);

        // Draw the editable field after the label
        let mut input_rect = input;
        if input_rect.width > label_width {
            input_rect.x += label_width;
            input_rect.width = input_rect.width.saturating_sub(label_width);
        }
        let mut state = self.input_state.borrow_mut();
        ratatui::widgets::StatefulWidgetRef::render_ref(
            &(&self.input),
            input_rect,
            buf,
            &mut state,
        );

        // Footer hint
        WidgetRef::render_ref(&standard_popup_hint_line(), hint, buf);
    }

    fn desired_height(&self, _width: u16) -> u16 {
        3
    }
}

impl BottomPaneView for RenameSessionView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if matches!(key_event.kind, KeyEventKind::Release) {
            return;
        }
        match key_event {
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                let name = self.input.text().trim().to_string();
                self.tx.send(AppEvent::UpdateSessionName(name));
                self.done = true;
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.done = true;
            }
            other => {
                // Delegate to the text area for editing behavior.
                self.input.input(other);
            }
        }
    }

    fn is_complete(&self) -> bool {
        self.done
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        // Cursor appears in the input line, after the "Name: " label.
        let label_cols: u16 = 6; // "Name: "
        let y = area.y + 1; // second row
        Some((area.x + label_cols + (self.input.cursor() as u16), y))
    }
}
