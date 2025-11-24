//! Implementation of `errno` functionality for WASI.
//!
//! Adapted from `unix.rs`.

// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::str;
use libc::{self, c_int, size_t, strerror_r, strlen};

use crate::Errno;

fn from_utf8_lossy(input: &[u8]) -> &str {
    match str::from_utf8(input) {
        Ok(valid) => valid,
        Err(error) => unsafe { str::from_utf8_unchecked(&input[..error.valid_up_to()]) },
    }
}

pub fn with_description<F, T>(err: Errno, callback: F) -> T
where
    F: FnOnce(Result<&str, Errno>) -> T,
{
    let mut buf = [0u8; 1024];
    let c_str = unsafe {
        let rc = strerror_r(err.0, buf.as_mut_ptr() as *mut _, buf.len() as size_t);
        if rc != 0 {
            let fm_err = Errno(rc);
            if fm_err != Errno(libc::ERANGE) {
                return callback(Err(fm_err));
            }
        }
        let c_str_len = strlen(buf.as_ptr() as *const _);
        &buf[..c_str_len]
    };
    callback(Ok(from_utf8_lossy(c_str)))
}

pub const STRERROR_NAME: &str = "strerror_r";

pub fn errno() -> Errno {
    unsafe { Errno(*__errno_location()) }
}

pub fn set_errno(Errno(new_errno): Errno) {
    unsafe {
        *__errno_location() = new_errno;
    }
}

extern "C" {
    fn __errno_location() -> *mut c_int;
}
