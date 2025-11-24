//! linux_raw syscalls supporting `rustix::io_uring`.
//!
//! # Safety
//!
//! See the `rustix::backend::syscalls` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::conv::{by_mut, c_uint, pass_usize, ret_c_uint, ret_owned_fd};
use crate::fd::{BorrowedFd, OwnedFd};
use crate::io;
use crate::io_uring::{io_uring_params, IoringEnterFlags, IoringRegisterFlags, IoringRegisterOp};
use core::ffi::c_void;

#[inline]
pub(crate) fn io_uring_setup(entries: u32, params: &mut io_uring_params) -> io::Result<OwnedFd> {
    unsafe {
        ret_owned_fd(syscall!(
            __NR_io_uring_setup,
            c_uint(entries),
            by_mut(params)
        ))
    }
}

#[inline]
pub(crate) unsafe fn io_uring_register(
    fd: BorrowedFd<'_>,
    opcode: IoringRegisterOp,
    arg: *const c_void,
    nr_args: u32,
) -> io::Result<u32> {
    ret_c_uint(syscall_readonly!(
        __NR_io_uring_register,
        fd,
        c_uint(opcode as u32),
        arg,
        c_uint(nr_args)
    ))
}

#[inline]
pub(crate) unsafe fn io_uring_register_with(
    fd: BorrowedFd<'_>,
    opcode: IoringRegisterOp,
    flags: IoringRegisterFlags,
    arg: *const c_void,
    nr_args: u32,
) -> io::Result<u32> {
    ret_c_uint(syscall_readonly!(
        __NR_io_uring_register,
        fd,
        c_uint((opcode as u32) | bitflags_bits!(flags)),
        arg,
        c_uint(nr_args)
    ))
}

#[inline]
pub(crate) unsafe fn io_uring_enter(
    fd: BorrowedFd<'_>,
    to_submit: u32,
    min_complete: u32,
    flags: IoringEnterFlags,
    arg: *const c_void,
    size: usize,
) -> io::Result<u32> {
    // This is not `_readonly` because `io_uring_enter` waits for I/O to
    // complete, and I/O could involve writing to memory buffers, which
    // could be a side effect depended on by the caller.
    ret_c_uint(syscall!(
        __NR_io_uring_enter,
        fd,
        c_uint(to_submit),
        c_uint(min_complete),
        flags,
        arg,
        pass_usize(size)
    ))
}
