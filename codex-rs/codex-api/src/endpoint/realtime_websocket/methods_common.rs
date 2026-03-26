use crate::endpoint::realtime_websocket::methods_v1::conversation_handoff_append_message as v1_conversation_handoff_append_message;
use crate::endpoint::realtime_websocket::methods_v1::conversation_item_create_message as v1_conversation_item_create_message;
use crate::endpoint::realtime_websocket::methods_v1::session_update_session as v1_session_update_session;
use crate::endpoint::realtime_websocket::methods_v1::websocket_intent as v1_websocket_intent;
use crate::endpoint::realtime_websocket::methods_v2::conversation_handoff_append_message as v2_conversation_handoff_append_message;
use crate::endpoint::realtime_websocket::methods_v2::conversation_item_create_message as v2_conversation_item_create_message;
use crate::endpoint::realtime_websocket::methods_v2::session_update_session as v2_session_update_session;
use crate::endpoint::realtime_websocket::methods_v2::websocket_intent as v2_websocket_intent;
use crate::endpoint::realtime_websocket::protocol::RealtimeEventParser;
use crate::endpoint::realtime_websocket::protocol::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::protocol::RealtimeSessionMode;
use crate::endpoint::realtime_websocket::protocol::SessionUpdateSession;

pub(super) const REALTIME_AUDIO_SAMPLE_RATE: u32 = 24_000;
const AGENT_FINAL_MESSAGE_PREFIX: &str = "\"Agent Final Message\":\n\n";

pub(super) fn normalized_session_mode(
    event_parser: RealtimeEventParser,
    session_mode: RealtimeSessionMode,
) -> RealtimeSessionMode {
    match event_parser {
        RealtimeEventParser::V1 => RealtimeSessionMode::Conversational,
        RealtimeEventParser::RealtimeV2 => session_mode,
    }
}

pub(super) fn conversation_item_create_message(
    event_parser: RealtimeEventParser,
    text: String,
) -> RealtimeOutboundMessage {
    match event_parser {
        RealtimeEventParser::V1 => v1_conversation_item_create_message(text),
        RealtimeEventParser::RealtimeV2 => v2_conversation_item_create_message(text),
    }
}

pub(super) fn conversation_handoff_append_message(
    event_parser: RealtimeEventParser,
    handoff_id: String,
    output_text: String,
) -> RealtimeOutboundMessage {
    let output_text = format!("{AGENT_FINAL_MESSAGE_PREFIX}{output_text}");
    match event_parser {
        RealtimeEventParser::V1 => v1_conversation_handoff_append_message(handoff_id, output_text),
        RealtimeEventParser::RealtimeV2 => {
            v2_conversation_handoff_append_message(handoff_id, output_text)
        }
    }
}

pub(super) fn session_update_session(
    event_parser: RealtimeEventParser,
    instructions: String,
    session_mode: RealtimeSessionMode,
) -> SessionUpdateSession {
    let session_mode = normalized_session_mode(event_parser, session_mode);
    match event_parser {
        RealtimeEventParser::V1 => v1_session_update_session(instructions),
        RealtimeEventParser::RealtimeV2 => v2_session_update_session(instructions, session_mode),
    }
}

pub(super) fn websocket_intent(event_parser: RealtimeEventParser) -> Option<&'static str> {
    match event_parser {
        RealtimeEventParser::V1 => v1_websocket_intent(),
        RealtimeEventParser::RealtimeV2 => v2_websocket_intent(),
    }
}
