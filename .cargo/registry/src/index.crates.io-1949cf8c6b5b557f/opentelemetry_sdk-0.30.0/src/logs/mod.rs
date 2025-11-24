//! # OpenTelemetry Log SDK
mod batch_log_processor;
mod export;
mod log_processor;
mod logger;
mod logger_provider;
pub(crate) mod record;
mod simple_log_processor;

/// In-Memory log exporter for testing purpose.
#[cfg(any(feature = "testing", test))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "testing", test))))]
pub mod in_memory_exporter;
#[cfg(any(feature = "testing", test))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "testing", test))))]
pub use in_memory_exporter::{InMemoryLogExporter, InMemoryLogExporterBuilder};

pub use batch_log_processor::{
    BatchConfig, BatchConfigBuilder, BatchLogProcessor, BatchLogProcessorBuilder,
};
pub use export::{LogBatch, LogExporter};
pub use log_processor::LogProcessor;
pub use logger::SdkLogger;
pub use logger_provider::{LoggerProviderBuilder, SdkLoggerProvider};
pub use record::{SdkLogRecord, TraceContext};
pub use simple_log_processor::SimpleLogProcessor;

#[cfg(feature = "experimental_logs_concurrent_log_processor")]
/// Module for ConcurrentLogProcessor.
pub mod concurrent_log_processor;

#[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
/// Module for BatchLogProcessor with async runtime.
pub mod log_processor_with_async_runtime;

#[cfg(all(test, feature = "testing"))]
mod tests {
    use super::*;
    use crate::error::OTelSdkResult;
    use crate::Resource;
    use opentelemetry::baggage::BaggageExt;
    use opentelemetry::logs::LogRecord;
    use opentelemetry::logs::{Logger, LoggerProvider, Severity};
    use opentelemetry::{logs::AnyValue, Key, KeyValue};
    use opentelemetry::{Context, InstrumentationScope};
    use std::borrow::Borrow;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[test]
    fn logging_sdk_test() {
        // Arrange
        let resource = Resource::builder_empty()
            .with_attributes([
                KeyValue::new("k1", "v1"),
                KeyValue::new("k2", "v2"),
                KeyValue::new("k3", "v3"),
                KeyValue::new("k4", "v4"),
            ])
            .build();
        let exporter: InMemoryLogExporter = InMemoryLogExporter::default();
        let logger_provider = SdkLoggerProvider::builder()
            .with_resource(resource.clone())
            .with_log_processor(SimpleLogProcessor::new(exporter.clone()))
            .build();

        // Act
        let logger = logger_provider.logger("test-logger");
        let mut log_record = logger.create_log_record();
        log_record.set_severity_number(Severity::Error);
        log_record.set_severity_text("Error");

        // Adding attributes using a vector with explicitly constructed Key and AnyValue objects.
        log_record.add_attributes(vec![
            (Key::new("key1"), AnyValue::from("value1")),
            (Key::new("key2"), AnyValue::from("value2")),
        ]);

        // Adding attributes using an array with explicitly constructed Key and AnyValue objects.
        log_record.add_attributes([
            (Key::new("key3"), AnyValue::from("value3")),
            (Key::new("key4"), AnyValue::from("value4")),
        ]);

        // Adding attributes using a vector with tuple auto-conversion to Key and AnyValue.
        log_record.add_attributes(vec![("key5", "value5"), ("key6", "value6")]);

        // Adding attributes using an array with tuple auto-conversion to Key and AnyValue.
        log_record.add_attributes([("key7", "value7"), ("key8", "value8")]);

        // Adding Attributes from a HashMap
        let mut attributes_map = HashMap::new();
        attributes_map.insert("key9", "value9");
        attributes_map.insert("key10", "value10");

        log_record.add_attributes(attributes_map);

        logger.emit(log_record);

        // Assert
        let exported_logs = exporter
            .get_emitted_logs()
            .expect("Logs are expected to be exported.");
        assert_eq!(exported_logs.len(), 1);
        let log = exported_logs
            .first()
            .expect("Atleast one log is expected to be present.");
        assert_eq!(log.instrumentation.name(), "test-logger");
        assert_eq!(log.record.severity_number, Some(Severity::Error));
        assert_eq!(log.record.attributes_len(), 10);
        for i in 1..=10 {
            assert!(log.record.attributes_contains(
                &Key::new(format!("key{}", i)),
                &AnyValue::String(format!("value{}", i).into())
            ));
        }

        // validate Resource
        assert_eq!(&resource, log.resource.borrow());
    }

    #[test]
    fn logger_attributes() {
        let exporter: InMemoryLogExporter = InMemoryLogExporter::default();
        let provider = SdkLoggerProvider::builder()
            .with_log_processor(SimpleLogProcessor::new(exporter.clone()))
            .build();

        let scope = InstrumentationScope::builder("test_logger")
            .with_schema_url("https://opentelemetry.io/schema/1.0.0")
            .with_attributes(vec![(KeyValue::new("test_k", "test_v"))])
            .build();

        let logger = provider.logger_with_scope(scope);

        let mut log_record = logger.create_log_record();
        log_record.set_severity_number(Severity::Error);

        logger.emit(log_record);

        let mut exported_logs = exporter
            .get_emitted_logs()
            .expect("Logs are expected to be exported.");
        assert_eq!(exported_logs.len(), 1);
        let log = exported_logs.remove(0);
        assert_eq!(log.record.severity_number, Some(Severity::Error));

        let instrumentation_scope = log.instrumentation;
        assert_eq!(instrumentation_scope.name(), "test_logger");
        assert_eq!(
            instrumentation_scope.schema_url(),
            Some("https://opentelemetry.io/schema/1.0.0")
        );
        assert!(instrumentation_scope
            .attributes()
            .eq(&[KeyValue::new("test_k", "test_v")]));
    }

    #[derive(Debug)]
    struct EnrichWithBaggageProcessor;
    impl LogProcessor for EnrichWithBaggageProcessor {
        fn emit(&self, data: &mut SdkLogRecord, _instrumentation: &InstrumentationScope) {
            Context::map_current(|cx| {
                for (kk, vv) in cx.baggage().iter() {
                    data.add_attribute(kk.clone(), vv.0.clone());
                }
            });
        }

        fn force_flush(&self) -> crate::error::OTelSdkResult {
            Ok(())
        }

        fn shutdown(&self) -> crate::error::OTelSdkResult {
            Ok(())
        }
    }
    #[test]
    fn log_and_baggage() {
        // Arrange
        let exporter: InMemoryLogExporter = InMemoryLogExporter::default();
        let logger_provider = SdkLoggerProvider::builder()
            .with_log_processor(EnrichWithBaggageProcessor)
            .with_log_processor(SimpleLogProcessor::new(exporter.clone()))
            .build();

        // Act
        let logger = logger_provider.logger("test-logger");
        let context_with_baggage =
            Context::current_with_baggage(vec![KeyValue::new("key-from-bag", "value-from-bag")]);
        let _cx_guard = context_with_baggage.attach();
        let mut log_record = logger.create_log_record();
        log_record.add_attribute("key", "value");
        logger.emit(log_record);

        // Assert
        let exported_logs = exporter
            .get_emitted_logs()
            .expect("Logs are expected to be exported.");
        assert_eq!(exported_logs.len(), 1);
        let log = exported_logs
            .first()
            .expect("Atleast one log is expected to be present.");
        assert_eq!(log.instrumentation.name(), "test-logger");
        assert_eq!(log.record.attributes_len(), 2);

        // Assert that the log record contains the baggage attribute
        // and the attribute added to the log record.
        assert!(log
            .record
            .attributes_contains(&Key::new("key"), &AnyValue::String("value".into())));
        assert!(log.record.attributes_contains(
            &Key::new("key-from-bag"),
            &AnyValue::String("value-from-bag".into())
        ));
    }

    #[test]
    fn log_suppression() {
        // Arrange
        let exporter: InMemoryLogExporter = InMemoryLogExporter::default();
        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();

        // Act
        let logger = logger_provider.logger("test-logger");
        let log_record = logger.create_log_record();
        {
            let _suppressed_context = Context::enter_telemetry_suppressed_scope();
            // This log emission should be suppressed and not exported.
            logger.emit(log_record);
        }

        // Assert
        let exported_logs = exporter.get_emitted_logs().expect("this should not fail.");
        assert_eq!(
            exported_logs.len(),
            0,
            "There should be a no logs as log emission is done inside a suppressed context"
        );
    }

    #[derive(Debug, Clone)]
    struct ReentrantLogProcessor {
        logger: Arc<Mutex<Option<SdkLogger>>>,
    }

    impl ReentrantLogProcessor {
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

    impl LogProcessor for ReentrantLogProcessor {
        fn emit(&self, _data: &mut SdkLogRecord, _instrumentation: &InstrumentationScope) {
            let _suppress = Context::enter_telemetry_suppressed_scope();
            // Without the suppression above, the logger.emit(log_record) below will cause a deadlock,
            // as it emits another log, which will attempt to acquire the same lock that is
            // already held by itself!
            let logger = self.logger.lock().unwrap();
            if let Some(logger) = logger.as_ref() {
                let mut log_record = logger.create_log_record();
                log_record.set_severity_number(Severity::Error);
                logger.emit(log_record);
            }
        }

        fn force_flush(&self) -> OTelSdkResult {
            Ok(())
        }

        fn shutdown(&self) -> OTelSdkResult {
            Ok(())
        }
    }

    #[test]
    fn processor_internal_log_does_not_deadlock_with_suppression_enabled() {
        let processor: ReentrantLogProcessor = ReentrantLogProcessor::new();
        let logger_provider = SdkLoggerProvider::builder()
            .with_log_processor(processor.clone())
            .build();
        processor.set_logger(logger_provider.logger("processor-logger"));

        let logger = logger_provider.logger("test-logger");
        let mut log_record = logger.create_log_record();
        log_record.set_severity_number(Severity::Error);
        logger.emit(log_record);
    }
}
