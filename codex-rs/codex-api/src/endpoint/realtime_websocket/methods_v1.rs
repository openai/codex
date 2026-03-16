use crate::endpoint::realtime_websocket::methods_common::REALTIME_AUDIO_FORMAT;
use crate::endpoint::realtime_websocket::methods_common::REALTIME_AUDIO_SAMPLE_RATE;
use crate::endpoint::realtime_websocket::protocol::ConversationItemContent;
use crate::endpoint::realtime_websocket::protocol::ConversationItemPayload;
use crate::endpoint::realtime_websocket::protocol::ConversationMessageItem;
use crate::endpoint::realtime_websocket::protocol::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::protocol::SessionAudio;
use crate::endpoint::realtime_websocket::protocol::SessionAudioFormat;
use crate::endpoint::realtime_websocket::protocol::SessionAudioInput;
use crate::endpoint::realtime_websocket::protocol::SessionAudioOutput;
use crate::endpoint::realtime_websocket::protocol::SessionAudioVoice;
use crate::endpoint::realtime_websocket::protocol::SessionUpdateSession;

const REALTIME_V1_SESSION_TYPE: &str = "quicksilver";

pub(super) fn conversation_item_create_message(text: String) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::ConversationItemCreate {
        item: ConversationItemPayload::Message(ConversationMessageItem {
            kind: "message".to_string(),
            role: "user".to_string(),
            content: vec![ConversationItemContent {
                kind: "text".to_string(),
                text,
            }],
        }),
    }
}

pub(super) fn conversation_handoff_append_message(
    handoff_id: String,
    output_text: String,
) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::ConversationHandoffAppend {
        handoff_id,
        output_text,
    }
}

pub(super) fn session_update_session(instructions: String) -> SessionUpdateSession {
    SessionUpdateSession {
        kind: REALTIME_V1_SESSION_TYPE.to_string(),
        instructions: Some(instructions),
        output_modalities: None,
        audio: SessionAudio {
            input: SessionAudioInput {
                format: SessionAudioFormat {
                    kind: REALTIME_AUDIO_FORMAT.to_string(),
                    rate: REALTIME_AUDIO_SAMPLE_RATE,
                },
                noise_reduction: None,
                turn_detection: None,
            },
            output: Some(SessionAudioOutput {
                format: None,
                voice: SessionAudioVoice::Fathom,
            }),
        },
        tools: None,
        tool_choice: None,
    }
}

pub(super) fn websocket_intent() -> Option<&'static str> {
    Some("quicksilver")
}
