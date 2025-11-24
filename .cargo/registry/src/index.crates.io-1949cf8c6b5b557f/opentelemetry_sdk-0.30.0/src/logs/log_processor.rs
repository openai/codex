//! # OpenTelemetry Log Processor Interface
//!
//! The `LogProcessor` interface provides hooks for log record processing and
//! exporting. Log processors receive `LogRecord`s emitted by the SDK's
//! `Logger` and determine how these records are handled.
//!
//! Built-in log processors are responsible for converting logs to exportable
//! representations and passing them to configured exporters. They can be
//! registered directly with a `LoggerProvider`.
//!
//! ## Types of Log Processors
//!
//! There are currently two types of log processors available in the SDK:
//! - **SimpleLogProcessor**: Forwards log records to the exporter immediately.
//! - **BatchLogProcessor**: Buffers log records and sends them to the exporter in batches.
//!
//! For more information, see simple_log_processor.rs and batch_log_processor.rs.
//!
//! ## Diagram
//!
//! ```ascii
//!   +-----+---------------+   +-----------------------+   +-------------------+
//!   |     |               |   |                       |   |                   |
//!   | SDK | Logger.emit() +---> (Simple)LogProcessor  +--->  LogExporter      |
//!   |     |               |   | (Batch)LogProcessor   +--->  (OTLPExporter)   |
//!   +-----+---------------+   +-----------------------+   +-------------------+
//! ```

use crate::error::OTelSdkResult;
use crate::{logs::SdkLogRecord, Resource};

#[cfg(feature = "spec_unstable_logs_enabled")]
use opentelemetry::logs::Severity;
use opentelemetry::InstrumentationScope;

use std::fmt::Debug;
use std::time::Duration;

/// The interface for plugging into a [`SdkLogger`].
///
/// [`SdkLogger`]: crate::logs::SdkLogger
pub trait LogProcessor: Send + Sync + Debug {
    /// Called when a log record is ready to processed and exported.
    ///
    /// This method receives a mutable reference to `LogRecord`. If the processor
    /// needs to handle the export asynchronously, it should clone the data to
    /// ensure it can be safely processed without lifetime issues. Any changes
    /// made to the log data in this method will be reflected in the next log
    /// processor in the chain.
    ///
    /// # Parameters
    /// - `record`: A mutable reference to `LogRecord` representing the log record.
    /// - `instrumentation`: The instrumentation scope associated with the log record.
    fn emit(&self, data: &mut SdkLogRecord, instrumentation: &InstrumentationScope);
    /// Force the logs lying in the cache to be exported.
    fn force_flush(&self) -> OTelSdkResult;
    /// Shuts down the processor.
    /// After shutdown returns the log processor should stop processing any logs.
    /// It's up to the implementation on when to drop the LogProcessor.
    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        Ok(())
    }
    /// Shuts down the processor with default timeout.
    fn shutdown(&self) -> OTelSdkResult {
        self.shutdown_with_timeout(Duration::from_secs(5))
    }
    #[cfg(feature = "spec_unstable_logs_enabled")]
    /// Check if logging is enabled
    fn event_enabled(&self, _level: Severity, _target: &str, _name: Option<&str>) -> bool {
        // By default, all logs are enabled
        true
    }

    /// Set the resource for the log processor.
    fn set_resource(&mut self, _resource: &Resource) {}
}

#[cfg(all(test, feature = "testing", feature = "logs"))]
pub(crate) mod tests {
    use crate::logs::{LogBatch, LogExporter, SdkLogRecord};
    use crate::Resource;
    use crate::{
        error::OTelSdkResult,
        logs::{LogProcessor, SdkLoggerProvider},
    };
    use opentelemetry::logs::AnyValue;
    use opentelemetry::logs::LogRecord as _;
    use opentelemetry::logs::{Logger, LoggerProvider};
    use opentelemetry::{InstrumentationScope, Key};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone)]
    pub(crate) struct MockLogExporter {
        pub resource: Arc<Mutex<Option<Resource>>>,
    }

    impl LogExporter for MockLogExporter {
        async fn export(&self, _batch: LogBatch<'_>) -> OTelSdkResult {
            Ok(())
        }

        fn set_resource(&mut self, resource: &Resource) {
            self.resource
                .lock()
                .map(|mut res_opt| {
                    res_opt.replace(resource.clone());
                })
                .expect("mock log exporter shouldn't error when setting resource");
        }
    }

    // Implementation specific to the MockLogExporter, not part of the LogExporter trait
    impl MockLogExporter {
        pub(crate) fn get_resource(&self) -> Option<Resource> {
            (*self.resource).lock().unwrap().clone()
        }
    }

    #[derive(Debug)]
    struct FirstProcessor {
        pub(crate) logs: Arc<Mutex<Vec<(SdkLogRecord, InstrumentationScope)>>>,
    }

    impl LogProcessor for FirstProcessor {
        fn emit(&self, record: &mut SdkLogRecord, instrumentation: &InstrumentationScope) {
            // add attribute
            record.add_attribute(
                Key::from_static_str("processed_by"),
                AnyValue::String("FirstProcessor".into()),
            );
            // update body
            record.body = Some("Updated by FirstProcessor".into());

            self.logs
                .lock()
                .unwrap()
                .push((record.clone(), instrumentation.clone())); //clone as the LogProcessor is storing the data.
        }

        fn force_flush(&self) -> OTelSdkResult {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct SecondProcessor {
        pub(crate) logs: Arc<Mutex<Vec<(SdkLogRecord, InstrumentationScope)>>>,
    }

    impl LogProcessor for SecondProcessor {
        fn emit(&self, record: &mut SdkLogRecord, instrumentation: &InstrumentationScope) {
            assert!(record.attributes_contains(
                &Key::from_static_str("processed_by"),
                &AnyValue::String("FirstProcessor".into())
            ));
            assert!(
                record.body.clone().unwrap()
                    == AnyValue::String("Updated by FirstProcessor".into())
            );
            self.logs
                .lock()
                .unwrap()
                .push((record.clone(), instrumentation.clone()));
        }

        fn force_flush(&self) -> OTelSdkResult {
            Ok(())
        }
    }

    #[test]
    fn test_log_data_modification_by_multiple_processors() {
        let first_processor_logs = Arc::new(Mutex::new(Vec::new()));
        let second_processor_logs = Arc::new(Mutex::new(Vec::new()));

        let first_processor = FirstProcessor {
            logs: Arc::clone(&first_processor_logs),
        };
        let second_processor = SecondProcessor {
            logs: Arc::clone(&second_processor_logs),
        };

        let logger_provider = SdkLoggerProvider::builder()
            .with_log_processor(first_processor)
            .with_log_processor(second_processor)
            .build();

        let logger = logger_provider.logger("test-logger");
        let mut log_record = logger.create_log_record();
        log_record.body = Some(AnyValue::String("Test log".into()));

        logger.emit(log_record);

        assert_eq!(first_processor_logs.lock().unwrap().len(), 1);
        assert_eq!(second_processor_logs.lock().unwrap().len(), 1);

        let first_log = &first_processor_logs.lock().unwrap()[0];
        let second_log = &second_processor_logs.lock().unwrap()[0];

        assert!(first_log.0.attributes_contains(
            &Key::from_static_str("processed_by"),
            &AnyValue::String("FirstProcessor".into())
        ));
        assert!(second_log.0.attributes_contains(
            &Key::from_static_str("processed_by"),
            &AnyValue::String("FirstProcessor".into())
        ));

        assert!(
            first_log.0.body.clone().unwrap()
                == AnyValue::String("Updated by FirstProcessor".into())
        );
        assert!(
            second_log.0.body.clone().unwrap()
                == AnyValue::String("Updated by FirstProcessor".into())
        );
    }
}
