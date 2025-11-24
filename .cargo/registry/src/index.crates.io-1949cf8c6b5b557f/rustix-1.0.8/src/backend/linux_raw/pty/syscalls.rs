//! linux_raw syscalls supporting `rustix::pty`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::conv::{by_ref, c_uint, ret};
use crate::fd::BorrowedFd;
use crate::io;
use linux_raw_sys::ioctl::TIOCSPTLCK;
#[cfg(feature = "alloc")]
use {
    crate::backend::c, crate::ffi::CString, crate::path::DecInt, alloc::vec::Vec,
    core::mem::MaybeUninit, linux_raw_sys::ioctl::TIOCGPTN,
};

#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn ptsname(fd: BorrowedFd<'_>, mut buffer: Vec<u8>) -> io::Result<CString> {
    unsafe {
        let mut n = MaybeUninit::<c::c_int>::uninit();
        ret(syscall!(__NR_ioctl, fd, c_uint(TIOCGPTN), &mut n))?;

        buffer.clear();
        buffer.extend_from_slice(b"/dev/pts/");
        buffer.extend_from_slice(DecInt::new(n.assume_init()).as_bytes());
        buffer.push(b'\0');
        Ok(CString::from_vec_with_nul_unchecked(buffer))
    }
}

#[inline]
pub(crate) fn unlockpt(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_ioctl,
            fd,
            c_uint(TIOCSPTLCK),
            by_ref(&0)
        ))
    }
}
