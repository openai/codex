use crate::DEFAULT_QUEUE_CAPACITY;
use crate::DEFAULT_SHUTDOWN_TIMEOUT;
use crate::ENVELOPE_CONTENT_TYPE;
use crate::SHUTDOWN_POLL_INTERVAL;
use crate::batch::HistogramBuckets;
use crate::batch::MetricsBatch;
use crate::config::MetricsConfig;
use crate::error::MetricsError;
use crate::error::Result;
use crate::statsd::build_statsd_envelope;
use crate::time::duration_to_millis;
use crate::util::error_or_panic;
use crate::validation::validate_tags;
use sentry::types::Dsn;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::SyncSender;
use std::sync::mpsc::TrySendError;
use std::thread;
use std::time::Duration;
use std::time::Instant;

enum WorkerMessage {
    Batch(MetricsBatch),
    Shutdown,
}

struct WorkerState {
    sender: Mutex<Option<SyncSender<WorkerMessage>>>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
    capacity: usize,
}

#[derive(Debug)]
struct ClientCore {
    dsn: Dsn,
    http: reqwest::blocking::Client,
    auth_header: String,
    default_tags: BTreeMap<String, String>,
}

impl ClientCore {
    fn send(&self, batch: MetricsBatch) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let payload = batch.render(&self.default_tags)?;
        let envelope = build_statsd_envelope(&self.dsn, &payload)?;

        let response = self
            .http
            .post(self.dsn.envelope_api_url())
            .header("X-Sentry-Auth", &self.auth_header)
            .header("Content-Type", ENVELOPE_CONTENT_TYPE)
            .body(envelope)
            .send()
            .map_err(|source| MetricsError::SendEnvelope { source })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .map(|body| {
                    if body.is_empty() {
                        String::new()
                    } else {
                        format!(" body: {body}")
                    }
                })
                .unwrap_or_default();
            return Err(MetricsError::SentryUploadFailed { status, body });
        }

        Ok(())
    }
}

/// Background metrics client that enqueues metrics to a dedicated worker thread.
#[derive(Clone)]
pub struct MetricsClient {
    state: Arc<WorkerState>,
}

impl MetricsClient {
    /// Build a metrics client from configuration and validate defaults.
    pub fn new(config: MetricsConfig) -> Result<Self> {
        let capacity = DEFAULT_QUEUE_CAPACITY;

        if capacity == 0 {
            return Err(MetricsError::QueueCapacityZero);
        }

        let dsn_value = config.dsn.clone();
        let dsn = dsn_value
            .parse::<Dsn>()
            .map_err(|source| MetricsError::InvalidDsn {
                dsn: dsn_value,
                source,
            })?;
        validate_tags(&config.default_tags)?;

        let http = reqwest::blocking::Client::builder()
            .timeout(config.timeout)
            .user_agent(config.user_agent.clone())
            .build()
            .map_err(|source| MetricsError::HttpClientBuild { source })?;

        let auth_header = dsn.to_auth(Some(&config.user_agent)).to_string();

        let core = ClientCore {
            dsn,
            http,
            auth_header,
            default_tags: config.default_tags,
        };

        let (sender, receiver) = mpsc::sync_channel(capacity);
        let handle = thread::spawn(move || run_worker(core, receiver));

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
        let mut batch = MetricsBatch::new();
        batch.counter(name, inc, tags)?;
        self.send(batch)
    }

    /// Send a single histogram sample with the provided buckets.
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

    /// Record a duration in milliseconds using histogram buckets.
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
        let Some(handle) = handle.take() else {
            return Ok(());
        };

        if let Some(sender) = sender {
            let _ = sender.try_send(WorkerMessage::Shutdown);
        }

        if timeout.is_zero() {
            if handle.is_finished() {
                handle.join().map_err(|_| MetricsError::WorkerPanicked)?;
            }
            return Ok(());
        }

        let start = Instant::now();
        while start.elapsed() < timeout {
            if handle.is_finished() {
                handle.join().map_err(|_| MetricsError::WorkerPanicked)?;
                return Ok(());
            }
            thread::sleep(SHUTDOWN_POLL_INTERVAL);
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

fn run_worker(client: ClientCore, receiver: Receiver<WorkerMessage>) {
    while let Ok(message) = receiver.recv() {
        match message {
            WorkerMessage::Batch(batch) => {
                if let Err(err) = client.send(batch) {
                    error_or_panic(format!("metrics send failed: {err}"));
                }
            }
            WorkerMessage::Shutdown => break,
        }
    }
}
