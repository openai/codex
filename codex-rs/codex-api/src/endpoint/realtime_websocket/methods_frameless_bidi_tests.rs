use super::CONTEXT_APPEND_MAX_BYTES;
use super::context_append_chunks;
use super::session_json;
use crate::endpoint::realtime_websocket::protocol::RealtimeVoice;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::ConversationTextRole;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn context_append_chunks_preserve_text_within_wire_limit() {
    for text in ["a".repeat(1_201), "🙂".repeat(200)] {
        let chunks = context_append_chunks(&text);
        assert_eq!(chunks.concat(), text);
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.len() <= CONTEXT_APPEND_MAX_BYTES)
        );
    }
}

#[test]
fn session_json_omits_initial_items_when_empty() {
    let session = session_json(
        Some("gpt-live".to_string()),
        "instructions".to_string(),
        Vec::new(),
        RealtimeVoice::Marin,
    );

    assert_eq!(
        session,
        json!({
            "model": "gpt-live",
            "instructions": "instructions",
            "audio": {
                "output": {
                    "voice": "marin",
                },
            },
            "delegation": {
                "type": "client",
            },
        })
    );
}

#[test]
fn session_json_encodes_role_bearing_initial_items() {
    let session = session_json(
        Some("gpt-live".to_string()),
        "instructions".to_string(),
        vec![
            ConversationTextParams {
                text: "Remember this.".to_string(),
                role: ConversationTextRole::Developer,
            },
            ConversationTextParams {
                text: "What do you remember?".to_string(),
                role: ConversationTextRole::User,
            },
            ConversationTextParams {
                text: "I remember.".to_string(),
                role: ConversationTextRole::Assistant,
            },
        ],
        RealtimeVoice::Marin,
    );

    assert_eq!(
        session["initial_items"],
        json!([
            {
                "type": "message",
                "role": "developer",
                "content": [{
                    "type": "input_text",
                    "text": "Remember this.",
                }],
            },
            {
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": "What do you remember?",
                }],
            },
            {
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": "I remember.",
                }],
            },
        ])
    );
}
