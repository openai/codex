// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use core::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use std::{
    io,
    os::fortanix_sgx::{
        thread::current as current_tcs,
        usercalls::{
            self,
            raw::{Tcs, EV_UNPARK, WAIT_INDEFINITE},
        },
    },
    thread,
};

// Helper type for putting a thread to sleep until some other thread wakes it up
pub struct ThreadParker {
    parked: AtomicBool,
    tcs: Tcs,
}

impl super::ThreadParkerT for ThreadParker {
    type UnparkHandle = UnparkHandle;

    const IS_CHEAP_TO_CONSTRUCT: bool = true;

    #[inline]
    fn new() -> ThreadParker {
        ThreadParker {
            parked: AtomicBool::new(false),
            tcs: current_tcs(),
        }
    }

    #[inline]
    unsafe fn prepare_park(&self) {
        self.parked.store(true, Ordering::Relaxed);
    }

    #[inline]
    unsafe fn timed_out(&self) -> bool {
        self.parked.load(Ordering::Relaxed)
    }

    #[inline]
    unsafe fn park(&self) {
        while self.parked.load(Ordering::Acquire) {
            let result = usercalls::wait(EV_UNPARK, WAIT_INDEFINITE);
            debug_assert_eq!(result.expect("wait returned error") & EV_UNPARK, EV_UNPARK);
        }
    }

    #[inline]
    unsafe fn park_until(&self, _timeout: Instant) -> bool {
        // FIXME: https://github.com/fortanix/rust-sgx/issues/31
        panic!("timeout not supported in SGX");
    }

    #[inline]
    unsafe fn unpark_lock(&self) -> UnparkHandle {
        // We don't need to lock anything, just clear the state
        self.parked.store(false, Ordering::Release);
        UnparkHandle(self.tcs)
    }
}

pub struct UnparkHandle(Tcs);

impl super::UnparkHandleT for UnparkHandle {
    #[inline]
    unsafe fn unpark(self) {
        let result = usercalls::send(EV_UNPARK, Some(self.0));
        if cfg!(debug_assertions) {
            if let Err(error) = result {
                // `InvalidInput` may be returned if the thread we send to has
                // already been unparked and exited.
                if error.kind() != io::ErrorKind::InvalidInput {
                    panic!("send returned an unexpected error: {:?}", error);
                }
            }
        }
    }
}

#[inline]
pub fn thread_yield() {
    thread::yield_now();
}
