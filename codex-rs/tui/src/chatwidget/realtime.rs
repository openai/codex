use super::*;
use codex_config::config_toml::RealtimeTransport;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationStartTransport;
use codex_protocol::protocol::RealtimeAudioFrame;
use codex_protocol::protocol::RealtimeConversationClosedEvent;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationStartedEvent;
use codex_protocol::protocol::RealtimeEvent;
use codex_realtime_webrtc::RealtimeWebrtcEvent;
use codex_realtime_webrtc::RealtimeWebrtcSession;
use codex_realtime_webrtc::RealtimeWebrtcSessionHandle;
#[cfg(not(target_os = "linux"))]
use std::sync::atomic::AtomicU16;
#[cfg(not(target_os = "linux"))]
use std::time::Duration;

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
    pub(super) phase: RealtimeConversationPhase,
    requested_close: bool,
    session_id: Option<String>,
    warned_audio_only_submission: bool,
    transport: RealtimeConversationUiTransport,
    #[cfg(not(target_os = "linux"))]
    pub(super) meter_placeholder_id: Option<String>,
    #[cfg(not(target_os = "linux"))]
    capture_stop_flag: Option<Arc<AtomicBool>>,
    #[cfg(not(target_os = "linux"))]
    capture: Option<crate::voice::VoiceCapture>,
    #[cfg(not(target_os = "linux"))]
    audio_player: Option<crate::voice::RealtimeAudioPlayer>,
}

#[derive(Debug, Default)]
enum RealtimeConversationUiTransport {
    #[default]
    Websocket,
    Webrtc {
        handle: Option<RealtimeWebrtcSessionHandle>,
    },
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

    #[cfg(not(target_os = "linux"))]
    pub(super) fn is_active(&self) -> bool {
        matches!(self.phase, RealtimeConversationPhase::Active)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RenderedUserMessageEvent {
    pub(super) message: String,
    pub(super) remote_image_urls: Vec<String>,
    pub(super) local_images: Vec<PathBuf>,
    pub(super) text_elements: Vec<TextElement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PendingSteerCompareKey {
    pub(super) message: String,
    pub(super) image_count: usize,
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

    /// Build the compare key for a submitted pending steer without invoking the
    /// expensive request-serialization path. Pending steers only need to match the
    /// committed `ItemCompleted(UserMessage)` emitted after core drains input, which
    /// preserves flattened text and total image count but not UI-only text ranges or
    /// local image paths.
    pub(super) fn pending_steer_compare_key_from_items(
        items: &[UserInput],
    ) -> PendingSteerCompareKey {
        let mut message = String::new();
        let mut image_count = 0;

        for item in items {
            match item {
                UserInput::Text { text, .. } => message.push_str(text),
                UserInput::Image { .. } | UserInput::LocalImage { .. } => image_count += 1,
                UserInput::Skill { .. } | UserInput::Mention { .. } => {}
                _ => {}
            }
        }

        PendingSteerCompareKey {
            message,
            image_count,
        }
    }

    #[cfg(test)]
    pub(super) fn pending_steer_compare_key_from_item(
        item: &codex_protocol::items::UserMessageItem,
    ) -> PendingSteerCompareKey {
        Self::pending_steer_compare_key_from_items(&item.content)
    }

    #[cfg(test)]
    pub(super) fn rendered_user_message_event_from_inputs(
        items: &[UserInput],
    ) -> RenderedUserMessageEvent {
        let mut message = String::new();
        let mut remote_image_urls = Vec::new();
        let mut local_images = Vec::new();
        let mut text_elements = Vec::new();

        for item in items {
            match item {
                UserInput::Text {
                    text,
                    text_elements: current_text_elements,
                } => append_text_with_rebased_elements(
                    &mut message,
                    &mut text_elements,
                    text,
                    current_text_elements.iter().map(|element| {
                        TextElement::new(
                            element.byte_range,
                            element.placeholder(text).map(str::to_string),
                        )
                    }),
                ),
                UserInput::Image { image_url } => remote_image_urls.push(image_url.clone()),
                UserInput::LocalImage { path } => local_images.push(path.clone()),
                UserInput::Skill { .. } | UserInput::Mention { .. } => {}
                _ => {}
            }
        }

        Self::rendered_user_message_event_from_parts(
            message,
            text_elements,
            local_images,
            remote_image_urls,
        )
    }

    #[cfg(test)]
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
                /*hint*/ None,
            );
        } else {
            self.request_redraw();
        }

        None
    }

    fn realtime_footer_hint_items() -> Vec<(String, String)> {
        vec![("/realtime".to_string(), "stop live voice".to_string())]
    }

    pub(super) fn stop_realtime_conversation_from_ui(&mut self) {
        self.request_realtime_conversation_close(/*info_message*/ None);
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn stop_realtime_conversation_for_deleted_meter(&mut self, id: &str) -> bool {
        if self.realtime_conversation.is_live()
            && self.realtime_conversation.meter_placeholder_id.as_deref() == Some(id)
        {
            self.realtime_conversation.meter_placeholder_id = None;
            self.stop_realtime_conversation_from_ui();
            return true;
        }

        false
    }

    pub(super) fn start_realtime_conversation(&mut self) {
        self.realtime_conversation.phase = RealtimeConversationPhase::Starting;
        self.realtime_conversation.requested_close = false;
        self.realtime_conversation.session_id = None;
        self.realtime_conversation.warned_audio_only_submission = false;
        self.set_footer_hint_override(Some(Self::realtime_footer_hint_items()));
        match self.config.realtime.transport {
            RealtimeTransport::Websocket => {
                self.realtime_conversation.transport = RealtimeConversationUiTransport::Websocket;
                self.submit_realtime_conversation_start(/*transport*/ None);
            }
            RealtimeTransport::WebRtc => {
                self.realtime_conversation.transport =
                    RealtimeConversationUiTransport::Webrtc { handle: None };
                start_realtime_webrtc_offer_task(self.app_event_tx.clone());
            }
        }
        self.request_redraw();
    }

    fn submit_realtime_conversation_start(
        &mut self,
        transport: Option<ConversationStartTransport>,
    ) {
        self.submit_op(AppCommand::realtime_conversation_start(
            ConversationStartParams {
                prompt: None,
                session_id: None,
                transport,
                voice: self.config.realtime.voice,
            },
        ));
    }

    pub(super) fn request_realtime_conversation_close(&mut self, info_message: Option<String>) {
        if !self.realtime_conversation.is_live() {
            if let Some(message) = info_message {
                self.add_info_message(message, /*hint*/ None);
            }
            return;
        }

        self.realtime_conversation.requested_close = true;
        self.realtime_conversation.phase = RealtimeConversationPhase::Stopping;
        self.submit_op(AppCommand::realtime_conversation_close());
        self.stop_realtime_local_audio();
        self.close_realtime_webrtc_transport();
        self.set_footer_hint_override(/*items*/ None);

        if let Some(message) = info_message {
            self.add_info_message(message, /*hint*/ None);
        } else {
            self.request_redraw();
        }
    }

    pub(super) fn reset_realtime_conversation_state(&mut self) {
        self.stop_realtime_local_audio();
        self.close_realtime_webrtc_transport();
        self.set_footer_hint_override(/*items*/ None);
        self.realtime_conversation.phase = RealtimeConversationPhase::Inactive;
        self.realtime_conversation.requested_close = false;
        self.realtime_conversation.session_id = None;
        self.realtime_conversation.warned_audio_only_submission = false;
        self.realtime_conversation.transport = RealtimeConversationUiTransport::Websocket;
    }

    fn fail_realtime_conversation(&mut self, message: String) {
        self.add_error_message(message);
        if self.realtime_conversation.is_live() {
            self.request_realtime_conversation_close(/*info_message*/ None);
        } else {
            self.reset_realtime_conversation_state();
            self.request_redraw();
        }
    }

    pub(super) fn on_realtime_conversation_started(
        &mut self,
        ev: RealtimeConversationStartedEvent,
    ) {
        if !self.realtime_conversation_enabled() {
            self.request_realtime_conversation_close(/*info_message*/ None);
            return;
        }
        self.realtime_conversation.session_id = ev.session_id;
        self.realtime_conversation.warned_audio_only_submission = false;
        self.set_footer_hint_override(Some(Self::realtime_footer_hint_items()));
        if self.realtime_conversation_uses_webrtc() {
            self.realtime_conversation.phase = RealtimeConversationPhase::Starting;
        } else {
            self.realtime_conversation.phase = RealtimeConversationPhase::Active;
            self.start_realtime_local_audio();
        }
        self.request_redraw();
    }

    pub(super) fn on_realtime_conversation_realtime(
        &mut self,
        ev: RealtimeConversationRealtimeEvent,
    ) {
        if self.realtime_conversation_uses_webrtc()
            && matches!(
                ev.payload,
                RealtimeEvent::AudioOut(_)
                    | RealtimeEvent::InputAudioSpeechStarted(_)
                    | RealtimeEvent::ResponseCreated(_)
                    | RealtimeEvent::ResponseCancelled(_)
                    | RealtimeEvent::ResponseDone(_)
            )
        {
            return;
        }
        match ev.payload {
            RealtimeEvent::SessionUpdated { session_id, .. } => {
                self.realtime_conversation.session_id = Some(session_id);
            }
            RealtimeEvent::InputAudioSpeechStarted(_) => self.interrupt_realtime_audio_playback(),
            RealtimeEvent::InputTranscriptDelta(_) => {}
            RealtimeEvent::OutputTranscriptDelta(_) => {}
            RealtimeEvent::AudioOut(frame) => self.enqueue_realtime_audio_out(&frame),
            RealtimeEvent::ResponseCreated(_) => {}
            RealtimeEvent::ResponseCancelled(_) => self.interrupt_realtime_audio_playback(),
            RealtimeEvent::ResponseDone(_) => {}
            RealtimeEvent::ConversationItemAdded(_item) => {}
            RealtimeEvent::ConversationItemDone { .. } => {}
            RealtimeEvent::HandoffRequested(_) => {}
            RealtimeEvent::Error(message) => {
                self.fail_realtime_conversation(format!("Realtime voice error: {message}"));
            }
        }
    }

    pub(super) fn on_realtime_conversation_closed(&mut self, ev: RealtimeConversationClosedEvent) {
        if self.realtime_conversation_uses_webrtc()
            && self.realtime_conversation.is_live()
            && ev.reason.as_deref() == Some("transport_closed")
        {
            return;
        }

        let requested = self.realtime_conversation.requested_close;
        let reason = ev.reason;
        self.reset_realtime_conversation_state();
        if !requested
            && let Some(reason) = reason
            && reason != "error"
        {
            self.add_info_message(
                format!("Realtime voice mode closed: {reason}"),
                /*hint*/ None,
            );
        }
        self.request_redraw();
    }

    pub(super) fn on_realtime_conversation_sdp(&mut self, sdp: String) {
        let RealtimeConversationUiTransport::Webrtc {
            handle: Some(handle),
        } = &self.realtime_conversation.transport
        else {
            return;
        };

        if let Err(err) = handle.apply_answer_sdp(sdp) {
            self.fail_realtime_conversation(format!("Failed to connect realtime WebRTC: {err}"));
        }
    }

    pub(crate) fn on_realtime_webrtc_offer_created(
        &mut self,
        result: Result<crate::app_event::RealtimeWebrtcOffer, String>,
    ) {
        if self.realtime_conversation.phase != RealtimeConversationPhase::Starting
            || !matches!(
                self.realtime_conversation.transport,
                RealtimeConversationUiTransport::Webrtc { handle: None }
            )
        {
            return;
        }

        let offer = match result {
            Ok(offer) => offer,
            Err(err) => {
                self.fail_realtime_conversation(format!("Failed to start realtime WebRTC: {err}"));
                return;
            }
        };

        self.realtime_conversation.transport = RealtimeConversationUiTransport::Webrtc {
            handle: Some(offer.handle),
        };
        self.submit_realtime_conversation_start(Some(ConversationStartTransport::Webrtc {
            sdp: offer.offer_sdp,
        }));
        self.request_redraw();
    }

    pub(crate) fn on_realtime_webrtc_event(&mut self, event: RealtimeWebrtcEvent) {
        if !self.realtime_conversation_uses_webrtc() {
            return;
        }

        match event {
            RealtimeWebrtcEvent::Connected => {
                if self.realtime_conversation.phase != RealtimeConversationPhase::Starting {
                    return;
                }
                self.realtime_conversation.phase = RealtimeConversationPhase::Active;
                self.set_footer_hint_override(Some(Self::realtime_footer_hint_items()));
                self.request_redraw();
            }
            RealtimeWebrtcEvent::Closed => {
                self.reset_realtime_conversation_state();
                self.request_redraw();
            }
            RealtimeWebrtcEvent::Failed(message) => {
                self.fail_realtime_conversation(format!("Realtime WebRTC error: {message}"));
            }
            RealtimeWebrtcEvent::LocalAudioLevel(_) => {}
        }
    }

    pub(crate) fn on_realtime_webrtc_local_audio_level(&mut self, peak: u16) {
        if !self.realtime_conversation_uses_webrtc() || peak == 0 {
            return;
        }

        #[cfg(target_os = "linux")]
        {
            let _ = peak;
        }

        #[cfg(not(target_os = "linux"))]
        {
            let RealtimeConversationUiTransport::Webrtc {
                handle: Some(handle),
            } = &self.realtime_conversation.transport
            else {
                return;
            };
            let peak = handle.local_audio_peak();
            if self.realtime_conversation.meter_placeholder_id.is_none() {
                self.start_realtime_webrtc_meter(peak);
            }
        }
    }

    fn realtime_conversation_uses_webrtc(&self) -> bool {
        matches!(
            self.realtime_conversation.transport,
            RealtimeConversationUiTransport::Webrtc { .. }
        )
    }

    fn close_realtime_webrtc_transport(&mut self) {
        if let RealtimeConversationUiTransport::Webrtc { handle } =
            &mut self.realtime_conversation.transport
            && let Some(handle) = handle.take()
        {
            handle.close();
        }
    }

    fn enqueue_realtime_audio_out(&mut self, frame: &RealtimeAudioFrame) {
        #[cfg(not(target_os = "linux"))]
        {
            if self.realtime_conversation.audio_player.is_none() {
                self.realtime_conversation.audio_player =
                    crate::voice::RealtimeAudioPlayer::start(&self.config).ok();
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
    fn interrupt_realtime_audio_playback(&mut self) {
        if let Some(player) = &self.realtime_conversation.audio_player {
            player.clear();
        }
    }

    #[cfg(target_os = "linux")]
    fn interrupt_realtime_audio_playback(&mut self) {}

    #[cfg(not(target_os = "linux"))]
    fn start_realtime_local_audio(&mut self) {
        if self.realtime_conversation.capture_stop_flag.is_some() {
            return;
        }

        let capture = match crate::voice::VoiceCapture::start_realtime(
            &self.config,
            self.app_event_tx.clone(),
        ) {
            Ok(capture) => capture,
            Err(err) => {
                self.fail_realtime_conversation(format!(
                    "Failed to start microphone capture: {err}"
                ));
                return;
            }
        };

        let stop_flag = capture.stopped_flag();
        let peak = capture.last_peak_arc();
        self.start_realtime_meter(stop_flag.clone(), peak);
        self.realtime_conversation.capture_stop_flag = Some(stop_flag);
        self.realtime_conversation.capture = Some(capture);
        if self.realtime_conversation.audio_player.is_none() {
            self.realtime_conversation.audio_player =
                crate::voice::RealtimeAudioPlayer::start(&self.config).ok();
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn start_realtime_webrtc_meter(&mut self, peak: Arc<AtomicU16>) {
        if self.realtime_conversation.capture_stop_flag.is_some() {
            return;
        }

        let stop_flag = Arc::new(AtomicBool::new(false));
        self.start_realtime_meter(stop_flag.clone(), peak);
        self.realtime_conversation.capture_stop_flag = Some(stop_flag);
    }

    #[cfg(not(target_os = "linux"))]
    fn start_realtime_meter(&mut self, stop_flag: Arc<AtomicBool>, peak: Arc<AtomicU16>) {
        let placeholder_id = self.bottom_pane.insert_recording_meter_placeholder("⠤⠤⠤⠤");
        self.realtime_conversation.meter_placeholder_id = Some(placeholder_id.clone());
        self.request_redraw();

        start_realtime_meter_task(placeholder_id, self.app_event_tx.clone(), stop_flag, peak);
    }

    #[cfg(target_os = "linux")]
    fn start_realtime_local_audio(&mut self) {}

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn restart_realtime_audio_device(&mut self, kind: RealtimeAudioDeviceKind) {
        if !self.realtime_conversation.is_active() {
            return;
        }

        match kind {
            RealtimeAudioDeviceKind::Microphone => {
                self.stop_realtime_microphone();
                self.start_realtime_local_audio();
            }
            RealtimeAudioDeviceKind::Speaker => {
                self.stop_realtime_speaker();
                match crate::voice::RealtimeAudioPlayer::start(&self.config) {
                    Ok(player) => {
                        self.realtime_conversation.audio_player = Some(player);
                    }
                    Err(err) => {
                        self.fail_realtime_conversation(format!(
                            "Failed to start speaker output: {err}"
                        ));
                    }
                }
            }
        }
        self.request_redraw();
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn restart_realtime_audio_device(&mut self, kind: RealtimeAudioDeviceKind) {
        let _ = kind;
    }

    #[cfg(not(target_os = "linux"))]
    fn stop_realtime_local_audio(&mut self) {
        self.stop_realtime_microphone();
        self.stop_realtime_speaker();
    }

    #[cfg(target_os = "linux")]
    fn stop_realtime_local_audio(&mut self) {}

    #[cfg(not(target_os = "linux"))]
    fn stop_realtime_microphone(&mut self) {
        if let Some(flag) = self.realtime_conversation.capture_stop_flag.take() {
            flag.store(true, Ordering::Relaxed);
        }
        if let Some(capture) = self.realtime_conversation.capture.take() {
            capture.stop();
        }
        if let Some(id) = self.realtime_conversation.meter_placeholder_id.take() {
            self.remove_recording_meter_placeholder(&id);
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn stop_realtime_speaker(&mut self) {
        if let Some(player) = self.realtime_conversation.audio_player.take() {
            player.clear();
        }
    }
}

fn start_realtime_webrtc_offer_task(app_event_tx: AppEventSender) {
    std::thread::spawn(move || {
        let result = match RealtimeWebrtcSession::start() {
            Ok(started) => {
                let event_tx = app_event_tx.clone();
                let local_audio_peak = started.handle.local_audio_peak();
                std::thread::spawn(move || {
                    for event in started.events {
                        if let RealtimeWebrtcEvent::LocalAudioLevel(peak) = event {
                            local_audio_peak.store(peak, Ordering::Relaxed);
                            event_tx.send(AppEvent::RealtimeWebrtcLocalAudioLevel(peak));
                        } else {
                            event_tx.send(AppEvent::RealtimeWebrtcEvent(event));
                        }
                    }
                });
                Ok(crate::app_event::RealtimeWebrtcOffer {
                    offer_sdp: started.offer_sdp,
                    handle: started.handle,
                })
            }
            Err(err) => Err(err.to_string()),
        };
        app_event_tx.send(AppEvent::RealtimeWebrtcOfferCreated { result });
    });
}

#[cfg(not(target_os = "linux"))]
fn start_realtime_meter_task(
    meter_placeholder_id: String,
    app_event_tx: AppEventSender,
    stop_flag: Arc<AtomicBool>,
    peak: Arc<AtomicU16>,
) {
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
