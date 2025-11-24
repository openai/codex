//! # Composite Propagator
//!
//! A utility over multiple `Propagator`s to group multiple Propagators from different cross-cutting
//! concerns in order to leverage them as a single entity.
//!
//! Each composite Propagator will implement a specific Propagator type, such as TextMapPropagator,
//! as different Propagator types will likely operate on different data types.
use crate::{
    propagation::{text_map_propagator::FieldIter, Extractor, Injector, TextMapPropagator},
    Context,
};
use std::collections::HashSet;

/// Composite propagator for [`TextMapPropagator`]s.
///
/// A propagator that chains multiple [`TextMapPropagator`] propagators together,
/// injecting or extracting by their respective HTTP header names.
///
/// Injection and extraction from this propagator will preserve the order of the
/// injectors and extractors passed in during initialization.
///
/// # Examples
///
/// ```
/// use opentelemetry::{
///     baggage::BaggageExt,
///     propagation::{TextMapPropagator, TextMapCompositePropagator},
///
///     trace::{TraceContextExt, Tracer, TracerProvider},
///     Context, KeyValue,
/// };
/// use opentelemetry_sdk::propagation::{
///     BaggagePropagator, TraceContextPropagator,
/// };
/// use opentelemetry_sdk::trace as sdktrace;
/// use std::collections::HashMap;
///
/// // First create 1 or more propagators
/// let baggage_propagator = BaggagePropagator::new();
/// let trace_context_propagator = TraceContextPropagator::new();
///
/// // Then create a composite propagator
/// let composite_propagator = TextMapCompositePropagator::new(vec![
///     Box::new(baggage_propagator),
///     Box::new(trace_context_propagator),
/// ]);
///
/// // Then for a given implementation of `Injector`
/// let mut injector = HashMap::new();
///
/// // And a given span
/// let example_span = sdktrace::SdkTracerProvider::default()
///     .tracer("example-component")
///     .start("span-name");
///
/// // with the current context, call inject to add the headers
/// composite_propagator.inject_context(
///     &Context::current_with_span(example_span)
///         .with_baggage(vec![KeyValue::new("test", "example")]),
///     &mut injector,
/// );
///
/// // The injector now has both `baggage` and `traceparent` headers
/// assert!(injector.get("baggage").is_some());
/// assert!(injector.get("traceparent").is_some());
/// ```
#[derive(Debug)]
pub struct TextMapCompositePropagator {
    propagators: Vec<Box<dyn TextMapPropagator + Send + Sync>>,
    fields: Vec<String>,
}

impl TextMapCompositePropagator {
    /// Constructs a new propagator out of instances of [`TextMapPropagator`].
    ///
    /// [`TextMapPropagator`]: TextMapPropagator
    pub fn new(propagators: Vec<Box<dyn TextMapPropagator + Send + Sync>>) -> Self {
        let mut fields = HashSet::new();
        for propagator in &propagators {
            fields.extend(propagator.fields().map(ToString::to_string));
        }

        TextMapCompositePropagator {
            propagators,
            fields: fields.into_iter().collect(),
        }
    }
}

impl TextMapPropagator for TextMapCompositePropagator {
    /// Encodes the values of the `Context` and injects them into the `Injector`.
    fn inject_context(&self, context: &Context, injector: &mut dyn Injector) {
        for propagator in &self.propagators {
            propagator.inject_context(context, injector)
        }
    }

    /// Retrieves encoded `Context` information using the `Extractor`. If no data was
    /// retrieved OR if the retrieved data is invalid, then the current `Context` is
    /// returned.
    fn extract_with_context(&self, cx: &Context, extractor: &dyn Extractor) -> Context {
        self.propagators
            .iter()
            .fold(cx.clone(), |current_cx, propagator| {
                propagator.extract_with_context(&current_cx, extractor)
            })
    }

    fn fields(&self) -> FieldIter<'_> {
        FieldIter::new(self.fields.as_slice())
    }
}

#[cfg(all(test, feature = "trace"))]
mod tests {
    use crate::baggage::BaggageExt;
    use crate::propagation::TextMapCompositePropagator;
    use crate::testing::trace::TestSpan;
    use crate::{
        propagation::{text_map_propagator::FieldIter, Extractor, Injector, TextMapPropagator},
        trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState},
        Context, KeyValue,
    };
    use std::collections::HashMap;

    /// A test propagator that injects and extracts a single header.
    #[derive(Debug)]
    struct TestPropagator {
        header: &'static str,
        fields: Vec<String>, // used by fields method
    }

    impl TestPropagator {
        #[allow(unreachable_pub)]
        pub fn new(header: &'static str) -> Self {
            TestPropagator {
                header,
                fields: vec![header.to_string()],
            }
        }
    }

    impl TextMapPropagator for TestPropagator {
        fn inject_context(&self, cx: &Context, injector: &mut dyn Injector) {
            let span = cx.span();
            let span_context = span.span_context();
            match self.header {
                "span-id" => injector.set(self.header, format!("{:x}", span_context.span_id())),
                "baggage" => injector.set(self.header, cx.baggage().to_string()),
                _ => {}
            }
        }

        fn extract_with_context(&self, cx: &Context, extractor: &dyn Extractor) -> Context {
            match (self.header, extractor.get(self.header)) {
                ("span-id", Some(val)) => cx.with_remote_span_context(SpanContext::new(
                    TraceId::from_u128(1),
                    SpanId::from_u64(u64::from_str_radix(val, 16).unwrap()),
                    TraceFlags::default(),
                    false,
                    TraceState::default(),
                )),
                ("baggage", Some(_)) => cx.with_baggage(vec![KeyValue::new("baggagekey", "value")]),
                _ => cx.clone(),
            }
        }

        fn fields(&self) -> FieldIter<'_> {
            FieldIter::new(self.fields.as_slice())
        }
    }

    fn setup() -> Context {
        let mut cx = Context::default();
        cx = cx.with_span(TestSpan(SpanContext::new(
            TraceId::from_u128(1),
            SpanId::from_u64(11),
            TraceFlags::default(),
            true,
            TraceState::default(),
        )));
        // setup for baggage propagator
        cx.with_baggage(vec![KeyValue::new("baggagekey", "value")])
    }

    fn test_data() -> Vec<(&'static str, &'static str)> {
        vec![("span-id", "b"), ("baggage", "baggagekey=value")]
    }

    #[test]
    fn zero_propogators_are_noop() {
        // setup
        let composite_propagator = TextMapCompositePropagator::new(vec![]);
        let cx = setup();

        let mut injector = HashMap::new();
        composite_propagator.inject_context(&cx, &mut injector);

        assert_eq!(injector.len(), 0);
        for (header_name, header_value) in test_data() {
            let mut extractor = HashMap::new();
            extractor.insert(header_name.to_string(), header_value.to_string());
            assert_eq!(
                composite_propagator
                    .extract(&extractor)
                    .span()
                    .span_context(),
                &SpanContext::empty_context()
            );
        }
    }

    #[test]
    fn inject_multiple_propagators() {
        let composite_propagator = TextMapCompositePropagator::new(vec![
            Box::new(TestPropagator::new("span-id")),
            Box::new(TestPropagator::new("baggage")),
        ]);

        let cx = setup();
        let mut injector = HashMap::new();
        composite_propagator.inject_context(&cx, &mut injector);

        for (header_name, header_value) in test_data() {
            assert_eq!(injector.get(header_name), Some(&header_value.to_string()));
        }
    }

    #[test]
    fn extract_multiple_propagators() {
        let composite_propagator = TextMapCompositePropagator::new(vec![
            Box::new(TestPropagator::new("span-id")),
            Box::new(TestPropagator::new("baggage")),
        ]);

        let mut extractor = HashMap::new();
        for (header_name, header_value) in test_data() {
            extractor.insert(header_name.to_string(), header_value.to_string());
        }
        let cx = composite_propagator.extract(&extractor);
        assert_eq!(
            cx.span().span_context(),
            &SpanContext::new(
                TraceId::from_u128(1),
                SpanId::from_u64(11),
                TraceFlags::default(),
                false,
                TraceState::default(),
            )
        );
        assert_eq!(cx.baggage().to_string(), "baggagekey=value",);
    }

    #[test]
    fn test_get_fields() {
        let test_cases = vec![
            // name, header_name, expected_result
            // ("single propagator", vec!["span-id"], vec!["span-id"]),
            (
                "multiple propagators with order",
                vec!["span-id", "baggage"],
                vec!["baggage", "span-id"],
            ),
        ];

        for test_case in test_cases {
            let test_propagators = test_case
                .1
                .into_iter()
                .map(|name| {
                    Box::new(TestPropagator::new(name)) as Box<dyn TextMapPropagator + Send + Sync>
                })
                .collect();

            let composite_propagator = TextMapCompositePropagator::new(test_propagators);

            let mut fields = composite_propagator
                .fields()
                .map(|s| s.to_string())
                .collect::<Vec<String>>();
            fields.sort();

            assert_eq!(fields, test_case.2);
        }
    }
}
