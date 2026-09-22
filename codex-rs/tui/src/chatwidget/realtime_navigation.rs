//! Transfers live voice ownership into a reconstructed view without replaying the call.

use super::*;

impl ChatWidget {
    pub(crate) fn add_realtime_error(&mut self, message: String) {
        if self.app_event_tx.voice_only.load(Ordering::Relaxed)
            && let Some(thread_id) = self.thread_id()
        {
            self.app_event_tx.send(AppEvent::BackgroundVoiceError {
                thread_id,
                message: message.chars().take(/*n*/ 4096).collect(),
            });
        } else {
            self.add_error_message(message);
        }
    }

    pub(crate) fn park_voice(&mut self) {
        self.app_event_tx.voice_only.store(true, Ordering::Relaxed);
        self.set_queue_autosend_suppressed(/*suppressed*/ true);
        self.stop_rate_limit_poller();
    }

    pub(crate) fn prepare_background_voice_replay(
        &mut self,
        owner: &Self,
    ) -> impl Iterator<Item = &str> {
        self.realtime_conversation.pending_speech = owner
            .realtime_conversation
            .pending_speech
            .iter()
            .filter(|pending| !pending.captioned)
            .cloned()
            .collect();
        self.realtime_conversation
            .pending_speech
            .iter()
            .filter_map(|pending| match &pending.item {
                codex_app_server_protocol::ThreadItem::AgentMessage { text, .. } => {
                    Some(text.as_str())
                }
                _ => None,
            })
    }

    pub(crate) fn resume_background_voice(&mut self, owner: &mut ChatWidget) {
        let replay = std::mem::replace(
            &mut self.realtime_conversation,
            std::mem::take(&mut owner.realtime_conversation),
        );
        // The reconstructed transcript has already restored historical captions.
        // Keep its pending insertions, while preserving the live call's partials,
        // delivery acknowledgments, and input generations.
        self.realtime_conversation.pending_history_cells = replay.pending_history_cells;
        self.update_realtime_footer();
        self.refresh_terminal_title();
        self.refresh_thread_usage_after_turn();
    }
}
