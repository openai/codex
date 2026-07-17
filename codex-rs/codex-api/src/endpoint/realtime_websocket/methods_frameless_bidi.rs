use crate::endpoint::realtime_websocket::protocol::FramelessContentType;
use crate::endpoint::realtime_websocket::protocol::FramelessInputTextContent;
use crate::endpoint::realtime_websocket::protocol::RealtimeContextAppendChannel;
use crate::endpoint::realtime_websocket::protocol::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::protocol::RealtimeVoice;
use serde_json::Value;
use serde_json::json;

const CONTEXT_APPEND_MAX_BYTES: usize = 500;

pub(super) fn delegation_context_append_message(
    delegation_item_id: String,
    text: String,
    channel: Option<RealtimeContextAppendChannel>,
) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::DelegationContextAppend {
        delegation_item_id,
        channel,
        content: input_text_content(text),
    }
}

pub(super) fn session_context_append_message(
    text: String,
    channel: Option<RealtimeContextAppendChannel>,
) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::SessionContextAppend {
        channel,
        content: input_text_content(text),
    }
}

pub(super) fn session_update_message(
    instructions: String,
    voice: RealtimeVoice,
) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::FramelessSessionUpdate {
        session: session_json(/*model*/ None, instructions, voice),
    }
}

pub(super) fn session_json(
    model: Option<String>,
    instructions: String,
    voice: RealtimeVoice,
) -> Value {
    let mut session = json!({
        "instructions": instructions,
        "audio": {
            "output": {
                "voice": voice,
            },
        },
        "delegation": {
            "type": "client",
        },
    });
    if let Some(model) = model {
        session["model"] = Value::String(model);
    }
    session
}

fn input_text_content(text: String) -> Vec<FramelessInputTextContent> {
    vec![FramelessInputTextContent {
        r#type: FramelessContentType::InputText,
        text,
    }]
}

pub(super) fn context_append_chunks(text: &str) -> Vec<String> {
    if text.len() <= CONTEXT_APPEND_MAX_BYTES {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut end = (start + CONTEXT_APPEND_MAX_BYTES).min(text.len());
        while end > start && !text.is_char_boundary(end) {
            end -= 1;
        }
        chunks.push(text[start..end].to_string());
        start = end;
    }
    chunks
}

#[cfg(test)]
#[path = "methods_frameless_bidi_tests.rs"]
mod tests;
