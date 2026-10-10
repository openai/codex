//! Bounded failure classification for locally owned voice sessions.
//! Each session records at most one cause, while lifecycle phase remains an independent fact.

use super::RealtimeConversationPhase;
use crate::chatwidget::ChatWidget;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RealtimeFailureCause {
    AppServerDisconnect,
    AppServerRequest,
    AudioControls,
    AudioDevices,
    AudioSession,
    Backend,
    ConnectionUnknown,
    HelperStartup,
    NegotiationTimedOut,
    LocalRequest,
    RuntimeInitialization,
    Shutdown,
    SpeechDelivery,
    Transport,
    TransportClosed,
    UnknownClose,
}

impl RealtimeFailureCause {
    fn as_str(self) -> &'static str {
        match self {
            Self::AppServerDisconnect => "app_server_disconnect",
            Self::AppServerRequest => "app_server_request",
            Self::AudioControls => "audio_controls",
            Self::AudioDevices => "audio_devices",
            Self::AudioSession => "audio_session",
            Self::Backend => "backend",
            Self::ConnectionUnknown => "connection_unknown",
            Self::HelperStartup => "helper_startup",
            Self::NegotiationTimedOut => "negotiation_timed_out",
            Self::LocalRequest => "local_request",
            Self::RuntimeInitialization => "runtime_initialization",
            Self::Shutdown => "shutdown",
            Self::SpeechDelivery => "speech_delivery",
            Self::Transport => "transport",
            Self::TransportClosed => "transport_closed",
            Self::UnknownClose => "unknown_close",
        }
    }
}

impl From<codex_realtime_webrtc::ConnectionError> for RealtimeFailureCause {
    fn from(error: codex_realtime_webrtc::ConnectionError) -> Self {
        match error {
            codex_realtime_webrtc::ConnectionError::NegotiationTimedOut => {
                Self::NegotiationTimedOut
            }
            codex_realtime_webrtc::ConnectionError::Failed => Self::ConnectionUnknown,
            codex_realtime_webrtc::ConnectionError::HelperStartup => Self::HelperStartup,
            codex_realtime_webrtc::ConnectionError::RuntimeInitialization => {
                Self::RuntimeInitialization
            }
            codex_realtime_webrtc::ConnectionError::Transport => Self::Transport,
            codex_realtime_webrtc::ConnectionError::AudioDevices => Self::AudioDevices,
            codex_realtime_webrtc::ConnectionError::AudioControls => Self::AudioControls,
            codex_realtime_webrtc::ConnectionError::AudioSession => Self::AudioSession,
            codex_realtime_webrtc::ConnectionError::Shutdown => Self::Shutdown,
        }
    }
}

impl ChatWidget {
    pub(crate) fn record_realtime_failure(&mut self, cause: RealtimeFailureCause) {
        if self.realtime_conversation.phase == RealtimeConversationPhase::Inactive
            || self.realtime_conversation.failure_recorded
        {
            return;
        }
        self.realtime_conversation.failure_recorded = true;
        let phase = match self.realtime_conversation.phase {
            RealtimeConversationPhase::Inactive => "inactive",
            RealtimeConversationPhase::Starting => "starting",
            RealtimeConversationPhase::Active => "active",
            RealtimeConversationPhase::Stopping => "stopping",
        };
        self.session_telemetry.counter(
            "codex.voice.session.failure",
            /*inc*/ 1,
            &[("cause", cause.as_str()), ("phase", phase)],
        );
    }
}
