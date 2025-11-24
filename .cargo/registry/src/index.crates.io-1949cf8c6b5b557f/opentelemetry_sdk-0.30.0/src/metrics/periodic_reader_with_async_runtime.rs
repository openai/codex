use std::{
    env, fmt, mem,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

use futures_channel::{mpsc, oneshot};
use futures_util::{
    future::{self, Either},
    pin_mut,
    stream::{self, FusedStream},
    StreamExt,
};
use opentelemetry::{otel_debug, otel_error};

use crate::runtime::{to_interval_stream, Runtime};
use crate::{
    error::{OTelSdkError, OTelSdkResult},
    metrics::{exporter::PushMetricExporter, reader::SdkProducer},
    Resource,
};

use super::{
    data::ResourceMetrics, instrument::InstrumentKind, pipeline::Pipeline, reader::MetricReader,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_INTERVAL: Duration = Duration::from_secs(60);

const METRIC_EXPORT_INTERVAL_NAME: &str = "OTEL_METRIC_EXPORT_INTERVAL";
const METRIC_EXPORT_TIMEOUT_NAME: &str = "OTEL_METRIC_EXPORT_TIMEOUT";

/// Configuration options for [PeriodicReader].
///
/// A periodic reader is a [MetricReader] that collects and exports metric data
/// to the exporter at a defined interval.
///
/// By default, the returned [MetricReader] will collect and export data every
/// 60 seconds, and will cancel export attempts that exceed 30 seconds. The
/// export time is not counted towards the interval between attempts.
///
/// The [collect] method of the returned [MetricReader] continues to gather and
/// return metric data to the user. It will not automatically send that data to
/// the exporter outside of the predefined interval.
///
/// [collect]: MetricReader::collect
#[derive(Debug)]
pub struct PeriodicReaderBuilder<E, RT> {
    interval: Duration,
    timeout: Duration,
    exporter: E,
    runtime: RT,
}

impl<E, RT> PeriodicReaderBuilder<E, RT>
where
    E: PushMetricExporter,
    RT: Runtime,
{
    fn new(exporter: E, runtime: RT) -> Self {
        let interval = env::var(METRIC_EXPORT_INTERVAL_NAME)
            .ok()
            .and_then(|v| v.parse().map(Duration::from_millis).ok())
            .unwrap_or(DEFAULT_INTERVAL);
        let timeout = env::var(METRIC_EXPORT_TIMEOUT_NAME)
            .ok()
            .and_then(|v| v.parse().map(Duration::from_millis).ok())
            .unwrap_or(DEFAULT_TIMEOUT);

        PeriodicReaderBuilder {
            interval,
            timeout,
            exporter,
            runtime,
        }
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

    /// Configures the time a [PeriodicReader] waits for an export to complete
    /// before canceling it.
    ///
    /// This option overrides any value set for the `OTEL_METRIC_EXPORT_TIMEOUT`
    /// environment variable.
    ///
    /// If this option is not used or `timeout` is equal to zero, 30 seconds is used
    /// as the default.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        if !timeout.is_zero() {
            self.timeout = timeout;
        }
        self
    }

    /// Create a [PeriodicReader] with the given config.
    pub fn build(self) -> PeriodicReader<E> {
        let (message_sender, message_receiver) = mpsc::channel(256);

        let worker = move |reader: &PeriodicReader<E>| {
            let runtime = self.runtime.clone();
            let reader = reader.clone();
            self.runtime.spawn(async move {
                let ticker = to_interval_stream(runtime.clone(), self.interval)
                    .skip(1) // The ticker is fired immediately, so we should skip the first one to align with the interval.
                    .map(|_| Message::Export);
                let messages = Box::pin(stream::select(message_receiver, ticker));
                PeriodicReaderWorker {
                    reader,
                    timeout: self.timeout,
                    runtime,
                    rm: ResourceMetrics {
                        resource: Resource::empty(),
                        scope_metrics: Vec::new(),
                    },
                }
                .run(messages)
                .await
            });
        };

        otel_debug!(
            name: "PeriodicReader.BuildCompleted",
            message = "Periodic reader built.",
            interval_in_secs = self.interval.as_secs(),
            temporality = format!("{:?}", self.exporter.temporality()),
        );

        PeriodicReader {
            exporter: Arc::new(self.exporter),
            inner: Arc::new(Mutex::new(PeriodicReaderInner {
                message_sender,
                is_shutdown: false,
                sdk_producer_or_worker: ProducerOrWorker::Worker(Box::new(worker)),
            })),
        }
    }
}

/// A [MetricReader] that continuously collects and exports metric data at a set
/// interval.
///
/// By default it will collect and export data every 60 seconds, and will cancel
/// export attempts that exceed 30 seconds. The export time is not counted
/// towards the interval between attempts.
///
/// The [collect] method of the returned continues to gather and
/// return metric data to the user. It will not automatically send that data to
/// the exporter outside of the predefined interval.
///
/// The [runtime] can be selected based on feature flags set for this crate.
///
/// The exporter can be any exporter that implements [PushMetricExporter] such
/// as [opentelemetry-otlp].
///
/// [collect]: MetricReader::collect
/// [runtime]: crate::runtime
/// [opentelemetry-otlp]: https://docs.rs/opentelemetry-otlp/latest/opentelemetry_otlp/
///
/// # Example
///
/// ```no_run
/// use opentelemetry_sdk::metrics::periodic_reader_with_async_runtime::PeriodicReader;
/// # fn example<E, R>(get_exporter: impl Fn() -> E, get_runtime: impl Fn() -> R)
/// # where
/// #     E: opentelemetry_sdk::metrics::exporter::PushMetricExporter,
/// #     R: opentelemetry_sdk::runtime::Runtime,
/// # {
///
/// let exporter = get_exporter(); // set up a push exporter like OTLP
/// let runtime = get_runtime(); // select runtime: e.g. opentelemetry_sdk:runtime::Tokio
///
/// let reader = PeriodicReader::builder(exporter, runtime).build();
/// # drop(reader);
/// # }
/// ```
pub struct PeriodicReader<E: PushMetricExporter> {
    exporter: Arc<E>,
    inner: Arc<Mutex<PeriodicReaderInner<E>>>,
}

impl<E: PushMetricExporter> Clone for PeriodicReader<E> {
    fn clone(&self) -> Self {
        Self {
            exporter: Arc::clone(&self.exporter),
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<E: PushMetricExporter> PeriodicReader<E> {
    /// Configuration options for a periodic reader
    pub fn builder<RT>(exporter: E, runtime: RT) -> PeriodicReaderBuilder<E, RT>
    where
        RT: Runtime,
    {
        PeriodicReaderBuilder::new(exporter, runtime)
    }
}

impl<E: PushMetricExporter> fmt::Debug for PeriodicReader<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeriodicReader").finish()
    }
}

struct PeriodicReaderInner<E: PushMetricExporter> {
    message_sender: mpsc::Sender<Message>,
    is_shutdown: bool,
    sdk_producer_or_worker: ProducerOrWorker<E>,
}

#[derive(Debug)]
enum Message {
    Export,
    Flush(oneshot::Sender<OTelSdkResult>),
    Shutdown(oneshot::Sender<OTelSdkResult>),
}

enum ProducerOrWorker<E: PushMetricExporter> {
    Producer(Weak<dyn SdkProducer>),
    #[allow(clippy::type_complexity)]
    Worker(Box<dyn FnOnce(&PeriodicReader<E>) + Send + Sync>),
}

struct PeriodicReaderWorker<E: PushMetricExporter, RT: Runtime> {
    reader: PeriodicReader<E>,
    timeout: Duration,
    runtime: RT,
    rm: ResourceMetrics,
}

impl<E: PushMetricExporter, RT: Runtime> PeriodicReaderWorker<E, RT> {
    async fn collect_and_export(&mut self) -> OTelSdkResult {
        self.reader
            .collect(&mut self.rm)
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;
        if self.rm.scope_metrics.is_empty() {
            otel_debug!(
                name: "PeriodicReaderWorker.NoMetricsToExport",
            );
            // No metrics to export.
            return Ok(());
        }

        otel_debug!(
            name: "PeriodicReaderWorker.InvokeExporter",
            message = "Calling exporter's export method with collected metrics.",
            count = self.rm.scope_metrics.len(),
        );
        let export = self.reader.exporter.export(&self.rm);
        let timeout = self.runtime.delay(self.timeout);
        pin_mut!(export);
        pin_mut!(timeout);

        match future::select(export, timeout).await {
            Either::Left((res, _)) => {
                res // return the status of export.
            }
            Either::Right(_) => Err(OTelSdkError::Timeout(self.timeout)),
        }
    }

    async fn process_message(&mut self, message: Message) -> bool {
        match message {
            Message::Export => {
                otel_debug!(
                    name: "PeriodicReader.ExportTriggered",
                    message = "Export message received.",
                );
                if let Err(err) = self.collect_and_export().await {
                    otel_error!(
                        name: "PeriodicReader.ExportFailed",
                        message = "Failed to export metrics",
                        reason = format!("{}", err));
                }
            }
            Message::Flush(ch) => {
                otel_debug!(
                    name: "PeriodicReader.ForceFlushCalled",
                    message = "Flush message received.",
                );
                let res = self.collect_and_export().await;
                if let Err(send_error) = ch.send(res) {
                    otel_debug!(
                        name: "PeriodicReader.Flush.SendResultError",
                        message = "Failed to send flush result.",
                        reason = format!("{:?}", send_error),
                    );
                }
            }
            Message::Shutdown(ch) => {
                otel_debug!(
                    name: "PeriodicReader.ShutdownCalled",
                    message = "Shutdown message received",
                );
                let res = self.collect_and_export().await;
                let _ = self.reader.exporter.shutdown();
                if let Err(send_error) =
                    ch.send(res.map_err(|e| OTelSdkError::InternalFailure(e.to_string())))
                {
                    otel_debug!(
                        name: "PeriodicReader.Shutdown.SendResultError",
                        message = "Failed to send shutdown result",
                        reason = format!("{:?}", send_error),
                    );
                }
                return false;
            }
        }

        true
    }

    async fn run(mut self, mut messages: impl FusedStream<Item = Message> + Unpin) {
        while let Some(message) = messages.next().await {
            if !self.process_message(message).await {
                break;
            }
        }
    }
}

impl<E: PushMetricExporter> MetricReader for PeriodicReader<E> {
    fn register_pipeline(&self, pipeline: Weak<Pipeline>) {
        let mut inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let worker = match &mut inner.sdk_producer_or_worker {
            ProducerOrWorker::Producer(_) => {
                // Only register once. If producer is already set, do nothing.
                otel_debug!(name: "PeriodicReader.DuplicateRegistration",
                    message = "duplicate registration found, did not register periodic reader.");
                return;
            }
            ProducerOrWorker::Worker(w) => mem::replace(w, Box::new(|_| {})),
        };

        inner.sdk_producer_or_worker = ProducerOrWorker::Producer(pipeline);
        worker(self);
    }

    fn collect(&self, rm: &mut ResourceMetrics) -> OTelSdkResult {
        let inner = self
            .inner
            .lock()
            .map_err(|_| OTelSdkError::InternalFailure("Failed to lock pipeline".into()))?;

        if inner.is_shutdown {
            return Err(OTelSdkError::AlreadyShutdown);
        }

        if let Some(producer) = match &inner.sdk_producer_or_worker {
            ProducerOrWorker::Producer(sdk_producer) => sdk_producer.upgrade(),
            ProducerOrWorker::Worker(_) => None,
        } {
            producer.produce(rm)?;
        } else {
            return Err(OTelSdkError::InternalFailure(
                "reader is not registered".into(),
            ));
        }

        Ok(())
    }

    fn force_flush(&self) -> OTelSdkResult {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;
        if inner.is_shutdown {
            return Err(OTelSdkError::AlreadyShutdown);
        }
        let (sender, receiver) = oneshot::channel();
        inner
            .message_sender
            .try_send(Message::Flush(sender))
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;

        drop(inner); // don't hold lock when blocking on future

        futures_executor::block_on(receiver)
            .map_err(|err| OTelSdkError::InternalFailure(err.to_string()))
            .and_then(|res| res)
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;
        if inner.is_shutdown {
            return Err(OTelSdkError::AlreadyShutdown);
        }

        let (sender, receiver) = oneshot::channel();
        inner
            .message_sender
            .try_send(Message::Shutdown(sender))
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;
        drop(inner); // don't hold lock when blocking on future

        let shutdown_result = futures_executor::block_on(receiver)
            .map_err(|err| OTelSdkError::InternalFailure(err.to_string()))?;

        // Acquire the lock again to set the shutdown flag
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;
        inner.is_shutdown = true;

        shutdown_result
    }

    /// To construct a [MetricReader][metric-reader] when setting up an SDK,
    /// The output temporality (optional), a function of instrument kind.
    /// This function SHOULD be obtained from the exporter.
    ///
    /// If not configured, the Cumulative temporality SHOULD be used.
    ///  
    /// [metric-reader]: https://github.com/open-telemetry/opentelemetry-specification/blob/0a78571045ca1dca48621c9648ec3c832c3c541c/specification/metrics/sdk.md#metricreader
    fn temporality(&self, kind: InstrumentKind) -> super::Temporality {
        kind.temporality_preference(self.exporter.temporality())
    }
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use super::PeriodicReader;
    use crate::error::OTelSdkError;
    use crate::metrics::reader::MetricReader;
    use crate::{
        metrics::data::ResourceMetrics, metrics::InMemoryMetricExporter, metrics::SdkMeterProvider,
        runtime, Resource,
    };
    use opentelemetry::metrics::MeterProvider;
    use std::sync::mpsc;

    #[test]
    fn collection_triggered_by_interval_tokio_current() {
        collection_triggered_by_interval_helper(runtime::TokioCurrentThread);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn collection_triggered_by_interval_from_tokio_multi_one_thread_on_runtime_tokio() {
        collection_triggered_by_interval_helper(runtime::Tokio);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn collection_triggered_by_interval_from_tokio_multi_two_thread_on_runtime_tokio() {
        collection_triggered_by_interval_helper(runtime::Tokio);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn collection_triggered_by_interval_from_tokio_multi_one_thread_on_runtime_tokio_current()
    {
        collection_triggered_by_interval_helper(runtime::TokioCurrentThread);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn collection_triggered_by_interval_from_tokio_multi_two_thread_on_runtime_tokio_current()
    {
        collection_triggered_by_interval_helper(runtime::TokioCurrentThread);
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "See issue https://github.com/open-telemetry/opentelemetry-rust/issues/2056"]
    async fn collection_triggered_by_interval_from_tokio_current_on_runtime_tokio() {
        collection_triggered_by_interval_helper(runtime::Tokio);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn collection_triggered_by_interval_from_tokio_current_on_runtime_tokio_current() {
        collection_triggered_by_interval_helper(runtime::TokioCurrentThread);
    }

    #[test]
    fn unregistered_collect() {
        // Arrange
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone(), runtime::Tokio).build();
        let mut rm = ResourceMetrics {
            resource: Resource::empty(),
            scope_metrics: Vec::new(),
        };

        // Act
        let result = reader.collect(&mut rm);

        // Assert
        assert!(
            matches!(result.unwrap_err(), OTelSdkError::InternalFailure(err) if err == "reader is not registered")
        );
    }

    fn collection_triggered_by_interval_helper<RT>(runtime: RT)
    where
        RT: crate::runtime::Runtime,
    {
        let interval = std::time::Duration::from_millis(1);
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone(), runtime)
            .with_interval(interval)
            .build();
        let (sender, receiver) = mpsc::channel();

        // Act
        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let meter = meter_provider.meter("test");
        let _counter = meter
            .u64_observable_counter("testcounter")
            .with_callback(move |_| {
                sender.send(()).expect("channel should still be open");
            })
            .build();

        // Assert
        receiver
            .recv()
            .expect("message should be available in channel, indicating a collection occurred");
    }
}
