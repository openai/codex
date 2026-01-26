//! Request-user-input overlay state machine.
//!
//! Core behaviors:
//! - Each question can be answered by selecting one option and/or providing notes.
//! - When options exist, notes are stored per selected option and appended as extra answers.
//! - Typing while focused on options jumps into notes to keep freeform input fast.
//! - Enter advances to the next question; the last question submits all answers.
//! - Freeform-only questions submit an empty answer list when empty.
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;

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
    // Final selection for the question (always set for option questions).
    selected: Option<usize>,
    // Scrollable cursor state for option navigation/highlight.
    option_state: ScrollState,
    // Notes for freeform-only questions.
    notes: NotesEntry,
    // Per-option notes for option questions.
    option_notes: Vec<NotesEntry>,
}

pub(crate) struct RequestUserInputOverlay {
    app_event_tx: AppEventSender,
    request: RequestUserInputEvent,
    // Queue of incoming requests to process after the current one.
    queue: VecDeque<RequestUserInputEvent>,
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
            queue: VecDeque::new(),
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
                .is_some_and(|answer| answer.selected.is_none())
        {
            SELECT_OPTION_PLACEHOLDER
        } else if self.has_options() {
            NOTES_PLACEHOLDER
        } else {
            ANSWER_PLACEHOLDER
        }
    }

    /// Ensure the focus mode is valid for the current question.
    fn ensure_focus_available(&mut self) {
        if self.question_count() == 0 {
            return;
        }
        if !self.has_options() {
            self.focus = Focus::Notes;
        }
    }

    /// Rebuild local answer state from the current request.
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

    /// Move to the next/previous question, wrapping in either direction.
    fn move_question(&mut self, next: bool) {
        let len = self.question_count();
        if len == 0 {
            return;
        }
        let offset = if next { 1 } else { len.saturating_sub(1) };
        self.current_idx = (self.current_idx + offset) % len;
        self.ensure_focus_available();
    }

    /// Synchronize selection state to the currently focused option.
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

    /// Ensure there is a selection before allowing notes entry.
    fn ensure_selected_for_notes(&mut self) {
        if self.has_options()
            && self
                .current_answer()
                .is_some_and(|answer| answer.selected.is_none())
        {
            self.select_current_option();
        }
    }

    /// Advance to next question, or submit when on the last one.
    fn go_next_or_submit(&mut self) {
        if self.current_index() + 1 >= self.question_count() {
            self.submit_answers();
        } else {
            self.move_question(true);
        }
    }

    /// Build the response payload and dispatch it to the app.
    fn submit_answers(&mut self) {
        let mut answers = HashMap::new();
        for (idx, question) in self.request.questions.iter().enumerate() {
            let answer_state = &self.answers[idx];
            let options = question.options.as_ref();
            // For option questions we always produce a selection.
            let selected_idx = if options.is_some_and(|opts| !opts.is_empty()) {
                answer_state
                    .selected
                    .or(answer_state.option_state.selected_idx)
            } else {
                answer_state.selected
            };
            // Notes are appended as extra answers. When options exist, notes are per selected option.
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
            let mut answer_list = selected_label.into_iter().collect::<Vec<_>>();
            if !notes.is_empty() {
                answer_list.push(format!("user_note: {notes}"));
            }
            answers.insert(
                question.id.clone(),
                RequestUserInputAnswer {
                    answers: answer_list,
                },
            );
        }
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::UserInputAnswer {
                id: self.request.turn_id.clone(),
                response: RequestUserInputResponse { answers },
            }));
        if let Some(next) = self.queue.pop_front() {
            self.request = next;
            self.reset_for_request();
            self.ensure_focus_available();
        } else {
            self.done = true;
        }
    }

    /// Count freeform-only questions that have no notes.
    fn unanswered_count(&self) -> usize {
        self.request
            .questions
            .iter()
            .enumerate()
            .filter(|(idx, question)| {
                let answer = &self.answers[*idx];
                let options = question.options.as_ref();
                if options.is_some_and(|opts| !opts.is_empty()) {
                    false
                } else {
                    answer.notes.text.text().trim().is_empty()
                }
            })
            .count()
    }

    /// Compute the preferred notes input height for the current question.
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

        // Question navigation is always available.
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
                // Keep selection synchronized as the user moves.
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
                        // Any typing while in options switches to notes for fast freeform input.
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
                if self.has_options() && matches!(key_event.code, KeyCode::Up | KeyCode::Down) {
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
                        _ => {}
                    }
                    return;
                }
                // Notes are per option when options exist.
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
            // Treat pastes the same as typing: switch into notes.
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
        self.queue.push_back(request);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::render::renderable::Renderable;
    use codex_protocol::request_user_input::RequestUserInputQuestion;
    use codex_protocol::request_user_input::RequestUserInputQuestionOption;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use tokio::sync::mpsc::unbounded_channel;

    fn test_sender() -> (
        AppEventSender,
        tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    ) {
        let (tx_raw, rx) = unbounded_channel::<AppEvent>();
        (AppEventSender::new(tx_raw), rx)
    }

    fn question_with_options(id: &str, header: &str) -> RequestUserInputQuestion {
        RequestUserInputQuestion {
            id: id.to_string(),
            header: header.to_string(),
            question: "Choose an option.".to_string(),
            is_other: false,
            options: Some(vec![
                RequestUserInputQuestionOption {
                    label: "Option 1".to_string(),
                    description: "First choice.".to_string(),
                },
                RequestUserInputQuestionOption {
                    label: "Option 2".to_string(),
                    description: "Second choice.".to_string(),
                },
                RequestUserInputQuestionOption {
                    label: "Option 3".to_string(),
                    description: "Third choice.".to_string(),
                },
            ]),
        }
    }

    fn question_without_options(id: &str, header: &str) -> RequestUserInputQuestion {
        RequestUserInputQuestion {
            id: id.to_string(),
            header: header.to_string(),
            question: "Share details.".to_string(),
            is_other: false,
            options: None,
        }
    }

    fn request_event(
        turn_id: &str,
        questions: Vec<RequestUserInputQuestion>,
    ) -> RequestUserInputEvent {
        RequestUserInputEvent {
            call_id: "call-1".to_string(),
            turn_id: turn_id.to_string(),
            questions,
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

    fn render_snapshot(overlay: &RequestUserInputOverlay, area: Rect) -> String {
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);
        snapshot_buffer(&buf)
    }

    #[test]
    fn queued_requests_are_fifo() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "First")]),
            tx,
        );
        overlay.try_consume_user_input_request(request_event(
            "turn-2",
            vec![question_with_options("q2", "Second")],
        ));
        overlay.try_consume_user_input_request(request_event(
            "turn-3",
            vec![question_with_options("q3", "Third")],
        ));

        overlay.submit_answers();
        assert_eq!(overlay.request.turn_id, "turn-2");

        overlay.submit_answers();
        assert_eq!(overlay.request.turn_id, "turn-3");
    }

    #[test]
    fn options_always_return_a_selection() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
        );

        overlay.submit_answers();

        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(Op::UserInputAnswer { id, response }) = event else {
            panic!("expected UserInputAnswer");
        };
        assert_eq!(id, "turn-1");
        let answer = response.answers.get("q1").expect("answer missing");
        assert_eq!(answer.answers, vec!["Option 1".to_string()]);
    }

    #[test]
    fn freeform_questions_submit_empty_when_empty() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_without_options("q1", "Notes")]),
            tx,
        );

        overlay.submit_answers();

        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(Op::UserInputAnswer { response, .. }) = event else {
            panic!("expected UserInputAnswer");
        };
        let answer = response.answers.get("q1").expect("answer missing");
        assert_eq!(answer.answers, Vec::<String>::new());
    }

    #[test]
    fn notes_are_captured_for_selected_option() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
        );

        {
            let answer = overlay.current_answer_mut().expect("answer missing");
            answer.option_state.selected_idx = Some(1);
        }
        overlay.select_current_option();
        overlay
            .current_notes_entry_mut()
            .expect("notes entry missing")
            .text
            .insert_str("Notes for option 2");

        overlay.submit_answers();

        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(Op::UserInputAnswer { response, .. }) = event else {
            panic!("expected UserInputAnswer");
        };
        let answer = response.answers.get("q1").expect("answer missing");
        assert_eq!(
            answer.answers,
            vec![
                "Option 2".to_string(),
                "user_note: Notes for option 2".to_string(),
            ]
        );
    }

    #[test]
    fn request_user_input_options_snapshot() {
        let (tx, _rx) = test_sender();
        let overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Area")]),
            tx,
        );
        let area = Rect::new(0, 0, 64, 16);
        insta::assert_snapshot!(
            "request_user_input_options",
            render_snapshot(&overlay, area)
        );
    }

    #[test]
    fn request_user_input_tight_height_snapshot() {
        let (tx, _rx) = test_sender();
        let overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Area")]),
            tx,
        );
        let area = Rect::new(0, 0, 60, 8);
        insta::assert_snapshot!(
            "request_user_input_tight_height",
            render_snapshot(&overlay, area)
        );
    }

    #[test]
    fn request_user_input_scroll_options_snapshot() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![RequestUserInputQuestion {
                    id: "q1".to_string(),
                    header: "Next Step".to_string(),
                    question: "What would you like to do next?".to_string(),
                    is_other: false,
                    options: Some(vec![
                        RequestUserInputQuestionOption {
                            label: "Discuss a code change (Recommended)".to_string(),
                            description: "Walk through a plan and edit code together.".to_string(),
                        },
                        RequestUserInputQuestionOption {
                            label: "Run tests".to_string(),
                            description: "Pick a crate and run its tests.".to_string(),
                        },
                        RequestUserInputQuestionOption {
                            label: "Review a diff".to_string(),
                            description: "Summarize or review current changes.".to_string(),
                        },
                        RequestUserInputQuestionOption {
                            label: "Refactor".to_string(),
                            description: "Tighten structure and remove dead code.".to_string(),
                        },
                        RequestUserInputQuestionOption {
                            label: "Ship it".to_string(),
                            description: "Finalize and open a PR.".to_string(),
                        },
                    ]),
                }],
            ),
            tx,
        );
        {
            let answer = overlay.current_answer_mut().expect("answer missing");
            answer.option_state.selected_idx = Some(3);
            answer.selected = Some(3);
        }
        let area = Rect::new(0, 0, 68, 10);
        insta::assert_snapshot!(
            "request_user_input_scrolling_options",
            render_snapshot(&overlay, area)
        );
    }

    #[test]
    fn request_user_input_freeform_snapshot() {
        let (tx, _rx) = test_sender();
        let overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_without_options("q1", "Goal")]),
            tx,
        );
        let area = Rect::new(0, 0, 64, 10);
        insta::assert_snapshot!(
            "request_user_input_freeform",
            render_snapshot(&overlay, area)
        );
    }

    #[test]
    fn options_scroll_while_editing_notes() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
        );
        overlay.focus = Focus::Notes;
        overlay
            .current_notes_entry_mut()
            .expect("notes entry missing")
            .text
            .insert_str("Notes");

        overlay.handle_key_event(KeyEvent::from(KeyCode::Down));

        let answer = overlay.current_answer().expect("answer missing");
        assert_eq!(answer.selected, Some(1));
    }
}
