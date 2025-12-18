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
use super::textarea::TextArea;
use super::textarea::TextAreaState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Select,
    FeedbackInput,
}

const MAX_PLAN_APPROVAL_OVERLAY_ROWS: u16 = 22;
const DEFAULT_PLAN_APPROVAL_VISIBLE_LINES: u16 = 12;
const FEEDBACK_BLOCK_HEIGHT: u16 = 8;

pub(crate) struct PlanApprovalOverlay {
    id: String,
    proposal: PlanProposal,
    mode: Mode,
    scroll_top: usize,
    selected_action: usize,
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
        Self {
            id,
            proposal: ev.proposal,
            mode: Mode::Select,
            scroll_top: 0,
            selected_action: 0,
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            error: None,
            app_event_tx,
            complete: false,
        }
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
        match self.selected_action {
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
                "↑/↓ ".into(),
                "scroll".bold(),
                ", ".into(),
                "←/→ ".into(),
                "action".bold(),
                ", ".into(),
                key_hint::plain(KeyCode::Enter).into(),
                " select, ".into(),
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

    fn plan_lines(&self, width: u16) -> Vec<Line<'static>> {
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

        let explanation = self
            .proposal
            .plan
            .explanation
            .as_deref()
            .unwrap_or_default()
            .trim();
        if !explanation.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from("Explanation:".bold()));
            for raw_line in explanation.lines() {
                let raw_line = raw_line.trim_end();
                if raw_line.trim().is_empty() {
                    lines.push(Line::from(""));
                    continue;
                }
                for w in wrap(raw_line, usable_width) {
                    lines.push(Line::from(vec!["  ".into(), w.into_owned().into()]));
                }
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

    fn action_bar(&self) -> Line<'static> {
        let selected = Style::default().cyan().bold();
        let normal = Style::default().dim();

        let approve_style = if self.selected_action == 0 {
            selected
        } else {
            normal
        };
        let revise_style = if self.selected_action == 1 {
            selected
        } else {
            normal
        };
        let reject_style = if self.selected_action == 2 {
            selected
        } else {
            normal
        };

        Line::from(vec![
            Span::from("[1] Approve").set_style(approve_style),
            "  ".into(),
            Span::from("[2] Revise").set_style(revise_style),
            "  ".into(),
            Span::from("[3] Reject").set_style(reject_style),
        ])
    }

    fn move_action_left(&mut self) {
        self.selected_action = self.selected_action.saturating_sub(1);
    }

    fn move_action_right(&mut self) {
        self.selected_action = (self.selected_action + 1).min(2);
    }

    fn scroll_up(&mut self) {
        self.scroll_top = self.scroll_top.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        self.scroll_top = self.scroll_top.saturating_add(1);
    }

    fn page_up(&mut self) {
        self.scroll_top = self.scroll_top.saturating_sub(8);
    }

    fn page_down(&mut self) {
        self.scroll_top = self.scroll_top.saturating_add(8);
    }

    fn scroll_home(&mut self) {
        self.scroll_top = 0;
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
                    code: KeyCode::Left,
                    ..
                } => self.move_action_left(),
                KeyEvent {
                    code: KeyCode::Right,
                    ..
                } => self.move_action_right(),
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
                } => self.scroll_up(),
                KeyEvent {
                    code: KeyCode::Char('k'),
                    modifiers: KeyModifiers::NONE,
                    ..
                } => self.scroll_up(),
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
                } => self.scroll_down(),
                KeyEvent {
                    code: KeyCode::Char('j'),
                    modifiers: KeyModifiers::NONE,
                    ..
                } => self.scroll_down(),
                KeyEvent {
                    code: KeyCode::PageUp,
                    ..
                } => self.page_up(),
                KeyEvent {
                    code: KeyCode::PageDown,
                    ..
                } => self.page_down(),
                KeyEvent {
                    code: KeyCode::Home,
                    ..
                } => self.scroll_home(),
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
                        && idx <= 2
                    {
                        self.selected_action = idx;
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
        let plan_lines = self.plan_lines(width);
        let plan_height = (plan_lines.len() as u16).min(DEFAULT_PLAN_APPROVAL_VISIBLE_LINES);

        let mut total = 2 // outer padding
            + 1 // action bar
            + 1 // footer hint
            + plan_height.max(4);
        if self.mode == Mode::FeedbackInput {
            total = total.saturating_add(FEEDBACK_BLOCK_HEIGHT);
        }
        total.clamp(8, MAX_PLAN_APPROVAL_OVERLAY_ROWS)
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

        match self.mode {
            Mode::Select => {
                let [plan_area, actions_area] =
                    Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inset);

                let plan_lines = self.plan_lines(plan_area.width);
                let max_scroll = plan_lines.len().saturating_sub(plan_area.height as usize);
                let scroll = self.scroll_top.min(max_scroll) as u16;
                Paragraph::new(plan_lines)
                    .scroll((scroll, 0))
                    .render(plan_area, buf);

                self.action_bar().render(actions_area, buf);
            }
            Mode::FeedbackInput => {
                let [plan_area, feedback_area] = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(FEEDBACK_BLOCK_HEIGHT),
                ])
                .areas(inset);

                let plan_lines = self.plan_lines(plan_area.width);
                let max_scroll = plan_lines.len().saturating_sub(plan_area.height as usize);
                let scroll = self.scroll_top.min(max_scroll) as u16;
                Paragraph::new(plan_lines)
                    .scroll((scroll, 0))
                    .render(plan_area, buf);

                let label_area = Rect {
                    x: feedback_area.x,
                    y: feedback_area.y,
                    width: feedback_area.width,
                    height: 1,
                };
                Paragraph::new(Line::from(vec![
                    Span::from("Feedback: ").bold(),
                    "(press Enter to submit)".dim(),
                ]))
                .render(label_area, buf);

                if let Some(err) = &self.error {
                    let err_area = Rect {
                        x: feedback_area.x,
                        y: feedback_area.y.saturating_add(1),
                        width: feedback_area.width,
                        height: 1,
                    };
                    Line::from(err.clone().red()).render(err_area, buf);
                }

                let input_outer = Rect {
                    x: feedback_area.x,
                    y: feedback_area.y.saturating_add(2),
                    width: feedback_area.width,
                    height: feedback_area.height.saturating_sub(2).max(1),
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
