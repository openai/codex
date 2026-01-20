use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::popup_consts::MAX_POPUP_ROWS;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_popup_common::GenericDisplayRow;
use crate::bottom_pane::selection_popup_common::measure_rows_height;
use crate::bottom_pane::selection_popup_common::render_rows;
use crate::key_hint;
use crate::render::renderable::Renderable;
use codex_core::protocol::AskUserQuestion;
use codex_core::protocol::Op;
use textwrap::wrap;

pub(crate) struct AskUserQuestionView {
    id: String,
    question: AskUserQuestion,
    selections: Vec<bool>,
    state: ScrollState,
    done: bool,
    app_event_tx: AppEventSender,
}

impl AskUserQuestionView {
    pub(crate) fn new(id: String, question: AskUserQuestion, app_event_tx: AppEventSender) -> Self {
        let mut state = ScrollState::new();
        state.clamp_selection(question.options.len());
        state.ensure_visible(question.options.len(), MAX_POPUP_ROWS);
        Self {
            id,
            selections: vec![false; question.options.len()],
            question,
            state,
            done: false,
            app_event_tx,
        }
    }

    fn move_up(&mut self) {
        let len = self.question.options.len();
        self.state.move_up_wrap(len);
        self.state
            .ensure_visible(len, MAX_POPUP_ROWS.min(len.max(1)));
    }

    fn move_down(&mut self) {
        let len = self.question.options.len();
        self.state.move_down_wrap(len);
        self.state
            .ensure_visible(len, MAX_POPUP_ROWS.min(len.max(1)));
    }

    fn toggle_selected(&mut self, idx: usize) {
        if let Some(selected) = self.selections.get_mut(idx) {
            *selected = !*selected;
        }
    }

    fn submit(&mut self, answers: Vec<String>) {
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::ResolveAskUserQuestion {
                id: self.id.clone(),
                answers,
            }));
        self.done = true;
    }

    fn submit_single(&mut self, idx: Option<usize>) {
        let answers = idx
            .and_then(|idx| self.question.options.get(idx))
            .map(|opt| vec![opt.label.clone()])
            .unwrap_or_default();
        self.submit(answers);
    }

    fn submit_multi(&mut self) {
        let answers = self
            .question
            .options
            .iter()
            .zip(self.selections.iter())
            .filter_map(|(opt, selected)| selected.then(|| opt.label.clone()))
            .collect();
        self.submit(answers);
    }

    fn header_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        if let Some(header) = self.question.header.clone() {
            lines.push(Line::from(header.cyan().bold()));
        }
        let question_text = format!("Question: {}", self.question.question);
        for line in wrap(&question_text, width.max(1) as usize) {
            lines.push(Line::from(line.to_string().bold()));
        }
        lines.push(Line::from(""));
        lines
    }

    fn footer_hint(&self) -> Line<'static> {
        if self.question.multi_select {
            Line::from(vec![
                "Press ".into(),
                key_hint::plain(KeyCode::Char(' ')).into(),
                " to toggle, ".into(),
                key_hint::plain(KeyCode::Enter).into(),
                " to submit or ".into(),
                key_hint::plain(KeyCode::Esc).into(),
                " to cancel".into(),
            ])
        } else {
            standard_popup_hint_line()
        }
    }

    fn build_rows(&self) -> Vec<GenericDisplayRow> {
        self.question
            .options
            .iter()
            .enumerate()
            .map(|(idx, opt)| {
                let name = if self.question.multi_select {
                    let marker = if *self.selections.get(idx).unwrap_or(&false) {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    format!("{marker} {}. {}", idx + 1, opt.label)
                } else {
                    format!("{}. {}", idx + 1, opt.label)
                };
                GenericDisplayRow {
                    name,
                    description: Some(opt.description.clone()),
                    display_shortcut: None,
                    match_indices: None,
                    wrap_indent: None,
                }
            })
            .collect()
    }
}

impl BottomPaneView for AskUserQuestionView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
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
            } /* ^P */ => self.move_up(),
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
            } /* ^N */ => self.move_down(),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_down(),
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            } if self.question.multi_select => {
                if let Some(idx) = self.state.selected_idx {
                    self.toggle_selected(idx);
                }
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            } if modifiers.is_empty() => {
                if let Some(idx) = c
                    .to_digit(10)
                    .map(|d| d as usize)
                    .and_then(|d| d.checked_sub(1))
                    && idx < self.question.options.len()
                {
                    if self.question.multi_select {
                        self.toggle_selected(idx);
                    } else {
                        self.submit_single(Some(idx));
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if self.question.multi_select {
                    self.submit_multi();
                } else {
                    self.submit_single(self.state.selected_idx);
                }
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.submit(Vec::new());
            }
            _ => {}
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        if self.done {
            return CancellationEvent::Handled;
        }
        self.submit(Vec::new());
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.done
    }
}

impl Renderable for AskUserQuestionView {
    fn desired_height(&self, width: u16) -> u16 {
        let header_lines = self.header_lines(width);
        let rows = self.build_rows();
        let rows_height = measure_rows_height(&rows, &self.state, MAX_POPUP_ROWS, width);
        let footer_height = 1u16;
        header_lines.len() as u16 + rows_height + footer_height
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let header_lines = self.header_lines(area.width);
        let header_height = header_lines.len() as u16;
        let footer_height = 1u16;
        let available_list_height = area
            .height
            .saturating_sub(header_height)
            .saturating_sub(footer_height);

        let mut y = area.y;
        for line in header_lines {
            if y >= area.y + area.height {
                break;
            }
            line.render(
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
            y = y.saturating_add(1);
        }

        if available_list_height > 0 {
            let list_area = Rect {
                x: area.x,
                y,
                width: area.width,
                height: available_list_height,
            };
            let rows = self.build_rows();
            render_rows(
                list_area,
                buf,
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                "No options",
            );
        }

        if area.height >= footer_height {
            let footer_area = Rect {
                x: area.x,
                y: area.y + area.height - footer_height,
                width: area.width,
                height: footer_height,
            };
            let line = self.footer_hint();
            line.render(footer_area, buf);
        }
    }

    fn cursor_pos(&self, _area: Rect) -> Option<(u16, u16)> {
        None
    }
}
