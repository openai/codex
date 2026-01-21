use std::cell::RefCell;
use std::collections::HashMap;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
mod layout;
mod render;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::textarea::TextArea;
use crate::bottom_pane::textarea::TextAreaState;

use codex_core::protocol::Op;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputEvent;
use codex_protocol::request_user_input::RequestUserInputResponse;

const NOTES_PLACEHOLDER: &str = "Add notes (optional)";
const ANSWER_PLACEHOLDER: &str = "Type your answer (optional)";
const SELECT_OPTION_PLACEHOLDER: &str = "Select an option to add notes (optional)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Options,
    Notes,
}

struct NotesEntry {
    text: TextArea,
    state: RefCell<TextAreaState>,
}

impl NotesEntry {
    fn new() -> Self {
        Self {
            text: TextArea::new(),
            state: RefCell::new(TextAreaState::default()),
        }
    }
}

struct AnswerState {
    selected: Option<usize>,
    option_state: ScrollState,
    notes: NotesEntry,
    option_notes: Vec<NotesEntry>,
}

pub(crate) struct RequestUserInputOverlay {
    app_event_tx: AppEventSender,
    request: RequestUserInputEvent,
    queue: Vec<RequestUserInputEvent>,
    answers: Vec<AnswerState>,
    current_idx: usize,
    focus: Focus,
    done: bool,
}

impl RequestUserInputOverlay {
    pub(crate) fn new(request: RequestUserInputEvent, app_event_tx: AppEventSender) -> Self {
        let mut overlay = Self {
            app_event_tx,
            request,
            queue: Vec::new(),
            answers: Vec::new(),
            current_idx: 0,
            focus: Focus::Options,
            done: false,
        };
        overlay.reset_for_request();
        overlay.ensure_focus_available();
        overlay
    }

    fn current_index(&self) -> usize {
        self.current_idx
    }

    fn current_question(
        &self,
    ) -> Option<&codex_protocol::request_user_input::RequestUserInputQuestion> {
        self.request.questions.get(self.current_index())
    }

    fn current_answer_mut(&mut self) -> Option<&mut AnswerState> {
        let idx = self.current_index();
        self.answers.get_mut(idx)
    }

    fn current_answer(&self) -> Option<&AnswerState> {
        let idx = self.current_index();
        self.answers.get(idx)
    }

    fn question_count(&self) -> usize {
        self.request.questions.len()
    }

    fn has_options(&self) -> bool {
        self.current_question()
            .and_then(|question| question.options.as_ref())
            .is_some_and(|options| !options.is_empty())
    }

    fn options_len(&self) -> usize {
        self.current_question()
            .and_then(|question| question.options.as_ref())
            .map(std::vec::Vec::len)
            .unwrap_or(0)
    }

    fn selected_option_index(&self) -> Option<usize> {
        if !self.has_options() {
            return None;
        }
        self.current_answer()
            .and_then(|answer| answer.selected.or(answer.option_state.selected_idx))
    }

    fn current_option_label(&self) -> Option<&str> {
        let idx = self.selected_option_index()?;
        self.current_question()
            .and_then(|question| question.options.as_ref())
            .and_then(|options| options.get(idx))
            .map(|option| option.label.as_str())
    }

    fn current_notes_entry(&self) -> Option<&NotesEntry> {
        let answer = self.current_answer()?;
        if !self.has_options() {
            return Some(&answer.notes);
        }
        let idx = self
            .selected_option_index()
            .or(answer.option_state.selected_idx)?;
        answer.option_notes.get(idx)
    }

    fn current_notes_entry_mut(&mut self) -> Option<&mut NotesEntry> {
        let has_options = self.has_options();
        let answer = self.current_answer_mut()?;
        if !has_options {
            return Some(&mut answer.notes);
        }
        let idx = answer
            .selected
            .or(answer.option_state.selected_idx)
            .or_else(|| answer.option_notes.is_empty().then_some(0))?;
        answer.option_notes.get_mut(idx)
    }

    fn notes_placeholder(&self) -> &'static str {
        if self.has_options()
            && self
                .current_answer()
                .is_some_and(|ans| ans.selected.is_none())
        {
            SELECT_OPTION_PLACEHOLDER
        } else if self.has_options() {
            NOTES_PLACEHOLDER
        } else {
            ANSWER_PLACEHOLDER
        }
    }

    fn ensure_focus_available(&mut self) {
        if self.question_count() == 0 {
            return;
        }
        if !self.has_options() {
            self.focus = Focus::Notes;
        }
    }

    fn reset_for_request(&mut self) {
        self.answers = self
            .request
            .questions
            .iter()
            .map(|question| {
                let mut option_state = ScrollState::new();
                let mut option_notes = Vec::new();
                if let Some(options) = question.options.as_ref()
                    && !options.is_empty()
                {
                    option_state.selected_idx = Some(0);
                    option_notes = (0..options.len()).map(|_| NotesEntry::new()).collect();
                }
                AnswerState {
                    selected: option_state.selected_idx,
                    option_state,
                    notes: NotesEntry::new(),
                    option_notes,
                }
            })
            .collect();

        self.current_idx = 0;
        self.focus = Focus::Options;
    }

    fn move_question(&mut self, next: bool) {
        let len = self.question_count();
        if len == 0 {
            return;
        }
        let offset = if next { 1 } else { len.saturating_sub(1) };
        self.current_idx = (self.current_idx + offset) % len;
        self.ensure_focus_available();
    }

    fn select_current_option(&mut self) {
        if !self.has_options() {
            return;
        }
        let options_len = self.options_len();
        let Some(answer) = self.current_answer_mut() else {
            return;
        };
        answer.option_state.clamp_selection(options_len);
        answer.selected = answer.option_state.selected_idx;
    }

    fn ensure_selected_for_notes(&mut self) {
        if self.has_options()
            && self
                .current_answer()
                .is_some_and(|answer| answer.selected.is_none())
        {
            self.select_current_option();
        }
    }

    fn go_next_or_submit(&mut self) {
        if self.current_index() + 1 >= self.question_count() {
            self.submit_answers();
        } else {
            self.move_question(true);
        }
    }

    fn submit_answers(&mut self) {
        let mut answers = HashMap::new();
        for (idx, question) in self.request.questions.iter().enumerate() {
            let answer_state = &self.answers[idx];
            let options = question.options.as_ref();
            let selected_idx = answer_state.selected;
            let notes = if options.is_some_and(|opts| !opts.is_empty()) {
                selected_idx
                    .and_then(|selected| answer_state.option_notes.get(selected))
                    .map(|entry| entry.text.text().trim().to_string())
                    .unwrap_or_default()
            } else {
                answer_state.notes.text.text().trim().to_string()
            };
            let selected_label = selected_idx.and_then(|selected_idx| {
                question
                    .options
                    .as_ref()
                    .and_then(|opts| opts.get(selected_idx))
                    .map(|opt| opt.label.clone())
            });
            let selected = selected_label.into_iter().collect::<Vec<_>>();
            let other = if notes.is_empty() {
                if selected.is_empty() {
                    Some("skipped".to_string())
                } else {
                    None
                }
            } else {
                Some(notes)
            };
            answers.insert(
                question.id.clone(),
                RequestUserInputAnswer { selected, other },
            );
        }
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::UserInputAnswer {
                id: self.request.turn_id.clone(),
                response: RequestUserInputResponse { answers },
            }));
        if let Some(next) = self.queue.pop() {
            self.request = next;
            self.reset_for_request();
            self.ensure_focus_available();
        } else {
            self.done = true;
        }
    }

    fn unanswered_count(&self) -> usize {
        self.request
            .questions
            .iter()
            .enumerate()
            .filter(|(idx, question)| {
                let answer = &self.answers[*idx];
                let options = question.options.as_ref();
                if options.is_some_and(|opts| !opts.is_empty()) {
                    let has_selection = answer.selected.is_some();
                    let has_notes = answer
                        .option_notes
                        .iter()
                        .any(|entry| !entry.text.text().trim().is_empty());
                    !(has_selection || has_notes)
                } else {
                    answer.notes.text.text().trim().is_empty()
                }
            })
            .count()
    }

    fn notes_input_height(&self, width: u16) -> u16 {
        let Some(entry) = self.current_notes_entry() else {
            return 3;
        };
        let usable_width = width.saturating_sub(2);
        let text_height = entry.text.desired_height(usable_width).clamp(1, 6);
        text_height.saturating_add(2).clamp(3, 8)
    }
}

impl BottomPaneView for RequestUserInputOverlay {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        if matches!(key_event.code, KeyCode::Esc) {
            self.app_event_tx.send(AppEvent::CodexOp(Op::Interrupt));
            self.done = true;
            return;
        }

        match key_event.code {
            KeyCode::PageUp => {
                self.move_question(false);
                return;
            }
            KeyCode::PageDown => {
                self.move_question(true);
                return;
            }
            _ => {}
        }

        match self.focus {
            Focus::Options => {
                let options_len = self.options_len();
                let Some(answer) = self.current_answer_mut() else {
                    return;
                };
                match key_event.code {
                    KeyCode::Up => {
                        answer.option_state.move_up_wrap(options_len);
                        answer.selected = answer.option_state.selected_idx;
                    }
                    KeyCode::Down => {
                        answer.option_state.move_down_wrap(options_len);
                        answer.selected = answer.option_state.selected_idx;
                    }
                    KeyCode::Char(' ') => {
                        self.select_current_option();
                    }
                    KeyCode::Enter => {
                        self.select_current_option();
                        self.go_next_or_submit();
                    }
                    KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete => {
                        self.focus = Focus::Notes;
                        self.ensure_selected_for_notes();
                        if let Some(entry) = self.current_notes_entry_mut() {
                            entry.text.input(key_event);
                        }
                    }
                    _ => {}
                }
            }
            Focus::Notes => {
                if matches!(key_event.code, KeyCode::Enter) {
                    self.go_next_or_submit();
                    return;
                }
                self.ensure_selected_for_notes();
                if let Some(entry) = self.current_notes_entry_mut() {
                    entry.text.input(key_event);
                }
            }
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.app_event_tx.send(AppEvent::CodexOp(Op::Interrupt));
        self.done = true;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.done
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if pasted.is_empty() {
            return false;
        }
        if matches!(self.focus, Focus::Notes) {
            self.ensure_selected_for_notes();
            if let Some(entry) = self.current_notes_entry_mut() {
                entry.text.insert_str(&pasted);
                return true;
            }
            return true;
        }
        if matches!(self.focus, Focus::Options) {
            self.focus = Focus::Notes;
            self.ensure_selected_for_notes();
            if let Some(entry) = self.current_notes_entry_mut() {
                entry.text.insert_str(&pasted);
                return true;
            }
            return true;
        }
        false
    }

    fn try_consume_user_input_request(
        &mut self,
        request: RequestUserInputEvent,
    ) -> Option<RequestUserInputEvent> {
        self.queue.push(request);
        None
    }
}
