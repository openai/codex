//! Implementation for Linux / Android without `/dev/urandom` fallback
use crate::{util_libc, Error};
use core::mem::MaybeUninit;

pub fn getrandom_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    util_libc::sys_fill_exact(dest, util_libc::getrandom_syscall)
}
