use crate::loom::sync::atomic::AtomicUsize;

use std::fmt;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};

pub(super) struct State {
    val: AtomicUsize,
}

/// Current state value.
#[derive(Copy, Clone)]
pub(super) struct Snapshot(usize);

type UpdateResult = Result<Snapshot, Snapshot>;

/// The task is currently being run.
const RUNNING: usize = 0b0001;

/// The task is complete.
///
/// Once this bit is set, it is never unset.
const COMPLETE: usize = 0b0010;

/// Extracts the task's lifecycle value from the state.
const LIFECYCLE_MASK: usize = 0b11;

/// Flag tracking if the task has been pushed into a run queue.
const NOTIFIED: usize = 0b100;

/// The join handle is still around.
const JOIN_INTEREST: usize = 0b1_000;

/// A join handle waker has been set.
const JOIN_WAKER: usize = 0b10_000;

/// The task has been forcibly cancelled.
const CANCELLED: usize = 0b100_000;

/// All bits.
const STATE_MASK: usize = LIFECYCLE_MASK | NOTIFIED | JOIN_INTEREST | JOIN_WAKER | CANCELLED;

/// Bits used by the ref count portion of the state.
const REF_COUNT_MASK: usize = !STATE_MASK;

/// Number of positions to shift the ref count.
const REF_COUNT_SHIFT: usize = REF_COUNT_MASK.count_zeros() as usize;

/// One ref count.
const REF_ONE: usize = 1 << REF_COUNT_SHIFT;

/// State a task is initialized with.
///
/// A task is initialized with three references:
///
///  * A reference that will be stored in an `OwnedTasks` or `LocalOwnedTasks`.
///  * A reference that will be sent to the scheduler as an ordinary notification.
///  * A reference for the `JoinHandle`.
///
/// As the task starts with a `JoinHandle`, `JOIN_INTEREST` is set.
/// As the task starts with a `Notified`, `NOTIFIED` is set.
const INITIAL_STATE: usize = (REF_ONE * 3) | JOIN_INTEREST | NOTIFIED;

#[must_use]
pub(super) enum TransitionToRunning {
    Success,
    Cancelled,
    Failed,
    Dealloc,
}

#[must_use]
pub(super) enum TransitionToIdle {
    Ok,
    OkNotified,
    OkDealloc,
    Cancelled,
}

#[must_use]
pub(super) enum TransitionToNotifiedByVal {
    DoNothing,
    Submit,
    Dealloc,
}

#[must_use]
pub(crate) enum TransitionToNotifiedByRef {
    DoNothing,
    Submit,
}

#[must_use]
pub(super) struct TransitionToJoinHandleDrop {
    pub(super) drop_waker: bool,
    pub(super) drop_output: bool,
}

/// All transitions are performed via RMW operations. This establishes an
/// unambiguous modification order.
impl State {
    /// Returns a task's initial state.
    pub(super) fn new() -> State {
        // The raw task returned by this method has a ref-count of three. See
        // the comment on INITIAL_STATE for more.
        State {
            val: AtomicUsize::new(INITIAL_STATE),
        }
    }

    /// Loads the current state, establishes `Acquire` ordering.
    pub(super) fn load(&self) -> Snapshot {
        Snapshot(self.val.load(Acquire))
    }

    /// Attempts to transition the lifecycle to `Running`. This sets the
    /// notified bit to false so notifications during the poll can be detected.
    pub(super) fn transition_to_running(&self) -> TransitionToRunning {
        self.fetch_update_action(|mut next| {
            let action;
            assert!(next.is_notified());

            if !next.is_idle() {
                // This happens if the task is either currently running or if it
                // has already completed, e.g. if it was cancelled during
                // shutdown. Consume the ref-count and return.
                next.ref_dec();
                if next.ref_count() == 0 {
                    action = TransitionToRunning::Dealloc;
                } else {
                    action = TransitionToRunning::Failed;
                }
            } else {
                // We are able to lock the RUNNING bit.
                next.set_running();
                next.unset_notified();

                if next.is_cancelled() {
                    action = TransitionToRunning::Cancelled;
                } else {
                    action = TransitionToRunning::Success;
                }
            }
            (action, Some(next))
        })
    }

    /// Transitions the task from `Running` -> `Idle`.
    ///
    /// The transition to `Idle` fails if the task has been flagged to be
    /// cancelled.
    pub(super) fn transition_to_idle(&self) -> TransitionToIdle {
        self.fetch_update_action(|curr| {
            assert!(curr.is_running());

            if curr.is_cancelled() {
                return (TransitionToIdle::Cancelled, None);
            }

            let mut next = curr;
            let action;
            next.unset_running();

            if !next.is_notified() {
                // Polling the future consumes the ref-count of the Notified.
                next.ref_dec();
                if next.ref_count() == 0 {
                    action = TransitionToIdle::OkDealloc;
                } else {
                    action = TransitionToIdle::Ok;
                }
            } else {
                // The caller will schedule a new notification, so we create a
                // new ref-count for the notification. Our own ref-count is kept
                // for now, and the caller will drop it shortly.
                next.ref_inc();
                action = TransitionToIdle::OkNotified;
            }

            (action, Some(next))
        })
    }

    /// Transitions the task from `Running` -> `Complete`.
    pub(super) fn transition_to_complete(&self) -> Snapshot {
        const DELTA: usize = RUNNING | COMPLETE;

        let prev = Snapshot(self.val.fetch_xor(DELTA, AcqRel));
        assert!(prev.is_running());
        assert!(!prev.is_complete());

        Snapshot(prev.0 ^ DELTA)
    }

    /// Transitions from `Complete` -> `Terminal`, decrementing the reference
    /// count the specified number of times.
    ///
    /// Returns true if the task should be deallocated.
    pub(super) fn transition_to_terminal(&self, count: usize) -> bool {
        let prev = Snapshot(self.val.fetch_sub(count * REF_ONE, AcqRel));
        assert!(
            prev.ref_count() >= count,
            "current: {}, sub: {}",
            prev.ref_count(),
            count
        );
        prev.ref_count() == count
    }

    /// Transitions the state to `NOTIFIED`.
    ///
    /// If no task needs to be submitted, a ref-count is consumed.
    ///
    /// If a task needs to be submitted, the ref-count is incremented for the
    /// new Notified.
    pub(super) fn transition_to_notified_by_val(&self) -> TransitionToNotifiedByVal {
        self.fetch_update_action(|mut snapshot| {
            let action;

            if snapshot.is_running() {
                // If the task is running, we mark it as notified, but we should
                // not submit anything as the thread currently running the
                // future is responsible for that.
                snapshot.set_notified();
                snapshot.ref_dec();

                // The thread that set the running bit also holds a ref-count.
                assert!(snapshot.ref_count() > 0);

                action = TransitionToNotifiedByVal::DoNothing;
            } else if snapshot.is_complete() || snapshot.is_notified() {
                // We do not need to submit any notifications, but we have to
                // decrement the ref-count.
                snapshot.ref_dec();

                if snapshot.ref_count() == 0 {
                    action = TransitionToNotifiedByVal::Dealloc;
                } else {
                    action = TransitionToNotifiedByVal::DoNothing;
                }
            } else {
                // We create a new notified that we can submit. The caller
                // retains ownership of the ref-count they passed in.
                snapshot.set_notified();
                snapshot.ref_inc();
                action = TransitionToNotifiedByVal::Submit;
            }

            (action, Some(snapshot))
        })
    }

    /// Transitions the state to `NOTIFIED`.
    pub(super) fn transition_to_notified_by_ref(&self) -> TransitionToNotifiedByRef {
        self.fetch_update_action(|mut snapshot| {
            if snapshot.is_complete() || snapshot.is_notified() {
                // There is nothing to do in this case.
                (TransitionToNotifiedByRef::DoNothing, None)
            } else if snapshot.is_running() {
                // If the task is running, we mark it as notified, but we should
                // not submit as the thread currently running the future is
                // responsible for that.
                snapshot.set_notified();
                (TransitionToNotifiedByRef::DoNothing, Some(snapshot))
            } else {
                // The task is idle and not notified. We should submit a
                // notification.
                snapshot.set_notified();
                snapshot.ref_inc();
                (TransitionToNotifiedByRef::Submit, Some(snapshot))
            }
        })
    }

    /// Transitions the state to `NOTIFIED`, unconditionally increasing the ref
    /// count.
    ///
    /// Returns `true` if the notified bit was transitioned from `0` to `1`;
    /// otherwise `false.`
    #[cfg(all(
        tokio_unstable,
        tokio_taskdump,
        feature = "rt",
        target_os = "linux",
        any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")
    ))]
    pub(super) fn transition_to_notified_for_tracing(&self) -> bool {
        self.fetch_update_action(|mut snapshot| {
            if snapshot.is_notified() {
                (false, None)
            } else {
                snapshot.set_notified();
                snapshot.ref_inc();
                (true, Some(snapshot))
            }
        })
    }

    /// Sets the cancelled bit and transitions the state to `NOTIFIED` if idle.
    ///
    /// Returns `true` if the task needs to be submitted to the pool for
    /// execution.
    pub(super) fn transition_to_notified_and_cancel(&self) -> bool {
        self.fetch_update_action(|mut snapshot| {
            if snapshot.is_cancelled() || snapshot.is_complete() {
                // Aborts to completed or cancelled tasks are no-ops.
                (false, None)
            } else if snapshot.is_running() {
                // If the task is running, we mark it as cancelled. The thread
                // running the task will notice the cancelled bit when it
                // stops polling and it will kill the task.
                //
                // The set_notified() call is not strictly necessary but it will
                // in some cases let a wake_by_ref call return without having
                // to perform a compare_exchange.
                snapshot.set_notified();
                snapshot.set_cancelled();
                (false, Some(snapshot))
            } else {
                // The task is idle. We set the cancelled and notified bits and
                // submit a notification if the notified bit was not already
                // set.
                snapshot.set_cancelled();
                if !snapshot.is_notified() {
                    snapshot.set_notified();
                    snapshot.ref_inc();
                    (true, Some(snapshot))
                } else {
                    (false, Some(snapshot))
                }
            }
        })
    }

    /// Sets the `CANCELLED` bit and attempts to transition to `Running`.
    ///
    /// Returns `true` if the transition to `Running` succeeded.
    pub(super) fn transition_to_shutdown(&self) -> bool {
        let mut prev = Snapshot(0);

        let _ = self.fetch_update(|mut snapshot| {
            prev = snapshot;

            if snapshot.is_idle() {
                snapshot.set_running();
            }

            // If the task was not idle, the thread currently running the task
            // will notice the cancelled bit and cancel it once the poll
            // completes.
            snapshot.set_cancelled();
            Some(snapshot)
        });

        prev.is_idle()
    }

    /// Optimistically tries to swap the state assuming the join handle is
    /// __immediately__ dropped on spawn.
    pub(super) fn drop_join_handle_fast(&self) -> Result<(), ()> {
        use std::sync::atomic::Ordering::Relaxed;

        // Relaxed is acceptable as if this function is called and succeeds,
        // then nothing has been done w/ the join handle.
        //
        // The moment the join handle is used (polled), the `JOIN_WAKER` flag is
        // set, at which point the CAS will fail.
        //
        // Given this, there is no risk if this operation is reordered.
        self.val
            .compare_exchange_weak(
                INITIAL_STATE,
                (INITIAL_STATE - REF_ONE) & !JOIN_INTEREST,
                Release,
                Relaxed,
            )
            .map(|_| ())
            .map_err(|_| ())
    }

    /// Unsets the `JOIN_INTEREST` flag. If `COMPLETE` is not set, the `JOIN_WAKER`
    /// flag is also unset.
    /// The returned `TransitionToJoinHandleDrop` indicates whether the `JoinHandle` should drop
    /// the output of the future or the join waker after the transition.
    pub(super) fn transition_to_join_handle_dropped(&self) -> TransitionToJoinHandleDrop {
        self.fetch_update_action(|mut snapshot| {
            assert!(snapshot.is_join_interested());

            let mut transition = TransitionToJoinHandleDrop {
                drop_waker: false,
                drop_output: false,
            };

            snapshot.unset_join_interested();

            if !snapshot.is_complete() {
                // If `COMPLETE` is unset we also unset `JOIN_WAKER` to give the
                // `JoinHandle` exclusive access to the waker following rule 6 in task/mod.rs.
                // The `JoinHandle` will drop the waker if it has exclusive access
                // to drop it.
                snapshot.unset_join_waker();
            } else {
                // If `COMPLETE` is set the task is completed so the `JoinHandle` is responsible
                // for dropping the output.
                transition.drop_output = true;
            }

            if !snapshot.is_join_waker_set() {
                // If the `JOIN_WAKER` bit is unset and the `JOIN_HANDLE` has exclusive access to
                // the join waker and should drop it following this transition.
                // This might happen in two situations:
                //  1. The task is not completed and we just unset the `JOIN_WAKer` above in this
                //     function.
                //  2. The task is completed. In that case the `JOIN_WAKER` bit was already unset
                //     by the runtime during completion.
                transition.drop_waker = true;
            }

            (transition, Some(snapshot))
        })
    }

    /// Sets the `JOIN_WAKER` bit.
    ///
    /// Returns `Ok` if the bit is set, `Err` otherwise. This operation fails if
    /// the task has completed.
    pub(super) fn set_join_waker(&self) -> UpdateResult {
        self.fetch_update(|curr| {
            assert!(curr.is_join_interested());
            assert!(!curr.is_join_waker_set());

            if curr.is_complete() {
                return None;
            }

            let mut next = curr;
            next.set_join_waker();

            Some(next)
        })
    }

    /// Unsets the `JOIN_WAKER` bit.
    ///
    /// Returns `Ok` has been unset, `Err` otherwise. This operation fails if
    /// the task has completed.
    pub(super) fn unset_waker(&self) -> UpdateResult {
        self.fetch_update(|curr| {
            assert!(curr.is_join_interested());

            if curr.is_complete() {
                return None;
            }

            // If the task is completed, this bit may have been unset by
            // `unset_waker_after_complete`.
            assert!(curr.is_join_waker_set());

            let mut next = curr;
            next.unset_join_waker();

            Some(next)
        })
    }

    /// Unsets the `JOIN_WAKER` bit unconditionally after task completion.
    ///
    /// This operation requires the task to be completed.
    pub(super) fn unset_waker_after_complete(&self) -> Snapshot {
        let prev = Snapshot(self.val.fetch_and(!JOIN_WAKER, AcqRel));
        assert!(prev.is_complete());
        assert!(prev.is_join_waker_set());
        Snapshot(prev.0 & !JOIN_WAKER)
    }

    pub(super) fn ref_inc(&self) {
        use std::process;
        use std::sync::atomic::Ordering::Relaxed;

        // Using a relaxed ordering is alright here, as knowledge of the
        // original reference prevents other threads from erroneously deleting
        // the object.
        //
        // As explained in the [Boost documentation][1], Increasing the
        // reference counter can always be done with memory_order_relaxed: New
        // references to an object can only be formed from an existing
        // reference, and passing an existing reference from one thread to
        // another must already provide any required synchronization.
        //
        // [1]: (www.boost.org/doc/libs/1_55_0/doc/html/atomic/usage_examples.html)
        let prev = self.val.fetch_add(REF_ONE, Relaxed);

        // If the reference count overflowed, abort.
        if prev > isize::MAX as usize {
            process::abort();
        }
    }

    /// Returns `true` if the task should be released.
    pub(super) fn ref_dec(&self) -> bool {
        let prev = Snapshot(self.val.fetch_sub(REF_ONE, AcqRel));
        assert!(prev.ref_count() >= 1);
        prev.ref_count() == 1
    }

    /// Returns `true` if the task should be released.
    pub(super) fn ref_dec_twice(&self) -> bool {
        let prev = Snapshot(self.val.fetch_sub(2 * REF_ONE, AcqRel));
        assert!(prev.ref_count() >= 2);
        prev.ref_count() == 2
    }

    fn fetch_update_action<F, T>(&self, mut f: F) -> T
    where
        F: FnMut(Snapshot) -> (T, Option<Snapshot>),
    {
        let mut curr = self.load();

        loop {
            let (output, next) = f(curr);
            let next = match next {
                Some(next) => next,
                None => return output,
            };

            let res = self.val.compare_exchange(curr.0, next.0, AcqRel, Acquire);

            match res {
                Ok(_) => return output,
                Err(actual) => curr = Snapshot(actual),
            }
        }
    }

    fn fetch_update<F>(&self, mut f: F) -> Result<Snapshot, Snapshot>
    where
        F: FnMut(Snapshot) -> Option<Snapshot>,
    {
        let mut curr = self.load();

        loop {
            let next = match f(curr) {
                Some(next) => next,
                None => return Err(curr),
            };

            let res = self.val.compare_exchange(curr.0, next.0, AcqRel, Acquire);

            match res {
                Ok(_) => return Ok(next),
                Err(actual) => curr = Snapshot(actual),
            }
        }
    }
}

// ===== impl Snapshot =====

impl Snapshot {
    /// Returns `true` if the task is in an idle state.
    pub(super) fn is_idle(self) -> bool {
        self.0 & (RUNNING | COMPLETE) == 0
    }

    /// Returns `true` if the task has been flagged as notified.
    pub(super) fn is_notified(self) -> bool {
        self.0 & NOTIFIED == NOTIFIED
    }

    fn unset_notified(&mut self) {
        self.0 &= !NOTIFIED;
    }

    fn set_notified(&mut self) {
        self.0 |= NOTIFIED;
    }

    pub(super) fn is_running(self) -> bool {
        self.0 & RUNNING == RUNNING
    }

    fn set_running(&mut self) {
        self.0 |= RUNNING;
    }

    fn unset_running(&mut self) {
        self.0 &= !RUNNING;
    }

    pub(super) fn is_cancelled(self) -> bool {
        self.0 & CANCELLED == CANCELLED
    }

    fn set_cancelled(&mut self) {
        self.0 |= CANCELLED;
    }

    /// Returns `true` if the task's future has completed execution.
    pub(super) fn is_complete(self) -> bool {
        self.0 & COMPLETE == COMPLETE
    }

    pub(super) fn is_join_interested(self) -> bool {
        self.0 & JOIN_INTEREST == JOIN_INTEREST
    }

    fn unset_join_interested(&mut self) {
        self.0 &= !JOIN_INTEREST;
    }

    pub(super) fn is_join_waker_set(self) -> bool {
        self.0 & JOIN_WAKER == JOIN_WAKER
    }

    fn set_join_waker(&mut self) {
        self.0 |= JOIN_WAKER;
    }

    fn unset_join_waker(&mut self) {
        self.0 &= !JOIN_WAKER;
    }

    pub(super) fn ref_count(self) -> usize {
        (self.0 & REF_COUNT_MASK) >> REF_COUNT_SHIFT
    }

    fn ref_inc(&mut self) {
        assert!(self.0 <= isize::MAX as usize);
        self.0 += REF_ONE;
    }

    pub(super) fn ref_dec(&mut self) {
        assert!(self.ref_count() > 0);
        self.0 -= REF_ONE;
    }
}

impl fmt::Debug for State {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snapshot = self.load();
        snapshot.fmt(fmt)
    }
}

impl fmt::Debug for Snapshot {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Snapshot")
            .field("is_running", &self.is_running())
            .field("is_complete", &self.is_complete())
            .field("is_notified", &self.is_notified())
            .field("is_cancelled", &self.is_cancelled())
            .field("is_join_interested", &self.is_join_interested())
            .field("is_join_waker_set", &self.is_join_waker_set())
            .field("ref_count", &self.ref_count())
            .finish()
    }
}
