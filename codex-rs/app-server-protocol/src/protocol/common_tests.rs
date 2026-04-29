use super::*;
use anyhow::Result;
use codex_protocol::protocol::TurnAbortReason;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn client_response_payload_returns_jsonrpc_parts_and_client_response() -> Result<()> {
    let (request_id, result, payload) =
        ClientResponsePayload::ThreadArchive(v2::ThreadArchiveResponse {})
            .into_jsonrpc_parts_and_payload(RequestId::Integer(7))?;

    assert_eq!(request_id, RequestId::Integer(7));
    assert_eq!(result, json!({}));

    let Some(ClientResponse::ThreadArchive {
        request_id,
        response: _,
    }) = payload.and_then(|payload| payload.into_client_response(RequestId::Integer(7)))
    else {
        panic!("expected thread/archive client response");
    };
    assert_eq!(request_id, RequestId::Integer(7));
    Ok(())
}

#[test]
fn interrupt_conversation_payload_stays_jsonrpc_only() -> Result<()> {
    let (request_id, result, payload) =
        ClientResponsePayload::InterruptConversation(v1::InterruptConversationResponse {
            abort_reason: TurnAbortReason::Interrupted,
        })
        .into_jsonrpc_parts_and_payload(RequestId::Integer(8))?;

    assert_eq!(request_id, RequestId::Integer(8));
    assert_eq!(
        result,
        json!({
            "abortReason": "interrupted",
        })
    );
    assert!(payload.is_none());
    Ok(())
}
