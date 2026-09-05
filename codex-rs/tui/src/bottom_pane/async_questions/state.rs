//! Pending async questions are local drafts; handling one removes it immediately.
//! Message IDs survive removal so replay cannot reopen an answered or skipped question.

use super::*;
use codex_context_fragments::AnsweredQuestion;
use codex_context_fragments::ContextualUserFragment;

impl AsyncQuestions {
    pub(crate) fn append(&mut self, message_id: &str, questions: &[AsyncUserInputQuestion]) {
        if questions.is_empty() || !self.state.seen_ids.insert(message_id.to_string()) {
            return;
        }
        let was_empty = self.state.pending.is_empty();
        self.state.pending.extend(questions.iter().map(|question| {
            // Bound work before cloning or wrapping model-authored suggestions.
            let question = AsyncUserInputQuestion {
                title: question.title.clone(),
                options: question.options.as_ref().map(|options| {
                    options
                        .iter()
                        .take(32)
                        .filter(|label| label.len() <= 512)
                        .cloned()
                        .collect()
                }),
            };
            let has_options = question
                .options
                .as_ref()
                .is_some_and(|options| !options.is_empty());
            let mut options_state = ScrollState::new();
            options_state.selected_idx = has_options.then_some(0);
            PendingQuestion {
                question,
                options_state,
                draft: ComposerDraft::default(),
            }
        }));
        if was_empty {
            self.state.current_idx = 0;
            self.restore_current_draft();
        }
    }

    pub(crate) fn set_expanded(&mut self, expanded: bool) {
        self.save_current_draft();
        self.expanded = expanded && !self.state.pending.is_empty();
    }

    pub(crate) fn navigate(&mut self, forward: bool) -> bool {
        let next = if forward {
            self.state.current_idx.checked_add(1)
        } else {
            self.state.current_idx.checked_sub(1)
        };
        let Some(next) = next.filter(|&index| index < self.state.pending.len()) else {
            return false;
        };
        self.save_current_draft();
        self.visible_options.set((0, 0));
        self.state.current_idx = next;
        self.restore_current_draft();
        true
    }

    pub(super) fn go_next_or_submit(&mut self) {
        self.save_current_draft();
        if !self.delivery_enabled {
            return;
        }
        let Some(answer) = self.current_answer() else {
            return;
        };
        let selected = answer
            .options_state
            .selected_idx
            .and_then(|index| answer.question.options.as_ref()?.get(index))
            .map(String::as_str)
            .unwrap_or_default();
        // Only a fully displayed model-authored option may become user authorization.
        let (first, count) = self.visible_options.get();
        let index = self.selected_option_index().unwrap_or(0);
        if !self.focus_is_notes() && !(first..first + count).contains(&index) {
            self.composer.show_footer_flash(
                "Expand terminal to read the entire option".into(),
                std::time::Duration::from_secs(5),
            );
            return;
        }
        let text = if self.focus_is_notes() {
            answer.draft.text_with_pending()
        } else {
            selected.to_string()
        };
        let text = text.trim();
        let framing = AnsweredQuestion::new(&answer.question.title).render();
        let limit = codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS - framing.chars().count();
        if text.chars().count() > limit {
            self.composer.show_footer_flash(
                format!("Answer too long; limit {limit} characters").into(),
                std::time::Duration::from_secs(5),
            );
        } else if !text.is_empty() {
            self.submission = Some(QuestionSubmission::Submit(format!("{framing}{text}")));
        }
    }

    pub(crate) fn accept_answer(&mut self) {
        if self.state.pending.is_empty() {
            return;
        }
        self.composer.flush_pending_input();
        self.visible_options.set((0, 0));
        self.state.pending.remove(self.state.current_idx);
        if self.state.current_idx >= self.state.pending.len() {
            self.state.current_idx = 0;
        }
        self.expanded &= !self.state.pending.is_empty();
        self.restore_current_draft();
        self.composer.reset_vim_mode();
    }
}
