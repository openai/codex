use super::*;
use codex_protocol::protocol::TurnAbortReason;
use pretty_assertions::assert_eq;

#[test]
fn client_response_payload_returns_native_client_response() {
    let Some(ClientResponse::ThreadArchive {
        request_id,
        response: _,
    }) = ClientResponsePayload::ThreadArchive(v2::ThreadArchiveResponse {})
        .into_client_response(RequestId::Integer(7))
    else {
        panic!("expected thread/archive client response");
    };
    assert_eq!(request_id, RequestId::Integer(7));
}

#[test]
fn v1_interrupt_conversation_has_no_native_v2_response() {
    let response =
        ClientResponsePayload::InterruptConversation(v1::InterruptConversationResponse {
            abort_reason: TurnAbortReason::Interrupted,
        })
        .into_client_response(RequestId::Integer(8));

    assert!(response.is_none());
}
