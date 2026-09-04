//! Route question actions through the shared composer, preserving drafts until delivery accepts them.

use super::*;

impl AsyncQuestions {
    pub(crate) fn handles_key_as_editing(&self, key: KeyEvent) -> bool {
        self.composer.handles_key_as_editing(key)
    }

    pub(super) fn edit(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Repeat && self.keymap.composer.queue.is_pressed(key) {
            return;
        }

        if self.keymap.composer.submit.is_pressed(key) && !self.composer.is_in_paste_burst() {
            self.composer.flush_pending_input();
        }
        let before = self.composer.snapshot_draft();
        let (result, _) = self.composer.handle_key_event(key);
        let queued = matches!(result, InputResult::Queued { .. });
        if matches!(
            result,
            InputResult::Submitted { .. } | InputResult::Queued { .. }
        ) {
            self.composer.restore_draft(before);
            self.go_next_or_submit();
            if queued && let Some(QuestionSubmission::Submit(text)) = self.submission.take() {
                self.submission = Some(QuestionSubmission::Queue(text));
            }
        }
    }

    pub(crate) fn set_keymap(&mut self, keymap: &RuntimeKeymap) {
        self.keymap = keymap.clone();
        self.composer.set_keymap_bindings(keymap);
    }

    pub(crate) fn set_vim_enabled(&mut self, enabled: bool) {
        self.composer.set_vim_enabled(enabled);
    }
}

impl BottomPaneView for AsyncQuestions {
    fn is_complete(&self) -> bool {
        self.unanswered_count() == 0
    }
    fn flush_paste_burst_if_due(&mut self) -> bool {
        self.composer.flush_paste_burst_if_due()
    }
    fn next_frame_delay(&self) -> Option<std::time::Duration> {
        self.composer.footer_flash_delay()
    }
    fn is_in_paste_burst(&self) -> bool {
        self.composer.is_in_paste_burst()
    }
}
