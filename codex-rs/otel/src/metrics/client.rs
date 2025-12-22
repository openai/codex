use crate::metrics::DEFAULT_QUEUE_CAPACITY;
use crate::metrics::DEFAULT_SHUTDOWN_TIMEOUT;
use crate::metrics::MetricEvent;
use crate::metrics::config::MetricsConfig;
use crate::metrics::config::MetricsExporter;
use crate::metrics::error::MetricsError;
use crate::metrics::error::Result;
use crate::metrics::sink::build_metric_sink;
use crate::metrics::tags::merge_tags;
use crate::metrics::time::duration_to_millis;
use crate::metrics::validation::validate_metric_name;
use crate::metrics::validation::validate_tags;
use crate::metrics::worker::spawn_worker;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

/// Background metrics client that enqueues metrics to a worker thread.
#[derive(Clone)]
pub struct MetricsClient {
    sender: std::sync::Arc<Mutex<Option<mpsc::Sender<MetricEvent>>>>,
    handle: std::sync::Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    capacity: usize,
    default_tags: BTreeMap<String, String>,
}

impl std::fmt::Debug for MetricsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsClient")
            .field("capacity", &self.capacity)
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

        validate_tags(&config.default_tags)?;

        if let MetricsExporter::StatsigHttp { endpoint, .. } = &config.exporter {
            if endpoint.is_empty() {
                return Err(MetricsError::EmptyEndpoint);
            }
            if config.api_key.is_empty() {
                return Err(MetricsError::EmptyApiKey);
            }
        }

        let exporter_label = config.exporter_label();
        let exporter = build_metric_sink(&config)?;
        let default_tags = config.default_tags.clone();
        let runtime = build_runtime()?;

        let (sender, receiver) = mpsc::channel(capacity);
        let handle = spawn_worker(runtime, exporter, exporter_label, receiver);

        Ok(Self {
            sender: std::sync::Arc::new(Mutex::new(Some(sender))),
            handle: std::sync::Arc::new(Mutex::new(Some(handle))),
            capacity,
            default_tags,
        })
    }

    /// Send a single counter increment without blocking the caller.
    pub fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) -> Result<()> {
        validate_metric_name(name)?;
        let tags = merge_tags(&self.default_tags, tags)?;
        self.send_event(MetricEvent::Counter {
            name: name.to_string(),
            value: inc,
            tags,
        })
    }

    /// Send a single histogram sample.
    pub fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) -> Result<()> {
        validate_metric_name(name)?;
        let tags = merge_tags(&self.default_tags, tags)?;
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
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(sender) = sender.as_ref() else {
            return Err(MetricsError::WorkerUnavailable);
        };

        match sender.try_send(event) {
            Ok(()) => Ok(()),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => Err(MetricsError::QueueFull {
                capacity: self.capacity,
            }),
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                Err(MetricsError::WorkerUnavailable)
            }
        }
    }

    /// Flush queued metrics and stop the worker thread.
    pub fn shutdown(&self) -> Result<()> {
        self.shutdown_inner(DEFAULT_SHUTDOWN_TIMEOUT)
    }

    fn shutdown_inner(&self, timeout: Duration) -> Result<()> {
        let sender = self
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let mut handle = self
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
                thread::sleep(crate::metrics::SHUTDOWN_POLL_INTERVAL);
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
        if std::sync::Arc::strong_count(&self.sender) == 1 {
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
