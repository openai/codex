//! Ask-user-question overlay state machine.
//!
//! This is a bottom-pane modal that mimics Claude Code's
//! "AskUserQuestionTool" UX:
//! - A tab per question plus a Submit tab.
//! - Single-choice questions advance on selection (except custom option).
//! - Multiple-choice questions advance only via Next (Enter).
//! - Each question may include one custom option whose label is user-provided.
use std::collections::HashMap;
use std::collections::VecDeque;

use codex_core::protocol::Op;
use codex_protocol::ask_user_question::AskUserQuestionEvent;
use codex_protocol::ask_user_question::AskUserQuestionKind;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::textarea::TextArea;
use crate::history_cell::AskUserQuestionAnswersHistoryCell;

mod render;

const CUSTOM_PLACEHOLDER_DEFAULT: &str = "Type something.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Options,
    Custom,
    SubmitButtons,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubmitFocus {
    Submit,
    Cancel,
}

struct CustomEntry {
    text: TextArea,
}

impl CustomEntry {
    fn new() -> Self {
        Self {
            text: TextArea::new(),
        }
    }
}

struct QuestionState {
    highlight: ScrollState,
    selected_single: Option<usize>,
    selected_multi: Vec<bool>,
    custom_idx: Option<usize>,
    custom_entry: Option<CustomEntry>,
}

pub(crate) struct AskUserQuestionOverlay {
    app_event_tx: AppEventSender,
    request: AskUserQuestionEvent,
    queue: VecDeque<AskUserQuestionEvent>,
    state: Vec<QuestionState>,
    active_tab: usize,
    focus: Focus,
    submit_focus: SubmitFocus,
    done: bool,
}

impl AskUserQuestionOverlay {
    pub(crate) fn new(request: AskUserQuestionEvent, app_event_tx: AppEventSender) -> Self {
        let mut overlay = Self {
            app_event_tx,
            request,
            queue: VecDeque::new(),
            state: Vec::new(),
            active_tab: 0,
            focus: Focus::Options,
            submit_focus: SubmitFocus::Submit,
            done: false,
        };
        overlay.reset_for_request();
        overlay
    }

    fn reset_for_request(&mut self) {
        self.state = self
            .request
            .questions
            .iter()
            .map(|question| {
                let mut highlight = ScrollState::new();
                highlight.selected_idx = (!question.options.is_empty()).then_some(0);
                let custom_idx = question.options.iter().position(|opt| opt.custom);
                QuestionState {
                    highlight,
                    selected_single: None,
                    selected_multi: vec![false; question.options.len()],
                    custom_idx,
                    custom_entry: custom_idx.map(|_| CustomEntry::new()),
                }
            })
            .collect();

        self.active_tab = 0;
        self.focus = Focus::Options;
        self.submit_focus = SubmitFocus::Submit;
    }

    fn is_submit_tab(&self) -> bool {
        self.active_tab >= self.request.questions.len()
    }

    fn tab_count(&self) -> usize {
        self.request.questions.len().saturating_add(1)
    }

    fn current_question_index(&self) -> Option<usize> {
        (!self.is_submit_tab()).then_some(self.active_tab)
    }

    fn current_question(
        &self,
    ) -> Option<&codex_protocol::ask_user_question::AskUserQuestionQuestion> {
        let idx = self.current_question_index()?;
        self.request.questions.get(idx)
    }

    fn current_state(&self) -> Option<&QuestionState> {
        let idx = self.current_question_index()?;
        self.state.get(idx)
    }

    fn current_kind(&self) -> AskUserQuestionKind {
        self.current_question()
            .map(|q| q.kind)
            .unwrap_or(AskUserQuestionKind::SingleChoice)
    }

    fn move_tab(&mut self, next: bool) {
        let len = self.tab_count().max(1);
        let offset = if next { 1 } else { len.saturating_sub(1) };
        self.active_tab = (self.active_tab + offset) % len;
        self.focus = if self.is_submit_tab() {
            Focus::SubmitButtons
        } else {
            Focus::Options
        };

        if let Some(idx) = self.current_question_index()
            && let Some(question_state) = self.state.get_mut(idx)
        {
            if let Some(sel) = question_state.selected_single {
                question_state.highlight.selected_idx = Some(sel);
            } else {
                question_state
                    .highlight
                    .clamp_selection(self.request.questions[idx].options.len());
            }
        }
    }

    fn select_single(&mut self, idx: usize) {
        let Some(q_idx) = self.current_question_index() else {
            return;
        };
        let question = &self.request.questions[q_idx];
        let Some(question_state) = self.state.get_mut(q_idx) else {
            return;
        };
        if idx >= question.options.len() {
            return;
        }

        question_state.selected_single = Some(idx);
        question_state.highlight.selected_idx = Some(idx);
        if question.options.get(idx).is_some_and(|opt| opt.custom) {
            self.focus = Focus::Custom;
            return;
        }
        self.move_to_next_after_answer();
    }

    fn toggle_multiple(&mut self, idx: usize) {
        let Some(q_idx) = self.current_question_index() else {
            return;
        };
        let question = &self.request.questions[q_idx];
        let Some(question_state) = self.state.get_mut(q_idx) else {
            return;
        };
        if idx >= question.options.len() {
            return;
        }
        if let Some(selected) = question_state.selected_multi.get_mut(idx) {
            *selected = !*selected;
        }
        if question.options.get(idx).is_some_and(|opt| opt.custom)
            && question_state
                .selected_multi
                .get(idx)
                .copied()
                .unwrap_or(false)
        {
            self.focus = Focus::Custom;
        }
    }

    fn move_to_next_after_answer(&mut self) {
        if self.is_submit_tab() {
            return;
        }
        if self.active_tab + 1 >= self.tab_count() {
            self.active_tab = self.tab_count().saturating_sub(1);
        } else {
            self.active_tab += 1;
        }
        self.focus = if self.is_submit_tab() {
            Focus::SubmitButtons
        } else {
            Focus::Options
        };
    }

    fn custom_is_selected(&self, q_idx: usize) -> bool {
        let question = &self.request.questions[q_idx];
        let question_state = &self.state[q_idx];
        let Some(custom_idx) = question_state.custom_idx else {
            return false;
        };
        match question.kind {
            AskUserQuestionKind::SingleChoice => question_state.selected_single == Some(custom_idx),
            AskUserQuestionKind::MultipleChoice => question_state
                .selected_multi
                .get(custom_idx)
                .copied()
                .unwrap_or(false),
        }
    }

    fn custom_text(&self, q_idx: usize) -> Option<&str> {
        self.state
            .get(q_idx)?
            .custom_entry
            .as_ref()
            .map(|entry| entry.text.text())
    }

    fn answer_for_question(&self, q_idx: usize) -> Vec<String> {
        let question = &self.request.questions[q_idx];
        let question_state = &self.state[q_idx];

        let custom_idx = question_state.custom_idx;
        let custom_text = question_state
            .custom_entry
            .as_ref()
            .map(|e| e.text.text().trim().to_string())
            .unwrap_or_default();

        match question.kind {
            AskUserQuestionKind::SingleChoice => {
                let Some(selected) = question_state.selected_single else {
                    return Vec::new();
                };
                if custom_idx == Some(selected) {
                    if custom_text.is_empty() {
                        return Vec::new();
                    }
                    return vec![custom_text];
                }
                question
                    .options
                    .get(selected)
                    .map(|opt| vec![opt.label.clone()])
                    .unwrap_or_default()
            }
            AskUserQuestionKind::MultipleChoice => {
                let mut out = Vec::new();
                for (idx, opt) in question.options.iter().enumerate() {
                    if !question_state
                        .selected_multi
                        .get(idx)
                        .copied()
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    if opt.custom {
                        if !custom_text.is_empty() {
                            out.push(custom_text.clone());
                        }
                    } else {
                        out.push(opt.label.clone());
                    }
                }
                out
            }
        }
    }

    fn question_answered(&self, q_idx: usize) -> bool {
        !self.answer_for_question(q_idx).is_empty()
    }

    fn format_answers_for_chat(answers: &[String]) -> String {
        match answers {
            [] => String::new(),
            [one] => one.clone(),
            [a, b] => format!("{a} + {b}"),
            _ => answers.join(", "),
        }
    }

    fn submit_answers(&mut self) {
        let mut items: Vec<(String, String)> = Vec::new();
        let mut answers = HashMap::new();
        for (idx, question) in self.request.questions.iter().enumerate() {
            let answer_list = self.answer_for_question(idx);
            items.push((
                question.question.clone(),
                Self::format_answers_for_chat(&answer_list),
            ));
            answers.insert(
                question.id.clone(),
                RequestUserInputAnswer {
                    answers: answer_list,
                },
            );
        }

        self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
            AskUserQuestionAnswersHistoryCell::new(items),
        )));

        self.app_event_tx
            .send(AppEvent::CodexOp(Op::UserInputAnswer {
                id: self.request.turn_id.clone(),
                response: RequestUserInputResponse { answers },
            }));

        if let Some(next) = self.queue.pop_front() {
            self.request = next;
            self.reset_for_request();
        } else {
            self.done = true;
        }
    }

    fn cancel(&mut self) {
        self.app_event_tx.send(AppEvent::CodexOp(Op::Interrupt));
        self.done = true;
    }

    fn digit_to_index(ch: char) -> Option<usize> {
        if !ch.is_ascii_digit() {
            return None;
        }
        let n = ch.to_digit(10)? as usize;
        (n >= 1).then_some(n - 1)
    }
}

impl BottomPaneView for AskUserQuestionOverlay {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        // Global tab navigation.
        match key_event.code {
            KeyCode::Left => {
                self.move_tab(false);
                return;
            }
            KeyCode::Right => {
                self.move_tab(true);
                return;
            }
            _ => {}
        }

        if self.is_submit_tab() {
            match key_event.code {
                KeyCode::Tab => {
                    self.submit_focus = match self.submit_focus {
                        SubmitFocus::Submit => SubmitFocus::Cancel,
                        SubmitFocus::Cancel => SubmitFocus::Submit,
                    };
                }
                KeyCode::Left | KeyCode::Right => {
                    self.submit_focus = match self.submit_focus {
                        SubmitFocus::Submit => SubmitFocus::Cancel,
                        SubmitFocus::Cancel => SubmitFocus::Submit,
                    };
                }
                KeyCode::Enter => match self.submit_focus {
                    SubmitFocus::Submit => self.submit_answers(),
                    SubmitFocus::Cancel => self.cancel(),
                },
                KeyCode::Char('s') if key_event.modifiers == KeyModifiers::NONE => {
                    self.submit_answers();
                }
                KeyCode::Char('c') if key_event.modifiers == KeyModifiers::NONE => {
                    self.cancel();
                }
                _ => {}
            }
            return;
        }

        let kind = self.current_kind();
        let Some(q_idx) = self.current_question_index() else {
            return;
        };

        if matches!(self.focus, Focus::Custom) {
            if matches!(key_event.code, KeyCode::Enter) {
                self.move_to_next_after_answer();
                return;
            }
            if let Some(custom) = self.state[q_idx].custom_entry.as_mut() {
                custom.text.input(key_event);
            }
            return;
        }

        let options_len = self.request.questions[q_idx].options.len();

        match key_event.code {
            KeyCode::Up => {
                if let Some(question_state) = self.state.get_mut(q_idx) {
                    question_state.highlight.move_up_wrap(options_len);
                }
            }
            KeyCode::Down => {
                if let Some(question_state) = self.state.get_mut(q_idx) {
                    question_state.highlight.move_down_wrap(options_len);
                }
            }
            KeyCode::Char(' ') => {
                let idx = self.state.get(q_idx).and_then(|s| s.highlight.selected_idx);
                if let Some(idx) = idx {
                    match kind {
                        AskUserQuestionKind::SingleChoice => self.select_single(idx),
                        AskUserQuestionKind::MultipleChoice => self.toggle_multiple(idx),
                    }
                }
            }
            KeyCode::Enter => {
                let idx = self.state.get(q_idx).and_then(|s| s.highlight.selected_idx);
                if let Some(idx) = idx {
                    match kind {
                        AskUserQuestionKind::SingleChoice => self.select_single(idx),
                        AskUserQuestionKind::MultipleChoice => self.move_to_next_after_answer(),
                    }
                }
            }
            KeyCode::Char(ch) => {
                if let Some(idx) = Self::digit_to_index(ch) {
                    match kind {
                        AskUserQuestionKind::SingleChoice => self.select_single(idx),
                        AskUserQuestionKind::MultipleChoice => self.toggle_multiple(idx),
                    }
                    return;
                }

                let highlighted_idx = self.state.get(q_idx).and_then(|s| s.highlight.selected_idx);
                let highlighted_custom = highlighted_idx
                    .and_then(|idx| self.request.questions[q_idx].options.get(idx))
                    .is_some_and(|opt| opt.custom);
                if highlighted_custom {
                    let idx = highlighted_idx.unwrap_or(0);
                    match kind {
                        AskUserQuestionKind::SingleChoice => self.select_single(idx),
                        AskUserQuestionKind::MultipleChoice => {
                            if !self.state[q_idx]
                                .selected_multi
                                .get(idx)
                                .copied()
                                .unwrap_or(false)
                            {
                                self.toggle_multiple(idx);
                            } else {
                                self.focus = Focus::Custom;
                            }
                        }
                    }
                    if let Some(custom) = self.state[q_idx].custom_entry.as_mut() {
                        custom.text.input(key_event);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.cancel();
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.done
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        let Some(q_idx) = self.current_question_index() else {
            return false;
        };

        if pasted.is_empty() {
            return false;
        }
        if !self.custom_is_selected(q_idx) {
            return false;
        }
        let Some(custom) = self.state[q_idx].custom_entry.as_mut() else {
            return false;
        };

        self.focus = Focus::Custom;
        custom.text.insert_str(&pasted);
        true
    }

    fn try_consume_ask_user_question_request(
        &mut self,
        request: AskUserQuestionEvent,
    ) -> Option<AskUserQuestionEvent> {
        self.queue.push_back(request);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use crate::history_cell::HistoryCell;
    use crate::render::renderable::Renderable;
    use codex_protocol::ask_user_question::AskUserQuestionKind;
    use codex_protocol::ask_user_question::AskUserQuestionOption;
    use codex_protocol::ask_user_question::AskUserQuestionQuestion;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::text::Text;
    use ratatui::widgets::Paragraph;
    use ratatui::widgets::Widget;
    use tokio::sync::mpsc::unbounded_channel;

    fn test_sender() -> (
        AppEventSender,
        tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    ) {
        let (tx_raw, rx) = unbounded_channel::<AppEvent>();
        (AppEventSender::new(tx_raw), rx)
    }

    fn event(questions: Vec<AskUserQuestionQuestion>) -> AskUserQuestionEvent {
        AskUserQuestionEvent {
            call_id: "call-1".to_string(),
            turn_id: "turn-1".to_string(),
            questions,
        }
    }

    fn single_backend_question(kind: AskUserQuestionKind) -> AskUserQuestionQuestion {
        AskUserQuestionQuestion {
            id: "backend".to_string(),
            header: "Backend".to_string(),
            question: "Какой фреймворк ты предпочитаешь для бэкенда?".to_string(),
            kind,
            options: vec![
                AskUserQuestionOption {
                    label: "Next.js API Routes".to_string(),
                    description: "Встроенные API routes в Next.js, удобно для full-stack"
                        .to_string(),
                    custom: false,
                },
                AskUserQuestionOption {
                    label: "Express.js".to_string(),
                    description: "Классический Node.js фреймворк".to_string(),
                    custom: false,
                },
                AskUserQuestionOption {
                    label: "Type something.".to_string(),
                    description: "".to_string(),
                    custom: true,
                },
            ],
        }
    }

    fn snapshot_buffer(buf: &Buffer) -> String {
        let mut lines = Vec::new();
        for y in 0..buf.area().height {
            let mut row = String::new();
            for x in 0..buf.area().width {
                row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            lines.push(row);
        }
        lines.join("\n")
    }

    fn render_snapshot(overlay: &AskUserQuestionOverlay, area: Rect) -> String {
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);
        snapshot_buffer(&buf)
    }

    fn render_history_snapshot(cell: &dyn HistoryCell, area: Rect) -> String {
        let mut buf = Buffer::empty(area);
        Paragraph::new(Text::from(cell.display_lines(area.width))).render(area, &mut buf);
        snapshot_buffer(&buf)
    }

    #[test]
    fn ask_user_question_single_choice_autoadvance_to_submit() {
        let (tx, _rx) = test_sender();
        let mut overlay = AskUserQuestionOverlay::new(
            event(vec![single_backend_question(
                AskUserQuestionKind::SingleChoice,
            )]),
            tx,
        );

        // Pick Express.js, which should auto-advance to Submit tab.
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));

        let area = Rect::new(0, 0, 90, 16);
        assert_snapshot!(render_snapshot(&overlay, area));
    }

    #[test]
    fn ask_user_question_selected_tab_uses_background_fill() {
        let (tx, _rx) = test_sender();
        let overlay = AskUserQuestionOverlay::new(
            event(vec![single_backend_question(
                AskUserQuestionKind::SingleChoice,
            )]),
            tx,
        );

        let area = Rect::new(0, 0, 80, 18);
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);

        let mut found_bg = false;
        for x in 0..area.width {
            if buf[(x, 0)].style().bg == Some(Color::LightBlue) {
                found_bg = true;
                break;
            }
        }
        assert!(found_bg, "expected selected tab background fill");
    }

    #[test]
    fn ask_user_question_single_choice_custom_stays_on_tab_for_edit() {
        let (tx, _rx) = test_sender();
        let mut overlay = AskUserQuestionOverlay::new(
            event(vec![single_backend_question(
                AskUserQuestionKind::SingleChoice,
            )]),
            tx,
        );

        // Pick custom option and type a value; should not auto-advance until Enter.
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        for ch in "FastAPI".chars() {
            overlay.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        let area = Rect::new(0, 0, 90, 16);
        assert_snapshot!(render_snapshot(&overlay, area));
    }

    #[test]
    fn ask_user_question_multiple_choice_custom_and_next() {
        let (tx, _rx) = test_sender();
        let question = AskUserQuestionQuestion {
            id: "features".to_string(),
            header: "Features".to_string(),
            question: "Какие фичи нужны в первую очередь?".to_string(),
            kind: AskUserQuestionKind::MultipleChoice,
            options: vec![
                AskUserQuestionOption {
                    label: "Auth".to_string(),
                    description: "Логин/регистрация и роли".to_string(),
                    custom: false,
                },
                AskUserQuestionOption {
                    label: "Payments".to_string(),
                    description: "Подписки и биллинг".to_string(),
                    custom: false,
                },
                AskUserQuestionOption {
                    label: "Other".to_string(),
                    description: "".to_string(),
                    custom: true,
                },
            ],
        };

        let mut overlay = AskUserQuestionOverlay::new(event(vec![question]), tx);

        // Toggle Auth and custom.
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        for ch in "Realtime".chars() {
            overlay.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        // Next -> Submit tab
        overlay.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let area = Rect::new(0, 0, 90, 16);
        assert_snapshot!(render_snapshot(&overlay, area));
    }

    #[test]
    fn ask_user_question_submit_shows_blank_for_unanswered() {
        let (tx, _rx) = test_sender();
        let q1 = single_backend_question(AskUserQuestionKind::SingleChoice);
        let q2 = AskUserQuestionQuestion {
            id: "database".to_string(),
            header: "Database".to_string(),
            question: "Какую БД использовать?".to_string(),
            kind: AskUserQuestionKind::MultipleChoice,
            options: vec![
                AskUserQuestionOption {
                    label: "Postgres".to_string(),
                    description: "Реляционная БД".to_string(),
                    custom: false,
                },
                AskUserQuestionOption {
                    label: "SQLite".to_string(),
                    description: "Файл, локально".to_string(),
                    custom: false,
                },
                AskUserQuestionOption {
                    label: "Other".to_string(),
                    description: "".to_string(),
                    custom: true,
                },
            ],
        };

        let mut overlay = AskUserQuestionOverlay::new(event(vec![q1, q2]), tx);

        // Answer first question and move to Database tab.
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        // Move to Submit without answering Database.
        overlay.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        let area = Rect::new(0, 0, 90, 16);
        assert_snapshot!(render_snapshot(&overlay, area));
    }

    #[test]
    fn ask_user_question_submit_inserts_chat_summary_cell() {
        let (tx, mut rx) = test_sender();
        let q1 = AskUserQuestionQuestion {
            id: "tool_type".to_string(),
            header: "Tool Type".to_string(),
            question: "Какой тип инструмента вы хотите реализовать первым?".to_string(),
            kind: AskUserQuestionKind::SingleChoice,
            options: vec![
                AskUserQuestionOption {
                    label: "getBlockingTree".to_string(),
                    description: "".to_string(),
                    custom: false,
                },
                AskUserQuestionOption {
                    label: "getDeadlockCount".to_string(),
                    description: "".to_string(),
                    custom: false,
                },
            ],
        };
        let q2 = AskUserQuestionQuestion {
            id: "providers".to_string(),
            header: "Providers".to_string(),
            question: "Какие провайдеры нужно поддерживать?".to_string(),
            kind: AskUserQuestionKind::MultipleChoice,
            options: vec![
                AskUserQuestionOption {
                    label: "AWS".to_string(),
                    description: "".to_string(),
                    custom: false,
                },
                AskUserQuestionOption {
                    label: "GCP".to_string(),
                    description: "".to_string(),
                    custom: false,
                },
            ],
        };
        let q3 = AskUserQuestionQuestion {
            id: "features".to_string(),
            header: "Features".to_string(),
            question: "Какие дополнительные функции включить?".to_string(),
            kind: AskUserQuestionKind::MultipleChoice,
            options: vec![
                AskUserQuestionOption {
                    label: "Version detection".to_string(),
                    description: "".to_string(),
                    custom: false,
                },
                AskUserQuestionOption {
                    label: "Token budget".to_string(),
                    description: "".to_string(),
                    custom: false,
                },
                AskUserQuestionOption {
                    label: "Playbook integration".to_string(),
                    description: "".to_string(),
                    custom: false,
                },
            ],
        };

        let mut overlay = AskUserQuestionOverlay::new(event(vec![q1, q2, q3]), tx);

        // Q1: pick first option, auto-advances.
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));

        // Q2: select AWS + GCP, then Next.
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // Q3: select all, then Next to Submit.
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // Submit.
        overlay.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let mut summary_cell: Option<Box<dyn HistoryCell>> = None;
        let mut saw_codex_op = false;
        while let Ok(ev) = rx.try_recv() {
            match ev {
                AppEvent::InsertHistoryCell(cell) => summary_cell = Some(cell),
                AppEvent::CodexOp(Op::UserInputAnswer { .. }) => saw_codex_op = true,
                _ => {}
            }
        }

        assert!(saw_codex_op);
        let summary_cell = summary_cell.expect("missing InsertHistoryCell");

        let width = 110u16;
        let height = HistoryCell::display_lines(summary_cell.as_ref(), width)
            .len()
            .max(1) as u16;
        assert_snapshot!(render_history_snapshot(
            summary_cell.as_ref(),
            Rect::new(0, 0, width, height)
        ));
    }
}
