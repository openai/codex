//! Inject queue used to send wakeups to a work-stealing scheduler

use crate::loom::sync::Mutex;
use crate::runtime::task;

mod pop;
pub(crate) use pop::Pop;

mod shared;
pub(crate) use shared::Shared;

mod synced;
pub(crate) use synced::Synced;

cfg_rt_multi_thread! {
    mod rt_multi_thread;
}

mod metrics;

/// Growable, MPMC queue used to inject new tasks into the scheduler and as an
/// overflow queue when the local, fixed-size, array queue overflows.
pub(crate) struct Inject<T: 'static> {
    shared: Shared<T>,
    synced: Mutex<Synced>,
}

impl<T: 'static> Inject<T> {
    pub(crate) fn new() -> Inject<T> {
        let (shared, synced) = Shared::new();

        Inject {
            shared,
            synced: Mutex::new(synced),
        }
    }

    // Kind of annoying to have to include the cfg here
    #[cfg(tokio_taskdump)]
    pub(crate) fn is_closed(&self) -> bool {
        let synced = self.synced.lock();
        self.shared.is_closed(&synced)
    }

    /// Closes the injection queue, returns `true` if the queue is open when the
    /// transition is made.
    pub(crate) fn close(&self) -> bool {
        let mut synced = self.synced.lock();
        self.shared.close(&mut synced)
    }

    /// Pushes a value into the queue.
    ///
    /// This does nothing if the queue is closed.
    pub(crate) fn push(&self, task: task::Notified<T>) {
        let mut synced = self.synced.lock();
        // safety: passing correct `Synced`
        unsafe { self.shared.push(&mut synced, task) }
    }

    pub(crate) fn pop(&self) -> Option<task::Notified<T>> {
        if self.shared.is_empty() {
            return None;
        }

        let mut synced = self.synced.lock();
        // safety: passing correct `Synced`
        unsafe { self.shared.pop(&mut synced) }
    }
}
