//! linux_raw syscalls supporting `rustix::system`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use super::types::RawUname;
use crate::backend::c;
use crate::backend::conv::{c_int, ret, ret_infallible, slice};
use crate::fd::BorrowedFd;
use crate::ffi::CStr;
use crate::io;
use crate::system::{RebootCommand, Sysinfo};
use core::mem::MaybeUninit;

#[inline]
pub(crate) fn uname() -> RawUname {
    let mut uname = MaybeUninit::<RawUname>::uninit();
    unsafe {
        ret_infallible(syscall!(__NR_uname, &mut uname));
        uname.assume_init()
    }
}

#[inline]
pub(crate) fn sysinfo() -> Sysinfo {
    let mut info = MaybeUninit::<Sysinfo>::uninit();
    unsafe {
        ret_infallible(syscall!(__NR_sysinfo, &mut info));
        info.assume_init()
    }
}

#[inline]
pub(crate) fn sethostname(name: &[u8]) -> io::Result<()> {
    let (ptr, len) = slice(name);
    unsafe { ret(syscall_readonly!(__NR_sethostname, ptr, len)) }
}

#[inline]
pub(crate) fn setdomainname(name: &[u8]) -> io::Result<()> {
    let (ptr, len) = slice(name);
    unsafe { ret(syscall_readonly!(__NR_setdomainname, ptr, len)) }
}

#[inline]
pub(crate) fn reboot(cmd: RebootCommand) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_reboot,
            c_int(c::LINUX_REBOOT_MAGIC1),
            c_int(c::LINUX_REBOOT_MAGIC2),
            c_int(cmd as i32)
        ))
    }
}

#[inline]
pub(crate) fn init_module(image: &[u8], param_values: &CStr) -> io::Result<()> {
    let (image, len) = slice(image);
    unsafe {
        ret(syscall_readonly!(
            __NR_init_module,
            image,
            len,
            param_values
        ))
    }
}

#[inline]
pub(crate) fn finit_module(
    fd: BorrowedFd<'_>,
    param_values: &CStr,
    flags: c::c_int,
) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_finit_module,
            fd,
            param_values,
            c_int(flags)
        ))
    }
}

#[inline]
pub(crate) fn delete_module(name: &CStr, flags: c::c_int) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_delete_module, name, c_int(flags))) }
}
