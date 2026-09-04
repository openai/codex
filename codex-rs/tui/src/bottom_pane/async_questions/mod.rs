//! Inline editing for asynchronous questions. Legacy request_user_input keeps its own overlay.
//! Only locally accepted submissions remove questions; arrival and expiry never steal focus.

use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::ChatComposerConfig;
use crate::bottom_pane::InputResult;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::chat_composer::ComposerDraft;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::RuntimeKeymap;
use codex_protocol::items::AsyncUserInputQuestion;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use std::collections::HashSet;

mod input;
mod render;
mod state;

#[derive(Debug, Clone, PartialEq)]
struct PendingQuestion {
    question: AsyncUserInputQuestion,
    draft: ComposerDraft,
}

/// Locally retained async questions; replayed message IDs cannot resurrect handled answers.
#[derive(Debug, Clone, Default, PartialEq)]
struct QuestionState {
    pending: Vec<PendingQuestion>,
    current_idx: usize,
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
    pub(crate) delivery_enabled: bool,
    pub(crate) submission: Option<QuestionSubmission>,
    pub(crate) next_hint: Option<crate::key_hint::ShortcutHint>,
    keymap: RuntimeKeymap,
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
            ChatComposerConfig::plain_text(),
        );
        composer.set_keymap_bindings(&keymap);
        composer.set_footer_hint_override(Some(Vec::new()));
        Self {
            app_event_tx,
            state: QuestionState::default(),
            expanded: false,
            delivery_enabled: true,
            submission: None,
            next_hint: None,
            keymap,
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

    fn save_current_draft(&mut self) {
        self.composer.flush_pending_input();
        let draft = self.composer.snapshot_draft();
        if let Some(answer) = self.current_answer_mut() {
            answer.draft = draft;
        }
    }

    fn restore_current_draft(&mut self) {
        let draft = self
            .current_answer()
            .map(|answer| answer.draft.clone())
            .unwrap_or_default();
        self.composer.restore_inline_draft(draft);
    }

    pub(crate) fn unanswered_count(&self) -> usize {
        self.state.pending.len()
    }
}
