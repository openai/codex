//! Utilities for dealing with message headers.
//!
//! These take closures rather than returning a `c::msghdr` directly because
//! the message headers may reference stack-local data.

#![allow(unsafe_code)]

use crate::backend::c;

use crate::io::{self, IoSlice, IoSliceMut};
use crate::net::addr::SocketAddrArg;
use crate::net::{RecvAncillaryBuffer, SendAncillaryBuffer, SocketAddrBuf};

use core::ptr::null_mut;

fn msg_iov_len(len: usize) -> c::size_t {
    // This cast cannot overflow.
    len as c::size_t
}

fn msg_control_len(len: usize) -> c::size_t {
    // Same as above.
    len as c::size_t
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

    let mut msghdr = c::msghdr {
        msg_name: name.storage.as_mut_ptr().cast(),
        msg_namelen: bitcast!(name.len),
        msg_iov: iov.as_mut_ptr().cast(),
        msg_iovlen: msg_iov_len(iov.len()),
        msg_control: control.as_control_ptr().cast(),
        msg_controllen: msg_control_len(control.control_len()),
        msg_flags: 0,
    };

    let res = f(&mut msghdr);

    // Reset the control length.
    if res.is_ok() {
        // SAFETY: `f` returned `Ok`, so our safety condition requires `f` to
        // have initialized `msg_controllen` bytes.
        control.set_control_len(msghdr.msg_controllen as usize);
    }

    name.len = bitcast!(msghdr.msg_namelen);

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
    c::msghdr {
        msg_name: null_mut(),
        msg_namelen: 0,
        msg_iov: iov.as_ptr() as _,
        msg_iovlen: msg_iov_len(iov.len()),
        msg_control: control.as_control_ptr().cast(),
        msg_controllen: msg_control_len(control.control_len()),
        msg_flags: 0,
    }
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
        // Pass a reference to the `c::msghdr` instead of passing it by value
        // because it may contain pointers to temporary objects that won't live
        // beyond the call to `with_sockaddr`.
        let mut msghdr = noaddr_msghdr(iov, control);
        msghdr.msg_name = addr_ptr as _;
        msghdr.msg_namelen = bitcast!(addr_len);

        f(&msghdr)
    })
}

/// Create a zero-initialized message header struct value.
pub(crate) fn zero_msghdr() -> c::msghdr {
    c::msghdr {
        msg_name: null_mut(),
        msg_namelen: 0,
        msg_iov: null_mut(),
        msg_iovlen: 0,
        msg_control: null_mut(),
        msg_controllen: 0,
        msg_flags: 0,
    }
}
