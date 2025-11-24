// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use core::{
    ffi,
    mem::{self, MaybeUninit},
    ptr,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

const STATE_UNPARKED: usize = 0;
const STATE_PARKED: usize = 1;
const STATE_TIMED_OUT: usize = 2;

use super::bindings::*;

#[allow(non_snake_case)]
pub struct KeyedEvent {
    handle: HANDLE,
    NtReleaseKeyedEvent: extern "system" fn(
        EventHandle: HANDLE,
        Key: *mut ffi::c_void,
        Alertable: BOOLEAN,
        Timeout: *mut i64,
    ) -> NTSTATUS,
    NtWaitForKeyedEvent: extern "system" fn(
        EventHandle: HANDLE,
        Key: *mut ffi::c_void,
        Alertable: BOOLEAN,
        Timeout: *mut i64,
    ) -> NTSTATUS,
}

impl KeyedEvent {
    #[inline]
    unsafe fn wait_for(&self, key: *mut ffi::c_void, timeout: *mut i64) -> NTSTATUS {
        (self.NtWaitForKeyedEvent)(self.handle, key, false.into(), timeout)
    }

    #[inline]
    unsafe fn release(&self, key: *mut ffi::c_void) -> NTSTATUS {
        (self.NtReleaseKeyedEvent)(self.handle, key, false.into(), ptr::null_mut())
    }

    #[allow(non_snake_case)]
    pub fn create() -> Option<KeyedEvent> {
        let ntdll = unsafe { GetModuleHandleA(b"ntdll.dll\0".as_ptr()) };
        if ntdll == 0 {
            return None;
        }

        let NtCreateKeyedEvent =
            unsafe { GetProcAddress(ntdll, b"NtCreateKeyedEvent\0".as_ptr())? };
        let NtReleaseKeyedEvent =
            unsafe { GetProcAddress(ntdll, b"NtReleaseKeyedEvent\0".as_ptr())? };
        let NtWaitForKeyedEvent =
            unsafe { GetProcAddress(ntdll, b"NtWaitForKeyedEvent\0".as_ptr())? };

        let NtCreateKeyedEvent: extern "system" fn(
            KeyedEventHandle: *mut HANDLE,
            DesiredAccess: u32,
            ObjectAttributes: *mut ffi::c_void,
            Flags: u32,
        ) -> NTSTATUS = unsafe { mem::transmute(NtCreateKeyedEvent) };
        let mut handle = MaybeUninit::uninit();
        let status = NtCreateKeyedEvent(
            handle.as_mut_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            ptr::null_mut(),
            0,
        );
        if status != STATUS_SUCCESS {
            return None;
        }

        Some(KeyedEvent {
            handle: unsafe { handle.assume_init() },
            NtReleaseKeyedEvent: unsafe { mem::transmute(NtReleaseKeyedEvent) },
            NtWaitForKeyedEvent: unsafe { mem::transmute(NtWaitForKeyedEvent) },
        })
    }

    #[inline]
    pub fn prepare_park(&'static self, key: &AtomicUsize) {
        key.store(STATE_PARKED, Ordering::Relaxed);
    }

    #[inline]
    pub fn timed_out(&'static self, key: &AtomicUsize) -> bool {
        key.load(Ordering::Relaxed) == STATE_TIMED_OUT
    }

    #[inline]
    pub unsafe fn park(&'static self, key: &AtomicUsize) {
        let status = self.wait_for(key as *const _ as *mut ffi::c_void, ptr::null_mut());
        debug_assert_eq!(status, STATUS_SUCCESS);
    }

    #[inline]
    pub unsafe fn park_until(&'static self, key: &AtomicUsize, timeout: Instant) -> bool {
        let now = Instant::now();
        if timeout <= now {
            // If another thread unparked us, we need to call
            // NtWaitForKeyedEvent otherwise that thread will stay stuck at
            // NtReleaseKeyedEvent.
            if key.swap(STATE_TIMED_OUT, Ordering::Relaxed) == STATE_UNPARKED {
                self.park(key);
                return true;
            }
            return false;
        }

        // NT uses a timeout in units of 100ns. We use a negative value to
        // indicate a relative timeout based on a monotonic clock.
        let diff = timeout - now;
        let value = (diff.as_secs() as i64)
            .checked_mul(-10000000)
            .and_then(|x| x.checked_sub((diff.subsec_nanos() as i64 + 99) / 100));

        let mut nt_timeout = match value {
            Some(x) => x,
            None => {
                // Timeout overflowed, just sleep indefinitely
                self.park(key);
                return true;
            }
        };

        let status = self.wait_for(key as *const _ as *mut ffi::c_void, &mut nt_timeout);
        if status == STATUS_SUCCESS {
            return true;
        }
        debug_assert_eq!(status, STATUS_TIMEOUT);

        // If another thread unparked us, we need to call NtWaitForKeyedEvent
        // otherwise that thread will stay stuck at NtReleaseKeyedEvent.
        if key.swap(STATE_TIMED_OUT, Ordering::Relaxed) == STATE_UNPARKED {
            self.park(key);
            return true;
        }
        false
    }

    #[inline]
    pub unsafe fn unpark_lock(&'static self, key: &AtomicUsize) -> UnparkHandle {
        // If the state was STATE_PARKED then we need to wake up the thread
        if key.swap(STATE_UNPARKED, Ordering::Relaxed) == STATE_PARKED {
            UnparkHandle {
                key: key,
                keyed_event: self,
            }
        } else {
            UnparkHandle {
                key: ptr::null(),
                keyed_event: self,
            }
        }
    }
}

impl Drop for KeyedEvent {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            let ok = CloseHandle(self.handle);
            debug_assert_eq!(ok, true.into());
        }
    }
}

// Handle for a thread that is about to be unparked. We need to mark the thread
// as unparked while holding the queue lock, but we delay the actual unparking
// until after the queue lock is released.
pub struct UnparkHandle {
    key: *const AtomicUsize,
    keyed_event: &'static KeyedEvent,
}

impl UnparkHandle {
    // Wakes up the parked thread. This should be called after the queue lock is
    // released to avoid blocking the queue for too long.
    #[inline]
    pub unsafe fn unpark(self) {
        if !self.key.is_null() {
            let status = self.keyed_event.release(self.key as *mut ffi::c_void);
            debug_assert_eq!(status, STATUS_SUCCESS);
        }
    }
}
