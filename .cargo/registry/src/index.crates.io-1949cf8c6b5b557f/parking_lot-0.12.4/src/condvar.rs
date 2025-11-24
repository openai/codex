// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use crate::mutex::MutexGuard;
use crate::raw_mutex::{RawMutex, TOKEN_HANDOFF, TOKEN_NORMAL};
use crate::{deadlock, util};
use core::{
    fmt, ptr,
    sync::atomic::{AtomicPtr, Ordering},
};
use lock_api::RawMutex as RawMutex_;
use parking_lot_core::{self, ParkResult, RequeueOp, UnparkResult, DEFAULT_PARK_TOKEN};
use std::ops::DerefMut;
use std::time::{Duration, Instant};

/// A type indicating whether a timed wait on a condition variable returned
/// due to a time out or not.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct WaitTimeoutResult(bool);

impl WaitTimeoutResult {
    /// Returns whether the wait was known to have timed out.
    #[inline]
    pub fn timed_out(self) -> bool {
        self.0
    }
}

/// A Condition Variable
///
/// Condition variables represent the ability to block a thread such that it
/// consumes no CPU time while waiting for an event to occur. Condition
/// variables are typically associated with a boolean predicate (a condition)
/// and a mutex. The predicate is always verified inside of the mutex before
/// determining that thread must block.
///
/// Note that this module places one additional restriction over the system
/// condition variables: each condvar can be used with only one mutex at a
/// time. Any attempt to use multiple mutexes on the same condition variable
/// simultaneously will result in a runtime panic. However it is possible to
/// switch to a different mutex if there are no threads currently waiting on
/// the condition variable.
///
/// # Differences from the standard library `Condvar`
///
/// - No spurious wakeups: A wait will only return a non-timeout result if it
///   was woken up by `notify_one` or `notify_all`.
/// - `Condvar::notify_all` will only wake up a single thread, the rest are
///   requeued to wait for the `Mutex` to be unlocked by the thread that was
///   woken up.
/// - Only requires 1 word of space, whereas the standard library boxes the
///   `Condvar` due to platform limitations.
/// - Can be statically constructed.
/// - Does not require any drop glue when dropped.
/// - Inline fast path for the uncontended case.
///
/// # Examples
///
/// ```
/// use parking_lot::{Mutex, Condvar};
/// use std::sync::Arc;
/// use std::thread;
///
/// let pair = Arc::new((Mutex::new(false), Condvar::new()));
/// let pair2 = pair.clone();
///
/// // Inside of our lock, spawn a new thread, and then wait for it to start
/// thread::spawn(move|| {
///     let &(ref lock, ref cvar) = &*pair2;
///     let mut started = lock.lock();
///     *started = true;
///     cvar.notify_one();
/// });
///
/// // wait for the thread to start up
/// let &(ref lock, ref cvar) = &*pair;
/// let mut started = lock.lock();
/// if !*started {
///     cvar.wait(&mut started);
/// }
/// // Note that we used an if instead of a while loop above. This is only
/// // possible because parking_lot's Condvar will never spuriously wake up.
/// // This means that wait() will only return after notify_one or notify_all is
/// // called.
/// ```
pub struct Condvar {
    state: AtomicPtr<RawMutex>,
}

impl Condvar {
    /// Creates a new condition variable which is ready to be waited on and
    /// notified.
    #[inline]
    pub const fn new() -> Condvar {
        Condvar {
            state: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// Wakes up one blocked thread on this condvar.
    ///
    /// Returns whether a thread was woken up.
    ///
    /// If there is a blocked thread on this condition variable, then it will
    /// be woken up from its call to `wait` or `wait_timeout`. Calls to
    /// `notify_one` are not buffered in any way.
    ///
    /// To wake up all threads, see `notify_all()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use parking_lot::Condvar;
    ///
    /// let condvar = Condvar::new();
    ///
    /// // do something with condvar, share it with other threads
    ///
    /// if !condvar.notify_one() {
    ///     println!("Nobody was listening for this.");
    /// }
    /// ```
    #[inline]
    pub fn notify_one(&self) -> bool {
        // Nothing to do if there are no waiting threads
        let state = self.state.load(Ordering::Relaxed);
        if state.is_null() {
            return false;
        }

        self.notify_one_slow(state)
    }

    #[cold]
    fn notify_one_slow(&self, mutex: *mut RawMutex) -> bool {
        // Unpark one thread and requeue the rest onto the mutex
        let from = self as *const _ as usize;
        let to = mutex as usize;
        let validate = || {
            // Make sure that our atomic state still points to the same
            // mutex. If not then it means that all threads on the current
            // mutex were woken up and a new waiting thread switched to a
            // different mutex. In that case we can get away with doing
            // nothing.
            if self.state.load(Ordering::Relaxed) != mutex {
                return RequeueOp::Abort;
            }

            // Unpark one thread if the mutex is unlocked, otherwise just
            // requeue everything to the mutex. This is safe to do here
            // since unlocking the mutex when the parked bit is set requires
            // locking the queue. There is the possibility of a race if the
            // mutex gets locked after we check, but that doesn't matter in
            // this case.
            if unsafe { (*mutex).mark_parked_if_locked() } {
                RequeueOp::RequeueOne
            } else {
                RequeueOp::UnparkOne
            }
        };
        let callback = |_op, result: UnparkResult| {
            // Clear our state if there are no more waiting threads
            if !result.have_more_threads {
                self.state.store(ptr::null_mut(), Ordering::Relaxed);
            }
            TOKEN_NORMAL
        };
        let res = unsafe { parking_lot_core::unpark_requeue(from, to, validate, callback) };

        res.unparked_threads + res.requeued_threads != 0
    }

    /// Wakes up all blocked threads on this condvar.
    ///
    /// Returns the number of threads woken up.
    ///
    /// This method will ensure that any current waiters on the condition
    /// variable are awoken. Calls to `notify_all()` are not buffered in any
    /// way.
    ///
    /// To wake up only one thread, see `notify_one()`.
    #[inline]
    pub fn notify_all(&self) -> usize {
        // Nothing to do if there are no waiting threads
        let state = self.state.load(Ordering::Relaxed);
        if state.is_null() {
            return 0;
        }

        self.notify_all_slow(state)
    }

    #[cold]
    fn notify_all_slow(&self, mutex: *mut RawMutex) -> usize {
        // Unpark one thread and requeue the rest onto the mutex
        let from = self as *const _ as usize;
        let to = mutex as usize;
        let validate = || {
            // Make sure that our atomic state still points to the same
            // mutex. If not then it means that all threads on the current
            // mutex were woken up and a new waiting thread switched to a
            // different mutex. In that case we can get away with doing
            // nothing.
            if self.state.load(Ordering::Relaxed) != mutex {
                return RequeueOp::Abort;
            }

            // Clear our state since we are going to unpark or requeue all
            // threads.
            self.state.store(ptr::null_mut(), Ordering::Relaxed);

            // Unpark one thread if the mutex is unlocked, otherwise just
            // requeue everything to the mutex. This is safe to do here
            // since unlocking the mutex when the parked bit is set requires
            // locking the queue. There is the possibility of a race if the
            // mutex gets locked after we check, but that doesn't matter in
            // this case.
            if unsafe { (*mutex).mark_parked_if_locked() } {
                RequeueOp::RequeueAll
            } else {
                RequeueOp::UnparkOneRequeueRest
            }
        };
        let callback = |op, result: UnparkResult| {
            // If we requeued threads to the mutex, mark it as having
            // parked threads. The RequeueAll case is already handled above.
            if op == RequeueOp::UnparkOneRequeueRest && result.requeued_threads != 0 {
                unsafe { (*mutex).mark_parked() };
            }
            TOKEN_NORMAL
        };
        let res = unsafe { parking_lot_core::unpark_requeue(from, to, validate, callback) };

        res.unparked_threads + res.requeued_threads
    }

    /// Blocks the current thread until this condition variable receives a
    /// notification.
    ///
    /// This function will atomically unlock the mutex specified (represented by
    /// `mutex_guard`) and block the current thread. This means that any calls
    /// to `notify_*()` which happen logically after the mutex is unlocked are
    /// candidates to wake this thread up. When this function call returns, the
    /// lock specified will have been re-acquired.
    ///
    /// # Panics
    ///
    /// This function will panic if another thread is waiting on the `Condvar`
    /// with a different `Mutex` object.
    #[inline]
    pub fn wait<T: ?Sized>(&self, mutex_guard: &mut MutexGuard<'_, T>) {
        self.wait_until_internal(unsafe { MutexGuard::mutex(mutex_guard).raw() }, None);
    }

    /// Waits on this condition variable for a notification, timing out after
    /// the specified time instant.
    ///
    /// The semantics of this function are equivalent to `wait()` except that
    /// the thread will be blocked roughly until `timeout` is reached. This
    /// method should not be used for precise timing due to anomalies such as
    /// preemption or platform differences that may not cause the maximum
    /// amount of time waited to be precisely `timeout`.
    ///
    /// Note that the best effort is made to ensure that the time waited is
    /// measured with a monotonic clock, and not affected by the changes made to
    /// the system time.
    ///
    /// The returned `WaitTimeoutResult` value indicates if the timeout is
    /// known to have elapsed.
    ///
    /// Like `wait`, the lock specified will be re-acquired when this function
    /// returns, regardless of whether the timeout elapsed or not.
    ///
    /// # Panics
    ///
    /// This function will panic if another thread is waiting on the `Condvar`
    /// with a different `Mutex` object.
    #[inline]
    pub fn wait_until<T: ?Sized>(
        &self,
        mutex_guard: &mut MutexGuard<'_, T>,
        timeout: Instant,
    ) -> WaitTimeoutResult {
        self.wait_until_internal(
            unsafe { MutexGuard::mutex(mutex_guard).raw() },
            Some(timeout),
        )
    }

    // This is a non-generic function to reduce the monomorphization cost of
    // using `wait_until`.
    fn wait_until_internal(&self, mutex: &RawMutex, timeout: Option<Instant>) -> WaitTimeoutResult {
        let result;
        let mut bad_mutex = false;
        let mut requeued = false;
        {
            let addr = self as *const _ as usize;
            let lock_addr = mutex as *const _ as *mut _;
            let validate = || {
                // Ensure we don't use two different mutexes with the same
                // Condvar at the same time. This is done while locked to
                // avoid races with notify_one
                let state = self.state.load(Ordering::Relaxed);
                if state.is_null() {
                    self.state.store(lock_addr, Ordering::Relaxed);
                } else if state != lock_addr {
                    bad_mutex = true;
                    return false;
                }
                true
            };
            let before_sleep = || {
                // Unlock the mutex before sleeping...
                unsafe { mutex.unlock() };
            };
            let timed_out = |k, was_last_thread| {
                // If we were requeued to a mutex, then we did not time out.
                // We'll just park ourselves on the mutex again when we try
                // to lock it later.
                requeued = k != addr;

                // If we were the last thread on the queue then we need to
                // clear our state. This is normally done by the
                // notify_{one,all} functions when not timing out.
                if !requeued && was_last_thread {
                    self.state.store(ptr::null_mut(), Ordering::Relaxed);
                }
            };
            result = unsafe {
                parking_lot_core::park(
                    addr,
                    validate,
                    before_sleep,
                    timed_out,
                    DEFAULT_PARK_TOKEN,
                    timeout,
                )
            };
        }

        // Panic if we tried to use multiple mutexes with a Condvar. Note
        // that at this point the MutexGuard is still locked. It will be
        // unlocked by the unwinding logic.
        if bad_mutex {
            panic!("attempted to use a condition variable with more than one mutex");
        }

        // ... and re-lock it once we are done sleeping
        if result == ParkResult::Unparked(TOKEN_HANDOFF) {
            unsafe { deadlock::acquire_resource(mutex as *const _ as usize) };
        } else {
            mutex.lock();
        }

        WaitTimeoutResult(!(result.is_unparked() || requeued))
    }

    /// Waits on this condition variable for a notification, timing out after a
    /// specified duration.
    ///
    /// The semantics of this function are equivalent to `wait()` except that
    /// the thread will be blocked for roughly no longer than `timeout`. This
    /// method should not be used for precise timing due to anomalies such as
    /// preemption or platform differences that may not cause the maximum
    /// amount of time waited to be precisely `timeout`.
    ///
    /// Note that the best effort is made to ensure that the time waited is
    /// measured with a monotonic clock, and not affected by the changes made to
    /// the system time.
    ///
    /// The returned `WaitTimeoutResult` value indicates if the timeout is
    /// known to have elapsed.
    ///
    /// Like `wait`, the lock specified will be re-acquired when this function
    /// returns, regardless of whether the timeout elapsed or not.
    #[inline]
    pub fn wait_for<T: ?Sized>(
        &self,
        mutex_guard: &mut MutexGuard<'_, T>,
        timeout: Duration,
    ) -> WaitTimeoutResult {
        let deadline = util::to_deadline(timeout);
        self.wait_until_internal(unsafe { MutexGuard::mutex(mutex_guard).raw() }, deadline)
    }

    #[inline]
    fn wait_while_until_internal<T, F>(
        &self,
        mutex_guard: &mut MutexGuard<'_, T>,
        mut condition: F,
        timeout: Option<Instant>,
    ) -> WaitTimeoutResult
    where
        T: ?Sized,
        F: FnMut(&mut T) -> bool,
    {
        let mut result = WaitTimeoutResult(false);

        while !result.timed_out() && condition(mutex_guard.deref_mut()) {
            result =
                self.wait_until_internal(unsafe { MutexGuard::mutex(mutex_guard).raw() }, timeout);
        }

        result
    }
    /// Blocks the current thread until this condition variable receives a
    /// notification. If the provided condition evaluates to `false`, then the
    /// thread is no longer blocked and the operation is completed. If the
    /// condition evaluates to `true`, then the thread is blocked again and
    /// waits for another notification before repeating this process.
    ///
    /// This function will atomically unlock the mutex specified (represented by
    /// `mutex_guard`) and block the current thread. This means that any calls
    /// to `notify_*()` which happen logically after the mutex is unlocked are
    /// candidates to wake this thread up. When this function call returns, the
    /// lock specified will have been re-acquired.
    ///
    /// # Panics
    ///
    /// This function will panic if another thread is waiting on the `Condvar`
    /// with a different `Mutex` object.
    #[inline]
    pub fn wait_while<T, F>(&self, mutex_guard: &mut MutexGuard<'_, T>, condition: F)
    where
        T: ?Sized,
        F: FnMut(&mut T) -> bool,
    {
        self.wait_while_until_internal(mutex_guard, condition, None);
    }

    /// Waits on this condition variable for a notification, timing out after
    /// the specified time instant. If the provided condition evaluates to
    /// `false`, then the thread is no longer blocked and the operation is
    /// completed. If the condition evaluates to `true`, then the thread is
    /// blocked again and waits for another notification before repeating
    /// this process.
    ///
    /// The semantics of this function are equivalent to `wait()` except that
    /// the thread will be blocked roughly until `timeout` is reached. This
    /// method should not be used for precise timing due to anomalies such as
    /// preemption or platform differences that may not cause the maximum
    /// amount of time waited to be precisely `timeout`.
    ///
    /// Note that the best effort is made to ensure that the time waited is
    /// measured with a monotonic clock, and not affected by the changes made to
    /// the system time.
    ///
    /// The returned `WaitTimeoutResult` value indicates if the timeout is
    /// known to have elapsed.
    ///
    /// Like `wait`, the lock specified will be re-acquired when this function
    /// returns, regardless of whether the timeout elapsed or not.
    ///
    /// # Panics
    ///
    /// This function will panic if another thread is waiting on the `Condvar`
    /// with a different `Mutex` object.
    #[inline]
    pub fn wait_while_until<T, F>(
        &self,
        mutex_guard: &mut MutexGuard<'_, T>,
        condition: F,
        timeout: Instant,
    ) -> WaitTimeoutResult
    where
        T: ?Sized,
        F: FnMut(&mut T) -> bool,
    {
        self.wait_while_until_internal(mutex_guard, condition, Some(timeout))
    }

    /// Waits on this condition variable for a notification, timing out after a
    /// specified duration. If the provided condition evaluates to `false`,
    /// then the thread is no longer blocked and the operation is completed.
    /// If the condition evaluates to `true`, then the thread is blocked again
    /// and waits for another notification before repeating this process.
    ///
    /// The semantics of this function are equivalent to `wait()` except that
    /// the thread will be blocked for roughly no longer than `timeout`. This
    /// method should not be used for precise timing due to anomalies such as
    /// preemption or platform differences that may not cause the maximum
    /// amount of time waited to be precisely `timeout`.
    ///
    /// Note that the best effort is made to ensure that the time waited is
    /// measured with a monotonic clock, and not affected by the changes made to
    /// the system time.
    ///
    /// The returned `WaitTimeoutResult` value indicates if the timeout is
    /// known to have elapsed.
    ///
    /// Like `wait`, the lock specified will be re-acquired when this function
    /// returns, regardless of whether the timeout elapsed or not.
    #[inline]
    pub fn wait_while_for<T: ?Sized, F>(
        &self,
        mutex_guard: &mut MutexGuard<'_, T>,
        condition: F,
        timeout: Duration,
    ) -> WaitTimeoutResult
    where
        F: FnMut(&mut T) -> bool,
    {
        let deadline = util::to_deadline(timeout);
        self.wait_while_until_internal(mutex_guard, condition, deadline)
    }
}

impl Default for Condvar {
    #[inline]
    fn default() -> Condvar {
        Condvar::new()
    }
}

impl fmt::Debug for Condvar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Condvar { .. }")
    }
}

#[cfg(test)]
mod tests {
    use crate::{Condvar, Mutex, MutexGuard};
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::thread;
    use std::thread::sleep;
    use std::thread::JoinHandle;
    use std::time::Duration;
    use std::time::Instant;

    #[test]
    fn smoke() {
        let c = Condvar::new();
        c.notify_one();
        c.notify_all();
    }

    #[test]
    fn notify_one() {
        let m = Arc::new(Mutex::new(()));
        let m2 = m.clone();
        let c = Arc::new(Condvar::new());
        let c2 = c.clone();

        let mut g = m.lock();
        let _t = thread::spawn(move || {
            let _g = m2.lock();
            c2.notify_one();
        });
        c.wait(&mut g);
    }

    #[test]
    fn notify_all() {
        const N: usize = 10;

        let data = Arc::new((Mutex::new(0), Condvar::new()));
        let (tx, rx) = channel();
        for _ in 0..N {
            let data = data.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let (lock, cond) = &*data;
                let mut cnt = lock.lock();
                *cnt += 1;
                if *cnt == N {
                    tx.send(()).unwrap();
                }
                while *cnt != 0 {
                    cond.wait(&mut cnt);
                }
                tx.send(()).unwrap();
            });
        }
        drop(tx);

        let (lock, cond) = &*data;
        rx.recv().unwrap();
        let mut cnt = lock.lock();
        *cnt = 0;
        cond.notify_all();
        drop(cnt);

        for _ in 0..N {
            rx.recv().unwrap();
        }
    }

    #[test]
    fn notify_one_return_true() {
        let m = Arc::new(Mutex::new(()));
        let m2 = m.clone();
        let c = Arc::new(Condvar::new());
        let c2 = c.clone();

        let mut g = m.lock();
        let _t = thread::spawn(move || {
            let _g = m2.lock();
            assert!(c2.notify_one());
        });
        c.wait(&mut g);
    }

    #[test]
    fn notify_one_return_false() {
        let m = Arc::new(Mutex::new(()));
        let c = Arc::new(Condvar::new());

        let _t = thread::spawn(move || {
            let _g = m.lock();
            assert!(!c.notify_one());
        });
    }

    #[test]
    fn notify_all_return() {
        const N: usize = 10;

        let data = Arc::new((Mutex::new(0), Condvar::new()));
        let (tx, rx) = channel();
        for _ in 0..N {
            let data = data.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let (lock, cond) = &*data;
                let mut cnt = lock.lock();
                *cnt += 1;
                if *cnt == N {
                    tx.send(()).unwrap();
                }
                while *cnt != 0 {
                    cond.wait(&mut cnt);
                }
                tx.send(()).unwrap();
            });
        }
        drop(tx);

        let (lock, cond) = &*data;
        rx.recv().unwrap();
        let mut cnt = lock.lock();
        *cnt = 0;
        assert_eq!(cond.notify_all(), N);
        drop(cnt);

        for _ in 0..N {
            rx.recv().unwrap();
        }

        assert_eq!(cond.notify_all(), 0);
    }

    #[test]
    fn wait_for() {
        let m = Arc::new(Mutex::new(()));
        let m2 = m.clone();
        let c = Arc::new(Condvar::new());
        let c2 = c.clone();

        let mut g = m.lock();
        let no_timeout = c.wait_for(&mut g, Duration::from_millis(1));
        assert!(no_timeout.timed_out());

        let _t = thread::spawn(move || {
            let _g = m2.lock();
            c2.notify_one();
        });
        let timeout_res = c.wait_for(&mut g, Duration::from_secs(u64::max_value()));
        assert!(!timeout_res.timed_out());

        drop(g);
    }

    #[test]
    fn wait_until() {
        let m = Arc::new(Mutex::new(()));
        let m2 = m.clone();
        let c = Arc::new(Condvar::new());
        let c2 = c.clone();

        let mut g = m.lock();
        let no_timeout = c.wait_until(&mut g, Instant::now() + Duration::from_millis(1));
        assert!(no_timeout.timed_out());
        let _t = thread::spawn(move || {
            let _g = m2.lock();
            c2.notify_one();
        });
        let timeout_res = c.wait_until(
            &mut g,
            Instant::now() + Duration::from_millis(u32::max_value() as u64),
        );
        assert!(!timeout_res.timed_out());
        drop(g);
    }

    fn spawn_wait_while_notifier(
        mutex: Arc<Mutex<u32>>,
        cv: Arc<Condvar>,
        num_iters: u32,
        timeout: Option<Instant>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            for epoch in 1..=num_iters {
                // spin to wait for main test thread to block
                // before notifying it to wake back up and check
                // its condition.
                let mut sleep_backoff = Duration::from_millis(1);
                let _mutex_guard = loop {
                    let mutex_guard = mutex.lock();

                    if let Some(timeout) = timeout {
                        if Instant::now() >= timeout {
                            return;
                        }
                    }

                    if *mutex_guard == epoch {
                        break mutex_guard;
                    }

                    drop(mutex_guard);

                    // give main test thread a good chance to
                    // acquire the lock before this thread does.
                    sleep(sleep_backoff);
                    sleep_backoff *= 2;
                };

                cv.notify_one();
            }
        })
    }

    #[test]
    fn wait_while_until_internal_does_not_wait_if_initially_false() {
        let mutex = Arc::new(Mutex::new(0));
        let cv = Arc::new(Condvar::new());

        let condition = |counter: &mut u32| {
            *counter += 1;
            false
        };

        let mut mutex_guard = mutex.lock();
        let timeout_result = cv.wait_while_until_internal(&mut mutex_guard, condition, None);

        assert!(!timeout_result.timed_out());
        assert!(*mutex_guard == 1);
    }

    #[test]
    fn wait_while_until_internal_times_out_before_false() {
        let mutex = Arc::new(Mutex::new(0));
        let cv = Arc::new(Condvar::new());

        let num_iters = 3;
        let condition = |counter: &mut u32| {
            *counter += 1;
            true
        };

        let mut mutex_guard = mutex.lock();
        let timeout = Some(Instant::now() + Duration::from_millis(500));
        let handle = spawn_wait_while_notifier(mutex.clone(), cv.clone(), num_iters, timeout);

        let timeout_result = cv.wait_while_until_internal(&mut mutex_guard, condition, timeout);

        assert!(timeout_result.timed_out());
        assert!(*mutex_guard == num_iters + 1);

        // prevent deadlock with notifier
        drop(mutex_guard);
        handle.join().unwrap();
    }

    #[test]
    fn wait_while_until_internal() {
        let mutex = Arc::new(Mutex::new(0));
        let cv = Arc::new(Condvar::new());

        let num_iters = 4;

        let condition = |counter: &mut u32| {
            *counter += 1;
            *counter <= num_iters
        };

        let mut mutex_guard = mutex.lock();
        let handle = spawn_wait_while_notifier(mutex.clone(), cv.clone(), num_iters, None);

        let timeout_result = cv.wait_while_until_internal(&mut mutex_guard, condition, None);

        assert!(!timeout_result.timed_out());
        assert!(*mutex_guard == num_iters + 1);

        let timeout_result = cv.wait_while_until_internal(&mut mutex_guard, condition, None);
        handle.join().unwrap();

        assert!(!timeout_result.timed_out());
        assert!(*mutex_guard == num_iters + 2);
    }

    #[test]
    #[should_panic]
    fn two_mutexes() {
        let m = Arc::new(Mutex::new(()));
        let m2 = m.clone();
        let m3 = Arc::new(Mutex::new(()));
        let c = Arc::new(Condvar::new());
        let c2 = c.clone();

        // Make sure we don't leave the child thread dangling
        struct PanicGuard<'a>(&'a Condvar);
        impl<'a> Drop for PanicGuard<'a> {
            fn drop(&mut self) {
                self.0.notify_one();
            }
        }

        let (tx, rx) = channel();
        let g = m.lock();
        let _t = thread::spawn(move || {
            let mut g = m2.lock();
            tx.send(()).unwrap();
            c2.wait(&mut g);
        });
        drop(g);
        rx.recv().unwrap();
        let _g = m.lock();
        let _guard = PanicGuard(&c);
        c.wait(&mut m3.lock());
    }

    #[test]
    fn two_mutexes_disjoint() {
        let m = Arc::new(Mutex::new(()));
        let m2 = m.clone();
        let m3 = Arc::new(Mutex::new(()));
        let c = Arc::new(Condvar::new());
        let c2 = c.clone();

        let mut g = m.lock();
        let _t = thread::spawn(move || {
            let _g = m2.lock();
            c2.notify_one();
        });
        c.wait(&mut g);
        drop(g);

        let _ = c.wait_for(&mut m3.lock(), Duration::from_millis(1));
    }

    #[test]
    fn test_debug_condvar() {
        let c = Condvar::new();
        assert_eq!(format!("{:?}", c), "Condvar { .. }");
    }

    #[test]
    fn test_condvar_requeue() {
        let m = Arc::new(Mutex::new(()));
        let m2 = m.clone();
        let c = Arc::new(Condvar::new());
        let c2 = c.clone();
        let t = thread::spawn(move || {
            let mut g = m2.lock();
            c2.wait(&mut g);
        });

        let mut g = m.lock();
        while !c.notify_one() {
            // Wait for the thread to get into wait()
            MutexGuard::bump(&mut g);
            // Yield, so the other thread gets a chance to do something.
            // (At least Miri needs this, because it doesn't preempt threads.)
            thread::yield_now();
        }
        // The thread should have been requeued to the mutex, which we wake up now.
        drop(g);
        t.join().unwrap();
    }

    #[test]
    fn test_issue_129() {
        let locks = Arc::new((Mutex::new(()), Condvar::new()));

        let (tx, rx) = channel();
        for _ in 0..4 {
            let locks = locks.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let mut guard = locks.0.lock();
                locks.1.wait(&mut guard);
                locks.1.wait_for(&mut guard, Duration::from_millis(1));
                locks.1.notify_one();
                tx.send(()).unwrap();
            });
        }

        thread::sleep(Duration::from_millis(100));
        locks.1.notify_one();

        for _ in 0..4 {
            assert_eq!(rx.recv_timeout(Duration::from_millis(500)), Ok(()));
        }
    }
}

/// This module contains an integration test that is heavily inspired from WebKit's own integration
/// tests for it's own Condvar.
#[cfg(test)]
mod webkit_queue_test {
    use crate::{Condvar, Mutex, MutexGuard};
    use std::{collections::VecDeque, sync::Arc, thread, time::Duration};

    #[derive(Clone, Copy)]
    enum Timeout {
        Bounded(Duration),
        Forever,
    }

    #[derive(Clone, Copy)]
    enum NotifyStyle {
        One,
        All,
    }

    struct Queue {
        items: VecDeque<usize>,
        should_continue: bool,
    }

    impl Queue {
        fn new() -> Self {
            Self {
                items: VecDeque::new(),
                should_continue: true,
            }
        }
    }

    fn wait<T: ?Sized>(
        condition: &Condvar,
        lock: &mut MutexGuard<'_, T>,
        predicate: impl Fn(&mut MutexGuard<'_, T>) -> bool,
        timeout: &Timeout,
    ) {
        while !predicate(lock) {
            match timeout {
                Timeout::Forever => condition.wait(lock),
                Timeout::Bounded(bound) => {
                    condition.wait_for(lock, *bound);
                }
            }
        }
    }

    fn notify(style: NotifyStyle, condition: &Condvar, should_notify: bool) {
        match style {
            NotifyStyle::One => {
                condition.notify_one();
            }
            NotifyStyle::All => {
                if should_notify {
                    condition.notify_all();
                }
            }
        }
    }

    fn run_queue_test(
        num_producers: usize,
        num_consumers: usize,
        max_queue_size: usize,
        messages_per_producer: usize,
        notify_style: NotifyStyle,
        timeout: Timeout,
        delay: Duration,
    ) {
        let input_queue = Arc::new(Mutex::new(Queue::new()));
        let empty_condition = Arc::new(Condvar::new());
        let full_condition = Arc::new(Condvar::new());

        let output_vec = Arc::new(Mutex::new(vec![]));

        let consumers = (0..num_consumers)
            .map(|_| {
                consumer_thread(
                    input_queue.clone(),
                    empty_condition.clone(),
                    full_condition.clone(),
                    timeout,
                    notify_style,
                    output_vec.clone(),
                    max_queue_size,
                )
            })
            .collect::<Vec<_>>();
        let producers = (0..num_producers)
            .map(|_| {
                producer_thread(
                    messages_per_producer,
                    input_queue.clone(),
                    empty_condition.clone(),
                    full_condition.clone(),
                    timeout,
                    notify_style,
                    max_queue_size,
                )
            })
            .collect::<Vec<_>>();

        thread::sleep(delay);

        for producer in producers.into_iter() {
            producer.join().expect("Producer thread panicked");
        }

        {
            let mut input_queue = input_queue.lock();
            input_queue.should_continue = false;
        }
        empty_condition.notify_all();

        for consumer in consumers.into_iter() {
            consumer.join().expect("Consumer thread panicked");
        }

        let mut output_vec = output_vec.lock();
        assert_eq!(output_vec.len(), num_producers * messages_per_producer);
        output_vec.sort();
        for msg_idx in 0..messages_per_producer {
            for producer_idx in 0..num_producers {
                assert_eq!(msg_idx, output_vec[msg_idx * num_producers + producer_idx]);
            }
        }
    }

    fn consumer_thread(
        input_queue: Arc<Mutex<Queue>>,
        empty_condition: Arc<Condvar>,
        full_condition: Arc<Condvar>,
        timeout: Timeout,
        notify_style: NotifyStyle,
        output_queue: Arc<Mutex<Vec<usize>>>,
        max_queue_size: usize,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || loop {
            let (should_notify, result) = {
                let mut queue = input_queue.lock();
                wait(
                    &empty_condition,
                    &mut queue,
                    |state| -> bool { !state.items.is_empty() || !state.should_continue },
                    &timeout,
                );
                if queue.items.is_empty() && !queue.should_continue {
                    return;
                }
                let should_notify = queue.items.len() == max_queue_size;
                let result = queue.items.pop_front();
                std::mem::drop(queue);
                (should_notify, result)
            };
            notify(notify_style, &full_condition, should_notify);

            if let Some(result) = result {
                output_queue.lock().push(result);
            }
        })
    }

    fn producer_thread(
        num_messages: usize,
        queue: Arc<Mutex<Queue>>,
        empty_condition: Arc<Condvar>,
        full_condition: Arc<Condvar>,
        timeout: Timeout,
        notify_style: NotifyStyle,
        max_queue_size: usize,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            for message in 0..num_messages {
                let should_notify = {
                    let mut queue = queue.lock();
                    wait(
                        &full_condition,
                        &mut queue,
                        |state| state.items.len() < max_queue_size,
                        &timeout,
                    );
                    let should_notify = queue.items.is_empty();
                    queue.items.push_back(message);
                    std::mem::drop(queue);
                    should_notify
                };
                notify(notify_style, &empty_condition, should_notify);
            }
        })
    }

    macro_rules! run_queue_tests {
        ( $( $name:ident(
            num_producers: $num_producers:expr,
            num_consumers: $num_consumers:expr,
            max_queue_size: $max_queue_size:expr,
            messages_per_producer: $messages_per_producer:expr,
            notification_style: $notification_style:expr,
            timeout: $timeout:expr,
            delay_seconds: $delay_seconds:expr);
        )* ) => {
            $(#[test]
            fn $name() {
                let delay = Duration::from_secs($delay_seconds);
                run_queue_test(
                    $num_producers,
                    $num_consumers,
                    $max_queue_size,
                    $messages_per_producer,
                    $notification_style,
                    $timeout,
                    delay,
                    );
            })*
        };
    }

    run_queue_tests! {
        sanity_check_queue(
            num_producers: 1,
            num_consumers: 1,
            max_queue_size: 1,
            messages_per_producer: 100_000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Bounded(Duration::from_secs(1)),
            delay_seconds: 0
        );
        sanity_check_queue_timeout(
            num_producers: 1,
            num_consumers: 1,
            max_queue_size: 1,
            messages_per_producer: 100_000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        new_test_without_timeout_5(
            num_producers: 1,
            num_consumers: 5,
            max_queue_size: 1,
            messages_per_producer: 100_000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        one_producer_one_consumer_one_slot(
            num_producers: 1,
            num_consumers: 1,
            max_queue_size: 1,
            messages_per_producer: 100_000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        one_producer_one_consumer_one_slot_timeout(
            num_producers: 1,
            num_consumers: 1,
            max_queue_size: 1,
            messages_per_producer: 100_000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Forever,
            delay_seconds: 1
        );
        one_producer_one_consumer_hundred_slots(
            num_producers: 1,
            num_consumers: 1,
            max_queue_size: 100,
            messages_per_producer: 1_000_000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        ten_producers_one_consumer_one_slot(
            num_producers: 10,
            num_consumers: 1,
            max_queue_size: 1,
            messages_per_producer: 10000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        ten_producers_one_consumer_hundred_slots_notify_all(
            num_producers: 10,
            num_consumers: 1,
            max_queue_size: 100,
            messages_per_producer: 10000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        ten_producers_one_consumer_hundred_slots_notify_one(
            num_producers: 10,
            num_consumers: 1,
            max_queue_size: 100,
            messages_per_producer: 10000,
            notification_style: NotifyStyle::One,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        one_producer_ten_consumers_one_slot(
            num_producers: 1,
            num_consumers: 10,
            max_queue_size: 1,
            messages_per_producer: 10000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        one_producer_ten_consumers_hundred_slots_notify_all(
            num_producers: 1,
            num_consumers: 10,
            max_queue_size: 100,
            messages_per_producer: 100_000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        one_producer_ten_consumers_hundred_slots_notify_one(
            num_producers: 1,
            num_consumers: 10,
            max_queue_size: 100,
            messages_per_producer: 100_000,
            notification_style: NotifyStyle::One,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        ten_producers_ten_consumers_one_slot(
            num_producers: 10,
            num_consumers: 10,
            max_queue_size: 1,
            messages_per_producer: 50000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        ten_producers_ten_consumers_hundred_slots_notify_all(
            num_producers: 10,
            num_consumers: 10,
            max_queue_size: 100,
            messages_per_producer: 50000,
            notification_style: NotifyStyle::All,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
        ten_producers_ten_consumers_hundred_slots_notify_one(
            num_producers: 10,
            num_consumers: 10,
            max_queue_size: 100,
            messages_per_producer: 50000,
            notification_style: NotifyStyle::One,
            timeout: Timeout::Forever,
            delay_seconds: 0
        );
    }
}
