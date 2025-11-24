use crate::runtime::Config;
use crate::util::metric_atomics::{MetricAtomicU64, MetricAtomicUsize};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Mutex;
use std::thread::ThreadId;

cfg_unstable_metrics! {
    use crate::runtime::metrics::Histogram;
}

/// Retrieve runtime worker metrics.
///
/// **Note**: This is an [unstable API][unstable]. The public API of this type
/// may break in 1.x releases. See [the documentation on unstable
/// features][unstable] for details.
///
/// [unstable]: crate#unstable-features
#[derive(Debug, Default)]
#[repr(align(128))]
pub(crate) struct WorkerMetrics {
    /// Amount of time the worker spent doing work vs. parking.
    pub(crate) busy_duration_total: MetricAtomicU64,

    /// Number of tasks currently in the local queue. Used only by the
    /// current-thread scheduler.
    pub(crate) queue_depth: MetricAtomicUsize,

    /// Thread id of worker thread.
    thread_id: Mutex<Option<ThreadId>>,

    ///  Number of times the worker parked.
    pub(crate) park_count: MetricAtomicU64,

    ///  Number of times the worker parked and unparked.
    pub(crate) park_unpark_count: MetricAtomicU64,

    #[cfg(tokio_unstable)]
    /// Number of times the worker woke then parked again without doing work.
    pub(crate) noop_count: MetricAtomicU64,

    #[cfg(tokio_unstable)]
    /// Number of tasks the worker stole.
    pub(crate) steal_count: MetricAtomicU64,

    #[cfg(tokio_unstable)]
    /// Number of times the worker stole
    pub(crate) steal_operations: MetricAtomicU64,

    #[cfg(tokio_unstable)]
    /// Number of tasks the worker polled.
    pub(crate) poll_count: MetricAtomicU64,

    #[cfg(tokio_unstable)]
    /// EWMA task poll time, in nanoseconds.
    pub(crate) mean_poll_time: MetricAtomicU64,

    #[cfg(tokio_unstable)]
    /// Number of tasks scheduled for execution on the worker's local queue.
    pub(crate) local_schedule_count: MetricAtomicU64,

    #[cfg(tokio_unstable)]
    /// Number of tasks moved from the local queue to the global queue to free space.
    pub(crate) overflow_count: MetricAtomicU64,

    #[cfg(tokio_unstable)]
    /// If `Some`, tracks the number of polls by duration range.
    pub(super) poll_count_histogram: Option<Histogram>,
}

impl WorkerMetrics {
    pub(crate) fn new() -> WorkerMetrics {
        WorkerMetrics::default()
    }

    pub(crate) fn set_queue_depth(&self, len: usize) {
        self.queue_depth.store(len, Relaxed);
    }

    pub(crate) fn set_thread_id(&self, thread_id: ThreadId) {
        *self.thread_id.lock().unwrap() = Some(thread_id);
    }

    cfg_metrics_variant! {
        stable: {
            pub(crate) fn from_config(_: &Config) -> WorkerMetrics {
                WorkerMetrics::new()
            }
        },
        unstable: {
            pub(crate) fn from_config(config: &Config) -> WorkerMetrics {
                let mut worker_metrics = WorkerMetrics::new();
                worker_metrics.poll_count_histogram = config
                    .metrics_poll_count_histogram
                    .as_ref()
                    .map(|histogram_builder| histogram_builder.build());
                worker_metrics
            }
        }
    }

    cfg_unstable_metrics! {
        pub(crate) fn queue_depth(&self) -> usize {
            self.queue_depth.load(Relaxed)
        }

        pub(crate) fn thread_id(&self) -> Option<ThreadId> {
            *self.thread_id.lock().unwrap()
        }
    }
}
