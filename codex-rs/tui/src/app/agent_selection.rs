//! Fallible preparation for switching the Parent pane to another agent thread.
//!
//! Preparing separately lets `/side` validate and attach the destination before it tears down the
//! Side pane. Applying the prepared selection stays in `session_lifecycle`, next to the existing
//! widget replacement and replay transition.

use super::*;

pub(super) struct PreparedAgentSelection {
    pub(super) thread_id: ThreadId,
    pub(super) is_replay_only: bool,
    pub(super) attached_replay_only: bool,
}

impl App {
    pub(super) async fn prepare_agent_thread_selection(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> Option<PreparedAgentSelection> {
        if self.chat_widget.active_thread_id == Some(thread_id) {
            return Some(PreparedAgentSelection {
                thread_id,
                is_replay_only: false,
                attached_replay_only: false,
            });
        }

        if !self
            .refresh_agent_picker_thread_liveness(app_server, thread_id)
            .await
        {
            self.chat_widget
                .add_error_message(format!("Agent thread {thread_id} is no longer available."));
            return None;
        }

        let mut is_replay_only = self
            .agent_navigation
            .get(&thread_id)
            .is_some_and(|entry| entry.is_closed);
        let mut attached_replay_only = false;
        if self.should_attach_live_thread_for_selection(thread_id) {
            match self
                .attach_live_thread_for_selection(app_server, thread_id)
                .await
            {
                Ok(live_attached) => {
                    attached_replay_only = !live_attached;
                    if attached_replay_only {
                        is_replay_only = true;
                    }
                }
                Err(err) => {
                    self.chat_widget.add_error_message(format!(
                        "Failed to attach to agent thread {thread_id}: {err}"
                    ));
                    return None;
                }
            }
        } else if !self.thread_event_channels.contains_key(&thread_id) && is_replay_only {
            self.chat_widget
                .add_error_message(format!("Agent thread {thread_id} is no longer available."));
            return None;
        }

        let target_is_attached = self
            .chat_widget
            .by_thread_id(thread_id)
            .is_some_and(|pane| pane.active_thread_id == Some(thread_id));
        let receiver_is_available = self
            .thread_event_channels
            .get(&thread_id)
            .is_some_and(|channel| channel.receiver.is_some());
        if !target_is_attached && !receiver_is_available {
            self.chat_widget
                .add_error_message(format!("Agent thread {thread_id} is already active."));
            return None;
        }

        Some(PreparedAgentSelection {
            thread_id,
            is_replay_only,
            attached_replay_only,
        })
    }
}
