use opentelemetry::{
    trace::{
        Link, SamplingDecision, SamplingResult, SpanKind, TraceContextExt, TraceId, TraceState,
    },
    Context, KeyValue,
};

#[cfg(feature = "jaeger_remote_sampler")]
mod jaeger_remote;

#[cfg(feature = "jaeger_remote_sampler")]
pub use jaeger_remote::{JaegerRemoteSampler, JaegerRemoteSamplerBuilder};
#[cfg(feature = "jaeger_remote_sampler")]
use opentelemetry_http::HttpClient;

/// The [`ShouldSample`] interface allows implementations to provide samplers
/// which will return a sampling [`SamplingResult`] based on information that
/// is typically available just before the [`Span`] was created.
///
/// # Sampling
///
/// Sampling is a mechanism to control the noise and overhead introduced by
/// OpenTelemetry by reducing the number of samples of traces collected and
/// sent to the backend.
///
/// Sampling may be implemented on different stages of a trace collection.
/// [OpenTelemetry SDK] defines a [`ShouldSample`] interface that can be used at
/// instrumentation points by libraries to check the sampling [`SamplingDecision`]
/// early and optimize the amount of telemetry that needs to be collected.
///
/// All other sampling algorithms may be implemented on SDK layer in exporters,
/// or even out of process in Agent or Collector.
///
/// The OpenTelemetry API has two properties responsible for the data collection:
///
/// * [`Span::is_recording()`]. If `true` the current [`Span`] records
///   tracing events (attributes, events, status, etc.), otherwise all tracing
///   events are dropped. Users can use this property to determine if expensive
///   trace events can be avoided. [`SpanProcessor`]s will receive
///   all spans with this flag set. However, [`SpanExporter`]s will
///   not receive them unless the `Sampled` flag was set.
/// * `Sampled` flag in [`SpanContext::trace_flags()`]. This flag is propagated
///   via the [`SpanContext`] to child Spans. For more details see the [W3C
///   specification](https://w3c.github.io/trace-context/). This flag indicates
///   that the [`Span`] has been `sampled` and will be exported. [`SpanProcessor`]s
///   and [`SpanExporter`]s will receive spans with the `Sampled` flag set for
///   processing.
///
/// The flag combination `Sampled == false` and `is_recording == true` means
/// that the current `Span` does record information, but most likely the child
/// `Span` will not.
///
/// The flag combination `Sampled == true` and `is_recording == false` could
/// cause gaps in the distributed trace, and because of this OpenTelemetry API
/// MUST NOT allow this combination.
///
/// [OpenTelemetry SDK]: https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/sdk.md#sampling
/// [`SpanContext`]: opentelemetry::trace::SpanContext
/// [`SpanContext::trace_flags()`]: opentelemetry::trace::SpanContext#method.trace_flags
/// [`SpanExporter`]: crate::trace::SpanExporter
/// [`SpanProcessor`]: crate::trace::SpanProcessor
/// [`Span`]: opentelemetry::trace::Span
/// [`Span::is_recording()`]: opentelemetry::trace::Span#tymethod.is_recording
pub trait ShouldSample: CloneShouldSample + Send + Sync + std::fmt::Debug {
    /// Returns the [`SamplingDecision`] for a [`Span`] to be created.
    ///
    /// The [`should_sample`] function can use any of the information provided to it in order to
    /// make a decision about whether or not a [`Span`] should or should not be sampled. However,
    /// there are performance implications on the creation of a span
    ///
    /// [`Span`]: opentelemetry::trace::Span
    /// [`should_sample`]: ShouldSample::should_sample
    #[allow(clippy::too_many_arguments)]
    fn should_sample(
        &self,
        parent_context: Option<&Context>,
        trace_id: TraceId,
        name: &str,
        span_kind: &SpanKind,
        attributes: &[KeyValue],
        links: &[Link],
    ) -> SamplingResult;
}

/// This trait should not be used directly instead users should use [`ShouldSample`].
pub trait CloneShouldSample {
    fn box_clone(&self) -> Box<dyn ShouldSample>;
}

impl<T> CloneShouldSample for T
where
    T: ShouldSample + Clone + 'static,
{
    fn box_clone(&self) -> Box<dyn ShouldSample> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn ShouldSample> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

/// Default Sampling options
///
/// The [built-in samplers] allow for simple decisions. For more complex scenarios consider
/// implementing your own sampler using [`ShouldSample`] trait.
///
/// [built-in samplers]: https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/sdk.md#built-in-samplers
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Sampler {
    /// Always sample the trace
    AlwaysOn,
    /// Never sample the trace
    AlwaysOff,
    /// Respects the parent span's sampling decision or delegates a delegate sampler for root spans.
    ParentBased(Box<dyn ShouldSample>),
    /// Sample a given fraction of traces. Fractions >= 1 will always sample. If the parent span is
    /// sampled, then it's child spans will automatically be sampled. Fractions < 0 are treated as
    /// zero, but spans may still be sampled if their parent is.
    /// *Note:* If this is used then all Spans in a trace will become sampled assuming that the
    /// first span is sampled as it is based on the `trace_id` not the `span_id`
    TraceIdRatioBased(f64),
    /// Jaeger remote sampler supports any remote service that implemented the jaeger remote sampler protocol.
    /// The proto definition can be found [here](https://github.com/jaegertracing/jaeger-idl/blob/main/proto/api_v2/sampling.proto)
    ///
    /// Jaeger remote sampler allows remotely controlling the sampling configuration for the SDKs.
    /// The sampling is typically configured at the collector and the SDKs actively poll for changes.
    /// The sampler uses TraceIdRatioBased or rate-limited sampler under the hood.
    /// These samplers can be configured per whole service (a.k.a default), or per span name in a
    /// given service (a.k.a per operation).
    #[cfg(feature = "jaeger_remote_sampler")]
    JaegerRemote(JaegerRemoteSampler),
}

impl Sampler {
    /// Create a jaeger remote sampler builder.
    ///
    /// ### Arguments
    /// * `runtime` - A runtime to run the HTTP client.
    /// * `http_client` - An HTTP client to query the sampling endpoint.
    /// * `default_sampler` - A default sampler to make a sampling decision when the remote is unavailable or before the SDK receives the first response from remote.
    /// * `service_name` - The name of the service. This is a required parameter to query the sampling endpoint.
    ///
    /// See [here](https://github.com/open-telemetry/opentelemetry-rust/blob/main/examples/jaeger-remote-sampler/src/main.rs) for an example.
    #[cfg(feature = "jaeger_remote_sampler")]
    pub fn jaeger_remote<C, Sampler, R, Svc>(
        runtime: R,
        http_client: C,
        default_sampler: Sampler,
        service_name: Svc,
    ) -> JaegerRemoteSamplerBuilder<C, Sampler, R>
    where
        C: HttpClient + 'static,
        Sampler: ShouldSample,
        R: crate::runtime::RuntimeChannel,
        Svc: Into<String>,
    {
        JaegerRemoteSamplerBuilder::new(runtime, http_client, default_sampler, service_name)
    }
}

impl ShouldSample for Sampler {
    fn should_sample(
        &self,
        parent_context: Option<&Context>,
        trace_id: TraceId,
        name: &str,
        span_kind: &SpanKind,
        attributes: &[KeyValue],
        links: &[Link],
    ) -> SamplingResult {
        let decision = match self {
            // Always sample the trace
            Sampler::AlwaysOn => SamplingDecision::RecordAndSample,
            // Never sample the trace
            Sampler::AlwaysOff => SamplingDecision::Drop,
            // The parent decision if sampled; otherwise the decision of delegate_sampler
            Sampler::ParentBased(delegate_sampler) => parent_context
                .filter(|cx| cx.has_active_span())
                .map_or_else(
                    || {
                        delegate_sampler
                            .should_sample(
                                parent_context,
                                trace_id,
                                name,
                                span_kind,
                                attributes,
                                links,
                            )
                            .decision
                    },
                    |ctx| {
                        let span = ctx.span();
                        let parent_span_context = span.span_context();
                        if parent_span_context.is_sampled() {
                            SamplingDecision::RecordAndSample
                        } else {
                            SamplingDecision::Drop
                        }
                    },
                ),
            // Probabilistically sample the trace.
            Sampler::TraceIdRatioBased(prob) => sample_based_on_probability(prob, trace_id),
            #[cfg(feature = "jaeger_remote_sampler")]
            Sampler::JaegerRemote(remote_sampler) => {
                remote_sampler
                    .should_sample(parent_context, trace_id, name, span_kind, attributes, links)
                    .decision
            }
        };
        SamplingResult {
            decision,
            // No extra attributes ever set by the SDK samplers.
            attributes: Vec::new(),
            // all sampler in SDK will not modify trace state.
            trace_state: match parent_context {
                Some(ctx) => ctx.span().span_context().trace_state().clone(),
                None => TraceState::default(),
            },
        }
    }
}

pub(crate) fn sample_based_on_probability(prob: &f64, trace_id: TraceId) -> SamplingDecision {
    if *prob >= 1.0 {
        SamplingDecision::RecordAndSample
    } else {
        let prob_upper_bound = (prob.max(0.0) * (1u64 << 63) as f64) as u64;
        // TODO: update behavior when the spec definition resolves
        // https://github.com/open-telemetry/opentelemetry-specification/issues/1413
        let bytes = trace_id.to_bytes();
        let (_, low) = bytes.split_at(8);
        let trace_id_low = u64::from_be_bytes(low.try_into().unwrap());
        let rnd_from_trace_id = trace_id_low >> 1;

        if rnd_from_trace_id < prob_upper_bound {
            SamplingDecision::RecordAndSample
        } else {
            SamplingDecision::Drop
        }
    }
}

#[cfg(all(test, feature = "testing", feature = "trace"))]
mod tests {
    use super::*;
    use crate::testing::trace::TestSpan;
    use opentelemetry::trace::{SpanContext, SpanId, TraceFlags};
    use rand::random;

    #[rustfmt::skip]
    fn sampler_data() -> Vec<(&'static str, Sampler, f64, bool, bool)> {
        vec![
            // Span w/o a parent
            ("never_sample", Sampler::AlwaysOff, 0.0, false, false),
            ("always_sample", Sampler::AlwaysOn, 1.0, false, false),
            ("ratio_-1", Sampler::TraceIdRatioBased(-1.0), 0.0, false, false),
            ("ratio_.25", Sampler::TraceIdRatioBased(0.25), 0.25, false, false),
            ("ratio_.50", Sampler::TraceIdRatioBased(0.50), 0.5, false, false),
            ("ratio_.75", Sampler::TraceIdRatioBased(0.75), 0.75, false, false),
            ("ratio_2.0", Sampler::TraceIdRatioBased(2.0), 1.0, false, false),

            // Spans w/o a parent delegate
            ("delegate_to_always_on", Sampler::ParentBased(Box::new(Sampler::AlwaysOn)), 1.0, false, false),
            ("delegate_to_always_off", Sampler::ParentBased(Box::new(Sampler::AlwaysOff)), 0.0, false, false),
            ("delegate_to_ratio_-1", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(-1.0))), 0.0, false, false),
            ("delegate_to_ratio_.25", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(0.25))), 0.25, false, false),
            ("delegate_to_ratio_.50", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(0.50))), 0.50, false, false),
            ("delegate_to_ratio_.75", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(0.75))), 0.75, false, false),
            ("delegate_to_ratio_2.0", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(2.0))), 1.0, false, false),

            // Spans with a parent that is *not* sampled act like spans w/o a parent
            ("unsampled_parent_with_ratio_-1", Sampler::TraceIdRatioBased(-1.0), 0.0, true, false),
            ("unsampled_parent_with_ratio_.25", Sampler::TraceIdRatioBased(0.25), 0.25, true, false),
            ("unsampled_parent_with_ratio_.50", Sampler::TraceIdRatioBased(0.50), 0.5, true, false),
            ("unsampled_parent_with_ratio_.75", Sampler::TraceIdRatioBased(0.75), 0.75, true, false),
            ("unsampled_parent_with_ratio_2.0", Sampler::TraceIdRatioBased(2.0), 1.0, true, false),
            ("unsampled_parent_or_else_with_always_on", Sampler::ParentBased(Box::new(Sampler::AlwaysOn)), 0.0, true, false),
            ("unsampled_parent_or_else_with_always_off", Sampler::ParentBased(Box::new(Sampler::AlwaysOff)), 0.0, true, false),
            ("unsampled_parent_or_else_with_ratio_.25", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(0.25))), 0.0, true, false),

            // A ratio sampler with a parent that is sampled will ignore the parent
            ("sampled_parent_with_ratio_-1", Sampler::TraceIdRatioBased(-1.0), 0.0, true, true),
            ("sampled_parent_with_ratio_.25", Sampler::TraceIdRatioBased(0.25), 0.25, true, true),
            ("sampled_parent_with_ratio_2.0", Sampler::TraceIdRatioBased(2.0), 1.0, true, true),

            // Spans with a parent that is sampled, will always sample, regardless of the delegate sampler
            ("sampled_parent_or_else_with_always_on", Sampler::ParentBased(Box::new(Sampler::AlwaysOn)), 1.0, true, true),
            ("sampled_parent_or_else_with_always_off", Sampler::ParentBased(Box::new(Sampler::AlwaysOff)), 1.0, true, true),
            ("sampled_parent_or_else_with_ratio_.25", Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(0.25))), 1.0, true, true),

            // Spans with a sampled parent, but when using the NeverSample Sampler, aren't sampled
            ("sampled_parent_span_with_never_sample", Sampler::AlwaysOff, 0.0, true, true),
        ]
    }

    #[test]
    fn sampling() {
        let total = 10_000;
        for (name, sampler, expectation, parent, sample_parent) in sampler_data() {
            let mut sampled = 0;
            for _ in 0..total {
                let parent_context = if parent {
                    let trace_flags = if sample_parent {
                        TraceFlags::SAMPLED
                    } else {
                        TraceFlags::default()
                    };
                    let span_context = SpanContext::new(
                        TraceId::from_u128(1),
                        SpanId::from_u64(1),
                        trace_flags,
                        false,
                        TraceState::default(),
                    );

                    Some(Context::current_with_span(TestSpan(span_context)))
                } else {
                    None
                };

                let trace_id = TraceId::from(random::<u128>());
                if sampler
                    .should_sample(
                        parent_context.as_ref(),
                        trace_id,
                        name,
                        &SpanKind::Internal,
                        &[],
                        &[],
                    )
                    .decision
                    == SamplingDecision::RecordAndSample
                {
                    sampled += 1;
                }
            }
            let mut tolerance = 0.0;
            let got = sampled as f64 / total as f64;

            if expectation > 0.0 && expectation < 1.0 {
                // See https://en.wikipedia.org/wiki/Binomial_proportion_confidence_interval
                let z = 4.75342; // This should succeed 99.9999% of the time
                tolerance = z * (got * (1.0 - got) / total as f64).sqrt();
            }

            let diff = (got - expectation).abs();
            assert!(
                diff <= tolerance,
                "{} got {:?} (diff: {}), expected {} (w/tolerance: {})",
                name,
                got,
                diff,
                expectation,
                tolerance
            );
        }
    }

    #[test]
    fn clone_a_parent_sampler() {
        let sampler = Sampler::ParentBased(Box::new(Sampler::AlwaysOn));
        #[allow(clippy::redundant_clone)]
        let cloned_sampler = sampler.clone();

        let cx = Context::current_with_value("some_value");

        let result = sampler.should_sample(
            Some(&cx),
            TraceId::from_u128(1),
            "should sample",
            &SpanKind::Internal,
            &[],
            &[],
        );

        let cloned_result = cloned_sampler.should_sample(
            Some(&cx),
            TraceId::from_u128(1),
            "should sample",
            &SpanKind::Internal,
            &[],
            &[],
        );

        assert_eq!(result, cloned_result);
    }

    #[test]
    fn parent_sampler() {
        // name, delegate, context(with or without parent), expected decision
        let test_cases = vec![
            (
                "should using delegate sampler",
                Sampler::AlwaysOn,
                Context::new(),
                SamplingDecision::RecordAndSample,
            ),
            (
                "should use parent result, always off",
                Sampler::AlwaysOn,
                Context::current_with_span(TestSpan(SpanContext::new(
                    TraceId::from_u128(1),
                    SpanId::from_u64(1),
                    TraceFlags::default(), // not sampling
                    false,
                    TraceState::default(),
                ))),
                SamplingDecision::Drop,
            ),
            (
                "should use parent result, always on",
                Sampler::AlwaysOff,
                Context::current_with_span(TestSpan(SpanContext::new(
                    TraceId::from_u128(1),
                    SpanId::from_u64(1),
                    TraceFlags::SAMPLED, // not sampling
                    false,
                    TraceState::default(),
                ))),
                SamplingDecision::RecordAndSample,
            ),
        ];

        for (name, delegate, parent_cx, expected) in test_cases {
            let sampler = Sampler::ParentBased(Box::new(delegate));
            let result = sampler.should_sample(
                Some(&parent_cx),
                TraceId::from_u128(1),
                name,
                &SpanKind::Internal,
                &[],
                &[],
            );

            assert_eq!(result.decision, expected);
        }
    }
}
