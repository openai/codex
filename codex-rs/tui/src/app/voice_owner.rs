//! Keeps the single voice owner's live widget and subscription across navigation.
//! Parked widgets process their own events; only voice and account updates leave their sender.

use super::*;

use crate::app_event::VoiceControl;

impl App {
    pub(super) fn voice_owner_thread_id(&self) -> Option<ThreadId> {
        self.background_voice
            .as_deref()
            .filter(|owner| owner.realtime_conversation_is_running())
            .or_else(|| {
                self.chat_widget
                    .realtime_conversation_is_running()
                    .then_some(&self.chat_widget)
            })
            .and_then(ChatWidget::thread_id)
    }

    pub(super) async fn stop_voice_for_removed_thread(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> Result<()> {
        if let Some(owner) = self.voice_owner_thread_id()
            && (owner == thread_id
                || app_server
                    .thread_read(owner, /*include_turns*/ false)
                    .await?
                    .session_id
                    == thread_id.to_string())
        {
            self.stop_realtime_conversation(app_server).await;
        }
        Ok(())
    }

    pub(super) fn voice_widget_for_thread(
        &mut self,
        thread_id: ThreadId,
    ) -> Option<&mut ChatWidget> {
        if let Some(owner) = self.background_voice.as_deref_mut()
            && owner.thread_id() == Some(thread_id)
        {
            return Some(owner);
        }
        (self.chat_widget.thread_id() == Some(thread_id)).then_some(&mut self.chat_widget)
    }

    pub(super) fn control_voice(&mut self, control: VoiceControl) {
        if let Some(thread_id) = self.voice_owner_thread_id()
            && let Some(owner) = self.voice_widget_for_thread(thread_id)
        {
            match control {
                VoiceControl::Toggle => owner.toggle_realtime_conversation(),
                VoiceControl::Stop => owner.stop_realtime_conversation(),
                VoiceControl::Mute => owner.toggle_realtime_microphone(),
            }
        } else if matches!(control, VoiceControl::Toggle) && !self.reconnect.offline {
            // Preserve the ended owner's transcript before allowing another call.
            self.retire_background_voice();
            self.chat_widget.toggle_realtime_conversation();
        } else if matches!(control, VoiceControl::Mute) {
            self.chat_widget.toggle_realtime_microphone();
        }
        self.repaint_agents_overview();
    }

    pub(super) fn retire_background_voice(&mut self) {
        if let Some(mut owner) = self.background_voice.take() {
            std::mem::swap(&mut self.chat_widget, &mut owner);
            self.retain_realtime_replay_state_before_replace();
            std::mem::swap(&mut self.chat_widget, &mut owner);
        }
    }

    pub(super) fn restore_voice_owner_after_replay(&mut self) {
        if let Some(mut owner) = self
            .background_voice
            .take_if(|owner| owner.thread_id() == self.chat_widget.thread_id())
        {
            self.chat_widget.resume_background_voice(&mut owner);
        }
        if let Some((_, message)) = self
            .background_voice_error
            .take_if(|(thread_id, _)| Some(*thread_id) == self.chat_widget.thread_id())
        {
            self.chat_widget.add_error_message(message);
        }
    }

    pub(super) fn deliver_background_voice_notification(
        &mut self,
        thread_id: ThreadId,
        notification: &ServerNotification,
    ) {
        if matches!(
            notification,
            ServerNotification::ThreadClosed(_)
                | ServerNotification::ThreadArchived(_)
                | ServerNotification::ThreadDeleted(_)
        ) && self.voice_owner_thread_id() == Some(thread_id)
            && let Some(owner) = self.voice_widget_for_thread(thread_id)
        {
            // The owning session is gone; no close acknowledgment can be required.
            owner.reset_realtime_conversation();
        }
        if let Some(owner) = self.background_voice.as_mut()
            && owner.thread_id() == Some(thread_id)
        {
            owner.handle_server_notification(notification.clone(), /*replay_kind*/ None);
        }
        if self
            .background_voice
            .as_ref()
            .is_some_and(|owner| !owner.may_receive_realtime_transcripts())
        {
            self.retire_background_voice();
        }
    }

    pub(super) async fn detach_current_thread_for_navigation(
        &mut self,
        app_server: &mut AppServerSession,
        destination: Option<ThreadId>,
    ) {
        self.shutdown_side_threads(app_server).await;
        let thread_ids: Vec<_> = self.thread_event_channels.keys().copied().collect();
        for thread_id in thread_ids {
            if self.voice_owner_thread_id() == Some(thread_id)
                || destination == Some(thread_id)
                || self
                    .agents_overview
                    .dispatched_requests
                    .contains_key(&thread_id)
                || self.agents_overview.blank_sessions.contains_key(&thread_id)
            {
                continue;
            }
            if let Err(err) = app_server.thread_unsubscribe(thread_id).await {
                tracing::warn!("failed to unsubscribe thread {thread_id}: {err}");
            }
            self.abort_thread_event_listener(thread_id);
            self.pending_server_profiles.remove(&thread_id);
        }
    }
}
