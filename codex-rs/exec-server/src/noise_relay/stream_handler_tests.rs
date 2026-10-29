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
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::InMemorySpanExporter;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use tracing_subscriber::prelude::*;
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let _subscriber =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(
            tracing_opentelemetry::layer().with_tracer(provider.tracer("noise-receipt-test")),
        ));
    tracing::callsite::rebuild_interest_cache();

    let request = JSONRPCRequest {
        id: RequestId::Integer(1),
        method: "test/queued".to_string(),
        params: Some(serde_json::json!({"secret": "SYNTHETIC_PRIVATE_PAYLOAD"})),
        trace: Some(codex_protocol::protocol::W3cTraceContext {
            traceparent: Some("00-00000000000000000000000000000001-0000000000000002-01".to_owned()),
            tracestate: None,
        }),
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
    let spans = exporter.get_finished_spans()?;
    assert_eq!(
        spans.len(),
        1,
        "receipt must export while the request is still queued"
    );
    assert_eq!(spans[0].name, "codex.exec_server.noise_request_received");
    assert_eq!(
        spans[0].span_context.trace_id().to_string(),
        "00000000000000000000000000000001"
    );
    assert!(!format!("{spans:?}").contains("SYNTHETIC_PRIVATE_PAYLOAD"));
    assert!(!format!("{spans:?}").contains("test/queued"));
    let receipt_parent = spans[0].parent_span_id;
    drop(request_span);
    let spans = exporter.get_finished_spans()?;
    assert_eq!(spans.len(), 2);
    assert_eq!(receipt_parent, spans[1].span_context.span_id());
    Ok(())
}
