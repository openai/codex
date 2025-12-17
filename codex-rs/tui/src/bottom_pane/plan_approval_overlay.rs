use std::cell::RefCell;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

use codex_core::protocol::Op;
use codex_core::protocol::PlanApprovalRequestEvent;
use codex_core::protocol::PlanApprovalResponse;
use codex_core::protocol::PlanProposal;
use codex_protocol::plan_tool::PlanItemArg;
use codex_protocol::plan_tool::StepStatus;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::line_utils::prefix_lines;
use crate::style::user_message_style;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Select,
    FeedbackInput,
}

pub(crate) struct PlanApprovalOverlay {
    id: String,
    proposal: PlanProposal,
    mode: Mode,
    state: ScrollState,
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    error: Option<String>,
    app_event_tx: AppEventSender,
    complete: bool,
}

impl PlanApprovalOverlay {
    pub(crate) fn new(
        id: String,
        ev: PlanApprovalRequestEvent,
        app_event_tx: AppEventSender,
    ) -> Self {
        let mut state = ScrollState::new();
        state.selected_idx = Some(0);
        Self {
            id,
            proposal: ev.proposal,
            mode: Mode::Select,
            state,
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            error: None,
            app_event_tx,
            complete: false,
        }
    }

    fn option_rows(&self) -> Vec<GenericDisplayRow> {
        vec![
            GenericDisplayRow {
                name: "1. Approve plan".to_string(),
                display_shortcut: None,
                match_indices: None,
                description: Some("Accept this plan and proceed.".to_string()),
                wrap_indent: None,
            },
            GenericDisplayRow {
                name: "2. Revise plan".to_string(),
                display_shortcut: None,
                match_indices: None,
                description: Some("Request changes and provide feedback.".to_string()),
                wrap_indent: None,
            },
            GenericDisplayRow {
                name: "3. Reject plan".to_string(),
                display_shortcut: None,
                match_indices: None,
                description: Some("Reject and stop plan mode.".to_string()),
                wrap_indent: None,
            },
        ]
    }

    fn max_visible_rows(&self) -> usize {
        MAX_POPUP_ROWS
    }

    fn move_up(&mut self) {
        let len = self.option_rows().len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, self.max_visible_rows());
    }

    fn move_down(&mut self) {
        let len = self.option_rows().len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, self.max_visible_rows());
    }

    fn current_selection(&self) -> Option<usize> {
        self.state.selected_idx
    }

    fn finish(&mut self, response: PlanApprovalResponse) {
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::ResolvePlanApproval {
                id: self.id.clone(),
                response,
            }));
        self.complete = true;
    }

    fn other_text(&self) -> String {
        self.textarea.text().trim().to_string()
    }

    fn accept_selection(&mut self) {
        let Some(idx) = self.current_selection() else {
            self.error = Some("Select an option.".to_string());
            return;
        };

        match idx {
            0 => self.finish(PlanApprovalResponse::Approved),
            1 => {
                self.mode = Mode::FeedbackInput;
                self.error = None;
            }
            _ => self.finish(PlanApprovalResponse::Rejected),
        }
    }

    fn accept_feedback(&mut self) {
        let feedback = self.other_text();
        if feedback.is_empty() {
            self.error = Some("Feedback cannot be empty.".to_string());
            return;
        }
        self.finish(PlanApprovalResponse::Revised { feedback });
    }

    fn footer_hint(&self) -> Line<'static> {
        match self.mode {
            Mode::Select => Line::from(vec![
                key_hint::plain(KeyCode::Enter).into(),
                " choose, ".into(),
                key_hint::plain(KeyCode::Esc).into(),
                " reject".into(),
            ]),
            Mode::FeedbackInput => Line::from(vec![
                key_hint::plain(KeyCode::Enter).into(),
                " submit, ".into(),
                key_hint::plain(KeyCode::Esc).into(),
                " back".into(),
            ]),
        }
    }

    fn header_lines(&self, width: u16) -> Vec<Line<'static>> {
        let usable_width = width.saturating_sub(4).max(1) as usize;
        let mut lines = Vec::new();

        lines.push(
            vec![
                "[".into(),
                "Plan".bold(),
                "] ".into(),
                self.proposal.title.clone().bold(),
            ]
            .into(),
        );

        let summary = self.proposal.summary.trim();
        if !summary.is_empty() {
            lines.push(Line::from(""));
            for w in wrap(summary, usable_width) {
                lines.push(Line::from(vec!["Summary: ".dim(), w.into_owned().into()]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from("Steps:".bold()));

        let mut step_lines = Vec::new();
        if self.proposal.plan.plan.is_empty() {
            step_lines.push(Line::from("(no steps provided)".dim().italic()));
        } else {
            for PlanItemArg { step, status } in &self.proposal.plan.plan {
                step_lines.extend(render_step_lines(width, status, step.as_str()));
            }
        }
        lines.extend(prefix_lines(step_lines, "  ".into(), "  ".into()));

        lines
    }

    fn cursor_pos_for_feedback(&self, area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::FeedbackInput {
            return None;
        }
        if area.height < 2 || area.width <= 2 {
            return None;
        }
        let textarea_rect = self.textarea_rect(area);
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }

    fn textarea_rect(&self, area: Rect) -> Rect {
        let inset = area.inset(Insets::vh(1, 2));
        Rect {
            x: inset.x,
            y: inset.y,
            width: inset.width,
            height: inset.height.clamp(1, 5),
        }
    }
}

impl BottomPaneView for PlanApprovalOverlay {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self.mode {
            Mode::Select => match key_event {
                KeyEvent {
                    code: KeyCode::Up, ..
                }
                | KeyEvent {
                    code: KeyCode::Char('p'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('\u{0010}'),
                    modifiers: KeyModifiers::NONE,
                    ..
                } => self.move_up(),
                KeyEvent {
                    code: KeyCode::Char('k'),
                    modifiers: KeyModifiers::NONE,
                    ..
                } => self.move_up(),
                KeyEvent {
                    code: KeyCode::Down,
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('n'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('\u{000e}'),
                    modifiers: KeyModifiers::NONE,
                    ..
                } => self.move_down(),
                KeyEvent {
                    code: KeyCode::Char('j'),
                    modifiers: KeyModifiers::NONE,
                    ..
                } => self.move_down(),
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    self.finish(PlanApprovalResponse::Rejected);
                }
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers,
                    ..
                } if !modifiers.contains(KeyModifiers::CONTROL)
                    && !modifiers.contains(KeyModifiers::ALT) =>
                {
                    if let Some(idx) = c
                        .to_digit(10)
                        .map(|d| d as usize)
                        .and_then(|d| d.checked_sub(1))
                        && idx < self.option_rows().len()
                    {
                        self.state.selected_idx = Some(idx);
                        self.state
                            .ensure_visible(self.option_rows().len(), self.max_visible_rows());
                        self.accept_selection();
                    }
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => self.accept_selection(),
                _ => {}
            },
            Mode::FeedbackInput => match key_event {
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    self.mode = Mode::Select;
                    self.error = None;
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => self.accept_feedback(),
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    self.textarea.input(key_event);
                }
                other => {
                    self.textarea.input(other);
                }
            },
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.finish(PlanApprovalResponse::Rejected);
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if self.mode != Mode::FeedbackInput {
            return false;
        }
        if pasted.is_empty() {
            return false;
        }
        self.textarea.insert_str(&pasted);
        true
    }
}

impl crate::render::renderable::Renderable for PlanApprovalOverlay {
    fn desired_height(&self, width: u16) -> u16 {
        let header_height = self.header_lines(width).len() as u16;
        let rows_height = measure_rows_height(
            &self.option_rows(),
            &self.state,
            MAX_POPUP_ROWS,
            width.saturating_sub(1).max(1),
        );
        let footer_height = 1u16;

        let mut total = header_height
            .saturating_add(1)
            .saturating_add(rows_height)
            .saturating_add(footer_height)
            .saturating_add(2);
        if self.mode == Mode::FeedbackInput {
            total = total.saturating_add(6);
        }
        total
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.cursor_pos_for_feedback(area)
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

        let header_lines = self.header_lines(inset.width);
        let header_height = header_lines.len() as u16;
        let [header_area, body_area] =
            Layout::vertical([Constraint::Length(header_height), Constraint::Fill(1)]).areas(inset);
        Paragraph::new(header_lines).render(header_area, buf);

        match self.mode {
            Mode::Select => {
                let rows = self.option_rows();
                let rows_height = measure_rows_height(
                    &rows,
                    &self.state,
                    MAX_POPUP_ROWS,
                    body_area.width.saturating_sub(1).max(1),
                );
                let list_area = Rect {
                    x: body_area.x,
                    y: body_area.y,
                    width: body_area.width,
                    height: rows_height.min(body_area.height),
                };
                render_rows(
                    list_area,
                    buf,
                    &rows,
                    &self.state,
                    MAX_POPUP_ROWS,
                    "no options",
                );
            }
            Mode::FeedbackInput => {
                let label_area = Rect {
                    x: body_area.x,
                    y: body_area.y,
                    width: body_area.width,
                    height: 1,
                };
                Paragraph::new(Line::from(vec![
                    Span::from("Feedback: ".to_string()).bold(),
                    "(press Enter to submit)".dim(),
                ]))
                .render(label_area, buf);

                if let Some(err) = &self.error {
                    let err_area = Rect {
                        x: body_area.x,
                        y: body_area.y.saturating_add(1),
                        width: body_area.width,
                        height: 1,
                    };
                    Line::from(err.clone().red()).render(err_area, buf);
                }

                let input_outer = Rect {
                    x: body_area.x,
                    y: body_area.y.saturating_add(2),
                    width: body_area.width,
                    height: body_area.height.saturating_sub(2).max(1),
                };
                let textarea_rect = self.textarea_rect(input_outer);
                let mut state = self.textarea_state.borrow_mut();
                StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
                if self.textarea.text().is_empty() {
                    Paragraph::new(Line::from("Type your feedback…".dim()))
                        .render(textarea_rect, buf);
                }
            }
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

fn render_step_lines(width: u16, status: &StepStatus, text: &str) -> Vec<Line<'static>> {
    let (box_str, step_style) = match status {
        StepStatus::Completed => ("[x] ", Style::default().crossed_out().dim()),
        StepStatus::InProgress => ("[~] ", Style::default().cyan().bold()),
        StepStatus::Pending => ("[ ] ", Style::default().dim()),
    };
    let wrap_width = (width as usize)
        .saturating_sub(4)
        .saturating_sub(box_str.width())
        .max(1);
    let parts = wrap(text, wrap_width);
    let lines: Vec<Line<'static>> = parts
        .into_iter()
        .map(|s| Line::from(Span::from(s.into_owned()).set_style(step_style)))
        .collect();
    prefix_lines(lines, box_str.into(), "    ".into())
}
