use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::prelude::*;

use super::current_trace_context_headers;

#[test]
fn creates_traceparent_header_from_current_span() {
    let provider = SdkTracerProvider::builder().build();
    let tracer = provider.tracer("exec-server-test");
    let subscriber =
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));
    let _guard = subscriber.set_default();
    tracing::callsite::rebuild_interest_cache();
    let span = tracing::info_span!("outbound-request");
    let _entered = span.enter();

    let headers = current_trace_context_headers();

    let traceparent = headers
        .get("traceparent")
        .expect("traceparent header")
        .to_str()
        .expect("valid traceparent header");
    assert!(traceparent.starts_with("00-"));
    assert_eq!(traceparent.len(), 55);
}
