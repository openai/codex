//! This file contains mocks of the types in src/runtime/metrics

pub(crate) struct SchedulerMetrics {}

#[derive(Clone, Default)]
pub(crate) struct HistogramBuilder {}

impl SchedulerMetrics {
    pub(crate) fn new() -> Self {
        Self {}
    }

    /// Increment the number of tasks scheduled externally
    pub(crate) fn inc_remote_schedule_count(&self) {}
}
