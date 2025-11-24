//! linux_raw syscalls supporting `rustix::pipe`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::conv::{c_int, c_uint, opt_mut, pass_usize, ret, ret_usize, slice};
use crate::backend::{c, MAX_IOV};
use crate::fd::{BorrowedFd, OwnedFd};
use crate::io;
use crate::pipe::{IoSliceRaw, PipeFlags, SpliceFlags};
use core::cmp;
use core::mem::MaybeUninit;
use linux_raw_sys::general::{F_GETPIPE_SZ, F_SETPIPE_SZ};

#[inline]
pub(crate) fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    // aarch64 and risc64 omit `__NR_pipe`. On mips, `__NR_pipe` uses a special
    // calling convention, but using it is not worth complicating our syscall
    // wrapping infrastructure at this time.
    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "riscv64",
    ))]
    {
        pipe_with(PipeFlags::empty())
    }
    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "riscv64",
    )))]
    unsafe {
        let mut result = MaybeUninit::<[OwnedFd; 2]>::uninit();
        ret(syscall!(__NR_pipe, &mut result))?;
        let [p0, p1] = result.assume_init();
        Ok((p0, p1))
    }
}

#[inline]
pub(crate) fn pipe_with(flags: PipeFlags) -> io::Result<(OwnedFd, OwnedFd)> {
    unsafe {
        let mut result = MaybeUninit::<[OwnedFd; 2]>::uninit();
        ret(syscall!(__NR_pipe2, &mut result, flags))?;
        let [p0, p1] = result.assume_init();
        Ok((p0, p1))
    }
}

#[inline]
pub(crate) fn splice(
    fd_in: BorrowedFd<'_>,
    off_in: Option<&mut u64>,
    fd_out: BorrowedFd<'_>,
    off_out: Option<&mut u64>,
    len: usize,
    flags: SpliceFlags,
) -> io::Result<usize> {
    unsafe {
        ret_usize(syscall!(
            __NR_splice,
            fd_in,
            opt_mut(off_in),
            fd_out,
            opt_mut(off_out),
            pass_usize(len),
            flags
        ))
    }
}

#[inline]
pub(crate) unsafe fn vmsplice(
    fd: BorrowedFd<'_>,
    bufs: &[IoSliceRaw<'_>],
    flags: SpliceFlags,
) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), MAX_IOV)]);
    ret_usize(syscall!(__NR_vmsplice, fd, bufs_addr, bufs_len, flags))
}

#[inline]
pub(crate) fn tee(
    fd_in: BorrowedFd<'_>,
    fd_out: BorrowedFd<'_>,
    len: usize,
    flags: SpliceFlags,
) -> io::Result<usize> {
    unsafe { ret_usize(syscall!(__NR_tee, fd_in, fd_out, pass_usize(len), flags)) }
}

#[inline]
pub(crate) fn fcntl_getpipe_size(fd: BorrowedFd<'_>) -> io::Result<usize> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_usize(syscall_readonly!(__NR_fcntl64, fd, c_uint(F_GETPIPE_SZ)))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall_readonly!(__NR_fcntl, fd, c_uint(F_GETPIPE_SZ)))
    }
}

#[inline]
pub(crate) fn fcntl_setpipe_size(fd: BorrowedFd<'_>, size: usize) -> io::Result<usize> {
    let size: c::c_int = size.try_into().map_err(|_| io::Errno::PERM)?;

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_fcntl64,
            fd,
            c_uint(F_SETPIPE_SZ),
            c_int(size)
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_fcntl,
            fd,
            c_uint(F_SETPIPE_SZ),
            c_int(size)
        ))
    }
}
