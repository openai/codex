use http::HeaderMap;
use http::HeaderValue;

pub(crate) fn current_trace_context_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    let Some(trace) = codex_otel::current_span_w3c_trace_context() else {
        return headers;
    };
    if let Some(traceparent) = trace.traceparent
        && let Ok(value) = HeaderValue::try_from(traceparent)
    {
        headers.insert("traceparent", value);
    }
    if let Some(tracestate) = trace.tracestate
        && let Ok(value) = HeaderValue::try_from(tracestate)
    {
        headers.insert("tracestate", value);
    }
    headers
}

#[cfg(test)]
#[path = "trace_context_tests.rs"]
mod tests;
