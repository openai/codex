//! Queued user input and pending-steer state for `ChatWidget`.
//!
//! This module keeps the mutable input queues together so `ChatWidget` can
//! apply UI/protocol effects around a focused reducer-style state bag.

use std::collections::VecDeque;

use codex_app_server_protocol::QueuedItem;
use codex_app_server_protocol::QueuedItemProvenance;
use codex_app_server_protocol::QueuedItemStatus;
use codex_app_server_protocol::ThreadQueueListResponse;

use super::ChatWidget;
use super::PendingSteer;
use super::QueuedUserMessage;
use super::UserMessage;
use super::UserMessageHistoryRecord;
use super::user_message_preview_text;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct UserMessageQueueState {
    items: Vec<QueuedItem>,
    has_more: bool,
    pub(crate) refresh_in_flight: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct PendingInputPreview {
    pub(super) queued_messages: Vec<String>,
    pub(super) pending_steers: Vec<String>,
    pub(super) rejected_steers: Vec<String>,
    pub(super) has_editable_queued_message: bool,
}

#[derive(Debug, Default)]
pub(super) struct InputQueueState {
    pub(super) user_message_queue: UserMessageQueueState,
    /// User inputs queued while a turn is in progress.
    pub(super) queued_user_messages: VecDeque<QueuedUserMessage>,
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

impl UserMessageQueueState {
    pub(crate) fn set_snapshot(&mut self, response: ThreadQueueListResponse) {
        let ThreadQueueListResponse { data, next_cursor } = response;
        self.items = data;
        self.has_more = next_cursor.is_some();
        self.refresh_in_flight = false;
    }

    fn clear(&mut self) {
        self.items.clear();
        self.has_more = false;
        self.refresh_in_flight = false;
    }
}

impl InputQueueState {
    pub(super) fn has_local_follow_up_messages(&self) -> bool {
        !self.rejected_steers_queue.is_empty() || !self.queued_user_messages.is_empty()
    }

    pub(super) fn has_queued_follow_up_messages(&self) -> bool {
        self.has_server_follow_up_messages() || self.has_local_follow_up_messages()
    }

    pub(super) fn blocks_local_queue_autosend(&self) -> bool {
        self.user_message_queue.refresh_in_flight || self.has_server_follow_up_messages()
    }

    fn has_server_follow_up_messages(&self) -> bool {
        self.user_message_queue.has_more
            || self
                .user_message_queue
                .items
                .iter()
                .any(|item| matches!(item.status, QueuedItemStatus::Pending))
    }

    pub(super) fn clear(&mut self) {
        self.user_message_queue.clear();
        self.queued_user_messages.clear();
        self.queued_user_message_history_records.clear();
        self.user_turn_pending_start = false;
        self.rejected_steers_queue.clear();
        self.rejected_steer_history_records.clear();
        self.pending_steers.clear();
        self.submit_pending_steers_after_interrupt = false;
    }

    pub(super) fn preview(&self) -> PendingInputPreview {
        let queued_messages = self
            .user_message_queue
            .items
            .iter()
            .map(server_queued_item_preview)
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
            has_editable_queued_message: !self.queued_user_messages.is_empty(),
        }
    }
}

fn server_queued_item_preview(item: &QueuedItem) -> String {
    let display = ChatWidget::user_message_display_from_inputs(&item.submission.input);
    let mut parts = Vec::new();
    if !display.message.is_empty() {
        parts.push(display.message);
    }
    parts.extend(
        std::iter::repeat_n("[image]".to_string(), display.local_images.len()).chain(
            std::iter::repeat_n("[image]".to_string(), display.remote_image_urls.len()),
        ),
    );
    if parts.is_empty() {
        parts.push("[queued input]".to_string());
    }
    let mut preview = parts.join("\n");
    if let QueuedItemProvenance::ExternalEvent { source, .. } = &item.provenance {
        preview = format!("[{source}] {preview}");
    }
    if let QueuedItemStatus::Failed { error } = &item.status {
        preview = format!("[failed: {error}] {preview}");
    }
    preview
}

#[cfg(test)]
#[path = "input_queue_tests.rs"]
mod tests;
