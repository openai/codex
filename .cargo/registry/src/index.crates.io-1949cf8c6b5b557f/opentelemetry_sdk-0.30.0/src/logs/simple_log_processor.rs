//! # OpenTelemetry Simple Log Processor
//! The `SimpleLogProcessor` is one implementation of the `LogProcessor` interface.
//!
//! It forwards log records to the exporter immediately after they are emitted
//! (or one exporter after another if applicable). This processor is
//! **synchronous** and is designed for debugging or testing purposes. It is
//! **not suitable for production** environments due to its lack of batching,
//! performance optimizations, or support for high-throughput scenarios.
//!
//! ## Diagram
//!
//! ```ascii
//!   +-----+---------------+   +-----------------------+   +-------------------+
//!   |     |               |   |                       |   |                   |
//!   | SDK | Logger.emit() +---> (Simple)LogProcessor  +--->  LogExporter      |
//!   +-----+---------------+   +-----------------------+   +-------------------+
//! ```

use crate::error::{OTelSdkError, OTelSdkResult};
use crate::logs::log_processor::LogProcessor;
use crate::{
    logs::{LogBatch, LogExporter, SdkLogRecord},
    Resource,
};

use opentelemetry::{otel_debug, otel_error, otel_warn, Context, InstrumentationScope};

use std::fmt::Debug;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

/// A [`LogProcessor`] designed for testing and debugging purpose, that immediately
/// exports log records as they are emitted. Log records are exported synchronously
/// in the same thread that emits the log record.
/// When using this processor with the OTLP Exporter, the following exporter
/// features are supported:
/// - `grpc-tonic`: This requires LoggerProvider to be created within a tokio
///   runtime. Logs can be emitted from any thread, including tokio runtime
///   threads.
/// - `reqwest-blocking-client`: LoggerProvider may be created anywhere, but
///   logs must be emitted from a non-tokio runtime thread.
/// - `reqwest-client`: LoggerProvider may be created anywhere, but logs must be
///   emitted from a tokio runtime thread.
///
/// ## Example
///
/// ### Using a SimpleLogProcessor
///
/// ```rust
/// use opentelemetry_sdk::logs::{SimpleLogProcessor, SdkLoggerProvider, LogExporter};
/// use opentelemetry::global;
/// use opentelemetry_sdk::logs::InMemoryLogExporter;
///
/// let exporter = InMemoryLogExporter::default(); // Replace with an actual exporter
/// let provider = SdkLoggerProvider::builder()
///     .with_simple_exporter(exporter)
///     .build();
///
/// ```
///
#[derive(Debug)]
pub struct SimpleLogProcessor<T: LogExporter> {
    exporter: Mutex<T>,
    is_shutdown: AtomicBool,
}

impl<T: LogExporter> SimpleLogProcessor<T> {
    /// Creates a new instance of `SimpleLogProcessor`.
    pub fn new(exporter: T) -> Self {
        SimpleLogProcessor {
            exporter: Mutex::new(exporter),
            is_shutdown: AtomicBool::new(false),
        }
    }
}

impl<T: LogExporter> LogProcessor for SimpleLogProcessor<T> {
    fn emit(&self, record: &mut SdkLogRecord, instrumentation: &InstrumentationScope) {
        let _suppress_guard = Context::enter_telemetry_suppressed_scope();
        // noop after shutdown
        if self.is_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            // this is a warning, as the user is trying to log after the processor has been shutdown
            otel_warn!(
                name: "SimpleLogProcessor.Emit.ProcessorShutdown",
            );
            return;
        }

        let result = self
            .exporter
            .lock()
            .map_err(|_| OTelSdkError::InternalFailure("SimpleLogProcessor mutex poison".into()))
            .and_then(|exporter| {
                let log_tuple = &[(record as &SdkLogRecord, instrumentation)];
                futures_executor::block_on(exporter.export(LogBatch::new(log_tuple)))
            });
        // Handle errors with specific static names
        match result {
            Err(OTelSdkError::InternalFailure(_)) => {
                // logging as debug as this is not a user error
                otel_debug!(
                    name: "SimpleLogProcessor.Emit.MutexPoisoning",
                );
            }
            Err(err) => {
                otel_error!(
                    name: "SimpleLogProcessor.Emit.ExportError",
                    error = format!("{}",err)
                );
            }
            _ => {}
        }
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        self.is_shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Ok(exporter) = self.exporter.lock() {
            exporter.shutdown()
        } else {
            Err(OTelSdkError::InternalFailure(
                "SimpleLogProcessor mutex poison at shutdown".into(),
            ))
        }
    }

    fn set_resource(&mut self, resource: &Resource) {
        if let Ok(mut exporter) = self.exporter.lock() {
            exporter.set_resource(resource);
        }
    }

    #[cfg(feature = "spec_unstable_logs_enabled")]
    #[inline]
    fn event_enabled(
        &self,
        level: opentelemetry::logs::Severity,
        target: &str,
        name: Option<&str>,
    ) -> bool {
        if let Ok(exporter) = self.exporter.lock() {
            exporter.event_enabled(level, target, name)
        } else {
            true
        }
    }
}

#[cfg(all(test, feature = "testing", feature = "logs"))]
mod tests {
    use crate::logs::log_processor::tests::MockLogExporter;
    use crate::logs::{LogBatch, LogExporter, SdkLogRecord, SdkLogger};
    use crate::{
        error::OTelSdkResult,
        logs::{InMemoryLogExporterBuilder, LogProcessor, SdkLoggerProvider, SimpleLogProcessor},
        Resource,
    };
    use opentelemetry::logs::{LogRecord, Logger, LoggerProvider};
    use opentelemetry::InstrumentationScope;
    use opentelemetry::KeyValue;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time;
    use std::time::Duration;

    #[derive(Debug, Clone)]
    struct LogExporterThatRequiresTokio {
        export_count: Arc<AtomicUsize>,
    }

    impl LogExporterThatRequiresTokio {
        /// Creates a new instance of `LogExporterThatRequiresTokio`.
        fn new() -> Self {
            LogExporterThatRequiresTokio {
                export_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        /// Returns the number of logs stored in the exporter.
        fn len(&self) -> usize {
            self.export_count.load(Ordering::Acquire)
        }
    }

    impl LogExporter for LogExporterThatRequiresTokio {
        async fn export(&self, batch: LogBatch<'_>) -> OTelSdkResult {
            // Simulate minimal dependency on tokio by sleeping asynchronously for a short duration
            tokio::time::sleep(Duration::from_millis(50)).await;

            for _ in batch.iter() {
                self.export_count.fetch_add(1, Ordering::Acquire);
            }
            Ok(())
        }
        fn shutdown_with_timeout(&self, _timeout: time::Duration) -> OTelSdkResult {
            Ok(())
        }
    }

    #[test]
    fn test_set_resource_simple_processor() {
        let exporter = MockLogExporter {
            resource: Arc::new(Mutex::new(None)),
        };
        let processor = SimpleLogProcessor::new(exporter.clone());
        let _ = SdkLoggerProvider::builder()
            .with_log_processor(processor)
            .with_resource(
                Resource::builder_empty()
                    .with_attributes([
                        KeyValue::new("k1", "v1"),
                        KeyValue::new("k2", "v3"),
                        KeyValue::new("k3", "v3"),
                        KeyValue::new("k4", "v4"),
                        KeyValue::new("k5", "v5"),
                    ])
                    .build(),
            )
            .build();
        assert_eq!(exporter.get_resource().unwrap().into_iter().count(), 5);
    }

    #[test]
    fn test_simple_shutdown() {
        let exporter = InMemoryLogExporterBuilder::default()
            .keep_records_on_shutdown()
            .build();
        let processor = SimpleLogProcessor::new(exporter.clone());

        let mut record: SdkLogRecord = SdkLogRecord::new();
        let instrumentation: InstrumentationScope = Default::default();

        processor.emit(&mut record, &instrumentation);

        processor.shutdown().unwrap();

        let is_shutdown = processor
            .is_shutdown
            .load(std::sync::atomic::Ordering::Relaxed);
        assert!(is_shutdown);

        processor.emit(&mut record, &instrumentation);

        assert_eq!(1, exporter.get_emitted_logs().unwrap().len());
        assert!(exporter.is_shutdown_called());
    }

    #[test]
    fn test_simple_processor_sync_exporter_without_runtime() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = SimpleLogProcessor::new(exporter.clone());

        let mut record: SdkLogRecord = SdkLogRecord::new();
        let instrumentation: InstrumentationScope = Default::default();

        processor.emit(&mut record, &instrumentation);

        assert_eq!(exporter.get_emitted_logs().unwrap().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_simple_processor_sync_exporter_with_runtime() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = SimpleLogProcessor::new(exporter.clone());

        let mut record: SdkLogRecord = SdkLogRecord::new();
        let instrumentation: InstrumentationScope = Default::default();

        processor.emit(&mut record, &instrumentation);

        assert_eq!(exporter.get_emitted_logs().unwrap().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_simple_processor_sync_exporter_with_multi_thread_runtime() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = Arc::new(SimpleLogProcessor::new(exporter.clone()));

        let mut handles = vec![];
        for _ in 0..10 {
            let processor_clone = Arc::clone(&processor);
            let handle = tokio::spawn(async move {
                let mut record: SdkLogRecord = SdkLogRecord::new();
                let instrumentation: InstrumentationScope = Default::default();
                processor_clone.emit(&mut record, &instrumentation);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        assert_eq!(exporter.get_emitted_logs().unwrap().len(), 10);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_simple_processor_sync_exporter_with_current_thread_runtime() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = SimpleLogProcessor::new(exporter.clone());

        let mut record: SdkLogRecord = SdkLogRecord::new();
        let instrumentation: InstrumentationScope = Default::default();

        processor.emit(&mut record, &instrumentation);

        assert_eq!(exporter.get_emitted_logs().unwrap().len(), 1);
    }

    #[test]
    fn test_simple_processor_async_exporter_without_runtime() {
        // Use `catch_unwind` to catch the panic caused by missing Tokio runtime
        let result = std::panic::catch_unwind(|| {
            let exporter = LogExporterThatRequiresTokio::new();
            let processor = SimpleLogProcessor::new(exporter.clone());

            let mut record: SdkLogRecord = SdkLogRecord::new();
            let instrumentation: InstrumentationScope = Default::default();

            // This will panic because an tokio async operation within exporter without a runtime.
            processor.emit(&mut record, &instrumentation);
        });

        // Verify that the panic occurred and check the panic message for the absence of a Tokio runtime
        assert!(
            result.is_err(),
            "The test should fail due to missing Tokio runtime, but it did not."
        );
        let panic_payload = result.unwrap_err();
        let panic_message = panic_payload
            .downcast_ref::<String>()
            .map(|s| s.as_str())
            .or_else(|| panic_payload.downcast_ref::<&str>().copied())
            .unwrap_or("No panic message");

        assert!(
            panic_message.contains("no reactor running")
                || panic_message.contains("must be called from the context of a Tokio 1.x runtime"),
            "Expected panic message about missing Tokio runtime, but got: {}",
            panic_message
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    // This test demonstrates a potential deadlock scenario in a multi-threaded Tokio runtime.
    // It spawns Tokio tasks equal to the number of runtime worker threads (4) to emit log events.
    // Each task attempts to acquire a mutex on the exporter in `SimpleLogProcessor::emit`.
    // Only one task obtains the lock, while the others are blocked, waiting for its release.
    //
    // The task holding the lock invokes the LogExporterThatRequiresTokio, which performs an
    // asynchronous operation (e.g., network I/O simulated by `tokio::sleep`). This operation
    // requires yielding control back to the Tokio runtime to make progress.
    //
    // However, all worker threads are occupied:
    // - One thread is executing the async exporter operation
    // - Three threads are blocked waiting for the mutex
    //
    // This leads to a deadlock as there are no available threads to drive the async operation
    // to completion, preventing the mutex from being released. Consequently, neither the blocked
    // tasks nor the exporter can proceed.
    async fn test_simple_processor_async_exporter_with_all_runtime_worker_threads_blocked() {
        let exporter = LogExporterThatRequiresTokio::new();
        let processor = Arc::new(SimpleLogProcessor::new(exporter.clone()));

        let concurrent_emit = 4; // number of worker threads

        let mut handles = vec![];
        // try send `concurrent_emit` events concurrently
        for _ in 0..concurrent_emit {
            let processor_clone = Arc::clone(&processor);
            let handle = tokio::spawn(async move {
                let mut record: SdkLogRecord = SdkLogRecord::new();
                let instrumentation: InstrumentationScope = Default::default();
                processor_clone.emit(&mut record, &instrumentation);
            });
            handles.push(handle);
        }

        // below code won't get executed
        for handle in handles {
            handle.await.unwrap();
        }
        assert_eq!(exporter.len(), concurrent_emit);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    // This test uses a multi-threaded runtime setup with a single worker thread. Note that even
    // though only one worker thread is created, it is distinct from the main thread. The processor
    // emits a log event, and the exporter performs an async operation that requires the runtime.
    // The single worker thread handles this operation without deadlocking, as long as no other
    // tasks occupy the runtime.
    async fn test_simple_processor_async_exporter_with_runtime() {
        let exporter = LogExporterThatRequiresTokio::new();
        let processor = SimpleLogProcessor::new(exporter.clone());

        let mut record: SdkLogRecord = SdkLogRecord::new();
        let instrumentation: InstrumentationScope = Default::default();

        processor.emit(&mut record, &instrumentation);

        assert_eq!(exporter.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    // This test uses a multi-threaded runtime setup with the default number of worker threads.
    // The processor emits a log event, and the exporter, which requires the runtime for its async
    // operations, can access one of the available worker threads to complete its task. As there
    // are multiple threads, the exporter can proceed without blocking other tasks, ensuring the
    // test completes successfully.
    async fn test_simple_processor_async_exporter_with_multi_thread_runtime() {
        let exporter = LogExporterThatRequiresTokio::new();

        let processor = SimpleLogProcessor::new(exporter.clone());

        let mut record: SdkLogRecord = SdkLogRecord::new();
        let instrumentation: InstrumentationScope = Default::default();

        processor.emit(&mut record, &instrumentation);

        assert_eq!(exporter.len(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore]
    // This test uses a current-thread runtime, where all operations run on the main thread.
    // The processor emits a log event while the runtime is blocked using `futures::block_on`
    // to complete the export operation. The exporter, which performs an async operation and
    // requires the runtime, cannot progress because the main thread is already blocked.
    // This results in a deadlock, as the runtime cannot move forward.
    async fn test_simple_processor_async_exporter_with_current_thread_runtime() {
        let exporter = LogExporterThatRequiresTokio::new();

        let processor = SimpleLogProcessor::new(exporter.clone());

        let mut record: SdkLogRecord = SdkLogRecord::new();
        let instrumentation: InstrumentationScope = Default::default();

        processor.emit(&mut record, &instrumentation);

        assert_eq!(exporter.len(), 1);
    }

    #[derive(Debug, Clone)]
    struct ReentrantLogExporter {
        logger: Arc<Mutex<Option<SdkLogger>>>,
    }

    impl ReentrantLogExporter {
        fn new() -> Self {
            Self {
                logger: Arc::new(Mutex::new(None)),
            }
        }

        fn set_logger(&self, logger: SdkLogger) {
            let mut guard = self.logger.lock().unwrap();
            *guard = Some(logger);
        }
    }

    impl LogExporter for ReentrantLogExporter {
        async fn export(&self, _batch: LogBatch<'_>) -> OTelSdkResult {
            let logger = self.logger.lock().unwrap();
            if let Some(logger) = logger.as_ref() {
                let mut log_record = logger.create_log_record();
                log_record.set_severity_number(opentelemetry::logs::Severity::Error);
                logger.emit(log_record);
            }

            Ok(())
        }
    }

    #[test]
    fn exporter_internal_log_does_not_deadlock_with_simple_processor() {
        // This tests that even when exporter produces logs while
        // exporting, it does not deadlock, as SimpleLogProcessor
        // activates SuppressGuard before calling the exporter.
        let exporter: ReentrantLogExporter = ReentrantLogExporter::new();
        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        exporter.set_logger(logger_provider.logger("processor-logger"));

        let logger = logger_provider.logger("test-logger");
        let mut log_record = logger.create_log_record();
        log_record.set_severity_number(opentelemetry::logs::Severity::Error);
        logger.emit(log_record);
    }
}
