use super::{Core, Handle, Shared};

use crate::loom::sync::Arc;
use crate::runtime::scheduler::multi_thread::Stats;
use crate::runtime::task::trace::trace_multi_thread;
use crate::runtime::{dump, WorkerMetrics};

use std::time::Duration;

impl Handle {
    pub(super) fn trace_core(&self, mut core: Box<Core>) -> Box<Core> {
        core.is_traced = false;

        if core.is_shutdown {
            return core;
        }

        // wait for other workers, or timeout without tracing
        let timeout = Duration::from_millis(250); // a _very_ generous timeout
        let barrier =
            if let Some(barrier) = self.shared.trace_status.trace_start.wait_timeout(timeout) {
                barrier
            } else {
                // don't attempt to trace
                return core;
            };

        if !barrier.is_leader() {
            // wait for leader to finish tracing
            self.shared.trace_status.trace_end.wait();
            return core;
        }

        // trace

        let owned = &self.shared.owned;
        let mut local = self.shared.steal_all();
        let synced = &self.shared.synced;
        let injection = &self.shared.inject;

        // safety: `trace_multi_thread` is invoked with the same `synced` that `injection`
        // was created with.
        let traces = unsafe { trace_multi_thread(owned, &mut local, synced, injection) }
            .into_iter()
            .map(|(id, trace)| dump::Task::new(id, trace))
            .collect();

        let result = dump::Dump::new(traces);

        // stash the result
        self.shared.trace_status.stash_result(result);

        // allow other workers to proceed
        self.shared.trace_status.trace_end.wait();

        core
    }
}

impl Shared {
    /// Steal all tasks from remotes into a single local queue.
    pub(super) fn steal_all(&self) -> super::queue::Local<Arc<Handle>> {
        let (_steal, mut local) = super::queue::local();

        let worker_metrics = WorkerMetrics::new();
        let mut stats = Stats::new(&worker_metrics);

        for remote in self.remotes.iter() {
            let steal = &remote.steal;
            while !steal.is_empty() {
                if let Some(task) = steal.steal_into(&mut local, &mut stats) {
                    local.push_back([task].into_iter());
                }
            }
        }

        local
    }
}
