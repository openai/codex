use crate::metrics::DEFAULT_QUEUE_CAPACITY;
use crate::metrics::DEFAULT_SHUTDOWN_TIMEOUT;
use crate::metrics::SHUTDOWN_POLL_INTERVAL;
use crate::metrics::batch::HistogramBuckets;
use crate::metrics::batch::MetricEvent;
use crate::metrics::batch::MetricsBatch;
use crate::metrics::config::MetricsConfig;
use crate::metrics::config::MetricsExporter;
use crate::metrics::error::MetricsError;
use crate::metrics::error::Result;
use crate::metrics::tags::merge_tags;
use crate::metrics::tags::tags_to_attributes;
use crate::metrics::time::duration_to_millis;
use crate::metrics::util::error_or_panic;
use crate::metrics::validation::validate_tags;
use opentelemetry::KeyValue;
use opentelemetry::metrics::Histogram;
use opentelemetry::metrics::Meter;
use opentelemetry::metrics::MeterProvider;
use opentelemetry::metrics::UpDownCounter;
use opentelemetry_otlp::MetricExporter;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_sdk::metrics::PeriodicReader;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::sync::mpsc::TrySendError;
use std::thread;
use std::time::Duration;
use std::time::Instant;

const METER_NAME: &str = "codex-otel-metrics";

enum WorkerMessage {
    Batch(MetricsBatch),
    Shutdown,
}

struct WorkerState {
    sender: Mutex<Option<mpsc::SyncSender<WorkerMessage>>>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
    capacity: usize,
    meter_provider: Mutex<Option<SdkMeterProvider>>,
}

#[derive(Debug)]
struct MetricRecorder {
    meter: Meter,
    counters: HashMap<String, UpDownCounter<i64>>,
    histograms: HashMap<String, Histogram<f64>>,
    default_tags: BTreeMap<String, String>,
}

impl MetricRecorder {
    fn new(meter: Meter, default_tags: BTreeMap<String, String>) -> Self {
        Self {
            meter,
            counters: HashMap::new(),
            histograms: HashMap::new(),
            default_tags,
        }
    }

    fn record_batch(&mut self, batch: MetricsBatch) {
        for event in batch.into_events() {
            match event {
                MetricEvent::Counter { name, value, tags } => {
                    self.record_counter(&name, value, &tags);
                }
                MetricEvent::Histogram { name, value, tags } => {
                    self.record_histogram(&name, value, &tags);
                }
            }
        }
    }

    fn record_counter(&mut self, name: &str, value: i64, tags: &[(String, String)]) {
        let attributes = self.attributes_for(tags);
        let name = name.to_string();
        let counter = self
            .counters
            .entry(name.clone())
            .or_insert_with(|| self.meter.i64_up_down_counter(name.clone()).build());
        counter.add(value, &attributes);
    }

    fn record_histogram(&mut self, name: &str, value: i64, tags: &[(String, String)]) {
        let attributes = self.attributes_for(tags);
        let name = name.to_string();
        let histogram = self
            .histograms
            .entry(name.clone())
            .or_insert_with(|| self.meter.f64_histogram(name.clone()).build());
        histogram.record(value as f64, &attributes);
    }

    fn attributes_for(&self, tags: &[(String, String)]) -> Vec<KeyValue> {
        let merged = merge_tags(&self.default_tags, tags);
        tags_to_attributes(&merged)
    }
}

/// Background metrics client that enqueues metrics to a dedicated worker thread.
#[derive(Clone)]
pub struct MetricsClient {
    state: Arc<WorkerState>,
}

impl std::fmt::Debug for MetricsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsClient")
            .field("capacity", &self.state.capacity)
            .finish()
    }
}

impl MetricsClient {
    /// Build a metrics client from configuration and validate defaults.
    pub fn new(config: MetricsConfig) -> Result<Self> {
        let capacity = DEFAULT_QUEUE_CAPACITY;

        if capacity == 0 {
            return Err(MetricsError::QueueCapacityZero);
        }

        if config.endpoint.is_empty() {
            return Err(MetricsError::EmptyEndpoint);
        }

        if config.api_key.is_empty() {
            return Err(MetricsError::EmptyApiKey);
        }

        validate_tags(&config.default_tags)?;

        let meter_provider = build_meter_provider(&config)?;
        let meter = meter_provider.meter(METER_NAME);

        let recorder = MetricRecorder::new(meter, config.default_tags);

        let (sender, receiver) = mpsc::sync_channel(capacity);
        let worker_provider = meter_provider.clone();
        let handle = thread::spawn(move || run_worker(recorder, receiver, worker_provider));

        Ok(Self {
            state: Arc::new(WorkerState {
                sender: Mutex::new(Some(sender)),
                handle: Mutex::new(Some(handle)),
                capacity,
                meter_provider: Mutex::new(Some(meter_provider)),
            }),
        })
    }

    /// Send a single counter increment without blocking the caller.
    pub fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) -> Result<()> {
        let mut batch = MetricsBatch::new();
        batch.counter(name, inc, tags)?;
        self.send(batch)
    }

    /// Send a single histogram sample.
    pub fn histogram(
        &self,
        name: &str,
        value: i64,
        buckets: &HistogramBuckets,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        let mut batch = MetricsBatch::new();
        batch.histogram(name, value, buckets, tags)?;
        self.send(batch)
    }

    /// Record a duration in milliseconds using a histogram.
    pub fn record_duration(
        &self,
        name: &str,
        duration: Duration,
        buckets: &HistogramBuckets,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        let millis = duration_to_millis(duration);
        self.histogram(name, millis, buckets, tags)
    }

    /// Measure a closure and emit a histogram sample for the elapsed time.
    pub fn time<T>(
        &self,
        name: &str,
        buckets: &HistogramBuckets,
        tags: &[(&str, &str)],
        f: impl FnOnce() -> T,
    ) -> Result<T> {
        let start = Instant::now();
        let output = f();
        self.record_duration(name, start.elapsed(), buckets, tags)?;
        Ok(output)
    }

    /// Measure a closure that returns a metrics result without nesting results.
    pub fn time_result<T>(
        &self,
        name: &str,
        buckets: &HistogramBuckets,
        tags: &[(&str, &str)],
        f: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let start = Instant::now();
        let output = f();
        match output {
            Ok(value) => {
                self.record_duration(name, start.elapsed(), buckets, tags)?;
                Ok(value)
            }
            Err(err) => {
                let _ = self.record_duration(name, start.elapsed(), buckets, tags);
                Err(err)
            }
        }
    }

    /// Create an empty batch for multi-metric sends.
    pub fn batch(&self) -> MetricsBatch {
        MetricsBatch::new()
    }

    /// Enqueue a batch of metrics for the worker to send.
    pub fn send(&self, batch: MetricsBatch) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let sender = self
            .state
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(sender) = sender.as_ref() else {
            return Err(MetricsError::WorkerUnavailable);
        };

        match sender.try_send(WorkerMessage::Batch(batch)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(MetricsError::QueueFull {
                capacity: self.state.capacity,
            }),
            Err(TrySendError::Disconnected(_)) => Err(MetricsError::WorkerUnavailable),
        }
    }

    /// Flush queued metrics and stop the worker thread.
    pub fn shutdown(&self) -> Result<()> {
        self.shutdown_inner(DEFAULT_SHUTDOWN_TIMEOUT)
    }

    fn shutdown_inner(&self, timeout: Duration) -> Result<()> {
        let sender = self
            .state
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let mut handle = self
            .state
            .handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut meter_provider = self
            .state
            .meter_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(handle) = handle.take() else {
            return Ok(());
        };
        let mut joined = false;

        if let Some(sender) = sender {
            let _ = sender.try_send(WorkerMessage::Shutdown);
        }

        if timeout.is_zero() {
            if handle.is_finished() {
                handle.join().map_err(|_| MetricsError::WorkerPanicked)?;
                joined = true;
            }
        } else {
            let start = Instant::now();
            while start.elapsed() < timeout {
                if handle.is_finished() {
                    handle.join().map_err(|_| MetricsError::WorkerPanicked)?;
                    joined = true;
                    break;
                }
                thread::sleep(SHUTDOWN_POLL_INTERVAL);
            }
        }

        if joined && let Some(meter_provider) = meter_provider.take() {
            meter_provider
                .force_flush()
                .map_err(|source| MetricsError::FlushFailed { source })?;
            meter_provider
                .shutdown()
                .map_err(|source| MetricsError::ShutdownFailed { source })?;
        }

        Ok(())
    }
}

impl Drop for MetricsClient {
    fn drop(&mut self) {
        if Arc::strong_count(&self.state) == 1 {
            let _ = self.shutdown_inner(DEFAULT_SHUTDOWN_TIMEOUT);
        }
    }
}

fn build_meter_provider(config: &MetricsConfig) -> Result<SdkMeterProvider> {
    match &config.exporter {
        MetricsExporter::OtlpHttp => build_otlp_http_provider(config),
        #[cfg(test)]
        MetricsExporter::InMemory(exporter) => {
            let reader = PeriodicReader::builder(exporter.clone()).build();
            Ok(SdkMeterProvider::builder().with_reader(reader).build())
        }
    }
}

fn build_otlp_http_provider(config: &MetricsConfig) -> Result<SdkMeterProvider> {
    let mut headers = HashMap::new();
    headers.insert(config.api_key_header.clone(), config.api_key.clone());
    if !config.user_agent.is_empty() {
        headers.insert("User-Agent".to_string(), config.user_agent.clone());
    }

    let exporter = MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(config.endpoint.clone())
        .with_timeout(config.timeout)
        .with_headers(headers)
        .build()
        .map_err(|source| MetricsError::ExporterBuild { source })?;

    let reader = PeriodicReader::builder(exporter)
        .with_interval(config.export_interval)
        .build();

    Ok(SdkMeterProvider::builder().with_reader(reader).build())
}

fn run_worker(
    mut recorder: MetricRecorder,
    receiver: mpsc::Receiver<WorkerMessage>,
    meter_provider: SdkMeterProvider,
) {
    for message in receiver {
        match message {
            WorkerMessage::Batch(batch) => {
                recorder.record_batch(batch);
                if let Err(err) = meter_provider.force_flush() {
                    error_or_panic(format!("metrics flush failed: {err}"));
                }
            }
            WorkerMessage::Shutdown => break,
        }
    }
}
