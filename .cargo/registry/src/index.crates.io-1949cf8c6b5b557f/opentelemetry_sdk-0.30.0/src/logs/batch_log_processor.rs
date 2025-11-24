//! # OpenTelemetry Batch Log Processor
//! The `BatchLogProcessor` is one implementation of the `LogProcessor` interface.
//!
//! It buffers log records and sends them to the exporter
//! in batches. This processor is designed for **production use** in high-throughput
//! applications and reduces the overhead of frequent exports by using a background
//! thread for batch processing.
//!
//! ## Diagram
//!
//! ```ascii
//!   +-----+---------------+   +-----------------------+   +-------------------+
//!   |     |               |   |                       |   |                   |
//!   | SDK | Logger.emit() +---> (Batch)LogProcessor   +--->  (OTLPExporter)   |
//!   +-----+---------------+   +-----------------------+   +-------------------+
//! ```

use crate::error::{OTelSdkError, OTelSdkResult};
use crate::logs::log_processor::LogProcessor;
use crate::{
    logs::{LogBatch, LogExporter, SdkLogRecord},
    Resource,
};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};

use opentelemetry::{otel_debug, otel_error, otel_warn, Context, InstrumentationScope};

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::{cmp::min, env, sync::Mutex};
use std::{
    fmt::{self, Debug, Formatter},
    str::FromStr,
    sync::Arc,
    thread,
    time::Duration,
    time::Instant,
};

/// Delay interval between two consecutive exports.
pub(crate) const OTEL_BLRP_SCHEDULE_DELAY: &str = "OTEL_BLRP_SCHEDULE_DELAY";
/// Default delay interval between two consecutive exports.
pub(crate) const OTEL_BLRP_SCHEDULE_DELAY_DEFAULT: Duration = Duration::from_millis(1_000);
/// Maximum allowed time to export data.
#[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
pub(crate) const OTEL_BLRP_EXPORT_TIMEOUT: &str = "OTEL_BLRP_EXPORT_TIMEOUT";
/// Default maximum allowed time to export data.
#[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
pub(crate) const OTEL_BLRP_EXPORT_TIMEOUT_DEFAULT: Duration = Duration::from_millis(30_000);
/// Maximum queue size.
pub(crate) const OTEL_BLRP_MAX_QUEUE_SIZE: &str = "OTEL_BLRP_MAX_QUEUE_SIZE";
/// Default maximum queue size.
pub(crate) const OTEL_BLRP_MAX_QUEUE_SIZE_DEFAULT: usize = 2_048;
/// Maximum batch size, must be less than or equal to OTEL_BLRP_MAX_QUEUE_SIZE.
pub(crate) const OTEL_BLRP_MAX_EXPORT_BATCH_SIZE: &str = "OTEL_BLRP_MAX_EXPORT_BATCH_SIZE";
/// Default maximum batch size.
pub(crate) const OTEL_BLRP_MAX_EXPORT_BATCH_SIZE_DEFAULT: usize = 512;

/// Messages sent between application thread and batch log processor's work thread.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum BatchMessage {
    /// This is ONLY sent when the number of logs records in the data channel has reached `max_export_batch_size`.
    ExportLog(Arc<AtomicBool>),
    /// ForceFlush flushes the current buffer to the exporter.
    ForceFlush(mpsc::SyncSender<OTelSdkResult>),
    /// Shut down the worker thread, push all logs in buffer to the exporter.
    Shutdown(mpsc::SyncSender<OTelSdkResult>),
    /// Set the resource for the exporter.
    SetResource(Arc<Resource>),
}

type LogsData = Box<(SdkLogRecord, InstrumentationScope)>;

/// The `BatchLogProcessor` collects finished logs in a buffer and exports them
/// in batches to the configured `LogExporter`. This processor is ideal for
/// high-throughput environments, as it minimizes the overhead of exporting logs
/// individually. It uses a **dedicated background thread** to manage and export logs
/// asynchronously, ensuring that the application's main execution flow is not blocked.
///
/// This processor supports the following configurations:
/// - **Queue size**: Maximum number of log records that can be buffered.
/// - **Batch size**: Maximum number of log records to include in a single export.
/// - **Scheduled delay**: Frequency at which the batch is exported.
///
/// When using this processor with the OTLP Exporter, the following exporter
/// features are supported:
/// - `grpc-tonic`: Requires `LoggerProvider` to be created within a tokio runtime.
/// - `reqwest-blocking-client`: Works with a regular `main` or `tokio::main`.
///
/// In other words, other clients like `reqwest` and `hyper` are not supported.
///
/// `BatchLogProcessor` buffers logs in memory and exports them in batches. An
/// export is triggered when `max_export_batch_size` is reached or every
/// `scheduled_delay` milliseconds. Users can explicitly trigger an export using
/// the `force_flush` method. Shutdown also triggers an export of all buffered
/// logs and is recommended to be called before the application exits to ensure
/// all buffered logs are exported.
///
/// **Warning**: When using tokio's current-thread runtime, `shutdown()`, which
/// is a blocking call ,should not be called from your main thread. This can
/// cause deadlock. Instead, call `shutdown()` from a separate thread or use
/// tokio's `spawn_blocking`.
///
///
/// ### Using a BatchLogProcessor:
///
/// ```rust
/// use opentelemetry_sdk::logs::{BatchLogProcessor, BatchConfigBuilder, SdkLoggerProvider};
/// use opentelemetry::global;
/// use std::time::Duration;
/// use opentelemetry_sdk::logs::InMemoryLogExporter;
///
/// let exporter = InMemoryLogExporter::default(); // Replace with an actual exporter
/// let processor = BatchLogProcessor::builder(exporter)
///     .with_batch_config(
///         BatchConfigBuilder::default()
///             .with_max_queue_size(2048)
///             .with_max_export_batch_size(512)
///             .with_scheduled_delay(Duration::from_secs(5))
///             .build(),
///     )
///     .build();
///
/// let provider = SdkLoggerProvider::builder()
///     .with_log_processor(processor)
///     .build();
///
pub struct BatchLogProcessor {
    logs_sender: SyncSender<LogsData>, // Data channel to store log records and instrumentation scopes
    message_sender: SyncSender<BatchMessage>, // Control channel to store control messages for the worker thread
    handle: Mutex<Option<thread::JoinHandle<()>>>,
    forceflush_timeout: Duration,
    export_log_message_sent: Arc<AtomicBool>,
    current_batch_size: Arc<AtomicUsize>,
    max_export_batch_size: usize,

    // Track dropped logs - we'll log this at shutdown
    dropped_logs_count: AtomicUsize,

    // Track the maximum queue size that was configured for this processor
    max_queue_size: usize,
}

impl Debug for BatchLogProcessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatchLogProcessor")
            .field("message_sender", &self.message_sender)
            .finish()
    }
}

impl LogProcessor for BatchLogProcessor {
    fn emit(&self, record: &mut SdkLogRecord, instrumentation: &InstrumentationScope) {
        let result = self
            .logs_sender
            .try_send(Box::new((record.clone(), instrumentation.clone())));

        // match for result and handle each separately
        match result {
            Ok(_) => {
                // Successfully sent the log record to the data channel.
                // Increment the current batch size and check if it has reached
                // the max export batch size.
                if self.current_batch_size.fetch_add(1, Ordering::Relaxed) + 1
                    >= self.max_export_batch_size
                {
                    // Check if the a control message for exporting logs is
                    // already sent to the worker thread. If not, send a control
                    // message to export logs. `export_log_message_sent` is set
                    // to false ONLY when the worker thread has processed the
                    // control message.

                    if !self.export_log_message_sent.load(Ordering::Relaxed) {
                        // This is a cost-efficient check as atomic load
                        // operations do not require exclusive access to cache
                        // line. Perform atomic swap to
                        // `export_log_message_sent` ONLY when the atomic load
                        // operation above returns false. Atomic
                        // swap/compare_exchange operations require exclusive
                        // access to cache line on most processor architectures.
                        // We could have used compare_exchange as well here, but
                        // it's more verbose than swap.
                        if !self.export_log_message_sent.swap(true, Ordering::Relaxed) {
                            match self.message_sender.try_send(BatchMessage::ExportLog(
                                self.export_log_message_sent.clone(),
                            )) {
                                Ok(_) => {
                                    // Control message sent successfully.
                                }
                                Err(_err) => {
                                    // TODO: Log error If the control message
                                    // could not be sent, reset the
                                    // `export_log_message_sent` flag.
                                    self.export_log_message_sent.store(false, Ordering::Relaxed);
                                }
                            }
                        }
                    }
                }
            }
            Err(mpsc::TrySendError::Full(_)) => {
                // Increment dropped logs count. The first time we have to drop
                // a log, emit a warning.
                if self.dropped_logs_count.fetch_add(1, Ordering::Relaxed) == 0 {
                    otel_warn!(name: "BatchLogProcessor.LogDroppingStarted",
                        message = "BatchLogProcessor dropped a LogRecord due to queue full. No further log will be emitted for further drops until Shutdown. During Shutdown time, a log will be emitted with exact count of total logs dropped.");
                }
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                // Given background thread is the only receiver, and it's
                // disconnected, it indicates the thread is shutdown
                otel_warn!(
                    name: "BatchLogProcessor.Emit.AfterShutdown",
                    message = "Logs are being emitted even after Shutdown. This indicates incorrect lifecycle management of OTelLoggerProvider in application. Logs will not be exported."
                );
            }
        }
    }

    fn force_flush(&self) -> OTelSdkResult {
        let (sender, receiver) = mpsc::sync_channel(1);
        match self
            .message_sender
            .try_send(BatchMessage::ForceFlush(sender))
        {
            Ok(_) => receiver
                .recv_timeout(self.forceflush_timeout)
                .map_err(|err| {
                    if err == RecvTimeoutError::Timeout {
                        OTelSdkError::Timeout(self.forceflush_timeout)
                    } else {
                        OTelSdkError::InternalFailure(format!("{}", err))
                    }
                })?,
            Err(mpsc::TrySendError::Full(_)) => {
                // If the control message could not be sent, emit a warning.
                otel_debug!(
                    name: "BatchLogProcessor.ForceFlush.ControlChannelFull",
                    message = "Control message to flush the worker thread could not be sent as the control channel is full. This can occur if user repeatedily calls force_flush/shutdown without finishing the previous call."
                );
                Err(OTelSdkError::InternalFailure("ForceFlush cannot be performed as Control channel is full. This can occur if user repeatedily calls force_flush/shutdown without finishing the previous call.".into()))
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                // Given background thread is the only receiver, and it's
                // disconnected, it indicates the thread is shutdown
                otel_debug!(
                    name: "BatchLogProcessor.ForceFlush.AlreadyShutdown",
                    message = "ForceFlush invoked after Shutdown. This will not perform Flush and indicates a incorrect lifecycle management in Application."
                );

                Err(OTelSdkError::AlreadyShutdown)
            }
        }
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        let dropped_logs = self.dropped_logs_count.load(Ordering::Relaxed);
        let max_queue_size = self.max_queue_size;
        if dropped_logs > 0 {
            otel_warn!(
                name: "BatchLogProcessor.LogsDropped",
                dropped_logs_count = dropped_logs,
                max_queue_size = max_queue_size,
                message = "Logs were dropped due to a queue being full. The count represents the total count of log records dropped in the lifetime of this BatchLogProcessor. Consider increasing the queue size and/or decrease delay between intervals."
            );
        }

        let (sender, receiver) = mpsc::sync_channel(1);
        match self.message_sender.try_send(BatchMessage::Shutdown(sender)) {
            Ok(_) => {
                receiver
                    .recv_timeout(timeout)
                    .map(|_| {
                        // join the background thread after receiving back the
                        // shutdown signal
                        if let Some(handle) = self.handle.lock().unwrap().take() {
                            handle.join().unwrap();
                        }
                        OTelSdkResult::Ok(())
                    })
                    .map_err(|err| match err {
                        RecvTimeoutError::Timeout => {
                            otel_error!(
                                name: "BatchLogProcessor.Shutdown.Timeout",
                                message = "BatchLogProcessor shutdown timing out."
                            );
                            OTelSdkError::Timeout(timeout)
                        }
                        _ => {
                            otel_error!(
                                name: "BatchLogProcessor.Shutdown.Error",
                                error = format!("{}", err)
                            );
                            OTelSdkError::InternalFailure(format!("{}", err))
                        }
                    })?
            }
            Err(mpsc::TrySendError::Full(_)) => {
                // If the control message could not be sent, emit a warning.
                otel_debug!(
                    name: "BatchLogProcessor.Shutdown.ControlChannelFull",
                    message = "Control message to shutdown the worker thread could not be sent as the control channel is full. This can occur if user repeatedily calls force_flush/shutdown without finishing the previous call."
                );
                Err(OTelSdkError::InternalFailure("Shutdown cannot be performed as Control channel is full. This can occur if user repeatedily calls force_flush/shutdown without finishing the previous call.".into()))
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                // Given background thread is the only receiver, and it's
                // disconnected, it indicates the thread is shutdown
                otel_debug!(
                    name: "BatchLogProcessor.Shutdown.AlreadyShutdown",
                    message = "Shutdown is being invoked more than once. This is noop, but indicates a potential issue in the application's lifecycle management."
                );

                Err(OTelSdkError::AlreadyShutdown)
            }
        }
    }

    fn set_resource(&mut self, resource: &Resource) {
        let resource = Arc::new(resource.clone());
        let _ = self
            .message_sender
            .try_send(BatchMessage::SetResource(resource));
    }
}

impl BatchLogProcessor {
    pub(crate) fn new<E>(mut exporter: E, config: BatchConfig) -> Self
    where
        E: LogExporter + Send + Sync + 'static,
    {
        let (logs_sender, logs_receiver) = mpsc::sync_channel::<LogsData>(config.max_queue_size);
        let (message_sender, message_receiver) = mpsc::sync_channel::<BatchMessage>(64); // Is this a reasonable bound?
        let max_queue_size = config.max_queue_size;
        let max_export_batch_size = config.max_export_batch_size;
        let current_batch_size = Arc::new(AtomicUsize::new(0));
        let current_batch_size_for_thread = current_batch_size.clone();

        let handle = thread::Builder::new()
            .name("OpenTelemetry.Logs.BatchProcessor".to_string())
            .spawn(move || {
                let _suppress_guard = Context::enter_telemetry_suppressed_scope();
                otel_debug!(
                    name: "BatchLogProcessor.ThreadStarted",
                    interval_in_millisecs = config.scheduled_delay.as_millis(),
                    max_export_batch_size = config.max_export_batch_size,
                    max_queue_size = max_queue_size,
                );
                let mut last_export_time = Instant::now();
                let mut logs = Vec::with_capacity(config.max_export_batch_size);
                let current_batch_size = current_batch_size_for_thread;

                // This method gets up to `max_export_batch_size` amount of logs from the channel and exports them.
                // It returns the result of the export operation.
                // It expects the logs vec to be empty when it's called.
                #[inline]
                fn get_logs_and_export<E>(
                    logs_receiver: &mpsc::Receiver<LogsData>,
                    exporter: &E,
                    logs: &mut Vec<LogsData>,
                    last_export_time: &mut Instant,
                    current_batch_size: &AtomicUsize,
                    config: &BatchConfig,
                ) -> OTelSdkResult
                where
                    E: LogExporter + Send + Sync + 'static,
                {
                    let target = current_batch_size.load(Ordering::Relaxed); // `target` is used to determine the stopping criteria for exporting logs.
                    let mut result = OTelSdkResult::Ok(());
                    let mut total_exported_logs: usize = 0;

                    while target > 0 && total_exported_logs < target {
                        // Get upto `max_export_batch_size` amount of logs log records from the channel and push them to the logs vec
                        while let Ok(log) = logs_receiver.try_recv() {
                            logs.push(log);
                            if logs.len() == config.max_export_batch_size {
                                break;
                            }
                        }

                        let count_of_logs = logs.len(); // Count of logs that will be exported
                        total_exported_logs += count_of_logs;

                        result = export_batch_sync(exporter, logs, last_export_time); // This method clears the logs vec after exporting

                        current_batch_size.fetch_sub(count_of_logs, Ordering::Relaxed);
                    }
                    result
                }

                loop {
                    let remaining_time = config
                        .scheduled_delay
                        .checked_sub(last_export_time.elapsed())
                        .unwrap_or(config.scheduled_delay);

                    match message_receiver.recv_timeout(remaining_time) {
                        Ok(BatchMessage::ExportLog(export_log_message_sent)) => {
                            // Reset the export log message sent flag now it has has been processed.
                            export_log_message_sent.store(false, Ordering::Relaxed);

                            otel_debug!(
                                name: "BatchLogProcessor.ExportingDueToBatchSize",
                            );

                            let _ = get_logs_and_export(
                                &logs_receiver,
                                &exporter,
                                &mut logs,
                                &mut last_export_time,
                                &current_batch_size,
                                &config,
                            );
                        }
                        Ok(BatchMessage::ForceFlush(sender)) => {
                            otel_debug!(name: "BatchLogProcessor.ExportingDueToForceFlush");
                            let result = get_logs_and_export(
                                &logs_receiver,
                                &exporter,
                                &mut logs,
                                &mut last_export_time,
                                &current_batch_size,
                                &config,
                            );
                            let _ = sender.send(result);
                        }
                        Ok(BatchMessage::Shutdown(sender)) => {
                            otel_debug!(name: "BatchLogProcessor.ExportingDueToShutdown");
                            let result = get_logs_and_export(
                                &logs_receiver,
                                &exporter,
                                &mut logs,
                                &mut last_export_time,
                                &current_batch_size,
                                &config,
                            );
                            let _ = exporter.shutdown();
                            let _ = sender.send(result);

                            otel_debug!(
                                name: "BatchLogProcessor.ThreadExiting",
                                reason = "ShutdownRequested"
                            );
                            //
                            // break out the loop and return from the current background thread.
                            //
                            break;
                        }
                        Ok(BatchMessage::SetResource(resource)) => {
                            exporter.set_resource(&resource);
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            otel_debug!(
                                name: "BatchLogProcessor.ExportingDueToTimer",
                            );

                            let _ = get_logs_and_export(
                                &logs_receiver,
                                &exporter,
                                &mut logs,
                                &mut last_export_time,
                                &current_batch_size,
                                &config,
                            );
                        }
                        Err(RecvTimeoutError::Disconnected) => {
                            // Channel disconnected, only thing to do is break
                            // out (i.e exit the thread)
                            otel_debug!(
                                name: "BatchLogProcessor.ThreadExiting",
                                reason = "MessageSenderDisconnected"
                            );
                            break;
                        }
                    }
                }
                otel_debug!(
                    name: "BatchLogProcessor.ThreadStopped"
                );
            })
            .expect("Thread spawn failed."); //TODO: Handle thread spawn failure

        // Return batch processor with link to worker
        BatchLogProcessor {
            logs_sender,
            message_sender,
            handle: Mutex::new(Some(handle)),
            forceflush_timeout: Duration::from_secs(5), // TODO: make this configurable
            dropped_logs_count: AtomicUsize::new(0),
            max_queue_size,
            export_log_message_sent: Arc::new(AtomicBool::new(false)),
            current_batch_size,
            max_export_batch_size,
        }
    }

    /// Create a new batch processor builder
    pub fn builder<E>(exporter: E) -> BatchLogProcessorBuilder<E>
    where
        E: LogExporter,
    {
        BatchLogProcessorBuilder {
            exporter,
            config: Default::default(),
        }
    }
}

#[allow(clippy::vec_box)]
fn export_batch_sync<E>(
    exporter: &E,
    batch: &mut Vec<Box<(SdkLogRecord, InstrumentationScope)>>,
    last_export_time: &mut Instant,
) -> OTelSdkResult
where
    E: LogExporter + ?Sized,
{
    *last_export_time = Instant::now();

    if batch.is_empty() {
        return OTelSdkResult::Ok(());
    }

    let export = exporter.export(LogBatch::new_with_owned_data(batch.as_slice()));
    let export_result = futures_executor::block_on(export);

    // Clear the batch vec after exporting
    batch.clear();

    match export_result {
        Ok(_) => OTelSdkResult::Ok(()),
        Err(err) => {
            otel_error!(
                name: "BatchLogProcessor.ExportError",
                error = format!("{}", err)
            );
            OTelSdkResult::Err(err)
        }
    }
}

///
/// A builder for creating [`BatchLogProcessor`] instances.
///
#[derive(Debug)]
pub struct BatchLogProcessorBuilder<E> {
    exporter: E,
    config: BatchConfig,
}

impl<E> BatchLogProcessorBuilder<E>
where
    E: LogExporter + 'static,
{
    /// Set the BatchConfig for [`BatchLogProcessorBuilder`]
    pub fn with_batch_config(self, config: BatchConfig) -> Self {
        BatchLogProcessorBuilder { config, ..self }
    }

    /// Build a batch processor
    pub fn build(self) -> BatchLogProcessor {
        BatchLogProcessor::new(self.exporter, self.config)
    }
}

/// Batch log processor configuration.
/// Use [`BatchConfigBuilder`] to configure your own instance of [`BatchConfig`].
#[derive(Debug)]
#[allow(dead_code)]
pub struct BatchConfig {
    /// The maximum queue size to buffer logs for delayed processing. If the
    /// queue gets full it drops the logs. The default value of is 2048.
    pub(crate) max_queue_size: usize,

    /// The delay interval in milliseconds between two consecutive processing
    /// of batches. The default value is 1 second.
    pub(crate) scheduled_delay: Duration,

    /// The maximum number of logs to process in a single batch. If there are
    /// more than one batch worth of logs then it processes multiple batches
    /// of logs one batch after the other without any delay. The default value
    /// is 512.
    pub(crate) max_export_batch_size: usize,

    /// The maximum duration to export a batch of data.
    #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
    pub(crate) max_export_timeout: Duration,
}

impl Default for BatchConfig {
    fn default() -> Self {
        BatchConfigBuilder::default().build()
    }
}

/// A builder for creating [`BatchConfig`] instances.
#[derive(Debug)]
pub struct BatchConfigBuilder {
    max_queue_size: usize,
    scheduled_delay: Duration,
    max_export_batch_size: usize,
    #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
    max_export_timeout: Duration,
}

impl Default for BatchConfigBuilder {
    /// Create a new [`BatchConfigBuilder`] initialized with default batch config values as per the specs.
    /// The values are overridden by environment variables if set.
    /// The supported environment variables are:
    /// * `OTEL_BLRP_MAX_QUEUE_SIZE`
    /// * `OTEL_BLRP_SCHEDULE_DELAY`
    /// * `OTEL_BLRP_MAX_EXPORT_BATCH_SIZE`
    /// * `OTEL_BLRP_EXPORT_TIMEOUT`
    ///
    /// Note: Programmatic configuration overrides any value set via the environment variable.
    fn default() -> Self {
        BatchConfigBuilder {
            max_queue_size: OTEL_BLRP_MAX_QUEUE_SIZE_DEFAULT,
            scheduled_delay: OTEL_BLRP_SCHEDULE_DELAY_DEFAULT,
            max_export_batch_size: OTEL_BLRP_MAX_EXPORT_BATCH_SIZE_DEFAULT,
            #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
            max_export_timeout: OTEL_BLRP_EXPORT_TIMEOUT_DEFAULT,
        }
        .init_from_env_vars()
    }
}

impl BatchConfigBuilder {
    /// Set max_queue_size for [`BatchConfigBuilder`].
    /// It's the maximum queue size to buffer logs for delayed processing.
    /// If the queue gets full it will drop the logs.
    /// The default value is 2048.
    ///
    /// Corresponding environment variable: `OTEL_BLRP_MAX_QUEUE_SIZE`.
    ///
    /// Note: Programmatically setting this will override any value set via the environment variable.
    pub fn with_max_queue_size(mut self, max_queue_size: usize) -> Self {
        self.max_queue_size = max_queue_size;
        self
    }

    /// Set scheduled_delay for [`BatchConfigBuilder`].
    /// It's the delay interval in milliseconds between two consecutive processing of batches.
    /// The default value is 1000 milliseconds.
    ///
    /// Corresponding environment variable: `OTEL_BLRP_SCHEDULE_DELAY`.
    ///
    /// Note: Programmatically setting this will override any value set via the environment variable.
    pub fn with_scheduled_delay(mut self, scheduled_delay: Duration) -> Self {
        self.scheduled_delay = scheduled_delay;
        self
    }

    /// Set max_export_timeout for [`BatchConfigBuilder`].
    /// It's the maximum duration to export a batch of data.
    /// The default value is 30000 milliseconds.
    ///
    /// Corresponding environment variable: `OTEL_BLRP_EXPORT_TIMEOUT`.
    ///
    /// Note: Programmatically setting this will override any value set via the environment variable.
    #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
    pub fn with_max_export_timeout(mut self, max_export_timeout: Duration) -> Self {
        self.max_export_timeout = max_export_timeout;
        self
    }

    /// Set max_export_batch_size for [`BatchConfigBuilder`].
    /// It's the maximum number of logs to process in a single batch. If there are
    /// more than one batch worth of logs then it processes multiple batches
    /// of logs one batch after the other without any delay.
    /// The default value is 512.
    ///
    /// Corresponding environment variable: `OTEL_BLRP_MAX_EXPORT_BATCH_SIZE`.
    ///
    /// Note: Programmatically setting this will override any value set via the environment variable.
    pub fn with_max_export_batch_size(mut self, max_export_batch_size: usize) -> Self {
        self.max_export_batch_size = max_export_batch_size;
        self
    }

    /// Builds a `BatchConfig` enforcing the following invariants:
    /// * `max_export_batch_size` must be less than or equal to `max_queue_size`.
    pub fn build(self) -> BatchConfig {
        // max export batch size must be less or equal to max queue size.
        // we set max export batch size to max queue size if it's larger than max queue size.
        let max_export_batch_size = min(self.max_export_batch_size, self.max_queue_size);

        BatchConfig {
            max_queue_size: self.max_queue_size,
            scheduled_delay: self.scheduled_delay,
            #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
            max_export_timeout: self.max_export_timeout,
            max_export_batch_size,
        }
    }

    fn init_from_env_vars(mut self) -> Self {
        if let Some(max_queue_size) = env::var(OTEL_BLRP_MAX_QUEUE_SIZE)
            .ok()
            .and_then(|queue_size| usize::from_str(&queue_size).ok())
        {
            self.max_queue_size = max_queue_size;
        }

        if let Some(max_export_batch_size) = env::var(OTEL_BLRP_MAX_EXPORT_BATCH_SIZE)
            .ok()
            .and_then(|batch_size| usize::from_str(&batch_size).ok())
        {
            self.max_export_batch_size = max_export_batch_size;
        }

        if let Some(scheduled_delay) = env::var(OTEL_BLRP_SCHEDULE_DELAY)
            .ok()
            .and_then(|delay| u64::from_str(&delay).ok())
        {
            self.scheduled_delay = Duration::from_millis(scheduled_delay);
        }

        #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
        if let Some(max_export_timeout) = env::var(OTEL_BLRP_EXPORT_TIMEOUT)
            .ok()
            .and_then(|s| u64::from_str(&s).ok())
        {
            self.max_export_timeout = Duration::from_millis(max_export_timeout);
        }

        self
    }
}

#[cfg(all(test, feature = "testing", feature = "logs"))]
mod tests {
    use super::{
        BatchConfig, BatchConfigBuilder, BatchLogProcessor, OTEL_BLRP_MAX_EXPORT_BATCH_SIZE,
        OTEL_BLRP_MAX_EXPORT_BATCH_SIZE_DEFAULT, OTEL_BLRP_MAX_QUEUE_SIZE,
        OTEL_BLRP_MAX_QUEUE_SIZE_DEFAULT, OTEL_BLRP_SCHEDULE_DELAY,
        OTEL_BLRP_SCHEDULE_DELAY_DEFAULT,
    };
    #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
    use super::{OTEL_BLRP_EXPORT_TIMEOUT, OTEL_BLRP_EXPORT_TIMEOUT_DEFAULT};
    use crate::logs::log_processor::tests::MockLogExporter;
    use crate::logs::SdkLogRecord;
    use crate::{
        logs::{InMemoryLogExporter, InMemoryLogExporterBuilder, LogProcessor, SdkLoggerProvider},
        Resource,
    };
    use opentelemetry::InstrumentationScope;
    use opentelemetry::KeyValue;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn test_default_const_values() {
        assert_eq!(OTEL_BLRP_SCHEDULE_DELAY, "OTEL_BLRP_SCHEDULE_DELAY");
        assert_eq!(OTEL_BLRP_SCHEDULE_DELAY_DEFAULT.as_millis(), 1_000);
        #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
        assert_eq!(OTEL_BLRP_EXPORT_TIMEOUT, "OTEL_BLRP_EXPORT_TIMEOUT");
        #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
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
            #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
            OTEL_BLRP_EXPORT_TIMEOUT,
            OTEL_BLRP_MAX_QUEUE_SIZE,
            OTEL_BLRP_MAX_EXPORT_BATCH_SIZE,
        ];

        let config = temp_env::with_vars_unset(env_vars, BatchConfig::default);

        assert_eq!(config.scheduled_delay, OTEL_BLRP_SCHEDULE_DELAY_DEFAULT);
        #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
        assert_eq!(config.max_export_timeout, OTEL_BLRP_EXPORT_TIMEOUT_DEFAULT);
        assert_eq!(config.max_queue_size, OTEL_BLRP_MAX_QUEUE_SIZE_DEFAULT);
        assert_eq!(
            config.max_export_batch_size,
            OTEL_BLRP_MAX_EXPORT_BATCH_SIZE_DEFAULT
        );
    }

    #[test]
    fn test_code_based_config_overrides_env_vars() {
        let env_vars = vec![
            (OTEL_BLRP_SCHEDULE_DELAY, Some("2000")),
            (OTEL_BLRP_MAX_QUEUE_SIZE, Some("4096")),
            (OTEL_BLRP_MAX_EXPORT_BATCH_SIZE, Some("1024")),
        ];

        temp_env::with_vars(env_vars, || {
            let config = BatchConfigBuilder::default()
                .with_max_queue_size(2048)
                .with_scheduled_delay(Duration::from_millis(1000))
                .with_max_export_batch_size(512)
                .build();

            assert_eq!(config.scheduled_delay, Duration::from_millis(1000));
            assert_eq!(config.max_queue_size, 2048);
            assert_eq!(config.max_export_batch_size, 512);
        });
    }

    #[test]
    fn test_batch_config_configurable_by_env_vars() {
        let env_vars = vec![
            (OTEL_BLRP_SCHEDULE_DELAY, Some("2000")),
            #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
            (OTEL_BLRP_EXPORT_TIMEOUT, Some("60000")),
            (OTEL_BLRP_MAX_QUEUE_SIZE, Some("4096")),
            (OTEL_BLRP_MAX_EXPORT_BATCH_SIZE, Some("1024")),
        ];

        let config = temp_env::with_vars(env_vars, BatchConfig::default);

        assert_eq!(config.scheduled_delay, Duration::from_millis(2000));
        #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
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
        #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
        assert_eq!(config.max_export_timeout, OTEL_BLRP_EXPORT_TIMEOUT_DEFAULT);
    }

    #[test]
    fn test_batch_config_with_fields() {
        let batch_builder = BatchConfigBuilder::default()
            .with_max_export_batch_size(1)
            .with_scheduled_delay(Duration::from_millis(2))
            .with_max_queue_size(4);

        #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
        let batch_builder = batch_builder.with_max_export_timeout(Duration::from_millis(3));
        let batch = batch_builder.build();

        assert_eq!(batch.max_export_batch_size, 1);
        assert_eq!(batch.scheduled_delay, Duration::from_millis(2));
        #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
        assert_eq!(batch.max_export_timeout, Duration::from_millis(3));
        assert_eq!(batch.max_queue_size, 4);
    }

    #[test]
    fn test_build_batch_log_processor_builder() {
        let mut env_vars = vec![
            (OTEL_BLRP_MAX_EXPORT_BATCH_SIZE, Some("500")),
            (OTEL_BLRP_SCHEDULE_DELAY, Some("I am not number")),
            #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
            (OTEL_BLRP_EXPORT_TIMEOUT, Some("2046")),
        ];
        temp_env::with_vars(env_vars.clone(), || {
            let builder = BatchLogProcessor::builder(InMemoryLogExporter::default());

            assert_eq!(builder.config.max_export_batch_size, 500);
            assert_eq!(
                builder.config.scheduled_delay,
                OTEL_BLRP_SCHEDULE_DELAY_DEFAULT
            );
            assert_eq!(
                builder.config.max_queue_size,
                OTEL_BLRP_MAX_QUEUE_SIZE_DEFAULT
            );

            #[cfg(feature = "experimental_logs_batch_log_processor_with_async_runtime")]
            assert_eq!(
                builder.config.max_export_timeout,
                Duration::from_millis(2046)
            );
        });

        env_vars.push((OTEL_BLRP_MAX_QUEUE_SIZE, Some("120")));

        temp_env::with_vars(env_vars, || {
            let builder = BatchLogProcessor::builder(InMemoryLogExporter::default());
            assert_eq!(builder.config.max_export_batch_size, 120);
            assert_eq!(builder.config.max_queue_size, 120);
        });
    }

    #[test]
    fn test_build_batch_log_processor_builder_with_custom_config() {
        let expected = BatchConfigBuilder::default()
            .with_max_export_batch_size(1)
            .with_scheduled_delay(Duration::from_millis(2))
            .with_max_queue_size(4)
            .build();

        let builder =
            BatchLogProcessor::builder(InMemoryLogExporter::default()).with_batch_config(expected);

        let actual = &builder.config;
        assert_eq!(actual.max_export_batch_size, 1);
        assert_eq!(actual.scheduled_delay, Duration::from_millis(2));
        assert_eq!(actual.max_queue_size, 4);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_set_resource_batch_processor() {
        let exporter = MockLogExporter {
            resource: Arc::new(Mutex::new(None)),
        };
        let processor = BatchLogProcessor::new(exporter.clone(), BatchConfig::default());
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
        let processor = BatchLogProcessor::new(exporter.clone(), BatchConfig::default());

        let mut record = SdkLogRecord::new();
        let instrumentation = InstrumentationScope::default();

        processor.emit(&mut record, &instrumentation);
        processor.force_flush().unwrap();
        processor.shutdown().unwrap();
        // todo: expect to see errors here. How should we assert this?
        processor.emit(&mut record, &instrumentation);
        assert_eq!(1, exporter.get_emitted_logs().unwrap().len());
        assert!(exporter.is_shutdown_called());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_batch_log_processor_shutdown_under_async_runtime_current_flavor_multi_thread() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = BatchLogProcessor::new(exporter.clone(), BatchConfig::default());

        processor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_batch_log_processor_shutdown_with_async_runtime_current_flavor_current_thread() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = BatchLogProcessor::new(exporter.clone(), BatchConfig::default());
        processor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_batch_log_processor_shutdown_with_async_runtime_multi_flavor_multi_thread() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = BatchLogProcessor::new(exporter.clone(), BatchConfig::default());
        processor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_batch_log_processor_shutdown_with_async_runtime_multi_flavor_current_thread() {
        let exporter = InMemoryLogExporterBuilder::default().build();
        let processor = BatchLogProcessor::new(exporter.clone(), BatchConfig::default());
        processor.shutdown().unwrap();
    }
}
