use crate::endpoint::realtime_websocket::methods_frameless_bidi::delegation_context_append_message as frameless_delegation_context_append_message;
use crate::endpoint::realtime_websocket::methods_frameless_bidi::session_context_append_message as frameless_session_context_append_message;
use crate::endpoint::realtime_websocket::methods_frameless_bidi::session_json as frameless_session_json;
use crate::endpoint::realtime_websocket::methods_frameless_bidi::session_update_message as frameless_session_update_message;
use crate::endpoint::realtime_websocket::methods_v1::conversation_handoff_append_message as v1_conversation_handoff_append_message;
use crate::endpoint::realtime_websocket::methods_v1::conversation_item_create_message as v1_conversation_item_create_message;
use crate::endpoint::realtime_websocket::methods_v1::session_update_session as v1_session_update_session;
use crate::endpoint::realtime_websocket::methods_v1::websocket_intent as v1_websocket_intent;
use crate::endpoint::realtime_websocket::methods_v2::conversation_function_call_output_message as v2_conversation_function_call_output_message;
use crate::endpoint::realtime_websocket::methods_v2::conversation_item_create_message as v2_conversation_item_create_message;
use crate::endpoint::realtime_websocket::methods_v2::session_update_session as v2_session_update_session;
use crate::endpoint::realtime_websocket::methods_v2::websocket_intent as v2_websocket_intent;
use crate::endpoint::realtime_websocket::protocol::RealtimeContextAppendChannel;
use crate::endpoint::realtime_websocket::protocol::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::protocol::RealtimeOutputModality;
use crate::endpoint::realtime_websocket::protocol::RealtimeSessionConfig;
use crate::endpoint::realtime_websocket::protocol::RealtimeSessionMode;
use crate::endpoint::realtime_websocket::protocol::RealtimeVoice;
use crate::endpoint::realtime_websocket::protocol::RealtimeWireAdapter;
use codex_protocol::protocol::ConversationTextRole;
use serde_json::Result as JsonResult;
use serde_json::Value;
use serde_json::to_value;

pub(super) const REALTIME_AUDIO_SAMPLE_RATE: u32 = 24_000;
const AGENT_FINAL_MESSAGE_PREFIX: &str = "\"Agent Final Message\":\n\n";

pub(super) fn normalized_session_mode(
    wire_adapter: RealtimeWireAdapter,
    session_mode: RealtimeSessionMode,
) -> RealtimeSessionMode {
    match wire_adapter {
        RealtimeWireAdapter::V1 | RealtimeWireAdapter::FramelessBidi => {
            RealtimeSessionMode::Conversational
        }
        RealtimeWireAdapter::RealtimeV2 => session_mode,
    }
}

pub(super) fn conversation_item_create_message(
    wire_adapter: RealtimeWireAdapter,
    text: String,
    role: ConversationTextRole,
    context_append_channel: Option<RealtimeContextAppendChannel>,
) -> RealtimeOutboundMessage {
    match wire_adapter {
        RealtimeWireAdapter::V1 => v1_conversation_item_create_message(text, role),
        RealtimeWireAdapter::FramelessBidi => {
            frameless_session_context_append_message(text, context_append_channel)
        }
        RealtimeWireAdapter::RealtimeV2 => v2_conversation_item_create_message(text, role),
    }
}

pub(super) fn conversation_handoff_append_message(
    wire_adapter: RealtimeWireAdapter,
    handoff_id: String,
    output_text: String,
    context_append_channel: Option<RealtimeContextAppendChannel>,
) -> RealtimeOutboundMessage {
    match wire_adapter {
        RealtimeWireAdapter::V1 => v1_conversation_handoff_append_message(handoff_id, output_text),
        RealtimeWireAdapter::FramelessBidi => frameless_delegation_context_append_message(
            handoff_id,
            output_text,
            context_append_channel,
        ),
        RealtimeWireAdapter::RealtimeV2 => {
            unreachable!("realtime v2 does not send conversation handoff output")
        }
    }
}

pub(super) fn standalone_handoff_message(
    wire_adapter: RealtimeWireAdapter,
    handoff_id: String,
    output_text: String,
    context_append_channel: Option<RealtimeContextAppendChannel>,
) -> RealtimeOutboundMessage {
    match wire_adapter {
        RealtimeWireAdapter::V1 => v1_conversation_handoff_append_message(handoff_id, output_text),
        RealtimeWireAdapter::FramelessBidi => {
            frameless_session_context_append_message(output_text, context_append_channel)
        }
        RealtimeWireAdapter::RealtimeV2 => {
            unreachable!("realtime v2 does not send standalone handoff output")
        }
    }
}

pub(super) fn conversation_function_call_output_message(
    wire_adapter: RealtimeWireAdapter,
    call_id: String,
    output_text: String,
    context_append_channel: Option<RealtimeContextAppendChannel>,
) -> RealtimeOutboundMessage {
    match wire_adapter {
        RealtimeWireAdapter::V1 => v1_conversation_handoff_append_message(
            call_id,
            format!("{AGENT_FINAL_MESSAGE_PREFIX}{output_text}"),
        ),
        RealtimeWireAdapter::FramelessBidi => frameless_delegation_context_append_message(
            call_id,
            output_text,
            context_append_channel,
        ),
        RealtimeWireAdapter::RealtimeV2 => {
            v2_conversation_function_call_output_message(call_id, output_text)
        }
    }
}

pub(super) fn session_update_message(
    wire_adapter: RealtimeWireAdapter,
    instructions: String,
    session_mode: RealtimeSessionMode,
    output_modality: RealtimeOutputModality,
    voice: RealtimeVoice,
) -> RealtimeOutboundMessage {
    let session_mode = normalized_session_mode(wire_adapter, session_mode);
    match wire_adapter {
        RealtimeWireAdapter::V1 => RealtimeOutboundMessage::SessionUpdate {
            session: v1_session_update_session(instructions, voice),
        },
        RealtimeWireAdapter::FramelessBidi => frameless_session_update_message(instructions, voice),
        RealtimeWireAdapter::RealtimeV2 => RealtimeOutboundMessage::SessionUpdate {
            session: v2_session_update_session(instructions, session_mode, output_modality, voice),
        },
    }
}

pub fn session_update_session_json(config: RealtimeSessionConfig) -> JsonResult<Value> {
    match config.event_parser {
        RealtimeWireAdapter::V1 | RealtimeWireAdapter::RealtimeV2 => {
            let mut session = match config.event_parser {
                RealtimeWireAdapter::V1 => {
                    v1_session_update_session(config.instructions, config.voice)
                }
                RealtimeWireAdapter::RealtimeV2 => v2_session_update_session(
                    config.instructions,
                    config.session_mode,
                    config.output_modality,
                    config.voice,
                ),
                RealtimeWireAdapter::FramelessBidi => unreachable!(),
            };
            session.id = config.session_id;
            session.model = config.model;
            to_value(session)
        }
        RealtimeWireAdapter::FramelessBidi => Ok(frameless_session_json(
            config.model,
            config.instructions,
            config.voice,
        )),
    }
}

pub(super) fn websocket_intent(wire_adapter: RealtimeWireAdapter) -> Option<&'static str> {
    match wire_adapter {
        RealtimeWireAdapter::V1 => v1_websocket_intent(),
        RealtimeWireAdapter::FramelessBidi => None,
        RealtimeWireAdapter::RealtimeV2 => v2_websocket_intent(),
    }
}

#[cfg(test)]
#[path = "methods_common_tests.rs"]
mod tests;
