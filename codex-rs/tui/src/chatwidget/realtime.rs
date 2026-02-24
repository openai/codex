use super::*;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::RealtimeAudioFrame;
use codex_protocol::protocol::RealtimeConversationClosedEvent;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationStartedEvent;
use codex_protocol::protocol::RealtimeEvent;

const REALTIME_CONVERSATION_PROMPT: &str = "You are in a realtime voice conversation in the Codex TUI. Respond conversationally and concisely.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum RealtimeConversationPhase {
    #[default]
    Inactive,
    Starting,
    Active,
    Stopping,
}

#[derive(Default)]
pub(super) struct RealtimeConversationUiState {
    phase: RealtimeConversationPhase,
    requested_close: bool,
    session_id: Option<String>,
    warned_audio_only_submission: bool,
    meter_placeholder_id: Option<String>,
    #[cfg(not(target_os = "linux"))]
    capture_stop_flag: Option<Arc<AtomicBool>>,
    #[cfg(not(target_os = "linux"))]
    capture: Option<crate::voice::VoiceCapture>,
    #[cfg(not(target_os = "linux"))]
    audio_player: Option<crate::voice::RealtimeAudioPlayer>,
}

impl RealtimeConversationUiState {
    pub(super) fn is_live(&self) -> bool {
        matches!(
            self.phase,
            RealtimeConversationPhase::Starting
                | RealtimeConversationPhase::Active
                | RealtimeConversationPhase::Stopping
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RenderedUserMessageEvent {
    message: String,
    remote_image_urls: Vec<String>,
    local_images: Vec<PathBuf>,
    text_elements: Vec<TextElement>,
}

impl ChatWidget {
    pub(super) fn rendered_user_message_event_from_parts(
        message: String,
        text_elements: Vec<TextElement>,
        local_images: Vec<PathBuf>,
        remote_image_urls: Vec<String>,
    ) -> RenderedUserMessageEvent {
        RenderedUserMessageEvent {
            message,
            remote_image_urls,
            local_images,
            text_elements,
        }
    }

    pub(super) fn rendered_user_message_event_from_event(
        event: &UserMessageEvent,
    ) -> RenderedUserMessageEvent {
        Self::rendered_user_message_event_from_parts(
            event.message.clone(),
            event.text_elements.clone(),
            event.local_images.clone(),
            event.images.clone().unwrap_or_default(),
        )
    }

    pub(super) fn should_render_realtime_user_message_event(
        &self,
        event: &UserMessageEvent,
    ) -> bool {
        if !self.realtime_conversation.is_live() {
            return false;
        }
        let key = Self::rendered_user_message_event_from_event(event);
        self.last_rendered_user_message_event.as_ref() != Some(&key)
    }

    pub(super) fn maybe_defer_user_message_for_realtime(
        &mut self,
        user_message: UserMessage,
    ) -> Option<UserMessage> {
        if !self.realtime_conversation.is_live() {
            return Some(user_message);
        }

        self.restore_user_message_to_composer(user_message);
        if !self.realtime_conversation.warned_audio_only_submission {
            self.realtime_conversation.warned_audio_only_submission = true;
            self.add_info_message(
                "Realtime voice mode is audio-only. Use /realtime to stop.".to_string(),
                None,
            );
        } else {
            self.request_redraw();
        }

        None
    }

    fn realtime_footer_hint_items() -> Vec<(String, String)> {
        vec![("/realtime".to_string(), "stop live voice".to_string())]
    }

    pub(super) fn start_realtime_conversation(&mut self) {
        self.realtime_conversation.phase = RealtimeConversationPhase::Starting;
        self.realtime_conversation.requested_close = false;
        self.realtime_conversation.session_id = None;
        self.realtime_conversation.warned_audio_only_submission = false;
        self.set_footer_hint_override(Some(Self::realtime_footer_hint_items()));
        self.submit_op(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: REALTIME_CONVERSATION_PROMPT.to_string(),
            session_id: None,
        }));
        self.request_redraw();
    }

    pub(super) fn request_realtime_conversation_close(&mut self, info_message: Option<String>) {
        if !self.realtime_conversation.is_live() {
            if let Some(message) = info_message {
                self.add_info_message(message, None);
            }
            return;
        }

        self.realtime_conversation.requested_close = true;
        self.realtime_conversation.phase = RealtimeConversationPhase::Stopping;
        self.submit_op(Op::RealtimeConversationClose);
        self.stop_realtime_local_audio();
        self.set_footer_hint_override(None);

        if let Some(message) = info_message {
            self.add_info_message(message, None);
        } else {
            self.request_redraw();
        }
    }

    pub(super) fn reset_realtime_conversation_state(&mut self) {
        self.stop_realtime_local_audio();
        self.set_footer_hint_override(None);
        self.realtime_conversation.phase = RealtimeConversationPhase::Inactive;
        self.realtime_conversation.requested_close = false;
        self.realtime_conversation.session_id = None;
        self.realtime_conversation.warned_audio_only_submission = false;
    }

    pub(super) fn on_realtime_conversation_started(
        &mut self,
        ev: RealtimeConversationStartedEvent,
    ) {
        if !self.realtime_conversation_enabled() {
            self.submit_op(Op::RealtimeConversationClose);
            self.reset_realtime_conversation_state();
            return;
        }
        self.realtime_conversation.phase = RealtimeConversationPhase::Active;
        self.realtime_conversation.session_id = ev.session_id;
        self.realtime_conversation.warned_audio_only_submission = false;
        self.set_footer_hint_override(Some(Self::realtime_footer_hint_items()));
        self.start_realtime_local_audio();
        self.request_redraw();
    }

    pub(super) fn on_realtime_conversation_realtime(
        &mut self,
        ev: RealtimeConversationRealtimeEvent,
    ) {
        match ev.payload {
            RealtimeEvent::SessionCreated { session_id } => {
                self.realtime_conversation.session_id = Some(session_id);
            }
            RealtimeEvent::SessionUpdated { .. } => {}
            RealtimeEvent::AudioOut(frame) => self.enqueue_realtime_audio_out(&frame),
            RealtimeEvent::ConversationItemAdded(_item) => {}
            RealtimeEvent::Error(message) => {
                self.add_error_message(format!("Realtime voice error: {message}"));
                self.reset_realtime_conversation_state();
            }
        }
    }

    pub(super) fn on_realtime_conversation_closed(&mut self, ev: RealtimeConversationClosedEvent) {
        let requested = self.realtime_conversation.requested_close;
        let reason = ev.reason;
        self.reset_realtime_conversation_state();
        if !requested && let Some(reason) = reason {
            self.add_info_message(format!("Realtime voice mode closed: {reason}"), None);
        }
        self.request_redraw();
    }

    fn enqueue_realtime_audio_out(&mut self, frame: &RealtimeAudioFrame) {
        #[cfg(not(target_os = "linux"))]
        {
            if self.realtime_conversation.audio_player.is_none() {
                self.realtime_conversation.audio_player =
                    crate::voice::RealtimeAudioPlayer::start().ok();
            }
            if let Some(player) = &self.realtime_conversation.audio_player
                && let Err(err) = player.enqueue_frame(frame)
            {
                warn!("failed to play realtime audio: {err}");
            }
        }
        #[cfg(target_os = "linux")]
        {
            let _ = frame;
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn start_realtime_local_audio(&mut self) {
        if self.realtime_conversation.capture_stop_flag.is_some() {
            return;
        }

        let placeholder_id = self.bottom_pane.insert_transcription_placeholder("⠤⠤⠤⠤");
        self.realtime_conversation.meter_placeholder_id = Some(placeholder_id.clone());
        self.request_redraw();

        let capture = match crate::voice::VoiceCapture::start_realtime(self.app_event_tx.clone()) {
            Ok(capture) => capture,
            Err(err) => {
                self.remove_transcription_placeholder(&placeholder_id);
                self.realtime_conversation.meter_placeholder_id = None;
                self.add_error_message(format!("Failed to start microphone capture: {err}"));
                return;
            }
        };

        let stop_flag = capture.stopped_flag();
        let peak = capture.last_peak_arc();
        let meter_placeholder_id = placeholder_id;
        let app_event_tx = self.app_event_tx.clone();

        self.realtime_conversation.capture_stop_flag = Some(stop_flag.clone());
        self.realtime_conversation.capture = Some(capture);
        if self.realtime_conversation.audio_player.is_none() {
            self.realtime_conversation.audio_player =
                crate::voice::RealtimeAudioPlayer::start().ok();
        }

        std::thread::spawn(move || {
            let mut meter = crate::voice::RecordingMeterState::new();

            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }

                let meter_text = meter.next_text(peak.load(Ordering::Relaxed));
                app_event_tx.send(AppEvent::UpdateRecordingMeter {
                    id: meter_placeholder_id.clone(),
                    text: meter_text,
                });

                std::thread::sleep(Duration::from_millis(60));
            }
        });
    }

    #[cfg(target_os = "linux")]
    fn start_realtime_local_audio(&mut self) {}

    #[cfg(not(target_os = "linux"))]
    fn stop_realtime_local_audio(&mut self) {
        if let Some(flag) = self.realtime_conversation.capture_stop_flag.take() {
            flag.store(true, Ordering::Relaxed);
        }
        if let Some(capture) = self.realtime_conversation.capture.take() {
            let _ = capture.stop();
        }
        if let Some(id) = self.realtime_conversation.meter_placeholder_id.take() {
            self.remove_transcription_placeholder(&id);
        }
        if let Some(player) = self.realtime_conversation.audio_player.take() {
            player.clear();
        }
    }

    #[cfg(target_os = "linux")]
    fn stop_realtime_local_audio(&mut self) {
        self.realtime_conversation.meter_placeholder_id = None;
    }
}
