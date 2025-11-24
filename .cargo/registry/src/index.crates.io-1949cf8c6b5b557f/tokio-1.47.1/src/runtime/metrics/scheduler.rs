use crate::loom::sync::atomic::Ordering::Relaxed;
use crate::util::metric_atomics::MetricAtomicU64;

/// Retrieves metrics from the Tokio runtime.
///
/// **Note**: This is an [unstable API][unstable]. The public API of this type
/// may break in 1.x releases. See [the documentation on unstable
/// features][unstable] for details.
///
/// [unstable]: crate#unstable-features
#[derive(Debug)]
pub(crate) struct SchedulerMetrics {
    /// Number of tasks that are scheduled from outside the runtime.
    pub(super) remote_schedule_count: MetricAtomicU64,
    pub(super) budget_forced_yield_count: MetricAtomicU64,
}

impl SchedulerMetrics {
    pub(crate) fn new() -> SchedulerMetrics {
        SchedulerMetrics {
            remote_schedule_count: MetricAtomicU64::new(0),
            budget_forced_yield_count: MetricAtomicU64::new(0),
        }
    }

    /// Increment the number of tasks scheduled externally
    pub(crate) fn inc_remote_schedule_count(&self) {
        self.remote_schedule_count.add(1, Relaxed);
    }

    /// Increment the number of tasks forced to yield due to budget exhaustion
    pub(crate) fn inc_budget_forced_yield_count(&self) {
        self.budget_forced_yield_count.add(1, Relaxed);
    }
}
