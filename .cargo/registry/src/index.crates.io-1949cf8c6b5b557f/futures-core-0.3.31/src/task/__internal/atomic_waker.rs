use core::cell::UnsafeCell;
use core::fmt;
use core::task::Waker;

use atomic::AtomicUsize;
use atomic::Ordering::{AcqRel, Acquire, Release};

#[cfg(feature = "portable-atomic")]
use portable_atomic as atomic;

#[cfg(not(feature = "portable-atomic"))]
use core::sync::atomic;

/// A synchronization primitive for task wakeup.
///
/// Sometimes the task interested in a given event will change over time.
/// An `AtomicWaker` can coordinate concurrent notifications with the consumer
/// potentially "updating" the underlying task to wake up. This is useful in
/// scenarios where a computation completes in another thread and wants to
/// notify the consumer, but the consumer is in the process of being migrated to
/// a new logical task.
///
/// Consumers should call `register` before checking the result of a computation
/// and producers should call `wake` after producing the computation (this
/// differs from the usual `thread::park` pattern). It is also permitted for
/// `wake` to be called **before** `register`. This results in a no-op.
///
/// A single `AtomicWaker` may be reused for any number of calls to `register` or
/// `wake`.
///
/// # Memory ordering
///
/// Calling `register` "acquires" all memory "released" by calls to `wake`
/// before the call to `register`.  Later calls to `wake` will wake the
/// registered waker (on contention this wake might be triggered in `register`).
///
/// For concurrent calls to `register` (should be avoided) the ordering is only
/// guaranteed for the winning call.
///
/// # Examples
///
/// Here is a simple example providing a `Flag` that can be signalled manually
/// when it is ready.
///
/// ```
/// use futures::future::Future;
/// use futures::task::{Context, Poll, AtomicWaker};
/// use std::sync::Arc;
/// use std::sync::atomic::AtomicBool;
/// use std::sync::atomic::Ordering::Relaxed;
/// use std::pin::Pin;
///
/// struct Inner {
///     waker: AtomicWaker,
///     set: AtomicBool,
/// }
///
/// #[derive(Clone)]
/// struct Flag(Arc<Inner>);
///
/// impl Flag {
///     pub fn new() -> Self {
///         Self(Arc::new(Inner {
///             waker: AtomicWaker::new(),
///             set: AtomicBool::new(false),
///         }))
///     }
///
///     pub fn signal(&self) {
///         self.0.set.store(true, Relaxed);
///         self.0.waker.wake();
///     }
/// }
///
/// impl Future for Flag {
///     type Output = ();
///
///     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
///         // quick check to avoid registration if already done.
///         if self.0.set.load(Relaxed) {
///             return Poll::Ready(());
///         }
///
///         self.0.waker.register(cx.waker());
///
///         // Need to check condition **after** `register` to avoid a race
///         // condition that would result in lost notifications.
///         if self.0.set.load(Relaxed) {
///             Poll::Ready(())
///         } else {
///             Poll::Pending
///         }
///     }
/// }
/// ```
pub struct AtomicWaker {
    state: AtomicUsize,
    waker: UnsafeCell<Option<Waker>>,
}

// `AtomicWaker` is a multi-consumer, single-producer transfer cell. The cell
// stores a `Waker` value produced by calls to `register` and many threads can
// race to take the waker (to wake it) by calling `wake`.
//
// If a new `Waker` instance is produced by calling `register` before an
// existing one is consumed, then the existing one is overwritten.
//
// While `AtomicWaker` is single-producer, the implementation ensures memory
// safety. In the event of concurrent calls to `register`, there will be a
// single winner whose waker will get stored in the cell. The losers will not
// have their tasks woken. As such, callers should ensure to add synchronization
// to calls to `register`.
//
// The implementation uses a single `AtomicUsize` value to coordinate access to
// the `Waker` cell. There are two bits that are operated on independently.
// These are represented by `REGISTERING` and `WAKING`.
//
// The `REGISTERING` bit is set when a producer enters the critical section. The
// `WAKING` bit is set when a consumer enters the critical section. Neither bit
// being set is represented by `WAITING`.
//
// A thread obtains an exclusive lock on the waker cell by transitioning the
// state from `WAITING` to `REGISTERING` or `WAKING`, depending on the operation
// the thread wishes to perform. When this transition is made, it is guaranteed
// that no other thread will access the waker cell.
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
// If the transition fails, then there was a concurrent call to `wake` that was
// unable to access the waker cell (due to the registering thread holding the
// lock). To handle this, the registering thread removes the waker it just set
// from the cell and calls `wake` on it. This call to wake represents the
// attempt to wake by the other thread (that set the `WAKING` bit). The state is
// then transitioned from `REGISTERING | WAKING` back to `WAITING`.  This
// transition must succeed because, at this point, the state cannot be
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
// If the thread is unable to obtain the lock, the `WAKING` bit is still.  This
// is because it has either been set by the current thread but the previous
// value included the `REGISTERING` bit **or** a concurrent thread is in the
// `WAKING` critical section. Either way, no action must be taken.
//
// If the current thread is the only concurrent call to `wake` and another
// thread is in the `register` critical section, when the other thread **exits**
// the `register` critical section, it will observe the `WAKING` bit and handle
// the wake itself.
//
// If another thread is in the `wake` critical section, then it will handle
// waking the task.
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
//   task being woken, but it cannot because thread A still holds the wake lock.
//
// This case is handled by requiring users of `AtomicWaker` to call `register`
// **before** attempting to observe the application state change that resulted
// in the task being awoken. The wakers also change the application state before
// calling wake.
//
// Because of this, the waker will do one of two things.
//
// 1) Observe the application state change that Thread B is woken for. In this
//    case, it is OK for Thread B's wake to be lost.
//
// 2) Call register before attempting to observe the application state. Since
//    Thread A still holds the `wake` lock, the call to `register` will result
//    in the task waking itself and get scheduled again.

/// Idle state
const WAITING: usize = 0;

/// A new waker value is being registered with the `AtomicWaker` cell.
const REGISTERING: usize = 0b01;

/// The waker currently registered with the `AtomicWaker` cell is being woken.
const WAKING: usize = 0b10;

impl AtomicWaker {
    /// Create an `AtomicWaker`.
    pub const fn new() -> Self {
        // Make sure that task is Sync
        #[allow(dead_code)]
        trait AssertSync: Sync {}
        impl AssertSync for Waker {}

        Self { state: AtomicUsize::new(WAITING), waker: UnsafeCell::new(None) }
    }

    /// Registers the waker to be notified on calls to `wake`.
    ///
    /// The new task will take place of any previous tasks that were registered
    /// by previous calls to `register`. Any calls to `wake` that happen after
    /// a call to `register` (as defined by the memory ordering rules), will
    /// notify the `register` caller's task and deregister the waker from future
    /// notifications. Because of this, callers should ensure `register` gets
    /// invoked with a new `Waker` **each** time they require a wakeup.
    ///
    /// It is safe to call `register` with multiple other threads concurrently
    /// calling `wake`. This will result in the `register` caller's current
    /// task being notified once.
    ///
    /// This function is safe to call concurrently, but this is generally a bad
    /// idea. Concurrent calls to `register` will attempt to register different
    /// tasks to be notified. One of the callers will win and have its task set,
    /// but there is no guarantee as to which caller will succeed.
    ///
    /// # Examples
    ///
    /// Here is how `register` is used when implementing a flag.
    ///
    /// ```
    /// use futures::future::Future;
    /// use futures::task::{Context, Poll, AtomicWaker};
    /// use std::sync::atomic::AtomicBool;
    /// use std::sync::atomic::Ordering::Relaxed;
    /// use std::pin::Pin;
    ///
    /// struct Flag {
    ///     waker: AtomicWaker,
    ///     set: AtomicBool,
    /// }
    ///
    /// impl Future for Flag {
    ///     type Output = ();
    ///
    ///     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    ///         // Register **before** checking `set` to avoid a race condition
    ///         // that would result in lost notifications.
    ///         self.waker.register(cx.waker());
    ///
    ///         if self.set.load(Relaxed) {
    ///             Poll::Ready(())
    ///         } else {
    ///             Poll::Pending
    ///         }
    ///     }
    /// }
    /// ```
    pub fn register(&self, waker: &Waker) {
        match self
            .state
            .compare_exchange(WAITING, REGISTERING, Acquire, Acquire)
            .unwrap_or_else(|x| x)
        {
            WAITING => {
                unsafe {
                    // Locked acquired, update the waker cell

                    // Avoid cloning the waker if the old waker will awaken the same task.
                    match &*self.waker.get() {
                        Some(old_waker) if old_waker.will_wake(waker) => (),
                        _ => *self.waker.get() = Some(waker.clone()),
                    }

                    // Release the lock. If the state transitioned to include
                    // the `WAKING` bit, this means that at least one wake has
                    // been called concurrently.
                    //
                    // Start by assuming that the state is `REGISTERING` as this
                    // is what we just set it to. If this holds, we know that no
                    // other writes were performed in the meantime, so there is
                    // nothing to acquire, only release. In case of concurrent
                    // wakers, we need to acquire their releases, so success needs
                    // to do both.
                    let res = self.state.compare_exchange(REGISTERING, WAITING, AcqRel, Acquire);

                    match res {
                        Ok(_) => {
                            // memory ordering: acquired self.state during CAS
                            // - if previous wakes went through it syncs with
                            //   their final release (`fetch_and`)
                            // - if there was no previous wake the next wake
                            //   will wake us, no sync needed.
                        }
                        Err(actual) => {
                            // This branch can only be reached if at least one
                            // concurrent thread called `wake`. In this
                            // case, `actual` **must** be `REGISTERING |
                            // `WAKING`.
                            debug_assert_eq!(actual, REGISTERING | WAKING);

                            // Take the waker to wake once the atomic operation has
                            // completed.
                            let waker = (*self.waker.get()).take().unwrap();

                            // We need to return to WAITING state (clear our lock and
                            // concurrent WAKING flag). This needs to acquire all
                            // WAKING fetch_or releases and it needs to release our
                            // update to self.waker, so we need a `swap` operation.
                            self.state.swap(WAITING, AcqRel);

                            // memory ordering: we acquired the state for all
                            // concurrent wakes, but future wakes might still
                            // need to wake us in case we can't make progress
                            // from the pending wakes.
                            //
                            // So we simply schedule to come back later (we could
                            // also simply leave the registration in place above).
                            waker.wake();
                        }
                    }
                }
            }
            WAKING => {
                // Currently in the process of waking the task, i.e.,
                // `wake` is currently being called on the old task handle.
                //
                // memory ordering: we acquired the state for all
                // concurrent wakes, but future wakes might still
                // need to wake us in case we can't make progress
                // from the pending wakes.
                //
                // So we simply schedule to come back later (we
                // could also spin here trying to acquire the lock
                // to register).
                waker.wake_by_ref();
            }
            state => {
                // In this case, a concurrent thread is holding the
                // "registering" lock. This probably indicates a bug in the
                // caller's code as racing to call `register` doesn't make much
                // sense.
                //
                // memory ordering: don't care. a concurrent register() is going
                // to succeed and provide proper memory ordering.
                //
                // We just want to maintain memory safety. It is ok to drop the
                // call to `register`.
                debug_assert!(state == REGISTERING || state == REGISTERING | WAKING);
            }
        }
    }

    /// Calls `wake` on the last `Waker` passed to `register`.
    ///
    /// If `register` has not been called yet, then this does nothing.
    pub fn wake(&self) {
        if let Some(waker) = self.take() {
            waker.wake();
        }
    }

    /// Returns the last `Waker` passed to `register`, so that the user can wake it.
    ///
    ///
    /// Sometimes, just waking the AtomicWaker is not fine grained enough. This allows the user
    /// to take the waker and then wake it separately, rather than performing both steps in one
    /// atomic action.
    ///
    /// If a waker has not been registered, this returns `None`.
    pub fn take(&self) -> Option<Waker> {
        // AcqRel ordering is used in order to acquire the value of the `task`
        // cell as well as to establish a `release` ordering with whatever
        // memory the `AtomicWaker` is associated with.
        match self.state.fetch_or(WAKING, AcqRel) {
            WAITING => {
                // The waking lock has been acquired.
                let waker = unsafe { (*self.waker.get()).take() };

                // Release the lock
                self.state.fetch_and(!WAKING, Release);

                waker
            }
            state => {
                // There is a concurrent thread currently updating the
                // associated task.
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
        Self::new()
    }
}

impl fmt::Debug for AtomicWaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AtomicWaker")
    }
}

unsafe impl Send for AtomicWaker {}
unsafe impl Sync for AtomicWaker {}
