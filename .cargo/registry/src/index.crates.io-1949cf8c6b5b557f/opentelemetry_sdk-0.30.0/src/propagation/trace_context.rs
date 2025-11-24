//! # W3C Trace Context Propagator
//!

use opentelemetry::{
    propagation::{text_map_propagator::FieldIter, Extractor, Injector, TextMapPropagator},
    trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState},
    Context,
};
use std::str::FromStr;
use std::sync::OnceLock;

const SUPPORTED_VERSION: u8 = 0;
const MAX_VERSION: u8 = 254;
const TRACEPARENT_HEADER: &str = "traceparent";
const TRACESTATE_HEADER: &str = "tracestate";

// TODO Replace this with LazyLock once it is stable.
static TRACE_CONTEXT_HEADER_FIELDS: OnceLock<[String; 2]> = OnceLock::new();

fn trace_context_header_fields() -> &'static [String; 2] {
    TRACE_CONTEXT_HEADER_FIELDS
        .get_or_init(|| [TRACEPARENT_HEADER.to_owned(), TRACESTATE_HEADER.to_owned()])
}

/// Propagates `SpanContext`s in [W3C TraceContext] format under `traceparent` and `tracestate` header.
///
/// The `traceparent` header represents the incoming request in a
/// tracing system in a common format, understood by all vendors.
/// Here’s an example of a `traceparent` header.
///
/// `traceparent: 00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01`
///
/// The `traceparent` HTTP header field identifies the incoming request in a
/// tracing system. It has four fields:
///
///    - version
///    - trace-id
///    - parent-id
///    - trace-flags
///
/// The `tracestate` header provides additional vendor-specific trace
/// identification information across different distributed tracing systems.
/// Here's an example of a `tracestate` header
///
/// `tracestate: vendorname1=opaqueValue1,vendorname2=opaqueValue2`
///
/// See the [w3c trace-context docs] for more details.
///
/// [w3c trace-context docs]: https://w3c.github.io/trace-context/
/// [W3C TraceContext]: https://www.w3.org/TR/trace-context/
#[derive(Clone, Debug, Default)]
pub struct TraceContextPropagator {
    _private: (),
}

impl TraceContextPropagator {
    /// Create a new `TraceContextPropagator`.
    pub fn new() -> Self {
        TraceContextPropagator { _private: () }
    }

    /// Extract span context from w3c trace-context header.
    fn extract_span_context(&self, extractor: &dyn Extractor) -> Result<SpanContext, ()> {
        let header_value = extractor.get(TRACEPARENT_HEADER).unwrap_or("").trim();
        let parts = header_value.split_terminator('-').collect::<Vec<&str>>();
        // Ensure parts are not out of range.
        if parts.len() < 4 {
            return Err(());
        }

        // Ensure version is within range, for version 0 there must be 4 parts.
        let version = u8::from_str_radix(parts[0], 16).map_err(|_| ())?;
        if version > MAX_VERSION || version == 0 && parts.len() != 4 {
            return Err(());
        }

        // Ensure trace id is lowercase
        if parts[1].chars().any(|c| c.is_ascii_uppercase()) {
            return Err(());
        }

        // Parse trace id section
        let trace_id = TraceId::from_hex(parts[1]).map_err(|_| ())?;

        // Ensure span id is lowercase
        if parts[2].chars().any(|c| c.is_ascii_uppercase()) {
            return Err(());
        }

        // Parse span id section
        let span_id = SpanId::from_hex(parts[2]).map_err(|_| ())?;

        // Parse trace flags section
        let opts = u8::from_str_radix(parts[3], 16).map_err(|_| ())?;

        // Ensure opts are valid for version 0
        if version == 0 && opts > 2 {
            return Err(());
        }

        // Build trace flags clearing all flags other than the trace-context
        // supported sampling bit.
        let trace_flags = TraceFlags::new(opts) & TraceFlags::SAMPLED;

        let trace_state = match extractor.get(TRACESTATE_HEADER) {
            Some(trace_state_str) => {
                TraceState::from_str(trace_state_str).unwrap_or_else(|_| TraceState::default())
            }
            None => TraceState::default(),
        };

        // create context
        let span_context = SpanContext::new(trace_id, span_id, trace_flags, true, trace_state);

        // Ensure span is valid
        if !span_context.is_valid() {
            return Err(());
        }

        Ok(span_context)
    }
}

impl TextMapPropagator for TraceContextPropagator {
    /// Properly encodes the values of the `SpanContext` and injects them
    /// into the `Injector`.
    fn inject_context(&self, cx: &Context, injector: &mut dyn Injector) {
        let span = cx.span();
        let span_context = span.span_context();
        if span_context.is_valid() {
            let header_value = format!(
                "{:02x}-{}-{}-{:02x}",
                SUPPORTED_VERSION,
                span_context.trace_id(),
                span_context.span_id(),
                span_context.trace_flags() & TraceFlags::SAMPLED
            );
            injector.set(TRACEPARENT_HEADER, header_value);
            injector.set(TRACESTATE_HEADER, span_context.trace_state().header());
        }
    }

    /// Retrieves encoded `SpanContext`s using the `Extractor`. It decodes
    /// the `SpanContext` and returns it. If no `SpanContext` was retrieved
    /// OR if the retrieved SpanContext is invalid then an empty `SpanContext`
    /// is returned.
    fn extract_with_context(&self, cx: &Context, extractor: &dyn Extractor) -> Context {
        self.extract_span_context(extractor)
            .map(|sc| cx.with_remote_span_context(sc))
            .unwrap_or_else(|_| cx.clone())
    }

    fn fields(&self) -> FieldIter<'_> {
        FieldIter::new(trace_context_header_fields())
    }
}

#[cfg(all(test, feature = "testing", feature = "trace"))]
mod tests {
    use super::*;
    use crate::testing::trace::TestSpan;
    use std::collections::HashMap;

    #[rustfmt::skip]
    fn extract_data() -> Vec<(&'static str, &'static str, SpanContext)> {
        vec![
            ("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00", "foo=bar", SpanContext::new(TraceId::from_u128(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736), SpanId::from_u64(0x00f0_67aa_0ba9_02b7), TraceFlags::default(), true, TraceState::from_str("foo=bar").unwrap())),
            ("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01", "foo=bar", SpanContext::new(TraceId::from_u128(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736), SpanId::from_u64(0x00f0_67aa_0ba9_02b7), TraceFlags::SAMPLED, true, TraceState::from_str("foo=bar").unwrap())),
            ("02-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01", "foo=bar", SpanContext::new(TraceId::from_u128(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736), SpanId::from_u64(0x00f0_67aa_0ba9_02b7), TraceFlags::SAMPLED, true, TraceState::from_str("foo=bar").unwrap())),
            ("02-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-09", "foo=bar", SpanContext::new(TraceId::from_u128(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736), SpanId::from_u64(0x00f0_67aa_0ba9_02b7), TraceFlags::SAMPLED, true, TraceState::from_str("foo=bar").unwrap())),
            ("02-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-08", "foo=bar", SpanContext::new(TraceId::from_u128(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736), SpanId::from_u64(0x00f0_67aa_0ba9_02b7), TraceFlags::default(), true, TraceState::from_str("foo=bar").unwrap())),
            ("02-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-09-XYZxsf09", "foo=bar", SpanContext::new(TraceId::from_u128(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736), SpanId::from_u64(0x00f0_67aa_0ba9_02b7), TraceFlags::SAMPLED, true, TraceState::from_str("foo=bar").unwrap())),
            ("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-", "foo=bar", SpanContext::new(TraceId::from_u128(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736), SpanId::from_u64(0x00f0_67aa_0ba9_02b7), TraceFlags::SAMPLED, true, TraceState::from_str("foo=bar").unwrap())),
            ("01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-09-", "foo=bar", SpanContext::new(TraceId::from_u128(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736), SpanId::from_u64(0x00f0_67aa_0ba9_02b7), TraceFlags::SAMPLED, true, TraceState::from_str("foo=bar").unwrap())),
        ]
    }

    #[rustfmt::skip]
    fn extract_data_invalid() -> Vec<(&'static str, &'static str)> {
        vec![
            ("0000-00000000000000000000000000000000-0000000000000000-01", "wrong version length"),
            ("00-ab00000000000000000000000000000000-cd00000000000000-01", "wrong trace ID length"),
            ("00-ab000000000000000000000000000000-cd0000000000000000-01", "wrong span ID length"),
            ("00-ab000000000000000000000000000000-cd00000000000000-0100", "wrong trace flag length"),
            ("qw-00000000000000000000000000000000-0000000000000000-01",   "bogus version"),
            ("00-qw000000000000000000000000000000-cd00000000000000-01",   "bogus trace ID"),
            ("00-ab000000000000000000000000000000-qw00000000000000-01",   "bogus span ID"),
            ("00-ab000000000000000000000000000000-cd00000000000000-qw",   "bogus trace flag"),
            ("A0-00000000000000000000000000000000-0000000000000000-01",   "upper case version"),
            ("00-AB000000000000000000000000000000-cd00000000000000-01",   "upper case trace ID"),
            ("00-ab000000000000000000000000000000-CD00000000000000-01",   "upper case span ID"),
            ("00-ab000000000000000000000000000000-cd00000000000000-A1",   "upper case trace flag"),
            ("00-00000000000000000000000000000000-0000000000000000-01",   "zero trace ID and span ID"),
            ("00-ab000000000000000000000000000000-cd00000000000000-09",   "trace-flag unused bits set"),
            ("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7",      "missing options"),
            ("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-",     "empty options"),
        ]
    }

    #[rustfmt::skip]
    fn inject_data() -> Vec<(&'static str, &'static str, SpanContext)> {
        vec![
            ("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01", "foo=bar", SpanContext::new(TraceId::from_u128(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736), SpanId::from_u64(0x00f0_67aa_0ba9_02b7), TraceFlags::SAMPLED, true, TraceState::from_str("foo=bar").unwrap())),
            ("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00", "foo=bar", SpanContext::new(TraceId::from_u128(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736), SpanId::from_u64(0x00f0_67aa_0ba9_02b7), TraceFlags::default(), true, TraceState::from_str("foo=bar").unwrap())),
            ("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01", "foo=bar", SpanContext::new(TraceId::from_u128(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736), SpanId::from_u64(0x00f0_67aa_0ba9_02b7), TraceFlags::new(0xff), true, TraceState::from_str("foo=bar").unwrap())),
            ("", "", SpanContext::empty_context()),
        ]
    }

    #[test]
    fn extract_w3c() {
        let propagator = TraceContextPropagator::new();

        for (trace_parent, trace_state, expected_context) in extract_data() {
            let mut extractor = HashMap::new();
            extractor.insert(TRACEPARENT_HEADER.to_string(), trace_parent.to_string());
            extractor.insert(TRACESTATE_HEADER.to_string(), trace_state.to_string());

            assert_eq!(
                propagator.extract(&extractor).span().span_context(),
                &expected_context
            )
        }
    }

    #[test]
    fn extract_w3c_tracestate() {
        let propagator = TraceContextPropagator::new();
        let state = "foo=bar".to_string();
        let parent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00".to_string();

        let mut extractor = HashMap::new();
        extractor.insert(TRACEPARENT_HEADER.to_string(), parent);
        extractor.insert(TRACESTATE_HEADER.to_string(), state.clone());

        assert_eq!(
            propagator
                .extract(&extractor)
                .span()
                .span_context()
                .trace_state()
                .header(),
            state
        )
    }

    #[test]
    fn extract_w3c_reject_invalid() {
        let propagator = TraceContextPropagator::new();

        for (invalid_header, reason) in extract_data_invalid() {
            let mut extractor = HashMap::new();
            extractor.insert(TRACEPARENT_HEADER.to_string(), invalid_header.to_string());

            assert_eq!(
                propagator.extract(&extractor).span().span_context(),
                &SpanContext::empty_context(),
                "{}",
                reason
            )
        }
    }

    #[test]
    fn inject_w3c() {
        let propagator = TraceContextPropagator::new();

        for (expected_trace_parent, expected_trace_state, context) in inject_data() {
            let mut injector = HashMap::new();
            propagator.inject_context(
                &Context::current_with_span(TestSpan(context)),
                &mut injector,
            );

            assert_eq!(
                Extractor::get(&injector, TRACEPARENT_HEADER).unwrap_or(""),
                expected_trace_parent
            );

            assert_eq!(
                Extractor::get(&injector, TRACESTATE_HEADER).unwrap_or(""),
                expected_trace_state
            );
        }
    }

    #[test]
    fn inject_w3c_tracestate() {
        let propagator = TraceContextPropagator::new();
        let state = "foo=bar";

        let mut injector: HashMap<String, String> = HashMap::new();
        injector.set(TRACESTATE_HEADER, state.to_string());

        Context::map_current(|cx| propagator.inject_context(cx, &mut injector));

        assert_eq!(Extractor::get(&injector, TRACESTATE_HEADER), Some(state))
    }
}
