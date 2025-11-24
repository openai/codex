//! Utilities for dealing with message headers.
//!
//! These take closures rather than returning a `c::msghdr` directly because
//! the message headers may reference stack-local data.

use crate::backend::c;

use crate::io::{self, IoSlice, IoSliceMut};
use crate::net::addr::SocketAddrArg;
use crate::net::{RecvAncillaryBuffer, SendAncillaryBuffer, SocketAddrBuf};

use core::mem::zeroed;

/// Convert the value to the `msg_iovlen` field of a `msghdr` struct.
#[cfg(all(
    not(any(windows, target_os = "espidf", target_os = "wasi")),
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
fn msg_iov_len(len: usize) -> c::size_t {
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
fn msg_iov_len(len: usize) -> c::c_int {
    len.try_into().unwrap_or(c::c_int::MAX)
}

/// Convert the value to a `socklen_t`.
#[cfg(any(
    bsd,
    solarish,
    target_env = "musl",
    target_os = "aix",
    target_os = "cygwin",
    target_os = "emscripten",
    target_os = "fuchsia",
    target_os = "haiku",
    target_os = "hurd",
    target_os = "nto",
))]
#[inline]
fn msg_control_len(len: usize) -> c::socklen_t {
    len.try_into().unwrap_or(c::socklen_t::MAX)
}

/// Convert the value to a `size_t`.
#[cfg(not(any(
    bsd,
    solarish,
    windows,
    target_env = "musl",
    target_os = "aix",
    target_os = "cygwin",
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
fn msg_control_len(len: usize) -> c::size_t {
    len
}

/// Create a message header intended to receive a datagram.
///
/// # Safety
///
/// If `f` dereferences the pointers in the `msghdr`, it must do so only within
/// the bounds indicated by the associated lengths in the `msghdr`.
///
/// And, if `f` returns `Ok`, it must have updated the `msg_controllen` field
/// of the `msghdr` to indicate how many bytes it initialized.
pub(crate) unsafe fn with_recv_msghdr<R>(
    name: &mut SocketAddrBuf,
    iov: &mut [IoSliceMut<'_>],
    control: &mut RecvAncillaryBuffer<'_>,
    f: impl FnOnce(&mut c::msghdr) -> io::Result<R>,
) -> io::Result<R> {
    control.clear();

    let mut msghdr = zero_msghdr();
    msghdr.msg_name = name.storage.as_mut_ptr().cast();
    msghdr.msg_namelen = name.len;
    msghdr.msg_iov = iov.as_mut_ptr().cast();
    msghdr.msg_iovlen = msg_iov_len(iov.len());
    msghdr.msg_control = control.as_control_ptr().cast();
    msghdr.msg_controllen = msg_control_len(control.control_len());

    let res = f(&mut msghdr);

    // Reset the control length.
    if res.is_ok() {
        // SAFETY: `f` returned `Ok`, so our safety condition requires `f` to
        // have initialized `msg_controllen` bytes.
        control.set_control_len(msghdr.msg_controllen as usize);
    }

    name.len = msghdr.msg_namelen;

    res
}

/// Create a message header intended to send without an address.
///
/// The returned `msghdr` will contain raw pointers to the memory
/// referenced by `iov` and `control`.
pub(crate) fn noaddr_msghdr(
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
) -> c::msghdr {
    let mut h = zero_msghdr();
    h.msg_iov = iov.as_ptr() as _;
    h.msg_iovlen = msg_iov_len(iov.len());
    h.msg_control = control.as_control_ptr().cast();
    h.msg_controllen = msg_control_len(control.control_len());
    h
}

/// Create a message header intended to send with the specified address.
///
/// This creates a `c::msghdr` and calls a function `f` on it. The `msghdr`'s
/// raw pointers may point to temporaries, so this function should avoid
/// storing the pointers anywhere that would outlive the function call.
///
/// # Safety
///
/// If `f` dereferences the pointers in the `msghdr`, it must do so only within
/// the bounds indicated by the associated lengths in the `msghdr`.
pub(crate) unsafe fn with_msghdr<R>(
    addr: &impl SocketAddrArg,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    f: impl FnOnce(&c::msghdr) -> R,
) -> R {
    addr.with_sockaddr(|addr_ptr, addr_len| {
        let mut h = zero_msghdr();
        h.msg_name = addr_ptr as *mut _;
        h.msg_namelen = bitcast!(addr_len);
        h.msg_iov = iov.as_ptr() as _;
        h.msg_iovlen = msg_iov_len(iov.len());
        h.msg_control = control.as_control_ptr().cast();
        h.msg_controllen = msg_control_len(control.control_len());
        // Pass a reference to the `c::msghdr` instead of passing it by value
        // because it may contain pointers to temporary objects that won't
        // live beyond the call to `with_sockaddr`.
        f(&h)
    })
}

/// Create a zero-initialized message header struct value.
#[cfg(all(unix, not(target_os = "redox")))]
pub(crate) fn zero_msghdr() -> c::msghdr {
    // SAFETY: We can't initialize all the fields by value because on some
    // platforms the `msghdr` struct in the libc crate contains private padding
    // fields. But it is still a C type that's meant to be zero-initializable.
    unsafe { zeroed() }
}
