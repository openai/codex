use std::cell::RefCell;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;

use codex_core::protocol::Op;
use codex_core::protocol::PlanRequest;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::style::user_message_style;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

pub(crate) struct PlanRequestOverlay {
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    error: Option<String>,
    app_event_tx: AppEventSender,
    complete: bool,
}

impl PlanRequestOverlay {
    pub(crate) fn new(app_event_tx: AppEventSender) -> Self {
        Self {
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            error: None,
            app_event_tx,
            complete: false,
        }
    }

    fn goal_text(&self) -> String {
        self.textarea.text().trim().to_string()
    }

    fn submit(&mut self) {
        let goal = self.goal_text();
        if goal.is_empty() {
            self.error = Some("Goal cannot be empty.".to_string());
            return;
        }
        self.app_event_tx.send(AppEvent::CodexOp(Op::Plan {
            plan_request: PlanRequest { goal },
        }));
        self.complete = true;
    }

    fn footer_hint(&self) -> Line<'static> {
        Line::from(vec![
            key_hint::plain(KeyCode::Enter).into(),
            " submit, ".into(),
            key_hint::plain(KeyCode::Esc).into(),
            " cancel".into(),
        ])
    }
}

impl BottomPaneView for PlanRequestOverlay {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.complete = true;
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.submit(),
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                self.textarea.input(key_event);
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
}

impl crate::render::renderable::Renderable for PlanRequestOverlay {
    fn desired_height(&self, _width: u16) -> u16 {
        10
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if area.height < 2 || area.width <= 2 {
            return None;
        }
        let textarea_rect = self.textarea_rect(area);
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        Clear.render(area, buf);
        Block::default()
            .style(user_message_style())
            .render(area, buf);

        let [content_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        let inset = content_area.inset(Insets::vh(1, 2));

        let header_lines = {
            let mut lines = vec![Line::from(vec![
                "[".into(),
                "Plan Mode".bold(),
                "] ".into(),
                "Describe what you want to do.".into(),
            ])];
            if let Some(err) = &self.error {
                lines.push(Line::from(""));
                lines.push(Line::from(err.clone().red()));
            }
            lines
        };
        let header_height = header_lines.len() as u16;
        let [header_area, body_area] =
            Layout::vertical([Constraint::Length(header_height), Constraint::Fill(1)]).areas(inset);
        Paragraph::new(header_lines).render(header_area, buf);

        let textarea_rect = self.textarea_rect(body_area);
        let mut state = self.textarea_state.borrow_mut();
        StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
        if self.textarea.text().is_empty() {
            Paragraph::new(Line::from(
                "e.g. \"Implement Plan Mode for Codex CLI\"".dim(),
            ))
            .render(textarea_rect, buf);
        }

        let hint_area = Rect {
            x: footer_area.x.saturating_add(2),
            y: footer_area.y,
            width: footer_area.width.saturating_sub(2),
            height: 1,
        };
        self.footer_hint().dim().render(hint_area, buf);
    }
}

impl PlanRequestOverlay {
    fn textarea_rect(&self, area: Rect) -> Rect {
        let inset = area.inset(Insets::vh(1, 2));
        Rect {
            x: inset.x,
            y: inset.y,
            width: inset.width,
            height: inset.height.clamp(1, 6),
        }
    }
}
