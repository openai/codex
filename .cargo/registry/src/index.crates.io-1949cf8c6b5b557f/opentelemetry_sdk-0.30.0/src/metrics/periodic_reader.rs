use std::{
    env, fmt,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, Weak,
    },
    thread,
    time::{Duration, Instant},
};

use opentelemetry::{otel_debug, otel_error, otel_info, otel_warn, Context};

use crate::{
    error::{OTelSdkError, OTelSdkResult},
    metrics::{exporter::PushMetricExporter, reader::SdkProducer},
    Resource,
};

use super::{
    data::ResourceMetrics, instrument::InstrumentKind, pipeline::Pipeline, reader::MetricReader,
    Temporality,
};

const DEFAULT_INTERVAL: Duration = Duration::from_secs(60);

const METRIC_EXPORT_INTERVAL_NAME: &str = "OTEL_METRIC_EXPORT_INTERVAL";

/// Configuration options for [PeriodicReader].
#[derive(Debug)]
pub struct PeriodicReaderBuilder<E> {
    interval: Duration,
    exporter: E,
}

impl<E> PeriodicReaderBuilder<E>
where
    E: PushMetricExporter,
{
    fn new(exporter: E) -> Self {
        let interval = env::var(METRIC_EXPORT_INTERVAL_NAME)
            .ok()
            .and_then(|v| v.parse().map(Duration::from_millis).ok())
            .unwrap_or(DEFAULT_INTERVAL);

        PeriodicReaderBuilder { interval, exporter }
    }

    /// Configures the intervening time between exports for a [PeriodicReader].
    ///
    /// This option overrides any value set for the `OTEL_METRIC_EXPORT_INTERVAL`
    /// environment variable.
    ///
    /// If this option is not used or `interval` is equal to zero, 60 seconds is
    /// used as the default.
    pub fn with_interval(mut self, interval: Duration) -> Self {
        if !interval.is_zero() {
            self.interval = interval;
        }
        self
    }

    /// Create a [PeriodicReader] with the given config.
    pub fn build(self) -> PeriodicReader<E> {
        PeriodicReader::new(self.exporter, self.interval)
    }
}

/// A `MetricReader` that periodically collects and exports metrics at a configurable interval.
///
/// By default, [`PeriodicReader`] collects and exports metrics every **60 seconds**.
/// The time taken for export is **not** included in the interval. Use [`PeriodicReaderBuilder`]
/// to customize the interval.
///
/// [`PeriodicReader`] spawns a background thread to handle metric collection and export.
/// This thread remains active until [`shutdown()`] is called.
///
/// ## Collection Process
/// "Collection" refers to gathering aggregated metrics from the SDK's internal storage.
/// During this phase, callbacks from observable instruments are also triggered.
///
/// [`PeriodicReader`] does **not** enforce a timeout for collection. If an
/// observable callback takes too long, it may delay the next collection cycle.
/// If a callback never returns, it **will stall** all metric collection (and exports)
/// indefinitely.
///
/// ## Exporter Compatibility
/// When used with the [`OTLP Exporter`](https://docs.rs/opentelemetry-otlp), the following
/// transport options are supported:
///
/// - **`grpc-tonic`**: Requires [`MeterProvider`] to be initialized within a `tokio` runtime.
/// - **`reqwest-blocking-client`**: Works with both a standard (`main`) function and `tokio::main`.
///
/// [`PeriodicReader`] does **not** enforce a timeout for exports either. Instead,
/// the configured exporter is responsible for enforcing timeouts. If an export operation
/// never returns, [`PeriodicReader`] will **stop exporting new metrics**, stalling
/// metric collection.
///
/// ## Manual Export & Shutdown
/// Users can manually trigger an export via [`force_flush()`]. Calling [`shutdown()`]
/// exports any remaining metrics and should be done before application exit to ensure
/// all data is sent.
///
/// **Warning**: If using **tokio’s current-thread runtime**, calling [`shutdown()`]
/// from the main thread may cause a deadlock. To prevent this, call [`shutdown()`]
/// from a separate thread or use tokio's `spawn_blocking`.
///
/// [`PeriodicReader`]: crate::metrics::PeriodicReader
/// [`PeriodicReaderBuilder`]: crate::metrics::PeriodicReaderBuilder
/// [`MeterProvider`]: crate::metrics::SdkMeterProvider
/// [`shutdown()`]: crate::metrics::SdkMeterProvider::shutdown
/// [`force_flush()`]: crate::metrics::SdkMeterProvider::force_flush
///
/// # Example
///
/// ```no_run
/// use opentelemetry_sdk::metrics::PeriodicReader;
/// # fn example<E>(get_exporter: impl Fn() -> E)
/// # where
/// #     E: opentelemetry_sdk::metrics::exporter::PushMetricExporter,
/// # {
///
/// let exporter = get_exporter(); // set up a push exporter
///
/// let reader = PeriodicReader::builder(exporter).build();
/// # drop(reader);
/// # }
/// ```
pub struct PeriodicReader<E: PushMetricExporter> {
    inner: Arc<PeriodicReaderInner<E>>,
}

impl<E: PushMetricExporter> Clone for PeriodicReader<E> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<E: PushMetricExporter> PeriodicReader<E> {
    /// Configuration options for a periodic reader with own thread
    pub fn builder(exporter: E) -> PeriodicReaderBuilder<E> {
        PeriodicReaderBuilder::new(exporter)
    }

    fn new(exporter: E, interval: Duration) -> Self {
        let (message_sender, message_receiver): (Sender<Message>, Receiver<Message>) =
            mpsc::channel();
        let exporter_arc = Arc::new(exporter);
        let reader = PeriodicReader {
            inner: Arc::new(PeriodicReaderInner {
                message_sender,
                producer: Mutex::new(None),
                exporter: exporter_arc.clone(),
            }),
        };
        let cloned_reader = reader.clone();

        let mut rm = ResourceMetrics {
            resource: Resource::empty(),
            scope_metrics: Vec::new(),
        };

        let result_thread_creation = thread::Builder::new()
            .name("OpenTelemetry.Metrics.PeriodicReader".to_string())
            .spawn(move || {
                let _suppress_guard = Context::enter_telemetry_suppressed_scope();
                let mut interval_start = Instant::now();
                let mut remaining_interval = interval;
                otel_debug!(
                    name: "PeriodReaderThreadStarted",
                    interval_in_millisecs = interval.as_millis(),
                );
                loop {
                    otel_debug!(
                        name: "PeriodReaderThreadLoopAlive", message = "Next export will happen after interval, unless flush or shutdown is triggered.", interval_in_millisecs = remaining_interval.as_millis()
                    );
                    match message_receiver.recv_timeout(remaining_interval) {
                        Ok(Message::Flush(response_sender)) => {
                            otel_debug!(
                                name: "PeriodReaderThreadExportingDueToFlush"
                            );
                            let export_result = cloned_reader.collect_and_export(&mut rm);
                            otel_debug!(
                                name: "PeriodReaderInvokedExport",
                                export_result = format!("{:?}", export_result)
                            );

                            // If response_sender is disconnected, we can't send
                            // the result back. This occurs when the thread that
                            // initiated flush gave up due to timeout.
                            // Gracefully handle that with internal logs. The
                            // internal errors are of Info level, as this is
                            // useful for user to know whether the flush was
                            // successful or not, when flush() itself merely
                            // tells that it timed out.

                            if export_result.is_err() {
                                if response_sender.send(false).is_err() {
                                    otel_debug!(
                                        name: "PeriodReader.Flush.ResponseSendError",
                                        message = "PeriodicReader's flush has failed, but unable to send this info back to caller.
                                        This occurs when the caller has timed out waiting for the response. If you see this occuring frequently, consider increasing the flush timeout."
                                    );
                                }
                            } else if response_sender.send(true).is_err() {
                                otel_debug!(
                                    name: "PeriodReader.Flush.ResponseSendError",
                                    message = "PeriodicReader's flush has completed successfully, but unable to send this info back to caller.
                                    This occurs when the caller has timed out waiting for the response. If you see this occuring frequently, consider increasing the flush timeout."
                                );
                            }

                            // Adjust the remaining interval after the flush
                            let elapsed = interval_start.elapsed();
                            if elapsed < interval {
                                remaining_interval = interval - elapsed;
                                otel_debug!(
                                    name: "PeriodReaderThreadAdjustingRemainingIntervalAfterFlush",
                                    remaining_interval = remaining_interval.as_secs()
                                );
                            } else {
                                otel_debug!(
                                    name: "PeriodReaderThreadAdjustingExportAfterFlush",
                                );
                                // Reset the interval if the flush finishes after the expected export time
                                // effectively missing the normal export.
                                // Should we attempt to do the missed export immediately?
                                // Or do the next export at the next interval?
                                // Currently this attempts the next export immediately.
                                // i.e calling Flush can affect the regularity.
                                interval_start = Instant::now();
                                remaining_interval = Duration::ZERO;
                            }
                        }
                        Ok(Message::Shutdown(response_sender)) => {
                            // Perform final export and break out of loop and exit the thread
                            otel_debug!(name: "PeriodReaderThreadExportingDueToShutdown");
                            let export_result = cloned_reader.collect_and_export(&mut rm);
                            otel_debug!(
                                name: "PeriodReaderInvokedExport",
                                export_result = format!("{:?}", export_result)
                            );
                            let shutdown_result = exporter_arc.shutdown();
                            otel_debug!(
                                name: "PeriodReaderInvokedExporterShutdown",
                                shutdown_result = format!("{:?}", shutdown_result)
                            );

                            // If response_sender is disconnected, we can't send
                            // the result back. This occurs when the thread that
                            // initiated shutdown gave up due to timeout.
                            // Gracefully handle that with internal logs and
                            // continue with shutdown (i.e exit thread) The
                            // internal errors are of Info level, as this is
                            // useful for user to know whether the shutdown was
                            // successful or not, when shutdown() itself merely
                            // tells that it timed out.
                            if export_result.is_err() || shutdown_result.is_err() {
                                if response_sender.send(false).is_err() {
                                    otel_info!(
                                        name: "PeriodReaderThreadShutdown.ResponseSendError",
                                        message = "PeriodicReader's shutdown has failed, but unable to send this info back to caller.
                                        This occurs when the caller has timed out waiting for the response. If you see this occuring frequently, consider increasing the shutdown timeout."
                                    );
                                }
                            } else if response_sender.send(true).is_err() {
                                otel_debug!(
                                    name: "PeriodReaderThreadShutdown.ResponseSendError",
                                    message = "PeriodicReader completed its shutdown, but unable to send this info back to caller.
                                    This occurs when the caller has timed out waiting for the response. If you see this occuring frequently, consider increasing the shutdown timeout."
                                );
                            }

                            otel_debug!(
                                name: "PeriodReaderThreadExiting",
                                reason = "ShutdownRequested"
                            );
                            break;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let export_start = Instant::now();
                            otel_debug!(
                                name: "PeriodReaderThreadExportingDueToTimer"
                            );

                            let export_result = cloned_reader.collect_and_export(&mut rm);
                            otel_debug!(
                                name: "PeriodReaderInvokedExport",
                                export_result = format!("{:?}", export_result)
                            );

                            let time_taken_for_export = export_start.elapsed();
                            if time_taken_for_export > interval {
                                otel_debug!(
                                    name: "PeriodReaderThreadExportTookLongerThanInterval"
                                );
                                // if export took longer than interval, do the
                                // next export immediately.
                                // Alternatively, we could skip the next export
                                // and wait for the next interval.
                                // Or enforce that export timeout is less than interval.
                                // What is the desired behavior?
                                interval_start = Instant::now();
                                remaining_interval = Duration::ZERO;
                            } else {
                                remaining_interval = interval - time_taken_for_export;
                                interval_start = Instant::now();
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            // Channel disconnected, only thing to do is break
                            // out (i.e exit the thread)
                            otel_debug!(
                                name: "PeriodReaderThreadExiting",
                                reason = "MessageSenderDisconnected"
                            );
                            break;
                        }
                    }
                }
                otel_debug!(
                    name: "PeriodReaderThreadStopped"
                );
            });

        // TODO: Should we fail-fast here and bubble up the error to user?
        #[allow(unused_variables)]
        if let Err(e) = result_thread_creation {
            otel_error!(
                name: "PeriodReaderThreadStartError",
                message = "Failed to start PeriodicReader thread. Metrics will not be exported.",
                error = format!("{:?}", e)
            );
        }
        reader
    }

    fn collect_and_export(&self, rm: &mut ResourceMetrics) -> OTelSdkResult {
        self.inner.collect_and_export(rm)
    }
}

impl<E: PushMetricExporter> fmt::Debug for PeriodicReader<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeriodicReader").finish()
    }
}

struct PeriodicReaderInner<E: PushMetricExporter> {
    exporter: Arc<E>,
    message_sender: mpsc::Sender<Message>,
    producer: Mutex<Option<Weak<dyn SdkProducer>>>,
}

impl<E: PushMetricExporter> PeriodicReaderInner<E> {
    fn register_pipeline(&self, producer: Weak<dyn SdkProducer>) {
        let mut inner = self.producer.lock().expect("lock poisoned");
        *inner = Some(producer);
    }

    fn temporality(&self, _kind: InstrumentKind) -> Temporality {
        self.exporter.temporality()
    }

    fn collect(&self, rm: &mut ResourceMetrics) -> OTelSdkResult {
        let producer = self.producer.lock().expect("lock poisoned");
        if let Some(p) = producer.as_ref() {
            p.upgrade()
                .ok_or(OTelSdkError::AlreadyShutdown)?
                .produce(rm)?;
            Ok(())
        } else {
            otel_warn!(
            name: "PeriodReader.MeterProviderNotRegistered",
            message = "PeriodicReader is not registered with MeterProvider. Metrics will not be collected. \
                   This occurs when a periodic reader is created but not associated with a MeterProvider \
                   by calling `.with_reader(reader)` on MeterProviderBuilder."
            );
            Err(OTelSdkError::InternalFailure(
                "MeterProvider is not registered".into(),
            ))
        }
    }

    fn collect_and_export(&self, rm: &mut ResourceMetrics) -> OTelSdkResult {
        let current_time = Instant::now();
        let collect_result = self.collect(rm);
        let time_taken_for_collect = current_time.elapsed();

        #[allow(clippy::question_mark)]
        if let Err(e) = collect_result {
            otel_warn!(
                name: "PeriodReaderCollectError",
                error = format!("{:?}", e)
            );
            return Err(OTelSdkError::InternalFailure(e.to_string()));
        }

        if rm.scope_metrics.is_empty() {
            otel_debug!(name: "NoMetricsCollected");
            return Ok(());
        }

        let metrics_count = rm.scope_metrics.iter().fold(0, |count, scope_metrics| {
            count + scope_metrics.metrics.len()
        });
        otel_debug!(name: "PeriodicReaderMetricsCollected", count = metrics_count, time_taken_in_millis = time_taken_for_collect.as_millis());

        // Relying on futures executor to execute async call.
        // TODO: Pass timeout to exporter
        futures_executor::block_on(self.exporter.export(rm))
    }

    fn force_flush(&self) -> OTelSdkResult {
        // TODO: Better message for this scenario.
        // Flush and Shutdown called from 2 threads Flush check shutdown
        // flag before shutdown thread sets it. Both threads attempt to send
        // message to the same channel. Case1: Flush thread sends message first,
        // shutdown thread sends message next. Flush would succeed, as
        // background thread won't process shutdown message until flush
        // triggered export is done. Case2: Shutdown thread sends message first,
        // flush thread sends message next. Shutdown would succeed, as
        // background thread would process shutdown message first. The
        // background exits so it won't receive the flush message. ForceFlush
        // returns Failure, but we could indicate specifically that shutdown has
        // completed. TODO is to see if this message can be improved.

        let (response_tx, response_rx) = mpsc::channel();
        self.message_sender
            .send(Message::Flush(response_tx))
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;

        if let Ok(response) = response_rx.recv() {
            // TODO: call exporter's force_flush method.
            if response {
                Ok(())
            } else {
                Err(OTelSdkError::InternalFailure("Failed to flush".into()))
            }
        } else {
            Err(OTelSdkError::InternalFailure("Failed to flush".into()))
        }
    }

    fn shutdown(&self) -> OTelSdkResult {
        // TODO: See if this is better to be created upfront.
        let (response_tx, response_rx) = mpsc::channel();
        self.message_sender
            .send(Message::Shutdown(response_tx))
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;

        // TODO: Make this timeout configurable.
        match response_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(response) => {
                if response {
                    Ok(())
                } else {
                    Err(OTelSdkError::InternalFailure("Failed to shutdown".into()))
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err(OTelSdkError::Timeout(Duration::from_secs(5)))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(OTelSdkError::InternalFailure("Failed to shutdown".into()))
            }
        }
    }
}

#[derive(Debug)]
enum Message {
    Flush(Sender<bool>),
    Shutdown(Sender<bool>),
}

impl<E: PushMetricExporter> MetricReader for PeriodicReader<E> {
    fn register_pipeline(&self, pipeline: Weak<Pipeline>) {
        self.inner.register_pipeline(pipeline);
    }

    fn collect(&self, rm: &mut ResourceMetrics) -> OTelSdkResult {
        self.inner.collect(rm)
    }

    fn force_flush(&self) -> OTelSdkResult {
        self.inner.force_flush()
    }

    // TODO: Offer an async version of shutdown so users can await the shutdown
    // completion, and avoid blocking the thread. The default shutdown on drop
    // can still use blocking call. If user already explicitly called shutdown,
    // drop won't call shutdown again.
    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        self.inner.shutdown()
    }

    /// To construct a [MetricReader][metric-reader] when setting up an SDK,
    /// The output temporality (optional), a function of instrument kind.
    /// This function SHOULD be obtained from the exporter.
    ///
    /// If not configured, the Cumulative temporality SHOULD be used.
    ///
    /// [metric-reader]: https://github.com/open-telemetry/opentelemetry-specification/blob/0a78571045ca1dca48621c9648ec3c832c3c541c/specification/metrics/sdk.md#metricreader
    fn temporality(&self, kind: InstrumentKind) -> Temporality {
        kind.temporality_preference(self.inner.temporality(kind))
    }
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use super::PeriodicReader;
    use crate::{
        error::{OTelSdkError, OTelSdkResult},
        metrics::{
            data::ResourceMetrics, exporter::PushMetricExporter, reader::MetricReader,
            InMemoryMetricExporter, SdkMeterProvider, Temporality,
        },
        Resource,
    };
    use opentelemetry::metrics::MeterProvider;
    use std::{
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            mpsc, Arc,
        },
        time::Duration,
    };

    // use below command to run all tests
    // cargo test metrics::periodic_reader::tests --features=testing,spec_unstable_metrics_views -- --nocapture

    #[derive(Debug, Clone)]
    struct MetricExporterThatFailsOnlyOnFirst {
        count: Arc<AtomicUsize>,
    }

    impl Default for MetricExporterThatFailsOnlyOnFirst {
        fn default() -> Self {
            MetricExporterThatFailsOnlyOnFirst {
                count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl MetricExporterThatFailsOnlyOnFirst {
        fn get_count(&self) -> usize {
            self.count.load(Ordering::Relaxed)
        }
    }

    impl PushMetricExporter for MetricExporterThatFailsOnlyOnFirst {
        async fn export(&self, _metrics: &ResourceMetrics) -> OTelSdkResult {
            if self.count.fetch_add(1, Ordering::Relaxed) == 0 {
                Err(OTelSdkError::InternalFailure("export failed".into()))
            } else {
                Ok(())
            }
        }

        fn force_flush(&self) -> OTelSdkResult {
            Ok(())
        }

        fn shutdown(&self) -> OTelSdkResult {
            Ok(())
        }

        fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
            Ok(())
        }

        fn temporality(&self) -> Temporality {
            Temporality::Cumulative
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MockMetricExporter {
        is_shutdown: Arc<AtomicBool>,
    }

    impl PushMetricExporter for MockMetricExporter {
        async fn export(&self, _metrics: &ResourceMetrics) -> OTelSdkResult {
            Ok(())
        }

        fn force_flush(&self) -> OTelSdkResult {
            Ok(())
        }

        fn shutdown(&self) -> OTelSdkResult {
            self.shutdown_with_timeout(Duration::from_secs(5))
        }

        fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
            self.is_shutdown.store(true, Ordering::Relaxed);
            Ok(())
        }

        fn temporality(&self) -> Temporality {
            Temporality::Cumulative
        }
    }

    #[test]
    fn collection_triggered_by_interval_multiple() {
        // Arrange
        let interval = std::time::Duration::from_millis(1);
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone())
            .with_interval(interval)
            .build();
        let i = Arc::new(AtomicUsize::new(0));
        let i_clone = i.clone();

        // Act
        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let meter = meter_provider.meter("test");
        let _counter = meter
            .u64_observable_counter("testcounter")
            .with_callback(move |_| {
                i_clone.fetch_add(1, Ordering::Relaxed);
            })
            .build();

        // Sleep for a duration 5X (plus liberal buffer to account for potential
        // CI slowness) the interval to ensure multiple collection.
        // Not a fan of such tests, but this seems to be the only way to test
        // if periodic reader is doing its job.
        // TODO: Decide if this should be ignored in CI
        std::thread::sleep(interval * 5 * 20);

        // Assert
        assert!(i.load(Ordering::Relaxed) >= 5);
    }

    #[test]
    fn shutdown_repeat() {
        // Arrange
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone()).build();

        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let result = meter_provider.shutdown();
        assert!(result.is_ok());

        // calling shutdown again should return Err
        let result = meter_provider.shutdown();
        assert!(result.is_err());
        assert!(matches!(result, Err(OTelSdkError::AlreadyShutdown)));

        // calling shutdown again should return Err
        let result = meter_provider.shutdown();
        assert!(result.is_err());
        assert!(matches!(result, Err(OTelSdkError::AlreadyShutdown)));
    }

    #[test]
    fn flush_after_shutdown() {
        // Arrange
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone()).build();

        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let result = meter_provider.force_flush();
        assert!(result.is_ok());

        let result = meter_provider.shutdown();
        assert!(result.is_ok());

        // calling force_flush after shutdown should return Err
        let result = meter_provider.force_flush();
        assert!(result.is_err());
    }

    #[test]
    fn flush_repeat() {
        // Arrange
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone()).build();

        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let result = meter_provider.force_flush();
        assert!(result.is_ok());

        // calling force_flush again should return Ok
        let result = meter_provider.force_flush();
        assert!(result.is_ok());
    }

    #[test]
    fn periodic_reader_without_pipeline() {
        // Arrange
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone()).build();

        let rm = &mut ResourceMetrics {
            resource: Resource::empty(),
            scope_metrics: Vec::new(),
        };
        // Pipeline is not registered, so collect should return an error
        let result = reader.collect(rm);
        assert!(result.is_err());

        // Pipeline is not registered, so flush should return an error
        let result = reader.force_flush();
        assert!(result.is_err());

        // Adding reader to meter provider should register the pipeline
        // TODO: This part might benefit from a different design.
        let meter_provider = SdkMeterProvider::builder()
            .with_reader(reader.clone())
            .build();

        // Now collect and flush should succeed
        let result = reader.collect(rm);
        assert!(result.is_ok());

        let result = meter_provider.force_flush();
        assert!(result.is_ok());
    }

    #[test]
    fn exporter_failures_are_handled() {
        // create a mock exporter that fails 1st time and succeeds 2nd time
        // Validate using this exporter that periodic reader can handle exporter failure
        // and continue to export metrics.
        // Arrange
        let interval = std::time::Duration::from_millis(10);
        let exporter = MetricExporterThatFailsOnlyOnFirst::default();
        let reader = PeriodicReader::builder(exporter.clone())
            .with_interval(interval)
            .build();

        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let meter = meter_provider.meter("test");
        let counter = meter.u64_counter("sync_counter").build();
        counter.add(1, &[]);
        let _obs_counter = meter
            .u64_observable_counter("testcounter")
            .with_callback(move |observer| {
                observer.observe(1, &[]);
            })
            .build();

        // Sleep for a duration much longer than the interval to trigger
        // multiple exports, including failures.
        // Not a fan of such tests, but this seems to be the
        // only way to test if periodic reader is doing its job. TODO: Decide if
        // this should be ignored in CI
        std::thread::sleep(Duration::from_millis(500));

        // Assert that atleast 2 exports are attempted given the 1st one fails.
        assert!(exporter.get_count() >= 2);
    }

    #[test]
    fn shutdown_passed_to_exporter() {
        // Arrange
        let exporter = MockMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone()).build();

        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let meter = meter_provider.meter("test");
        let counter = meter.u64_counter("sync_counter").build();
        counter.add(1, &[]);

        // shutdown the provider, which should call shutdown on periodic reader
        // which in turn should call shutdown on exporter.
        let result = meter_provider.shutdown();
        assert!(result.is_ok());
        assert!(exporter.is_shutdown.load(Ordering::Relaxed));
    }

    #[test]
    fn collection() {
        collection_triggered_by_interval_helper();
        collection_triggered_by_flush_helper();
        collection_triggered_by_shutdown_helper();
        collection_triggered_by_drop_helper();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn collection_from_tokio_multi_with_one_worker() {
        collection_triggered_by_interval_helper();
        collection_triggered_by_flush_helper();
        collection_triggered_by_shutdown_helper();
        collection_triggered_by_drop_helper();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn collection_from_tokio_with_two_worker() {
        collection_triggered_by_interval_helper();
        collection_triggered_by_flush_helper();
        collection_triggered_by_shutdown_helper();
        collection_triggered_by_drop_helper();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn collection_from_tokio_current() {
        collection_triggered_by_interval_helper();
        collection_triggered_by_flush_helper();
        collection_triggered_by_shutdown_helper();
        collection_triggered_by_drop_helper();
    }

    fn collection_triggered_by_interval_helper() {
        collection_helper(|_| {
            // Sleep for a duration longer than the interval to ensure at least one collection
            // Not a fan of such tests, but this seems to be the only way to test
            // if periodic reader is doing its job.
            // TODO: Decide if this should be ignored in CI
            std::thread::sleep(Duration::from_millis(500));
        });
    }

    fn collection_triggered_by_flush_helper() {
        collection_helper(|meter_provider| {
            meter_provider.force_flush().expect("flush should succeed");
        });
    }

    fn collection_triggered_by_shutdown_helper() {
        collection_helper(|meter_provider| {
            meter_provider.shutdown().expect("shutdown should succeed");
        });
    }

    fn collection_triggered_by_drop_helper() {
        collection_helper(|meter_provider| {
            drop(meter_provider);
        });
    }

    fn collection_helper(trigger: fn(SdkMeterProvider)) {
        // Arrange
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone()).build();
        let (sender, receiver) = mpsc::channel();

        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let meter = meter_provider.meter("test");
        let _counter = meter
            .u64_observable_counter("testcounter")
            .with_callback(move |observer| {
                observer.observe(1, &[]);
                sender.send(()).expect("channel should still be open");
            })
            .build();

        // Act
        trigger(meter_provider);

        // Assert
        receiver
            .recv_timeout(Duration::ZERO)
            .expect("message should be available in channel, indicating a collection occurred, which should trigger observable callback");

        let exported_metrics = exporter
            .get_finished_metrics()
            .expect("this should not fail");
        assert!(
            !exported_metrics.is_empty(),
            "Metrics should be available in exporter."
        );
    }

    async fn some_async_function() -> u64 {
        // No dependency on any particular async runtime.
        std::thread::sleep(std::time::Duration::from_millis(1));
        1
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn async_inside_observable_callback_from_tokio_multi_with_one_worker() {
        async_inside_observable_callback_helper();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn async_inside_observable_callback_from_tokio_multi_with_two_worker() {
        async_inside_observable_callback_helper();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn async_inside_observable_callback_from_tokio_current_thread() {
        async_inside_observable_callback_helper();
    }

    #[test]
    fn async_inside_observable_callback_from_regular_main() {
        async_inside_observable_callback_helper();
    }

    fn async_inside_observable_callback_helper() {
        let interval = std::time::Duration::from_millis(10);
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone())
            .with_interval(interval)
            .build();

        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let meter = meter_provider.meter("test");
        let _gauge = meter
            .u64_observable_gauge("my_observable_gauge")
            .with_callback(|observer| {
                // using futures_executor::block_on intentionally and avoiding
                // any particular async runtime.
                let value = futures_executor::block_on(some_async_function());
                observer.observe(value, &[]);
            })
            .build();

        meter_provider.force_flush().expect("flush should succeed");
        let exported_metrics = exporter
            .get_finished_metrics()
            .expect("this should not fail");
        assert!(
            !exported_metrics.is_empty(),
            "Metrics should be available in exporter."
        );
    }

    async fn some_tokio_async_function() -> u64 {
        // Tokio specific async function
        tokio::time::sleep(Duration::from_millis(1)).await;
        1
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]

    async fn tokio_async_inside_observable_callback_from_tokio_multi_with_one_worker() {
        tokio_async_inside_observable_callback_helper(true);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tokio_async_inside_observable_callback_from_tokio_multi_with_two_worker() {
        tokio_async_inside_observable_callback_helper(true);
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore] //TODO: Investigate if this can be fixed.
    async fn tokio_async_inside_observable_callback_from_tokio_current_thread() {
        tokio_async_inside_observable_callback_helper(true);
    }

    #[test]
    fn tokio_async_inside_observable_callback_from_regular_main() {
        tokio_async_inside_observable_callback_helper(false);
    }

    fn tokio_async_inside_observable_callback_helper(use_current_tokio_runtime: bool) {
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone()).build();

        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let meter = meter_provider.meter("test");

        if use_current_tokio_runtime {
            let rt = tokio::runtime::Handle::current().clone();
            let _gauge = meter
                .u64_observable_gauge("my_observable_gauge")
                .with_callback(move |observer| {
                    // call tokio specific async function from here
                    let value = rt.block_on(some_tokio_async_function());
                    observer.observe(value, &[]);
                })
                .build();
            // rt here is a reference to the current tokio runtime.
            // Dropping it occurs when the tokio::main itself ends.
        } else {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let _gauge = meter
                .u64_observable_gauge("my_observable_gauge")
                .with_callback(move |observer| {
                    // call tokio specific async function from here
                    let value = rt.block_on(some_tokio_async_function());
                    observer.observe(value, &[]);
                })
                .build();
            // rt is not dropped here as it is moved to the closure,
            // and is dropped only when MeterProvider itself is dropped.
            // This works when called from normal main.
        };

        meter_provider.force_flush().expect("flush should succeed");
        let exported_metrics = exporter
            .get_finished_metrics()
            .expect("this should not fail");
        assert!(
            !exported_metrics.is_empty(),
            "Metrics should be available in exporter."
        );
    }
}
