//! Request-user-input overlay state machine.
//!
//! Core behaviors:
//! - Each question can be answered by selecting one option and/or providing notes.
//! - Notes are stored per question and appended as extra answers.
//! - Typing while focused on options jumps into notes to keep freeform input fast.
//! - Enter advances to the next question; the last question submits all answers.
//! - Freeform-only questions submit an empty answer list when empty.
use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
mod layout;
mod render;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::ChatComposerConfig;
use crate::bottom_pane::InputResult;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_popup_common::GenericDisplayRow;
use crate::bottom_pane::selection_popup_common::measure_rows_height;
use crate::render::renderable::Renderable;

use codex_core::protocol::Op;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputEvent;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::TextElement;

const NOTES_PLACEHOLDER: &str = "Add notes (optional)";
const ANSWER_PLACEHOLDER: &str = "Type your answer (optional)";
// Keep in sync with ChatComposer's minimum composer height.
const MIN_COMPOSER_HEIGHT: u16 = 3;
const SELECT_OPTION_PLACEHOLDER: &str = "Select an option to add notes (optional)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Options,
    Notes,
}

#[derive(Default, Clone)]
struct ComposerDraft {
    text: String,
    text_elements: Vec<TextElement>,
    local_image_paths: Vec<PathBuf>,
}

struct AnswerState {
    // Final selection for the question (always set for option questions).
    selected: Option<usize>,
    // Scrollable cursor state for option navigation/highlight.
    option_state: ScrollState,
    // Per-question notes draft.
    draft: ComposerDraft,
}

pub(crate) struct RequestUserInputOverlay {
    app_event_tx: AppEventSender,
    request: RequestUserInputEvent,
    // Queue of incoming requests to process after the current one.
    queue: VecDeque<RequestUserInputEvent>,
    // Reuse the shared chat composer so notes/freeform answers match the
    // primary input styling and behavior.
    composer: ChatComposer,
    // One entry per question: selection state plus a stored notes draft.
    answers: Vec<AnswerState>,
    current_idx: usize,
    focus: Focus,
    done: bool,
}

impl RequestUserInputOverlay {
    pub(crate) fn new(
        request: RequestUserInputEvent,
        app_event_tx: AppEventSender,
        has_input_focus: bool,
        enhanced_keys_supported: bool,
        disable_paste_burst: bool,
    ) -> Self {
        // Use the same composer widget, but disable popups/slash-commands and
        // image-path attachment so it behaves like a focused notes field.
        let mut composer = ChatComposer::new_with_config(
            has_input_focus,
            app_event_tx.clone(),
            enhanced_keys_supported,
            ANSWER_PLACEHOLDER.to_string(),
            disable_paste_burst,
            ChatComposerConfig::plain_text(),
        );
        // The overlay renders its own footer hints, so keep the composer footer empty.
        composer.set_footer_hint_override(Some(Vec::new()));
        let mut overlay = Self {
            app_event_tx,
            request,
            queue: VecDeque::new(),
            composer,
            answers: Vec::new(),
            current_idx: 0,
            focus: Focus::Options,
            done: false,
        };
        overlay.reset_for_request();
        overlay.ensure_focus_available();
        overlay.restore_current_draft();
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

    pub(super) fn wrapped_question_lines(&self, width: u16) -> Vec<String> {
        self.current_question()
            .map(|q| {
                textwrap::wrap(&q.question, width.max(1) as usize)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    pub(super) fn option_rows(&self) -> Vec<GenericDisplayRow> {
        self.current_question()
            .and_then(|question| question.options.as_ref())
            .map(|options| {
                options
                    .iter()
                    .enumerate()
                    .map(|(idx, opt)| {
                        let selected = self
                            .current_answer()
                            .and_then(|answer| answer.selected)
                            .is_some_and(|sel| sel == idx);
                        let prefix = if selected { "(x)" } else { "( )" };
                        GenericDisplayRow {
                            name: format!("{prefix} {}", opt.label),
                            description: Some(opt.description.clone()),
                            ..Default::default()
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    pub(super) fn options_required_height(&self, width: u16) -> u16 {
        if !self.has_options() {
            return 0;
        }

        let rows = self.option_rows();
        if rows.is_empty() {
            return 1;
        }

        let mut state = self
            .current_answer()
            .map(|answer| answer.option_state)
            .unwrap_or_default();
        if state.selected_idx.is_none() {
            state.selected_idx = Some(0);
        }

        measure_rows_height(&rows, &state, rows.len(), width.max(1))
    }

    fn capture_composer_draft(&self) -> ComposerDraft {
        ComposerDraft {
            text: self.composer.current_text_with_pending(),
            text_elements: self.composer.text_elements(),
            local_image_paths: self
                .composer
                .local_images()
                .into_iter()
                .map(|img| img.path)
                .collect(),
        }
    }

    fn save_current_draft(&mut self) {
        let draft = self.capture_composer_draft();
        if let Some(answer) = self.current_answer_mut() {
            answer.draft = draft;
        }
    }

    fn restore_current_draft(&mut self) {
        self.composer
            .set_placeholder_text(self.notes_placeholder().to_string());
        self.composer.set_footer_hint_override(Some(Vec::new()));
        let Some(answer) = self.current_answer() else {
            self.composer
                .set_text_content(String::new(), Vec::new(), Vec::new());
            self.composer.move_cursor_to_end();
            return;
        };
        let draft = answer.draft.clone();
        self.composer
            .set_text_content(draft.text, draft.text_elements, draft.local_image_paths);
        self.composer.move_cursor_to_end();
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
                if let Some(options) = question.options.as_ref()
                    && !options.is_empty()
                {
                    option_state.selected_idx = Some(0);
                }
                AnswerState {
                    selected: option_state.selected_idx,
                    option_state,
                    draft: ComposerDraft::default(),
                }
            })
            .collect();

        self.current_idx = 0;
        self.focus = Focus::Options;
        self.composer
            .set_text_content(String::new(), Vec::new(), Vec::new());
    }

    /// Move to the next/previous question, wrapping in either direction.
    fn move_question(&mut self, next: bool) {
        let len = self.question_count();
        if len == 0 {
            return;
        }
        self.save_current_draft();
        let offset = if next { 1 } else { len.saturating_sub(1) };
        self.current_idx = (self.current_idx + offset) % len;
        self.ensure_focus_available();
        self.restore_current_draft();
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
        self.save_current_draft();
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
            // Notes are appended as extra answers.
            let notes = answer_state.draft.text.trim().to_string();
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
            self.restore_current_draft();
        } else {
            self.done = true;
        }
    }

    /// Count freeform-only questions that have no notes.
    fn unanswered_count(&self) -> usize {
        let current_text = self.composer.current_text();
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
                    let notes = if *idx == self.current_index() {
                        current_text.as_str()
                    } else {
                        answer.draft.text.as_str()
                    };
                    notes.trim().is_empty()
                }
            })
            .count()
    }

    /// Compute the preferred notes input height for the current question.
    fn notes_input_height(&self, width: u16) -> u16 {
        let min_height = MIN_COMPOSER_HEIGHT;
        self.composer
            .desired_height(width.max(1))
            .clamp(min_height, min_height.saturating_add(5))
    }

    fn apply_submission_to_draft(&mut self, text: String, text_elements: Vec<TextElement>) {
        let local_image_paths = self
            .composer
            .local_images()
            .into_iter()
            .map(|img| img.path)
            .collect::<Vec<_>>();
        if let Some(answer) = self.current_answer_mut() {
            answer.draft = ComposerDraft {
                text: text.clone(),
                text_elements: text_elements.clone(),
                local_image_paths: local_image_paths.clone(),
            };
        }
        self.composer
            .set_text_content(text, text_elements, local_image_paths);
        self.composer.move_cursor_to_end();
        self.composer.set_footer_hint_override(Some(Vec::new()));
    }

    fn handle_composer_input_result(&mut self, result: InputResult) -> bool {
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            }
            | InputResult::Queued {
                text,
                text_elements,
            } => {
                self.apply_submission_to_draft(text, text_elements);
                self.go_next_or_submit();
                true
            }
            _ => false,
        }
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
                // Keep selection synchronized as the user moves.
                match key_event.code {
                    KeyCode::Up => {
                        if let Some(answer) = self.current_answer_mut() {
                            answer.option_state.move_up_wrap(options_len);
                            answer.selected = answer.option_state.selected_idx;
                        }
                    }
                    KeyCode::Down => {
                        if let Some(answer) = self.current_answer_mut() {
                            answer.option_state.move_down_wrap(options_len);
                            answer.selected = answer.option_state.selected_idx;
                        }
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
                        let (result, _) = self.composer.handle_key_event(key_event);
                        self.handle_composer_input_result(result);
                    }
                    _ => {}
                }
            }
            Focus::Notes => {
                if matches!(key_event.code, KeyCode::Enter) {
                    self.ensure_selected_for_notes();
                    let (result, _) = self.composer.handle_key_event(key_event);
                    if !self.handle_composer_input_result(result) {
                        self.go_next_or_submit();
                    }
                    return;
                }
                if self.has_options() && matches!(key_event.code, KeyCode::Up | KeyCode::Down) {
                    let options_len = self.options_len();
                    match key_event.code {
                        KeyCode::Up => {
                            if let Some(answer) = self.current_answer_mut() {
                                answer.option_state.move_up_wrap(options_len);
                                answer.selected = answer.option_state.selected_idx;
                            }
                        }
                        KeyCode::Down => {
                            if let Some(answer) = self.current_answer_mut() {
                                answer.option_state.move_down_wrap(options_len);
                                answer.selected = answer.option_state.selected_idx;
                            }
                        }
                        _ => {}
                    }
                    return;
                }
                self.ensure_selected_for_notes();
                let (result, _) = self.composer.handle_key_event(key_event);
                self.handle_composer_input_result(result);
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
        if matches!(self.focus, Focus::Options) {
            // Treat pastes the same as typing: switch into notes.
            self.focus = Focus::Notes;
        }
        self.ensure_selected_for_notes();
        self.composer.handle_paste(pasted)
    }

    fn flush_paste_burst_if_due(&mut self) -> bool {
        self.composer.flush_paste_burst_if_due()
    }

    fn is_in_paste_burst(&self) -> bool {
        self.composer.is_in_paste_burst()
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

    fn question_with_wrapped_options(id: &str, header: &str) -> RequestUserInputQuestion {
        RequestUserInputQuestion {
            id: id.to_string(),
            header: header.to_string(),
            question: "Choose the next step for this task.".to_string(),
            is_other: false,
            options: Some(vec![
                RequestUserInputQuestionOption {
                    label: "Discuss a code change".to_string(),
                    description:
                        "Walk through a plan, then implement it together with careful checks."
                            .to_string(),
                },
                RequestUserInputQuestionOption {
                    label: "Run targeted tests".to_string(),
                    description:
                        "Pick the most relevant crate and validate the current behavior first."
                            .to_string(),
                },
                RequestUserInputQuestionOption {
                    label: "Review the diff".to_string(),
                    description:
                        "Summarize the changes and highlight the most important risks and gaps."
                            .to_string(),
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
            true,
            false,
            false,
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
            true,
            false,
            false,
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
            true,
            false,
            false,
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
            true,
            false,
            false,
        );

        {
            let answer = overlay.current_answer_mut().expect("answer missing");
            answer.option_state.selected_idx = Some(1);
        }
        overlay.select_current_option();
        overlay
            .composer
            .set_text_content("Notes for option 2".to_string(), Vec::new(), Vec::new());
        overlay.composer.move_cursor_to_end();

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
    fn large_paste_is_preserved_when_switching_questions() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_without_options("q1", "First"),
                    question_without_options("q2", "Second"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );

        let large = "x".repeat(1_500);
        overlay.composer.handle_paste(large.clone());
        overlay.move_question(true);

        assert_eq!(overlay.answers[0].draft.text, large);
    }

    #[test]
    fn request_user_input_options_snapshot() {
        let (tx, _rx) = test_sender();
        let overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Area")]),
            tx,
            true,
            false,
            false,
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
            true,
            false,
            false,
        );
        let area = Rect::new(0, 0, 60, 8);
        insta::assert_snapshot!(
            "request_user_input_tight_height",
            render_snapshot(&overlay, area)
        );
    }

    #[test]
    fn layout_allocates_all_wrapped_options_when_space_allows() {
        let (tx, _rx) = test_sender();
        let overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![question_with_wrapped_options("q1", "Next Step")],
            ),
            tx,
            true,
            false,
            false,
        );

        let width = 48u16;
        let question_height = overlay.wrapped_question_lines(width).len() as u16;
        let options_height = overlay.options_required_height(width);
        let height = 1u16
            .saturating_add(question_height)
            .saturating_add(options_height)
            .saturating_add(4);
        let sections = overlay.layout_sections(Rect::new(0, 0, width, height));

        assert_eq!(sections.options_area.height, options_height);
    }

    #[test]
    fn request_user_input_wrapped_options_snapshot() {
        let (tx, _rx) = test_sender();
        let overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![question_with_wrapped_options("q1", "Next Step")],
            ),
            tx,
            true,
            false,
            false,
        );

        let width = 52u16;
        let question_height = overlay.wrapped_question_lines(width).len() as u16;
        let options_height = overlay.options_required_height(width);
        let height = 1u16
            .saturating_add(question_height)
            .saturating_add(options_height)
            .saturating_add(4);
        let area = Rect::new(0, 0, width, height);
        insta::assert_snapshot!(
            "request_user_input_wrapped_options",
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
            true,
            false,
            false,
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
            true,
            false,
            false,
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
            true,
            false,
            false,
        );
        overlay.focus = Focus::Notes;
        overlay
            .composer
            .set_text_content("Notes".to_string(), Vec::new(), Vec::new());
        overlay.composer.move_cursor_to_end();

        overlay.handle_key_event(KeyEvent::from(KeyCode::Down));

        let answer = overlay.current_answer().expect("answer missing");
        assert_eq!(answer.selected, Some(1));
    }
}
