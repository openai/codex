// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use crate::{deadlock, util};
use core::{
    sync::atomic::{AtomicU8, Ordering},
    time::Duration,
};
use lock_api::RawMutex as RawMutex_;
use parking_lot_core::{self, ParkResult, SpinWait, UnparkResult, UnparkToken, DEFAULT_PARK_TOKEN};
use std::time::Instant;

// UnparkToken used to indicate that that the target thread should attempt to
// lock the mutex again as soon as it is unparked.
pub(crate) const TOKEN_NORMAL: UnparkToken = UnparkToken(0);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
pub(crate) const TOKEN_HANDOFF: UnparkToken = UnparkToken(1);

/// This bit is set in the `state` of a `RawMutex` when that mutex is locked by some thread.
const LOCKED_BIT: u8 = 0b01;
/// This bit is set in the `state` of a `RawMutex` just before parking a thread. A thread is being
/// parked if it wants to lock the mutex, but it is currently being held by some other thread.
const PARKED_BIT: u8 = 0b10;

/// Raw mutex type backed by the parking lot.
pub struct RawMutex {
    /// This atomic integer holds the current state of the mutex instance. Only the two lowest bits
    /// are used. See `LOCKED_BIT` and `PARKED_BIT` for the bitmask for these bits.
    ///
    /// # State table:
    ///
    /// PARKED_BIT | LOCKED_BIT | Description
    ///     0      |     0      | The mutex is not locked, nor is anyone waiting for it.
    /// -----------+------------+------------------------------------------------------------------
    ///     0      |     1      | The mutex is locked by exactly one thread. No other thread is
    ///            |            | waiting for it.
    /// -----------+------------+------------------------------------------------------------------
    ///     1      |     0      | The mutex is not locked. One or more thread is parked or about to
    ///            |            | park. At least one of the parked threads are just about to be
    ///            |            | unparked, or a thread heading for parking might abort the park.
    /// -----------+------------+------------------------------------------------------------------
    ///     1      |     1      | The mutex is locked by exactly one thread. One or more thread is
    ///            |            | parked or about to park, waiting for the lock to become available.
    ///            |            | In this state, PARKED_BIT is only ever cleared when a bucket lock
    ///            |            | is held (i.e. in a parking_lot_core callback). This ensures that
    ///            |            | we never end up in a situation where there are parked threads but
    ///            |            | PARKED_BIT is not set (which would result in those threads
    ///            |            | potentially never getting woken up).
    state: AtomicU8,
}

unsafe impl lock_api::RawMutex for RawMutex {
    const INIT: RawMutex = RawMutex {
        state: AtomicU8::new(0),
    };

    type GuardMarker = crate::GuardMarker;

    #[inline]
    fn lock(&self) {
        if self
            .state
            .compare_exchange_weak(0, LOCKED_BIT, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            self.lock_slow(None);
        }
        unsafe { deadlock::acquire_resource(self as *const _ as usize) };
    }

    #[inline]
    fn try_lock(&self) -> bool {
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            if state & LOCKED_BIT != 0 {
                return false;
            }
            match self.state.compare_exchange_weak(
                state,
                state | LOCKED_BIT,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    unsafe { deadlock::acquire_resource(self as *const _ as usize) };
                    return true;
                }
                Err(x) => state = x,
            }
        }
    }

    #[inline]
    unsafe fn unlock(&self) {
        deadlock::release_resource(self as *const _ as usize);
        if self
            .state
            .compare_exchange(LOCKED_BIT, 0, Ordering::Release, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }
        self.unlock_slow(false);
    }

    #[inline]
    fn is_locked(&self) -> bool {
        let state = self.state.load(Ordering::Relaxed);
        state & LOCKED_BIT != 0
    }
}

unsafe impl lock_api::RawMutexFair for RawMutex {
    #[inline]
    unsafe fn unlock_fair(&self) {
        deadlock::release_resource(self as *const _ as usize);
        if self
            .state
            .compare_exchange(LOCKED_BIT, 0, Ordering::Release, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }
        self.unlock_slow(true);
    }

    #[inline]
    unsafe fn bump(&self) {
        if self.state.load(Ordering::Relaxed) & PARKED_BIT != 0 {
            self.bump_slow();
        }
    }
}

unsafe impl lock_api::RawMutexTimed for RawMutex {
    type Duration = Duration;
    type Instant = Instant;

    #[inline]
    fn try_lock_until(&self, timeout: Instant) -> bool {
        let result = if self
            .state
            .compare_exchange_weak(0, LOCKED_BIT, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            true
        } else {
            self.lock_slow(Some(timeout))
        };
        if result {
            unsafe { deadlock::acquire_resource(self as *const _ as usize) };
        }
        result
    }

    #[inline]
    fn try_lock_for(&self, timeout: Duration) -> bool {
        let result = if self
            .state
            .compare_exchange_weak(0, LOCKED_BIT, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            true
        } else {
            self.lock_slow(util::to_deadline(timeout))
        };
        if result {
            unsafe { deadlock::acquire_resource(self as *const _ as usize) };
        }
        result
    }
}

impl RawMutex {
    // Used by Condvar when requeuing threads to us, must be called while
    // holding the queue lock.
    #[inline]
    pub(crate) fn mark_parked_if_locked(&self) -> bool {
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            if state & LOCKED_BIT == 0 {
                return false;
            }
            match self.state.compare_exchange_weak(
                state,
                state | PARKED_BIT,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(x) => state = x,
            }
        }
    }

    // Used by Condvar when requeuing threads to us, must be called while
    // holding the queue lock.
    #[inline]
    pub(crate) fn mark_parked(&self) {
        self.state.fetch_or(PARKED_BIT, Ordering::Relaxed);
    }

    #[cold]
    fn lock_slow(&self, timeout: Option<Instant>) -> bool {
        let mut spinwait = SpinWait::new();
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            // Grab the lock if it isn't locked, even if there is a queue on it
            if state & LOCKED_BIT == 0 {
                match self.state.compare_exchange_weak(
                    state,
                    state | LOCKED_BIT,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return true,
                    Err(x) => state = x,
                }
                continue;
            }

            // If there is no queue, try spinning a few times
            if state & PARKED_BIT == 0 && spinwait.spin() {
                state = self.state.load(Ordering::Relaxed);
                continue;
            }

            // Set the parked bit
            if state & PARKED_BIT == 0 {
                if let Err(x) = self.state.compare_exchange_weak(
                    state,
                    state | PARKED_BIT,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    state = x;
                    continue;
                }
            }

            // Park our thread until we are woken up by an unlock
            let addr = self as *const _ as usize;
            let validate = || self.state.load(Ordering::Relaxed) == LOCKED_BIT | PARKED_BIT;
            let before_sleep = || {};
            let timed_out = |_, was_last_thread| {
                // Clear the parked bit if we were the last parked thread
                if was_last_thread {
                    self.state.fetch_and(!PARKED_BIT, Ordering::Relaxed);
                }
            };
            // SAFETY:
            //   * `addr` is an address we control.
            //   * `validate`/`timed_out` does not panic or call into any function of `parking_lot`.
            //   * `before_sleep` does not call `park`, nor does it panic.
            match unsafe {
                parking_lot_core::park(
                    addr,
                    validate,
                    before_sleep,
                    timed_out,
                    DEFAULT_PARK_TOKEN,
                    timeout,
                )
            } {
                // The thread that unparked us passed the lock on to us
                // directly without unlocking it.
                ParkResult::Unparked(TOKEN_HANDOFF) => return true,

                // We were unparked normally, try acquiring the lock again
                ParkResult::Unparked(_) => (),

                // The validation function failed, try locking again
                ParkResult::Invalid => (),

                // Timeout expired
                ParkResult::TimedOut => return false,
            }

            // Loop back and try locking again
            spinwait.reset();
            state = self.state.load(Ordering::Relaxed);
        }
    }

    #[cold]
    fn unlock_slow(&self, force_fair: bool) {
        // Unpark one thread and leave the parked bit set if there might
        // still be parked threads on this address.
        let addr = self as *const _ as usize;
        let callback = |result: UnparkResult| {
            // If we are using a fair unlock then we should keep the
            // mutex locked and hand it off to the unparked thread.
            if result.unparked_threads != 0 && (force_fair || result.be_fair) {
                // Clear the parked bit if there are no more parked
                // threads.
                if !result.have_more_threads {
                    self.state.store(LOCKED_BIT, Ordering::Relaxed);
                }
                return TOKEN_HANDOFF;
            }

            // Clear the locked bit, and the parked bit as well if there
            // are no more parked threads.
            if result.have_more_threads {
                self.state.store(PARKED_BIT, Ordering::Release);
            } else {
                self.state.store(0, Ordering::Release);
            }
            TOKEN_NORMAL
        };
        // SAFETY:
        //   * `addr` is an address we control.
        //   * `callback` does not panic or call into any function of `parking_lot`.
        unsafe {
            parking_lot_core::unpark_one(addr, callback);
        }
    }

    #[cold]
    fn bump_slow(&self) {
        unsafe { deadlock::release_resource(self as *const _ as usize) };
        self.unlock_slow(true);
        self.lock();
    }
}
