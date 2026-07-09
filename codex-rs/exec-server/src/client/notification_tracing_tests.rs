use codex_exec_server_protocol::JSONRPCNotification;
use codex_protocol::protocol::W3cTraceContext;
use opentelemetry::trace::SpanId;
use opentelemetry::trace::TraceId;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::InMemorySpanExporter;
use opentelemetry_sdk::trace::SdkTracerProvider;
use pretty_assertions::assert_eq;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::prelude::*;

use super::notification_span;
use crate::protocol::EXEC_CLOSED_METHOD;
use crate::protocol::EXEC_EXITED_METHOD;
use crate::protocol::EXEC_OUTPUT_DELTA_METHOD;
use crate::protocol::HTTP_REQUEST_BODY_DELTA_METHOD;

#[test]
#[serial_test::serial(exec_server_tracing)]
fn receive_span_uses_inbound_trace_parent() {
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
    let notification = notification(
        EXEC_EXITED_METHOD,
        serde_json::json!({}),
        Some(W3cTraceContext {
            traceparent: Some(format!("00-{trace_id}-{parent_span_id}-01")),
            tracestate: None,
        }),
    );

    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        let span = notification_span(&notification);
        span.in_scope(|| {});
        drop(span);
    });

    tracer_provider.force_flush().expect("flush traces");
    let spans = span_exporter.get_finished_spans().expect("span export");
    let receive_span = spans
        .iter()
        .find(|span| span.name.as_ref() == EXEC_EXITED_METHOD)
        .expect("process exited receive span");
    assert_eq!(receive_span.span_context.trace_id(), trace_id);
    assert_eq!(receive_span.parent_span_id, parent_span_id);
}

#[test]
#[serial_test::serial(exec_server_tracing)]
fn receive_span_policy_avoids_hot_path_volume() {
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

    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        for _ in 0..100 {
            finish_span(notification_span(&notification(
                EXEC_OUTPUT_DELTA_METHOD,
                serde_json::json!({}),
                /*trace*/ None,
            )));
            finish_span(notification_span(&notification(
                HTTP_REQUEST_BODY_DELTA_METHOD,
                serde_json::json!({"done": false, "error": null}),
                /*trace*/ None,
            )));
        }

        for notification in [
            notification(
                EXEC_EXITED_METHOD,
                serde_json::json!({}),
                /*trace*/ None,
            ),
            notification(
                EXEC_CLOSED_METHOD,
                serde_json::json!({}),
                /*trace*/ None,
            ),
            notification(
                HTTP_REQUEST_BODY_DELTA_METHOD,
                serde_json::json!({"done": true, "error": null}),
                /*trace*/ None,
            ),
            notification(
                HTTP_REQUEST_BODY_DELTA_METHOD,
                serde_json::json!({"done": false, "error": "stream failed"}),
                /*trace*/ None,
            ),
        ] {
            finish_span(notification_span(&notification));
        }
    });

    tracer_provider.force_flush().expect("flush traces");
    let mut span_names = span_exporter
        .get_finished_spans()
        .expect("span export")
        .into_iter()
        .map(|span| span.name.into_owned())
        .collect::<Vec<_>>();
    span_names.sort();
    let mut expected = vec![
        EXEC_CLOSED_METHOD.to_string(),
        EXEC_EXITED_METHOD.to_string(),
        HTTP_REQUEST_BODY_DELTA_METHOD.to_string(),
        HTTP_REQUEST_BODY_DELTA_METHOD.to_string(),
    ];
    expected.sort();
    assert_eq!(span_names, expected);
}

fn notification(
    method: &str,
    params: serde_json::Value,
    trace: Option<W3cTraceContext>,
) -> JSONRPCNotification {
    JSONRPCNotification {
        method: method.to_string(),
        params: Some(params),
        trace,
    }
}

fn finish_span(span: tracing::Span) {
    span.in_scope(|| {});
    span.record("result", "success");
}
