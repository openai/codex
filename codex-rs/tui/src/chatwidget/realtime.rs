//! TUI orchestration for an app-server-signaled, locally owned WebRTC voice session.
//! Completed captions and both speakers' partials stay bounded across widget replacement.
//! Interleaved speakers retain separate displays so settled caption text never reanimates.

mod recording_controls;
mod transcript_replay;

use super::ChatWidget;
use super::HistoryCell;
use super::PARENT_OWNED_INPUT_MESSAGE;
use super::realtime_split_flap::SplitFlapTranscriptCell;
use super::realtime_split_flap::VoiceAmplitudeHistory;
use crate::app_command::AppCommand;
use crate::app_event::AppEvent;
use crate::bottom_pane::VoiceStripPhase;
use crate::bottom_pane::VoiceStripState;
use crate::history_cell;
use crate::key_hint::KeyBindingListExt;
use crate::motion::MotionMode;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::models::MessagePhase;
use codex_realtime_webrtc::RealtimeWebrtcSession;
use codex_realtime_webrtc::RealtimeWebrtcSessionHandle;
use codex_realtime_webrtc::StartedRealtimeWebrtcSession;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use futures::future::AbortHandle;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

#[cfg(test)]
#[path = "realtime_tests.rs"]
pub(crate) mod tests;

pub(crate) const MAX_TRANSCRIPT_BYTES: usize = 1024;
const MAX_PENDING_TRANSCRIPT_CELLS: usize = 32;
pub(crate) const MAX_REPLAY_TRANSCRIPT_CELLS: usize = MAX_PENDING_TRANSCRIPT_CELLS + 2;
const MAX_PENDING_SPEECH_DELIVERIES: usize = 16;
const MAX_RETAINED_DELEGATED_REASONING_TURNS: usize = 16;
const MAX_PENDING_SPEECH_ITEM_BYTES: usize = 64 * 1024;
const MAX_PENDING_SPEECH_TURN_ID_BYTES: usize = 512;
// Leave room for Core's optional backend prefix within its 1,000-token speech limit.
const MAX_SPEAKABLE_FINAL_TOKENS: usize = 990;
const AUDIO_METER_SEGMENTS: usize = 5;
const AUDIO_METER_NOISE_FLOOR: u16 = 512;
const AUDIO_METER_FULL_SCALE: u16 = 8192;
const MAX_REALTIME_AUDIO_METER_FRAMES: usize = 12;
const MICROPHONE_METER_INTERVAL: Duration = Duration::from_millis(100);
const SPEAKER_ACTIVITY_HOLD: Duration = Duration::from_millis(500);
const INTERRUPTION_ACKNOWLEDGMENT: Duration = Duration::from_millis(400);
static NEXT_REALTIME_ATTEMPT_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_REALTIME_SPEECH_DELIVERY_ID: AtomicU64 = AtomicU64::new(1);

struct PendingRealtimeSpeech {
    state: PendingSpeechState,
    captioned: bool,
    input_generation: u64,
    thread_id: ThreadId,
    turn_id: String,
    item: ThreadItem,
}

pub(crate) struct RealtimeTranscriptRecord {
    pub(crate) role: String,
    pub(crate) text: String,
    pub(crate) complete: bool,
    pub(crate) before_turn_id: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingSpeechState {
    AwaitingTurn,
    Queued(u64),
    Accepted,
}

fn can_retain_realtime_speech(turn_id: &str, item: &ThreadItem) -> bool {
    turn_id.len() <= MAX_PENDING_SPEECH_TURN_ID_BYTES
        && serde_json::to_vec(item)
            .is_ok_and(|encoded| encoded.len() <= MAX_PENDING_SPEECH_ITEM_BYTES)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum RealtimeConversationPhase {
    #[default]
    Inactive,
    Starting,
    Active,
    Stopping,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RealtimeAgentItemOrigin {
    Typed,
    Delegated {
        may_speak: bool,
        completed: bool,
        suppressed_nonfinal: bool,
        input_generation: u64,
    },
}

#[derive(Clone, Copy, Debug)]
enum RealtimeTurnOrigin {
    Typed {
        input_generation: u64,
    },
    Delegated {
        may_speak: bool,
        input_generation: u64,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StartupRetry {
    #[default]
    Available,
    WaitingForStop,
    Used,
}

#[derive(Default)]
pub(super) struct RealtimeConversationUiState {
    startup_retry: StartupRetry,
    pub(super) phase: RealtimeConversationPhase,
    recover_late_transcripts: bool,
    attempt_id: u64,
    thread_id: Option<ThreadId>,
    pub(super) handle: Option<RealtimeWebrtcSessionHandle>,
    startup_abort: Option<AbortHandle>,
    backend_started: bool,
    webrtc_connected: bool,
    active_since: Option<Instant>,
    failure_recorded: bool,
    microphone_muted: bool,
    microphone_level: usize,
    speaker_level: usize,
    microphone_intensity: u8,
    speaker_intensity: u8,
    microphone_history: VoiceAmplitudeHistory,
    speaker_history: VoiceAmplitudeHistory,
    audio_meter_history: VecDeque<(u8, u8)>,
    next_audio_meter_sample_at: Option<Instant>,
    speaker_active_until: Option<Instant>,
    interruption_acknowledged_until: Option<Instant>,
    speaker_suppression_generation: Option<u64>,
    transcript_role: Option<String>,
    transcript: String,
    // The other speaker's bounded partial while duplex deltas interleave.
    interleaved_transcript: Option<(String, String)>,
    interleaved_transcript_cell: Option<Box<dyn HistoryCell>>,
    transcript_input_generation: Option<u64>,
    assistant_transcript_generation: Option<u64>,
    assistant_caption_started_after_speech_queue: bool,
    pub(super) live_transcript_cell: Option<Box<dyn HistoryCell>>,
    pub(super) pending_history_cells: VecDeque<Box<dyn HistoryCell>>,
    accepted_transcripts: VecDeque<RealtimeTranscriptRecord>,
    replay_transcripts: Option<VecDeque<RealtimeTranscriptRecord>>,
    latest_input_was_voice: bool,
    input_generation: u64,
    latest_voice_input_fingerprint: Option<(usize, u64)>,
    pending_typed_input: Option<String>,
    turn_origins: HashMap<String, RealtimeTurnOrigin>,
    delegated_reasoning_turns: VecDeque<String>,
    pub(super) agent_items: HashMap<(String, String), RealtimeAgentItemOrigin>,
    pending_speech: VecDeque<PendingRealtimeSpeech>,
}

impl RealtimeConversationUiState {
    pub(super) fn live_transcript_cells(&self) -> impl Iterator<Item = &Box<dyn HistoryCell>> {
        // Keep the user above the reply even when their last packets interleave.
        let cells = if self.transcript_role.as_deref() == Some("user") {
            [
                &self.live_transcript_cell,
                &self.interleaved_transcript_cell,
            ]
        } else {
            [
                &self.interleaved_transcript_cell,
                &self.live_transcript_cell,
            ]
        };
        cells.into_iter().filter_map(Option::as_ref)
    }
}

pub(crate) fn realtime_delegation_input(items: &[UserInput]) -> Option<&str> {
    let [
        UserInput::Text {
            text,
            text_elements,
        },
    ] = items
    else {
        return None;
    };
    if !text_elements.is_empty() {
        return None;
    }
    let body = text
        .trim()
        .strip_prefix("<realtime_delegation>")
        .and_then(|body| body.strip_suffix("</realtime_delegation>"))?;
    let (_, input) = body.split_once("<input>")?;
    input.split_once("</input>").map(|(input, _)| input)
}

pub(crate) fn is_private_realtime_agent_item(item: &ThreadItem) -> bool {
    if matches!(item, ThreadItem::Reasoning { .. }) {
        return true;
    }
    let ThreadItem::AgentMessage { text, phase, .. } = item else {
        return false;
    };
    let text = text.trim_start();
    matches!(phase, Some(MessagePhase::Commentary))
        || (!matches!(phase, Some(MessagePhase::FinalAnswer))
            && (text.starts_with("[ANALYSIS]") || text.starts_with("[COMMENTARY]")))
}

pub(crate) fn realtime_delegation_display_text(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn realtime_input_fingerprint(input: &str) -> (usize, u64) {
    let input = input.trim();
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    (input.len(), hasher.finish())
}

impl ChatWidget {
    pub(crate) fn realtime_conversation_is_running(&self) -> bool {
        self.realtime_conversation.phase != RealtimeConversationPhase::Inactive
    }

    pub(crate) fn may_receive_realtime_transcripts(&self) -> bool {
        self.realtime_conversation.phase != RealtimeConversationPhase::Inactive
            || self.realtime_conversation.recover_late_transcripts
    }

    pub(crate) fn toggle_realtime_conversation(&mut self) {
        if self.realtime_conversation.phase == RealtimeConversationPhase::Stopping {
            self.realtime_conversation.startup_retry = StartupRetry::Used;
            self.add_info_message(
                "Voice conversation is still stopping.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        if self.realtime_conversation.phase != RealtimeConversationPhase::Inactive {
            self.stop_realtime_conversation();
            return;
        }

        if self.has_misalignment_policy_violation() {
            self.show_misalignment_policy_precaution();
            return;
        }

        if self.side_conversation_active() {
            self.add_error_message(
                "Voice mode is unavailable in side conversations. Return to the main thread first."
                    .to_string(),
            );
            return;
        }

        if self.blocks_direct_input {
            self.add_error_message(PARENT_OWNED_INPUT_MESSAGE.to_string());
            return;
        }

        if !self.config.features.enabled(Feature::RealtimeConversation) {
            self.add_error_message("Voice conversations are not enabled.".to_string());
            return;
        }

        if !RealtimeWebrtcSession::is_supported() {
            self.add_error_message(
                "Voice requires macOS, an MSVC-based Windows build, or a glibc-based Linux build."
                    .to_string(),
            );
            return;
        }

        let Some(thread_id) = self.thread_id() else {
            self.add_error_message("Start a conversation before using voice mode.".to_string());
            return;
        };

        self.session_telemetry
            .counter("codex.voice.session.start", /*inc*/ 1, &[]);
        self.start_realtime_conversation(thread_id);
    }

    fn start_realtime_conversation(&mut self, thread_id: ThreadId) {
        self.realtime_conversation.recover_late_transcripts = false;
        self.realtime_conversation.attempt_id =
            NEXT_REALTIME_ATTEMPT_ID.fetch_add(1, Ordering::Relaxed);
        self.realtime_conversation.thread_id = Some(thread_id);
        let attempt_id = self.realtime_conversation.attempt_id;
        let (startup_abort, abort_registration) = AbortHandle::new_pair();
        self.realtime_conversation.startup_abort = Some(startup_abort);
        self.realtime_conversation.phase = RealtimeConversationPhase::Starting;
        self.update_realtime_footer();
        let app_event_tx = self.app_event_tx.clone();
        std::thread::spawn(move || {
            let result =
                RealtimeWebrtcSession::start(abort_registration).map_err(|error| error.to_string());
            app_event_tx.send(AppEvent::RealtimeWebrtcOfferCreated {
                thread_id,
                attempt_id,
                result,
            });
        });
        self.request_redraw();
    }

    pub(super) fn stop_realtime_conversation(&mut self) {
        self.realtime_conversation.startup_retry = StartupRetry::Used;
        if matches!(
            self.realtime_conversation.phase,
            RealtimeConversationPhase::Inactive | RealtimeConversationPhase::Stopping
        ) {
            return;
        }
        self.finish_realtime_session_metrics();
        self.restore_all_undelivered_realtime_speech();

        // Stopping may prevent final transcript events from arriving.
        self.finish_realtime_partial_transcripts();

        if let Some(handle) = self.realtime_conversation.handle.take() {
            handle.close();
            self.realtime_conversation.phase = RealtimeConversationPhase::Stopping;
            self.refresh_terminal_title();
            self.bottom_pane.set_voice_strip(/*state*/ None);
            let Some(thread_id) = self.realtime_conversation.thread_id else {
                self.reset_realtime_conversation();
                return;
            };
            if !self.submit_op(AppCommand::RealtimeConversationStop { thread_id }) {
                self.record_realtime_failure();
                self.reset_realtime_conversation();
            }
        } else {
            if let Some(thread_id) = self.reset_realtime_conversation() {
                self.submit_op(AppCommand::RealtimeConversationStop { thread_id });
            }
        }
        self.request_redraw();
    }

    pub(crate) fn on_realtime_webrtc_offer_created(
        &mut self,
        thread_id: ThreadId,
        attempt_id: u64,
        result: Result<StartedRealtimeWebrtcSession, String>,
    ) {
        if self.realtime_conversation.phase != RealtimeConversationPhase::Starting
            || self.realtime_conversation.attempt_id != attempt_id
        {
            if let Ok(offer) = result {
                offer.handle.close();
            }
            return;
        }

        let offer = match result {
            Ok(offer) => offer,
            Err(error) => {
                self.on_realtime_error(format!("Failed to start voice mode: {error}"));
                return;
            }
        };
        self.realtime_conversation.startup_abort = None;
        if self.realtime_conversation.microphone_muted
            && let Err(error) = offer.handle.set_microphone_muted(/*muted*/ true)
        {
            offer.handle.close();
            self.on_realtime_error(format!("Failed to restore microphone mute: {error}"));
            return;
        }
        self.realtime_conversation.handle = Some(offer.handle);
        self.update_realtime_footer();
        self.refresh_terminal_title();
        self.frame_requester
            .schedule_frame_in(MICROPHONE_METER_INTERVAL);
        if !self.submit_op(AppCommand::RealtimeConversationStart {
            thread_id,
            offer_sdp: offer.offer_sdp.into(),
        }) {
            self.record_realtime_failure();
            self.reset_realtime_conversation();
        }
    }

    pub(super) fn on_realtime_conversation_sdp(&mut self, answer_sdp: String) {
        if self.realtime_conversation.phase != RealtimeConversationPhase::Starting {
            return;
        }
        let Some(handle) = self.realtime_conversation.handle.clone() else {
            return;
        };
        let Some(thread_id) = self.thread_id() else {
            return;
        };
        let attempt_id = self.realtime_conversation.attempt_id;
        let app_event_tx = self.app_event_tx.clone();
        std::thread::spawn(move || {
            let result = handle.apply_answer_sdp(answer_sdp);
            app_event_tx.send(AppEvent::RealtimeWebrtcConnected {
                thread_id,
                attempt_id,
                result,
            });
        });
    }

    pub(super) fn on_realtime_conversation_started(&mut self) {
        if self.realtime_conversation.phase != RealtimeConversationPhase::Starting {
            return;
        }
        self.realtime_conversation.backend_started = true;
        self.maybe_activate_realtime_conversation();
    }

    pub(crate) fn on_realtime_webrtc_connected(
        &mut self,
        attempt_id: u64,
        result: Result<(), codex_realtime_webrtc::ConnectionError>,
    ) {
        if self.realtime_conversation.phase != RealtimeConversationPhase::Starting
            || self.realtime_conversation.attempt_id != attempt_id
        {
            return;
        }
        if let Err(error) = result {
            if error == codex_realtime_webrtc::ConnectionError::NegotiationTimedOut
                && self.realtime_conversation.startup_retry == StartupRetry::Available
                && let Some(thread_id) = self.realtime_conversation.thread_id
            {
                if let Some(handle) = self.realtime_conversation.handle.take() {
                    handle.close();
                }
                self.realtime_conversation.phase = RealtimeConversationPhase::Stopping;
                self.realtime_conversation.startup_retry = StartupRetry::WaitingForStop;
                self.refresh_terminal_title();
                if !self.submit_op(AppCommand::RealtimeConversationStop { thread_id }) {
                    self.record_realtime_failure();
                    self.reset_realtime_conversation();
                } else {
                    self.add_info_message(
                        "Voice connection timed out. Retrying once after cleanup.".into(),
                        /*hint*/ None,
                    );
                }
                return;
            }
            self.on_realtime_error(format!("Failed to connect voice mode: {error}"));
            return;
        }

        self.realtime_conversation.webrtc_connected = true;
        self.maybe_activate_realtime_conversation();
    }

    fn maybe_activate_realtime_conversation(&mut self) {
        if self.realtime_conversation.phase != RealtimeConversationPhase::Starting
            || !self.realtime_conversation.backend_started
            || !self.realtime_conversation.webrtc_connected
        {
            return;
        }
        self.realtime_conversation.phase = RealtimeConversationPhase::Active;
        self.realtime_conversation.active_since = Some(Instant::now());
        self.session_telemetry
            .counter("codex.voice.session.connected", /*inc*/ 1, &[]);
        let running_delegation =
            self.turn_lifecycle
                .last_turn_id
                .as_deref()
                .is_some_and(|turn_id| {
                    matches!(
                        self.realtime_conversation.turn_origins.get(turn_id),
                        Some(RealtimeTurnOrigin::Delegated {
                            may_speak: true,
                            ..
                        })
                    )
                });
        self.realtime_conversation.latest_input_was_voice =
            !self.turn_lifecycle.agent_turn_running || running_delegation;
        if self.turn_lifecycle.agent_turn_running
            && let Some(turn_id) = self.turn_lifecycle.last_turn_id.clone()
        {
            self.realtime_conversation
                .turn_origins
                .entry(turn_id)
                .or_insert(RealtimeTurnOrigin::Typed {
                    input_generation: self.realtime_conversation.input_generation,
                });
        }
        self.update_realtime_footer();
        self.frame_requester
            .schedule_frame_in(MICROPHONE_METER_INTERVAL);
        self.add_info_message(
            "Voice conversation started.".to_string(),
            Some("Use /voice mute to mute or /voice to stop.".to_string()),
        );
    }

    pub(crate) fn is_current_realtime_attempt(
        &self,
        thread_id: ThreadId,
        attempt_id: u64,
        input_generation: u64,
    ) -> bool {
        self.realtime_conversation.phase == RealtimeConversationPhase::Active
            && self.realtime_conversation.thread_id == Some(thread_id)
            && self.realtime_conversation.attempt_id == attempt_id
            && self.realtime_conversation.input_generation == input_generation
            && self.realtime_conversation.latest_input_was_voice
    }

    pub(super) fn note_realtime_typed_input(&mut self, text: &str) {
        self.invalidate_realtime_voice_input();
        if self.realtime_conversation.phase == RealtimeConversationPhase::Active {
            self.realtime_conversation.pending_typed_input = Some(text.to_string());
        }
    }

    fn invalidate_realtime_voice_input(&mut self) {
        if self.realtime_conversation.phase != RealtimeConversationPhase::Active {
            return;
        }
        self.realtime_conversation.latest_input_was_voice = false;
        self.realtime_conversation.input_generation = self
            .realtime_conversation
            .input_generation
            .wrapping_add(/*rhs*/ 1);
        // Typing supersedes the old voice answer. Suppress the helper before
        // another queued audio frame can play, even when the mic is muted.
        self.realtime_conversation.speaker_suppression_generation =
            Some(self.realtime_conversation.input_generation);
        if let Some(handle) = self.realtime_conversation.handle.as_ref() {
            handle.set_speaker_suppressed(/*suppressed*/ true);
        }
        self.realtime_conversation.speaker_level = 0;
        self.realtime_conversation.speaker_active_until = None;
        self.update_realtime_footer();
        for origin in self.realtime_conversation.turn_origins.values_mut() {
            if let RealtimeTurnOrigin::Delegated { may_speak, .. } = origin {
                *may_speak = false;
            }
        }
        for origin in self.realtime_conversation.agent_items.values_mut() {
            if let RealtimeAgentItemOrigin::Delegated { may_speak, .. } = origin {
                *may_speak = false;
            }
        }
    }

    pub(super) fn note_realtime_user_item_started(&mut self, turn_id: &str, items: &[UserInput]) {
        if !matches!(
            self.realtime_conversation.phase,
            RealtimeConversationPhase::Starting | RealtimeConversationPhase::Active
        ) {
            if realtime_delegation_input(items).is_some() {
                self.remember_realtime_delegated_reasoning_turn(turn_id);
            }
            return;
        }
        let matches_local_typed_input = self
            .realtime_conversation
            .pending_typed_input
            .as_ref()
            .is_some_and(|pending| {
                items
                    .iter()
                    .any(|item| matches!(item, UserInput::Text { text, .. } if text == pending))
            });
        if matches_local_typed_input {
            self.realtime_conversation.pending_typed_input = None;
        }
        if let Some(input) = realtime_delegation_input(items)
            && !matches_local_typed_input
        {
            let transcript_tail_flush = items.iter().any(|item| {
                matches!(item, UserInput::Text { text, .. }
                    if text.contains("<source>transcript_tail_flush</source>"))
            });
            let stale_voice_input = !self.realtime_conversation.latest_input_was_voice
                && self
                    .realtime_conversation
                    .latest_voice_input_fingerprint
                    .is_some_and(|latest| {
                        latest
                            == realtime_input_fingerprint(&realtime_delegation_display_text(input))
                    });
            let (typed_turn_generation, delegated_turn_generation) =
                match self.realtime_conversation.turn_origins.get(turn_id) {
                    Some(RealtimeTurnOrigin::Typed { input_generation }) => {
                        (Some(*input_generation), None)
                    }
                    Some(RealtimeTurnOrigin::Delegated {
                        input_generation, ..
                    }) => (None, Some(*input_generation)),
                    None => (None, None),
                };
            let voice_supersedes_typed_turn = typed_turn_generation.is_some_and(|generation| {
                generation != self.realtime_conversation.input_generation
                    || !self.realtime_conversation.latest_input_was_voice
            }) && !transcript_tail_flush
                && !stale_voice_input;
            let typed_turn = typed_turn_generation.is_some() && !voice_supersedes_typed_turn;
            let may_speak = !transcript_tail_flush && !stale_voice_input && !typed_turn;
            if may_speak {
                let replaces_current_delegation = self
                    .realtime_conversation
                    .turn_origins
                    .values()
                    .any(|origin| {
                        matches!(
                            origin,
                            RealtimeTurnOrigin::Delegated {
                                may_speak: true,
                                input_generation,
                            } if *input_generation == self.realtime_conversation.input_generation
                        )
                    });
                if !self.realtime_conversation.latest_input_was_voice || replaces_current_delegation
                {
                    self.realtime_conversation.input_generation = self
                        .realtime_conversation
                        .input_generation
                        .wrapping_add(/*rhs*/ 1);
                    if self
                        .realtime_conversation
                        .speaker_suppression_generation
                        .is_some()
                    {
                        self.realtime_conversation.speaker_suppression_generation =
                            Some(self.realtime_conversation.input_generation);
                    }
                }
                self.realtime_conversation.latest_input_was_voice = true;
            }
            let delegated_origin = RealtimeTurnOrigin::Delegated {
                may_speak,
                input_generation: self.realtime_conversation.input_generation,
            };
            let supersedes_previous_delegation = may_speak
                && delegated_turn_generation.is_some_and(|generation| {
                    generation != self.realtime_conversation.input_generation
                });
            if voice_supersedes_typed_turn || supersedes_previous_delegation {
                self.realtime_conversation
                    .turn_origins
                    .insert(turn_id.to_string(), delegated_origin);
            } else {
                self.realtime_conversation
                    .turn_origins
                    .entry(turn_id.to_string())
                    .or_insert(delegated_origin);
            }
            if matches!(
                self.realtime_conversation.turn_origins.get(turn_id),
                Some(RealtimeTurnOrigin::Delegated { .. })
            ) {
                self.remember_realtime_delegated_reasoning_turn(turn_id);
            }
            return;
        }

        self.invalidate_realtime_voice_input();
        self.realtime_conversation.turn_origins.insert(
            turn_id.to_string(),
            RealtimeTurnOrigin::Typed {
                input_generation: self.realtime_conversation.input_generation,
            },
        );
        self.realtime_conversation
            .delegated_reasoning_turns
            .retain(|saved| saved != turn_id);
    }

    pub(crate) fn remember_realtime_delegated_reasoning_turn(&mut self, turn_id: &str) {
        if turn_id.len() > 512
            || self
                .realtime_conversation
                .delegated_reasoning_turns
                .iter()
                .any(|saved| saved == turn_id)
        {
            return;
        }
        self.realtime_conversation
            .delegated_reasoning_turns
            .push_back(turn_id.to_string());
        if self.realtime_conversation.delegated_reasoning_turns.len()
            > MAX_RETAINED_DELEGATED_REASONING_TURNS
        {
            self.realtime_conversation
                .delegated_reasoning_turns
                .pop_front();
        }
    }

    pub(super) fn is_realtime_delegated_reasoning_turn(&self, turn_id: &str) -> bool {
        self.realtime_conversation
            .delegated_reasoning_turns
            .iter()
            .any(|saved| saved == turn_id)
    }

    pub(super) fn is_realtime_delegated_reasoning_item(
        &self,
        turn_id: &str,
        item_id: &str,
    ) -> bool {
        self.is_realtime_delegated_reasoning_turn(turn_id)
            && !matches!(
                self.realtime_conversation
                    .agent_items
                    .get(&(turn_id.to_string(), item_id.to_string())),
                Some(RealtimeAgentItemOrigin::Typed)
            )
    }

    pub(super) fn should_hide_realtime_delegation(&self, turn_id: &str) -> bool {
        matches!(
            self.realtime_conversation.phase,
            RealtimeConversationPhase::Starting | RealtimeConversationPhase::Active
        ) && matches!(
            self.realtime_conversation.turn_origins.get(turn_id),
            Some(RealtimeTurnOrigin::Delegated { .. })
        )
    }

    pub(super) fn is_realtime_delegated_agent_item(
        &mut self,
        turn_id: &str,
        item_id: &str,
    ) -> bool {
        if !matches!(
            self.realtime_conversation.phase,
            RealtimeConversationPhase::Starting | RealtimeConversationPhase::Active
        ) {
            return false;
        }
        let default_origin = match self.realtime_conversation.turn_origins.get(turn_id) {
            Some(RealtimeTurnOrigin::Delegated {
                may_speak,
                input_generation,
            }) => RealtimeAgentItemOrigin::Delegated {
                may_speak: *may_speak,
                completed: false,
                suppressed_nonfinal: false,
                input_generation: *input_generation,
            },
            Some(RealtimeTurnOrigin::Typed { .. }) | None => RealtimeAgentItemOrigin::Typed,
        };
        matches!(
            self.realtime_conversation
                .agent_items
                .entry((turn_id.to_string(), item_id.to_string()))
                .or_insert(default_origin),
            RealtimeAgentItemOrigin::Delegated { .. }
        )
    }

    pub(super) fn complete_realtime_delegated_agent_item(
        &mut self,
        turn_id: &str,
        item: &ThreadItem,
        from_replay: bool,
    ) -> bool {
        let ThreadItem::AgentMessage {
            id: item_id,
            text,
            phase,
            questions,
            ..
        } = item
        else {
            return false;
        };
        // The ordinary completion path opens the structured question editor.
        // A question-bearing answer must remain visible instead of being speech-only.
        if questions.is_some() {
            return false;
        }
        if from_replay || !self.is_realtime_delegated_agent_item(turn_id, item_id) {
            return false;
        }
        let Some(RealtimeAgentItemOrigin::Delegated {
            may_speak,
            completed,
            suppressed_nonfinal,
            ..
        }) = self
            .realtime_conversation
            .agent_items
            .get_mut(&(turn_id.to_string(), item_id.to_string()))
        else {
            return false;
        };
        if *completed {
            return true;
        }
        // The delegated agent's commentary and reasoning are private to the
        // voice handoff. Never put them in the fallback queue, even when the
        // item is too large to retain.
        let trimmed = text.trim();
        let explicitly_final = matches!(phase, Some(MessagePhase::FinalAnswer));
        if is_private_realtime_agent_item(item)
            || (!explicitly_final && trimmed == "[FINAL]")
            || trimmed.is_empty()
        {
            *completed = true;
            *suppressed_nonfinal = true;
            return true;
        }
        // Typing ends the old item's speech ownership, but it does not make
        // private voice commentary into typed output. Only a visible answer
        // returns to the ordinary completion path.
        if !*may_speak {
            return false;
        }
        // A final item that cannot be retained for delivery recovery stays in
        // ordinary history instead of becoming a voice-only answer.
        if !can_retain_realtime_speech(turn_id, item) {
            return false;
        }
        let Some(thread_id) = self.realtime_conversation.thread_id else {
            return false;
        };
        let input_generation = self.realtime_conversation.input_generation;
        *completed = true;
        self.transcript.last_completed_agent_message =
            Some((turn_id.to_string(), item_id.to_string()));
        if self.realtime_conversation.pending_speech.len() >= MAX_PENDING_SPEECH_DELIVERIES
            && let Some(oldest) = self.realtime_conversation.pending_speech.pop_front()
        {
            self.restore_realtime_speech(oldest);
        }
        self.realtime_conversation
            .pending_speech
            .push_back(PendingRealtimeSpeech {
                state: PendingSpeechState::AwaitingTurn,
                captioned: false,
                input_generation,
                thread_id,
                turn_id: turn_id.to_string(),
                item: item.clone(),
            });

        true
    }

    pub(super) fn speak_completed_realtime_delegation(&mut self, turn_id: &str, item: &ThreadItem) {
        let ThreadItem::AgentMessage {
            id: item_id, text, ..
        } = item
        else {
            return;
        };
        let Some(RealtimeAgentItemOrigin::Delegated {
            may_speak: true,
            completed: true,
            suppressed_nonfinal,
            input_generation,
        }) = self
            .realtime_conversation
            .agent_items
            .get(&(turn_id.to_string(), item_id.to_string()))
        else {
            return;
        };
        let input_generation = *input_generation;
        let suppressed_nonfinal = *suppressed_nonfinal;
        if input_generation != self.realtime_conversation.input_generation {
            return;
        }
        let trimmed = text.trim();
        if is_private_realtime_agent_item(item)
            || trimmed.is_empty()
            || !self.realtime_conversation.latest_input_was_voice
        {
            return;
        }
        let Some(thread_id) = self.realtime_conversation.thread_id else {
            return;
        };
        if self.thread_id() != Some(thread_id) {
            return;
        }
        let text = trimmed
            .strip_prefix("[FINAL]")
            .map(str::trim_start)
            .unwrap_or(trimmed)
            .to_string();
        if text.is_empty() {
            return;
        }
        let Some(RealtimeTurnOrigin::Delegated {
            may_speak,
            input_generation: turn_input_generation,
        }) = self.realtime_conversation.turn_origins.get_mut(turn_id)
        else {
            return;
        };
        if !*may_speak || *turn_input_generation != input_generation {
            return;
        }
        *may_speak = false;
        if !can_retain_realtime_speech(turn_id, item)
            || codex_utils_string::approx_token_count(&text) > MAX_SPEAKABLE_FINAL_TOKENS
        {
            self.remove_waiting_realtime_speech(turn_id, item_id);
            self.finish_realtime_turn(turn_id);
            self.handle_thread_item(
                item.clone(),
                turn_id.to_string(),
                super::ThreadItemRenderSource::Live,
            );
            return;
        }
        let delivery_id = NEXT_REALTIME_SPEECH_DELIVERY_ID.fetch_add(1, Ordering::Relaxed);
        let index = self
            .realtime_conversation
            .pending_speech
            .iter_mut()
            .position(|pending| {
                pending.turn_id == turn_id
                    && matches!(&pending.item, ThreadItem::AgentMessage { id, .. } if id == item_id)
                    && pending.state == PendingSpeechState::AwaitingTurn
            });
        let index = match index {
            Some(index) => index,
            None if suppressed_nonfinal => {
                if self.realtime_conversation.pending_speech.len() >= MAX_PENDING_SPEECH_DELIVERIES
                    && let Some(oldest) = self.realtime_conversation.pending_speech.pop_front()
                {
                    self.restore_realtime_speech(oldest);
                }
                self.realtime_conversation
                    .pending_speech
                    .push_back(PendingRealtimeSpeech {
                        state: PendingSpeechState::AwaitingTurn,
                        captioned: false,
                        input_generation,
                        thread_id,
                        turn_id: turn_id.to_string(),
                        item: item.clone(),
                    });
                self.realtime_conversation.pending_speech.len() - 1
            }
            None => return,
        };
        let pending = &mut self.realtime_conversation.pending_speech[index];
        pending.item = item.clone();
        pending.state = PendingSpeechState::Queued(delivery_id);
        if !self.submit_op(AppCommand::RealtimeConversationSpeech {
            thread_id,
            attempt_id: self.realtime_conversation.attempt_id,
            input_generation,
            delivery_id,
            text: text.into(),
        }) {
            self.restore_undelivered_realtime_speech(delivery_id);
            self.on_realtime_error("Failed to deliver the voice response.".to_string());
        } else {
            self.realtime_conversation.pending_speech.retain(|pending| {
                pending.turn_id != turn_id || pending.state != PendingSpeechState::AwaitingTurn
            });
        }
    }

    pub(crate) fn has_pending_realtime_speech(&self, delivery_id: u64) -> bool {
        self.realtime_conversation
            .pending_speech
            .iter()
            .any(|delivery| delivery.state == PendingSpeechState::Queued(delivery_id))
    }

    pub(crate) fn take_undelivered_realtime_speech_for_replay(
        &mut self,
    ) -> Vec<(ThreadId, String, ThreadItem)> {
        self.realtime_conversation
            .pending_speech
            .drain(..)
            .filter(|delivery| !delivery.captioned)
            .map(|delivery| (delivery.thread_id, delivery.turn_id, delivery.item))
            .collect()
    }

    pub(crate) fn take_realtime_transcript_cells_for_replay(
        &mut self,
    ) -> VecDeque<RealtimeTranscriptRecord> {
        let mut records = std::mem::take(&mut self.realtime_conversation.accepted_transcripts);
        // Accepted records include cells already flushed into the old widget as
        // well as those still deferred behind another assistant stream.
        self.realtime_conversation.pending_history_cells.clear();
        let mut retain_partial = |role: String, text: String| {
            if text.trim().is_empty() {
                return;
            }
            if let Some(partial) = records
                .iter_mut()
                .rev()
                .find(|record| record.role == role && !record.complete)
            {
                partial.text = text;
                return;
            }
            if records.len() >= MAX_REPLAY_TRANSCRIPT_CELLS {
                records.pop_front();
            }
            records.push_back(RealtimeTranscriptRecord {
                role,
                text,
                complete: false,
                before_turn_id: None,
            });
        };
        if let Some((role, text)) = self.realtime_conversation.interleaved_transcript.take() {
            self.realtime_conversation.interleaved_transcript_cell = None;
            retain_partial(role, text);
        }
        if let Some(role) = self.realtime_conversation.transcript_role.take() {
            let text = std::mem::take(&mut self.realtime_conversation.transcript);
            retain_partial(role, text);
            self.realtime_conversation.live_transcript_cell = None;
        }
        records
    }

    pub(crate) fn restore_realtime_transcript_cells(
        &mut self,
        records: VecDeque<RealtimeTranscriptRecord>,
    ) {
        self.realtime_conversation.recover_late_transcripts = true;
        for record in records {
            if self.realtime_conversation.accepted_transcripts.len() >= MAX_REPLAY_TRANSCRIPT_CELLS
            {
                self.realtime_conversation.accepted_transcripts.pop_front();
            }
            if record.complete {
                if self.realtime_conversation.pending_history_cells.len()
                    >= MAX_REPLAY_TRANSCRIPT_CELLS
                {
                    self.realtime_conversation.pending_history_cells.pop_front();
                }
                let cell = self.realtime_transcript_history_cell(&record.role, &record.text);
                self.realtime_conversation
                    .pending_history_cells
                    .push_back(cell);
                self.bump_active_cell_revision();
            } else {
                // Keep an unfinished caption editable until its late completion or close.
                self.on_realtime_transcript_delta(record.role.clone(), record.text.clone());
            }
            self.realtime_conversation
                .accepted_transcripts
                .push_back(record);
        }
        self.flush_realtime_transcript_history();
    }

    pub(crate) fn render_undelivered_realtime_speech(
        &mut self,
        thread_id: ThreadId,
        turn_id: String,
        item: ThreadItem,
    ) {
        if self.thread_id() == Some(thread_id) {
            self.handle_thread_item(item, turn_id, super::ThreadItemRenderSource::Live);
        }
    }

    pub(crate) fn accept_realtime_speech(&mut self, delivery_id: u64) {
        if let Some(delivery) = self
            .realtime_conversation
            .pending_speech
            .iter_mut()
            .find(|delivery| delivery.state == PendingSpeechState::Queued(delivery_id))
        {
            delivery.state = PendingSpeechState::Accepted;
        }
        self.realtime_conversation
            .pending_speech
            .retain(|delivery| !delivery.captioned);
    }

    pub(crate) fn restore_undelivered_realtime_speech(&mut self, delivery_id: u64) {
        if let Some(index) = self
            .realtime_conversation
            .pending_speech
            .iter()
            .position(|delivery| delivery.state == PendingSpeechState::Queued(delivery_id))
            && let Some(delivery) = self.realtime_conversation.pending_speech.remove(index)
        {
            self.restore_realtime_speech(delivery);
        }
    }

    fn restore_realtime_speech(&mut self, delivery: PendingRealtimeSpeech) {
        if delivery.captioned {
            return;
        }
        if self.thread_id() != Some(delivery.thread_id) {
            return;
        }
        self.forget_realtime_turn_origin(&delivery.turn_id);
        self.handle_thread_item(
            delivery.item,
            delivery.turn_id,
            super::ThreadItemRenderSource::Live,
        );
    }

    fn restore_all_undelivered_realtime_speech(&mut self) {
        while let Some(delivery) = self.realtime_conversation.pending_speech.pop_front() {
            self.restore_realtime_speech(delivery);
        }
    }

    pub(super) fn finish_realtime_turn(&mut self, turn_id: &str) {
        let mut waiting = Vec::new();
        let mut index = 0;
        while index < self.realtime_conversation.pending_speech.len() {
            if self.realtime_conversation.pending_speech[index].turn_id == turn_id
                && self.realtime_conversation.pending_speech[index].state
                    == PendingSpeechState::AwaitingTurn
            {
                if let Some(delivery) = self.realtime_conversation.pending_speech.remove(index) {
                    waiting.push(delivery);
                }
            } else {
                index += 1;
            }
        }
        self.forget_realtime_turn_origin(turn_id);
        for delivery in waiting {
            if self.thread_id() == Some(delivery.thread_id) {
                self.handle_thread_item(
                    delivery.item,
                    delivery.turn_id,
                    super::ThreadItemRenderSource::Live,
                );
            }
        }
    }

    fn remove_waiting_realtime_speech(&mut self, turn_id: &str, item_id: &str) {
        self.realtime_conversation.pending_speech.retain(|pending| {
            pending.turn_id != turn_id
                || pending.state != PendingSpeechState::AwaitingTurn
                || !matches!(&pending.item, ThreadItem::AgentMessage { id, .. } if id == item_id)
        });
    }

    fn forget_realtime_turn_origin(&mut self, turn_id: &str) {
        self.realtime_conversation.turn_origins.remove(turn_id);
        self.realtime_conversation
            .delegated_reasoning_turns
            .retain(|saved| saved != turn_id);
        self.realtime_conversation
            .agent_items
            .retain(|(item_turn_id, _), _| item_turn_id != turn_id);
    }

    pub(super) fn on_realtime_transcript_delta(&mut self, role: String, delta: String) {
        if self.realtime_conversation.phase == RealtimeConversationPhase::Inactive
            && !self.realtime_conversation.recover_late_transcripts
        {
            return;
        }
        let active = matches!(
            self.realtime_conversation.phase,
            RealtimeConversationPhase::Starting | RealtimeConversationPhase::Active
        );
        // Duplex transcript chunks can interleave. Display role changes are not input boundaries.
        if active
            && role == "user"
            && self
                .realtime_conversation
                .transcript_input_generation
                .is_none()
        {
            let interrupted = self.realtime_conversation.phase == RealtimeConversationPhase::Active
                && !self.realtime_conversation.microphone_muted
                && !delta.trim().is_empty()
                && (self.realtime_conversation.speaker_level > 0
                    || self
                        .realtime_conversation
                        .speaker_active_until
                        .is_some_and(|deadline| deadline > Instant::now()));
            self.realtime_conversation.input_generation = self
                .realtime_conversation
                .input_generation
                .wrapping_add(/*rhs*/ 1);
            self.realtime_conversation.transcript_input_generation =
                Some(self.realtime_conversation.input_generation);
            if interrupted && self.local_settings.tui.animations {
                self.realtime_conversation.interruption_acknowledged_until =
                    Some(Instant::now() + INTERRUPTION_ACKNOWLEDGMENT);
            }
            self.suppress_active_realtime_speaker();
        }
        if active
            && role == "assistant"
            && !delta.trim().is_empty()
            && self
                .realtime_conversation
                .assistant_transcript_generation
                .is_none()
        {
            self.realtime_conversation.assistant_transcript_generation =
                Some(self.realtime_conversation.input_generation);
            self.realtime_conversation
                .assistant_caption_started_after_speech_queue = self
                .realtime_conversation
                .pending_speech
                .iter()
                .any(|delivery| {
                    delivery.input_generation == self.realtime_conversation.input_generation
                        && delivery.state != PendingSpeechState::AwaitingTurn
                });
        }
        if active {
            self.resume_realtime_speaker_for(&role, &delta);
        }
        if active
            && role == "user"
            && self.realtime_conversation.transcript_input_generation
                == Some(self.realtime_conversation.input_generation)
        {
            self.realtime_conversation.latest_input_was_voice = true;
        }
        if self.realtime_conversation.transcript_role.as_deref() != Some(role.as_str()) {
            let previous = self.realtime_conversation.transcript_role.take();
            let previous_text = std::mem::take(&mut self.realtime_conversation.transcript);
            let saved = self.realtime_conversation.interleaved_transcript.take();
            let saved_cell = self
                .realtime_conversation
                .interleaved_transcript_cell
                .take();
            let previous_cell = self.realtime_conversation.live_transcript_cell.take();
            self.realtime_conversation.transcript = match saved {
                Some((saved_role, saved_text)) if saved_role == role => {
                    self.realtime_conversation.live_transcript_cell = saved_cell;
                    saved_text
                }
                _ => String::new(),
            };
            if let Some(previous_role) = previous {
                self.realtime_conversation.interleaved_transcript =
                    Some((previous_role, previous_text));
                self.realtime_conversation.interleaved_transcript_cell = previous_cell;
            }
            self.realtime_conversation.transcript_role = Some(role);
        }
        self.realtime_conversation.transcript.push_str(&delta);
        let mut discarded_prefix_bytes = 0;
        if self.realtime_conversation.transcript.len() > MAX_TRANSCRIPT_BYTES {
            let mut start = self.realtime_conversation.transcript.len() - MAX_TRANSCRIPT_BYTES;
            while !self
                .realtime_conversation
                .transcript
                .is_char_boundary(start)
            {
                start += 1;
            }
            self.realtime_conversation.transcript.drain(..start);
            discarded_prefix_bytes = start;
        }
        let role = self
            .realtime_conversation
            .transcript_role
            .as_deref()
            .unwrap_or("");
        let previous = self
            .realtime_conversation
            .live_transcript_cell
            .as_deref()
            .and_then(|cell| cell.as_any().downcast_ref::<SplitFlapTranscriptCell>());
        let live_cell = SplitFlapTranscriptCell::new(
            |text| self.realtime_transcript_history_cell(role, text),
            role,
            &self.realtime_conversation.transcript,
            previous,
            discarded_prefix_bytes,
            MotionMode::from_animations_enabled(self.local_settings.tui.animations),
            self.frame_requester.clone(),
        );
        self.realtime_conversation.live_transcript_cell = Some(Box::new(live_cell));
        self.bump_active_cell_revision();
        self.request_redraw();
    }

    pub(super) fn on_realtime_transcript_done(&mut self, role: String, mut text: String) {
        if matches!(
            self.realtime_conversation.phase,
            RealtimeConversationPhase::Inactive | RealtimeConversationPhase::Stopping
        ) {
            if self.realtime_conversation.phase == RealtimeConversationPhase::Inactive
                && !self.realtime_conversation.recover_late_transcripts
            {
                return;
            }
            if self.realtime_conversation.transcript_role.as_deref() == Some(role.as_str()) {
                self.realtime_conversation.transcript_role = None;
                self.realtime_conversation.transcript.clear();
                self.realtime_conversation.live_transcript_cell = None;
                self.bump_active_cell_revision();
            } else if self
                .realtime_conversation
                .interleaved_transcript
                .as_ref()
                .is_some_and(|(saved_role, _)| saved_role == &role)
            {
                self.realtime_conversation.interleaved_transcript = None;
                self.realtime_conversation.interleaved_transcript_cell = None;
                self.bump_active_cell_revision();
            }
            if text.trim().is_empty() {
                if let Some(index) = self
                    .realtime_conversation
                    .accepted_transcripts
                    .iter()
                    .rposition(|record| record.role == role && !record.complete)
                {
                    self.realtime_conversation
                        .accepted_transcripts
                        .remove(index);
                }
                return;
            }
            if text.len() > MAX_TRANSCRIPT_BYTES {
                let mut end = MAX_TRANSCRIPT_BYTES;
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                text.truncate(end);
            }
            let display_text = if let Some(partial) = self
                .realtime_conversation
                .accepted_transcripts
                .iter_mut()
                .rev()
                .find(|record| record.role == role && !record.complete)
            {
                partial.text = text.clone();
                partial.complete = true;
                text.clone()
            } else {
                // A separate final can share a prefix with an earlier caption.
                // Show it in full rather than guessing it extends that caption.
                if self.realtime_conversation.accepted_transcripts.len()
                    >= MAX_REPLAY_TRANSCRIPT_CELLS
                {
                    self.realtime_conversation.accepted_transcripts.pop_front();
                }
                self.realtime_conversation.accepted_transcripts.push_back(
                    RealtimeTranscriptRecord {
                        role: role.clone(),
                        text: text.clone(),
                        complete: true,
                        before_turn_id: None,
                    },
                );
                text.clone()
            };
            if display_text.is_empty() {
                return;
            }
            if self.realtime_conversation.pending_history_cells.len() >= MAX_REPLAY_TRANSCRIPT_CELLS
            {
                self.realtime_conversation.pending_history_cells.pop_front();
            }
            let cell = self.realtime_transcript_history_cell(&role, &display_text);
            self.realtime_conversation
                .pending_history_cells
                .push_back(cell);
            self.bump_active_cell_revision();
            self.flush_realtime_transcript_history();
            return;
        }
        if !matches!(
            self.realtime_conversation.phase,
            RealtimeConversationPhase::Starting | RealtimeConversationPhase::Active
        ) {
            return;
        }
        let has_text = !text.trim().is_empty();
        if role == "assistant" && has_text {
            // appendSpeech's RPC acknowledgement only means the request was
            // queued. A subsequent assistant caption is the closest signal
            // available here that its answer reached the voice session. The
            // protocol does not identify which speech request produced it.
            // A done-only caption has no generation owner, so it cannot retire
            // a newer answer's fallback.
            if self
                .realtime_conversation
                .assistant_caption_started_after_speech_queue
                && let Some(caption_generation) =
                self.realtime_conversation.assistant_transcript_generation
                && let Some(delivery) =
                    self.realtime_conversation
                        .pending_speech
                        .iter_mut()
                        .find(|delivery| {
                            !delivery.captioned
                                && delivery.input_generation == caption_generation
                                && delivery.state != PendingSpeechState::AwaitingTurn
                                && matches!(&delivery.item, ThreadItem::AgentMessage { text: answer, .. }
                                    if answer.trim().strip_prefix("[FINAL]").unwrap_or(answer.trim())
                                        .split_whitespace().eq(text.split_whitespace()))
                        })
            {
                delivery.captioned = true;
            }
            self.realtime_conversation
                .pending_speech
                .retain(|delivery| {
                    !delivery.captioned || matches!(delivery.state, PendingSpeechState::Queued(_))
                });
        }
        if has_text
            && self
                .realtime_conversation
                .transcript_input_generation
                .is_none()
        {
            self.resume_realtime_speaker_for(&role, &text);
        }
        // Check ownership before retiring the old output: its final chunk must not unmute it.
        if role == "assistant" {
            self.realtime_conversation.assistant_transcript_generation = None;
            self.realtime_conversation
                .assistant_caption_started_after_speech_queue = false;
        }
        let voice_input_fingerprint = (role == "user").then(|| realtime_input_fingerprint(&text));
        if role == "user" {
            match self
                .realtime_conversation
                .transcript_input_generation
                .take()
            {
                Some(generation) => {
                    if generation == self.realtime_conversation.input_generation {
                        self.realtime_conversation.latest_input_was_voice = has_text;
                        if !has_text {
                            self.release_realtime_speaker();
                        }
                    }
                }
                None => {
                    let old_superseded_transcript =
                        !self.realtime_conversation.latest_input_was_voice
                            && self
                                .realtime_conversation
                                .latest_voice_input_fingerprint
                                .is_some_and(|latest| Some(latest) == voice_input_fingerprint);
                    if old_superseded_transcript || !has_text {
                        return;
                    }
                    self.realtime_conversation.input_generation = self
                        .realtime_conversation
                        .input_generation
                        .wrapping_add(/*rhs*/ 1);
                    self.realtime_conversation.latest_input_was_voice = true;
                    self.suppress_active_realtime_speaker();
                }
            }
        }
        if self.realtime_conversation.transcript_role.as_deref() == Some(role.as_str()) {
            self.realtime_conversation.transcript.clear();
            self.realtime_conversation.transcript_role = None;
            self.realtime_conversation.live_transcript_cell = None;
            self.bump_active_cell_revision();
        } else if self
            .realtime_conversation
            .interleaved_transcript
            .as_ref()
            .is_some_and(|(saved_role, _)| saved_role == &role)
        {
            self.realtime_conversation.interleaved_transcript = None;
            self.realtime_conversation.interleaved_transcript_cell = None;
            self.bump_active_cell_revision();
        }
        if text.len() > MAX_TRANSCRIPT_BYTES {
            let mut end = MAX_TRANSCRIPT_BYTES;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
        if role == "user" {
            self.realtime_conversation.latest_voice_input_fingerprint = voice_input_fingerprint;
        }
        if !text.trim().is_empty() {
            while self.realtime_conversation.accepted_transcripts.len()
                >= MAX_PENDING_TRANSCRIPT_CELLS
            {
                self.realtime_conversation.accepted_transcripts.pop_front();
            }
            self.realtime_conversation
                .accepted_transcripts
                .push_back(RealtimeTranscriptRecord {
                    role: role.clone(),
                    text: text.clone(),
                    complete: true,
                    before_turn_id: None,
                });
            while self.realtime_conversation.pending_history_cells.len()
                >= MAX_PENDING_TRANSCRIPT_CELLS
            {
                self.realtime_conversation.pending_history_cells.pop_front();
                tracing::warn!("dropping the oldest deferred voice transcript");
            }
            let cell = self.realtime_transcript_history_cell(&role, &text);
            self.realtime_conversation
                .pending_history_cells
                .push_back(cell);
            self.bump_active_cell_revision();
            self.flush_realtime_transcript_history();
        }
    }

    fn finish_realtime_partial_transcripts(&mut self) {
        if let Some((role, text)) = self.realtime_conversation.interleaved_transcript.take() {
            self.realtime_conversation.interleaved_transcript_cell = None;
            self.on_realtime_transcript_done(role, text);
        }
        if let Some(role) = self.realtime_conversation.transcript_role.clone() {
            let text = std::mem::take(&mut self.realtime_conversation.transcript);
            self.on_realtime_transcript_done(role, text);
        }
    }

    fn realtime_transcript_history_cell(&self, role: &str, text: &str) -> Box<dyn HistoryCell> {
        if role == "user" {
            Box::new(history_cell::new_spoken_user_prompt(text.to_string()))
        } else {
            Box::new(history_cell::AgentMarkdownCell::new_spoken(
                text.to_string(),
                &self.config.cwd,
            ))
        }
    }

    pub(super) fn flush_realtime_transcript_history(&mut self) {
        if self.stream_controller.is_some()
            || self.plan_stream_controller.is_some()
            || self.pending_stream_consolidations > 0
        {
            return;
        }
        if !self.realtime_conversation.pending_history_cells.is_empty() {
            self.app_event_tx
                .send(AppEvent::CommitRealtimeTranscriptHistory);
            self.request_redraw();
        }
    }

    pub(crate) fn take_realtime_transcript_history(&mut self) -> Vec<Box<dyn HistoryCell>> {
        if self.stream_controller.is_some()
            || self.plan_stream_controller.is_some()
            || self.pending_stream_consolidations > 0
        {
            return Vec::new();
        }
        let mut cells = Vec::new();
        while let Some(cell) = self.realtime_conversation.pending_history_cells.pop_front() {
            if let Some(active) = self.take_history_insertion_prefix(cell.as_ref()) {
                cells.push(active);
            }
            cells.push(cell);
        }
        if !cells.is_empty() {
            self.bump_active_cell_revision();
        }
        cells
    }

    pub(crate) fn record_realtime_failure(&mut self) {
        if self.realtime_conversation.phase == RealtimeConversationPhase::Inactive
            || self.realtime_conversation.failure_recorded
        {
            return;
        }
        self.realtime_conversation.failure_recorded = true;
        self.session_telemetry
            .counter("codex.voice.session.failure", /*inc*/ 1, &[]);
    }

    pub(super) fn realtime_retry_cleanup_pending(&self) -> bool {
        self.realtime_conversation.startup_retry == StartupRetry::WaitingForStop
    }

    pub(crate) fn on_realtime_error(&mut self, message: String) {
        if self.realtime_conversation.phase == RealtimeConversationPhase::Inactive {
            return;
        }
        if self.realtime_conversation.phase != RealtimeConversationPhase::Stopping
            || self.realtime_retry_cleanup_pending()
        {
            self.record_realtime_failure();
        }
        self.stop_realtime_conversation();
        self.add_error_message(message);
    }

    pub(super) fn on_realtime_conversation_closed(&mut self, reason: Option<String>) {
        if self.realtime_conversation.phase == RealtimeConversationPhase::Inactive {
            if self.realtime_conversation.recover_late_transcripts {
                self.finish_realtime_partial_transcripts();
                self.realtime_conversation.recover_late_transcripts = false;
            }
            return;
        }
        let retry_after_stop =
            self.realtime_conversation.startup_retry == StartupRetry::WaitingForStop;
        let retry_after_early_close = self.realtime_conversation.phase
            == RealtimeConversationPhase::Starting
            && self.realtime_conversation.startup_retry == StartupRetry::Available
            && reason.as_deref() == Some("transport_closed");
        if self.realtime_conversation.phase == RealtimeConversationPhase::Stopping
            && reason.as_deref() != Some("requested")
        {
            return;
        }
        // A transport close may arrive without final transcript events. Recover
        // answers before flushing partial captions, which do not prove delivery.
        self.restore_all_undelivered_realtime_speech();
        self.finish_realtime_partial_transcripts();
        let muted = self.realtime_conversation.microphone_muted;
        let thread_id = self.realtime_conversation.thread_id;
        let retry_thread_id = if retry_after_stop || retry_after_early_close {
            thread_id.filter(|id| Some(*id) == self.thread_id())
        } else {
            None
        };
        if retry_thread_id.is_none()
            && (reason.is_none() || matches!(reason.as_deref(), Some("transport_closed" | "error")))
        {
            self.record_realtime_failure();
        }
        let failed = self.realtime_conversation.failure_recorded;
        self.reset_realtime_conversation();
        if let Some(thread_id) = retry_thread_id {
            // The old backend is closed. A late peer result belongs to its attempt ID.
            self.realtime_conversation.startup_retry = StartupRetry::Used;
            self.realtime_conversation.microphone_muted = muted;
            if retry_after_early_close {
                self.add_info_message(
                    "Voice connection closed during startup. Retrying once.".into(),
                    /*hint*/ None,
                );
            }
            self.start_realtime_conversation(thread_id);
            return;
        }
        if let Some(reason) = reason
            && reason != "error"
            && !(failed && reason == "requested")
        {
            self.add_info_message(
                format!("Voice conversation ended: {reason}"),
                /*hint*/ None,
            );
        }
        self.request_redraw();
    }

    fn finish_realtime_session_metrics(&mut self) {
        if let Some(active_since) = self.realtime_conversation.active_since.take() {
            self.session_telemetry
                .counter("codex.voice.session.ended", /*inc*/ 1, &[]);
            self.session_telemetry.record_duration(
                "codex.voice.session.duration",
                active_since.elapsed(),
                &[],
            );
        }
    }

    pub(crate) fn reset_realtime_conversation(&mut self) -> Option<ThreadId> {
        self.finish_realtime_session_metrics();
        let should_refresh_terminal_title = self.realtime_conversation.phase
            != RealtimeConversationPhase::Inactive
            && self.last_terminal_title.is_some();
        self.restore_all_undelivered_realtime_speech();
        // A delegated item is hidden until the turn completes so it can be spoken.
        // If voice ends first, let the later turn completion render it normally.
        if self
            .transcript
            .last_completed_agent_message
            .as_ref()
            .is_some_and(|completed_item| {
                matches!(
                    self.realtime_conversation.agent_items.get(completed_item),
                    Some(RealtimeAgentItemOrigin::Delegated {
                        completed: true,
                        ..
                    })
                )
            })
        {
            self.transcript.last_completed_agent_message = None;
        }
        // The helper may already have exited while app-server still owns the
        // voice session. An acknowledged backend start still needs a stop RPC.
        let backend_thread_id = if self.realtime_conversation.handle.is_some()
            || self.realtime_conversation.backend_started
        {
            self.realtime_conversation.thread_id
        } else {
            None
        };
        if let Some(abort) = self.realtime_conversation.startup_abort.take() {
            abort.abort();
        }
        if let Some(handle) = self.realtime_conversation.handle.take() {
            handle.close();
        }
        // Direct resets (for example, switching to a resumed thread) do not
        // pass through the normal stop/close transcript flush.
        let partials = [
            self.realtime_conversation.interleaved_transcript.take(),
            self.realtime_conversation
                .transcript_role
                .take()
                .map(|role| {
                    (
                        role,
                        std::mem::take(&mut self.realtime_conversation.transcript),
                    )
                }),
        ];
        for (role, text) in partials.into_iter().flatten() {
            if text.trim().is_empty() {
                continue;
            }
            if self.realtime_conversation.accepted_transcripts.len() >= MAX_REPLAY_TRANSCRIPT_CELLS
            {
                self.realtime_conversation.accepted_transcripts.pop_front();
            }
            if self.realtime_conversation.pending_history_cells.len() >= MAX_REPLAY_TRANSCRIPT_CELLS
            {
                self.realtime_conversation.pending_history_cells.pop_front();
            }
            let cell = self.realtime_transcript_history_cell(&role, &text);
            self.realtime_conversation
                .pending_history_cells
                .push_back(cell);
            self.bump_active_cell_revision();
            self.realtime_conversation
                .accepted_transcripts
                .push_back(RealtimeTranscriptRecord {
                    role,
                    text,
                    complete: true,
                    before_turn_id: None,
                });
        }
        let pending_history_cells =
            std::mem::take(&mut self.realtime_conversation.pending_history_cells);
        let accepted_transcripts =
            std::mem::take(&mut self.realtime_conversation.accepted_transcripts);
        let delegated_reasoning_turns =
            std::mem::take(&mut self.realtime_conversation.delegated_reasoning_turns);
        let had_live_transcript = self
            .realtime_conversation
            .live_transcript_cells()
            .next()
            .is_some();
        self.realtime_conversation = RealtimeConversationUiState {
            attempt_id: self.realtime_conversation.attempt_id,
            pending_history_cells,
            accepted_transcripts,
            delegated_reasoning_turns,
            ..RealtimeConversationUiState::default()
        };
        if had_live_transcript {
            self.bump_active_cell_revision();
        }
        self.bottom_pane.set_voice_strip(/*state*/ None);
        self.flush_realtime_transcript_history();
        if should_refresh_terminal_title {
            self.refresh_terminal_title();
        }
        backend_thread_id
    }
}
