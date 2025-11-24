use crate::runtime::metrics::WorkerMetrics;

cfg_unstable_metrics! {
    use crate::runtime::metrics::HistogramBatch;
}

use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration, Instant};

pub(crate) struct MetricsBatch {
    /// The total busy duration in nanoseconds.
    busy_duration_total: u64,

    /// Instant at which work last resumed (continued after park).
    processing_scheduled_tasks_started_at: Option<Instant>,

    /// Number of times the worker parked.
    park_count: u64,

    /// Number of times the worker parked and unparked.
    park_unpark_count: u64,

    #[cfg(tokio_unstable)]
    /// Number of times the worker woke w/o doing work.
    noop_count: u64,

    #[cfg(tokio_unstable)]
    /// Number of tasks stolen.
    steal_count: u64,

    #[cfg(tokio_unstable)]
    /// Number of times tasks where stolen.
    steal_operations: u64,

    #[cfg(tokio_unstable)]
    /// Number of tasks that were polled by the worker.
    poll_count: u64,

    #[cfg(tokio_unstable)]
    /// Number of tasks polled when the worker entered park. This is used to
    /// track the noop count.
    poll_count_on_last_park: u64,

    #[cfg(tokio_unstable)]
    /// Number of tasks that were scheduled locally on this worker.
    local_schedule_count: u64,

    #[cfg(tokio_unstable)]
    /// Number of tasks moved to the global queue to make space in the local
    /// queue
    overflow_count: u64,

    #[cfg(tokio_unstable)]
    /// If `Some`, tracks poll times in nanoseconds
    poll_timer: Option<PollTimer>,
}

cfg_unstable_metrics! {
    struct PollTimer {
        /// Histogram of poll counts within each band.
        poll_counts: HistogramBatch,

        /// Instant when the most recent task started polling.
        poll_started_at: Instant,
    }
}

impl MetricsBatch {
    pub(crate) fn new(worker_metrics: &WorkerMetrics) -> MetricsBatch {
        let maybe_now = now();
        Self::new_unstable(worker_metrics, maybe_now)
    }

    cfg_metrics_variant! {
        stable: {
            #[inline(always)]
            fn new_unstable(_worker_metrics: &WorkerMetrics, maybe_now: Option<Instant>) -> MetricsBatch {
                MetricsBatch {
                    busy_duration_total: 0,
                    processing_scheduled_tasks_started_at: maybe_now,
                    park_count: 0,
                    park_unpark_count: 0,
                }
            }
        },
        unstable: {
            #[inline(always)]
            fn new_unstable(worker_metrics: &WorkerMetrics, maybe_now: Option<Instant>) -> MetricsBatch {
                let poll_timer = maybe_now.and_then(|now| {
                    worker_metrics
                        .poll_count_histogram
                        .as_ref()
                        .map(|worker_poll_counts| PollTimer {
                            poll_counts: HistogramBatch::from_histogram(worker_poll_counts),
                            poll_started_at: now,
                        })
                });
                MetricsBatch {
                    park_count: 0,
                    park_unpark_count: 0,
                    noop_count: 0,
                    steal_count: 0,
                    steal_operations: 0,
                    poll_count: 0,
                    poll_count_on_last_park: 0,
                    local_schedule_count: 0,
                    overflow_count: 0,
                    busy_duration_total: 0,
                    processing_scheduled_tasks_started_at: maybe_now,
                    poll_timer,
                }
            }
        }
    }

    pub(crate) fn submit(&mut self, worker: &WorkerMetrics, mean_poll_time: u64) {
        worker
            .busy_duration_total
            .store(self.busy_duration_total, Relaxed);

        self.submit_unstable(worker, mean_poll_time);
    }

    cfg_metrics_variant! {
        stable: {
            #[inline(always)]
            fn submit_unstable(&mut self, worker: &WorkerMetrics, _mean_poll_time: u64) {
                worker.park_count.store(self.park_count, Relaxed);
                worker
                    .park_unpark_count
                    .store(self.park_unpark_count, Relaxed);
            }
        },
        unstable: {
            #[inline(always)]
            fn submit_unstable(&mut self, worker: &WorkerMetrics, mean_poll_time: u64) {
                worker.mean_poll_time.store(mean_poll_time, Relaxed);
                worker.park_count.store(self.park_count, Relaxed);
                worker
                    .park_unpark_count
                    .store(self.park_unpark_count, Relaxed);
                worker.noop_count.store(self.noop_count, Relaxed);
                worker.steal_count.store(self.steal_count, Relaxed);
                worker
                    .steal_operations
                    .store(self.steal_operations, Relaxed);
                worker.poll_count.store(self.poll_count, Relaxed);

                worker
                    .local_schedule_count
                    .store(self.local_schedule_count, Relaxed);
                worker.overflow_count.store(self.overflow_count, Relaxed);

                if let Some(poll_timer) = &self.poll_timer {
                    let dst = worker.poll_count_histogram.as_ref().unwrap();
                    poll_timer.poll_counts.submit(dst);
                }
            }
        }
    }

    cfg_metrics_variant! {
        stable: {
            /// The worker is about to park.
            pub(crate) fn about_to_park(&mut self) {
                self.park_count += 1;
                self.park_unpark_count += 1;
            }
        },
        unstable: {
            /// The worker is about to park.
            pub(crate) fn about_to_park(&mut self) {
                {
                    self.park_count += 1;
                    self.park_unpark_count += 1;

                    if self.poll_count_on_last_park == self.poll_count {
                        self.noop_count += 1;
                    } else {
                        self.poll_count_on_last_park = self.poll_count;
                    }
                }
            }
        }
    }
    /// The worker was unparked.
    pub(crate) fn unparked(&mut self) {
        self.park_unpark_count += 1;
    }

    /// Start processing a batch of tasks
    pub(crate) fn start_processing_scheduled_tasks(&mut self) {
        self.processing_scheduled_tasks_started_at = now();
    }

    /// Stop processing a batch of tasks
    pub(crate) fn end_processing_scheduled_tasks(&mut self) {
        if let Some(processing_scheduled_tasks_started_at) =
            self.processing_scheduled_tasks_started_at
        {
            let busy_duration = processing_scheduled_tasks_started_at.elapsed();
            self.busy_duration_total += duration_as_u64(busy_duration);
        }
    }

    cfg_metrics_variant! {
        stable: {
            /// Start polling an individual task
            pub(crate) fn start_poll(&mut self) {}
        },
        unstable: {
            /// Start polling an individual task
            pub(crate) fn start_poll(&mut self) {
                self.poll_count += 1;
                if let Some(poll_timer) = &mut self.poll_timer {
                    poll_timer.poll_started_at = Instant::now();
                }
            }
        }
    }

    cfg_metrics_variant! {
        stable: {
            /// Stop polling an individual task
            pub(crate) fn end_poll(&mut self) {}
        },
        unstable: {
            /// Stop polling an individual task
            pub(crate) fn end_poll(&mut self) {
                if let Some(poll_timer) = &mut self.poll_timer {
                    let elapsed = duration_as_u64(poll_timer.poll_started_at.elapsed());
                    poll_timer.poll_counts.measure(elapsed, 1);
                }
            }
        }
    }

    cfg_metrics_variant! {
        stable: {
            pub(crate) fn inc_local_schedule_count(&mut self) {}
        },
        unstable: {
            pub(crate) fn inc_local_schedule_count(&mut self) {
                self.local_schedule_count += 1;
            }
        }
    }
}

cfg_rt_multi_thread! {
    impl MetricsBatch {
        cfg_metrics_variant! {
            stable: {
                pub(crate) fn incr_steal_count(&mut self, _by: u16) {}
            },
            unstable: {
                pub(crate) fn incr_steal_count(&mut self, by: u16) {
                    self.steal_count += by as u64;
                }
            }
        }

        cfg_metrics_variant! {
            stable: {
                pub(crate) fn incr_steal_operations(&mut self) {}
            },
            unstable: {
                pub(crate) fn incr_steal_operations(&mut self) {
                    self.steal_operations += 1;
                }
            }
        }

        cfg_metrics_variant! {
            stable: {
                pub(crate) fn incr_overflow_count(&mut self) {}
            },
            unstable: {
                pub(crate) fn incr_overflow_count(&mut self) {
                    self.overflow_count += 1;
                }
            }
        }
    }
}

pub(crate) fn duration_as_u64(dur: Duration) -> u64 {
    u64::try_from(dur.as_nanos()).unwrap_or(u64::MAX)
}

/// Gate unsupported time metrics for `wasm32-unknown-unknown`
/// <https://github.com/tokio-rs/tokio/issues/7319>
fn now() -> Option<Instant> {
    if cfg!(all(
        target_arch = "wasm32",
        target_os = "unknown",
        target_vendor = "unknown"
    )) {
        None
    } else {
        Some(Instant::now())
    }
}
