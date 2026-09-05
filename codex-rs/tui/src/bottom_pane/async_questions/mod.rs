//! Inline editing for asynchronous questions. Legacy request_user_input keeps its own overlay.
//! Only locally accepted submissions remove questions; arrival and expiry never steal focus.

use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::ChatComposerConfig;
use crate::bottom_pane::InputResult;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::chat_composer::ComposerDraft;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_popup_common::GenericDisplayRow;
use crate::bottom_pane::selection_popup_common::measure_rows_height;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::KeymapContext;
use crate::keymap::ListAction;
use crate::keymap::RuntimeKeymap;
use codex_protocol::items::AsyncUserInputQuestion;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use std::collections::HashSet;
use std::time::Duration;
use std::time::Instant;
use unicode_width::UnicodeWidthStr;

mod input;
mod layout;
mod render;
mod state;

const OTHER_OPTION_LABEL: &str = "Other";
pub(super) const TIP_SEPARATOR: &str = "   ";
pub(super) const DESIRED_SPACERS_BETWEEN_SECTIONS: u16 = 2;

#[derive(Debug, Clone, PartialEq)]
struct PendingQuestion {
    message_id: String,
    question: AsyncUserInputQuestion,
    options_state: ScrollState,
    draft: ComposerDraft,
    expires_at: Option<Instant>,
}

/// Locally retained async questions; replayed message IDs cannot resurrect handled answers.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct QuestionState {
    pending: Vec<PendingQuestion>,
    current_idx: usize,
    expanded: bool,
    seen_ids: HashSet<String>,
}

pub(crate) enum QuestionSubmission {
    Submit(String),
    Queue(String),
}

pub(crate) struct AsyncQuestions {
    app_event_tx: AppEventSender,
    state: QuestionState,
    pub(crate) expanded: bool,
    pub(crate) has_queued_messages: bool,
    pub(crate) delivery_enabled: bool,
    pub(crate) submission: Option<QuestionSubmission>,
    visible_options: std::cell::Cell<(usize, usize)>,
    pub(crate) next_hint: Option<crate::key_hint::ShortcutHint>,
    keymap: RuntimeKeymap,
    // Ignore autorepeat from the number key that opened Other.
    other_selector: Option<KeyCode>,
    pub(super) composer: ChatComposer,
}

impl AsyncQuestions {
    pub(crate) fn new(
        app_event_tx: AppEventSender,
        has_input_focus: bool,
        enhanced_keys_supported: bool,
        disable_paste_burst: bool,
        keymap: RuntimeKeymap,
    ) -> Self {
        let mut composer = ChatComposer::new_with_config(
            has_input_focus,
            app_event_tx.clone(),
            enhanced_keys_supported,
            "Type your answer".into(),
            disable_paste_burst,
            ChatComposerConfig {
                reset_vim_on_submission: false,
                ..ChatComposerConfig::plain_text()
            },
        );
        composer.set_keymap_bindings(&keymap);
        composer.set_footer_hint_override(Some(Vec::new()));
        Self {
            app_event_tx,
            state: QuestionState::default(),
            expanded: false,
            has_queued_messages: false,
            delivery_enabled: true,
            submission: None,
            visible_options: std::cell::Cell::new((0, 0)),
            next_hint: None,
            keymap,
            other_selector: None,
            composer,
        }
    }

    fn current_question(&self) -> Option<&AsyncUserInputQuestion> {
        self.current_answer().map(|answer| &answer.question)
    }

    fn current_answer_mut(&mut self) -> Option<&mut PendingQuestion> {
        self.state.pending.get_mut(self.state.current_idx)
    }

    fn current_answer(&self) -> Option<&PendingQuestion> {
        self.state.pending.get(self.state.current_idx)
    }

    pub(super) fn progress_prefix_text(&self) -> String {
        let current = self.state.current_idx + 1;
        let total = self.unanswered_count();
        format!("{current} of {total}")
    }

    fn options(&self) -> &[String] {
        self.current_question()
            .and_then(|q| q.options.as_deref())
            .unwrap_or_default()
    }

    fn has_options(&self) -> bool {
        !self.options().is_empty()
    }

    fn options_len(&self) -> usize {
        self.options().len() + usize::from(self.has_options())
    }

    fn option_index_for_digit(&self, ch: char) -> Option<usize> {
        let idx = ch.to_digit(10)?.checked_sub(1)? as usize;
        (idx < self.options_len()).then_some(idx)
    }

    fn selected_option_index(&self) -> Option<usize> {
        self.current_answer()
            .and_then(|answer| answer.options_state.selected_idx)
    }

    pub(super) fn wrapped_question_lines(&self, width: u16) -> Vec<String> {
        self.current_question()
            .map(|q| {
                textwrap::wrap(&q.title, width.max(1) as usize)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn focus_is_notes(&self) -> bool {
        !self.has_options() || self.other_selected()
    }

    pub(super) fn option_rows(&self) -> Vec<GenericDisplayRow> {
        if !self.has_options() {
            return Vec::new();
        };
        let other = self.other_label();
        self.options()
            .iter()
            .chain(std::iter::once(&other))
            .enumerate()
            .map(|(index, label)| {
                let prefix = if self.selected_option_index() == Some(index) {
                    '›'
                } else {
                    ' '
                };
                let number = index + 1;
                let prefix = format!("{prefix} {number}. ");
                GenericDisplayRow {
                    name: format!("{prefix}{label}"),
                    wrap_indent: Some(prefix.width()),
                    ..Default::default()
                }
            })
            .collect()
    }

    pub(super) fn options_required_height(&self, width: u16) -> u16 {
        if !self.has_options() {
            return 0;
        }
        let row_width = width.saturating_add(1);
        let rows = self.option_rows();
        if self.other_selected() {
            let prefix = self.other_prefix_width(width);
            measure_rows_height(
                &rows[..rows.len() - 1],
                &ScrollState::default(),
                rows.len(),
                row_width,
            ) + self
                .composer
                .inline_input_height(width.saturating_sub(prefix).max(1))
                .clamp(1, 8)
        } else {
            measure_rows_height(&rows, &ScrollState::default(), rows.len(), row_width)
        }
    }

    fn save_current_draft(&mut self) {
        self.composer.flush_pending_input();
        if self.focus_is_notes() {
            let draft = self.composer.snapshot_draft();
            if let Some(answer) = self.current_answer_mut() {
                answer.draft = draft;
            }
        }
    }

    fn restore_current_draft(&mut self) {
        self.sync_composer_placeholder();
        let draft = self
            .current_answer()
            .map(|answer| answer.draft.clone())
            .unwrap_or_default();
        self.composer.restore_inline_draft(draft);
    }

    fn other_placeholder(&self) -> &'static str {
        if self
            .options()
            .iter()
            .any(|label| label.eq_ignore_ascii_case("Other"))
        {
            "Other (write an answer)"
        } else {
            OTHER_OPTION_LABEL
        }
    }

    fn sync_composer_placeholder(&mut self) {
        let text = if self.other_selected() {
            self.other_placeholder()
        } else {
            "Type your answer"
        };
        self.composer.set_placeholder_text(text.to_string());
    }

    pub(crate) fn unanswered_count(&self) -> usize {
        self.state.pending.len()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
