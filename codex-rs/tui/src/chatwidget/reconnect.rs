//! Disconnects preserve editable input but never automatically retry queued submissions.

use super::*;

impl ChatWidget {
    pub(crate) fn pause_for_disconnect(&mut self) {
        if let Some(questions) = &mut self.bottom_pane.questions {
            questions.delivery_enabled = false;
        }
        self.input_queue.recovered_queue = true;
        self.input_queue.suppress_queue_autosend = true;
        self.set_initial_user_message_submit_suppressed(/*suppressed*/ true);
        if let Some(message) = self.initial_user_message.take() {
            self.restore_user_message_to_composer(message);
        }
        self.bottom_pane.ensure_status_indicator();
        self.bottom_pane
            .set_interrupt_hint_visible(/*visible*/ false);
        self.set_status_header("Reconnecting to app-server…".to_string());
        self.set_footer_hint_override(Some(vec![("ctrl+c".into(), "quit".into())]));
        self.add_error_message("Connection lost. Attempting to reconnect…".into());
    }

    /// Restore local input only after replay, which can otherwise move interrupted queues into the draft.
    pub(crate) fn restore_reconnected_input(&mut self, input: Option<ThreadInputState>) {
        let running = self.turn_lifecycle.agent_turn_running;
        if let Some(mut input) = input {
            // Its acceptance is unknown. Keep a local copy for manual recovery without
            // comparing against partial history or automatically submitting it again.
            if input.user_turn_pending_start
                && let Some(prompt) = input.safety_buffering_prompt.take()
            {
                input.queued_user_messages.push_front(prompt.into());
                input
                    .queued_user_message_history_records
                    .push_front(UserMessageHistoryRecord::UserMessageText);
                input.recovered_queue = true;
            }
            input.current_collaboration_mode = self.current_collaboration_mode.clone();
            // Resume supplies model/effort, but not the user's selected collaboration mode.
            if let Some(mask) = input.active_collaboration_mask.as_mut() {
                mask.model = Some(self.current_model().to_string());
                mask.reasoning_effort = Some(self.effective_reasoning_effort());
            }
            self.restore_thread_input_state(
                Some(input),
                ThreadInputStateRestoreMode {
                    preserve_in_flight_turn: false,
                },
            );
        }
        self.turn_lifecycle.restore_running(running, Instant::now());
        self.update_task_running_state();
    }

    pub(crate) fn pause_unavailable_thread(&mut self) {
        if let Some(questions) = &mut self.bottom_pane.questions {
            questions.delivery_enabled = false;
        }
        self.turn_lifecycle
            .restore_running(/*running*/ false, Instant::now());
        self.update_task_running_state();
    }

    pub(crate) fn handle_disconnected_view_key(&mut self, key: KeyEvent) {
        self.bottom_pane.handle_key_event(key);
    }

    pub(crate) fn handle_disconnected_key(&mut self, key: KeyEvent) {
        if self.handle_question_key(key) {
            return;
        }
        if key.kind == KeyEventKind::Press && self.chat_keymap.edit_queued_message.is_pressed(key) {
            if let Some(composer) = self.pop_latest_queued_composer_state() {
                self.restore_composer_state(composer);
            } else if let Some(steer) = self.input_queue.pending_steers.pop_back() {
                self.restore_user_message_to_composer(user_message_for_restore(
                    steer.user_message,
                    &steer.history_record,
                ));
                self.input_queue.recovered_queue &= !self.input_queue.pending_steers.is_empty();
            }
            self.bottom_pane
                .handle_disconnected_key(KeyEvent::new(KeyCode::Null, KeyModifiers::NONE));
            self.refresh_pending_input_preview();
            self.request_redraw();
        } else {
            self.bottom_pane.handle_disconnected_key(key);
        }
    }

    pub(crate) fn reconnect_failed(&mut self) {
        self.set_status_header("Reconnect failed — check the endpoint, then relaunch".into());
        self.add_error_message(
            "Automatic reconnect could not restore this session. Your draft is still editable. Copy it before quitting with Ctrl+C, then reconnect with the same command.".into(),
        );
    }
}
