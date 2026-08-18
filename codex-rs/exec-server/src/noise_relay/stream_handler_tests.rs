use std::time::Instant;

use anyhow::Result;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::RequestId;
use pretty_assertions::assert_eq;

use super::NoiseStreamHandler;
use crate::connection::JsonRpcConnectionEvent;
use crate::server::ConnectionProcessor;

#[test]
fn local_decode_starts_request_span_before_queueing() -> Result<()> {
    let _subscriber = tracing::subscriber::set_default(tracing_subscriber::registry());
    tracing::callsite::rebuild_interest_cache();

    let request = JSONRPCRequest {
        id: RequestId::Integer(1),
        method: "test/queued".to_string(),
        params: None,
        trace: None,
    };
    let before_decode = Instant::now();
    let event = <ConnectionProcessor as NoiseStreamHandler>::decode(
        serde_json::to_vec(&JSONRPCMessage::Request(request.clone()))?.into(),
    )?;
    let after_decode = Instant::now();
    let JsonRpcConnectionEvent::QueuedRequest {
        request: actual_request,
        request_span,
        queued_at,
    } = event
    else {
        panic!("local ingress must create the queued request synchronously");
    };
    assert_eq!(actual_request, request);
    assert!(!request_span.is_disabled());
    assert!((before_decode..=after_decode).contains(&queued_at));
    Ok(())
}
