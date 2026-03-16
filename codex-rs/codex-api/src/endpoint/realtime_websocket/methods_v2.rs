use crate::endpoint::realtime_websocket::methods_common::REALTIME_AUDIO_FORMAT;
use crate::endpoint::realtime_websocket::methods_common::REALTIME_AUDIO_SAMPLE_RATE;
use crate::endpoint::realtime_websocket::protocol::ConversationFunctionCallOutputItem;
use crate::endpoint::realtime_websocket::protocol::ConversationItemContent;
use crate::endpoint::realtime_websocket::protocol::ConversationItemPayload;
use crate::endpoint::realtime_websocket::protocol::ConversationMessageItem;
use crate::endpoint::realtime_websocket::protocol::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::protocol::RealtimeSessionMode;
use crate::endpoint::realtime_websocket::protocol::SessionAudio;
use crate::endpoint::realtime_websocket::protocol::SessionAudioFormat;
use crate::endpoint::realtime_websocket::protocol::SessionAudioInput;
use crate::endpoint::realtime_websocket::protocol::SessionAudioOutput;
use crate::endpoint::realtime_websocket::protocol::SessionAudioVoice;
use crate::endpoint::realtime_websocket::protocol::SessionFunctionTool;
use crate::endpoint::realtime_websocket::protocol::SessionUpdateSession;
use serde_json::json;

const REALTIME_V2_SESSION_TYPE: &str = "realtime";
const REALTIME_V2_CODEX_TOOL_NAME: &str = "codex";
const REALTIME_V2_CODEX_TOOL_DESCRIPTION: &str = "Delegate work to Codex and return the result.";

pub(super) fn conversation_item_create_message(text: String) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::ConversationItemCreate {
        item: ConversationItemPayload::Message(ConversationMessageItem {
            kind: "message".to_string(),
            role: "user".to_string(),
            content: vec![ConversationItemContent {
                kind: "input_text".to_string(),
                text,
            }],
        }),
    }
}

pub(super) fn conversation_handoff_append_message(
    handoff_id: String,
    output_text: String,
) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::ConversationItemCreate {
        item: ConversationItemPayload::FunctionCallOutput(ConversationFunctionCallOutputItem {
            kind: "function_call_output".to_string(),
            call_id: handoff_id,
            output: output_text,
        }),
    }
}

pub(super) fn session_update_session(
    instructions: String,
    session_mode: RealtimeSessionMode,
) -> SessionUpdateSession {
    match session_mode {
        RealtimeSessionMode::Conversational => SessionUpdateSession {
            kind: REALTIME_V2_SESSION_TYPE.to_string(),
            instructions: Some(instructions),
            audio: SessionAudio {
                input: SessionAudioInput {
                    format: SessionAudioFormat {
                        kind: REALTIME_AUDIO_FORMAT.to_string(),
                        rate: REALTIME_AUDIO_SAMPLE_RATE,
                    },
                },
                output: Some(SessionAudioOutput {
                    voice: SessionAudioVoice::Alloy,
                }),
            },
            tools: Some(vec![SessionFunctionTool {
                kind: "function".to_string(),
                name: REALTIME_V2_CODEX_TOOL_NAME.to_string(),
                description: REALTIME_V2_CODEX_TOOL_DESCRIPTION.to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "Prompt text for the delegated Codex task."
                        }
                    },
                    "required": ["prompt"],
                    "additionalProperties": false
                }),
            }]),
        },
        RealtimeSessionMode::Transcription => SessionUpdateSession {
            kind: "transcription".to_string(),
            instructions: None,
            audio: SessionAudio {
                input: SessionAudioInput {
                    format: SessionAudioFormat {
                        kind: REALTIME_AUDIO_FORMAT.to_string(),
                        rate: REALTIME_AUDIO_SAMPLE_RATE,
                    },
                },
                output: None,
            },
            tools: None,
        },
    }
}

pub(super) fn websocket_intent() -> Option<&'static str> {
    None
}
