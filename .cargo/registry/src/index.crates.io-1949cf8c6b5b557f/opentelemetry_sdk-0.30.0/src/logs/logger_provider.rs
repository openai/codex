use super::{BatchLogProcessor, LogProcessor, SdkLogger, SimpleLogProcessor};
use crate::error::{OTelSdkError, OTelSdkResult};
use crate::logs::LogExporter;
use crate::Resource;
use opentelemetry::{otel_debug, otel_info, InstrumentationScope};
use std::time::Duration;
use std::{
    borrow::Cow,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
};

// a no nop logger provider used as placeholder when the provider is shutdown
// TODO - replace it with LazyLock once it is stable
static NOOP_LOGGER_PROVIDER: OnceLock<SdkLoggerProvider> = OnceLock::new();

#[inline]
fn noop_logger_provider() -> &'static SdkLoggerProvider {
    NOOP_LOGGER_PROVIDER.get_or_init(|| SdkLoggerProvider {
        inner: Arc::new(LoggerProviderInner {
            processors: Vec::new(),
            is_shutdown: AtomicBool::new(true),
        }),
    })
}

#[derive(Debug, Clone)]
/// Handles the creation and coordination of [`Logger`]s.
///
/// All `Logger`s created by a `SdkLoggerProvider` will share the same
/// [`Resource`] and have their created log records processed by the
/// configured log processors. This is a clonable handle to the `SdkLoggerProvider`
/// itself, and cloning it will create a new reference, not a new instance of a
/// `SdkLoggerProvider`. Dropping the last reference will trigger the shutdown of
/// the provider, ensuring that all remaining logs are flushed and no further
/// logs are processed. Shutdown can also be triggered manually by calling
/// the [`shutdown`](SdkLoggerProvider::shutdown) method.
///
/// [`Logger`]: opentelemetry::logs::Logger
/// [`Resource`]: crate::Resource
pub struct SdkLoggerProvider {
    inner: Arc<LoggerProviderInner>,
}

impl opentelemetry::logs::LoggerProvider for SdkLoggerProvider {
    type Logger = SdkLogger;

    fn logger(&self, name: impl Into<Cow<'static, str>>) -> Self::Logger {
        let scope = InstrumentationScope::builder(name).build();
        self.logger_with_scope(scope)
    }

    fn logger_with_scope(&self, scope: InstrumentationScope) -> Self::Logger {
        // If the provider is shutdown, new logger will refer a no-op logger provider.
        if self.inner.is_shutdown.load(Ordering::Relaxed) {
            otel_debug!(
                name: "LoggerProvider.NoOpLoggerReturned",
                logger_name = scope.name(),
            );
            return SdkLogger::new(scope, noop_logger_provider().clone());
        }
        if scope.name().is_empty() {
            otel_info!(name: "LoggerNameEmpty",  message = "Logger name is empty; consider providing a meaningful name. Logger will function normally and the provided name will be used as-is.");
        };
        otel_debug!(
            name: "LoggerProvider.NewLoggerReturned",
            logger_name = scope.name(),
        );
        SdkLogger::new(scope, self.clone())
    }
}

impl SdkLoggerProvider {
    /// Create a new `LoggerProvider` builder.
    pub fn builder() -> LoggerProviderBuilder {
        LoggerProviderBuilder::default()
    }

    pub(crate) fn log_processors(&self) -> &[Box<dyn LogProcessor>] {
        &self.inner.processors
    }

    /// Force flush all remaining logs in log processors and return results.
    pub fn force_flush(&self) -> OTelSdkResult {
        let result: Vec<_> = self
            .log_processors()
            .iter()
            .map(|processor| processor.force_flush())
            .collect();
        if result.iter().all(|r| r.is_ok()) {
            Ok(())
        } else {
            Err(OTelSdkError::InternalFailure(format!("errs: {:?}", result)))
        }
    }

    /// Shuts down this `LoggerProvider`
    pub fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        otel_debug!(
            name: "LoggerProvider.ShutdownInvokedByUser",
        );
        if self
            .inner
            .is_shutdown
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // propagate the shutdown signal to processors
            let result = self.inner.shutdown_with_timeout(timeout);
            if result.iter().all(|res| res.is_ok()) {
                Ok(())
            } else {
                Err(OTelSdkError::InternalFailure(format!(
                    "Shutdown errors: {:?}",
                    result
                        .into_iter()
                        .filter_map(Result::err)
                        .collect::<Vec<_>>()
                )))
            }
        } else {
            Err(OTelSdkError::AlreadyShutdown)
        }
    }

    /// Shuts down this `LoggerProvider` with default timeout
    pub fn shutdown(&self) -> OTelSdkResult {
        self.shutdown_with_timeout(Duration::from_secs(5))
    }
}

#[derive(Debug)]
struct LoggerProviderInner {
    processors: Vec<Box<dyn LogProcessor>>,
    is_shutdown: AtomicBool,
}

impl LoggerProviderInner {
    /// Shuts down the `LoggerProviderInner` and returns any errors.
    pub(crate) fn shutdown_with_timeout(&self, timeout: Duration) -> Vec<OTelSdkResult> {
        let mut results = vec![];
        for processor in &self.processors {
            let result = processor.shutdown_with_timeout(timeout);
            if let Err(err) = &result {
                // Log at debug level because:
                //  - The error is also returned to the user for handling (if applicable)
                //  - Or the error occurs during `TracerProviderInner::Drop` as part of telemetry shutdown,
                //    which is non-actionable by the user
                otel_debug!(name: "LoggerProvider.ShutdownError",
                        error = format!("{err}"));
            }
            results.push(result);
        }
        results
    }

    /// Shuts down the `LoggerProviderInner` with default timeout and returns any errors.
    pub(crate) fn shutdown(&self) -> Vec<OTelSdkResult> {
        self.shutdown_with_timeout(Duration::from_secs(5))
    }
}

impl Drop for LoggerProviderInner {
    fn drop(&mut self) {
        if !self.is_shutdown.load(Ordering::Relaxed) {
            otel_info!(
                name: "LoggerProvider.Drop",
                message = "Last reference of LoggerProvider dropped, initiating shutdown."
            );
            let _ = self.shutdown(); // errors are handled within shutdown
        } else {
            otel_debug!(
                name: "LoggerProvider.Drop.AlreadyShutdown",
                message = "LoggerProvider was already shut down; drop will not attempt shutdown again."
            );
        }
    }
}

#[derive(Debug, Default)]
/// Builder for provider attributes.
pub struct LoggerProviderBuilder {
    processors: Vec<Box<dyn LogProcessor>>,
    resource: Option<Resource>,
}

impl LoggerProviderBuilder {
    /// Adds a [SimpleLogProcessor] with the configured exporter to the pipeline.
    ///
    /// # Arguments
    ///
    /// * `exporter` - The exporter to be used by the SimpleLogProcessor.
    ///
    /// # Returns
    ///
    /// A new `Builder` instance with the SimpleLogProcessor added to the pipeline.
    ///
    /// Processors are invoked in the order they are added.
    pub fn with_simple_exporter<T: LogExporter + 'static>(self, exporter: T) -> Self {
        let mut processors = self.processors;
        processors.push(Box::new(SimpleLogProcessor::new(exporter)));

        LoggerProviderBuilder { processors, ..self }
    }

    /// Adds a [BatchLogProcessor] with the configured exporter to the pipeline,
    /// using the default [super::BatchConfig].
    ///
    /// The following environment variables can be used to configure the batching configuration:
    ///
    /// * `OTEL_BLRP_SCHEDULE_DELAY` - Corresponds to `with_scheduled_delay`.
    /// * `OTEL_BLRP_MAX_QUEUE_SIZE` - Corresponds to `with_max_queue_size`.
    /// * `OTEL_BLRP_MAX_EXPORT_BATCH_SIZE` - Corresponds to `with_max_export_batch_size`.
    ///
    /// # Arguments
    ///
    /// * `exporter` - The exporter to be used by the `BatchLogProcessor`.
    ///
    /// # Returns
    ///
    /// A new `LoggerProviderBuilder` instance with the `BatchLogProcessor` added to the pipeline.
    ///
    /// Processors are invoked in the order they are added.
    pub fn with_batch_exporter<T: LogExporter + 'static>(self, exporter: T) -> Self {
        let batch = BatchLogProcessor::builder(exporter).build();
        self.with_log_processor(batch)
    }

    /// Adds a custom [LogProcessor] to the pipeline.
    ///
    /// # Arguments
    ///
    /// * `processor` - The `LogProcessor` to be added.
    ///
    /// # Returns
    ///
    /// A new `Builder` instance with the custom `LogProcessor` added to the pipeline.
    ///
    /// Processors are invoked in the order they are added.
    pub fn with_log_processor<T: LogProcessor + 'static>(self, processor: T) -> Self {
        let mut processors = self.processors;
        processors.push(Box::new(processor));

        LoggerProviderBuilder { processors, ..self }
    }

    /// The `Resource` to be associated with this Provider.
    ///
    /// *Note*: Calls to this method are additive, each call merges the provided
    /// resource with the previous one.
    pub fn with_resource(self, resource: Resource) -> Self {
        let resource = match self.resource {
            Some(existing) => Some(existing.merge(&resource)),
            None => Some(resource),
        };

        LoggerProviderBuilder { resource, ..self }
    }

    /// Create a new provider from this configuration.
    pub fn build(self) -> SdkLoggerProvider {
        let resource = self.resource.unwrap_or(Resource::builder().build());
        let mut processors = self.processors;
        for processor in &mut processors {
            processor.set_resource(&resource);
        }

        let logger_provider = SdkLoggerProvider {
            inner: Arc::new(LoggerProviderInner {
                processors,
                is_shutdown: AtomicBool::new(false),
            }),
        };

        otel_debug!(
            name: "LoggerProvider.Built",
        );
        logger_provider
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        logs::{InMemoryLogExporter, LogBatch, SdkLogRecord, TraceContext},
        resource::{
            SERVICE_NAME, TELEMETRY_SDK_LANGUAGE, TELEMETRY_SDK_NAME, TELEMETRY_SDK_VERSION,
        },
        trace::SdkTracerProvider,
        Resource,
    };

    use super::*;
    use opentelemetry::trace::{SpanId, TraceId, Tracer as _, TracerProvider};
    use opentelemetry::{
        logs::{AnyValue, LogRecord as _, Logger, LoggerProvider},
        trace::TraceContextExt,
    };
    use opentelemetry::{Key, KeyValue, Value};
    use std::fmt::{Debug, Formatter};
    use std::sync::atomic::AtomicU64;
    use std::sync::Mutex;
    use std::{thread, time};

    struct ShutdownTestLogProcessor {
        is_shutdown: Arc<Mutex<bool>>,
        counter: Arc<AtomicU64>,
    }

    impl Debug for ShutdownTestLogProcessor {
        fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
            todo!()
        }
    }

    impl ShutdownTestLogProcessor {
        pub(crate) fn new(counter: Arc<AtomicU64>) -> Self {
            ShutdownTestLogProcessor {
                is_shutdown: Arc::new(Mutex::new(false)),
                counter,
            }
        }
    }

    impl LogProcessor for ShutdownTestLogProcessor {
        fn emit(&self, _data: &mut SdkLogRecord, _scope: &InstrumentationScope) {
            self.is_shutdown
                .lock()
                .map(|is_shutdown| {
                    if !*is_shutdown {
                        self.counter
                            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                })
                .expect("lock poisoned");
        }

        fn force_flush(&self) -> OTelSdkResult {
            Ok(())
        }

        fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
            self.is_shutdown
                .lock()
                .map(|mut is_shutdown| *is_shutdown = true)
                .expect("lock poisoned");
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    struct TestExporterForResource {
        resource: Arc<Mutex<Resource>>,
    }
    impl TestExporterForResource {
        fn new() -> Self {
            TestExporterForResource {
                resource: Arc::new(Mutex::new(Resource::empty())),
            }
        }

        fn resource(&self) -> Resource {
            self.resource.lock().unwrap().clone()
        }
    }
    impl LogExporter for TestExporterForResource {
        async fn export(&self, _: LogBatch<'_>) -> OTelSdkResult {
            Ok(())
        }

        fn set_resource(&mut self, resource: &Resource) {
            let mut res = self.resource.lock().unwrap();
            *res = resource.clone();
        }

        fn shutdown_with_timeout(&self, _timeout: time::Duration) -> OTelSdkResult {
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    struct TestProcessorForResource {
        resource: Arc<Mutex<Resource>>,
        exporter: TestExporterForResource,
    }
    impl LogProcessor for TestProcessorForResource {
        fn emit(&self, _data: &mut SdkLogRecord, _scope: &InstrumentationScope) {
            // nothing to do.
        }

        fn force_flush(&self) -> OTelSdkResult {
            Ok(())
        }

        fn set_resource(&mut self, resource: &Resource) {
            let mut res = self.resource.lock().unwrap();
            *res = resource.clone();
            self.exporter.set_resource(resource);
        }
    }
    impl TestProcessorForResource {
        fn new(exporter: TestExporterForResource) -> Self {
            TestProcessorForResource {
                resource: Arc::new(Mutex::new(Resource::empty())),
                exporter,
            }
        }
        fn resource(&self) -> Resource {
            self.resource.lock().unwrap().clone()
        }
    }

    #[test]
    fn test_resource_handling_provider_processor_exporter() {
        let assert_resource = |processor: &TestProcessorForResource,
                               exporter: &TestExporterForResource,
                               resource_key: &'static str,
                               expect: Option<&'static str>| {
            assert_eq!(
                processor
                    .resource()
                    .get(&Key::from_static_str(resource_key))
                    .map(|v| v.to_string()),
                expect.map(|s| s.to_string())
            );

            assert_eq!(
                exporter
                    .resource()
                    .get(&Key::from_static_str(resource_key))
                    .map(|v| v.to_string()),
                expect.map(|s| s.to_string())
            );
        };
        let assert_telemetry_resource =
            |processor: &TestProcessorForResource, exporter: &TestExporterForResource| {
                assert_eq!(
                    processor.resource().get(&TELEMETRY_SDK_LANGUAGE.into()),
                    Some(Value::from("rust"))
                );
                assert_eq!(
                    processor.resource().get(&TELEMETRY_SDK_NAME.into()),
                    Some(Value::from("opentelemetry"))
                );
                assert_eq!(
                    processor.resource().get(&TELEMETRY_SDK_VERSION.into()),
                    Some(Value::from(env!("CARGO_PKG_VERSION")))
                );
                assert_eq!(
                    exporter.resource().get(&TELEMETRY_SDK_LANGUAGE.into()),
                    Some(Value::from("rust"))
                );
                assert_eq!(
                    exporter.resource().get(&TELEMETRY_SDK_NAME.into()),
                    Some(Value::from("opentelemetry"))
                );
                assert_eq!(
                    exporter.resource().get(&TELEMETRY_SDK_VERSION.into()),
                    Some(Value::from(env!("CARGO_PKG_VERSION")))
                );
            };

        // If users didn't provide a resource and there isn't a env var set. Use default one.
        temp_env::with_var_unset("OTEL_RESOURCE_ATTRIBUTES", || {
            let exporter_with_resource = TestExporterForResource::new();
            let processor_with_resource =
                TestProcessorForResource::new(exporter_with_resource.clone());
            let _ = super::SdkLoggerProvider::builder()
                .with_log_processor(processor_with_resource.clone())
                .build();
            assert_resource(
                &processor_with_resource,
                &exporter_with_resource,
                SERVICE_NAME,
                Some("unknown_service"),
            );
            assert_telemetry_resource(&processor_with_resource, &exporter_with_resource);
        });

        // If user provided a resource, use that.
        let exporter_with_resource = TestExporterForResource::new();
        let processor_with_resource = TestProcessorForResource::new(exporter_with_resource.clone());
        let _ = super::SdkLoggerProvider::builder()
            .with_resource(
                Resource::builder_empty()
                    .with_service_name("test_service")
                    .build(),
            )
            .with_log_processor(processor_with_resource.clone())
            .build();
        assert_resource(
            &processor_with_resource,
            &exporter_with_resource,
            SERVICE_NAME,
            Some("test_service"),
        );
        assert_eq!(processor_with_resource.resource().len(), 1);

        // If `OTEL_RESOURCE_ATTRIBUTES` is set, read them automatically
        temp_env::with_var(
            "OTEL_RESOURCE_ATTRIBUTES",
            Some("key1=value1, k2, k3=value2"),
            || {
                let exporter_with_resource = TestExporterForResource::new();
                let processor_with_resource =
                    TestProcessorForResource::new(exporter_with_resource.clone());
                let _ = super::SdkLoggerProvider::builder()
                    .with_log_processor(processor_with_resource.clone())
                    .build();
                assert_resource(
                    &processor_with_resource,
                    &exporter_with_resource,
                    SERVICE_NAME,
                    Some("unknown_service"),
                );
                assert_resource(
                    &processor_with_resource,
                    &exporter_with_resource,
                    "key1",
                    Some("value1"),
                );
                assert_resource(
                    &processor_with_resource,
                    &exporter_with_resource,
                    "k3",
                    Some("value2"),
                );
                assert_telemetry_resource(&processor_with_resource, &exporter_with_resource);
                assert_eq!(processor_with_resource.resource().len(), 6);
            },
        );

        // When `OTEL_RESOURCE_ATTRIBUTES` is set and also user provided config
        temp_env::with_var(
            "OTEL_RESOURCE_ATTRIBUTES",
            Some("my-custom-key=env-val,k2=value2"),
            || {
                let exporter_with_resource = TestExporterForResource::new();
                let processor_with_resource =
                    TestProcessorForResource::new(exporter_with_resource.clone());
                let _ = super::SdkLoggerProvider::builder()
                    .with_resource(
                        Resource::builder()
                            .with_attributes([
                                KeyValue::new("my-custom-key", "my-custom-value"),
                                KeyValue::new("my-custom-key2", "my-custom-value2"),
                            ])
                            .build(),
                    )
                    .with_log_processor(processor_with_resource.clone())
                    .build();
                assert_resource(
                    &processor_with_resource,
                    &exporter_with_resource,
                    SERVICE_NAME,
                    Some("unknown_service"),
                );
                assert_resource(
                    &processor_with_resource,
                    &exporter_with_resource,
                    "my-custom-key",
                    Some("my-custom-value"),
                );
                assert_resource(
                    &processor_with_resource,
                    &exporter_with_resource,
                    "my-custom-key2",
                    Some("my-custom-value2"),
                );
                assert_resource(
                    &processor_with_resource,
                    &exporter_with_resource,
                    "k2",
                    Some("value2"),
                );
                assert_telemetry_resource(&processor_with_resource, &exporter_with_resource);
                assert_eq!(processor_with_resource.resource().len(), 7);
            },
        );

        // If user provided a resource, it takes priority during collision.
        let exporter_with_resource = TestExporterForResource::new();
        let processor_with_resource = TestProcessorForResource::new(exporter_with_resource);
        let _ = super::SdkLoggerProvider::builder()
            .with_resource(Resource::empty())
            .with_log_processor(processor_with_resource.clone())
            .build();
        assert_eq!(processor_with_resource.resource().len(), 0);
    }

    #[test]
    fn trace_context_test() {
        let exporter = InMemoryLogExporter::default();

        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();

        let logger = logger_provider.logger("test-logger");

        let tracer_provider = SdkTracerProvider::builder().build();

        let tracer = tracer_provider.tracer("test-tracer");

        tracer.in_span("test-span", |cx| {
            let ambient_ctxt = cx.span().span_context().clone();
            let explicit_ctxt = TraceContext {
                trace_id: TraceId::from_u128(13),
                span_id: SpanId::from_u64(14),
                trace_flags: None,
            };

            let mut ambient_ctxt_record = logger.create_log_record();
            ambient_ctxt_record.set_body(AnyValue::String("ambient".into()));

            let mut explicit_ctxt_record = logger.create_log_record();
            explicit_ctxt_record.set_body(AnyValue::String("explicit".into()));
            explicit_ctxt_record.set_trace_context(
                explicit_ctxt.trace_id,
                explicit_ctxt.span_id,
                explicit_ctxt.trace_flags,
            );

            logger.emit(ambient_ctxt_record);
            logger.emit(explicit_ctxt_record);

            let emitted = exporter.get_emitted_logs().unwrap();

            assert_eq!(
                Some(AnyValue::String("ambient".into())),
                emitted[0].record.body
            );
            assert_eq!(
                ambient_ctxt.trace_id(),
                emitted[0].record.trace_context.as_ref().unwrap().trace_id
            );
            assert_eq!(
                ambient_ctxt.span_id(),
                emitted[0].record.trace_context.as_ref().unwrap().span_id
            );

            assert_eq!(
                Some(AnyValue::String("explicit".into())),
                emitted[1].record.body
            );
            assert_eq!(
                explicit_ctxt.trace_id,
                emitted[1].record.trace_context.as_ref().unwrap().trace_id
            );
            assert_eq!(
                explicit_ctxt.span_id,
                emitted[1].record.trace_context.as_ref().unwrap().span_id
            );
        });
    }

    #[test]
    fn shutdown_test() {
        let counter = Arc::new(AtomicU64::new(0));
        let logger_provider = SdkLoggerProvider::builder()
            .with_log_processor(ShutdownTestLogProcessor::new(counter.clone()))
            .build();

        let logger1 = logger_provider.logger("test-logger1");
        let logger2 = logger_provider.logger("test-logger2");
        logger1.emit(logger1.create_log_record());
        logger2.emit(logger1.create_log_record());

        let logger3 = logger_provider.logger("test-logger3");
        let handle = thread::spawn(move || {
            logger3.emit(logger3.create_log_record());
        });
        handle.join().expect("thread panicked");

        let _ = logger_provider.shutdown();
        logger1.emit(logger1.create_log_record());

        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[test]
    fn shutdown_idempotent_test() {
        let counter = Arc::new(AtomicU64::new(0));
        let logger_provider = SdkLoggerProvider::builder()
            .with_log_processor(ShutdownTestLogProcessor::new(counter.clone()))
            .build();

        let shutdown_res = logger_provider.shutdown();
        assert!(shutdown_res.is_ok());

        // Subsequent shutdowns should return an error.
        let shutdown_res = logger_provider.shutdown();
        assert!(shutdown_res.is_err());

        // Subsequent shutdowns should return an error.
        let shutdown_res = logger_provider.shutdown();
        assert!(shutdown_res.is_err());
    }

    #[test]
    fn global_shutdown_test() {
        // cargo test global_shutdown_test --features=testing

        // Arrange
        let shutdown_called = Arc::new(Mutex::new(false));
        let flush_called = Arc::new(Mutex::new(false));
        let logger_provider = SdkLoggerProvider::builder()
            .with_log_processor(LazyLogProcessor::new(
                shutdown_called.clone(),
                flush_called.clone(),
            ))
            .build();
        //set_logger_provider(logger_provider);
        let logger1 = logger_provider.logger("test-logger1");
        let logger2 = logger_provider.logger("test-logger2");

        // Acts
        logger1.emit(logger1.create_log_record());
        logger2.emit(logger1.create_log_record());

        // explicitly calling shutdown on logger_provider. This will
        // indeed do the shutdown, even if there are loggers still alive.
        let _ = logger_provider.shutdown();

        // Assert

        // shutdown is called.
        assert!(*shutdown_called.lock().unwrap());

        // flush is never called by the sdk.
        assert!(!*flush_called.lock().unwrap());
    }

    #[test]
    fn drop_test_with_multiple_providers() {
        let shutdown_called = Arc::new(Mutex::new(false));
        let flush_called = Arc::new(Mutex::new(false));
        {
            // Create a shared LoggerProviderInner and use it across multiple providers
            let shared_inner = Arc::new(LoggerProviderInner {
                processors: vec![Box::new(LazyLogProcessor::new(
                    shutdown_called.clone(),
                    flush_called.clone(),
                ))],
                is_shutdown: AtomicBool::new(false),
            });

            {
                let logger_provider1 = SdkLoggerProvider {
                    inner: shared_inner.clone(),
                };
                let logger_provider2 = SdkLoggerProvider {
                    inner: shared_inner.clone(),
                };

                let logger1 = logger_provider1.logger("test-logger1");
                let logger2 = logger_provider2.logger("test-logger2");

                logger1.emit(logger1.create_log_record());
                logger2.emit(logger1.create_log_record());

                // LoggerProviderInner should not be dropped yet, since both providers and `shared_inner`
                // are still holding a reference.
            }
            // At this point, both `logger_provider1` and `logger_provider2` are dropped,
            // but `shared_inner` still holds a reference, so `LoggerProviderInner` is NOT dropped yet.
        }
        // Verify shutdown was called during the drop of the shared LoggerProviderInner
        assert!(*shutdown_called.lock().unwrap());
        // Verify flush was not called during drop
        assert!(!*flush_called.lock().unwrap());
    }

    #[test]
    fn drop_after_shutdown_test_with_multiple_providers() {
        let shutdown_called = Arc::new(Mutex::new(0)); // Count the number of times shutdown is called
        let flush_called = Arc::new(Mutex::new(false));

        // Create a shared LoggerProviderInner and use it across multiple providers
        let shared_inner = Arc::new(LoggerProviderInner {
            processors: vec![Box::new(CountingShutdownProcessor::new(
                shutdown_called.clone(),
                flush_called.clone(),
            ))],
            is_shutdown: AtomicBool::new(false),
        });

        // Create a scope to test behavior when providers are dropped
        {
            let logger_provider1 = SdkLoggerProvider {
                inner: shared_inner.clone(),
            };
            let logger_provider2 = SdkLoggerProvider {
                inner: shared_inner.clone(),
            };

            // Explicitly shut down the logger provider
            let shutdown_result = logger_provider1.shutdown();
            println!("---->Result: {:?}", shutdown_result);
            assert!(shutdown_result.is_ok());

            // Verify that shutdown was called exactly once
            assert_eq!(*shutdown_called.lock().unwrap(), 1);

            // LoggerProvider2 should observe the shutdown state but not trigger another shutdown
            let shutdown_result2 = logger_provider2.shutdown();
            assert!(shutdown_result2.is_err());

            // Both logger providers will be dropped at the end of this scope
        }

        // Verify that shutdown was only called once, even after drop
        assert_eq!(*shutdown_called.lock().unwrap(), 1);
    }

    #[test]
    fn test_empty_logger_name() {
        let exporter = InMemoryLogExporter::default();
        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        let logger = logger_provider.logger("");
        let mut record = logger.create_log_record();
        record.set_body("Testing empty logger name".into());
        logger.emit(record);

        // Create a logger using a scope with an empty name
        let scope = InstrumentationScope::builder("").build();
        let scoped_logger = logger_provider.logger_with_scope(scope);
        let mut scoped_record = scoped_logger.create_log_record();
        scoped_record.set_body("Testing empty logger scope name".into());
        scoped_logger.emit(scoped_record);

        // Assert: Verify that the emitted logs are processed correctly
        let mut emitted_logs = exporter.get_emitted_logs().unwrap();
        assert_eq!(emitted_logs.len(), 2);
        let log1 = emitted_logs.remove(0);
        // Assert the first log
        assert_eq!(
            log1.record.body,
            Some(AnyValue::String("Testing empty logger name".into()))
        );
        assert_eq!(log1.instrumentation.name(), "");

        // Assert the second log created through the scope
        let log2 = emitted_logs.remove(0);
        assert_eq!(
            log2.record.body,
            Some(AnyValue::String("Testing empty logger scope name".into()))
        );
        assert_eq!(log1.instrumentation.name(), "");
    }

    #[test]
    fn with_resource_multiple_calls_ensure_additive() {
        let builder = SdkLoggerProvider::builder()
            .with_resource(Resource::new(vec![KeyValue::new("key1", "value1")]))
            .with_resource(Resource::new(vec![KeyValue::new("key2", "value2")]))
            .with_resource(
                Resource::builder_empty()
                    .with_schema_url(vec![], "http://example.com")
                    .build(),
            )
            .with_resource(Resource::new(vec![KeyValue::new("key3", "value3")]));

        let resource = builder.resource.unwrap();

        assert_eq!(
            resource.get(&Key::from_static_str("key1")),
            Some(Value::from("value1"))
        );
        assert_eq!(
            resource.get(&Key::from_static_str("key2")),
            Some(Value::from("value2"))
        );
        assert_eq!(
            resource.get(&Key::from_static_str("key3")),
            Some(Value::from("value3"))
        );
        assert_eq!(resource.schema_url(), Some("http://example.com"));
    }

    #[derive(Debug)]
    pub(crate) struct LazyLogProcessor {
        shutdown_called: Arc<Mutex<bool>>,
        flush_called: Arc<Mutex<bool>>,
    }

    impl LazyLogProcessor {
        pub(crate) fn new(
            shutdown_called: Arc<Mutex<bool>>,
            flush_called: Arc<Mutex<bool>>,
        ) -> Self {
            LazyLogProcessor {
                shutdown_called,
                flush_called,
            }
        }
    }

    impl LogProcessor for LazyLogProcessor {
        fn emit(&self, _data: &mut SdkLogRecord, _scope: &InstrumentationScope) {
            // nothing to do.
        }

        fn force_flush(&self) -> OTelSdkResult {
            *self.flush_called.lock().unwrap() = true;
            Ok(())
        }

        fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
            *self.shutdown_called.lock().unwrap() = true;
            Ok(())
        }
    }

    #[derive(Debug)]
    struct CountingShutdownProcessor {
        shutdown_count: Arc<Mutex<i32>>,
        flush_called: Arc<Mutex<bool>>,
    }

    impl CountingShutdownProcessor {
        fn new(shutdown_count: Arc<Mutex<i32>>, flush_called: Arc<Mutex<bool>>) -> Self {
            CountingShutdownProcessor {
                shutdown_count,
                flush_called,
            }
        }
    }

    impl LogProcessor for CountingShutdownProcessor {
        fn emit(&self, _data: &mut SdkLogRecord, _scope: &InstrumentationScope) {
            // nothing to do
        }

        fn force_flush(&self) -> OTelSdkResult {
            *self.flush_called.lock().unwrap() = true;
            Ok(())
        }

        fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
            let mut count = self.shutdown_count.lock().unwrap();
            *count += 1;
            Ok(())
        }
    }
}
