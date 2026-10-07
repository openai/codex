//! Start diagnostics retain routing-independent metadata without exposing input content.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn realtime_start_debug_redacts_text_and_transport() {
    let secret = "credential-bearing-content";
    let mut params = ConversationStartParams {
        client_managed_handoffs: false,
        delegation_ack_filler: None,
        flush_transcript_tail_on_session_end: false,
        codex_responses_as_items: false,
        codex_response_item_prefix: Some(secret.to_owned()),
        codex_response_handoff_mode: Default::default(),
        backend_reasoning_status: false,
        codex_response_handoff_channel_prefixes: Some(BTreeMap::from([(
            secret.to_owned(),
            vec![secret.to_owned()],
        )])),
        model: Some(secret.to_owned()),
        output_modality: RealtimeOutputModality::Audio,
        include_startup_context: true,
        initial_items: vec![ConversationTextParams {
            role: ConversationTextRole::Developer,
            text: secret.to_owned(),
        }],
        realtime_start_instructions: Some(secret.to_owned()),
        realtime_end_instructions: Some(secret.to_owned()),
        prompt: Some(Some(secret.to_owned())),
        realtime_session_id: Some(secret.to_owned()),
        transport: None,
        version: Some(RealtimeConversationVersion::V3),
        voice: None,
    };
    for transport in [
        ConversationStartTransport::ExistingCall {
            call_id: secret.to_owned(),
            sideband_base_url: Some(format!("wss://example.test/?token={secret}")),
        },
        ConversationStartTransport::Webrtc {
            sdp: secret.to_owned(),
        },
    ] {
        params.transport = Some(transport);
        assert_eq!(
            format!("{params:?}"),
            "ConversationStartParams { output_modality: Audio, initial_items_count: 1, version: Some(V3), .. }"
        );
    }
}
