//! User input submission, queue draining, and draft restore flow for `ChatWidget`.
//!
//! The queue data itself lives in `input_queue`; this module owns the app-level
//! effects around taking composer input, submitting user turns, draining queued
//! follow-ups, and restoring draft state across interrupts or thread switches.

use super::*;

impl ChatWidget {
    pub(super) fn handle_composer_input_result(
        &mut self,
        input_result: InputResult,
        had_modal_or_popup: bool,
    ) {
        match input_result {
            InputResult::Submitted {
                text,
                text_elements,
            } => {
                let user_message = self.user_message_from_submission(text, text_elements);
                if user_message.text.is_empty()
                    && user_message.local_images.is_empty()
                    && user_message.remote_image_urls.is_empty()
                {
                    return;
                }
                let should_submit_now =
                    self.is_session_configured() && !self.is_plan_streaming_in_tui();
                if should_submit_now {
                    if self.only_user_shell_commands_running()
                        && !user_message.text.starts_with('!')
                    {
                        self.queue_user_message(user_message);
                        return;
                    }
                    // Submitted is emitted when user submits.
                    // Reset any reasoning header only when we are actually submitting a turn.
                    self.reasoning_buffer.clear();
                    self.full_reasoning_buffer.clear();
                    self.set_status_header(String::from("Working"));
                    self.submit_user_message(user_message);
                } else {
                    self.queue_user_message(user_message);
                }
            }
            InputResult::Queued {
                text,
                text_elements,
                action,
            } => {
                let user_message = self.user_message_from_submission(text, text_elements);
                self.queue_user_message_with_options(user_message, action);
            }
            InputResult::Command(cmd) => {
                self.handle_slash_command_dispatch(cmd);
            }
            InputResult::ServiceTierCommand(command) => {
                self.handle_service_tier_command_dispatch(command);
            }
            InputResult::CommandWithArgs(cmd, args, text_elements) => {
                self.handle_slash_command_with_args_dispatch(cmd, args, text_elements);
            }
            InputResult::None => {}
        }
        if had_modal_or_popup && self.bottom_pane.no_modal_or_popup_active() {
            self.maybe_send_next_queued_input();
        }
        self.refresh_plan_mode_nudge();
    }

    pub(super) fn queue_user_message(&mut self, user_message: UserMessage) {
        self.queue_user_message_with_options(user_message, QueuedInputAction::Plain);
    }

    pub(super) fn queue_user_message_with_options(
        &mut self,
        user_message: UserMessage,
        action: QueuedInputAction,
    ) {
        if !self.is_session_configured() || self.is_user_turn_pending_or_running() {
            self.input_queue
                .queued_user_messages
                .push_back(QueuedUserMessage::new(user_message, action));
            self.input_queue
                .queued_user_message_history_records
                .push_back(UserMessageHistoryRecord::UserMessageText);
            self.refresh_pending_input_preview();
        } else {
            self.submit_user_message(user_message);
        }
    }

    /// If idle and there are queued inputs, submit exactly one to start the next turn.
    pub(crate) fn maybe_send_next_queued_input(&mut self) -> bool {
        if self.input_queue.suppress_queue_autosend {
            return false;
        }
        if self.is_user_turn_pending_or_running() {
            return false;
        }
        let mut submitted_follow_up = false;
        while !self.is_user_turn_pending_or_running() {
            let Some((queued_message, history_record)) = self.pop_next_queued_user_message() else {
                break;
            };
            match queued_message.action {
                QueuedInputAction::Plain => {
                    submitted_follow_up = self.submit_user_message_with_history_record(
                        queued_message.into_user_message(),
                        history_record,
                    );
                    break;
                }
                QueuedInputAction::ParseSlash => {
                    let drain = self.submit_queued_slash_prompt(queued_message.into_user_message());
                    if drain == QueueDrain::Stop {
                        submitted_follow_up = self.is_user_turn_pending_or_running();
                        break;
                    }
                }
                QueuedInputAction::RunShell => {
                    let drain = self.submit_queued_shell_prompt(queued_message.into_user_message());
                    if drain == QueueDrain::Stop {
                        submitted_follow_up = self.is_user_turn_pending_or_running();
                        break;
                    }
                }
            }
        }
        // Update the list to reflect the remaining queued messages (if any).
        self.refresh_pending_input_preview();
        submitted_follow_up
    }

    pub(super) fn is_user_turn_pending_or_running(&self) -> bool {
        self.input_queue.user_turn_pending_start || self.bottom_pane.is_task_running()
    }

    pub(super) fn only_user_shell_commands_running(&self) -> bool {
        self.turn_lifecycle.agent_turn_running
            && !self.running_commands.is_empty()
            && self
                .running_commands
                .values()
                .all(|command| command.source == ExecCommandSource::UserShell)
    }

    /// Rebuild and update the bottom-pane pending-input preview.
    pub(super) fn refresh_pending_input_preview(&mut self) {
        let preview = self.input_queue.preview();
        self.bottom_pane.set_pending_input_preview(
            preview.queued_messages,
            preview.pending_steers,
            preview.rejected_steers,
        );
    }

    pub(crate) fn submit_user_message_with_mode(
        &mut self,
        text: String,
        mut collaboration_mode: CollaborationModeMask,
    ) {
        if collaboration_mode.mode == Some(ModeKind::Plan)
            && let Some(effort) = self.config.plan_mode_reasoning_effort
        {
            collaboration_mode.reasoning_effort = Some(Some(effort));
        }
        if self.turn_lifecycle.agent_turn_running
            && self.active_collaboration_mask.as_ref() != Some(&collaboration_mode)
        {
            self.add_error_message(
                "Cannot switch collaboration mode while a turn is running.".to_string(),
            );
            return;
        }
        self.set_collaboration_mask(collaboration_mode);
        let should_queue = self.is_plan_streaming_in_tui();
        let user_message = UserMessage {
            text,
            local_images: Vec::new(),
            remote_image_urls: Vec::new(),
            text_elements: Vec::new(),
            mention_bindings: Vec::new(),
        };
        if should_queue {
            self.queue_user_message(user_message);
        } else {
            self.submit_user_message(user_message);
        }
    }

    #[cfg(test)]
    pub(crate) fn queued_user_message_texts(&self) -> Vec<String> {
        self.input_queue
            .rejected_steers_queue
            .iter()
            .map(|message| message.text.clone())
            .chain(
                self.input_queue
                    .queued_user_messages
                    .iter()
                    .map(|message| message.text.clone()),
            )
            .collect()
    }
}
