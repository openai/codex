use crate::loom::sync::atomic::{AtomicBool, Ordering};
use crate::loom::sync::{Barrier, Mutex};
use crate::runtime::dump::Dump;
use crate::runtime::scheduler::multi_thread::Handle;
use crate::sync::notify::Notify;

/// Tracing status of the worker.
pub(super) struct TraceStatus {
    pub(super) trace_requested: AtomicBool,
    pub(super) trace_start: Barrier,
    pub(super) trace_end: Barrier,
    pub(super) result_ready: Notify,
    pub(super) trace_result: Mutex<Option<Dump>>,
}

impl TraceStatus {
    pub(super) fn new(remotes_len: usize) -> Self {
        Self {
            trace_requested: AtomicBool::new(false),
            trace_start: Barrier::new(remotes_len),
            trace_end: Barrier::new(remotes_len),
            result_ready: Notify::new(),
            trace_result: Mutex::new(None),
        }
    }

    pub(super) fn trace_requested(&self) -> bool {
        self.trace_requested.load(Ordering::Relaxed)
    }

    pub(super) async fn start_trace_request(&self, handle: &Handle) {
        while self
            .trace_requested
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            handle.notify_all();
            crate::task::yield_now().await;
        }
    }

    pub(super) fn stash_result(&self, dump: Dump) {
        let _ = self.trace_result.lock().insert(dump);
        self.result_ready.notify_one();
    }

    pub(super) fn take_result(&self) -> Option<Dump> {
        self.trace_result.lock().take()
    }

    pub(super) async fn end_trace_request(&self, handle: &Handle) {
        while self
            .trace_requested
            .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            handle.notify_all();
            crate::task::yield_now().await;
        }
    }
}
