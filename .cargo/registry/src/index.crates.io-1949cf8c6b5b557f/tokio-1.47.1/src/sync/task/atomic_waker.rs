#![cfg_attr(any(loom, not(feature = "sync")), allow(dead_code, unreachable_pub))]

use crate::loom::cell::UnsafeCell;
use crate::loom::hint;
use crate::loom::sync::atomic::AtomicUsize;

use std::fmt;
use std::panic::{resume_unwind, AssertUnwindSafe, RefUnwindSafe, UnwindSafe};
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};
use std::task::Waker;

/// A synchronization primitive for task waking.
///
/// `AtomicWaker` will coordinate concurrent wakes with the consumer
/// potentially "waking" the underlying task. This is useful in scenarios
/// where a computation completes in another thread and wants to wake the
/// consumer, but the consumer is in the process of being migrated to a new
/// logical task.
///
/// Consumers should call `register` before checking the result of a computation
/// and producers should call `wake` after producing the computation (this
/// differs from the usual `thread::park` pattern). It is also permitted for
/// `wake` to be called **before** `register`. This results in a no-op.
///
/// A single `AtomicWaker` may be reused for any number of calls to `register` or
/// `wake`.
pub(crate) struct AtomicWaker {
    state: AtomicUsize,
    waker: UnsafeCell<Option<Waker>>,
}

impl RefUnwindSafe for AtomicWaker {}
impl UnwindSafe for AtomicWaker {}

// `AtomicWaker` is a multi-consumer, single-producer transfer cell. The cell
// stores a `Waker` value produced by calls to `register` and many threads can
// race to take the waker by calling `wake`.
//
// If a new `Waker` instance is produced by calling `register` before an existing
// one is consumed, then the existing one is overwritten.
//
// While `AtomicWaker` is single-producer, the implementation ensures memory
// safety. In the event of concurrent calls to `register`, there will be a
// single winner whose waker will get stored in the cell. The losers will not
// have their tasks woken. As such, callers should ensure to add synchronization
// to calls to `register`.
//
// The implementation uses a single `AtomicUsize` value to coordinate access to
// the `Waker` cell. There are two bits that are operated on independently. These
// are represented by `REGISTERING` and `WAKING`.
//
// The `REGISTERING` bit is set when a producer enters the critical section. The
// `WAKING` bit is set when a consumer enters the critical section. Neither
// bit being set is represented by `WAITING`.
//
// A thread obtains an exclusive lock on the waker cell by transitioning the
// state from `WAITING` to `REGISTERING` or `WAKING`, depending on the
// operation the thread wishes to perform. When this transition is made, it is
// guaranteed that no other thread will access the waker cell.
//
// # Registering
//
// On a call to `register`, an attempt to transition the state from WAITING to
// REGISTERING is made. On success, the caller obtains a lock on the waker cell.
//
// If the lock is obtained, then the thread sets the waker cell to the waker
// provided as an argument. Then it attempts to transition the state back from
// `REGISTERING` -> `WAITING`.
//
// If this transition is successful, then the registering process is complete
// and the next call to `wake` will observe the waker.
//
// If the transition fails, then there was a concurrent call to `wake` that
// was unable to access the waker cell (due to the registering thread holding the
// lock). To handle this, the registering thread removes the waker it just set
// from the cell and calls `wake` on it. This call to wake represents the
// attempt to wake by the other thread (that set the `WAKING` bit). The
// state is then transitioned from `REGISTERING | WAKING` back to `WAITING`.
// This transition must succeed because, at this point, the state cannot be
// transitioned by another thread.
//
// # Waking
//
// On a call to `wake`, an attempt to transition the state from `WAITING` to
// `WAKING` is made. On success, the caller obtains a lock on the waker cell.
//
// If the lock is obtained, then the thread takes ownership of the current value
// in the waker cell, and calls `wake` on it. The state is then transitioned
// back to `WAITING`. This transition must succeed as, at this point, the state
// cannot be transitioned by another thread.
//
// If the thread is unable to obtain the lock, the `WAKING` bit is still set.
// This is because it has either been set by the current thread but the previous
// value included the `REGISTERING` bit **or** a concurrent thread is in the
// `WAKING` critical section. Either way, no action must be taken.
//
// If the current thread is the only concurrent call to `wake` and another
// thread is in the `register` critical section, when the other thread **exits**
// the `register` critical section, it will observe the `WAKING` bit and
// handle the waker itself.
//
// If another thread is in the `waker` critical section, then it will handle
// waking the caller task.
//
// # A potential race (is safely handled).
//
// Imagine the following situation:
//
// * Thread A obtains the `wake` lock and wakes a task.
//
// * Before thread A releases the `wake` lock, the woken task is scheduled.
//
// * Thread B attempts to wake the task. In theory this should result in the
//   task being woken, but it cannot because thread A still holds the wake
//   lock.
//
// This case is handled by requiring users of `AtomicWaker` to call `register`
// **before** attempting to observe the application state change that resulted
// in the task being woken. The wakers also change the application state
// before calling wake.
//
// Because of this, the task will do one of two things.
//
// 1) Observe the application state change that Thread B is waking on. In
//    this case, it is OK for Thread B's wake to be lost.
//
// 2) Call register before attempting to observe the application state. Since
//    Thread A still holds the `wake` lock, the call to `register` will result
//    in the task waking itself and get scheduled again.

/// Idle state.
const WAITING: usize = 0;

/// A new waker value is being registered with the `AtomicWaker` cell.
const REGISTERING: usize = 0b01;

/// The task currently registered with the `AtomicWaker` cell is being woken.
const WAKING: usize = 0b10;

impl AtomicWaker {
    /// Create an `AtomicWaker`
    pub(crate) fn new() -> AtomicWaker {
        AtomicWaker {
            state: AtomicUsize::new(WAITING),
            waker: UnsafeCell::new(None),
        }
    }

    /*
    /// Registers the current waker to be notified on calls to `wake`.
    pub(crate) fn register(&self, waker: Waker) {
        self.do_register(waker);
    }
    */

    /// Registers the provided waker to be notified on calls to `wake`.
    ///
    /// The new waker will take place of any previous wakers that were registered
    /// by previous calls to `register`. Any calls to `wake` that happen after
    /// a call to `register` (as defined by the memory ordering rules), will
    /// wake the `register` caller's task.
    ///
    /// It is safe to call `register` with multiple other threads concurrently
    /// calling `wake`. This will result in the `register` caller's current
    /// task being woken once.
    ///
    /// This function is safe to call concurrently, but this is generally a bad
    /// idea. Concurrent calls to `register` will attempt to register different
    /// tasks to be woken. One of the callers will win and have its task set,
    /// but there is no guarantee as to which caller will succeed.
    pub(crate) fn register_by_ref(&self, waker: &Waker) {
        self.do_register(waker);
    }

    fn do_register<W>(&self, waker: W)
    where
        W: WakerRef,
    {
        fn catch_unwind<F: FnOnce() -> R, R>(f: F) -> std::thread::Result<R> {
            std::panic::catch_unwind(AssertUnwindSafe(f))
        }

        match self
            .state
            .compare_exchange(WAITING, REGISTERING, Acquire, Acquire)
            .unwrap_or_else(|x| x)
        {
            WAITING => {
                unsafe {
                    // If `into_waker` panics (because it's code outside of
                    // AtomicWaker) we need to prime a guard that is called on
                    // unwind to restore the waker to a WAITING state. Otherwise
                    // any future calls to register will incorrectly be stuck
                    // believing it's being updated by someone else.
                    let new_waker_or_panic = catch_unwind(move || waker.into_waker());

                    // Set the field to contain the new waker, or if
                    // `into_waker` panicked, leave the old value.
                    let mut maybe_panic = None;
                    let mut old_waker = None;
                    match new_waker_or_panic {
                        Ok(new_waker) => {
                            old_waker = self.waker.with_mut(|t| (*t).take());
                            self.waker.with_mut(|t| *t = Some(new_waker));
                        }
                        Err(panic) => maybe_panic = Some(panic),
                    }

                    // Release the lock. If the state transitioned to include
                    // the `WAKING` bit, this means that a wake has been
                    // called concurrently, so we have to remove the waker and
                    // wake it.`
                    //
                    // Start by assuming that the state is `REGISTERING` as this
                    // is what we jut set it to.
                    let res = self
                        .state
                        .compare_exchange(REGISTERING, WAITING, AcqRel, Acquire);

                    match res {
                        Ok(_) => {
                            // We don't want to give the caller the panic if it
                            // was someone else who put in that waker.
                            let _ = catch_unwind(move || {
                                drop(old_waker);
                            });
                        }
                        Err(actual) => {
                            // This branch can only be reached if a
                            // concurrent thread called `wake`. In this
                            // case, `actual` **must** be `REGISTERING |
                            // WAKING`.
                            debug_assert_eq!(actual, REGISTERING | WAKING);

                            // Take the waker to wake once the atomic operation has
                            // completed.
                            let mut waker = self.waker.with_mut(|t| (*t).take());

                            // Just swap, because no one could change state
                            // while state == `Registering | `Waking`
                            self.state.swap(WAITING, AcqRel);

                            // If `into_waker` panicked, then the waker in the
                            // waker slot is actually the old waker.
                            if maybe_panic.is_some() {
                                old_waker = waker.take();
                            }

                            // We don't want to give the caller the panic if it
                            // was someone else who put in that waker.
                            if let Some(old_waker) = old_waker {
                                let _ = catch_unwind(move || {
                                    old_waker.wake();
                                });
                            }

                            // The atomic swap was complete, now wake the waker
                            // and return.
                            //
                            // If this panics, we end up in a consumed state and
                            // return the panic to the caller.
                            if let Some(waker) = waker {
                                debug_assert!(maybe_panic.is_none());
                                waker.wake();
                            }
                        }
                    }

                    if let Some(panic) = maybe_panic {
                        // If `into_waker` panicked, return the panic to the caller.
                        resume_unwind(panic);
                    }
                }
            }
            WAKING => {
                // Currently in the process of waking the task, i.e.,
                // `wake` is currently being called on the old waker.
                // So, we call wake on the new waker.
                //
                // If this panics, someone else is responsible for restoring the
                // state of the waker.
                waker.wake();

                // This is equivalent to a spin lock, so use a spin hint.
                hint::spin_loop();
            }
            state => {
                // In this case, a concurrent thread is holding the
                // "registering" lock. This probably indicates a bug in the
                // caller's code as racing to call `register` doesn't make much
                // sense.
                //
                // We just want to maintain memory safety. It is ok to drop the
                // call to `register`.
                debug_assert!(state == REGISTERING || state == REGISTERING | WAKING);
            }
        }
    }

    /// Wakes the task that last called `register`.
    ///
    /// If `register` has not been called yet, then this does nothing.
    pub(crate) fn wake(&self) {
        if let Some(waker) = self.take_waker() {
            // If wake panics, we've consumed the waker which is a legitimate
            // outcome.
            waker.wake();
        }
    }

    /// Attempts to take the `Waker` value out of the `AtomicWaker` with the
    /// intention that the caller will wake the task later.
    pub(crate) fn take_waker(&self) -> Option<Waker> {
        // AcqRel ordering is used in order to acquire the value of the `waker`
        // cell as well as to establish a `release` ordering with whatever
        // memory the `AtomicWaker` is associated with.
        match self.state.fetch_or(WAKING, AcqRel) {
            WAITING => {
                // SAFETY: the waking lock has been acquired.
                let waker = unsafe { self.waker.with_mut(|t| (*t).take()) };

                // Release the lock.
                let old_state = self.state.swap(WAITING, Release);
                debug_assert!(old_state == WAKING);

                waker
            }
            state => {
                // There is a concurrent thread currently updating the
                // associated waker.
                //
                // Nothing more to do as the `WAKING` bit has been set. It
                // doesn't matter if there are concurrent registering threads or
                // not.
                //
                debug_assert!(
                    state == REGISTERING || state == REGISTERING | WAKING || state == WAKING
                );
                None
            }
        }
    }
}

impl Default for AtomicWaker {
    fn default() -> Self {
        AtomicWaker::new()
    }
}

impl fmt::Debug for AtomicWaker {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "AtomicWaker")
    }
}

unsafe impl Send for AtomicWaker {}
unsafe impl Sync for AtomicWaker {}

trait WakerRef {
    fn wake(self);
    fn into_waker(self) -> Waker;
}

impl WakerRef for Waker {
    fn wake(self) {
        self.wake();
    }

    fn into_waker(self) -> Waker {
        self
    }
}

impl WakerRef for &Waker {
    fn wake(self) {
        self.wake_by_ref();
    }

    fn into_waker(self) -> Waker {
        self.clone()
    }
}
