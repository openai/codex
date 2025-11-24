use super::{Pop, Synced};

use crate::loom::sync::atomic::AtomicUsize;
use crate::runtime::task;

use std::marker::PhantomData;
use std::sync::atomic::Ordering::{Acquire, Release};

pub(crate) struct Shared<T: 'static> {
    /// Number of pending tasks in the queue. This helps prevent unnecessary
    /// locking in the hot path.
    pub(super) len: AtomicUsize,

    _p: PhantomData<T>,
}

unsafe impl<T> Send for Shared<T> {}
unsafe impl<T> Sync for Shared<T> {}

impl<T: 'static> Shared<T> {
    pub(crate) fn new() -> (Shared<T>, Synced) {
        let inject = Shared {
            len: AtomicUsize::new(0),
            _p: PhantomData,
        };

        let synced = Synced {
            is_closed: false,
            head: None,
            tail: None,
        };

        (inject, synced)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // Kind of annoying to have to include the cfg here
    #[cfg(any(tokio_taskdump, feature = "rt-multi-thread"))]
    pub(crate) fn is_closed(&self, synced: &Synced) -> bool {
        synced.is_closed
    }

    /// Closes the injection queue, returns `true` if the queue is open when the
    /// transition is made.
    pub(crate) fn close(&self, synced: &mut Synced) -> bool {
        if synced.is_closed {
            return false;
        }

        synced.is_closed = true;
        true
    }

    pub(crate) fn len(&self) -> usize {
        self.len.load(Acquire)
    }

    /// Pushes a value into the queue.
    ///
    /// This does nothing if the queue is closed.
    ///
    /// # Safety
    ///
    /// Must be called with the same `Synced` instance returned by `Inject::new`
    pub(crate) unsafe fn push(&self, synced: &mut Synced, task: task::Notified<T>) {
        if synced.is_closed {
            return;
        }

        // safety: only mutated with the lock held
        let len = self.len.unsync_load();
        let task = task.into_raw();

        // The next pointer should already be null
        debug_assert!(unsafe { task.get_queue_next().is_none() });

        if let Some(tail) = synced.tail {
            // safety: Holding the Notified for a task guarantees exclusive
            // access to the `queue_next` field.
            unsafe { tail.set_queue_next(Some(task)) };
        } else {
            synced.head = Some(task);
        }

        synced.tail = Some(task);
        self.len.store(len + 1, Release);
    }

    /// Pop a value from the queue.
    ///
    /// # Safety
    ///
    /// Must be called with the same `Synced` instance returned by `Inject::new`
    pub(crate) unsafe fn pop(&self, synced: &mut Synced) -> Option<task::Notified<T>> {
        self.pop_n(synced, 1).next()
    }

    /// Pop `n` values from the queue
    ///
    /// # Safety
    ///
    /// Must be called with the same `Synced` instance returned by `Inject::new`
    pub(crate) unsafe fn pop_n<'a>(&'a self, synced: &'a mut Synced, n: usize) -> Pop<'a, T> {
        use std::cmp;

        debug_assert!(n > 0);

        // safety: All updates to the len atomic are guarded by the mutex. As
        // such, a non-atomic load followed by a store is safe.
        let len = self.len.unsync_load();
        let n = cmp::min(n, len);

        // Decrement the count.
        self.len.store(len - n, Release);

        Pop::new(n, synced)
    }
}
