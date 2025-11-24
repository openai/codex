//! # Tracer
//!
//! The OpenTelemetry library achieves in-process context propagation of
//! `Span`s by way of the `Tracer`.
//!
//! The `Tracer` is responsible for tracking the currently active `Span`,
//! and exposes methods for creating and activating new `Spans`.
//!
//! Docs: <https://github.com/open-telemetry/opentelemetry-specification/blob/v1.3.0/specification/trace/api.md#tracer>
use crate::trace::{
    provider::SdkTracerProvider,
    span::{Span, SpanData},
    IdGenerator, ShouldSample, SpanEvents, SpanLimits, SpanLinks,
};
use opentelemetry::{
    trace::{SamplingDecision, SpanBuilder, SpanContext, SpanKind, TraceContextExt, TraceFlags},
    Context, InstrumentationScope, KeyValue,
};
use std::fmt;

/// `Tracer` implementation to create and manage spans
#[derive(Clone)]
pub struct SdkTracer {
    scope: InstrumentationScope,
    provider: SdkTracerProvider,
}

impl fmt::Debug for SdkTracer {
    /// Formats the `Tracer` using the given formatter.
    /// Omitting `provider` here is necessary to avoid cycles.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tracer")
            .field("name", &self.scope.name())
            .field("version", &self.scope.version())
            .finish()
    }
}

impl SdkTracer {
    /// Create a new tracer (used internally by `TracerProvider`s).
    pub(crate) fn new(scope: InstrumentationScope, provider: SdkTracerProvider) -> Self {
        SdkTracer { scope, provider }
    }

    /// TracerProvider associated with this tracer.
    pub(crate) fn provider(&self) -> &SdkTracerProvider {
        &self.provider
    }

    /// Instrumentation scope of this tracer.
    pub(crate) fn instrumentation_scope(&self) -> &InstrumentationScope {
        &self.scope
    }

    fn build_recording_span(
        &self,
        psc: &SpanContext,
        sc: SpanContext,
        mut builder: SpanBuilder,
        attrs: Vec<KeyValue>,
        span_limits: SpanLimits,
    ) -> Span {
        let mut attribute_options = builder.attributes.take().unwrap_or_default();
        for extra_attr in attrs {
            attribute_options.push(extra_attr);
        }
        let span_attributes_limit = span_limits.max_attributes_per_span as usize;
        let dropped_attributes_count = attribute_options
            .len()
            .saturating_sub(span_attributes_limit);
        attribute_options.truncate(span_attributes_limit);
        let dropped_attributes_count = dropped_attributes_count as u32;

        // Links are available as Option<Vec<Link>> in the builder
        // If it is None, then there are no links to process.
        // In that case Span.Links will be default (empty Vec<Link>, 0 drop count)
        // Otherwise, truncate Vec<Link> to keep until limits and use that in Span.Links.
        // Store the count of excess links into Span.Links.dropped_count.
        // There is no ability today to add Links after Span creation,
        // but such a capability will be needed in the future
        // once the spec for that stabilizes.

        let spans_links_limit = span_limits.max_links_per_span as usize;
        let span_links: SpanLinks = if let Some(mut links) = builder.links.take() {
            let dropped_count = links.len().saturating_sub(spans_links_limit);
            links.truncate(spans_links_limit);
            let link_attributes_limit = span_limits.max_attributes_per_link as usize;
            for link in links.iter_mut() {
                let dropped_attributes_count =
                    link.attributes.len().saturating_sub(link_attributes_limit);
                link.attributes.truncate(link_attributes_limit);
                link.dropped_attributes_count = dropped_attributes_count as u32;
            }
            SpanLinks {
                links,
                dropped_count: dropped_count as u32,
            }
        } else {
            SpanLinks::default()
        };

        let SpanBuilder {
            name,
            start_time,
            end_time,
            events,
            status,
            ..
        } = builder;

        let start_time = start_time.unwrap_or_else(opentelemetry::time::now);
        let end_time = end_time.unwrap_or(start_time);
        let spans_events_limit = span_limits.max_events_per_span as usize;
        let span_events: SpanEvents = if let Some(mut events) = events {
            let dropped_count = events.len().saturating_sub(spans_events_limit);
            events.truncate(spans_events_limit);
            let event_attributes_limit = span_limits.max_attributes_per_event as usize;
            for event in events.iter_mut() {
                let dropped_attributes_count = event
                    .attributes
                    .len()
                    .saturating_sub(event_attributes_limit);
                event.attributes.truncate(event_attributes_limit);
                event.dropped_attributes_count = dropped_attributes_count as u32;
            }
            SpanEvents {
                events,
                dropped_count: dropped_count as u32,
            }
        } else {
            SpanEvents::default()
        };
        Span::new(
            sc,
            Some(SpanData {
                parent_span_id: psc.span_id(),
                span_kind: builder.span_kind.take().unwrap_or(SpanKind::Internal),
                name,
                start_time,
                end_time,
                attributes: attribute_options,
                dropped_attributes_count,
                events: span_events,
                links: span_links,
                status,
            }),
            self.clone(),
            span_limits,
        )
    }

    /// The [`IdGenerator`] associated with this tracer.
    ///
    // Note: this is necessary for tracing-opentelemetry's `PreSampledTracer`.
    #[doc(hidden)]
    pub fn id_generator(&self) -> &dyn IdGenerator {
        &*self.provider.config().id_generator
    }

    /// The [`ShouldSample`] associated with this tracer.
    ///
    // Note: this is necessary for tracing-opentelemetry's `PreSampledTracer`.
    #[doc(hidden)]
    pub fn should_sample(&self) -> &dyn ShouldSample {
        &*self.provider.config().sampler
    }
}

impl opentelemetry::trace::Tracer for SdkTracer {
    /// This implementation of `Tracer` produces `sdk::Span` instances.
    type Span = Span;

    /// Starts a span from a `SpanBuilder`.
    ///
    /// Each span has zero or one parent spans and zero or more child spans, which
    /// represent causally related operations. A tree of related spans comprises a
    /// trace. A span is said to be a _root span_ if it does not have a parent. Each
    /// trace includes a single root span, which is the shared ancestor of all other
    /// spans in the trace.
    fn build_with_context(&self, mut builder: SpanBuilder, parent_cx: &Context) -> Self::Span {
        if parent_cx.is_telemetry_suppressed() {
            return Span::new(
                SpanContext::empty_context(),
                None,
                self.clone(),
                SpanLimits::default(),
            );
        }

        let provider = self.provider();
        // no point start a span if the tracer provider has already being shutdown
        if provider.is_shutdown() {
            return Span::new(
                SpanContext::empty_context(),
                None,
                self.clone(),
                SpanLimits::default(),
            );
        }

        let config = provider.config();
        let span_id = builder
            .span_id
            .take()
            .unwrap_or_else(|| config.id_generator.new_span_id());
        let trace_id;
        let mut psc = &SpanContext::empty_context();

        let parent_span = if parent_cx.has_active_span() {
            Some(parent_cx.span())
        } else {
            None
        };

        // Build context for sampling decision
        if let Some(sc) = parent_span.as_ref().map(|parent| parent.span_context()) {
            trace_id = sc.trace_id();
            psc = sc;
        } else {
            trace_id = builder
                .trace_id
                .unwrap_or_else(|| config.id_generator.new_trace_id());
        };

        // In order to accommodate use cases like `tracing-opentelemetry` we there is the ability
        // to use pre-sampling. Otherwise, the standard method of sampling is followed.
        let samplings_result = if let Some(sr) = builder.sampling_result.take() {
            sr
        } else {
            config.sampler.should_sample(
                Some(parent_cx),
                trace_id,
                &builder.name,
                builder.span_kind.as_ref().unwrap_or(&SpanKind::Internal),
                builder.attributes.as_ref().unwrap_or(&Vec::new()),
                builder.links.as_deref().unwrap_or(&[]),
            )
        };

        let trace_flags = parent_cx.span().span_context().trace_flags();
        let trace_state = samplings_result.trace_state;
        let span_limits = config.span_limits;
        // Build optional inner context, `None` if not recording.
        let mut span = match samplings_result.decision {
            SamplingDecision::RecordAndSample => {
                let sc = SpanContext::new(
                    trace_id,
                    span_id,
                    trace_flags.with_sampled(true),
                    false,
                    trace_state,
                );
                self.build_recording_span(
                    psc,
                    sc,
                    builder,
                    samplings_result.attributes,
                    span_limits,
                )
            }
            SamplingDecision::RecordOnly => {
                let sc = SpanContext::new(
                    trace_id,
                    span_id,
                    trace_flags.with_sampled(false),
                    false,
                    trace_state,
                );
                self.build_recording_span(
                    psc,
                    sc,
                    builder,
                    samplings_result.attributes,
                    span_limits,
                )
            }
            SamplingDecision::Drop => {
                let span_context =
                    SpanContext::new(trace_id, span_id, TraceFlags::default(), false, trace_state);
                Span::new(span_context, None, self.clone(), span_limits)
            }
        };

        // Call `on_start` for all processors
        for processor in provider.span_processors() {
            processor.on_start(&mut span, parent_cx)
        }

        span
    }
}

#[cfg(all(test, feature = "testing", feature = "trace"))]
mod tests {
    use crate::{
        testing::trace::TestSpan,
        trace::{Sampler, ShouldSample},
    };
    use opentelemetry::{
        trace::{
            Link, SamplingDecision, SamplingResult, Span, SpanContext, SpanId, SpanKind,
            TraceContextExt, TraceFlags, TraceId, TraceState, Tracer, TracerProvider,
        },
        Context, KeyValue,
    };

    #[derive(Clone, Debug)]
    struct TestSampler {}

    impl ShouldSample for TestSampler {
        fn should_sample(
            &self,
            parent_context: Option<&Context>,
            _trace_id: TraceId,
            _name: &str,
            _span_kind: &SpanKind,
            _attributes: &[KeyValue],
            _links: &[Link],
        ) -> SamplingResult {
            let trace_state = parent_context
                .unwrap()
                .span()
                .span_context()
                .trace_state()
                .clone();
            SamplingResult {
                decision: SamplingDecision::RecordAndSample,
                attributes: Vec::new(),
                trace_state: trace_state.insert("foo", "notbar").unwrap(),
            }
        }
    }

    #[test]
    fn allow_sampler_to_change_trace_state() {
        // Setup
        let sampler = TestSampler {};
        let tracer_provider = crate::trace::SdkTracerProvider::builder()
            .with_sampler(sampler)
            .build();
        let tracer = tracer_provider.tracer("test");
        let trace_state = TraceState::from_key_value(vec![("foo", "bar")]).unwrap();

        let parent_context = Context::new().with_span(TestSpan(SpanContext::new(
            TraceId::from_u128(128),
            SpanId::from_u64(64),
            TraceFlags::SAMPLED,
            true,
            trace_state,
        )));

        // Test sampler should change trace state
        let span = tracer.start_with_context("foo", &parent_context);
        let span_context = span.span_context();
        let expected = span_context.trace_state();
        assert_eq!(expected.get("foo"), Some("notbar"))
    }

    #[test]
    fn drop_parent_based_children() {
        let sampler = Sampler::ParentBased(Box::new(Sampler::AlwaysOn));
        let tracer_provider = crate::trace::SdkTracerProvider::builder()
            .with_sampler(sampler)
            .build();

        let context = Context::current_with_span(TestSpan(SpanContext::empty_context()));
        let tracer = tracer_provider.tracer("test");
        let span = tracer.start_with_context("must_not_be_sampled", &context);

        assert!(!span.span_context().is_sampled());
    }

    #[test]
    fn uses_current_context_for_builders_if_unset() {
        let sampler = Sampler::ParentBased(Box::new(Sampler::AlwaysOn));
        let tracer_provider = crate::trace::SdkTracerProvider::builder()
            .with_sampler(sampler)
            .build();
        let tracer = tracer_provider.tracer("test");

        let _attached = Context::current_with_span(TestSpan(SpanContext::empty_context())).attach();
        let span = tracer.span_builder("must_not_be_sampled").start(&tracer);
        assert!(!span.span_context().is_sampled());

        let context = Context::map_current(|cx| {
            cx.with_remote_span_context(SpanContext::new(
                TraceId::from_u128(1),
                SpanId::from_u64(1),
                TraceFlags::default(),
                true,
                Default::default(),
            ))
        });
        let _attached = context.attach();
        let span = tracer.span_builder("must_not_be_sampled").start(&tracer);

        assert!(!span.span_context().is_sampled());
    }
}
