//! Protect connection credentials in diagnostics without changing signaling data.

use super::AppCommand;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn realtime_offer_is_redacted_in_diagnostics_and_preserved_for_signaling() {
    let thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000001").expect("valid thread id");
    let original_offer =
        "v=0\r\na=ice-ufrag:private-user\r\na=ice-pwd:private-credential\r\n".to_string();
    let command = AppCommand::RealtimeConversationStart {
        thread_id,
        attempt_id: 1,
        offer_sdp: original_offer.clone().into(),
    };

    assert_eq!(
        format!("{command:?}"),
        format!(
            "RealtimeConversationStart {{ thread_id: {thread_id:?}, attempt_id: 1, offer_sdp: <redacted> }}"
        )
    );
    assert_eq!(
        serde_json::to_value(&command).expect("command diagnostics serialize"),
        json!({
            "RealtimeConversationStart": {
                "thread_id": thread_id,
                "attempt_id": 1,
                "offer_sdp": "<redacted>"
            }
        })
    );

    let AppCommand::RealtimeConversationStart { offer_sdp, .. } = command else {
        panic!("expected the start command");
    };
    assert_eq!(String::from(offer_sdp), original_offer);
}

#[test]
fn realtime_speech_is_redacted_in_diagnostics_and_preserved_for_signaling() {
    let thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000001").expect("valid thread id");
    let original_text = "Private spoken answer".to_string();
    let command = AppCommand::RealtimeConversationSpeech {
        thread_id,
        attempt_id: 1,
        input_generation: 2,
        delivery_id: 3,
        text: original_text.clone().into(),
    };

    assert!(!format!("{command:?}").contains(&original_text));
    assert_eq!(
        serde_json::to_value(&command).expect("command diagnostics serialize"),
        json!({
            "RealtimeConversationSpeech": {
                "thread_id": thread_id,
                "attempt_id": 1,
                "input_generation": 2,
                "delivery_id": 3,
                "text": "<redacted>"
            }
        })
    );

    let AppCommand::RealtimeConversationSpeech { text, .. } = command else {
        panic!("expected the speech command");
    };
    assert_eq!(String::from(text), original_text);
}
