use crate::metrics::DEFAULT_QUEUE_CAPACITY;
use crate::metrics::DEFAULT_SHUTDOWN_TIMEOUT;
use crate::metrics::SHUTDOWN_POLL_INTERVAL;
use crate::metrics::config::MetricsConfig;
use crate::metrics::config::MetricsExporter;
use crate::metrics::error::MetricsError;
use crate::metrics::error::Result;
use crate::metrics::tags::collect_tags;
use crate::metrics::tags::merge_tags;
use crate::metrics::tags::tags_to_attributes;
use crate::metrics::time::duration_to_millis;
use crate::metrics::util::error_or_panic;
use crate::metrics::validation::validate_metric_name;
use crate::metrics::validation::validate_tags;
use chrono::Utc;
use opentelemetry::KeyValue;
use opentelemetry::metrics::Histogram;
use opentelemetry::metrics::Meter;
use opentelemetry::metrics::MeterProvider;
use opentelemetry::metrics::UpDownCounter;
use opentelemetry_sdk::metrics::PeriodicReader;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::header::USER_AGENT;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::error::TrySendError;

const METER_NAME: &str = "codex-otel-metrics";
const STATSIG_USER_ID: &str = "codex-metrics";
const STATSIG_SDK_TYPE: &str = "codex-otel-rust";
const STATSIG_MAX_BATCH_EVENTS: usize = 50;
const STATSIG_BATCH_WINDOW: Duration = Duration::from_millis(1000);

#[derive(Clone, Debug)]
enum MetricEvent {
    Counter {
        name: String,
        value: i64,
        tags: Vec<(String, String)>,
    },
    Histogram {
        name: String,
        value: i64,
        tags: Vec<(String, String)>,
    },
}

enum WorkerMessage {
    Event(MetricEvent),
}

struct WorkerState {
    sender: Mutex<Option<mpsc::Sender<WorkerMessage>>>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
    capacity: usize,
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

    fn record_event(&mut self, event: MetricEvent) {
        match event {
            MetricEvent::Counter { name, value, tags } => {
                self.record_counter(&name, value, &tags);
            }
            MetricEvent::Histogram { name, value, tags } => {
                self.record_histogram(&name, value, &tags);
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

/// Background metrics client that enqueues metrics to a tokio-backed worker.
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

        let exporter_label = config.exporter_label();
        let worker_exporter_label = exporter_label;
        let exporter = build_worker_exporter(&config)?;
        let runtime = build_runtime()?;

        let (sender, receiver) = mpsc::channel(capacity);
        let handle = spawn_worker(runtime, exporter, worker_exporter_label, receiver);

        Ok(Self {
            state: Arc::new(WorkerState {
                sender: Mutex::new(Some(sender)),
                handle: Mutex::new(Some(handle)),
                capacity,
            }),
        })
    }

    /// Send a single counter increment without blocking the caller.
    pub fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) -> Result<()> {
        validate_metric_name(name)?;
        let tags = collect_tags(tags)?;
        self.send_event(MetricEvent::Counter {
            name: name.to_string(),
            value: inc,
            tags,
        })
    }

    /// Send a single histogram sample.
    pub fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) -> Result<()> {
        validate_metric_name(name)?;
        let tags = collect_tags(tags)?;
        self.send_event(MetricEvent::Histogram {
            name: name.to_string(),
            value,
            tags,
        })
    }

    /// Record a duration in milliseconds using a histogram.
    pub fn record_duration(
        &self,
        name: &str,
        duration: Duration,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        let millis = duration_to_millis(duration);
        self.histogram(name, millis, tags)
    }

    /// Measure a closure and emit a histogram sample for the elapsed time.
    pub fn time<T>(&self, name: &str, tags: &[(&str, &str)], f: impl FnOnce() -> T) -> Result<T> {
        let start = Instant::now();
        let output = f();
        self.record_duration(name, start.elapsed(), tags)?;
        Ok(output)
    }

    /// Measure a closure that returns a metrics result without nesting results.
    pub fn time_result<T>(
        &self,
        name: &str,
        tags: &[(&str, &str)],
        f: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let start = Instant::now();
        let output = f();
        match output {
            Ok(value) => {
                self.record_duration(name, start.elapsed(), tags)?;
                Ok(value)
            }
            Err(err) => {
                let _ = self.record_duration(name, start.elapsed(), tags);
                Err(err)
            }
        }
    }

    fn send_event(&self, event: MetricEvent) -> Result<()> {
        let sender = self
            .state
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(sender) = sender.as_ref() else {
            return Err(MetricsError::WorkerUnavailable);
        };

        match sender.try_send(WorkerMessage::Event(event)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(MetricsError::QueueFull {
                capacity: self.state.capacity,
            }),
            Err(TrySendError::Closed(_)) => Err(MetricsError::WorkerUnavailable),
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
        let Some(handle) = handle.take() else {
            return Ok(());
        };
        let mut joined = false;

        // Dropping the sender closes the channel; the worker drains pending events and exits.
        drop(sender);

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

        if joined {
            return Ok(());
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

fn build_runtime() -> Result<Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|source| MetricsError::RuntimeBuild { source })
}

fn build_worker_exporter(config: &MetricsConfig) -> Result<WorkerExporter> {
    match &config.exporter {
        MetricsExporter::StatsigHttp => Ok(WorkerExporter::Statsig(StatsigExporter::from(config)?)),
        MetricsExporter::InMemory(exporter) => Ok(WorkerExporter::InMemory(
            InMemoryExporter::from(config.default_tags.clone(), exporter.clone()),
        )),
    }
}

fn spawn_worker(
    runtime: Runtime,
    exporter: WorkerExporter,
    exporter_label: String,
    receiver: mpsc::Receiver<WorkerMessage>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let worker = MetricsWorker::new(exporter, exporter_label);
        runtime.block_on(worker.run(receiver));
    })
}

struct MetricsWorker {
    exporter: WorkerExporter,
    exporter_label: String,
}

impl MetricsWorker {
    fn new(exporter: WorkerExporter, exporter_label: String) -> Self {
        Self {
            exporter,
            exporter_label,
        }
    }

    async fn run(mut self, mut receiver: mpsc::Receiver<WorkerMessage>) {
        while let Some(message) = receiver.recv().await {
            match message {
                WorkerMessage::Event(event) => {
                    let events = Self::collect_batch(event, &mut receiver).await;
                    self.export_batch(events).await;
                }
            }
        }
        self.shutdown().await;
    }

    async fn export_batch(&mut self, events: Vec<MetricEvent>) {
        match &mut self.exporter {
            WorkerExporter::Statsig(exporter) => {
                if let Err(err) = exporter.export_events(events).await {
                    error_or_panic(format!(
                        "statsig metrics export failed: {err} (exporter={})",
                        self.exporter_label
                    ));
                }
            }
            WorkerExporter::InMemory(exporter) => {
                exporter.export_events(events, &self.exporter_label).await;
            }
        }
    }

    async fn collect_batch(
        first: MetricEvent,
        receiver: &mut mpsc::Receiver<WorkerMessage>,
    ) -> Vec<MetricEvent> {
        let mut events = Vec::with_capacity(1);
        events.push(first);

        // Fast-path: drain anything already enqueued.
        while events.len() < STATSIG_MAX_BATCH_EVENTS {
            match receiver.try_recv() {
                Ok(WorkerMessage::Event(event)) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return events,
            }
        }

        if events.len() >= STATSIG_MAX_BATCH_EVENTS {
            return events;
        }

        // Small coalescing window to catch near-simultaneous metrics without blocking callers.
        let deadline = Instant::now() + STATSIG_BATCH_WINDOW;
        while events.len() < STATSIG_MAX_BATCH_EVENTS {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }

            match tokio::time::timeout(remaining, receiver.recv()).await {
                Ok(Some(WorkerMessage::Event(event))) => events.push(event),
                Ok(None) => break,
                Err(_) => break,
            }
        }

        events
    }

    async fn shutdown(&mut self) {
        if let WorkerExporter::InMemory(exporter) = &mut self.exporter {
            exporter.shutdown(&self.exporter_label).await;
        }
    }
}

enum WorkerExporter {
    Statsig(StatsigExporter),
    InMemory(InMemoryExporter),
}

struct InMemoryExporter {
    recorder: MetricRecorder,
    meter_provider: SdkMeterProvider,
}

impl InMemoryExporter {
    fn from(
        default_tags: BTreeMap<String, String>,
        exporter: opentelemetry_sdk::metrics::InMemoryMetricExporter,
    ) -> Self {
        let reader = PeriodicReader::builder(exporter).build();
        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let meter = meter_provider.meter(METER_NAME);
        let recorder = MetricRecorder::new(meter, default_tags);
        Self {
            recorder,
            meter_provider,
        }
    }

    async fn export_events(&mut self, events: Vec<MetricEvent>, exporter_label: &str) {
        for event in events {
            self.recorder.record_event(event);
        }
        if let Err(err) = self.meter_provider.force_flush() {
            error_or_panic(format!(
                "metrics flush failed: {err} (exporter={exporter_label})"
            ));
        }
    }

    async fn shutdown(&mut self, exporter_label: &str) {
        if let Err(err) = self.meter_provider.force_flush() {
            error_or_panic(format!(
                "metrics flush failed during shutdown: {err} (exporter={exporter_label})"
            ));
        }
        if let Err(err) = self.meter_provider.shutdown() {
            error_or_panic(format!(
                "metrics shutdown failed: {err} (exporter={exporter_label})"
            ));
        }
    }
}

struct StatsigExporter {
    client: reqwest::Client,
    endpoint: String,
    api_key_header: HeaderName,
    api_key: HeaderValue,
    user_agent: Option<HeaderValue>,
    default_tags: BTreeMap<String, String>,
}

impl StatsigExporter {
    fn from(config: &MetricsConfig) -> Result<Self> {
        let api_key_header =
            HeaderName::from_bytes(config.api_key_header.as_bytes()).map_err(|source| {
                MetricsError::InvalidApiKeyHeader {
                    header: config.api_key_header.clone(),
                    source,
                }
            })?;
        let api_key = HeaderValue::from_str(&config.api_key).map_err(|source| {
            MetricsError::InvalidHeaderValue {
                header: config.api_key_header.clone(),
                source,
            }
        })?;
        let user_agent = if config.user_agent.is_empty() {
            None
        } else {
            Some(HeaderValue::from_str(&config.user_agent).map_err(|source| {
                MetricsError::InvalidHeaderValue {
                    header: "User-Agent".to_string(),
                    source,
                }
            })?)
        };
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|source| MetricsError::HttpClientBuild { source })?;

        Ok(Self {
            client,
            endpoint: config.endpoint.clone(),
            api_key_header,
            api_key,
            user_agent,
            default_tags: config.default_tags.clone(),
        })
    }

    async fn export_events(&self, events: Vec<MetricEvent>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let payload = self.build_payload(events);

        let mut request = self
            .client
            .post(&self.endpoint)
            .header(self.api_key_header.clone(), self.api_key.clone());

        if let Some(user_agent) = &self.user_agent {
            request = request.header(USER_AGENT, user_agent.clone());
        }

        let response = request
            .json(&payload)
            .send()
            .await
            .map_err(|source| MetricsError::StatsigRequestFailed { source })?;

        if let Err(status_err) = response.error_for_status_ref() {
            let status = status_err
                .status()
                .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR);
            let body = response.text().await.unwrap_or_default();
            return Err(MetricsError::StatsigResponseError { status, body });
        }

        Ok(())
    }

    fn build_payload(&self, events: Vec<MetricEvent>) -> StatsigPayload {
        let timestamp = Utc::now().timestamp_millis();
        let events = events
            .into_iter()
            .map(|event| self.event_from_metric(event, timestamp))
            .collect();

        StatsigPayload {
            events,
            statsig_metadata: StatsigMetadata {
                sdk_type: STATSIG_SDK_TYPE.to_string(),
                sdk_version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }

    fn event_from_metric(&self, event: MetricEvent, timestamp: i64) -> StatsigEvent {
        match event {
            MetricEvent::Counter { name, value, tags } => StatsigEvent {
                event_name: name,
                value: value as f64,
                metadata: StatsigEventMetadata {
                    metric_type: "counter".to_string(),
                    tags: merge_tags(&self.default_tags, &tags),
                },
                user: StatsigUser {
                    user_id: STATSIG_USER_ID.to_string(),
                },
                time: timestamp,
            },
            MetricEvent::Histogram { name, value, tags } => StatsigEvent {
                event_name: name,
                value: value as f64,
                metadata: StatsigEventMetadata {
                    metric_type: "histogram".to_string(),
                    tags: merge_tags(&self.default_tags, &tags),
                },
                user: StatsigUser {
                    user_id: STATSIG_USER_ID.to_string(),
                },
                time: timestamp,
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct StatsigPayload {
    events: Vec<StatsigEvent>,
    #[serde(rename = "statsigMetadata")]
    statsig_metadata: StatsigMetadata,
}

#[derive(Debug, Serialize)]
struct StatsigEvent {
    #[serde(rename = "eventName")]
    event_name: String,
    value: f64,
    metadata: StatsigEventMetadata,
    user: StatsigUser,
    time: i64,
}

#[derive(Debug, Serialize)]
struct StatsigEventMetadata {
    #[serde(rename = "metric_type")]
    metric_type: String,
    #[serde(flatten)]
    tags: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct StatsigUser {
    #[serde(rename = "userID")]
    user_id: String,
}

#[derive(Debug, Serialize)]
struct StatsigMetadata {
    #[serde(rename = "sdkType")]
    sdk_type: String,
    #[serde(rename = "sdkVersion")]
    sdk_version: String,
}
