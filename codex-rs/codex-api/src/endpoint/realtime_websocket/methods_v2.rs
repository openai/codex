use crate::endpoint::realtime_websocket::methods_common::REALTIME_AUDIO_SAMPLE_RATE;
use crate::endpoint::realtime_websocket::protocol::AudioFormatType;
use crate::endpoint::realtime_websocket::protocol::ConversationContentType;
use crate::endpoint::realtime_websocket::protocol::ConversationFunctionCallOutputItem;
use crate::endpoint::realtime_websocket::protocol::ConversationItemContent;
use crate::endpoint::realtime_websocket::protocol::ConversationItemPayload;
use crate::endpoint::realtime_websocket::protocol::ConversationItemType;
use crate::endpoint::realtime_websocket::protocol::ConversationMessageItem;
use crate::endpoint::realtime_websocket::protocol::ConversationRole;
use crate::endpoint::realtime_websocket::protocol::NoiseReductionType;
use crate::endpoint::realtime_websocket::protocol::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::protocol::RealtimeSessionMode;
use crate::endpoint::realtime_websocket::protocol::RealtimeVoice;
use crate::endpoint::realtime_websocket::protocol::SessionAudio;
use crate::endpoint::realtime_websocket::protocol::SessionAudioFormat;
use crate::endpoint::realtime_websocket::protocol::SessionAudioInput;
use crate::endpoint::realtime_websocket::protocol::SessionFunctionTool;
use crate::endpoint::realtime_websocket::protocol::SessionNoiseReduction;
use crate::endpoint::realtime_websocket::protocol::SessionToolType;
use crate::endpoint::realtime_websocket::protocol::SessionTurnDetection;
use crate::endpoint::realtime_websocket::protocol::SessionType;
use crate::endpoint::realtime_websocket::protocol::SessionUpdateSession;
use crate::endpoint::realtime_websocket::protocol::TurnDetectionType;
use serde_json::json;

const REALTIME_V2_OUTPUT_MODALITY_TEXT: &str = "text";
const REALTIME_V2_TOOL_CHOICE: &str = "auto";
const REALTIME_V2_BACKGROUND_AGENT_TOOL_NAME: &str = "background_agent";
const REALTIME_V2_BACKGROUND_AGENT_TOOL_DESCRIPTION: &str = "Send a user request to the background agent. Use this as the default action. Do not rephrase the user's ask or rewrite it in your own words; pass along the user's own words. If the background agent is idle, this starts a new task and returns the final result to the user. If the background agent is already working on a task, this sends the request as guidance to steer that previous task. If the user asks to do something next, later, after this, or once current work finishes, call this tool so the work is actually queued instead of merely promising to do it later.";

pub(super) fn conversation_item_create_message(text: String) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::ConversationItemCreate {
        item: ConversationItemPayload::Message(ConversationMessageItem {
            r#type: ConversationItemType::Message,
            role: ConversationRole::User,
            content: vec![ConversationItemContent {
                r#type: ConversationContentType::InputText,
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
            r#type: ConversationItemType::FunctionCallOutput,
            call_id: handoff_id,
            output: output_text,
        }),
    }
}

pub(super) fn session_update_session(
    instructions: String,
    session_mode: RealtimeSessionMode,
    _voice: RealtimeVoice,
) -> SessionUpdateSession {
    match session_mode {
        RealtimeSessionMode::Conversational => SessionUpdateSession {
            id: None,
            r#type: SessionType::Realtime,
            model: None,
            instructions: Some(instructions),
            output_modalities: Some(vec![REALTIME_V2_OUTPUT_MODALITY_TEXT.to_string()]),
            audio: SessionAudio {
                input: SessionAudioInput {
                    format: SessionAudioFormat {
                        r#type: AudioFormatType::AudioPcm,
                        rate: REALTIME_AUDIO_SAMPLE_RATE,
                    },
                    noise_reduction: Some(SessionNoiseReduction {
                        r#type: NoiseReductionType::NearField,
                    }),
                    turn_detection: Some(SessionTurnDetection {
                        r#type: TurnDetectionType::ServerVad,
                        interrupt_response: true,
                        create_response: true,
                    }),
                },
                output: None,
            },
            tools: Some(vec![SessionFunctionTool {
                r#type: SessionToolType::Function,
                name: REALTIME_V2_BACKGROUND_AGENT_TOOL_NAME.to_string(),
                description: REALTIME_V2_BACKGROUND_AGENT_TOOL_DESCRIPTION.to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The user request to delegate to the background agent."
                        }
                    },
                    "required": ["prompt"],
                    "additionalProperties": false
                }),
            }]),
            tool_choice: Some(REALTIME_V2_TOOL_CHOICE.to_string()),
        },
        RealtimeSessionMode::Transcription => SessionUpdateSession {
            id: None,
            r#type: SessionType::Transcription,
            model: None,
            instructions: None,
            output_modalities: None,
            audio: SessionAudio {
                input: SessionAudioInput {
                    format: SessionAudioFormat {
                        r#type: AudioFormatType::AudioPcm,
                        rate: REALTIME_AUDIO_SAMPLE_RATE,
                    },
                    noise_reduction: None,
                    turn_detection: None,
                },
                output: None,
            },
            tools: None,
            tool_choice: None,
        },
    }
}

pub(super) fn websocket_intent() -> Option<&'static str> {
    None
}
