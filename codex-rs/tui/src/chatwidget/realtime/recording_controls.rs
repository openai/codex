//! Live voice capture controls, activity meters, and interruption presentation.
//! Meter samples remain bounded and the current thread owns every shortcut.

use super::*;

impl ChatWidget {
    pub(in crate::chatwidget) fn toggle_realtime_microphone(&mut self) {
        let muted = !self.realtime_conversation.microphone_muted;
        if let Some(handle) = self.realtime_conversation.handle.as_ref() {
            if let Err(error) = handle.set_microphone_muted(muted) {
                self.on_realtime_error(format!("Failed to update microphone: {error}"));
                return;
            }
        } else if self.realtime_conversation.phase != RealtimeConversationPhase::Starting {
            self.add_error_message("Start voice mode before muting the microphone.".to_string());
            return;
        }

        self.realtime_conversation.microphone_muted = muted;
        self.realtime_conversation.microphone_level = 0;
        self.realtime_conversation.microphone_intensity = 0;
        if muted {
            self.realtime_conversation.microphone_history = VoiceAmplitudeHistory::default();
            for (microphone, _) in &mut self.realtime_conversation.audio_meter_history {
                *microphone = 0;
            }
            self.realtime_conversation.interruption_acknowledged_until = None;
        }
        self.update_realtime_footer();
        self.refresh_terminal_title();
    }

    pub(crate) fn realtime_microphone_shortcut_available(&self) -> bool {
        (self.realtime_conversation.phase == RealtimeConversationPhase::Active
            || (self.realtime_conversation.phase == RealtimeConversationPhase::Starting
                && self.realtime_conversation.handle.is_some()))
            && self.realtime_conversation.thread_id.is_some()
            && self.realtime_conversation.thread_id == self.thread_id()
            && self.bottom_pane.no_modal_or_popup_active()
    }

    pub(crate) fn handle_realtime_microphone_shortcut(&mut self, key_event: KeyEvent) -> bool {
        if key_event.kind != KeyEventKind::Press
            || !self.chat_keymap.toggle_voice_mute.is_pressed(key_event)
            || !self.realtime_microphone_shortcut_available()
        {
            return false;
        }
        self.toggle_realtime_microphone();
        true
    }

    pub(in crate::chatwidget) fn realtime_microphone_is_listening(&self) -> bool {
        !self.realtime_conversation.microphone_muted
            && self.realtime_conversation.thread_id.is_some()
            && self.realtime_conversation.thread_id == self.thread_id()
            && match self.realtime_conversation.phase {
                RealtimeConversationPhase::Active => true,
                RealtimeConversationPhase::Starting => self.realtime_conversation.handle.is_some(),
                RealtimeConversationPhase::Inactive | RealtimeConversationPhase::Stopping => false,
            }
    }

    pub(super) fn release_realtime_speaker(&mut self) {
        let acknowledged = self
            .realtime_conversation
            .interruption_acknowledged_until
            .take()
            .is_some();
        self.realtime_conversation.speaker_suppression_generation = None;
        if let Some(handle) = self.realtime_conversation.handle.as_ref() {
            handle.set_speaker_suppressed(/*suppressed*/ false);
        }
        if acknowledged {
            self.update_realtime_footer();
        }
    }

    pub(super) fn suppress_active_realtime_speaker(&mut self) {
        // Quiet turns must not wait for captions before accepting their first
        // audio packets. Only interrupt output that may belong to an older turn.
        if self
            .realtime_conversation
            .assistant_transcript_generation
            .is_some()
            || self
                .realtime_conversation
                .pending_speech
                .iter()
                .any(|pending| {
                    pending.input_generation != self.realtime_conversation.input_generation
                        && !pending.captioned
                        && matches!(
                            pending.state,
                            PendingSpeechState::Queued(_) | PendingSpeechState::Accepted
                        )
                })
            || self.realtime_conversation.speaker_level > 0
            || self
                .realtime_conversation
                .speaker_active_until
                .is_some_and(|deadline| deadline > Instant::now())
            || self
                .realtime_conversation
                .speaker_suppression_generation
                .is_some()
        {
            self.suppress_realtime_speaker();
        }
    }

    pub(super) fn suppress_realtime_speaker(&mut self) {
        self.realtime_conversation.speaker_suppression_generation =
            Some(self.realtime_conversation.input_generation);
        if let Some(handle) = self.realtime_conversation.handle.as_ref() {
            handle.set_speaker_suppressed(/*suppressed*/ true);
        }
        self.realtime_conversation.speaker_level = 0;
        self.realtime_conversation.speaker_intensity = 0;
        for (_, speaker) in &mut self.realtime_conversation.audio_meter_history {
            *speaker = 0;
        }
        self.realtime_conversation.speaker_active_until = None;
        self.update_realtime_footer();
    }

    pub(super) fn resume_realtime_speaker_for(&mut self, role: &str, text: &str) {
        if role == "assistant"
            && !text.trim().is_empty()
            && self.realtime_conversation.assistant_transcript_generation
                == Some(self.realtime_conversation.input_generation)
            && self.realtime_conversation.latest_input_was_voice
            && self.realtime_conversation.speaker_suppression_generation
                == Some(self.realtime_conversation.input_generation)
        {
            self.release_realtime_speaker();
        }
    }

    pub(in crate::chatwidget) fn refresh_realtime_microphone_level(&mut self) {
        if !matches!(
            self.realtime_conversation.phase,
            RealtimeConversationPhase::Starting | RealtimeConversationPhase::Active
        ) {
            return;
        }
        let Some(handle) = self.realtime_conversation.handle.as_ref() else {
            return;
        };
        if let Some(error) = handle.take_error() {
            self.on_realtime_error(format!("Voice conversation failed: {error}"));
            return;
        }
        if self.realtime_conversation.phase == RealtimeConversationPhase::Starting {
            self.frame_requester
                .schedule_frame_in(MICROPHONE_METER_INTERVAL);
            return;
        }
        let handle = handle.clone();
        self.refresh_realtime_audio_meters(Instant::now(), || {
            (handle.take_microphone_peak(), handle.take_speaker_peak())
        });
    }

    pub(super) fn refresh_realtime_audio_meters(
        &mut self,
        now: Instant,
        take_peaks: impl FnOnce() -> (u16, u16),
    ) {
        // Redraws are not sampling ticks: updating the footer itself requests a redraw.
        // Leave the peaks untouched until a full sampling interval has elapsed.
        if let Some(next_sample_at) = self.realtime_conversation.next_audio_meter_sample_at
            && now < next_sample_at
        {
            self.frame_requester.schedule_frame_in(next_sample_at - now);
            return;
        }
        self.realtime_conversation.next_audio_meter_sample_at =
            Some(now + MICROPHONE_METER_INTERVAL);
        let (microphone_peak, speaker_peak) = take_peaks();
        let microphone_peak = if self.realtime_conversation.microphone_muted {
            0
        } else {
            microphone_peak
        };
        let microphone_level = audio_meter_level(microphone_peak);
        let speaker_level = audio_meter_level(speaker_peak);
        let microphone_intensity = audio_meter_intensity(microphone_peak);
        let speaker_intensity = audio_meter_intensity(speaker_peak);
        let previous_microphone_history = self.realtime_conversation.microphone_history;
        let previous_speaker_history = self.realtime_conversation.speaker_history;
        let mut changed = self.realtime_conversation.microphone_level != microphone_level
            || self.realtime_conversation.speaker_level != speaker_level
            || self.realtime_conversation.microphone_intensity != microphone_intensity
            || self.realtime_conversation.speaker_intensity != speaker_intensity;
        if speaker_level > 0 {
            self.realtime_conversation.speaker_active_until = Some(now + SPEAKER_ACTIVITY_HOLD);
        } else {
            changed |= self
                .realtime_conversation
                .speaker_active_until
                .take_if(|deadline| *deadline <= now)
                .is_some();
        }
        changed |= self
            .realtime_conversation
            .interruption_acknowledged_until
            .take_if(|deadline| *deadline <= now)
            .is_some();
        self.realtime_conversation.microphone_level = microphone_level;
        self.realtime_conversation.speaker_level = speaker_level;
        self.realtime_conversation.microphone_intensity = microphone_intensity;
        self.realtime_conversation.speaker_intensity = speaker_intensity;
        self.realtime_conversation
            .microphone_history
            .push(microphone_level);
        self.realtime_conversation
            .speaker_history
            .push(speaker_level);
        let had_meter_activity = self
            .realtime_conversation
            .audio_meter_history
            .iter()
            .any(|(microphone, speaker)| *microphone > 0 || *speaker > 0);
        if self.realtime_conversation.audio_meter_history.len() >= MAX_REALTIME_AUDIO_METER_FRAMES {
            self.realtime_conversation.audio_meter_history.pop_front();
        }
        self.realtime_conversation
            .audio_meter_history
            .push_back((microphone_intensity, speaker_intensity));
        changed |= previous_microphone_history != self.realtime_conversation.microphone_history
            || previous_speaker_history != self.realtime_conversation.speaker_history
            || had_meter_activity;
        if changed {
            self.update_realtime_footer();
        }
        self.frame_requester
            .schedule_frame_in(MICROPHONE_METER_INTERVAL);
    }

    pub(in crate::chatwidget) fn update_realtime_footer(&mut self) {
        let microphone_live = self.realtime_microphone_is_listening();
        let active = self.realtime_conversation.phase == RealtimeConversationPhase::Active;
        if !matches!(
            self.realtime_conversation.phase,
            RealtimeConversationPhase::Starting | RealtimeConversationPhase::Active
        ) || self.realtime_conversation.thread_id != self.thread_id()
        {
            self.bottom_pane.set_voice_strip(/*state*/ None);
            return;
        }

        let activity = if self.realtime_conversation.phase == RealtimeConversationPhase::Starting {
            "connecting"
        } else if self.realtime_conversation.microphone_muted {
            "muted"
        } else if self
            .realtime_conversation
            .interruption_acknowledged_until
            .is_some_and(|deadline| deadline > Instant::now())
        {
            "heard"
        } else if self.realtime_conversation.speaker_level > 0
            || self
                .realtime_conversation
                .speaker_active_until
                .is_some_and(|deadline| deadline > Instant::now())
        {
            "speaking"
        } else {
            "listening"
        };
        self.bottom_pane.set_voice_strip(Some(VoiceStripState {
            mute_hint: self.chat_keymap.voice_mute_hint(),
            phase: if active {
                VoiceStripPhase::Active
            } else {
                VoiceStripPhase::Connecting
            },
            microphone_live,
            microphone_muted: self.realtime_conversation.microphone_muted,
            microphone_history: self
                .realtime_conversation
                .audio_meter_history
                .iter()
                .map(|(microphone, _)| *microphone)
                .collect(),
            speaker_history: self
                .realtime_conversation
                .audio_meter_history
                .iter()
                .map(|(_, speaker)| *speaker)
                .collect(),
            activity,
            animations: self.local_settings.tui.animations,
        }));
    }
}

pub(super) fn audio_meter_intensity(peak: u16) -> u8 {
    let intensity = usize::from(peak.saturating_sub(AUDIO_METER_NOISE_FLOOR))
        .saturating_mul(usize::from(u8::MAX))
        .div_ceil(usize::from(
            AUDIO_METER_FULL_SCALE - AUDIO_METER_NOISE_FLOOR,
        ))
        .min(usize::from(u8::MAX));
    u8::try_from(intensity).unwrap_or(u8::MAX)
}

pub(super) fn audio_meter_level(peak: u16) -> usize {
    usize::from(peak.saturating_sub(AUDIO_METER_NOISE_FLOOR))
        .saturating_mul(AUDIO_METER_SEGMENTS)
        .div_ceil(usize::from(
            AUDIO_METER_FULL_SCALE - AUDIO_METER_NOISE_FLOOR,
        ))
        .min(AUDIO_METER_SEGMENTS)
}
