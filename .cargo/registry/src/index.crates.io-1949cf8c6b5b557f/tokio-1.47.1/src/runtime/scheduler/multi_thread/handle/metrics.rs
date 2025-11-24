use super::Handle;
use crate::runtime::WorkerMetrics;

cfg_unstable_metrics! {
    use crate::runtime::SchedulerMetrics;
}

impl Handle {
    pub(crate) fn num_workers(&self) -> usize {
        self.shared.worker_metrics.len()
    }

    pub(crate) fn num_alive_tasks(&self) -> usize {
        self.shared.owned.num_alive_tasks()
    }

    pub(crate) fn injection_queue_depth(&self) -> usize {
        self.shared.injection_queue_depth()
    }

    pub(crate) fn worker_metrics(&self, worker: usize) -> &WorkerMetrics {
        &self.shared.worker_metrics[worker]
    }

    cfg_unstable_metrics! {
        cfg_64bit_metrics! {
            pub(crate) fn spawned_tasks_count(&self) -> u64 {
                self.shared.owned.spawned_tasks_count()
            }
        }

        pub(crate) fn num_blocking_threads(&self) -> usize {
            // workers are currently spawned using spawn_blocking
            self.blocking_spawner
                .num_threads()
                .saturating_sub(self.num_workers())
        }

        pub(crate) fn num_idle_blocking_threads(&self) -> usize {
            self.blocking_spawner.num_idle_threads()
        }

        pub(crate) fn scheduler_metrics(&self) -> &SchedulerMetrics {
            &self.shared.scheduler_metrics
        }

        pub(crate) fn worker_local_queue_depth(&self, worker: usize) -> usize {
            self.shared.worker_local_queue_depth(worker)
        }

        pub(crate) fn blocking_queue_depth(&self) -> usize {
            self.blocking_spawner.queue_depth()
        }
    }
}
