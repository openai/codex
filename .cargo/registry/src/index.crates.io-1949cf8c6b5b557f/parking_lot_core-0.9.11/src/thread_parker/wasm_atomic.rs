// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use core::{
    arch::wasm32,
    sync::atomic::{AtomicI32, Ordering},
};
use std::time::{Duration, Instant};
use std::{convert::TryFrom, thread};

// Helper type for putting a thread to sleep until some other thread wakes it up
pub struct ThreadParker {
    parked: AtomicI32,
}

const UNPARKED: i32 = 0;
const PARKED: i32 = 1;

impl super::ThreadParkerT for ThreadParker {
    type UnparkHandle = UnparkHandle;

    const IS_CHEAP_TO_CONSTRUCT: bool = true;

    #[inline]
    fn new() -> ThreadParker {
        ThreadParker {
            parked: AtomicI32::new(UNPARKED),
        }
    }

    #[inline]
    unsafe fn prepare_park(&self) {
        self.parked.store(PARKED, Ordering::Relaxed);
    }

    #[inline]
    unsafe fn timed_out(&self) -> bool {
        self.parked.load(Ordering::Relaxed) == PARKED
    }

    #[inline]
    unsafe fn park(&self) {
        while self.parked.load(Ordering::Acquire) == PARKED {
            let r = wasm32::memory_atomic_wait32(self.ptr(), PARKED, -1);
            // we should have either woken up (0) or got a not-equal due to a
            // race (1). We should never time out (2)
            debug_assert!(r == 0 || r == 1);
        }
    }

    #[inline]
    unsafe fn park_until(&self, timeout: Instant) -> bool {
        while self.parked.load(Ordering::Acquire) == PARKED {
            if let Some(left) = timeout.checked_duration_since(Instant::now()) {
                let nanos_left = i64::try_from(left.as_nanos()).unwrap_or(i64::max_value());
                let r = wasm32::memory_atomic_wait32(self.ptr(), PARKED, nanos_left);
                debug_assert!(r == 0 || r == 1 || r == 2);
            } else {
                return false;
            }
        }
        true
    }

    #[inline]
    unsafe fn unpark_lock(&self) -> UnparkHandle {
        // We don't need to lock anything, just clear the state
        self.parked.store(UNPARKED, Ordering::Release);
        UnparkHandle(self.ptr())
    }
}

impl ThreadParker {
    #[inline]
    fn ptr(&self) -> *mut i32 {
        &self.parked as *const AtomicI32 as *mut i32
    }
}

pub struct UnparkHandle(*mut i32);

impl super::UnparkHandleT for UnparkHandle {
    #[inline]
    unsafe fn unpark(self) {
        let num_notified = wasm32::memory_atomic_notify(self.0 as *mut i32, 1);
        debug_assert!(num_notified == 0 || num_notified == 1);
    }
}

#[inline]
pub fn thread_yield() {
    thread::yield_now();
}
