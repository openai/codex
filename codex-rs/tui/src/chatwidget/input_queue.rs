//! Queued user input and pending-steer state for `ChatWidget`.
//!
//! This module keeps the mutable input queues together so `ChatWidget` can
//! apply UI/protocol effects around a focused reducer-style state bag.

use std::collections::VecDeque;

use codex_app_server_protocol::QueuedTurn;
use codex_app_server_protocol::QueuedTurnStatus;
use codex_app_server_protocol::UserInput;

use super::PendingSteer;
use super::QueuedUserMessage;
use super::UserMessage;
use super::UserMessageHistoryRecord;
use super::user_message_preview_text;

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct PendingInputPreview {
    pub(super) queued_messages: Vec<String>,
    pub(super) pending_steers: Vec<String>,
    pub(super) rejected_steers: Vec<String>,
    pub(super) has_editable_queued_message: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum ServerQueueBarrier {
    #[default]
    Inactive,
    WaitingForSnapshot,
    WaitingForTurn,
    TurnRunning,
}

#[derive(Debug, Default)]
pub(super) struct InputQueueState {
    /// User inputs queued while a turn is in progress.
    pub(super) queued_user_messages: VecDeque<QueuedUserMessage>,
    /// User inputs persisted by app-server while a turn is in progress.
    pub(super) server_queued_messages: Vec<String>,
    pub(super) server_queue_has_pending_turn: bool,
    pub(super) server_queue_barrier: ServerQueueBarrier,
    /// History records for queued user messages. Slash commands such as `/goal`
    /// can render history that differs from the text submitted to core, so this
    /// stays in lockstep with `queued_user_messages`, with missing entries
    /// treated as user-message text.
    pub(super) queued_user_message_history_records: VecDeque<UserMessageHistoryRecord>,
    /// A user turn has been submitted to core, but `TurnStarted` has not arrived yet.
    pub(super) user_turn_pending_start: bool,
    /// User messages that tried to steer a non-regular turn and must be retried first.
    pub(super) rejected_steers_queue: VecDeque<UserMessage>,
    /// History records for rejected steers. Slash commands such as `/goal` can
    /// render history that differs from the text submitted to core, so this stays
    /// in lockstep with `rejected_steers_queue`, with missing entries treated as
    /// user-message text.
    pub(super) rejected_steer_history_records: VecDeque<UserMessageHistoryRecord>,
    /// Steers already submitted to core but not yet committed into history.
    pub(super) pending_steers: VecDeque<PendingSteer>,
    /// When set, the next interrupt should resubmit all pending steers as one
    /// fresh user turn instead of restoring them into the composer.
    pub(super) submit_pending_steers_after_interrupt: bool,
    pub(super) suppress_queue_autosend: bool,
}

impl InputQueueState {
    pub(super) fn has_queued_follow_up_messages(&self) -> bool {
        !self.rejected_steers_queue.is_empty()
            || !self.queued_user_messages.is_empty()
            || self.server_queue_has_pending_turn
            || self.server_queue_barrier != ServerQueueBarrier::Inactive
    }

    pub(super) fn has_editable_queued_message(&self) -> bool {
        self.has_locally_owned_follow_up_messages()
    }

    pub(super) fn has_locally_owned_follow_up_messages(&self) -> bool {
        !self.rejected_steers_queue.is_empty() || !self.queued_user_messages.is_empty()
    }

    pub(super) fn clear(&mut self) {
        self.queued_user_messages.clear();
        self.server_queued_messages.clear();
        self.server_queue_has_pending_turn = false;
        self.server_queue_barrier = ServerQueueBarrier::Inactive;
        self.queued_user_message_history_records.clear();
        self.user_turn_pending_start = false;
        self.rejected_steers_queue.clear();
        self.rejected_steer_history_records.clear();
        self.pending_steers.clear();
        self.submit_pending_steers_after_interrupt = false;
    }

    pub(super) fn preview(&self) -> PendingInputPreview {
        let queued_messages = self
            .server_queued_messages
            .iter()
            .cloned()
            .chain(
                self.queued_user_messages
                    .iter()
                    .enumerate()
                    .map(|(idx, message)| {
                        user_message_preview_text(
                            message,
                            self.queued_user_message_history_records.get(idx),
                        )
                    }),
            )
            .collect();
        let pending_steers = self
            .pending_steers
            .iter()
            .map(|steer| {
                user_message_preview_text(&steer.user_message, Some(&steer.history_record))
            })
            .collect();
        let rejected_steers = self
            .rejected_steers_queue
            .iter()
            .enumerate()
            .map(|(idx, message)| {
                user_message_preview_text(message, self.rejected_steer_history_records.get(idx))
            })
            .collect();

        PendingInputPreview {
            queued_messages,
            pending_steers,
            rejected_steers,
            has_editable_queued_message: self.has_editable_queued_message(),
        }
    }

    pub(super) fn set_server_queued_turns(
        &mut self,
        queued_turns: Vec<QueuedTurn>,
        dispatching_queued_turn_id: Option<String>,
    ) {
        self.server_queue_has_pending_turn = false;
        self.server_queued_messages = queued_turns
            .into_iter()
            .map(|queued_turn| {
                let preview = queued_turn
                    .submission
                    .input
                    .into_iter()
                    .find_map(|item| match item {
                        UserInput::Text { text, .. } => Some(
                            crate::ide_context::extract_prompt_request_with_offset(&text)
                                .0
                                .to_string(),
                        ),
                        _ => None,
                    })
                    .unwrap_or_else(|| "[queued input]".to_string());
                match queued_turn.status {
                    QueuedTurnStatus::Pending => {
                        self.server_queue_has_pending_turn = true;
                        preview
                    }
                    QueuedTurnStatus::Failed { error } => {
                        format!("[failed: {}] {preview}", error.message)
                    }
                }
            })
            .collect();
        if dispatching_queued_turn_id.is_some()
            && self.server_queue_barrier != ServerQueueBarrier::TurnRunning
        {
            self.server_queue_barrier = ServerQueueBarrier::WaitingForTurn;
        } else if !self.server_queue_has_pending_turn
            && self.server_queue_barrier != ServerQueueBarrier::TurnRunning
        {
            self.server_queue_barrier = ServerQueueBarrier::Inactive;
        } else if self.server_queue_has_pending_turn
            && self.server_queue_barrier != ServerQueueBarrier::TurnRunning
        {
            self.server_queue_barrier = ServerQueueBarrier::WaitingForTurn;
        }
    }

    pub(super) fn note_server_queue_submission(&mut self) {
        if self.server_queue_barrier == ServerQueueBarrier::Inactive {
            self.server_queue_barrier = ServerQueueBarrier::WaitingForSnapshot;
        }
    }

    pub(super) fn note_server_queue_submission_failed(&mut self) {
        if self.server_queue_barrier == ServerQueueBarrier::WaitingForSnapshot {
            self.server_queue_barrier = ServerQueueBarrier::Inactive;
        }
    }

    pub(super) fn note_server_queue_turn_started(&mut self) {
        if self.server_queue_barrier == ServerQueueBarrier::WaitingForTurn {
            self.server_queue_barrier = ServerQueueBarrier::TurnRunning;
        }
    }

    pub(super) fn note_server_queue_turn_completed(&mut self) {
        if matches!(
            self.server_queue_barrier,
            ServerQueueBarrier::WaitingForTurn | ServerQueueBarrier::TurnRunning
        ) {
            self.server_queue_barrier = if self.server_queue_has_pending_turn {
                ServerQueueBarrier::WaitingForTurn
            } else {
                ServerQueueBarrier::Inactive
            };
        }
    }

    pub(super) fn blocks_local_queue_autosend(&self) -> bool {
        self.server_queue_barrier != ServerQueueBarrier::Inactive
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn preview_keeps_queue_categories_separate() {
        let mut state = InputQueueState::default();
        state
            .queued_user_messages
            .push_back(UserMessage::from("queued").into());
        state
            .server_queued_messages
            .push("server queued".to_string());
        state.server_queue_barrier = ServerQueueBarrier::TurnRunning;
        state
            .rejected_steers_queue
            .push_back(UserMessage::from("rejected"));
        state.pending_steers.push_back(PendingSteer {
            user_message: UserMessage::from("pending"),
            history_record: UserMessageHistoryRecord::UserMessageText,
            compare_key: crate::chatwidget::user_messages::PendingSteerCompareKey {
                message: "pending".to_string(),
                image_count: 0,
            },
        });

        assert_eq!(
            state.preview(),
            PendingInputPreview {
                queued_messages: vec!["server queued".to_string(), "queued".to_string()],
                pending_steers: vec!["pending".to_string()],
                rejected_steers: vec!["rejected".to_string()],
                has_editable_queued_message: true,
            }
        );
    }

    #[test]
    fn clear_resets_all_input_queues() {
        let mut state = InputQueueState::default();
        state
            .queued_user_messages
            .push_back(UserMessage::from("queued").into());
        state
            .server_queued_messages
            .push("server queued".to_string());
        state
            .rejected_steers_queue
            .push_back(UserMessage::from("rejected"));
        state.user_turn_pending_start = true;
        state.submit_pending_steers_after_interrupt = true;

        state.clear();

        assert!(state.queued_user_messages.is_empty());
        assert!(state.server_queued_messages.is_empty());
        assert!(!state.server_queue_has_pending_turn);
        assert_eq!(state.server_queue_barrier, ServerQueueBarrier::Inactive);
        assert!(state.queued_user_message_history_records.is_empty());
        assert!(!state.user_turn_pending_start);
        assert!(state.rejected_steers_queue.is_empty());
        assert!(state.rejected_steer_history_records.is_empty());
        assert!(state.pending_steers.is_empty());
        assert!(!state.submit_pending_steers_after_interrupt);
    }

    #[test]
    fn server_queue_barrier_waits_for_snapshot_before_tracking_turn_lifecycle() {
        let mut state = InputQueueState::default();

        state.note_server_queue_submission();
        state.note_server_queue_turn_started();
        assert_eq!(
            state.server_queue_barrier,
            ServerQueueBarrier::WaitingForSnapshot
        );

        state.set_server_queued_turns(
            vec![QueuedTurn {
                id: "queued".to_string(),
                submission: Default::default(),
                status: QueuedTurnStatus::Pending,
            }],
            /*dispatching_queued_turn_id*/ None,
        );
        assert_eq!(
            state.server_queue_barrier,
            ServerQueueBarrier::WaitingForTurn
        );

        state.set_server_queued_turns(Vec::new(), Some("queued".to_string()));
        state.note_server_queue_turn_started();
        assert_eq!(state.server_queue_barrier, ServerQueueBarrier::TurnRunning);
        state.note_server_queue_turn_completed();
        assert_eq!(state.server_queue_barrier, ServerQueueBarrier::Inactive);
    }

    #[test]
    fn failed_server_queue_preview_includes_error() {
        let mut state = InputQueueState::default();
        state.set_server_queued_turns(
            vec![QueuedTurn {
                id: "queued".to_string(),
                submission: Default::default(),
                status: QueuedTurnStatus::Failed {
                    error: codex_app_server_protocol::TurnError {
                        message: "dispatch failed".to_string(),
                        codex_error_info: None,
                        additional_details: None,
                    },
                },
            }],
            /*dispatching_queued_turn_id*/ None,
        );

        assert_eq!(
            state.server_queued_messages,
            vec!["[failed: dispatch failed] [queued input]".to_string()]
        );
        assert_eq!(state.server_queue_barrier, ServerQueueBarrier::Inactive);
        assert!(!state.has_queued_follow_up_messages());
    }

    #[test]
    fn replayed_dispatch_completion_releases_waiting_barrier() {
        let mut state = InputQueueState::default();
        state.set_server_queued_turns(Vec::new(), Some("queued".to_string()));
        assert_eq!(
            state.server_queue_barrier,
            ServerQueueBarrier::WaitingForTurn
        );

        state.note_server_queue_turn_completed();

        assert_eq!(state.server_queue_barrier, ServerQueueBarrier::Inactive);
    }

    #[test]
    fn empty_server_queue_snapshot_releases_waiting_barrier_without_dispatch() {
        let mut state = InputQueueState::default();
        state.note_server_queue_submission();
        state.set_server_queued_turns(
            vec![QueuedTurn {
                id: "queued".to_string(),
                submission: Default::default(),
                status: QueuedTurnStatus::Pending,
            }],
            /*dispatching_queued_turn_id*/ None,
        );

        state.set_server_queued_turns(Vec::new(), /*dispatching_queued_turn_id*/ None);

        assert_eq!(state.server_queue_barrier, ServerQueueBarrier::Inactive);
    }

    #[test]
    fn server_queue_preview_strips_injected_ide_context() {
        let mut state = InputQueueState::default();
        state.set_server_queued_turns(
            vec![QueuedTurn {
                id: "queued".to_string(),
                submission: codex_app_server_protocol::TurnSubmission {
                    input: vec![UserInput::Text {
                        text: "# Context from my IDE setup:\n\n## My request for Codex:\nqueue me"
                            .to_string(),
                        text_elements: Vec::new(),
                    }],
                    ..Default::default()
                },
                status: QueuedTurnStatus::Pending,
            }],
            /*dispatching_queued_turn_id*/ None,
        );

        assert_eq!(state.server_queued_messages, vec!["queue me".to_string()]);
    }
}
