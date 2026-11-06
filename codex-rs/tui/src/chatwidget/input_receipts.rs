//! Apply submission receipts and recover draft input by client message ID.
//! Ambiguous delivery stays out of automatic submission until history confirms it.

use super::*;
use crate::app_server_session::user_message_submission::UserMessageAdmission;

impl ChatWidget {
    pub(crate) fn user_message_admitted(
        &mut self,
        receipt: UserMessageAdmission,
        turn: &AppCommand,
        client_id: &str,
    ) {
        if let UserMessageAdmission::Started(turn_id) | UserMessageAdmission::Steered(turn_id) =
            &receipt
            && (!self.turn_lifecycle.agent_turn_running
                || self.last_turn_id() != Some(turn_id.as_str()))
        {
            self.turn_lifecycle.admitted_turn_id = Some(turn_id.clone());
            self.update_task_running_state();
        }
        match receipt {
            UserMessageAdmission::StartResponse(turn_id) => {
                self.record_safety_buffering_turn(turn_id, turn)
            }
            UserMessageAdmission::Queued(message) => {
                if !self.enqueue_rejected_steer_by_id(Some(client_id)) {
                    self.add_error_message(message);
                }
            }
            UserMessageAdmission::Started(turn_id) => {
                if let Some(pending) = self.input_queue.take_pending_steer(client_id) {
                    let display = user_message_display_for_history(
                        pending.user_message.clone(),
                        &pending.history_record,
                    );
                    self.on_user_message_display(display);
                    self.last_rendered_user_message_client_id = Some(pending.client_id);
                    self.safety_buffering_prompt = Some(pending.user_message);
                    self.safety_buffering_source = pending.source;
                    self.refresh_pending_input_preview();
                }
                self.record_safety_buffering_turn(turn_id, turn);
            }
            // A duplicate identifies the message, but only its history event confirms
            // whether a steer committed. Keep it out of automatic delivery until then.
            UserMessageAdmission::Duplicate => {
                self.hold_unconfirmed_user_message(client_id);
            }
            UserMessageAdmission::Steered(turn_id) => {
                if let Some(pending) = self
                    .input_queue
                    .pending_steers
                    .iter_mut()
                    .find(|pending| pending.client_id == client_id)
                {
                    pending.admitted_turn_id = Some(turn_id);
                } else if let AppCommand::UserTurn { items, .. } = turn
                    && let Some((message, history_record, source)) =
                        self.take_submitted_message(client_id)
                {
                    self.input_queue.pending_steers.push_back(PendingSteer {
                        client_id: client_id.to_string(),
                        admitted_turn_id: Some(turn_id),
                        optimistically_rendered: true,
                        user_message: message,
                        history_record,
                        source,
                        compare_key: Self::pending_steer_compare_key_from_items(items),
                    });
                }
                if self.input_queue.pending_user_message_client_id.as_deref() == Some(client_id) {
                    self.input_queue.user_turn_pending_start = false;
                    self.input_queue.pending_user_message_client_id = None;
                }
                self.refresh_pending_input_preview();
            }
        }
    }

    pub(super) fn take_submitted_message(
        &mut self,
        client_id: &str,
    ) -> Option<(UserMessage, UserMessageHistoryRecord, UserMessageSource)> {
        let queue = &mut self.input_queue;
        if queue.pending_user_message_client_id.as_deref() == Some(client_id) {
            queue.pending_user_message_client_id = None;
            queue.user_turn_pending_start = false;
            let history = queue
                .pending_user_message_history_record
                .take()
                .unwrap_or(UserMessageHistoryRecord::UserMessageText);
            self.safety_buffering_prompt
                .take()
                .map(|message| (message, history, self.safety_buffering_source))
        } else {
            queue
                .take_pending_steer(client_id)
                .map(|p| (p.user_message, p.history_record, p.source))
        }
    }

    pub(crate) fn hold_unconfirmed_user_message(&mut self, client_id: &str) {
        if let Some((message, record, source)) = self.take_submitted_message(client_id) {
            self.input_queue
                .queued_user_messages
                .push_front(QueuedUserMessage {
                    source,
                    delivery: MessageDelivery::Unconfirmed(Some(client_id.to_owned())),
                    ..message.into()
                });
            self.input_queue
                .queued_user_message_history_records
                .push_front(record);
            self.refresh_pending_input_preview();
            self.update_task_running_state();
        }
    }

    pub(crate) fn restore_rejected_user_message(&mut self, client_id: &str, message: String) {
        if let Some((draft, history, _)) = self.take_submitted_message(client_id) {
            self.restore_user_message_to_composer(user_message_for_restore(draft, &history));
        }
        self.add_error_message(message);
        self.refresh_pending_input_preview();
        self.update_task_running_state();
    }
}
