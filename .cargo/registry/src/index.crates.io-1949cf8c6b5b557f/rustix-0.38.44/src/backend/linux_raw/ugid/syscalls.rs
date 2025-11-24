//! linux_raw syscalls for UIDs and GIDs
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::c;
use crate::backend::conv::ret_usize_infallible;
use crate::ugid::{Gid, Uid};

#[inline]
pub(crate) fn getuid() -> Uid {
    #[cfg(any(target_arch = "arm", target_arch = "sparc", target_arch = "x86"))]
    unsafe {
        let uid = ret_usize_infallible(syscall_readonly!(__NR_getuid32)) as c::uid_t;
        Uid::from_raw(uid)
    }
    #[cfg(not(any(target_arch = "arm", target_arch = "sparc", target_arch = "x86")))]
    unsafe {
        let uid = ret_usize_infallible(syscall_readonly!(__NR_getuid)) as c::uid_t;
        Uid::from_raw(uid)
    }
}

#[inline]
pub(crate) fn geteuid() -> Uid {
    #[cfg(any(target_arch = "arm", target_arch = "sparc", target_arch = "x86"))]
    unsafe {
        let uid = ret_usize_infallible(syscall_readonly!(__NR_geteuid32)) as c::uid_t;
        Uid::from_raw(uid)
    }
    #[cfg(not(any(target_arch = "arm", target_arch = "sparc", target_arch = "x86")))]
    unsafe {
        let uid = ret_usize_infallible(syscall_readonly!(__NR_geteuid)) as c::uid_t;
        Uid::from_raw(uid)
    }
}

#[inline]
pub(crate) fn getgid() -> Gid {
    #[cfg(any(target_arch = "arm", target_arch = "sparc", target_arch = "x86"))]
    unsafe {
        let gid = ret_usize_infallible(syscall_readonly!(__NR_getgid32)) as c::gid_t;
        Gid::from_raw(gid)
    }
    #[cfg(not(any(target_arch = "arm", target_arch = "sparc", target_arch = "x86")))]
    unsafe {
        let gid = ret_usize_infallible(syscall_readonly!(__NR_getgid)) as c::gid_t;
        Gid::from_raw(gid)
    }
}

#[inline]
pub(crate) fn getegid() -> Gid {
    #[cfg(any(target_arch = "arm", target_arch = "sparc", target_arch = "x86"))]
    unsafe {
        let gid = ret_usize_infallible(syscall_readonly!(__NR_getegid32)) as c::gid_t;
        Gid::from_raw(gid)
    }
    #[cfg(not(any(target_arch = "arm", target_arch = "sparc", target_arch = "x86")))]
    unsafe {
        let gid = ret_usize_infallible(syscall_readonly!(__NR_getegid)) as c::gid_t;
        Gid::from_raw(gid)
    }
}
