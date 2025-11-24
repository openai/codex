//! linux_raw syscalls supporting `rustix::rand`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::conv::{pass_usize, ret_usize};
use crate::io;
use crate::rand::GetRandomFlags;

#[inline]
pub(crate) unsafe fn getrandom(
    buf: *mut u8,
    cap: usize,
    flags: GetRandomFlags,
) -> io::Result<usize> {
    ret_usize(syscall!(__NR_getrandom, buf, pass_usize(cap), flags))
}
