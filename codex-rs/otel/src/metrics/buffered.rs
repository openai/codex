//! Buffers bounded operation counts and durations until process metrics are configured.
//! Each pair is admitted together; lifecycle transitions serialize with local SDK recording.
//! Shutting down the active installation resumes buffering until its replacement is ready.

use super::MetricsClient;
use super::MetricsError;
use super::Result;
use super::validation::validate_metric_name;
use super::validation::validate_tag_key;
use super::validation::validate_tag_value;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

const MAX_PENDING_OPERATIONS: usize = 256;
const MAX_METADATA_BYTES: usize = 1024;
const MAX_TAGS: usize = 16;

struct PendingOperation {
    count_name: Box<str>,
    duration_name: Box<str>,
    duration: Duration,
    tags: Vec<(Box<str>, Box<str>)>,
}

enum State {
    Startup(Vec<PendingOperation>),
    Ready(MetricsClient),
    Disabled,
}

pub(crate) struct BufferedMetrics {
    state: Mutex<State>,
}

pub(crate) static GLOBAL: BufferedMetrics = BufferedMetrics::new();

/// Records one operation count and its duration in milliseconds through `MetricsClient`.
/// While awaiting an active provider, retains at most 256 pairs, each with at most 16 tags and
/// 1024 bytes of metric names, tag keys, and tag values. Overflow drops the entire pair.
/// Supply bounded classifications, never credentials or user-provided identifiers.
/// Disabling metrics discards pending pairs and suppresses subsequent recording.
pub fn record_global_operation(
    count_name: &str,
    duration_name: &str,
    duration: Duration,
    tags: &[(&str, &str)],
) -> Result<()> {
    GLOBAL.record(count_name, duration_name, duration, tags)
}

impl BufferedMetrics {
    const fn new() -> Self {
        Self {
            state: Mutex::new(State::Startup(Vec::new())),
        }
    }

    fn record(
        &self,
        count_name: &str,
        duration_name: &str,
        duration: Duration,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        if tags.len() > MAX_TAGS
            || tags.iter().fold(
                count_name.len().saturating_add(duration_name.len()),
                |bytes, (key, value)| bytes.saturating_add(key.len()).saturating_add(value.len()),
            ) > MAX_METADATA_BYTES
        {
            return Err(MetricsError::OperationMetadataTooLarge);
        }
        // Validate both instruments before accepting either half of the pair.
        for name in [count_name, duration_name] {
            validate_metric_name(name)?;
            // The SDK otherwise silently builds a no-op instrument for these names.
            if name.len() > 255 || !name.as_bytes()[0].is_ascii_alphabetic() {
                return Err(MetricsError::InvalidMetricName {
                    name: name.to_owned(),
                });
            }
        }
        for (key, value) in tags {
            validate_tag_key(key)?;
            validate_tag_value(value)?;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &mut *state {
            State::Startup(pending) => {
                if pending.len() < MAX_PENDING_OPERATIONS {
                    pending.push(PendingOperation {
                        count_name: count_name.into(),
                        duration_name: duration_name.into(),
                        duration,
                        tags: tags
                            .iter()
                            .map(|(key, value)| ((*key).into(), (*value).into()))
                            .collect(),
                    });
                }
                Ok(())
            }
            State::Ready(metrics) => record(metrics, count_name, duration_name, duration, tags),
            State::Disabled => Ok(()),
        }
    }

    pub(crate) fn enable(&self, metrics: &MetricsClient) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Bind to this installation so a provider replacement cannot split a pair
        // between exporters through the redirectable global MetricsClient handle.
        let metrics = MetricsClient {
            inner: Arc::clone(&metrics.inner),
            active: None,
        };
        if let State::Startup(pending) = &mut *state {
            for operation in pending.drain(..) {
                let tags: Vec<_> = operation
                    .tags
                    .iter()
                    .map(|(key, value)| (key.as_ref(), value.as_ref()))
                    .collect();
                let _ = record(
                    &metrics,
                    &operation.count_name,
                    &operation.duration_name,
                    operation.duration,
                    &tags,
                );
            }
        }
        *state = State::Ready(metrics);
    }

    pub(crate) fn disable(&self) {
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = State::Disabled;
    }

    pub(super) fn suspend(&self, metrics: &MetricsClient) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // A replaced provider may shut down after its successor is installed.
        // It must neither detach that successor nor undo an explicit opt-out.
        if let State::Ready(current) = &*state
            && Arc::ptr_eq(&current.inner, &metrics.inner)
        {
            *state = State::Startup(Vec::new());
        }
    }
}

fn record(
    metrics: &MetricsClient,
    count_name: &str,
    duration_name: &str,
    duration: Duration,
    tags: &[(&str, &str)],
) -> Result<()> {
    metrics.counter(count_name, /*inc*/ 1, tags)?;
    metrics.record_duration(duration_name, duration, tags)
}

#[cfg(test)]
#[path = "buffered_tests.rs"]
mod tests;
