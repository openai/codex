//! Libc call arguments and return values are often things like `c_int`,
//! `c_uint`, or libc-specific pointer types. This module provides functions
//! for converting between rustix's types and libc types.

use super::c;
#[cfg(all(feature = "alloc", not(any(windows, target_os = "espidf"))))]
use super::fd::IntoRawFd;
use super::fd::{AsRawFd, BorrowedFd, FromRawFd, LibcFd, OwnedFd, RawFd};
#[cfg(not(windows))]
use crate::ffi::CStr;
use crate::io;

#[cfg(not(windows))]
#[inline]
pub(super) fn c_str(c: &CStr) -> *const c::c_char {
    c.as_ptr().cast()
}

#[cfg(not(any(windows, target_os = "espidf", target_os = "vita", target_os = "wasi")))]
#[inline]
pub(super) fn no_fd() -> LibcFd {
    -1
}

#[inline]
pub(super) fn borrowed_fd(fd: BorrowedFd<'_>) -> LibcFd {
    fd.as_raw_fd() as LibcFd
}

#[cfg(all(
    feature = "alloc",
    not(any(windows, target_os = "espidf", target_os = "redox"))
))]
#[inline]
pub(super) fn owned_fd(fd: OwnedFd) -> LibcFd {
    fd.into_raw_fd() as LibcFd
}

#[inline]
pub(super) fn ret(raw: c::c_int) -> io::Result<()> {
    if raw == 0 {
        Ok(())
    } else {
        Err(io::Errno::last_os_error())
    }
}

#[cfg(apple)]
#[inline]
pub(super) fn nonnegative_ret(raw: c::c_int) -> io::Result<()> {
    if raw >= 0 {
        Ok(())
    } else {
        Err(io::Errno::last_os_error())
    }
}

#[cfg(not(any(windows, target_os = "wasi")))]
#[inline]
pub(super) unsafe fn ret_infallible(raw: c::c_int) {
    debug_assert_eq!(raw, 0, "unexpected error: {:?}", io::Errno::last_os_error());
}

#[inline]
pub(super) fn ret_c_int(raw: c::c_int) -> io::Result<c::c_int> {
    if raw == -1 {
        Err(io::Errno::last_os_error())
    } else {
        Ok(raw)
    }
}

#[cfg(any(
    linux_kernel,
    all(target_os = "illumos", feature = "event"),
    all(target_os = "redox", feature = "event")
))]
#[inline]
pub(super) fn ret_u32(raw: c::c_int) -> io::Result<u32> {
    if raw == -1 {
        Err(io::Errno::last_os_error())
    } else {
        Ok(raw as u32)
    }
}

#[inline]
pub(super) fn ret_usize(raw: c::ssize_t) -> io::Result<usize> {
    if raw == -1 {
        Err(io::Errno::last_os_error())
    } else {
        debug_assert!(raw >= 0);
        Ok(raw as usize)
    }
}

#[cfg(not(windows))]
#[cfg(feature = "fs")]
#[inline]
pub(super) fn ret_off_t(raw: c::off_t) -> io::Result<c::off_t> {
    if raw == -1 {
        Err(io::Errno::last_os_error())
    } else {
        Ok(raw)
    }
}

#[cfg(not(any(windows, target_os = "wasi")))]
#[inline]
pub(super) fn ret_pid_t(raw: c::pid_t) -> io::Result<c::pid_t> {
    if raw == -1 {
        Err(io::Errno::last_os_error())
    } else {
        Ok(raw)
    }
}

/// Convert a `c_int` returned from a libc function to an `OwnedFd`, if valid.
///
/// # Safety
///
/// The caller must ensure that this is the return value of a libc function
/// which returns an owned file descriptor.
#[inline]
pub(super) unsafe fn ret_owned_fd(raw: LibcFd) -> io::Result<OwnedFd> {
    if raw == !0 {
        Err(io::Errno::last_os_error())
    } else {
        Ok(OwnedFd::from_raw_fd(raw as RawFd))
    }
}

#[cfg(not(any(windows, target_os = "wasi")))]
#[inline]
pub(super) fn ret_discarded_fd(raw: LibcFd) -> io::Result<()> {
    if raw == !0 {
        Err(io::Errno::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(all(feature = "alloc", not(any(windows, target_os = "wasi"))))]
#[inline]
pub(super) fn ret_discarded_char_ptr(raw: *mut c::c_char) -> io::Result<()> {
    if raw.is_null() {
        Err(io::Errno::last_os_error())
    } else {
        Ok(())
    }
}

/// Convert the buffer-length argument value of a `send` or `recv` call.
#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
#[inline]
pub(super) fn send_recv_len(len: usize) -> usize {
    len
}

/// Convert the buffer-length argument value of a `send` or `recv` call.
#[cfg(windows)]
#[inline]
pub(super) fn send_recv_len(len: usize) -> i32 {
    // On Windows, the length argument has type `i32`; saturate the length,
    // since `send` and `recv` are allowed to send and recv less data than
    // requested.
    len.try_into().unwrap_or(i32::MAX)
}

/// Convert the return value of a `send` or `recv` call.
#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
#[inline]
pub(super) fn ret_send_recv(len: isize) -> io::Result<usize> {
    ret_usize(len)
}

/// Convert the return value of a `send` or `recv` call.
#[cfg(windows)]
#[inline]
pub(super) fn ret_send_recv(len: i32) -> io::Result<usize> {
    ret_usize(len as isize)
}

/// Convert the value to the `msg_iovlen` field of a `msghdr` struct.
#[cfg(all(
    not(any(windows, target_os = "espidf", target_os = "redox", target_os = "wasi")),
    any(
        target_os = "android",
        all(
            target_os = "linux",
            not(target_env = "musl"),
            not(all(target_env = "uclibc", any(target_arch = "arm", target_arch = "mips")))
        )
    )
))]
#[inline]
pub(super) fn msg_iov_len(len: usize) -> c::size_t {
    len
}

/// Convert the value to the `msg_iovlen` field of a `msghdr` struct.
#[cfg(all(
    not(any(
        windows,
        target_os = "espidf",
        target_os = "redox",
        target_os = "vita",
        target_os = "wasi"
    )),
    not(any(
        target_os = "android",
        all(
            target_os = "linux",
            not(target_env = "musl"),
            not(all(target_env = "uclibc", any(target_arch = "arm", target_arch = "mips")))
        )
    ))
))]
#[inline]
pub(crate) fn msg_iov_len(len: usize) -> c::c_int {
    len.try_into().unwrap_or(c::c_int::MAX)
}

/// Convert the value to a `socklen_t`.
#[cfg(any(
    bsd,
    solarish,
    target_env = "musl",
    target_os = "aix",
    target_os = "emscripten",
    target_os = "fuchsia",
    target_os = "haiku",
    target_os = "hurd",
    target_os = "nto",
))]
#[inline]
pub(crate) fn msg_control_len(len: usize) -> c::socklen_t {
    len.try_into().unwrap_or(c::socklen_t::MAX)
}

/// Convert the value to a `size_t`.
#[cfg(not(any(
    bsd,
    solarish,
    windows,
    target_env = "musl",
    target_os = "aix",
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "haiku",
    target_os = "hurd",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
#[inline]
pub(crate) fn msg_control_len(len: usize) -> c::size_t {
    len
}
