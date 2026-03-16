use std::collections::HashMap;
use std::env;
use std::sync::OnceLock;

use codex_protocol::protocol::W3cTraceContext;
use opentelemetry::Context;
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry::trace::TraceContextExt;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tracing::Span;
use tracing::debug;
use tracing::warn;
use tracing_opentelemetry::OpenTelemetrySpanExt;

const TRACEPARENT_ENV_VAR: &str = "TRACEPARENT";
const TRACESTATE_ENV_VAR: &str = "TRACESTATE";
static TRACEPARENT_CONTEXT: OnceLock<Option<Context>> = OnceLock::new();

pub fn current_span_w3c_trace_context() -> Option<W3cTraceContext> {
    span_w3c_trace_context(&Span::current())
}

pub fn span_w3c_trace_context(span: &Span) -> Option<W3cTraceContext> {
    let context = span.context();
    if !context.span().span_context().is_valid() {
        return None;
    }

    let mut headers = HashMap::new();
    TraceContextPropagator::new().inject_context(&context, &mut headers);

    Some(W3cTraceContext {
        traceparent: headers.remove("traceparent"),
        tracestate: headers.remove("tracestate"),
    })
}

pub fn current_span_trace_id() -> Option<String> {
    let context = Span::current().context();
    let span = context.span();
    let span_context = span.span_context();
    if !span_context.is_valid() {
        return None;
    }

    Some(span_context.trace_id().to_string())
}

pub fn context_from_w3c_trace_context(trace: &W3cTraceContext) -> Option<Context> {
    context_from_trace_headers(trace.traceparent.as_deref(), trace.tracestate.as_deref())
}

pub fn set_parent_from_w3c_trace_context(span: &Span, trace: &W3cTraceContext) -> bool {
    if let Some(context) = context_from_w3c_trace_context(trace) {
        set_parent_from_context(span, context);
        true
    } else {
        false
    }
}

pub fn set_parent_from_context(span: &Span, context: Context) {
    let _ = span.set_parent(context);
}

pub fn traceparent_context_from_env() -> Option<Context> {
    TRACEPARENT_CONTEXT
        .get_or_init(load_traceparent_context)
        .clone()
}

pub(crate) fn context_from_trace_headers(
    traceparent: Option<&str>,
    tracestate: Option<&str>,
) -> Option<Context> {
    let traceparent = traceparent?;
    let mut headers = HashMap::new();
    headers.insert("traceparent".to_string(), traceparent.to_string());
    if let Some(tracestate) = tracestate {
        headers.insert("tracestate".to_string(), tracestate.to_string());
    }

    let context = TraceContextPropagator::new().extract(&headers);
    if !context.span().span_context().is_valid() {
        return None;
    }
    Some(context)
}

fn load_traceparent_context() -> Option<Context> {
    let traceparent = env::var(TRACEPARENT_ENV_VAR).ok()?;
    let tracestate = env::var(TRACESTATE_ENV_VAR).ok();

    match context_from_trace_headers(Some(&traceparent), tracestate.as_deref()) {
        Some(context) => {
            debug!("TRACEPARENT detected; continuing trace from parent context");
            Some(context)
        }
        None => {
            warn!("TRACEPARENT is set but invalid; ignoring trace context");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::context_from_trace_headers;
    use super::context_from_w3c_trace_context;
    use super::current_span_trace_id;
    use codex_protocol::protocol::W3cTraceContext;
    use opentelemetry::trace::SpanId;
    use opentelemetry::trace::TraceContextExt;
    use opentelemetry::trace::TraceId;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use pretty_assertions::assert_eq;
    use tracing::trace_span;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    #[test]
    fn parses_valid_w3c_trace_context() {
        let trace_id = "00000000000000000000000000000001";
        let span_id = "0000000000000002";
        let context = context_from_w3c_trace_context(&W3cTraceContext {
            traceparent: Some(format!("00-{trace_id}-{span_id}-01")),
            tracestate: None,
        })
        .expect("trace context");

        let span = context.span();
        let span_context = span.span_context();
        assert_eq!(
            span_context.trace_id(),
            TraceId::from_hex(trace_id).unwrap()
        );
        assert_eq!(span_context.span_id(), SpanId::from_hex(span_id).unwrap());
        assert!(span_context.is_remote());
    }

    #[test]
    fn invalid_traceparent_returns_none() {
        assert!(context_from_trace_headers(Some("not-a-traceparent"), None).is_none());
    }

    #[test]
    fn missing_traceparent_returns_none() {
        assert!(
            context_from_w3c_trace_context(&W3cTraceContext {
                traceparent: None,
                tracestate: Some("vendor=value".to_string()),
            })
            .is_none()
        );
    }

    #[test]
    fn current_span_trace_id_returns_hex_trace_id() {
        let provider = SdkTracerProvider::builder().build();
        let tracer = provider.tracer("codex-otel-tests");
        let subscriber =
            tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));
        let _guard = subscriber.set_default();

        let span = trace_span!("test_span");
        let _entered = span.enter();
        let trace_id = current_span_trace_id().expect("trace id");

        assert_eq!(trace_id.len(), 32);
        assert!(trace_id.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_ne!(trace_id, "00000000000000000000000000000000");
    }
}
