//! The `cwd` function, representing the current working directory.
//!
//! # Safety
//!
//! This file uses `AT_FDCWD`, which is a raw file descriptor, but which is
//! always valid.

#![allow(unsafe_code)]

use crate::backend;
use backend::c;
use backend::fd::{BorrowedFd, RawFd};

/// Return the value of [`CWD`].
#[deprecated(note = "Use `CWD` in place of `cwd()`.")]
pub const fn cwd() -> BorrowedFd<'static> {
    let at_fdcwd = c::AT_FDCWD as RawFd;

    // SAFETY: `AT_FDCWD` is a reserved value that is never dynamically
    // allocated, so it'll remain valid for the duration of `'static`.
    unsafe { BorrowedFd::<'static>::borrow_raw(at_fdcwd) }
}
