//! # OpenTelemetry Trace SDK
//!
//! The tracing SDK consist of a few main structs:
//!
//! * The [`SdkTracer`] struct which performs all tracing operations.
//! * The [`Span`] struct with is a mutable object storing information about the
//!   current operation execution.
//! * The [`SdkTracerProvider`] struct which configures and produces [`SdkTracer`]s.
mod config;
mod error;
mod events;
mod export;
mod id_generator;
mod links;
mod provider;
mod sampler;
mod span;
mod span_limit;
mod span_processor;
#[cfg(feature = "experimental_trace_batch_span_processor_with_async_runtime")]
/// Experimental feature to use async runtime with batch span processor.
pub mod span_processor_with_async_runtime;
mod tracer;

pub use config::Config;
pub use error::{TraceError, TraceResult};
pub use events::SpanEvents;
pub use export::{SpanData, SpanExporter};

/// In-Memory span exporter for testing purpose.
#[cfg(any(feature = "testing", test))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "testing", test))))]
pub mod in_memory_exporter;
#[cfg(any(feature = "testing", test))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "testing", test))))]
pub use in_memory_exporter::{InMemorySpanExporter, InMemorySpanExporterBuilder};

pub use id_generator::{IdGenerator, RandomIdGenerator};
pub use links::SpanLinks;
pub use provider::{SdkTracerProvider, TracerProviderBuilder};
pub use sampler::{Sampler, ShouldSample};
pub use span::Span;
pub use span_limit::SpanLimits;
pub use span_processor::{
    BatchConfig, BatchConfigBuilder, BatchSpanProcessor, BatchSpanProcessorBuilder,
    SimpleSpanProcessor, SpanProcessor,
};

pub use tracer::SdkTracer;
pub use tracer::SdkTracer as Tracer; // for back-compat else tracing-opentelemetry won't build

#[cfg(feature = "jaeger_remote_sampler")]
pub use sampler::{JaegerRemoteSampler, JaegerRemoteSamplerBuilder};

#[cfg(feature = "experimental_trace_batch_span_processor_with_async_runtime")]
#[cfg(test)]
mod runtime_tests;

#[cfg(all(test, feature = "testing"))]
mod tests {
    use super::*;
    use crate::error::OTelSdkResult;
    use crate::{
        trace::span_limit::{DEFAULT_MAX_EVENT_PER_SPAN, DEFAULT_MAX_LINKS_PER_SPAN},
        trace::{InMemorySpanExporter, InMemorySpanExporterBuilder},
    };
    use opentelemetry::{
        baggage::BaggageExt,
        trace::{SamplingDecision, SamplingResult, SpanKind, Status, TraceContextExt, TraceState},
    };
    use opentelemetry::{testing::trace::TestSpan, InstrumentationScope};
    use opentelemetry::{
        trace::{
            Event, Link, Span, SpanBuilder, SpanContext, SpanId, TraceFlags, TraceId, Tracer,
            TracerProvider,
        },
        Context, KeyValue,
    };
    use std::time::Duration;

    #[test]
    fn span_modification_via_context() {
        let exporter = InMemorySpanExporterBuilder::new().build();
        let provider = SdkTracerProvider::builder()
            .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
            .build();
        let tracer = provider.tracer("test_tracer");

        #[derive(Debug, PartialEq)]
        struct ValueA(u64);

        let span = tracer.start("span-name");

        // start with Current, which should have no span
        let cx = Context::current();
        assert!(!cx.has_active_span());

        // add span to context
        let cx_with_span = cx.with_span(span);
        assert!(cx_with_span.has_active_span());
        assert!(!cx.has_active_span());

        // modify the span by using span_ref from the context
        // this is the only way to modify the span as span
        // is moved to context.
        let span_ref = cx_with_span.span();
        span_ref.set_attribute(KeyValue::new("attribute1", "value1"));

        // create a new context, which should not affect the original
        let cx_with_span_and_more = cx_with_span.with_value(ValueA(1));

        // modify the span again using the new context.
        // this should still be using the original span itself.
        let span_ref_new = cx_with_span_and_more.span();
        span_ref_new.set_attribute(KeyValue::new("attribute2", "value2"));

        span_ref_new.end();

        let exported_spans = exporter
            .get_finished_spans()
            .expect("Spans are expected to be exported.");
        // There should be a single span, with attributes from both modifications.
        assert_eq!(exported_spans.len(), 1);
        let span = &exported_spans[0];
        assert_eq!(span.attributes.len(), 2);
    }

    #[derive(Debug)]
    struct BaggageInspectingSpanProcessor;
    impl SpanProcessor for BaggageInspectingSpanProcessor {
        fn on_start(&self, span: &mut crate::trace::Span, cx: &Context) {
            let baggage = cx.baggage();
            if let Some(baggage_value) = baggage.get("bag-key") {
                span.set_attribute(KeyValue::new("bag-key", baggage_value.to_string()));
            } else {
                unreachable!("Baggage should be present in the context");
            }
        }

        fn on_end(&self, _span: SpanData) {
            // TODO: Accessing Context::current() will panic today and hence commented out.
            // See https://github.com/open-telemetry/opentelemetry-rust/issues/2871
            // let _c = Context::current();
        }

        fn force_flush(&self) -> crate::error::OTelSdkResult {
            Ok(())
        }

        fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
            Ok(())
        }
    }

    #[test]
    fn span_and_baggage() {
        let provider = SdkTracerProvider::builder()
            .with_span_processor(BaggageInspectingSpanProcessor)
            .build();

        let cx_with_baggage =
            Context::current_with_baggage(vec![KeyValue::new("bag-key", "bag-value")]);

        // assert baggage is in the context
        assert_eq!(
            cx_with_baggage
                .baggage()
                .get("bag-key")
                .unwrap()
                .to_string(),
            "bag-value"
        );

        // Attach context to current
        let _cx_guard1 = cx_with_baggage.attach();
        // now Current should have the baggage
        assert_eq!(
            Context::current()
                .baggage()
                .get("bag-key")
                .unwrap()
                .to_string(),
            "bag-value"
        );

        let tracer = provider.tracer("test_tracer");
        let mut span = tracer
            .span_builder("span-name")
            .start_with_context(&tracer, &Context::current());
        span.set_attribute(KeyValue::new("attribute1", "value1"));

        // We have not added span to the context yet
        // so the current context should not have any span.
        let cx = Context::current();
        assert!(!cx.has_active_span());

        // Now add span to context which already has baggage.
        let cx_with_baggage_and_span = cx.with_span(span);
        assert!(cx_with_baggage_and_span.has_active_span());
        assert_eq!(
            cx_with_baggage_and_span
                .baggage()
                .get("bag-key")
                .unwrap()
                .to_string(),
            "bag-value"
        );

        let _cx_guard2 = cx_with_baggage_and_span.attach();
        // Now current context should have both baggage and span.
        assert!(Context::current().has_active_span());
        assert_eq!(
            Context::current()
                .baggage()
                .get("bag-key")
                .unwrap()
                .to_string(),
            "bag-value"
        );
    }

    #[test]
    fn tracer_in_span() {
        // Arrange
        let exporter = InMemorySpanExporterBuilder::new().build();
        let provider = SdkTracerProvider::builder()
            .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
            .build();

        // Act
        let tracer = provider.tracer("test_tracer");
        tracer.in_span("span_name", |cx| {
            let span = cx.span();
            assert!(span.is_recording());
            span.update_name("span_name_updated");
            span.set_attribute(KeyValue::new("attribute1", "value1"));
            span.add_event("test-event".to_string(), vec![]);
            span.add_link(
                SpanContext::new(
                    TraceId::from(47),
                    SpanId::from(11),
                    TraceFlags::default(),
                    false,
                    Default::default(),
                ),
                vec![],
            );
        });

        // Assert
        let exported_spans = exporter
            .get_finished_spans()
            .expect("Spans are expected to be exported.");
        assert_eq!(exported_spans.len(), 1);
        let span = &exported_spans[0];
        assert_eq!(span.name, "span_name_updated");
        assert_eq!(span.instrumentation_scope.name(), "test_tracer");
        assert_eq!(span.attributes.len(), 1);
        assert_eq!(span.events.len(), 1);
        assert_eq!(span.events[0].name, "test-event");
        assert_eq!(span.links.len(), 1);
        assert_eq!(span.links[0].span_context.trace_id(), TraceId::from(47));
        assert_eq!(span.links[0].span_context.span_id(), SpanId::from(11));
        assert_eq!(span.span_context.trace_flags(), TraceFlags::SAMPLED);
        assert!(!span.span_context.is_remote());
        assert_eq!(span.status, Status::Unset);
    }

    #[test]
    fn tracer_start() {
        // Arrange
        let exporter = InMemorySpanExporterBuilder::new().build();
        let provider = SdkTracerProvider::builder()
            .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
            .build();

        // Act
        let tracer = provider.tracer("test_tracer");
        let mut span = tracer.start("span_name");
        span.set_attribute(KeyValue::new("attribute1", "value1"));
        span.add_event("test-event".to_string(), vec![]);
        span.set_status(Status::error("cancelled"));
        span.end();

        // After span end, further operations should not have any effect
        span.update_name("span_name_updated");

        // Assert
        let exported_spans = exporter
            .get_finished_spans()
            .expect("Spans are expected to be exported.");
        assert_eq!(exported_spans.len(), 1);
        let span = &exported_spans[0];
        assert_eq!(span.name, "span_name");
        assert_eq!(span.instrumentation_scope.name(), "test_tracer");
        assert_eq!(span.attributes.len(), 1);
        assert_eq!(span.events.len(), 1);
        assert_eq!(span.events[0].name, "test-event");
        assert_eq!(span.span_context.trace_flags(), TraceFlags::SAMPLED);
        assert!(!span.span_context.is_remote());
        let status_expected = Status::error("cancelled");
        assert_eq!(span.status, status_expected);
    }

    #[test]
    fn tracer_span_builder() {
        // Arrange
        let exporter = InMemorySpanExporterBuilder::new().build();
        let provider = SdkTracerProvider::builder()
            .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
            .build();

        // Act
        let tracer = provider.tracer("test_tracer");
        let mut span = tracer
            .span_builder("span_name")
            .with_kind(SpanKind::Server)
            .start(&tracer);
        span.set_attribute(KeyValue::new("attribute1", "value1"));
        span.add_event("test-event".to_string(), vec![]);
        span.set_status(Status::Ok);
        drop(span);

        // Assert
        let exported_spans = exporter
            .get_finished_spans()
            .expect("Spans are expected to be exported.");
        assert_eq!(exported_spans.len(), 1);
        let span = &exported_spans[0];
        assert_eq!(span.name, "span_name");
        assert_eq!(span.span_kind, SpanKind::Server);
        assert_eq!(span.instrumentation_scope.name(), "test_tracer");
        assert_eq!(span.attributes.len(), 1);
        assert_eq!(span.events.len(), 1);
        assert_eq!(span.events[0].name, "test-event");
        assert_eq!(span.span_context.trace_flags(), TraceFlags::SAMPLED);
        assert!(!span.span_context.is_remote());
        assert_eq!(span.status, Status::Ok);
    }

    #[test]
    fn exceed_span_links_limit() {
        // Arrange
        let exporter = InMemorySpanExporterBuilder::new().build();
        let provider = SdkTracerProvider::builder()
            .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
            .build();

        // Act
        let tracer = provider.tracer("test_tracer");

        let mut links = Vec::new();
        for _i in 0..(DEFAULT_MAX_LINKS_PER_SPAN * 2) {
            links.push(Link::with_context(SpanContext::new(
                TraceId::from_u128(12),
                SpanId::from_u64(12),
                TraceFlags::default(),
                false,
                Default::default(),
            )))
        }

        let span_builder = SpanBuilder::from_name("span_name").with_links(links);
        let mut span = tracer.build(span_builder);
        span.end();

        // Assert
        let exported_spans = exporter
            .get_finished_spans()
            .expect("Spans are expected to be exported.");
        assert_eq!(exported_spans.len(), 1);
        let span = &exported_spans[0];
        assert_eq!(span.name, "span_name");
        assert_eq!(span.links.len(), DEFAULT_MAX_LINKS_PER_SPAN as usize);
    }

    #[test]
    fn exceed_span_events_limit() {
        // Arrange
        let exporter = InMemorySpanExporterBuilder::new().build();
        let provider = SdkTracerProvider::builder()
            .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
            .build();

        // Act
        let tracer = provider.tracer("test_tracer");

        let mut events = Vec::new();
        for _i in 0..(DEFAULT_MAX_EVENT_PER_SPAN * 2) {
            events.push(Event::with_name("test event"))
        }

        // add events via span builder
        let span_builder = SpanBuilder::from_name("span_name").with_events(events);
        let mut span = tracer.build(span_builder);

        // add events using span api after building the span
        span.add_event("test event again, after span builder", Vec::new());
        span.add_event("test event once again, after span builder", Vec::new());
        span.end();

        // Assert
        let exported_spans = exporter
            .get_finished_spans()
            .expect("Spans are expected to be exported.");
        assert_eq!(exported_spans.len(), 1);
        let span = &exported_spans[0];
        assert_eq!(span.name, "span_name");
        assert_eq!(span.events.len(), DEFAULT_MAX_EVENT_PER_SPAN as usize);
        assert_eq!(span.events.dropped_count, DEFAULT_MAX_EVENT_PER_SPAN + 2);
    }

    #[test]
    fn trace_state_for_dropped_sampler() {
        let exporter = InMemorySpanExporterBuilder::new().build();
        let provider = SdkTracerProvider::builder()
            .with_sampler(Sampler::AlwaysOff)
            .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
            .build();

        let tracer = provider.tracer("test");
        let trace_state = TraceState::from_key_value(vec![("foo", "bar")]).unwrap();

        let parent_context = Context::new().with_span(TestSpan(SpanContext::new(
            TraceId::from_u128(10000),
            SpanId::from_u64(20),
            TraceFlags::SAMPLED,
            true,
            trace_state.clone(),
        )));

        let span = tracer.start_with_context("span", &parent_context);
        assert_eq!(
            span.span_context().trace_state().get("foo"),
            trace_state.get("foo")
        )
    }

    #[derive(Clone, Debug, Default)]
    struct TestRecordOnlySampler {}

    impl ShouldSample for TestRecordOnlySampler {
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
                decision: SamplingDecision::RecordOnly,
                attributes: vec![KeyValue::new("record_only_key", "record_only_value")],
                trace_state,
            }
        }
    }

    #[test]
    fn trace_state_for_record_only_sampler() {
        let exporter = InMemorySpanExporterBuilder::new().build();
        let provider = SdkTracerProvider::builder()
            .with_sampler(TestRecordOnlySampler::default())
            .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
            .build();

        let tracer = provider.tracer("test");
        let trace_state = TraceState::from_key_value(vec![("foo", "bar")]).unwrap();

        let parent_context = Context::new().with_span(TestSpan(SpanContext::new(
            TraceId::from_u128(10000),
            SpanId::from_u64(20),
            TraceFlags::SAMPLED,
            true,
            trace_state.clone(),
        )));

        let span = tracer.build_with_context(
            SpanBuilder::from_name("span")
                .with_attributes(vec![KeyValue::new("extra_attr_key", "extra_attr_value")]),
            &parent_context,
        );
        assert!(!span.span_context().trace_flags().is_sampled());
        assert_eq!(
            span.exported_data().unwrap().attributes,
            vec![
                KeyValue::new("extra_attr_key", "extra_attr_value"),
                KeyValue::new("record_only_key", "record_only_value")
            ]
        );
        assert_eq!(span.span_context().trace_state().get("foo"), Some("bar"));
    }

    #[test]
    fn tracer_attributes() {
        let provider = SdkTracerProvider::builder().build();
        let scope = InstrumentationScope::builder("basic")
            .with_attributes(vec![KeyValue::new("test_k", "test_v")])
            .build();

        let tracer = provider.tracer_with_scope(scope);
        let instrumentation_scope = tracer.instrumentation_scope();
        assert!(instrumentation_scope
            .attributes()
            .eq(&[KeyValue::new("test_k", "test_v")]));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn empty_tracer_name_retained() {
        async fn tracer_name_retained_helper(
            tracer: super::SdkTracer,
            provider: SdkTracerProvider,
            exporter: InMemorySpanExporter,
        ) {
            // Act
            tracer.start("my_span").end();

            // Force flush to ensure spans are exported
            assert!(provider.force_flush().is_ok());

            // Assert
            let finished_spans = exporter
                .get_finished_spans()
                .expect("spans are expected to be exported.");
            assert_eq!(finished_spans.len(), 1, "There should be a single span");

            let tracer_name = finished_spans[0].instrumentation_scope.name();
            assert_eq!(tracer_name, "", "The tracer name should be an empty string");

            exporter.reset();
        }

        let exporter = InMemorySpanExporter::default();
        let span_processor = SimpleSpanProcessor::new(exporter.clone());
        let tracer_provider = SdkTracerProvider::builder()
            .with_span_processor(span_processor)
            .build();

        // Test Tracer creation in 2 ways, both with empty string as tracer name
        let tracer1 = tracer_provider.tracer("");
        tracer_name_retained_helper(tracer1, tracer_provider.clone(), exporter.clone()).await;

        let tracer_scope = InstrumentationScope::builder("").build();
        let tracer2 = tracer_provider.tracer_with_scope(tracer_scope);
        tracer_name_retained_helper(tracer2, tracer_provider, exporter).await;
    }

    #[test]
    fn trace_suppression() {
        // Arrange
        let exporter = InMemorySpanExporter::default();
        let span_processor = SimpleSpanProcessor::new(exporter.clone());
        let tracer_provider = SdkTracerProvider::builder()
            .with_span_processor(span_processor)
            .build();

        // Act
        let tracer = tracer_provider.tracer("test");
        {
            let _suppressed_context = Context::enter_telemetry_suppressed_scope();
            // This span should not be emitted as it is created in a suppressed context
            let _span = tracer.span_builder("span_name").start(&tracer);
        }

        // Assert
        let finished_spans = exporter.get_finished_spans().expect("this should not fail");
        assert_eq!(
            finished_spans.len(),
            0,
            "There should be a no spans as span emission is done inside a suppressed context"
        );
    }
}
