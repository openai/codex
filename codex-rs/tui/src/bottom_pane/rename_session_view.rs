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
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::bottom_pane::textarea::TextArea;
use crate::bottom_pane::textarea::TextAreaState;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::style::user_message_style;
use crate::terminal_palette;

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
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Panel background + hint line layout
        let [content_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        Block::default()
            .style(user_message_style(terminal_palette::default_bg()))
            .render(content_area, buf);

        // Inside panel: header (title + subtitle), spacer, then input row
        let [header_area, _, input_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(content_area.inset(Insets::vh(1, 2)));

        Paragraph::new(vec![
            Line::from("Rename Session".bold().cyan()),
            Line::from("Set a short display name for this session".dim()),
        ])
        .render(header_area, buf);

        // Input label + field
        let label = "Name: ";
        WidgetRef::render_ref(&Line::from(Span::from(label).dim()), input_area, buf);
        let mut text_rect = input_area;
        let label_width = label.len() as u16;
        if text_rect.width > label_width {
            text_rect.x += label_width;
            text_rect.width = text_rect.width.saturating_sub(label_width);
        }
        let mut state = self.input_state.borrow_mut();
        ratatui::widgets::StatefulWidgetRef::render_ref(&(&self.input), text_rect, buf, &mut state);

        // Footer hint, outside the panel
        WidgetRef::render_ref(&standard_popup_hint_line().dim(), footer_area, buf);
    }

    fn desired_height(&self, _width: u16) -> u16 {
        5
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
        // Match panel layout: content inset + input row placement
        let [content_area, _] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        let inset = content_area.inset(Insets::vh(1, 2));
        let label_cols: u16 = 6; // "Name: "
        let input_y = inset.y + 3; // header(2) + spacer(1)
        let x = inset.x + label_cols + (self.input.cursor() as u16);
        Some((x, input_y))
    }
}
