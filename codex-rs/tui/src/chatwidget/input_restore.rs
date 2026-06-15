//! Input queue restore and thread-input snapshot behavior for `ChatWidget`.

use std::collections::HashSet;

use super::user_messages::remap_colliding_paste_placeholders;
use super::*;

impl ChatWidget {
    pub(super) fn record_cancel_edit_candidate(&mut self, prompt: UserMessage) {
        self.cancel_edit.prompt = Some(prompt);
        self.cancel_edit.eligible = true;
        self.cancel_edit.armed = false;
    }

    pub(super) fn record_visible_turn_activity(&mut self) {
        self.cancel_edit.eligible = false;
        self.cancel_edit.armed = false;
    }

    pub(super) fn arm_cancel_edit(&mut self) {
        self.cancel_edit.armed = self.cancel_edit.eligible
            && self.cancel_edit.prompt.is_some()
            && self.bottom_pane.composer_is_empty()
            && self.input_queue.pending_steers.is_empty()
            && !self.has_queued_follow_up_messages()
            && !self.active_side_conversation;
    }

    fn take_armed_cancel_edit_prompt(&mut self, reason: TurnAbortReason) -> Option<UserMessage> {
        (reason == TurnAbortReason::Interrupted
            && self.cancel_edit.armed
            && self.cancel_edit.eligible)
            .then(|| self.cancel_edit.prompt.take())
            .flatten()
    }

    pub(super) fn clear_cancel_edit(&mut self) {
        self.cancel_edit = CancelEditState::default();
    }

    pub(crate) fn set_initial_user_message_submit_suppressed(&mut self, suppressed: bool) {
        self.suppress_initial_user_message_submit = suppressed;
    }

    pub(crate) fn submit_initial_user_message_if_pending(&mut self) {
        if self.suppress_initial_user_message_submit {
            return;
        }
        #[cfg(any(target_os = "windows", test))]
        if self.elevated_windows_sandbox_setup_required() {
            return;
        }
        if let Some(user_message) = self.initial_user_message.take() {
            if self.input_queue.blocks_local_queue_autosend() {
                self.queue_user_message(user_message);
            } else {
                self.submit_user_message(user_message);
            }
        }
    }

    pub(super) fn pop_next_queued_user_message(
        &mut self,
    ) -> Option<(QueuedUserMessage, UserMessageHistoryRecord)> {
        if self.input_queue.rejected_steers_queue.is_empty() {
            self.input_queue
                .queued_user_messages
                .pop_front()
                .map(|user_message| {
                    let history_record = self
                        .input_queue
                        .queued_user_message_history_records
                        .pop_front()
                        .unwrap_or(UserMessageHistoryRecord::UserMessageText);
                    (user_message, history_record)
                })
        } else {
            let rejected_messages = self
                .input_queue
                .rejected_steers_queue
                .drain(..)
                .collect::<Vec<_>>();
            let mut history_records = self
                .input_queue
                .rejected_steer_history_records
                .drain(..)
                .collect::<Vec<_>>();
            history_records.resize(
                rejected_messages.len(),
                UserMessageHistoryRecord::UserMessageText,
            );
            let (message, history_record) = merge_user_messages_with_history_record(
                rejected_messages
                    .into_iter()
                    .zip(history_records)
                    .collect::<Vec<_>>(),
            );
            Some((QueuedUserMessage::from(message), history_record))
        }
    }

    pub(super) fn pop_latest_queued_composer_state(&mut self) -> Option<ThreadComposerState> {
        if let Some(user_message) = self.input_queue.queued_user_messages.pop_back() {
            let history_record = self
                .input_queue
                .queued_user_message_history_records
                .pop_back()
                .unwrap_or(UserMessageHistoryRecord::UserMessageText);
            let QueuedUserMessage {
                user_message,
                pending_pastes,
                ..
            } = user_message;
            Some(Self::composer_state_from_user_message(
                user_message_for_restore(user_message, &history_record),
                pending_pastes,
            ))
        } else {
            let user_message = self.input_queue.rejected_steers_queue.pop_back()?;
            let history_record = self
                .input_queue
                .rejected_steer_history_records
                .pop_back()
                .unwrap_or(UserMessageHistoryRecord::UserMessageText);
            Some(Self::composer_state_from_user_message(
                user_message_for_restore(user_message, &history_record),
                Vec::new(),
            ))
        }
    }

    pub(crate) fn enqueue_rejected_steer(&mut self) -> bool {
        let Some(pending_steer) = self.input_queue.pending_steers.pop_front() else {
            tracing::warn!(
                "received active-turn-not-steerable error without a matching pending steer"
            );
            return false;
        };
        self.input_queue
            .rejected_steers_queue
            .push_back(pending_steer.user_message);
        self.input_queue
            .rejected_steer_history_records
            .push_back(pending_steer.history_record);
        self.refresh_pending_input_preview();
        true
    }

    /// Handle a turn aborted due to user interrupt (Esc), budget exhaustion,
    /// or review completion.
    /// When there are queued user messages, restore them into the composer
    /// separated by newlines rather than auto-submitting the next one.
    pub(super) fn on_interrupted_turn(&mut self, reason: TurnAbortReason) {
        let cancelled_prompt = self.take_armed_cancel_edit_prompt(reason);
        // Finalize, log a gentle prompt, and clear running state.
        self.finalize_turn();
        let send_pending_steers_immediately =
            self.input_queue.submit_pending_steers_after_interrupt;
        self.input_queue.submit_pending_steers_after_interrupt = false;
        if cancelled_prompt.is_none()
            && self.interrupted_turn_notice_mode != InterruptedTurnNoticeMode::Suppress
        {
            if send_pending_steers_immediately {
                self.add_to_history(history_cell::new_info_event(
                    "Model interrupted to submit steer instructions.".to_owned(),
                    /*hint*/ None,
                ));
            } else {
                self.add_to_history(history_cell::new_error_event(
                    self.interrupted_turn_message(reason),
                ));
            }
        }

        // The server has already discarded pending input by the time the
        // interrupted turn reaches the UI, so any unacknowledged steers still
        // tracked here must be restored locally instead of waiting for a later commit.
        if send_pending_steers_immediately {
            let pending_steers = self
                .input_queue
                .pending_steers
                .drain(..)
                .map(|pending| (pending.user_message, pending.history_record))
                .collect::<Vec<_>>();
            if !pending_steers.is_empty() {
                let (user_message, history_record) =
                    merge_user_messages_with_history_record(pending_steers);
                self.submit_user_message_with_history_record(user_message, history_record);
            } else if let Some(combined) = self.drain_pending_messages_for_restore() {
                self.restore_composer_state(combined);
            }
        } else if let Some(combined) = self.drain_pending_messages_for_restore() {
            self.restore_composer_state(combined);
        }
        self.refresh_pending_input_preview();
        if let Some(prompt) = cancelled_prompt {
            self.app_event_tx
                .send(AppEvent::RestoreCancelledTurn(prompt));
        }

        self.request_redraw();
    }

    /// Merge pending steers, queued drafts, and the current composer state into a single message.
    ///
    /// Each pending message numbers attachments from `[Image #1]` relative to its own remote
    /// images. When we concatenate multiple messages after interrupt, we must renumber local-image
    /// placeholders in a stable order and rebase text element byte ranges so the restored composer
    /// state stays aligned with the merged attachment list. Returns `None` when there is nothing to
    /// restore.
    fn drain_pending_messages_for_restore(&mut self) -> Option<ThreadComposerState> {
        if self.input_queue.pending_steers.is_empty() && !self.has_queued_follow_up_messages() {
            return None;
        }

        let composer = self.bottom_pane.composer_draft_snapshot();
        let composer_pending_pastes = composer.pending_pastes;
        let existing_message = UserMessage {
            text: composer.text,
            text_elements: composer.text_elements,
            local_images: composer.local_images,
            remote_image_urls: composer.remote_image_urls,
            mention_bindings: composer.mention_bindings,
        };

        let rejected_messages = self
            .input_queue
            .rejected_steers_queue
            .drain(..)
            .collect::<Vec<_>>();
        let mut rejected_history_records = self
            .input_queue
            .rejected_steer_history_records
            .drain(..)
            .collect::<Vec<_>>();
        rejected_history_records.resize(
            rejected_messages.len(),
            UserMessageHistoryRecord::UserMessageText,
        );
        let mut to_merge: Vec<UserMessage> = rejected_messages
            .into_iter()
            .zip(rejected_history_records.iter())
            .map(|(message, history_record)| user_message_for_restore(message, history_record))
            .collect();
        to_merge.extend(
            self.input_queue
                .pending_steers
                .drain(..)
                .map(|steer| user_message_for_restore(steer.user_message, &steer.history_record)),
        );
        let queued_messages = self
            .input_queue
            .queued_user_messages
            .drain(..)
            .collect::<Vec<_>>();
        let mut queued_history_records = self
            .input_queue
            .queued_user_message_history_records
            .drain(..)
            .collect::<Vec<_>>();
        queued_history_records.resize(
            queued_messages.len(),
            UserMessageHistoryRecord::UserMessageText,
        );
        let mut pending_pastes = Vec::new();
        let mut used_paste_placeholders = HashSet::new();
        for (message, history_record) in queued_messages
            .into_iter()
            .zip(queued_history_records.iter())
        {
            let (message, message_pastes) = remap_colliding_paste_placeholders(
                user_message_for_restore(message.user_message, history_record),
                message.pending_pastes,
                &mut used_paste_placeholders,
            );
            pending_pastes.extend(message_pastes);
            to_merge.push(message);
        }
        let has_existing_message = !existing_message.text.is_empty()
            || !existing_message.local_images.is_empty()
            || !existing_message.remote_image_urls.is_empty();
        if has_existing_message {
            let (existing_message, composer_pending_pastes) = remap_colliding_paste_placeholders(
                existing_message,
                composer_pending_pastes,
                &mut used_paste_placeholders,
            );
            to_merge.push(existing_message);
            pending_pastes.extend(composer_pending_pastes);
        }

        Some(Self::composer_state_from_user_message(
            merge_user_messages(to_merge),
            pending_pastes,
        ))
    }

    pub(crate) fn restore_user_message_to_composer(&mut self, user_message: UserMessage) {
        self.restore_composer_state(Self::composer_state_from_user_message(
            user_message,
            Vec::new(),
        ));
    }

    pub(super) fn restore_composer_state(&mut self, composer: ThreadComposerState) {
        let ThreadComposerState {
            text,
            local_images,
            remote_image_urls,
            text_elements,
            mention_bindings,
            pending_pastes,
        } = composer;
        let local_image_paths = local_images.into_iter().map(|img| img.path).collect();
        self.set_remote_image_urls(remote_image_urls);
        self.bottom_pane.set_composer_text_with_mention_bindings(
            text,
            text_elements,
            local_image_paths,
            mention_bindings,
        );
        self.bottom_pane.set_composer_pending_pastes(pending_pastes);
    }

    fn composer_state_from_user_message(
        user_message: UserMessage,
        pending_pastes: Vec<(String, String)>,
    ) -> ThreadComposerState {
        let UserMessage {
            text,
            local_images,
            remote_image_urls,
            text_elements,
            mention_bindings,
        } = user_message;
        ThreadComposerState {
            text,
            local_images,
            remote_image_urls,
            text_elements,
            mention_bindings,
            pending_pastes,
        }
    }

    pub(crate) fn capture_thread_input_state(&self) -> Option<ThreadInputState> {
        let draft = self.bottom_pane.composer_draft_snapshot();
        let composer = ThreadComposerState {
            text: draft.text,
            text_elements: draft.text_elements,
            local_images: draft.local_images,
            remote_image_urls: draft.remote_image_urls,
            mention_bindings: draft.mention_bindings,
            pending_pastes: draft.pending_pastes,
        };
        Some(ThreadInputState {
            composer: composer.has_content().then_some(composer),
            user_message_queue: self.input_queue.user_message_queue.clone(),
            pending_steers: self
                .input_queue
                .pending_steers
                .iter()
                .map(|pending| pending.user_message.clone())
                .collect(),
            pending_steer_history_records: self
                .input_queue
                .pending_steers
                .iter()
                .map(|pending| pending.history_record.clone())
                .collect(),
            pending_steer_compare_keys: self
                .input_queue
                .pending_steers
                .iter()
                .map(|pending| pending.compare_key.clone())
                .collect(),
            rejected_steers_queue: self.input_queue.rejected_steers_queue.clone(),
            rejected_steer_history_records: self.input_queue.rejected_steer_history_records.clone(),
            queued_user_messages: self.input_queue.queued_user_messages.clone(),
            queued_user_message_history_records: self
                .input_queue
                .queued_user_message_history_records
                .clone(),
            user_turn_pending_start: self.input_queue.user_turn_pending_start,
            current_collaboration_mode: self.current_collaboration_mode.clone(),
            active_collaboration_mask: self.active_collaboration_mask.clone(),
            task_running: self.bottom_pane.is_task_running(),
            agent_turn_running: self.turn_lifecycle.agent_turn_running,
        })
    }

    pub(crate) fn restore_thread_input_state(&mut self, input_state: Option<ThreadInputState>) {
        let restored_task_running = input_state.as_ref().is_some_and(|state| state.task_running);
        if let Some(input_state) = input_state {
            let refresh_in_flight = self.input_queue.user_message_queue.refresh_in_flight;
            self.current_collaboration_mode = input_state.current_collaboration_mode;
            self.active_collaboration_mask = input_state.active_collaboration_mask;
            self.turn_lifecycle
                .restore_running(input_state.agent_turn_running, Instant::now());
            self.input_queue.user_turn_pending_start = input_state.user_turn_pending_start;
            self.input_queue.user_message_queue = input_state.user_message_queue;
            self.input_queue.user_message_queue.refresh_in_flight |= refresh_in_flight;
            self.update_collaboration_mode_indicator();
            self.refresh_model_dependent_surfaces();
            self.restore_composer_state(input_state.composer.unwrap_or_default());
            let mut pending_steer_history_records = input_state.pending_steer_history_records;
            pending_steer_history_records.resize(
                input_state.pending_steers.len(),
                UserMessageHistoryRecord::UserMessageText,
            );
            let mut pending_steer_compare_keys = input_state.pending_steer_compare_keys;
            self.input_queue.pending_steers = input_state
                .pending_steers
                .into_iter()
                .zip(pending_steer_history_records)
                .map(|(user_message, history_record)| PendingSteer {
                    compare_key: pending_steer_compare_keys.pop_front().unwrap_or_else(|| {
                        PendingSteerCompareKey {
                            message: user_message.text.clone(),
                            image_count: user_message.local_images.len()
                                + user_message.remote_image_urls.len(),
                        }
                    }),
                    history_record,
                    user_message,
                })
                .collect();
            self.input_queue.rejected_steers_queue = input_state.rejected_steers_queue;
            self.input_queue.rejected_steer_history_records =
                input_state.rejected_steer_history_records;
            self.input_queue.rejected_steer_history_records.resize(
                self.input_queue.rejected_steers_queue.len(),
                UserMessageHistoryRecord::UserMessageText,
            );
            self.input_queue.queued_user_messages = input_state.queued_user_messages;
            self.input_queue.queued_user_message_history_records =
                input_state.queued_user_message_history_records;
            self.input_queue.queued_user_message_history_records.resize(
                self.input_queue.queued_user_messages.len(),
                UserMessageHistoryRecord::UserMessageText,
            );
        } else {
            self.turn_lifecycle
                .restore_running(/*running*/ false, Instant::now());
            self.input_queue.clear();
            self.restore_composer_state(Default::default());
        }
        self.turn_lifecycle
            .restore_running(self.turn_lifecycle.agent_turn_running, Instant::now());
        self.update_task_running_state();
        if restored_task_running && !self.bottom_pane.is_task_running() {
            self.bottom_pane.set_task_running(/*running*/ true);
            self.refresh_status_surfaces();
        }
        self.refresh_pending_input_preview();
        self.request_redraw();
    }

    pub(crate) fn set_queue_autosend_suppressed(&mut self, suppressed: bool) {
        self.input_queue.suppress_queue_autosend = suppressed;
    }
}
