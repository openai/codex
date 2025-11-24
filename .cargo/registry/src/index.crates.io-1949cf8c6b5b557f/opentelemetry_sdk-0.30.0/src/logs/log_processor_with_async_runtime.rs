use crate::error::{OTelSdkError, OTelSdkResult};
use crate::{
    logs::{LogBatch, LogExporter, SdkLogRecord},
    Resource,
};

use opentelemetry::{otel_debug, otel_error, otel_warn, InstrumentationScope};

use std::{
    fmt::{self, Debug, Formatter},
    sync::Arc,
};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use super::{BatchConfig, LogProcessor};
#[cfg(feature = "experimental_async_runtime")]
use crate::runtime::{to_interval_stream, RuntimeChannel, TrySend};
use futures_channel::oneshot;
use futures_util::{
    future::{self, Either},
    {pin_mut, stream, StreamExt as _},
};

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum BatchMessage {
    /// Export logs, usually called when the log is emitted.
    ExportLog((SdkLogRecord, InstrumentationScope)),
    /// Flush the current buffer to the backend, it can be triggered by
    /// pre configured interval or a call to `force_push` function.
    Flush(Option<oneshot::Sender<OTelSdkResult>>),
    /// Shut down the worker thread, push all logs in buffer to the backend.
    Shutdown(oneshot::Sender<OTelSdkResult>),
    /// Set the resource for the exporter.
    SetResource(Arc<Resource>),
}

/// A [`LogProcessor`] that asynchronously buffers log records and reports
/// them at a pre-configured interval.
pub struct BatchLogProcessor<R: RuntimeChannel> {
    message_sender: R::Sender<BatchMessage>,

    // Track dropped logs - we'll log this at shutdown
    dropped_logs_count: AtomicUsize,

    // Track the maximum queue size that was configured for this processor
    max_queue_size: usize,
}

impl<R: RuntimeChannel> Debug for BatchLogProcessor<R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatchLogProcessor")
            .field("message_sender", &self.message_sender)
            .finish()
    }
}

impl<R: RuntimeChannel> LogProcessor for BatchLogProcessor<R> {
    fn emit(&self, record: &mut SdkLogRecord, instrumentation: &InstrumentationScope) {
        let result = self.message_sender.try_send(BatchMessage::ExportLog((
            record.clone(),
            instrumentation.clone(),
        )));

        // TODO - Implement throttling to prevent error flooding when the queue is full or closed.
        if result.is_err() {
            // Increment dropped logs count. The first time we have to drop a log,
            // emit a warning.
            if self.dropped_logs_count.fetch_add(1, Ordering::Relaxed) == 0 {
                otel_warn!(name: "BatchLogProcessor.LogDroppingStarted",
                    message = "BatchLogProcessor dropped a LogRecord due to queue full/internal errors. No further log will be emitted for further drops until Shutdown. During Shutdown time, a log will be emitted with exact count of total logs dropped.");
            }
        }
    }

    fn force_flush(&self) -> OTelSdkResult {
        let (res_sender, res_receiver) = oneshot::channel();
        self.message_sender
            .try_send(BatchMessage::Flush(Some(res_sender)))
            .map_err(|err| OTelSdkError::InternalFailure(format!("{:?}", err)))?;

        futures_executor::block_on(res_receiver)
            .map_err(|err| OTelSdkError::InternalFailure(format!("{:?}", err)))
            .and_then(std::convert::identity)
    }

    fn shutdown(&self) -> OTelSdkResult {
        let dropped_logs = self.dropped_logs_count.load(Ordering::Relaxed);
        let max_queue_size = self.max_queue_size;
        if dropped_logs > 0 {
            otel_warn!(
                name: "BatchLogProcessor.LogsDropped",
                dropped_logs_count = dropped_logs,
                max_queue_size = max_queue_size,
                message = "Logs were dropped due to a queue being full or other error. The count represents the total count of log records dropped in the lifetime of this BatchLogProcessor. Consider increasing the queue size and/or decrease delay between intervals."
            );
        }
        let (res_sender, res_receiver) = oneshot::channel();
        self.message_sender
            .try_send(BatchMessage::Shutdown(res_sender))
            .map_err(|err| OTelSdkError::InternalFailure(format!("{:?}", err)))?;

        futures_executor::block_on(res_receiver)
            .map_err(|err| OTelSdkError::InternalFailure(format!("{:?}", err)))
            .and_then(std::convert::identity)
    }

    fn set_resource(&mut self, resource: &Resource) {
        let resource = Arc::new(resource.clone());
        let _ = self
            .message_sender
            .try_send(BatchMessage::SetResource(resource));
    }
}

impl<R: RuntimeChannel> BatchLogProcessor<R> {
    pub(crate) fn new<E>(mut exporter: E, config: BatchConfig, runtime: R) -> Self
    where
        E: LogExporter + Send + Sync + 'static,
    {
        let (message_sender, message_receiver) =
            runtime.batch_message_channel(config.max_queue_size);
        let inner_runtime = runtime.clone();

        // Spawn worker process via user-defined spawn function.
        runtime.spawn(async move {
            // Timer will take a reference to the current runtime, so its important we do this within the
            // runtime.spawn()
            let ticker = to_interval_stream(inner_runtime.clone(), config.scheduled_delay)
                .skip(1) // The ticker is fired immediately, so we should skip the first one to align with the interval.
                .map(|_| BatchMessage::Flush(None));

            let timeout_runtime = inner_runtime.clone();
            let mut logs = Vec::new();
            let mut messages = Box::pin(stream::select(message_receiver, ticker));

            while let Some(message) = messages.next().await {
                match message {
                    // Log has finished, add to buffer of pending logs.
                    BatchMessage::ExportLog(log) => {
                        logs.push(log);
                        if logs.len() == config.max_export_batch_size {
                            let result = export_with_timeout(
                                config.max_export_timeout,
                                &mut exporter,
                                &timeout_runtime,
                                logs.split_off(0),
                            )
                            .await;

                            if let Err(err) = result {
                                otel_error!(
                                    name: "BatchLogProcessor.Export.Error",
                                    error = format!("{}", err)
                                );
                            }
                        }
                    }
                    // Log batch interval time reached or a force flush has been invoked, export current logs.
                    BatchMessage::Flush(res_channel) => {
                        let result = export_with_timeout(
                            config.max_export_timeout,
                            &mut exporter,
                            &timeout_runtime,
                            logs.split_off(0),
                        )
                        .await;

                        if let Some(channel) = res_channel {
                            if let Err(send_error) = channel.send(result) {
                                otel_debug!(
                                    name: "BatchLogProcessor.Flush.SendResultError",
                                    error = format!("{:?}", send_error),
                                );
                            }
                        }
                    }
                    // Stream has terminated or processor is shutdown, return to finish execution.
                    BatchMessage::Shutdown(ch) => {
                        let result = export_with_timeout(
                            config.max_export_timeout,
                            &mut exporter,
                            &timeout_runtime,
                            logs.split_off(0),
                        )
                        .await;

                        let _ = exporter.shutdown(); //TODO - handle error

                        if let Err(send_error) = ch.send(result) {
                            otel_debug!(
                                name: "BatchLogProcessor.Shutdown.SendResultError",
                                error = format!("{:?}", send_error),
                            );
                        }
                        break;
                    }
                    // propagate the resource
                    BatchMessage::SetResource(resource) => {
                        exporter.set_resource(&resource);
                    }
                }
            }
        });
        // Return batch processor with link to worker
        BatchLogProcessor {
            message_sender,
            dropped_logs_count: AtomicUsize::new(0),
            max_queue_size: config.max_queue_size,
        }
    }

    /// Create a new batch processor builder
    pub fn builder<E>(exporter: E, runtime: R) -> BatchLogProcessorBuilder<E, R>
    where
        E: LogExporter,
    {
        BatchLogProcessorBuilder {
            exporter,
            config: Default::default(),
            runtime,
        }
    }
}

async fn export_with_timeout<E, R>(
    time_out: Duration,
    exporter: &mut E,
    runtime: &R,
    batch: Vec<(SdkLogRecord, InstrumentationScope)>,
) -> OTelSdkResult
where
    R: RuntimeChannel,
    E: LogExporter + ?Sized,
{
    if batch.is_empty() {
        return Ok(());
    }

    // TBD - Can we avoid this conversion as it involves heap allocation with new vector?
    let log_vec: Vec<(&SdkLogRecord, &InstrumentationScope)> = batch
        .iter()
        .map(|log_data| (&log_data.0, &log_data.1))
        .collect();
    let export = exporter.export(LogBatch::new(log_vec.as_slice()));
    let timeout = runtime.delay(time_out);
    pin_mut!(export);
    pin_mut!(timeout);
    match future::select(export, timeout).await {
        Either::Left((export_res, _)) => export_res,
        Either::Right((_, _)) => OTelSdkResult::Err(OTelSdkError::Timeout(time_out)),
    }
}

/// A builder for creating [`BatchLogProcessor`] instances.
///
#[derive(Debug)]
pub struct BatchLogProcessorBuilder<E, R> {
    exporter: E,
    config: BatchConfig,
    runtime: R,
}

impl<E, R> BatchLogProcessorBuilder<E, R>
where
    E: LogExporter + 'static,
    R: RuntimeChannel,
{
    /// Set the BatchConfig for [`BatchLogProcessorBuilder`]
    pub fn with_batch_config(self, config: BatchConfig) -> Self {
        BatchLogProcessorBuilder { config, ..self }
    }

    /// Build a batch processor
    pub fn build(self) -> BatchLogProcessor<R> {
        BatchLogProcessor::new(self.exporter, self.config, self.runtime)
    }
}

#[cfg(all(test, feature = "testing", feature = "logs"))]
mod tests {
    use crate::error::OTelSdkResult;
    use crate::logs::batch_log_processor::{
        OTEL_BLRP_EXPORT_TIMEOUT, OTEL_BLRP_MAX_EXPORT_BATCH_SIZE, OTEL_BLRP_MAX_QUEUE_SIZE,
        OTEL_BLRP_SCHEDULE_DELAY,
    };
    use crate::logs::log_processor_with_async_runtime::BatchLogProcessor;
    use crate::logs::InMemoryLogExporterBuilder;
    use crate::logs::SdkLogRecord;
    use crate::logs::{LogBatch, LogExporter};
    use crate::runtime;
    use crate::{
        logs::{
            batch_log_processor::{
                OTEL_BLRP_EXPORT_TIMEOUT_DEFAULT, OTEL_BLRP_MAX_EXPORT_BATCH_SIZE_DEFAULT,
                OTEL_BLRP_MAX_QUEUE_SIZE_DEFAULT, OTEL_BLRP_SCHEDULE_DELAY_DEFAULT,
            },
            BatchConfig, BatchConfigBuilder, InMemoryLogExporter, LogProcessor, SdkLoggerProvider,
            SimpleLogProcessor,
        },
        Resource,
    };
    use opentelemetry::logs::AnyValue;
    use opentelemetry::logs::LogRecord;
    use opentelemetry::logs::{Logger, LoggerProvider};
    use opentelemetry::KeyValue;
    use opentelemetry::{InstrumentationScope, Key};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Debug, Clone)]
    struct MockLogExporter {
        resource: Arc<Mutex<Option<Resource>>>,
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
        fn get_resource(&self) -> Option<Resource> {
            (*self.resource).lock().unwrap().clone()
        }
    }

    #[test]
    fn test_default_const_values() {
        assert_eq!(OTEL_BLRP_SCHEDULE_DELAY, "OTEL_BLRP_SCHEDULE_DELAY");
        assert_eq!(OTEL_BLRP_SCHEDULE_DELAY_DEFAULT.as_millis(), 1_000);
        assert_eq!(OTEL_BLRP_EXPORT_TIMEOUT, "OTEL_BLRP_EXPORT_TIMEOUT");
        assert_eq!(OTEL_BLRP_EXPORT_TIMEOUT_DEFAULT.as_millis(), 30_000);
        assert_eq!(OTEL_BLRP_MAX_QUEUE_SIZE, "OTEL_BLRP_MAX_QUEUE_SIZE");
        assert_eq!(OTEL_BLRP_MAX_QUEUE_SIZE_DEFAULT, 2_048);
        assert_eq!(
            OTEL_BLRP_MAX_EXPORT_BATCH_SIZE,
            "OTEL_BLRP_MAX_EXPORT_BATCH_SIZE"
        );
        assert_eq!(OTEL_BLRP_MAX_EXPORT_BATCH_SIZE_DEFAULT, 512);
    }

    #[test]
    fn test_default_batch_config_adheres_to_specification() {
        // The following environment variables are expected to be unset so that their default values are used.
        let env_vars = vec![
            OTEL_BLRP_SCHEDULE_DELAY,
            OTEL_BLRP_EXPORT_TIMEOUT,
            OTEL_BLRP_MAX_QUEUE_SIZE,
            OTEL_BLRP_MAX_EXPORT_BATCH_SIZE,
        ];

        let config = temp_env::with_vars_unset(env_vars, BatchConfig::default);

        assert_eq!(config.scheduled_delay, OTEL_BLRP_SCHEDULE_DELAY_DEFAULT);
        assert_eq!(config.max_export_timeout, OTEL_BLRP_EXPORT_TIMEOUT_DEFAULT);
        assert_eq!(config.max_queue_size, OTEL_BLRP_MAX_QUEUE_SIZE_DEFAULT);
        assert_eq!(
            config.max_export_batch_size,
            OTEL_BLRP_MAX_EXPORT_BATCH_SIZE_DEFAULT
        );
    }

    #[test]
    fn test_batch_config_configurable_by_env_vars() {
        let env_vars = vec![
            (OTEL_BLRP_SCHEDULE_DELAY, Some("2000")),
            (OTEL_BLRP_EXPORT_TIMEOUT, Some("60000")),
            (OTEL_BLRP_MAX_QUEUE_SIZE, Some("4096")),
            (OTEL_BLRP_MAX_EXPORT_BATCH_SIZE, Some("1024")),
        ];

        let config = temp_env::with_vars(env_vars, BatchConfig::default);

        assert_eq!(config.scheduled_delay, Duration::from_millis(2000));
        assert_eq!(config.max_export_timeout, Duration::from_millis(60000));
        assert_eq!(config.max_queue_size, 4096);
        assert_eq!(config.max_export_batch_size, 1024);
    }

    #[test]
    fn test_batch_config_max_export_batch_size_validation() {
        let env_vars = vec![
            (OTEL_BLRP_MAX_QUEUE_SIZE, Some("256")),
            (OTEL_BLRP_MAX_EXPORT_BATCH_SIZE, Some("1024")),
        ];

        let config = temp_env::with_vars(env_vars, BatchConfig::default);

        assert_eq!(config.max_queue_size, 256);
        assert_eq!(config.max_export_batch_size, 256);
        assert_eq!(config.scheduled_delay, OTEL_BLRP_SCHEDULE_DELAY_DEFAULT);
        assert_eq!(config.max_export_timeout, OTEL_BLRP_EXPORT_TIMEOUT_DEFAULT);
    }

    #[test]
    fn test_batch_config_with_fields() {
        let batch = BatchConfigBuilder::default()
            .with_max_export_batch_size(1)
            .with_scheduled_delay(Duration::from_millis(2))
            .with_max_export_timeout(Duration::from_millis(3))
            .with_max_queue_size(4)
            .build();

        assert_eq!(batch.max_export_batch_size, 1);
        assert_eq!(batch.scheduled_delay, Duration::from_millis(2));
        assert_eq!(batch.max_export_timeout, Duration::from_millis(3));
        assert_eq!(batch.max_queue_size, 4);
    }

    #[test]
    fn test_build_batch_log_processor_builder() {
        let mut env_vars = vec![
            (OTEL_BLRP_MAX_EXPORT_BATCH_SIZE, Some("500")),
            (OTEL_BLRP_SCHEDULE_DELAY, Some("I am not number")),
            (OTEL_BLRP_EXPORT_TIMEOUT, Some("2046")),
        ];
        temp_env::with_vars(env_vars.clone(), || {
            let builder =
                BatchLogProcessor::builder(InMemoryLogExporter::default(), runtime::Tokio);

            assert_eq!(builder.config.max_export_batch_size, 500);
            assert_eq!(
                builder.config.scheduled_delay,
                OTEL_BLRP_SCHEDULE_DELAY_DEFAULT
            );
            assert_eq!(
                builder.config.max_queue_size,
                OTEL_BLRP_MAX_QUEUE_SIZE_DEFAULT
            );
            assert_eq!(
                builder.config.max_export_timeout,
                Duration::from_millis(2046)
            );
        });

        env_vars.push((OTEL_BLRP_MAX_QUEUE_SIZE, Some("120")));

        temp_env::with_vars(env_vars, || {
            let builder =
                BatchLogProcessor::builder(InMemoryLogExporter::default(), runtime::Tokio);
            assert_eq!(builder.config.max_export_batch_size, 120);
            assert_eq!(builder.config.max_queue_size, 120);
        });
    }

    #[test]
    fn test_build_batch_log_processor_builder_with_custom_config() {
        let expected = BatchConfigBuilder::default()
            .with_max_export_batch_size(1)
            .with_scheduled_delay(Duration::from_millis(2))
            .with_max_export_timeout(Duration::from_millis(3))
            .with_max_queue_size(4)
            .build();

        let builder = BatchLogProcessor::builder(InMemoryLogExporter::default(), runtime::Tokio)
            .with_batch_config(expected);

        let actual = &builder.config;
        assert_eq!(actual.max_export_batch_size, 1);
        assert_eq!(actual.scheduled_delay, Duration::from_millis(2));
        assert_eq!(actual.max_export_timeout, Duration::from_millis(3));
        assert_eq!(actual.max_queue_size, 4);
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_set_resource_batch_processor() {
        let exporter = MockLogExporter {
            resource: Arc::new(Mutex::new(None)),
        };
        let processor =
            BatchLogProcessor::new(exporter.clone(), BatchConfig::default(), runtime::Tokio);
        let provider = SdkLoggerProvider::builder()
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

        provider.force_flush().unwrap();

        assert_eq!(exporter.get_resource().unwrap().into_iter().count(), 5);
        let _ = provider.shutdown();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_batch_shutdown() {
        // assert we will receive an error
        // setup
        let exporter = InMemoryLogExporterBuilder::default()
            .keep_records_on_shutdown()
            .build();
        let processor =
            BatchLogProcessor::new(exporter.clone(), BatchConfig::default(), runtime::Tokio);

        let mut record = SdkLogRecord::new();
        let instrumentation = InstrumentationScope::default();

        processor.emit(&mut record, &instrumentation);
        processor.force_flush().unwrap();
        processor.shutdown().unwrap();
        // todo: expect to see errors here. How should we assert this?
        processor.emit(&mut record, &instrumentation);
        assert_eq!(1, exporter.get_emitted_logs().unwrap().len())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_batch_log_processor_shutdown_under_async_runtime_current_flavor_multi_thread() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = BatchLogProcessor::new(
            exporter.clone(),
            BatchConfig::default(),
            runtime::TokioCurrentThread,
        );

        processor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "See issue https://github.com/open-telemetry/opentelemetry-rust/issues/1968"]
    async fn test_batch_log_processor_with_async_runtime_shutdown_under_async_runtime_current_flavor_multi_thread(
    ) {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = BatchLogProcessor::new(
            exporter.clone(),
            BatchConfig::default(),
            runtime::TokioCurrentThread,
        );

        //
        // deadlock happens in shutdown with tokio current_thread runtime
        //
        processor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_batch_log_processor_shutdown_with_async_runtime_current_flavor_current_thread() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = BatchLogProcessor::new(
            exporter.clone(),
            BatchConfig::default(),
            runtime::TokioCurrentThread,
        );
        processor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_batch_log_processor_shutdown_with_async_runtime_multi_flavor_multi_thread() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor =
            BatchLogProcessor::new(exporter.clone(), BatchConfig::default(), runtime::Tokio);
        processor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_batch_log_processor_shutdown_with_async_runtime_multi_flavor_current_thread() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor =
            BatchLogProcessor::new(exporter.clone(), BatchConfig::default(), runtime::Tokio);
        processor.shutdown().unwrap();
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

    #[test]
    fn test_build_batch_log_processor_builder_rt() {
        let mut env_vars = vec![
            (OTEL_BLRP_MAX_EXPORT_BATCH_SIZE, Some("500")),
            (OTEL_BLRP_SCHEDULE_DELAY, Some("I am not number")),
            (OTEL_BLRP_EXPORT_TIMEOUT, Some("2046")),
        ];
        temp_env::with_vars(env_vars.clone(), || {
            let builder =
                BatchLogProcessor::builder(InMemoryLogExporter::default(), runtime::Tokio);

            assert_eq!(builder.config.max_export_batch_size, 500);
            assert_eq!(
                builder.config.scheduled_delay,
                OTEL_BLRP_SCHEDULE_DELAY_DEFAULT
            );
            assert_eq!(
                builder.config.max_queue_size,
                OTEL_BLRP_MAX_QUEUE_SIZE_DEFAULT
            );
            assert_eq!(
                builder.config.max_export_timeout,
                Duration::from_millis(2046)
            );
        });

        env_vars.push((OTEL_BLRP_MAX_QUEUE_SIZE, Some("120")));

        temp_env::with_vars(env_vars, || {
            let builder =
                BatchLogProcessor::builder(InMemoryLogExporter::default(), runtime::Tokio);
            assert_eq!(builder.config.max_export_batch_size, 120);
            assert_eq!(builder.config.max_queue_size, 120);
        });
    }

    #[test]
    fn test_build_batch_log_processor_builder_rt_with_custom_config() {
        let expected = BatchConfigBuilder::default()
            .with_max_export_batch_size(1)
            .with_scheduled_delay(Duration::from_millis(2))
            .with_max_export_timeout(Duration::from_millis(3))
            .with_max_queue_size(4)
            .build();

        let builder = BatchLogProcessor::builder(InMemoryLogExporter::default(), runtime::Tokio)
            .with_batch_config(expected);

        let actual = &builder.config;
        assert_eq!(actual.max_export_batch_size, 1);
        assert_eq!(actual.scheduled_delay, Duration::from_millis(2));
        assert_eq!(actual.max_export_timeout, Duration::from_millis(3));
        assert_eq!(actual.max_queue_size, 4);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_set_resource_batch_processor_rt() {
        let exporter = MockLogExporter {
            resource: Arc::new(Mutex::new(None)),
        };
        let processor =
            BatchLogProcessor::new(exporter.clone(), BatchConfig::default(), runtime::Tokio);
        let provider = SdkLoggerProvider::builder()
            .with_log_processor(processor)
            .with_resource(Resource::new(vec![
                KeyValue::new("k1", "v1"),
                KeyValue::new("k2", "v3"),
                KeyValue::new("k3", "v3"),
                KeyValue::new("k4", "v4"),
                KeyValue::new("k5", "v5"),
            ]))
            .build();
        provider.force_flush().unwrap();
        assert_eq!(exporter.get_resource().unwrap().into_iter().count(), 5);
        let _ = provider.shutdown();
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "See issue https://github.com/open-telemetry/opentelemetry-rust/issues/1968"]
    async fn test_batch_shutdown_rt() {
        // assert we will receive an error
        // setup
        let exporter = InMemoryLogExporterBuilder::default()
            .keep_records_on_shutdown()
            .build();
        let processor =
            BatchLogProcessor::new(exporter.clone(), BatchConfig::default(), runtime::Tokio);

        let mut record = SdkLogRecord::new();
        let instrumentation = InstrumentationScope::default();

        processor.emit(&mut record, &instrumentation);
        processor.force_flush().unwrap();
        processor.shutdown().unwrap();
        // todo: expect to see errors here. How should we assert this?
        processor.emit(&mut record, &instrumentation);
        assert_eq!(1, exporter.get_emitted_logs().unwrap().len())
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "See issue https://github.com/open-telemetry/opentelemetry-rust/issues/1968"]
    async fn test_batch_log_processor_rt_shutdown_with_async_runtime_current_flavor_multi_thread() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor =
            BatchLogProcessor::new(exporter.clone(), BatchConfig::default(), runtime::Tokio);

        //
        // deadlock happens in shutdown with tokio current_thread runtime
        //
        processor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_batch_log_processor_rt_shutdown_with_async_runtime_current_flavor_current_thread()
    {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = BatchLogProcessor::new(
            exporter.clone(),
            BatchConfig::default(),
            runtime::TokioCurrentThread,
        );

        processor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_batch_log_processor_rt_shutdown_with_async_runtime_multi_flavor_multi_thread() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor =
            BatchLogProcessor::new(exporter.clone(), BatchConfig::default(), runtime::Tokio);

        processor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_batch_log_processor_rt_shutdown_with_async_runtime_multi_flavor_current_thread() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = BatchLogProcessor::new(
            exporter.clone(),
            BatchConfig::default(),
            runtime::TokioCurrentThread,
        );

        processor.shutdown().unwrap();
    }
}
