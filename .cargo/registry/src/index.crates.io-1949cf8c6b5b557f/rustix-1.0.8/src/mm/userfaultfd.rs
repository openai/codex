//! The Linux `userfaultfd` API.
//!
//! # Safety
//!
//! Calling `userfaultfd` is safe, but the returned file descriptor lets users
//! observe and manipulate process memory in magical ways.
#![allow(unsafe_code)]

use crate::fd::OwnedFd;
use crate::{backend, io};

pub use backend::mm::types::UserfaultfdFlags;

/// `userfaultfd(flags)`—Create userspace page-fault handler.
///
/// # Safety
///
/// The call itself is safe, but the returned file descriptor lets users
/// observe and manipulate process memory in magical ways.
///
/// # References
///  - [Linux]
///  - [Linux userfaultfd]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/userfaultfd.2.html
/// [Linux userfaultfd]: https://www.kernel.org/doc/Documentation/vm/userfaultfd.txt
#[inline]
pub unsafe fn userfaultfd(flags: UserfaultfdFlags) -> io::Result<OwnedFd> {
    backend::mm::syscalls::userfaultfd(flags)
}
