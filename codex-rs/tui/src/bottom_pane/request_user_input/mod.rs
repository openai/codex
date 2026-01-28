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
use crossterm::event::KeyModifiers;
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
use unicode_width::UnicodeWidthStr;

const NOTES_PLACEHOLDER: &str = "Add notes";
const ANSWER_PLACEHOLDER: &str = "Type your answer (optional)";
// Keep in sync with ChatComposer's minimum composer height.
const MIN_COMPOSER_HEIGHT: u16 = 3;
const SELECT_OPTION_PLACEHOLDER: &str = "Select an option to add notes";
pub(super) const TIP_SEPARATOR: &str = " | ";
pub(super) const MAX_VISIBLE_OPTION_ROWS: usize = 4;
pub(super) const DESIRED_SPACERS_WHEN_NOTES_HIDDEN: u16 = 2;

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
    // Committed selection for the question (may be None when unanswered).
    committed_option_idx: Option<usize>,
    // Scrollable cursor state for option navigation/highlight.
    options_ui_state: ScrollState,
    // Per-question notes draft.
    draft: ComposerDraft,
    // Whether a freeform answer has been explicitly submitted.
    freeform_committed: bool,
    // Whether the notes UI has been explicitly opened for this question.
    notes_visible: bool,
}

#[derive(Clone, Debug)]
pub(super) struct FooterTip {
    pub(super) text: String,
    pub(super) highlight: bool,
}

impl FooterTip {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            highlight: false,
        }
    }

    fn highlighted(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            highlight: true,
        }
    }
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
        self.current_answer().and_then(|answer| {
            answer
                .committed_option_idx
                .or(answer.options_ui_state.selected_idx)
        })
    }

    fn current_option_label(&self) -> Option<&str> {
        let idx = self.selected_option_index()?;
        self.current_question()
            .and_then(|question| question.options.as_ref())
            .and_then(|options| options.get(idx))
            .map(|option| option.label.as_str())
    }

    fn notes_has_content(&self, idx: usize) -> bool {
        if idx == self.current_index() {
            !self.composer.current_text_with_pending().trim().is_empty()
        } else {
            !self.answers[idx].draft.text.trim().is_empty()
        }
    }

    pub(super) fn notes_ui_visible(&self) -> bool {
        if !self.has_options() {
            return true;
        }
        let idx = self.current_index();
        self.current_answer()
            .is_some_and(|answer| answer.notes_visible || self.notes_has_content(idx))
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

    fn focus_is_notes(&self) -> bool {
        matches!(self.focus, Focus::Notes)
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
                            .and_then(|answer| answer.committed_option_idx)
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
            .map(|answer| answer.options_ui_state)
            .unwrap_or_default();
        if state.selected_idx.is_none() {
            state.selected_idx = Some(0);
        }

        measure_rows_height(&rows, &state, rows.len(), width.max(1))
    }

    pub(super) fn options_preferred_height(&self, width: u16) -> u16 {
        if !self.has_options() {
            return 0;
        }

        let rows = self.option_rows();
        if rows.is_empty() {
            return 1;
        }

        let mut state = self
            .current_answer()
            .map(|answer| answer.options_ui_state)
            .unwrap_or_default();
        if state.selected_idx.is_none() {
            state.selected_idx = Some(0);
        }

        let visible_items = rows.len().min(MAX_VISIBLE_OPTION_ROWS);
        measure_rows_height(&rows, &state, visible_items, width.max(1))
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
        let notes_empty = draft.text.trim().is_empty();
        if let Some(answer) = self.current_answer_mut() {
            answer.draft = draft;
            if !notes_empty {
                answer.notes_visible = true;
            }
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
        if self.has_options() && self.selected_option_index().is_none() {
            SELECT_OPTION_PLACEHOLDER
        } else if self.has_options() {
            NOTES_PLACEHOLDER
        } else {
            ANSWER_PLACEHOLDER
        }
    }

    fn sync_composer_placeholder(&mut self) {
        self.composer
            .set_placeholder_text(self.notes_placeholder().to_string());
    }

    fn footer_tips(&self) -> Vec<FooterTip> {
        let mut tips = Vec::new();
        let notes_visible = self.notes_ui_visible();
        if self.has_options() {
            let options_len = self.options_len();
            if let Some(selected_idx) = self.selected_option_index() {
                let option_index = selected_idx + 1;
                tips.push(FooterTip::new(format!(
                    "Option {option_index} of {options_len}"
                )));
            } else {
                tips.push(FooterTip::new("No option selected"));
            }
            tips.push(FooterTip::new("\u{2191}/\u{2193} scroll"));
            if self.selected_option_index().is_some() && !notes_visible {
                tips.push(FooterTip::highlighted("Tab: add notes"));
            }
            if self.selected_option_index().is_some() && notes_visible && self.focus_is_notes() {
                tips.push(FooterTip::new("Tab: clear notes"));
            }
        }

        let question_count = self.question_count();
        let is_last_question = question_count > 0 && self.current_index() + 1 >= question_count;
        let enter_tip = if question_count > 1 && is_last_question {
            "Enter: submit all answers"
        } else {
            "Enter: submit answer"
        };
        tips.push(FooterTip::new(enter_tip));
        if question_count > 1 {
            tips.push(FooterTip::new("Ctrl+n next"));
        }
        tips.push(FooterTip::new("Esc: interrupt"));
        tips
    }

    pub(super) fn footer_tip_lines(&self, width: u16) -> Vec<Vec<FooterTip>> {
        let max_width = width.max(1) as usize;
        let separator_width = UnicodeWidthStr::width(TIP_SEPARATOR);
        let tips = self.footer_tips();
        if tips.is_empty() {
            return vec![Vec::new()];
        }

        let mut lines: Vec<Vec<FooterTip>> = Vec::new();
        let mut current: Vec<FooterTip> = Vec::new();
        let mut used = 0usize;

        for tip in tips {
            let tip_width = UnicodeWidthStr::width(tip.text.as_str()).min(max_width);
            let extra = if current.is_empty() {
                tip_width
            } else {
                separator_width.saturating_add(tip_width)
            };
            if !current.is_empty() && used.saturating_add(extra) > max_width {
                lines.push(current);
                current = Vec::new();
                used = 0;
            }
            if current.is_empty() {
                used = tip_width;
            } else {
                used = used
                    .saturating_add(separator_width)
                    .saturating_add(tip_width);
            }
            current.push(tip);
        }

        if current.is_empty() {
            lines.push(Vec::new());
        } else {
            lines.push(current);
        }
        lines
    }

    pub(super) fn footer_required_height(&self, width: u16) -> u16 {
        self.footer_tip_lines(width).len() as u16
    }

    /// Ensure the focus mode is valid for the current question.
    fn ensure_focus_available(&mut self) {
        if self.question_count() == 0 {
            return;
        }
        if !self.has_options() {
            self.focus = Focus::Notes;
            if let Some(answer) = self.current_answer_mut() {
                answer.notes_visible = true;
            }
            return;
        }
        if matches!(self.focus, Focus::Notes) && !self.notes_ui_visible() {
            self.focus = Focus::Options;
            self.sync_composer_placeholder();
        }
    }

    /// Rebuild local answer state from the current request.
    fn reset_for_request(&mut self) {
        self.answers = self
            .request
            .questions
            .iter()
            .map(|question| {
                let has_options = question
                    .options
                    .as_ref()
                    .is_some_and(|options| !options.is_empty());
                let mut options_ui_state = ScrollState::new();
                if has_options {
                    options_ui_state.selected_idx = Some(0);
                }
                AnswerState {
                    committed_option_idx: None,
                    options_ui_state,
                    draft: ComposerDraft::default(),
                    freeform_committed: false,
                    notes_visible: !has_options,
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
        self.restore_current_draft();
        self.ensure_focus_available();
    }

    /// Synchronize selection state to the currently focused option.
    fn select_current_option(&mut self) {
        if !self.has_options() {
            return;
        }
        let options_len = self.options_len();
        let updated = if let Some(answer) = self.current_answer_mut() {
            answer.options_ui_state.clamp_selection(options_len);
            answer.committed_option_idx = answer.options_ui_state.selected_idx;
            true
        } else {
            false
        };
        if updated {
            self.sync_composer_placeholder();
        }
    }

    /// Clear the current option selection and hide notes when empty.
    fn clear_selection(&mut self) {
        if !self.has_options() {
            return;
        }
        self.save_current_draft();
        let notes_empty = self.composer.current_text_with_pending().trim().is_empty();
        if let Some(answer) = self.current_answer_mut() {
            answer.committed_option_idx = None;
            answer.options_ui_state.reset();
            if notes_empty {
                answer.notes_visible = false;
            }
        }
        self.sync_composer_placeholder();
    }

    /// Ensure there is a selection before allowing notes entry.
    fn ensure_selected_for_notes(&mut self) {
        if let Some(answer) = self.current_answer_mut() {
            answer.notes_visible = true;
        }
        self.sync_composer_placeholder();
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
            // For option questions we may still produce no selection.
            let selected_idx = if options.is_some_and(|opts| !opts.is_empty()) {
                answer_state.committed_option_idx
            } else {
                None
            };
            // Notes are appended as extra answers. For freeform questions, only submit when
            // the user explicitly committed the draft.
            let notes = if options.is_some_and(|opts| !opts.is_empty())
                || answer_state.freeform_committed
            {
                answer_state.draft.text.trim().to_string()
            } else {
                String::new()
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
            self.restore_current_draft();
        } else {
            self.done = true;
        }
    }

    fn is_question_answered(&self, idx: usize, _current_text: &str) -> bool {
        let Some(question) = self.request.questions.get(idx) else {
            return false;
        };
        let Some(answer) = self.answers.get(idx) else {
            return false;
        };
        let has_options = question
            .options
            .as_ref()
            .is_some_and(|options| !options.is_empty());
        if has_options {
            answer.committed_option_idx.is_some()
        } else {
            answer.freeform_committed
        }
    }

    fn current_question_answered(&self) -> bool {
        let current_text = self.composer.current_text();
        self.is_question_answered(self.current_index(), &current_text)
    }

    /// Count questions that would submit an empty answer list.
    fn unanswered_count(&self) -> usize {
        let current_text = self.composer.current_text();
        self.request
            .questions
            .iter()
            .enumerate()
            .filter(|(idx, _question)| !self.is_question_answered(*idx, &current_text))
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
                if self.has_options()
                    && matches!(self.focus, Focus::Notes)
                    && !text.trim().is_empty()
                {
                    let options_len = self.options_len();
                    if let Some(answer) = self.current_answer_mut() {
                        answer.options_ui_state.clamp_selection(options_len);
                        answer.committed_option_idx = answer.options_ui_state.selected_idx;
                    }
                }
                if !self.has_options()
                    && let Some(answer) = self.current_answer_mut()
                {
                    answer.freeform_committed = !text.trim().is_empty();
                }
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
        match key_event {
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_question(false);
                return;
            }
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_question(true);
                return;
            }
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                ..
            } if self.has_options() && matches!(self.focus, Focus::Options) => {
                self.move_question(false);
                return;
            }
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                ..
            } if self.has_options() && matches!(self.focus, Focus::Options) => {
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
                    KeyCode::Up | KeyCode::Char('k') => {
                        let moved = if let Some(answer) = self.current_answer_mut() {
                            answer.options_ui_state.move_up_wrap(options_len);
                            true
                        } else {
                            false
                        };
                        if moved {
                            self.sync_composer_placeholder();
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let moved = if let Some(answer) = self.current_answer_mut() {
                            answer.options_ui_state.move_down_wrap(options_len);
                            true
                        } else {
                            false
                        };
                        if moved {
                            self.sync_composer_placeholder();
                        }
                    }
                    KeyCode::Char(' ') => {
                        self.select_current_option();
                    }
                    KeyCode::Backspace => {
                        self.clear_selection();
                    }
                    KeyCode::Tab => {
                        if self.selected_option_index().is_some() {
                            self.focus = Focus::Notes;
                            self.ensure_selected_for_notes();
                        }
                    }
                    KeyCode::Enter => {
                        let has_selection = self.selected_option_index().is_some();
                        if has_selection {
                            self.select_current_option();
                        }
                        self.go_next_or_submit();
                    }
                    _ => {}
                }
            }
            Focus::Notes => {
                let notes_empty = self.composer.current_text_with_pending().trim().is_empty();
                if self.has_options() && matches!(key_event.code, KeyCode::Tab) {
                    if let Some(answer) = self.current_answer_mut() {
                        answer.draft = ComposerDraft::default();
                        answer.notes_visible = false;
                    }
                    self.composer
                        .set_text_content(String::new(), Vec::new(), Vec::new());
                    self.composer.move_cursor_to_end();
                    self.focus = Focus::Options;
                    self.sync_composer_placeholder();
                    return;
                }
                if self.has_options() && matches!(key_event.code, KeyCode::Backspace) && notes_empty
                {
                    self.save_current_draft();
                    if let Some(answer) = self.current_answer_mut() {
                        answer.notes_visible = false;
                    }
                    self.focus = Focus::Options;
                    self.sync_composer_placeholder();
                    return;
                }
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
                            let moved = if let Some(answer) = self.current_answer_mut() {
                                answer.options_ui_state.move_up_wrap(options_len);
                                true
                            } else {
                                false
                            };
                            if moved {
                                self.sync_composer_placeholder();
                            }
                        }
                        KeyCode::Down => {
                            let moved = if let Some(answer) = self.current_answer_mut() {
                                answer.options_ui_state.move_down_wrap(options_len);
                                true
                            } else {
                                false
                            };
                            if moved {
                                self.sync_composer_placeholder();
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
    use crate::bottom_pane::selection_popup_common::menu_surface_inset;
    use crate::render::renderable::Renderable;
    use codex_protocol::request_user_input::RequestUserInputQuestion;
    use codex_protocol::request_user_input::RequestUserInputQuestionOption;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use tokio::sync::mpsc::unbounded_channel;
    use unicode_width::UnicodeWidthStr;

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
    fn options_can_submit_empty_when_unanswered() {
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
        assert_eq!(answer.answers, Vec::<String>::new());
    }

    #[test]
    fn enter_commits_default_selection_on_last_option_question() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
            true,
            false,
            false,
        );

        overlay.handle_key_event(KeyEvent::from(KeyCode::Enter));

        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(Op::UserInputAnswer { response, .. }) = event else {
            panic!("expected UserInputAnswer");
        };
        let answer = response.answers.get("q1").expect("answer missing");
        assert_eq!(answer.answers, vec!["Option 1".to_string()]);
    }

    #[test]
    fn enter_commits_default_selection_on_non_last_option_question() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_with_options("q1", "Pick one"),
                    question_with_options("q2", "Pick two"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );

        overlay.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(overlay.current_index(), 1);
        let first_answer = &overlay.answers[0];
        assert_eq!(first_answer.committed_option_idx, Some(0));
        assert_eq!(first_answer.options_ui_state.selected_idx, Some(0));
        assert!(rx.try_recv().is_err());

        overlay.handle_key_event(KeyEvent::from(KeyCode::Enter));
        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(Op::UserInputAnswer { response, .. }) = event else {
            panic!("expected UserInputAnswer");
        };
        let answer = response.answers.get("q1").expect("answer missing");
        assert_eq!(answer.answers, vec!["Option 1".to_string()]);
    }

    #[test]
    fn vim_keys_move_option_selection() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
            true,
            false,
            false,
        );
        let answer = overlay.current_answer().expect("answer missing");
        assert_eq!(answer.options_ui_state.selected_idx, Some(0));

        overlay.handle_key_event(KeyEvent::from(KeyCode::Char('j')));
        let answer = overlay.current_answer().expect("answer missing");
        assert_eq!(answer.options_ui_state.selected_idx, Some(1));

        overlay.handle_key_event(KeyEvent::from(KeyCode::Char('k')));
        let answer = overlay.current_answer().expect("answer missing");
        assert_eq!(answer.options_ui_state.selected_idx, Some(0));
    }

    #[test]
    fn typing_in_options_does_not_open_notes() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_with_options("q1", "Pick one"),
                    question_with_options("q2", "Pick two"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );

        assert_eq!(overlay.current_index(), 0);
        assert_eq!(overlay.notes_ui_visible(), false);
        overlay.handle_key_event(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!(overlay.current_index(), 0);
        assert_eq!(overlay.notes_ui_visible(), false);
        assert!(matches!(overlay.focus, Focus::Options));
        assert_eq!(overlay.composer.current_text_with_pending(), "");
    }

    #[test]
    fn h_l_move_between_questions_in_options() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_with_options("q1", "Pick one"),
                    question_with_options("q2", "Pick two"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );

        assert_eq!(overlay.current_index(), 0);
        overlay.handle_key_event(KeyEvent::from(KeyCode::Char('l')));
        assert_eq!(overlay.current_index(), 1);
        overlay.handle_key_event(KeyEvent::from(KeyCode::Char('h')));
        assert_eq!(overlay.current_index(), 0);
    }

    #[test]
    fn tab_opens_notes_when_option_selected() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
            true,
            false,
            false,
        );
        let answer = overlay.current_answer_mut().expect("answer missing");
        answer.options_ui_state.selected_idx = Some(1);
        answer.committed_option_idx = Some(1);

        assert_eq!(overlay.notes_ui_visible(), false);
        overlay.handle_key_event(KeyEvent::from(KeyCode::Tab));
        assert_eq!(overlay.notes_ui_visible(), true);
        assert!(matches!(overlay.focus, Focus::Notes));
    }

    #[test]
    fn switching_to_options_resets_notes_focus_when_notes_hidden() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_without_options("q1", "Notes"),
                    question_with_options("q2", "Pick one"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );

        assert!(matches!(overlay.focus, Focus::Notes));
        overlay.move_question(true);

        let answer = overlay.current_answer().expect("answer missing");
        assert!(matches!(overlay.focus, Focus::Options));
        assert_eq!(answer.committed_option_idx, None);
        assert_eq!(overlay.notes_ui_visible(), false);
    }

    #[test]
    fn switching_from_freeform_with_text_resets_focus_and_keeps_last_option_empty() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_without_options("q1", "Notes"),
                    question_with_options("q2", "Pick one"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );

        overlay
            .composer
            .set_text_content("freeform notes".to_string(), Vec::new(), Vec::new());
        overlay.composer.move_cursor_to_end();

        overlay.move_question(true);

        let answer = overlay.current_answer().expect("answer missing");
        assert!(matches!(overlay.focus, Focus::Options));
        assert_eq!(overlay.notes_ui_visible(), false);
        assert_eq!(answer.committed_option_idx, None);

        overlay.handle_key_event(KeyEvent::from(KeyCode::Enter));

        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(Op::UserInputAnswer { response, .. }) = event else {
            panic!("expected UserInputAnswer");
        };
        let answer = response.answers.get("q2").expect("answer missing");
        assert_eq!(answer.answers, vec!["Option 1".to_string()]);
    }

    #[test]
    fn esc_in_notes_mode_without_options_interrupts() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_without_options("q1", "Notes")]),
            tx,
            true,
            false,
            false,
        );

        overlay.handle_key_event(KeyEvent::from(KeyCode::Esc));

        assert_eq!(overlay.done, true);
        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(op) = event else {
            panic!("expected CodexOp");
        };
        assert_eq!(op, Op::Interrupt);
    }

    #[test]
    fn esc_in_options_mode_interrupts() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
            true,
            false,
            false,
        );

        overlay.handle_key_event(KeyEvent::from(KeyCode::Esc));

        assert_eq!(overlay.done, true);
        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(op) = event else {
            panic!("expected CodexOp");
        };
        assert_eq!(op, Op::Interrupt);
    }

    #[test]
    fn esc_in_notes_mode_interrupts() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
            true,
            false,
            false,
        );
        let answer = overlay.current_answer_mut().expect("answer missing");
        answer.options_ui_state.selected_idx = Some(0);
        answer.committed_option_idx = Some(0);

        overlay.handle_key_event(KeyEvent::from(KeyCode::Tab));
        overlay.handle_key_event(KeyEvent::from(KeyCode::Esc));

        assert_eq!(overlay.done, true);
        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(op) = event else {
            panic!("expected CodexOp");
        };
        assert_eq!(op, Op::Interrupt);
    }

    #[test]
    fn esc_in_notes_mode_interrupts_with_notes_visible() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
            true,
            false,
            false,
        );
        let answer = overlay.current_answer_mut().expect("answer missing");
        answer.options_ui_state.selected_idx = Some(0);
        answer.committed_option_idx = Some(0);

        overlay.handle_key_event(KeyEvent::from(KeyCode::Tab));
        overlay.handle_key_event(KeyEvent::from(KeyCode::Char('a')));
        overlay.handle_key_event(KeyEvent::from(KeyCode::Esc));

        assert_eq!(overlay.done, true);
        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(op) = event else {
            panic!("expected CodexOp");
        };
        assert_eq!(op, Op::Interrupt);
    }

    #[test]
    fn backspace_in_options_clears_selection() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
            true,
            false,
            false,
        );
        let answer = overlay.current_answer_mut().expect("answer missing");
        answer.options_ui_state.selected_idx = Some(1);
        answer.committed_option_idx = Some(1);

        overlay.handle_key_event(KeyEvent::from(KeyCode::Backspace));

        let answer = overlay.current_answer().expect("answer missing");
        assert_eq!(answer.committed_option_idx, None);
        assert_eq!(answer.options_ui_state.selected_idx, None);
        assert_eq!(overlay.notes_ui_visible(), false);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn backspace_on_empty_notes_closes_notes_ui() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
            true,
            false,
            false,
        );
        let answer = overlay.current_answer_mut().expect("answer missing");
        answer.options_ui_state.selected_idx = Some(0);
        answer.committed_option_idx = Some(0);

        overlay.handle_key_event(KeyEvent::from(KeyCode::Tab));
        assert!(matches!(overlay.focus, Focus::Notes));
        assert_eq!(overlay.notes_ui_visible(), true);

        overlay.handle_key_event(KeyEvent::from(KeyCode::Backspace));

        let answer = overlay.current_answer().expect("answer missing");
        assert!(matches!(overlay.focus, Focus::Options));
        assert_eq!(overlay.notes_ui_visible(), false);
        assert_eq!(answer.committed_option_idx, Some(0));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn tab_in_notes_clears_notes_and_hides_ui() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
            true,
            false,
            false,
        );
        let answer = overlay.current_answer_mut().expect("answer missing");
        answer.options_ui_state.selected_idx = Some(0);
        answer.committed_option_idx = Some(0);

        overlay.handle_key_event(KeyEvent::from(KeyCode::Tab));
        overlay
            .composer
            .set_text_content("Some notes".to_string(), Vec::new(), Vec::new());

        overlay.handle_key_event(KeyEvent::from(KeyCode::Tab));

        let answer = overlay.current_answer().expect("answer missing");
        assert!(matches!(overlay.focus, Focus::Options));
        assert_eq!(overlay.notes_ui_visible(), false);
        assert_eq!(overlay.composer.current_text_with_pending(), "");
        assert_eq!(answer.draft.text, "");
        assert_eq!(answer.committed_option_idx, Some(0));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn skipped_option_questions_count_as_unanswered() {
        let (tx, _rx) = test_sender();
        let overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
            true,
            false,
            false,
        );

        assert_eq!(overlay.unanswered_count(), 1);
    }

    #[test]
    fn highlighted_option_questions_are_unanswered() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Pick one")]),
            tx,
            true,
            false,
            false,
        );
        let answer = overlay.current_answer_mut().expect("answer missing");
        answer.options_ui_state.selected_idx = Some(0);

        assert_eq!(overlay.unanswered_count(), 1);
    }

    #[test]
    fn freeform_requires_enter_with_text_to_mark_answered() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_without_options("q1", "Notes"),
                    question_without_options("q2", "More"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );

        overlay
            .composer
            .set_text_content("Draft".to_string(), Vec::new(), Vec::new());
        overlay.composer.move_cursor_to_end();
        assert_eq!(overlay.unanswered_count(), 2);

        overlay.handle_key_event(KeyEvent::from(KeyCode::Enter));

        assert_eq!(overlay.answers[0].freeform_committed, true);
        assert_eq!(overlay.unanswered_count(), 1);
    }

    #[test]
    fn freeform_enter_with_empty_text_is_unanswered() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_without_options("q1", "Notes"),
                    question_without_options("q2", "More"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );

        overlay.handle_key_event(KeyEvent::from(KeyCode::Enter));

        assert_eq!(overlay.answers[0].freeform_committed, false);
        assert_eq!(overlay.unanswered_count(), 2);
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
    fn freeform_draft_is_not_submitted_without_enter() {
        let (tx, mut rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_without_options("q1", "Notes")]),
            tx,
            true,
            false,
            false,
        );
        overlay
            .composer
            .set_text_content("Draft text".to_string(), Vec::new(), Vec::new());
        overlay.composer.move_cursor_to_end();

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
            answer.options_ui_state.selected_idx = Some(1);
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
    fn notes_submission_commits_selected_option() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_with_options("q1", "Pick one"),
                    question_with_options("q2", "Pick two"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );

        overlay.handle_key_event(KeyEvent::from(KeyCode::Down));
        overlay.handle_key_event(KeyEvent::from(KeyCode::Tab));
        overlay
            .composer
            .set_text_content("Notes".to_string(), Vec::new(), Vec::new());
        overlay.composer.move_cursor_to_end();

        overlay.handle_key_event(KeyEvent::from(KeyCode::Enter));

        assert_eq!(overlay.current_index(), 1);
        let answer = overlay.answers.first().expect("answer missing");
        assert_eq!(answer.committed_option_idx, Some(1));
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
        let area = Rect::new(0, 0, 120, 16);
        insta::assert_snapshot!(
            "request_user_input_options",
            render_snapshot(&overlay, area)
        );
    }

    #[test]
    fn request_user_input_options_notes_visible_snapshot() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event("turn-1", vec![question_with_options("q1", "Area")]),
            tx,
            true,
            false,
            false,
        );
        {
            let answer = overlay.current_answer_mut().expect("answer missing");
            answer.options_ui_state.selected_idx = Some(0);
            answer.committed_option_idx = Some(0);
        }
        overlay.handle_key_event(KeyEvent::from(KeyCode::Tab));

        let area = Rect::new(0, 0, 120, 16);
        insta::assert_snapshot!(
            "request_user_input_options_notes_visible",
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
        let area = Rect::new(0, 0, 120, 10);
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
        let extras = 1u16 // header
            .saturating_add(1) // progress
            .saturating_add(DESIRED_SPACERS_WHEN_NOTES_HIDDEN)
            .saturating_add(overlay.footer_required_height(width));
        let height = question_height
            .saturating_add(options_height)
            .saturating_add(extras);
        let sections = overlay.layout_sections(Rect::new(0, 0, width, height));

        assert_eq!(sections.options_area.height, options_height);
    }

    #[test]
    fn desired_height_keeps_spacers_and_preferred_options_visible() {
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

        let width = 110u16;
        let height = overlay.desired_height(width);
        let content_area = menu_surface_inset(Rect::new(0, 0, width, height));
        let sections = overlay.layout_sections(content_area);
        let preferred = overlay.options_preferred_height(content_area.width);

        assert_eq!(sections.options_area.height, preferred);
        let question_bottom = sections.question_area.y + sections.question_area.height;
        let options_bottom = sections.options_area.y + sections.options_area.height;
        let spacer_after_question = sections.options_area.y.saturating_sub(question_bottom);
        let spacer_after_options = sections.notes_title_area.y.saturating_sub(options_bottom);
        assert_eq!(spacer_after_question, 1);
        assert_eq!(spacer_after_options, 1);
    }

    #[test]
    fn footer_wraps_tips_without_splitting_individual_tips() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_with_options("q1", "Pick one"),
                    question_with_options("q2", "Pick two"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );
        let answer = overlay.current_answer_mut().expect("answer missing");
        answer.options_ui_state.selected_idx = Some(0);
        answer.committed_option_idx = Some(0);

        let width = 36u16;
        let lines = overlay.footer_tip_lines(width);
        assert!(lines.len() > 1);
        let separator_width = UnicodeWidthStr::width(TIP_SEPARATOR);
        for tips in lines {
            let used = tips.iter().enumerate().fold(0usize, |acc, (idx, tip)| {
                let tip_width = UnicodeWidthStr::width(tip.text.as_str()).min(width as usize);
                let extra = if idx == 0 {
                    tip_width
                } else {
                    separator_width.saturating_add(tip_width)
                };
                acc.saturating_add(extra)
            });
            assert!(used <= width as usize);
        }
    }

    #[test]
    fn request_user_input_wrapped_options_snapshot() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![question_with_wrapped_options("q1", "Next Step")],
            ),
            tx,
            true,
            false,
            false,
        );

        {
            let answer = overlay.current_answer_mut().expect("answer missing");
            answer.options_ui_state.selected_idx = Some(0);
            answer.committed_option_idx = Some(0);
        }

        let width = 110u16;
        let question_height = overlay.wrapped_question_lines(width).len() as u16;
        let options_height = overlay.options_required_height(width);
        let height = 1u16
            .saturating_add(question_height)
            .saturating_add(options_height)
            .saturating_add(8);
        let area = Rect::new(0, 0, width, height);
        insta::assert_snapshot!(
            "request_user_input_wrapped_options",
            render_snapshot(&overlay, area)
        );
    }

    #[test]
    fn request_user_input_footer_wrap_snapshot() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_with_options("q1", "Pick one"),
                    question_with_options("q2", "Pick two"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );
        let answer = overlay.current_answer_mut().expect("answer missing");
        answer.options_ui_state.selected_idx = Some(1);
        answer.committed_option_idx = Some(1);

        let width = 52u16;
        let height = overlay.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        insta::assert_snapshot!(
            "request_user_input_footer_wrap",
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
            answer.options_ui_state.selected_idx = Some(3);
            answer.committed_option_idx = Some(3);
        }
        let area = Rect::new(0, 0, 120, 12);
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
        let area = Rect::new(0, 0, 120, 10);
        insta::assert_snapshot!(
            "request_user_input_freeform",
            render_snapshot(&overlay, area)
        );
    }

    #[test]
    fn request_user_input_multi_question_first_snapshot() {
        let (tx, _rx) = test_sender();
        let overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_with_options("q1", "Area"),
                    question_without_options("q2", "Goal"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );
        let area = Rect::new(0, 0, 120, 15);
        insta::assert_snapshot!(
            "request_user_input_multi_question_first",
            render_snapshot(&overlay, area)
        );
    }

    #[test]
    fn request_user_input_multi_question_last_snapshot() {
        let (tx, _rx) = test_sender();
        let mut overlay = RequestUserInputOverlay::new(
            request_event(
                "turn-1",
                vec![
                    question_with_options("q1", "Area"),
                    question_without_options("q2", "Goal"),
                ],
            ),
            tx,
            true,
            false,
            false,
        );
        overlay.move_question(true);
        let area = Rect::new(0, 0, 120, 12);
        insta::assert_snapshot!(
            "request_user_input_multi_question_last",
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
        overlay.select_current_option();
        overlay.focus = Focus::Notes;
        overlay
            .composer
            .set_text_content("Notes".to_string(), Vec::new(), Vec::new());
        overlay.composer.move_cursor_to_end();

        overlay.handle_key_event(KeyEvent::from(KeyCode::Down));

        let answer = overlay.current_answer().expect("answer missing");
        assert_eq!(answer.committed_option_idx, Some(0));
        assert_eq!(answer.options_ui_state.selected_idx, Some(1));
    }
}
