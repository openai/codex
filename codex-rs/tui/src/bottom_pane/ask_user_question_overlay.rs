use std::cell::RefCell;
use std::collections::HashMap;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;
use textwrap::wrap;

use codex_core::protocol::AskUserQuestion;
use codex_core::protocol::AskUserQuestionRequestEvent;
use codex_core::protocol::AskUserQuestionResponse;
use codex_core::protocol::Op;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
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
    OtherInput,
}

fn normalize_choice_label(label: &str) -> String {
    let trimmed = label.trim_start();

    let mut chars = trimmed.char_indices().peekable();
    let mut saw_digit = false;
    let mut after_digits = 0usize;
    while let Some((idx, ch)) = chars.peek().copied()
        && ch.is_ascii_digit()
    {
        saw_digit = true;
        chars.next();
        after_digits = idx + ch.len_utf8();
    }

    if !saw_digit {
        return trimmed.to_string();
    }

    // Only strip numeric prefixes when they look like enumeration: "1) Foo", "2. Bar", "3: Baz".
    let Some((idx, ch)) = chars.peek().copied() else {
        return trimmed.to_string();
    };
    if !matches!(ch, ')' | '.' | ':') {
        return trimmed.to_string();
    }

    chars.next();
    let mut end = idx + ch.len_utf8();

    while let Some((idx, ch)) = chars.peek().copied()
        && ch.is_whitespace()
    {
        chars.next();
        end = idx + ch.len_utf8();
    }

    if end <= after_digits {
        return trimmed.to_string();
    }

    let rest = trimmed[end..].trim_start();
    if rest.is_empty() {
        trimmed.to_string()
    } else {
        rest.to_string()
    }
}

pub(crate) struct AskUserQuestionOverlay {
    id: String,
    questions: Vec<AskUserQuestion>,
    current_idx: usize,
    answers: HashMap<String, String>,

    mode: Mode,
    state: ScrollState,
    multi_select: bool,
    selected: Vec<bool>,
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    error: Option<String>,

    app_event_tx: AppEventSender,
    complete: bool,
}

impl AskUserQuestionOverlay {
    pub(crate) fn new(
        id: String,
        ev: AskUserQuestionRequestEvent,
        app_event_tx: AppEventSender,
    ) -> Self {
        let mut overlay = Self {
            id,
            questions: ev.questions,
            current_idx: 0,
            answers: HashMap::new(),
            mode: Mode::Select,
            state: ScrollState::new(),
            multi_select: false,
            selected: Vec::new(),
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            error: None,
            app_event_tx,
            complete: false,
        };
        overlay.reset_for_current_question();
        overlay
    }

    fn current_question(&self) -> Option<&AskUserQuestion> {
        self.questions.get(self.current_idx)
    }

    fn reset_for_current_question(&mut self) {
        self.mode = Mode::Select;
        self.error = None;
        self.state.reset();
        self.textarea.set_text("");
        self.textarea_state.replace(TextAreaState::default());

        let Some(q) = self.current_question() else {
            self.multi_select = false;
            self.selected.clear();
            self.state.selected_idx = None;
            return;
        };

        let multi_select = q.multi_select;
        let option_count = q.options.len();
        self.multi_select = multi_select;
        self.selected = vec![false; option_count + 1]; // + Other
        self.state.selected_idx = Some(0);
    }

    fn options_len(&self) -> usize {
        self.current_question()
            .map(|q| q.options.len() + 1)
            .unwrap_or(0)
    }

    fn is_other_idx(&self, idx: usize) -> bool {
        self.current_question()
            .map(|q| idx == q.options.len())
            .unwrap_or(false)
    }

    fn move_up(&mut self) {
        let len = self.options_len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, self.max_visible_rows());
    }

    fn move_down(&mut self) {
        let len = self.options_len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, self.max_visible_rows());
    }

    fn max_visible_rows(&self) -> usize {
        MAX_POPUP_ROWS.min(self.options_len().max(1))
    }

    fn toggle_current(&mut self) {
        let Some(idx) = self.state.selected_idx else {
            return;
        };
        if let Some(flag) = self.selected.get_mut(idx) {
            *flag = !*flag;
        }
        self.error = None;
    }

    fn select_single(&mut self) {
        let Some(idx) = self.state.selected_idx else {
            return;
        };
        self.selected.iter_mut().for_each(|s| *s = false);
        if let Some(flag) = self.selected.get_mut(idx) {
            *flag = true;
        }
        self.error = None;
    }

    fn any_selected(&self) -> bool {
        self.selected.iter().any(|s| *s)
    }

    fn other_selected(&self) -> bool {
        let Some(q) = self.current_question() else {
            return false;
        };
        self.selected.get(q.options.len()).copied().unwrap_or(false)
    }

    fn other_text(&self) -> String {
        self.textarea.text().trim().to_string()
    }

    fn confirm_selection(&mut self) {
        let Some(q) = self.current_question() else {
            self.finish_answered();
            return;
        };

        if self.multi_select {
            if !self.any_selected() {
                self.error = Some("Select at least one option.".to_string());
                return;
            }
            if self.other_selected() && self.other_text().is_empty() {
                self.mode = Mode::OtherInput;
                self.error = None;
                return;
            }
            let mut parts = Vec::new();
            for (idx, selected) in self.selected.iter().enumerate() {
                if !*selected {
                    continue;
                }
                if self.is_other_idx(idx) {
                    parts.push(self.other_text());
                } else if let Some(opt) = q.options.get(idx) {
                    parts.push(normalize_choice_label(opt.label.as_str()));
                }
            }
            self.answers.insert(q.header.clone(), parts.join(", "));
            self.advance_or_finish();
        } else {
            let Some((idx, _)) = self.selected.iter().enumerate().find(|(_, s)| **s) else {
                self.error = Some("Select an option.".to_string());
                return;
            };
            if self.is_other_idx(idx) {
                if self.other_text().is_empty() {
                    self.mode = Mode::OtherInput;
                    self.error = None;
                    return;
                }
                self.answers.insert(q.header.clone(), self.other_text());
                self.advance_or_finish();
                return;
            }
            let label = q
                .options
                .get(idx)
                .map(|o| normalize_choice_label(o.label.as_str()))
                .unwrap_or_default();
            self.answers.insert(q.header.clone(), label);
            self.advance_or_finish();
        }
    }

    fn accept_other_input(&mut self) {
        if self.other_text().is_empty() {
            self.error = Some("Other response cannot be empty.".to_string());
            return;
        }
        self.mode = Mode::Select;
        self.confirm_selection();
    }

    fn advance_or_finish(&mut self) {
        if self.current_idx + 1 >= self.questions.len() {
            self.finish_answered();
        } else {
            self.current_idx += 1;
            self.reset_for_current_question();
        }
    }

    fn finish_answered(&mut self) {
        let response = AskUserQuestionResponse::Answered {
            answers: std::mem::take(&mut self.answers),
        };
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::ResolveAskUserQuestion {
                id: self.id.clone(),
                response,
            }));
        self.complete = true;
    }

    fn finish_cancelled(&mut self) {
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::ResolveAskUserQuestion {
                id: self.id.clone(),
                response: AskUserQuestionResponse::Cancelled,
            }));
        self.complete = true;
    }

    fn build_rows(&self) -> Vec<GenericDisplayRow> {
        let Some(q) = self.current_question() else {
            return Vec::new();
        };

        let mut rows = Vec::with_capacity(q.options.len() + 1);
        for (idx, opt) in q.options.iter().enumerate() {
            rows.push(GenericDisplayRow {
                name: self.row_name(idx, opt.label.as_str()),
                display_shortcut: None,
                match_indices: None,
                description: Some(opt.description.clone()),
                wrap_indent: None,
            });
        }
        rows.push(GenericDisplayRow {
            name: self.row_name(q.options.len(), "Other"),
            display_shortcut: None,
            match_indices: None,
            description: Some("Provide custom text input.".to_string()),
            wrap_indent: None,
        });
        rows
    }

    fn row_name(&self, idx: usize, label: &str) -> String {
        let n = idx + 1;
        let label = normalize_choice_label(label);
        if self.multi_select {
            let checked = self.selected.get(idx).copied().unwrap_or(false);
            let box_mark = if checked { "[x]" } else { "[ ]" };
            format!("{n}. {box_mark} {label}")
        } else {
            format!("{n}. {label}")
        }
    }

    fn footer_hint(&self) -> Line<'static> {
        match self.mode {
            Mode::Select => {
                if self.multi_select {
                    Line::from(vec![
                        "Space".into(),
                        " toggle, ".into(),
                        key_hint::plain(KeyCode::Enter).into(),
                        " next, ".into(),
                        key_hint::plain(KeyCode::Esc).into(),
                        " cancel".into(),
                    ])
                } else {
                    Line::from(vec![
                        key_hint::plain(KeyCode::Enter).into(),
                        " choose, ".into(),
                        key_hint::plain(KeyCode::Esc).into(),
                        " cancel".into(),
                    ])
                }
            }
            Mode::OtherInput => Line::from(vec![
                key_hint::plain(KeyCode::Enter).into(),
                " submit, ".into(),
                key_hint::plain(KeyCode::Esc).into(),
                " cancel".into(),
            ]),
        }
    }

    fn header_lines(&self, width: u16) -> Vec<Line<'static>> {
        let Some(q) = self.current_question() else {
            return vec![Line::from("No questions.".dim())];
        };

        let usable_width = width.saturating_sub(4).max(1) as usize;
        let progress = format!(
            "{} ({}/{})",
            q.header,
            self.current_idx + 1,
            self.questions.len()
        );

        let mut lines = vec![Line::from(vec!["[".into(), progress.bold(), "]".into()])];

        for w in wrap(q.question.as_str(), usable_width) {
            lines.push(Line::from(w.into_owned()));
        }

        if let Some(err) = &self.error {
            lines.push(Line::from(vec!["".into()]));
            lines.push(Line::from(err.clone().red()));
        }

        lines
    }

    fn cursor_pos_for_other_input(&self, area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::OtherInput {
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

impl BottomPaneView for AskUserQuestionOverlay {
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
                    code: KeyCode::Esc, ..
                } => {
                    self.on_ctrl_c();
                }
                KeyEvent {
                    code: KeyCode::Char(' '),
                    modifiers: KeyModifiers::NONE,
                    ..
                } if self.multi_select => {
                    self.toggle_current();
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
                        && idx < self.options_len()
                    {
                        self.state.selected_idx = Some(idx);
                        self.state.ensure_visible(self.options_len(), self.max_visible_rows());
                        if self.multi_select {
                            self.toggle_current();
                        } else {
                            self.select_single();
                            self.confirm_selection();
                        }
                    }
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if self.multi_select {
                        self.confirm_selection();
                    } else {
                        self.select_single();
                        self.confirm_selection();
                    }
                }
                _ => {}
            },
            Mode::OtherInput => match key_event {
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
                    self.accept_other_input();
                }
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
        self.finish_cancelled();
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if self.mode != Mode::OtherInput {
            return false;
        }
        if pasted.is_empty() {
            return false;
        }
        self.textarea.insert_str(&pasted);
        true
    }
}

impl crate::render::renderable::Renderable for AskUserQuestionOverlay {
    fn desired_height(&self, width: u16) -> u16 {
        let header_height = self.header_lines(width).len() as u16;
        let rows_height = measure_rows_height(
            &self.build_rows(),
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
        if self.mode == Mode::OtherInput {
            total = total.saturating_add(6);
        }
        total
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.cursor_pos_for_other_input(area)
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
                let rows = self.build_rows();
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
            Mode::OtherInput => {
                let label_area = Rect {
                    x: body_area.x,
                    y: body_area.y,
                    width: body_area.width,
                    height: 1,
                };
                Paragraph::new(Line::from(vec![
                    Span::from("Other response: ".to_string()).bold(),
                    "(press Enter to submit)".dim(),
                ]))
                .render(label_area, buf);

                let input_outer = Rect {
                    x: body_area.x,
                    y: body_area.y.saturating_add(1),
                    width: body_area.width,
                    height: body_area.height.saturating_sub(1).max(1),
                };
                let textarea_rect = self.textarea_rect(input_outer);
                let mut state = self.textarea_state.borrow_mut();
                StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
                if self.textarea.text().is_empty() {
                    Paragraph::new(Line::from("Type your response…".dim()))
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
