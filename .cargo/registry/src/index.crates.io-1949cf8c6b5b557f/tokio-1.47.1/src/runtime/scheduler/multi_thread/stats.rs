use crate::runtime::{Config, MetricsBatch, WorkerMetrics};

use std::time::{Duration, Instant};

/// Per-worker statistics. This is used for both tuning the scheduler and
/// reporting runtime-level metrics/stats.
pub(crate) struct Stats {
    /// The metrics batch used to report runtime-level metrics/stats to the
    /// user.
    batch: MetricsBatch,

    /// Instant at which work last resumed (continued after park).
    ///
    /// This duplicates the value stored in `MetricsBatch`. We will unify
    /// `Stats` and `MetricsBatch` when we stabilize metrics.
    processing_scheduled_tasks_started_at: Instant,

    /// Number of tasks polled in the batch of scheduled tasks
    tasks_polled_in_batch: usize,

    /// Exponentially-weighted moving average of time spent polling scheduled a
    /// task.
    ///
    /// Tracked in nanoseconds, stored as a `f64` since that is what we use with
    /// the EWMA calculations
    task_poll_time_ewma: f64,
}

/// How to weigh each individual poll time, value is plucked from thin air.
const TASK_POLL_TIME_EWMA_ALPHA: f64 = 0.1;

/// Ideally, we wouldn't go above this, value is plucked from thin air.
const TARGET_GLOBAL_QUEUE_INTERVAL: f64 = Duration::from_micros(200).as_nanos() as f64;

/// Max value for the global queue interval. This is 2x the previous default
const MAX_TASKS_POLLED_PER_GLOBAL_QUEUE_INTERVAL: u32 = 127;

/// This is the previous default
const TARGET_TASKS_POLLED_PER_GLOBAL_QUEUE_INTERVAL: u32 = 61;

impl Stats {
    pub(crate) fn new(worker_metrics: &WorkerMetrics) -> Stats {
        // Seed the value with what we hope to see.
        let task_poll_time_ewma =
            TARGET_GLOBAL_QUEUE_INTERVAL / TARGET_TASKS_POLLED_PER_GLOBAL_QUEUE_INTERVAL as f64;

        Stats {
            batch: MetricsBatch::new(worker_metrics),
            processing_scheduled_tasks_started_at: Instant::now(),
            tasks_polled_in_batch: 0,
            task_poll_time_ewma,
        }
    }

    pub(crate) fn tuned_global_queue_interval(&self, config: &Config) -> u32 {
        // If an interval is explicitly set, don't tune.
        if let Some(configured) = config.global_queue_interval {
            return configured;
        }

        // As of Rust 1.45, casts from f64 -> u32 are saturating, which is fine here.
        let tasks_per_interval = (TARGET_GLOBAL_QUEUE_INTERVAL / self.task_poll_time_ewma) as u32;

        // If we are using self-tuning, we don't want to return less than 2 as that would result in the
        // global queue always getting checked first.
        tasks_per_interval.clamp(2, MAX_TASKS_POLLED_PER_GLOBAL_QUEUE_INTERVAL)
    }

    pub(crate) fn submit(&mut self, to: &WorkerMetrics) {
        self.batch.submit(to, self.task_poll_time_ewma as u64);
    }

    pub(crate) fn about_to_park(&mut self) {
        self.batch.about_to_park();
    }

    pub(crate) fn unparked(&mut self) {
        self.batch.unparked();
    }

    pub(crate) fn inc_local_schedule_count(&mut self) {
        self.batch.inc_local_schedule_count();
    }

    pub(crate) fn start_processing_scheduled_tasks(&mut self) {
        self.batch.start_processing_scheduled_tasks();

        self.processing_scheduled_tasks_started_at = Instant::now();
        self.tasks_polled_in_batch = 0;
    }

    pub(crate) fn end_processing_scheduled_tasks(&mut self) {
        self.batch.end_processing_scheduled_tasks();

        // Update the EWMA task poll time
        if self.tasks_polled_in_batch > 0 {
            let now = Instant::now();

            // If we "overflow" this conversion, we have bigger problems than
            // slightly off stats.
            let elapsed = (now - self.processing_scheduled_tasks_started_at).as_nanos() as f64;
            let num_polls = self.tasks_polled_in_batch as f64;

            // Calculate the mean poll duration for a single task in the batch
            let mean_poll_duration = elapsed / num_polls;

            // Compute the alpha weighted by the number of tasks polled this batch.
            let weighted_alpha = 1.0 - (1.0 - TASK_POLL_TIME_EWMA_ALPHA).powf(num_polls);

            // Now compute the new weighted average task poll time.
            self.task_poll_time_ewma = weighted_alpha * mean_poll_duration
                + (1.0 - weighted_alpha) * self.task_poll_time_ewma;
        }
    }

    pub(crate) fn start_poll(&mut self) {
        self.batch.start_poll();

        self.tasks_polled_in_batch += 1;
    }

    pub(crate) fn end_poll(&mut self) {
        self.batch.end_poll();
    }

    pub(crate) fn incr_steal_count(&mut self, by: u16) {
        self.batch.incr_steal_count(by);
    }

    pub(crate) fn incr_steal_operations(&mut self) {
        self.batch.incr_steal_operations();
    }

    pub(crate) fn incr_overflow_count(&mut self) {
        self.batch.incr_overflow_count();
    }
}
