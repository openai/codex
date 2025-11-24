use opentelemetry::{
    logs::{AnyValue, LogRecord, Logger, LoggerProvider, Severity},
    Key,
};
use tracing_core::Level;
#[cfg(feature = "experimental_metadata_attributes")]
use tracing_core::Metadata;
#[cfg(feature = "experimental_metadata_attributes")]
use tracing_log::NormalizeEvent;
use tracing_subscriber::{registry::LookupSpan, Layer};

/// Visitor to record the fields from the event record.
struct EventVisitor<'a, LR: LogRecord> {
    log_record: &'a mut LR,
}

/// Logs from the log crate have duplicated attributes that we removed here.
#[cfg(feature = "experimental_metadata_attributes")]
fn is_duplicated_metadata(field: &'static str) -> bool {
    field
        .strip_prefix("log.")
        .map(|remainder| matches!(remainder, "file" | "line" | "module_path" | "target"))
        .unwrap_or(false)
}

#[cfg(feature = "experimental_metadata_attributes")]
fn get_filename(filepath: &str) -> &str {
    if let Some((_, filename)) = filepath.rsplit_once('/') {
        return filename;
    }
    if let Some((_, filename)) = filepath.rsplit_once('\\') {
        return filename;
    }
    filepath
}

impl<'a, LR: LogRecord> EventVisitor<'a, LR> {
    fn new(log_record: &'a mut LR) -> Self {
        EventVisitor { log_record }
    }

    #[cfg(feature = "experimental_metadata_attributes")]
    fn visit_experimental_metadata(&mut self, meta: &Metadata) {
        if let Some(module_path) = meta.module_path() {
            self.log_record.add_attribute(
                Key::new("code.namespace"),
                AnyValue::from(module_path.to_owned()),
            );
        }

        if let Some(filepath) = meta.file() {
            self.log_record.add_attribute(
                Key::new("code.filepath"),
                AnyValue::from(filepath.to_owned()),
            );
            self.log_record.add_attribute(
                Key::new("code.filename"),
                AnyValue::from(get_filename(filepath).to_owned()),
            );
        }

        if let Some(line) = meta.line() {
            self.log_record
                .add_attribute(Key::new("code.lineno"), AnyValue::from(line));
        }
    }
}

impl<LR: LogRecord> tracing::field::Visit for EventVisitor<'_, LR> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        #[cfg(feature = "experimental_metadata_attributes")]
        if is_duplicated_metadata(field.name()) {
            return;
        }
        if field.name() == "message" {
            self.log_record.set_body(format!("{:?}", value).into());
        } else {
            self.log_record
                .add_attribute(Key::new(field.name()), AnyValue::from(format!("{value:?}")));
        }
    }

    fn record_error(
        &mut self,
        _field: &tracing_core::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.log_record.add_attribute(
            Key::new("exception.message"),
            AnyValue::from(value.to_string()),
        );
        // No ability to get exception.stacktrace or exception.type from the error today.
    }

    fn record_bytes(&mut self, field: &tracing_core::Field, value: &[u8]) {
        self.log_record
            .add_attribute(Key::new(field.name()), AnyValue::from(value));
    }

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        #[cfg(feature = "experimental_metadata_attributes")]
        if is_duplicated_metadata(field.name()) {
            return;
        }
        //TODO: Fix heap allocation. Check if lifetime of &str can be used
        // to optimize sync exporter scenario.
        if field.name() == "message" {
            self.log_record.set_body(AnyValue::from(value.to_owned()));
        } else {
            self.log_record
                .add_attribute(Key::new(field.name()), AnyValue::from(value.to_owned()));
        }
    }

    fn record_bool(&mut self, field: &tracing_core::Field, value: bool) {
        self.log_record
            .add_attribute(Key::new(field.name()), AnyValue::from(value));
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.log_record
            .add_attribute(Key::new(field.name()), AnyValue::from(value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        #[cfg(feature = "experimental_metadata_attributes")]
        if is_duplicated_metadata(field.name()) {
            return;
        }
        self.log_record
            .add_attribute(Key::new(field.name()), AnyValue::from(value));
    }

    // TODO: We might need to do similar for record_i128,record_u128 too
    // to avoid stringification, unless needed.
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        #[cfg(feature = "experimental_metadata_attributes")]
        if is_duplicated_metadata(field.name()) {
            return;
        }
        if let Ok(signed) = i64::try_from(value) {
            self.log_record
                .add_attribute(Key::new(field.name()), AnyValue::from(signed));
        } else {
            self.log_record
                .add_attribute(Key::new(field.name()), AnyValue::from(format!("{value:?}")));
        }
    }

    fn record_i128(&mut self, field: &tracing::field::Field, value: i128) {
        #[cfg(feature = "experimental_metadata_attributes")]
        if is_duplicated_metadata(field.name()) {
            return;
        }
        if let Ok(signed) = i64::try_from(value) {
            self.log_record
                .add_attribute(Key::new(field.name()), AnyValue::from(signed));
        } else {
            self.log_record
                .add_attribute(Key::new(field.name()), AnyValue::from(format!("{value:?}")));
        }
    }

    fn record_u128(&mut self, field: &tracing::field::Field, value: u128) {
        #[cfg(feature = "experimental_metadata_attributes")]
        if is_duplicated_metadata(field.name()) {
            return;
        }
        if let Ok(signed) = i64::try_from(value) {
            self.log_record
                .add_attribute(Key::new(field.name()), AnyValue::from(signed));
        } else {
            self.log_record
                .add_attribute(Key::new(field.name()), AnyValue::from(format!("{value:?}")));
        }
    }

    // TODO: Remaining field types from AnyValue : Bytes, ListAny, Boolean
}

pub struct OpenTelemetryTracingBridge<P, L>
where
    P: LoggerProvider<Logger = L> + Send + Sync,
    L: Logger + Send + Sync,
{
    logger: L,
    _phantom: std::marker::PhantomData<P>, // P is not used.
}

impl<P, L> OpenTelemetryTracingBridge<P, L>
where
    P: LoggerProvider<Logger = L> + Send + Sync,
    L: Logger + Send + Sync,
{
    pub fn new(provider: &P) -> Self {
        OpenTelemetryTracingBridge {
            // Using empty scope name.
            // The name/version of this library itself can be added
            // as a Scope attribute, once a semantic convention is
            // defined for the same.
            // See https://github.com/open-telemetry/semantic-conventions/issues/1550
            logger: provider.logger(""),
            _phantom: Default::default(),
        }
    }
}

impl<S, P, L> Layer<S> for OpenTelemetryTracingBridge<P, L>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    P: LoggerProvider<Logger = L> + Send + Sync + 'static,
    L: Logger + Send + Sync + 'static,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let severity = severity_of_level(metadata.level());
        let target = metadata.target();
        let name = metadata.name();
        #[cfg(feature = "spec_unstable_logs_enabled")]
        if !self.logger.event_enabled(severity, target, Some(name)) {
            // TODO: See if we need internal logs or track the count.
            return;
        }

        #[cfg(feature = "experimental_metadata_attributes")]
        let normalized_meta = event.normalized_metadata();

        #[cfg(feature = "experimental_metadata_attributes")]
        let meta = normalized_meta.as_ref().unwrap_or_else(|| event.metadata());

        let mut log_record = self.logger.create_log_record();

        log_record.set_target(target);
        log_record.set_event_name(name);
        log_record.set_severity_number(severity);
        log_record.set_severity_text(metadata.level().as_str());
        let mut visitor = EventVisitor::new(&mut log_record);
        #[cfg(feature = "experimental_metadata_attributes")]
        visitor.visit_experimental_metadata(meta);
        // Visit fields.
        event.record(&mut visitor);

        #[cfg(feature = "experimental_use_tracing_span_context")]
        if let Some(span) = _ctx.event_span(event) {
            use opentelemetry::trace::TraceContextExt;
            use tracing_opentelemetry::OtelData;
            if let Some(otd) = span.extensions().get::<OtelData>() {
                if let Some(span_id) = otd.builder.span_id {
                    let opt_trace_id = if otd.parent_cx.has_active_span() {
                        Some(otd.parent_cx.span().span_context().trace_id())
                    } else {
                        span.scope().last().and_then(|root_span| {
                            root_span
                                .extensions()
                                .get::<OtelData>()
                                .and_then(|otd| otd.builder.trace_id)
                        })
                    };
                    if let Some(trace_id) = opt_trace_id {
                        log_record.set_trace_context(trace_id, span_id, None);
                    }
                }
            }
        }

        //emit record
        self.logger.emit(log_record);
    }
}

const fn severity_of_level(level: &Level) -> Severity {
    match *level {
        Level::TRACE => Severity::Trace,
        Level::DEBUG => Severity::Debug,
        Level::INFO => Severity::Info,
        Level::WARN => Severity::Warn,
        Level::ERROR => Severity::Error,
    }
}

#[cfg(test)]
mod tests {
    use crate::layer;
    use opentelemetry::logs::Severity;
    use opentelemetry::trace::TracerProvider;
    use opentelemetry::trace::{TraceContextExt, TraceFlags, Tracer};
    use opentelemetry::InstrumentationScope;
    use opentelemetry::{logs::AnyValue, Key};
    use opentelemetry_sdk::error::{OTelSdkError, OTelSdkResult};
    use opentelemetry_sdk::logs::{InMemoryLogExporter, LogProcessor};
    use opentelemetry_sdk::logs::{SdkLogRecord, SdkLoggerProvider};
    use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
    use tracing::error;
    use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
    use tracing_subscriber::Layer;

    pub fn attributes_contains(log_record: &SdkLogRecord, key: &Key, value: &AnyValue) -> bool {
        log_record
            .attributes_iter()
            .any(|(k, v)| k == key && v == value)
    }

    #[allow(impl_trait_overcaptures)] // can only be fixed with Rust 1.82+
    fn create_tracing_subscriber(logger_provider: &SdkLoggerProvider) -> impl tracing::Subscriber {
        let level_filter = tracing_subscriber::filter::LevelFilter::WARN; // Capture WARN and ERROR levels
        let layer =
            layer::OpenTelemetryTracingBridge::new(logger_provider).with_filter(level_filter); // No filter based on target, only based on log level

        tracing_subscriber::registry().with(layer)
    }

    // cargo test --features=testing
    #[test]
    fn tracing_appender_standalone() {
        // Arrange
        let exporter: InMemoryLogExporter = InMemoryLogExporter::default();
        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();

        let subscriber = create_tracing_subscriber(&logger_provider);

        // avoiding setting tracing subscriber as global as that does not
        // play well with unit tests.
        let _guard = tracing::subscriber::set_default(subscriber);

        // Act
        let small_u64value: u64 = 42;
        let big_u64value: u64 = u64::MAX;
        let small_usizevalue: usize = 42;
        let big_usizevalue: usize = usize::MAX;
        let small_u128value: u128 = 42;
        let big_u128value: u128 = u128::MAX;
        let small_i128value: i128 = 42;
        let big_i128value: i128 = i128::MAX;
        error!(name: "my-event-name", target: "my-system", event_id = 20, bytes = &b"abc"[..], error = &OTelSdkError::AlreadyShutdown as &dyn std::error::Error, small_u64value, big_u64value, small_usizevalue, big_usizevalue, small_u128value, big_u128value, small_i128value, big_i128value, user_name = "otel", user_email = "otel@opentelemetry.io");
        assert!(logger_provider.force_flush().is_ok());

        // Assert TODO: move to helper methods
        let exported_logs = exporter
            .get_emitted_logs()
            .expect("Logs are expected to be exported.");
        assert_eq!(exported_logs.len(), 1);
        let log = exported_logs
            .first()
            .expect("Atleast one log is expected to be present.");

        // Validate common fields
        assert_eq!(log.instrumentation.name(), "");
        assert_eq!(log.record.severity_number(), Some(Severity::Error));
        // Validate target
        assert_eq!(
            log.record.target().expect("target is expected").to_string(),
            "my-system"
        );
        // Validate event name
        assert_eq!(
            log.record.event_name().expect("event_name is expected"),
            "my-event-name"
        );

        // Validate trace context is none.
        assert!(log.record.trace_context().is_none());

        // Validate attributes
        #[cfg(not(feature = "experimental_metadata_attributes"))]
        assert_eq!(log.record.attributes_iter().count(), 13);
        #[cfg(feature = "experimental_metadata_attributes")]
        assert_eq!(log.record.attributes_iter().count(), 17);
        assert!(attributes_contains(
            &log.record,
            &Key::new("event_id"),
            &AnyValue::Int(20)
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("user_name"),
            &AnyValue::String("otel".into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("user_email"),
            &AnyValue::String("otel@opentelemetry.io".into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("exception.message"),
            &AnyValue::String(OTelSdkError::AlreadyShutdown.to_string().into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("small_u64value"),
            &AnyValue::Int(42.into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("big_u64value"),
            &AnyValue::String(format!("{}", u64::MAX).into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("small_usizevalue"),
            &AnyValue::Int(42.into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("big_usizevalue"),
            &AnyValue::String(format!("{}", u64::MAX).into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("small_u128value"),
            &AnyValue::Int(42.into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("big_u128value"),
            &AnyValue::String(format!("{}", u128::MAX).into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("small_i128value"),
            &AnyValue::Int(42.into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("big_i128value"),
            &AnyValue::String(format!("{}", i128::MAX).into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("bytes"),
            &AnyValue::Bytes(Box::new(b"abc".to_vec()))
        ));
        #[cfg(feature = "experimental_metadata_attributes")]
        {
            assert!(attributes_contains(
                &log.record,
                &Key::new("code.filename"),
                &AnyValue::String("layer.rs".into())
            ));
            assert!(attributes_contains(
                &log.record,
                &Key::new("code.namespace"),
                &AnyValue::String("opentelemetry_appender_tracing::layer::tests".into())
            ));
            // The other 3 experimental_metadata_attributes are too unstable to check their value.
            // Ex.: The path will be different on a Windows and Linux machine.
            // Ex.: The line can change easily if someone makes changes in this source file.
            let attributes_key: Vec<Key> = log
                .record
                .attributes_iter()
                .map(|(key, _)| key.clone())
                .collect();
            assert!(attributes_key.contains(&Key::new("code.filepath")));
            assert!(attributes_key.contains(&Key::new("code.lineno")));
            assert!(!attributes_key.contains(&Key::new("log.target")));
        }

        // Test when target, eventname are not explicitly provided
        exporter.reset();
        error!(
            event_id = 20,
            user_name = "otel",
            user_email = "otel@opentelemetry.io"
        );
        assert!(logger_provider.force_flush().is_ok());

        // Assert TODO: move to helper methods
        let exported_logs = exporter
            .get_emitted_logs()
            .expect("Logs are expected to be exported.");
        assert_eq!(exported_logs.len(), 1);
        let log = exported_logs
            .first()
            .expect("Atleast one log is expected to be present.");

        // Validate target - tracing defaults to module path
        assert_eq!(
            log.record.target().expect("target is expected").to_string(),
            "opentelemetry_appender_tracing::layer::tests"
        );
        // Validate event name - tracing defaults to event followed source & line number
        // Assert is doing "contains" check to avoid tests failing when line number changes.
        // and also account for the fact that the module path is different on different platforms.
        // Ex.: The path will be different on a Windows and Linux machine.
        assert!(log
            .record
            .event_name()
            .expect("event_name is expected")
            .contains("event opentelemetry-appender-tracing"),);
    }

    #[test]
    fn tracing_appender_inside_tracing_context() {
        // Arrange
        let exporter: InMemoryLogExporter = InMemoryLogExporter::default();
        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();

        let subscriber = create_tracing_subscriber(&logger_provider);

        // avoiding setting tracing subscriber as global as that does not
        // play well with unit tests.
        let _guard = tracing::subscriber::set_default(subscriber);

        // setup tracing as well.
        let tracer_provider = SdkTracerProvider::builder()
            .with_sampler(Sampler::AlwaysOn)
            .build();
        let tracer = tracer_provider.tracer("test-tracer");

        // Act
        let (trace_id_expected, span_id_expected) = tracer.in_span("test-span", |cx| {
            let trace_id = cx.span().span_context().trace_id();
            let span_id = cx.span().span_context().span_id();

            // logging is done inside span context.
            error!(name: "my-event-name", target: "my-system", event_id = 20, user_name = "otel", user_email = "otel@opentelemetry.io");
            (trace_id, span_id)
        });

        assert!(logger_provider.force_flush().is_ok());

        // Assert TODO: move to helper methods
        let exported_logs = exporter
            .get_emitted_logs()
            .expect("Logs are expected to be exported.");
        assert_eq!(exported_logs.len(), 1);
        let log = exported_logs
            .first()
            .expect("Atleast one log is expected to be present.");

        // validate common fields.
        assert_eq!(log.instrumentation.name(), "");
        assert_eq!(log.record.severity_number(), Some(Severity::Error));
        // Validate target
        assert_eq!(
            log.record.target().expect("target is expected").to_string(),
            "my-system"
        );
        // Validate event name
        assert_eq!(
            log.record.event_name().expect("event_name is expected"),
            "my-event-name"
        );

        // validate trace context.
        assert!(log.record.trace_context().is_some());
        assert_eq!(
            log.record.trace_context().unwrap().trace_id,
            trace_id_expected
        );
        assert_eq!(
            log.record.trace_context().unwrap().span_id,
            span_id_expected
        );
        assert_eq!(
            log.record.trace_context().unwrap().trace_flags.unwrap(),
            TraceFlags::SAMPLED
        );

        // validate attributes.
        #[cfg(not(feature = "experimental_metadata_attributes"))]
        assert_eq!(log.record.attributes_iter().count(), 3);
        #[cfg(feature = "experimental_metadata_attributes")]
        assert_eq!(log.record.attributes_iter().count(), 7);
        assert!(attributes_contains(
            &log.record,
            &Key::new("event_id"),
            &AnyValue::Int(20.into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("user_name"),
            &AnyValue::String("otel".into())
        ));
        assert!(attributes_contains(
            &log.record,
            &Key::new("user_email"),
            &AnyValue::String("otel@opentelemetry.io".into())
        ));
        #[cfg(feature = "experimental_metadata_attributes")]
        {
            assert!(attributes_contains(
                &log.record,
                &Key::new("code.filename"),
                &AnyValue::String("layer.rs".into())
            ));
            assert!(attributes_contains(
                &log.record,
                &Key::new("code.namespace"),
                &AnyValue::String("opentelemetry_appender_tracing::layer::tests".into())
            ));
            // The other 3 experimental_metadata_attributes are too unstable to check their value.
            // Ex.: The path will be different on a Windows and Linux machine.
            // Ex.: The line can change easily if someone makes changes in this source file.
            let attributes_key: Vec<Key> = log
                .record
                .attributes_iter()
                .map(|(key, _)| key.clone())
                .collect();
            assert!(attributes_key.contains(&Key::new("code.filepath")));
            assert!(attributes_key.contains(&Key::new("code.lineno")));
            assert!(!attributes_key.contains(&Key::new("log.target")));
        }
    }

    #[cfg(feature = "experimental_use_tracing_span_context")]
    #[test]
    fn tracing_appender_inside_tracing_crate_context() {
        use opentelemetry::{trace::SpanContext, Context, SpanId, TraceId};
        use opentelemetry_sdk::trace::InMemorySpanExporterBuilder;
        use tracing_opentelemetry::OpenTelemetrySpanExt;

        // Arrange
        let exporter: InMemoryLogExporter = InMemoryLogExporter::default();
        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();

        // setup tracing layer to compare trace/span IDs against
        let span_exporter = InMemorySpanExporterBuilder::new().build();
        let tracer_provider = SdkTracerProvider::builder()
            .with_simple_exporter(span_exporter.clone())
            .build();
        let tracer = tracer_provider.tracer("test-tracer");

        let level_filter = tracing_subscriber::filter::LevelFilter::ERROR;
        let log_layer =
            layer::OpenTelemetryTracingBridge::new(&logger_provider).with_filter(level_filter);

        let subscriber = tracing_subscriber::registry()
            .with(log_layer)
            .with(tracing_opentelemetry::layer().with_tracer(tracer));

        // Avoiding global subscriber.init() as that does not play well with unit tests.
        let _guard = tracing::subscriber::set_default(subscriber);

        // Act
        tracing::error_span!("outer-span").in_scope(|| {
            error!("first-event");

            tracing::error_span!("inner-span").in_scope(|| {
                error!("second-event");
            });
        });

        assert!(logger_provider.force_flush().is_ok());

        let logs = exporter.get_emitted_logs().expect("No emitted logs");
        assert_eq!(logs.len(), 2, "Expected 2 logs, got: {logs:?}");

        let spans = span_exporter.get_finished_spans().unwrap();
        assert_eq!(spans.len(), 2);

        let trace_id = spans[0].span_context.trace_id();
        assert_eq!(trace_id, spans[1].span_context.trace_id());
        let inner_span_id = spans[0].span_context.span_id();
        let outer_span_id = spans[1].span_context.span_id();
        assert_eq!(outer_span_id, spans[0].parent_span_id);

        let trace_ctx0 = logs[0].record.trace_context().unwrap();
        let trace_ctx1 = logs[1].record.trace_context().unwrap();

        assert_eq!(trace_ctx0.trace_id, trace_id);
        assert_eq!(trace_ctx1.trace_id, trace_id);
        assert_eq!(trace_ctx0.span_id, outer_span_id);
        assert_eq!(trace_ctx1.span_id, inner_span_id);

        // Set context from remote.
        let remote_trace_id = TraceId::from_u128(233);
        let remote_span_id = SpanId::from_u64(2333);
        let remote_span_context = SpanContext::new(
            remote_trace_id,
            remote_span_id,
            TraceFlags::SAMPLED,
            true,
            Default::default(),
        );

        // Act again.
        tracing::error_span!("outer-span").in_scope(|| {
            let span = tracing::Span::current();
            let parent_context = Context::current().with_remote_span_context(remote_span_context);
            span.set_parent(parent_context);

            error!("first-event");

            tracing::error_span!("inner-span").in_scope(|| {
                error!("second-event");
            });
        });

        assert!(logger_provider.force_flush().is_ok());

        let logs = exporter.get_emitted_logs().expect("No emitted logs");
        assert_eq!(logs.len(), 4, "Expected 4 logs, got: {logs:?}");
        let logs = &logs[2..];

        let spans = span_exporter.get_finished_spans().unwrap();
        assert_eq!(spans.len(), 4);
        let spans = &spans[2..];

        let trace_id = spans[0].span_context.trace_id();
        assert_eq!(trace_id, remote_trace_id);
        assert_eq!(trace_id, spans[1].span_context.trace_id());
        let inner_span_id = spans[0].span_context.span_id();
        let outer_span_id = spans[1].span_context.span_id();
        assert_eq!(outer_span_id, spans[0].parent_span_id);

        let trace_ctx0 = logs[0].record.trace_context().unwrap();
        let trace_ctx1 = logs[1].record.trace_context().unwrap();

        assert_eq!(trace_ctx0.trace_id, trace_id);
        assert_eq!(trace_ctx1.trace_id, trace_id);
        assert_eq!(trace_ctx0.span_id, outer_span_id);
        assert_eq!(trace_ctx1.span_id, inner_span_id);
    }

    #[test]
    fn tracing_appender_standalone_with_tracing_log() {
        // Arrange
        let exporter: InMemoryLogExporter = InMemoryLogExporter::default();
        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();

        let subscriber = create_tracing_subscriber(&logger_provider);

        // avoiding setting tracing subscriber as global as that does not
        // play well with unit tests.
        let _guard = tracing::subscriber::set_default(subscriber);
        drop(tracing_log::LogTracer::init());

        // Act
        log::error!("log from log crate");
        assert!(logger_provider.force_flush().is_ok());

        // Assert TODO: move to helper methods
        let exported_logs = exporter
            .get_emitted_logs()
            .expect("Logs are expected to be exported.");
        assert_eq!(exported_logs.len(), 1);
        let log = exported_logs
            .first()
            .expect("Atleast one log is expected to be present.");

        // Validate common fields
        assert_eq!(log.instrumentation.name(), "");
        assert_eq!(log.record.severity_number(), Some(Severity::Error));
        // Target and EventName from Log crate are "log" and "log event" respectively.
        // Validate target
        assert_eq!(
            log.record.target().expect("target is expected").to_string(),
            "log"
        );
        // Validate event name
        assert_eq!(
            log.record.event_name().expect("event_name is expected"),
            "log event"
        );

        // Validate trace context is none.
        assert!(log.record.trace_context().is_none());

        // Attributes can be polluted when we don't use this feature.
        #[cfg(feature = "experimental_metadata_attributes")]
        assert_eq!(log.record.attributes_iter().count(), 4);

        #[cfg(feature = "experimental_metadata_attributes")]
        {
            assert!(attributes_contains(
                &log.record,
                &Key::new("code.filename"),
                &AnyValue::String("layer.rs".into())
            ));
            assert!(attributes_contains(
                &log.record,
                &Key::new("code.namespace"),
                &AnyValue::String("opentelemetry_appender_tracing::layer::tests".into())
            ));
            // The other 3 experimental_metadata_attributes are too unstable to check their value.
            // Ex.: The path will be different on a Windows and Linux machine.
            // Ex.: The line can change easily if someone makes changes in this source file.
            let attributes_key: Vec<Key> = log
                .record
                .attributes_iter()
                .map(|(key, _)| key.clone())
                .collect();
            assert!(attributes_key.contains(&Key::new("code.filepath")));
            assert!(attributes_key.contains(&Key::new("code.lineno")));
            assert!(!attributes_key.contains(&Key::new("log.target")));
        }
    }

    #[test]
    fn tracing_appender_inside_tracing_context_with_tracing_log() {
        // Arrange
        let exporter: InMemoryLogExporter = InMemoryLogExporter::default();
        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();

        let subscriber = create_tracing_subscriber(&logger_provider);

        // avoiding setting tracing subscriber as global as that does not
        // play well with unit tests.
        let _guard = tracing::subscriber::set_default(subscriber);
        drop(tracing_log::LogTracer::init());

        // setup tracing as well.
        let tracer_provider = SdkTracerProvider::builder()
            .with_sampler(Sampler::AlwaysOn)
            .build();
        let tracer = tracer_provider.tracer("test-tracer");

        // Act
        let (trace_id_expected, span_id_expected) = tracer.in_span("test-span", |cx| {
            let trace_id = cx.span().span_context().trace_id();
            let span_id = cx.span().span_context().span_id();

            // logging is done inside span context.
            log::error!(target: "my-system", "log from log crate");
            (trace_id, span_id)
        });

        assert!(logger_provider.force_flush().is_ok());

        // Assert TODO: move to helper methods
        let exported_logs = exporter
            .get_emitted_logs()
            .expect("Logs are expected to be exported.");
        assert_eq!(exported_logs.len(), 1);
        let log = exported_logs
            .first()
            .expect("Atleast one log is expected to be present.");

        // validate common fields.
        assert_eq!(log.instrumentation.name(), "");
        assert_eq!(log.record.severity_number(), Some(Severity::Error));

        // validate trace context.
        assert!(log.record.trace_context().is_some());
        assert_eq!(
            log.record.trace_context().unwrap().trace_id,
            trace_id_expected
        );
        assert_eq!(
            log.record.trace_context().unwrap().span_id,
            span_id_expected
        );
        assert_eq!(
            log.record.trace_context().unwrap().trace_flags.unwrap(),
            TraceFlags::SAMPLED
        );

        for attribute in log.record.attributes_iter() {
            println!("key: {:?}, value: {:?}", attribute.0, attribute.1);
        }

        // Attributes can be polluted when we don't use this feature.
        #[cfg(feature = "experimental_metadata_attributes")]
        assert_eq!(log.record.attributes_iter().count(), 4);

        #[cfg(feature = "experimental_metadata_attributes")]
        {
            assert!(attributes_contains(
                &log.record,
                &Key::new("code.filename"),
                &AnyValue::String("layer.rs".into())
            ));
            assert!(attributes_contains(
                &log.record,
                &Key::new("code.namespace"),
                &AnyValue::String("opentelemetry_appender_tracing::layer::tests".into())
            ));
            // The other 3 experimental_metadata_attributes are too unstable to check their value.
            // Ex.: The path will be different on a Windows and Linux machine.
            // Ex.: The line can change easily if someone makes changes in this source file.
            let attributes_key: Vec<Key> = log
                .record
                .attributes_iter()
                .map(|(key, _)| key.clone())
                .collect();
            assert!(attributes_key.contains(&Key::new("code.filepath")));
            assert!(attributes_key.contains(&Key::new("code.lineno")));
            assert!(!attributes_key.contains(&Key::new("log.target")));
        }
    }

    #[derive(Debug)]
    struct LogProcessorWithIsEnabled {
        severity_level: Severity,
        name: String,
        target: String,
    }

    impl LogProcessorWithIsEnabled {
        fn new(severity_level: Severity, name: String, target: String) -> Self {
            LogProcessorWithIsEnabled {
                severity_level,
                name,
                target,
            }
        }
    }

    impl LogProcessor for LogProcessorWithIsEnabled {
        fn emit(&self, _record: &mut SdkLogRecord, _scope: &InstrumentationScope) {
            // no-op
        }

        #[cfg(feature = "spec_unstable_logs_enabled")]
        fn event_enabled(&self, level: Severity, target: &str, name: Option<&str>) -> bool {
            // assert that passed in arguments are same as the ones set in the test.
            assert_eq!(self.severity_level, level);
            assert_eq!(self.target, target);
            assert_eq!(
                self.name,
                name.expect("name is expected from tracing appender")
            );
            true
        }

        fn force_flush(&self) -> OTelSdkResult {
            Ok(())
        }
    }

    #[cfg(feature = "spec_unstable_logs_enabled")]
    #[test]
    fn is_enabled() {
        // Arrange
        let logger_provider = SdkLoggerProvider::builder()
            .with_log_processor(LogProcessorWithIsEnabled::new(
                Severity::Error,
                "my-event-name".to_string(),
                "my-system".to_string(),
            ))
            .build();

        let subscriber = create_tracing_subscriber(&logger_provider);

        // avoiding setting tracing subscriber as global as that does not
        // play well with unit tests.
        let _guard = tracing::subscriber::set_default(subscriber);

        // Name, Target and Severity are expected to be passed to the IsEnabled check
        // The validation is done in the LogProcessorWithIsEnabled struct.
        error!(name: "my-event-name", target: "my-system", event_id = 20, user_name = "otel", user_email = "otel@opentelemetry.io");
    }
}
