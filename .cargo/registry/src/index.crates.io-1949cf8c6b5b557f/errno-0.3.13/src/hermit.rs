//! Implementation of `errno` functionality for RustyHermit.
//!
//! Currently, the error handling in RustyHermit isn't clearly
//! defined. At the current stage of RustyHermit, only a placeholder
//! is provided to be compatible to the classical errno interface.

// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::Errno;

pub fn with_description<F, T>(_err: Errno, callback: F) -> T
where
    F: FnOnce(Result<&str, Errno>) -> T,
{
    callback(Ok("unknown error"))
}

pub const STRERROR_NAME: &str = "strerror_r";

pub fn errno() -> Errno {
    Errno(0)
}

pub fn set_errno(_: Errno) {}
