use std::cell::UnsafeCell;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

/// A "lock" around data `D`, which employs a *helping* strategy.
///
/// Used to ensure that concurrent `unpark` invocations lead to (1) `poll` being
/// invoked on only a single thread at a time (2) `poll` being invoked at least
/// once after each `unpark` (unless the future has completed).
pub(crate) struct UnparkMutex<D> {
    // The state of task execution (state machine described below)
    status: AtomicUsize,

    // The actual task data, accessible only in the POLLING state
    inner: UnsafeCell<Option<D>>,
}

// `UnparkMutex<D>` functions in many ways like a `Mutex<D>`, except that on
// acquisition failure, the current lock holder performs the desired work --
// re-polling.
//
// As such, these impls mirror those for `Mutex<D>`. In particular, a reference
// to `UnparkMutex` can be used to gain `&mut` access to the inner data, which
// must therefore be `Send`.
unsafe impl<D: Send> Send for UnparkMutex<D> {}
unsafe impl<D: Send> Sync for UnparkMutex<D> {}

// There are four possible task states, listed below with their possible
// transitions:

// The task is blocked, waiting on an event
const WAITING: usize = 0; // --> POLLING

// The task is actively being polled by a thread; arrival of additional events
// of interest should move it to the REPOLL state
const POLLING: usize = 1; // --> WAITING, REPOLL, or COMPLETE

// The task is actively being polled, but will need to be re-polled upon
// completion to ensure that all events were observed.
const REPOLL: usize = 2; // --> POLLING

// The task has finished executing (either successfully or with an error/panic)
const COMPLETE: usize = 3; // No transitions out

impl<D> UnparkMutex<D> {
    pub(crate) fn new() -> Self {
        Self { status: AtomicUsize::new(WAITING), inner: UnsafeCell::new(None) }
    }

    /// Attempt to "notify" the mutex that a poll should occur.
    ///
    /// An `Ok` result indicates that the `POLLING` state has been entered, and
    /// the caller can proceed to poll the future. An `Err` result indicates
    /// that polling is not necessary (because the task is finished or the
    /// polling has been delegated).
    pub(crate) fn notify(&self) -> Result<D, ()> {
        let mut status = self.status.load(SeqCst);
        loop {
            match status {
                // The task is idle, so try to run it immediately.
                WAITING => {
                    match self.status.compare_exchange(WAITING, POLLING, SeqCst, SeqCst) {
                        Ok(_) => {
                            let data = unsafe {
                                // SAFETY: we've ensured mutual exclusion via
                                // the status protocol; we are the only thread
                                // that has transitioned to the POLLING state,
                                // and we won't transition back to QUEUED until
                                // the lock is "released" by this thread. See
                                // the protocol diagram above.
                                (*self.inner.get()).take().unwrap()
                            };
                            return Ok(data);
                        }
                        Err(cur) => status = cur,
                    }
                }

                // The task is being polled, so we need to record that it should
                // be *repolled* when complete.
                POLLING => match self.status.compare_exchange(POLLING, REPOLL, SeqCst, SeqCst) {
                    Ok(_) => return Err(()),
                    Err(cur) => status = cur,
                },

                // The task is already scheduled for polling, or is complete, so
                // we've got nothing to do.
                _ => return Err(()),
            }
        }
    }

    /// Alert the mutex that polling is about to begin, clearing any accumulated
    /// re-poll requests.
    ///
    /// # Safety
    ///
    /// Callable only from the `POLLING`/`REPOLL` states, i.e. between
    /// successful calls to `notify` and `wait`/`complete`.
    pub(crate) unsafe fn start_poll(&self) {
        self.status.store(POLLING, SeqCst);
    }

    /// Alert the mutex that polling completed with `Pending`.
    ///
    /// # Safety
    ///
    /// Callable only from the `POLLING`/`REPOLL` states, i.e. between
    /// successful calls to `notify` and `wait`/`complete`.
    pub(crate) unsafe fn wait(&self, data: D) -> Result<(), D> {
        unsafe { *self.inner.get() = Some(data) }

        match self.status.compare_exchange(POLLING, WAITING, SeqCst, SeqCst) {
            // no unparks came in while we were running
            Ok(_) => Ok(()),

            // guaranteed to be in REPOLL state; just clobber the
            // state and run again.
            Err(status) => {
                assert_eq!(status, REPOLL);
                self.status.store(POLLING, SeqCst);
                Err(unsafe { (*self.inner.get()).take().unwrap() })
            }
        }
    }

    /// Alert the mutex that the task has completed execution and should not be
    /// notified again.
    ///
    /// # Safety
    ///
    /// Callable only from the `POLLING`/`REPOLL` states, i.e. between
    /// successful calls to `notify` and `wait`/`complete`.
    pub(crate) unsafe fn complete(&self) {
        self.status.store(COMPLETE, SeqCst);
    }
}
