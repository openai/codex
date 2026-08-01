use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::RequestId;
use opentelemetry::trace::SpanId;
use opentelemetry::trace::TraceId;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::InMemorySpanExporter;
use opentelemetry_sdk::trace::SdkTracerProvider;
use pretty_assertions::assert_eq;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::prelude::*;

use super::request_span;

/// Request spans retain the wire method and inbound trace while bounding their exported names.
#[test]
fn request_span_uses_bounded_name_wire_method_and_inbound_trace_parent() {
    let span_exporter = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter.clone())
        .build();
    let tracer = tracer_provider.tracer("exec-server-test");
    let subscriber = tracing_subscriber::registry().with(
        tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(filter_fn(codex_otel::OtelProvider::trace_export_filter)),
    );
    let trace_id = TraceId::from_hex("00000000000000000000000000000001").expect("trace id");
    let parent_span_id = SpanId::from_hex("0000000000000002").expect("span id");
    let trace = codex_protocol::protocol::W3cTraceContext {
        traceparent: Some(format!("00-{trace_id}-{parent_span_id}-01")),
        tracestate: None,
    };

    let method = "custom/method";
    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        let request = JSONRPCRequest {
            id: RequestId::Integer(1),
            method: method.to_string(),
            params: None,
            trace: Some(trace),
        };
        let request_span = request_span("unknown", &request);
        request_span.in_scope(|| {});
        drop(request_span);
    });

    tracer_provider.force_flush().expect("flush traces");
    let spans = span_exporter.get_finished_spans().expect("span export");
    let request_span = spans
        .iter()
        .find(|span| span.name.as_ref() == "unknown")
        .expect("unknown method span");
    assert_eq!(
        request_span
            .attributes
            .iter()
            .find(|attribute| attribute.key.as_str() == "method")
            .map(|attribute| attribute.value.clone()),
        Some(opentelemetry::Value::String(method.into()))
    );
    assert_eq!(request_span.span_context.trace_id(), trace_id);
    assert_eq!(request_span.parent_span_id, parent_span_id);
}
