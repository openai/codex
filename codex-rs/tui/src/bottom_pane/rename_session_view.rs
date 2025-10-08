use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;
use std::cell::RefCell;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::render::Insets;
use crate::render::RectExt;
use crate::render::renderable::Renderable;
use crate::style::user_message_style;
use crate::terminal_palette;

use super::popup_consts::standard_popup_hint_line;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

/// Simple single-line input to rename the current session.
pub(crate) struct RenameSessionView {
    app_event_tx: AppEventSender,
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    complete: bool,
}

impl RenameSessionView {
    pub(crate) fn new(app_event_tx: AppEventSender, initial: Option<String>) -> Self {
        let mut textarea = TextArea::new();
        if let Some(name) = initial {
            textarea.set_text(&name);
            textarea.set_cursor(name.len());
        }
        Self {
            app_event_tx,
            textarea,
            textarea_state: RefCell::new(TextAreaState::default()),
            complete: false,
        }
    }
}

impl BottomPaneView for RenameSessionView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.on_ctrl_c();
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Empty name clears the label.
                let text = self.textarea.text().trim().to_string();
                self.app_event_tx.send(AppEvent::UpdateSessionName(text));
                self.complete = true;
            }
            other => {
                self.textarea.input(other);
            }
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if pasted.is_empty() {
            return false;
        }
        self.textarea.insert_str(&pasted);
        true
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if area.height < 7 || area.width <= 4 {
            return None;
        }
        // Mirror render() layout: panel excludes hint; inner has 1v/2h padding.
        let panel = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(1),
        };
        let inner = panel.inset(Insets::vh(1, 2));
        // Title at inner.y, spacer at inner.y+1, gutter/input at inner.y+2
        let textarea_rect = Rect {
            x: inner.x.saturating_add(2),
            y: inner.y.saturating_add(2),
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }
}

impl Renderable for RenameSessionView {
    fn desired_height(&self, _width: u16) -> u16 {
        7
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Background panel (content rows only). Last row is the hint line.
        let panel_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(1),
        };
        Block::default()
            .style(user_message_style(terminal_palette::default_bg()))
            .render(panel_area, buf);
        let inner = panel_area.inset(Insets::vh(1, 2));

        // Title (indented by panel padding)
        let title: Line = vec!["Rename Session".bold()].into();
        Paragraph::new(title).render(
            Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: 1,
            },
            buf,
        );

        // Input gutter and line
        let gutter: Span<'static> = "▌ ".cyan();
        let input_y = inner.y.saturating_add(2);
        Paragraph::new(Line::from(vec![gutter.clone()])).render(
            Rect {
                x: inner.x,
                y: input_y,
                width: 2,
                height: 1,
            },
            buf,
        );

        // Render one-line textarea on the same row as the gutter (no extra blank row).
        if inner.width > 4 {
            let textarea_rect = Rect {
                x: inner.x.saturating_add(2),
                y: input_y,
                width: inner.width.saturating_sub(2),
                height: 1,
            };
            let mut state = self.textarea_state.borrow_mut();
            StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
            if self.textarea.text().is_empty() {
                Paragraph::new(Line::from("Enter a name (blank to clear)".dim()))
                    .render(textarea_rect, buf);
            }
        }

        // Hint line below (not covered by the panel)
        let hint_area = Rect {
            x: area.x,
            y: area.y.saturating_add(area.height.saturating_sub(1)),
            width: area.width,
            height: 1,
        };
        Clear.render(hint_area, buf);
        Paragraph::new(standard_popup_hint_line()).render(hint_area, buf);
    }
}
