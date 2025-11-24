// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use core::{
    mem,
    sync::atomic::{AtomicUsize, Ordering},
};
use std::{ffi, time::Instant};

use super::bindings::*;

#[allow(non_snake_case)]
pub struct WaitAddress {
    WaitOnAddress: WaitOnAddress,
    WakeByAddressSingle: WakeByAddressSingle,
}

impl WaitAddress {
    #[allow(non_snake_case)]
    pub fn create() -> Option<WaitAddress> {
        let synch_dll = unsafe { GetModuleHandleA(b"api-ms-win-core-synch-l1-2-0.dll\0".as_ptr()) };
        if synch_dll == 0 {
            return None;
        }

        let WaitOnAddress = unsafe { GetProcAddress(synch_dll, b"WaitOnAddress\0".as_ptr())? };
        let WakeByAddressSingle =
            unsafe { GetProcAddress(synch_dll, b"WakeByAddressSingle\0".as_ptr())? };

        Some(WaitAddress {
            WaitOnAddress: unsafe { mem::transmute(WaitOnAddress) },
            WakeByAddressSingle: unsafe { mem::transmute(WakeByAddressSingle) },
        })
    }

    #[inline]
    pub fn prepare_park(&'static self, key: &AtomicUsize) {
        key.store(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn timed_out(&'static self, key: &AtomicUsize) -> bool {
        key.load(Ordering::Relaxed) != 0
    }

    #[inline]
    pub fn park(&'static self, key: &AtomicUsize) {
        while key.load(Ordering::Acquire) != 0 {
            let r = self.wait_on_address(key, INFINITE);
            debug_assert!(r == true.into());
        }
    }

    #[inline]
    pub fn park_until(&'static self, key: &AtomicUsize, timeout: Instant) -> bool {
        while key.load(Ordering::Acquire) != 0 {
            let now = Instant::now();
            if timeout <= now {
                return false;
            }
            let diff = timeout - now;
            let timeout = diff
                .as_secs()
                .checked_mul(1000)
                .and_then(|x| x.checked_add((diff.subsec_nanos() as u64 + 999999) / 1000000))
                .map(|ms| {
                    if ms > std::u32::MAX as u64 {
                        INFINITE
                    } else {
                        ms as u32
                    }
                })
                .unwrap_or(INFINITE);
            if self.wait_on_address(key, timeout) == false.into() {
                debug_assert_eq!(unsafe { GetLastError() }, ERROR_TIMEOUT);
            }
        }
        true
    }

    #[inline]
    pub fn unpark_lock(&'static self, key: &AtomicUsize) -> UnparkHandle {
        // We don't need to lock anything, just clear the state
        key.store(0, Ordering::Release);

        UnparkHandle {
            key: key,
            waitaddress: self,
        }
    }

    #[inline]
    fn wait_on_address(&'static self, key: &AtomicUsize, timeout: u32) -> BOOL {
        let cmp = 1usize;
        unsafe {
            (self.WaitOnAddress)(
                key as *const _ as *mut ffi::c_void,
                &cmp as *const _ as *mut ffi::c_void,
                mem::size_of::<usize>(),
                timeout,
            )
        }
    }
}

// Handle for a thread that is about to be unparked. We need to mark the thread
// as unparked while holding the queue lock, but we delay the actual unparking
// until after the queue lock is released.
pub struct UnparkHandle {
    key: *const AtomicUsize,
    waitaddress: &'static WaitAddress,
}

impl UnparkHandle {
    // Wakes up the parked thread. This should be called after the queue lock is
    // released to avoid blocking the queue for too long.
    #[inline]
    pub fn unpark(self) {
        unsafe { (self.waitaddress.WakeByAddressSingle)(self.key as *mut ffi::c_void) };
    }
}
