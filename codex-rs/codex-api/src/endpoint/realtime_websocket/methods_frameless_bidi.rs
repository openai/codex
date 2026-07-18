use crate::endpoint::realtime_websocket::protocol::FramelessContentType;
use crate::endpoint::realtime_websocket::protocol::FramelessInputTextContent;
use crate::endpoint::realtime_websocket::protocol::RealtimeContextAppendChannel;
use crate::endpoint::realtime_websocket::protocol::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::protocol::RealtimeVoice;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::ConversationTextRole;
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
    initial_items: Vec<ConversationTextParams>,
    voice: RealtimeVoice,
) -> RealtimeOutboundMessage {
    RealtimeOutboundMessage::FramelessSessionUpdate {
        session: session_json(/*model*/ None, instructions, initial_items, voice),
    }
}

pub(super) fn session_json(
    model: Option<String>,
    instructions: String,
    initial_items: Vec<ConversationTextParams>,
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
    if !initial_items.is_empty() {
        session["initial_items"] = Value::Array(
            initial_items
                .into_iter()
                .map(|item| {
                    let content_type = match item.role {
                        ConversationTextRole::User | ConversationTextRole::Developer => {
                            "input_text"
                        }
                        ConversationTextRole::Assistant => "output_text",
                    };
                    json!({
                        "type": "message",
                        "role": item.role,
                        "content": [{
                            "type": content_type,
                            "text": item.text,
                        }],
                    })
                })
                .collect(),
        );
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
