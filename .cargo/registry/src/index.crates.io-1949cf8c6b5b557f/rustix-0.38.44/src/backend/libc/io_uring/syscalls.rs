//! libc syscalls supporting `rustix::io_uring`.

use crate::backend::c;
use crate::backend::conv::{borrowed_fd, ret_owned_fd, ret_u32};
use crate::fd::{BorrowedFd, OwnedFd};
use crate::io;
use crate::io_uring::{io_uring_params, IoringEnterFlags, IoringRegisterFlags, IoringRegisterOp};

#[inline]
pub(crate) fn io_uring_setup(entries: u32, params: &mut io_uring_params) -> io::Result<OwnedFd> {
    syscall! {
        fn io_uring_setup(
            entries: u32,
            params: *mut io_uring_params
        ) via SYS_io_uring_setup -> c::c_int
    }
    unsafe { ret_owned_fd(io_uring_setup(entries, params)) }
}

#[inline]
pub(crate) unsafe fn io_uring_register(
    fd: BorrowedFd<'_>,
    opcode: IoringRegisterOp,
    arg: *const c::c_void,
    nr_args: u32,
) -> io::Result<u32> {
    syscall! {
        fn io_uring_register(
            fd: c::c_uint,
            opcode: c::c_uint,
            arg: *const c::c_void,
            nr_args: c::c_uint
        ) via SYS_io_uring_register -> c::c_int
    }
    ret_u32(io_uring_register(
        borrowed_fd(fd) as _,
        opcode as u32,
        arg,
        nr_args,
    ))
}

#[inline]
pub(crate) unsafe fn io_uring_register_with(
    fd: BorrowedFd<'_>,
    opcode: IoringRegisterOp,
    flags: IoringRegisterFlags,
    arg: *const c::c_void,
    nr_args: u32,
) -> io::Result<u32> {
    syscall! {
        fn io_uring_register(
            fd: c::c_uint,
            opcode: c::c_uint,
            arg: *const c::c_void,
            nr_args: c::c_uint
        ) via SYS_io_uring_register -> c::c_int
    }
    ret_u32(io_uring_register(
        borrowed_fd(fd) as _,
        (opcode as u32) | bitflags_bits!(flags),
        arg,
        nr_args,
    ))
}

#[inline]
pub(crate) unsafe fn io_uring_enter(
    fd: BorrowedFd<'_>,
    to_submit: u32,
    min_complete: u32,
    flags: IoringEnterFlags,
    arg: *const c::c_void,
    size: usize,
) -> io::Result<u32> {
    syscall! {
        fn io_uring_enter2(
            fd: c::c_uint,
            to_submit: c::c_uint,
            min_complete: c::c_uint,
            flags: c::c_uint,
            arg: *const c::c_void,
            size: usize
        ) via SYS_io_uring_enter -> c::c_int
    }
    ret_u32(io_uring_enter2(
        borrowed_fd(fd) as _,
        to_submit,
        min_complete,
        bitflags_bits!(flags),
        arg,
        size,
    ))
}
