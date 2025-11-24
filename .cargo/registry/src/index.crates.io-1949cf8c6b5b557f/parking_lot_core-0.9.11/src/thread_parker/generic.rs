// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! A simple spin lock based thread parker. Used on platforms without better
//! parking facilities available.

use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Instant;

// Helper type for putting a thread to sleep until some other thread wakes it up
pub struct ThreadParker {
    parked: AtomicBool,
}

impl super::ThreadParkerT for ThreadParker {
    type UnparkHandle = UnparkHandle;

    const IS_CHEAP_TO_CONSTRUCT: bool = true;

    #[inline]
    fn new() -> ThreadParker {
        ThreadParker {
            parked: AtomicBool::new(false),
        }
    }

    #[inline]
    unsafe fn prepare_park(&self) {
        self.parked.store(true, Ordering::Relaxed);
    }

    #[inline]
    unsafe fn timed_out(&self) -> bool {
        self.parked.load(Ordering::Relaxed) != false
    }

    #[inline]
    unsafe fn park(&self) {
        while self.parked.load(Ordering::Acquire) != false {
            spin_loop();
        }
    }

    #[inline]
    unsafe fn park_until(&self, timeout: Instant) -> bool {
        while self.parked.load(Ordering::Acquire) != false {
            if Instant::now() >= timeout {
                return false;
            }
            spin_loop();
        }
        true
    }

    #[inline]
    unsafe fn unpark_lock(&self) -> UnparkHandle {
        // We don't need to lock anything, just clear the state
        self.parked.store(false, Ordering::Release);
        UnparkHandle(())
    }
}

pub struct UnparkHandle(());

impl super::UnparkHandleT for UnparkHandle {
    #[inline]
    unsafe fn unpark(self) {}
}

#[inline]
pub fn thread_yield() {
    thread::yield_now();
}
