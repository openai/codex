//! Route question actions through the shared composer, preserving drafts until delivery accepts them.

use super::*;

impl AsyncQuestions {
    pub(crate) fn handles_key_as_editing(&self, key: KeyEvent) -> bool {
        self.focus_is_notes() && self.composer.handles_key_as_editing(key)
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
        self.composer.cancel_history_search();
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

    pub(super) fn select_option(&mut self, index: usize) {
        self.state.pending[self.state.current_idx]
            .options_state
            .selected_idx = Some(index);
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
    fn keymap_contexts(&self) -> crate::keymap::KeymapContextSet {
        if self.has_options() {
            crate::keymap::KeymapContextSet::new(KeymapContext::List).with(KeymapContext::Chat)
        } else {
            self.composer.keymap_contexts().with(KeymapContext::Chat)
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release || self.is_complete() {
            return;
        }
        if self.handles_key_as_editing(key) {
            self.edit(key);
            return;
        }
        if self.keymap.chat.interrupt_turn.is_pressed(key) {
            self.app_event_tx.interrupt();
            return;
        }
        if key.kind == KeyEventKind::Press && self.keymap.chat.skip_question.is_pressed(key) {
            if self.delivery_enabled {
                self.accept_answer();
            }
            return;
        }
        if self.keymap.chat.edit_queued_message.is_pressed(key) {
            self.navigate(/*forward*/ true);
            return;
        }
        if self.keymap.chat.prompt_stack_back.is_pressed(key) {
            self.navigate(/*forward*/ false);
            return;
        }
        if !self.has_options() {
            if key.kind == KeyEventKind::Press || !self.keymap.composer.submit.is_pressed(key) {
                self.edit(key);
            }
            return;
        }
        let count = self.options_len();
        let visible = self.visible_options.get().1.max(1);
        let mut state = self.state.pending[self.state.current_idx].options_state;
        match self.keymap.list.action_for(key) {
            Some(ListAction::MoveUp) => state.move_up_wrap(count),
            Some(ListAction::MoveDown) => state.move_down_wrap(count),
            Some(ListAction::PageUp) => state.page_up_clamped(count, visible),
            Some(ListAction::PageDown) => state.page_down_clamped(count, visible),
            Some(ListAction::JumpTop) => state.jump_top(count, visible),
            Some(ListAction::JumpBottom) => state.jump_bottom(count, visible),
            Some(ListAction::Accept) => {
                if key.kind == KeyEventKind::Press {
                    self.go_next_or_submit();
                }
            }
            Some(ListAction::Cancel) => self.set_expanded(/*expanded*/ false),
            Some(ListAction::MoveLeft | ListAction::MoveRight) => {}
            None => {
                if key.kind == KeyEventKind::Press
                    && !crate::key_hint::has_ctrl_or_alt(key.modifiers)
                    && let KeyCode::Char(ch) = key.code
                    && let Some(index) = self.option_index_for_digit(ch)
                {
                    self.select_option(index);
                    self.go_next_or_submit();
                    return;
                }
            }
        }
        self.state.pending[self.state.current_idx].options_state = state;
    }

    fn is_complete(&self) -> bool {
        self.unanswered_count() == 0
    }
    fn on_ctrl_c(&mut self) -> CancellationEvent {
        if self.composer.cancel_vim_search() || self.composer.cancel_history_search() {
            return CancellationEvent::Handled;
        }
        if self.composer.clear_for_ctrl_c().is_some() {
            self.save_current_draft();
        } else {
            return CancellationEvent::NotHandled;
        }
        CancellationEvent::Handled
    }
    fn handle_paste(&mut self, text: String) -> bool {
        if self.is_complete() || text.is_empty() {
            return false;
        }
        self.composer.flush_pending_input();
        !self.has_options() && self.composer.handle_paste(text)
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
