//! Utilities for dealing with message headers.
//!
//! These take closures rather than returning a `c::msghdr` directly because
//! the message headers may reference stack-local data.

#![allow(unsafe_code)]

use crate::backend::c;
#[cfg(target_os = "linux")]
use crate::backend::net::write_sockaddr::encode_sockaddr_xdp;
use crate::backend::net::write_sockaddr::{encode_sockaddr_v4, encode_sockaddr_v6};

use crate::io::{self, IoSlice, IoSliceMut};
#[cfg(target_os = "linux")]
use crate::net::xdp::SocketAddrXdp;
use crate::net::{RecvAncillaryBuffer, SendAncillaryBuffer, SocketAddrV4, SocketAddrV6};
use crate::utils::as_ptr;

use core::mem::{size_of, MaybeUninit};
use core::ptr::null_mut;

fn msg_iov_len(len: usize) -> c::size_t {
    // This cast cannot overflow.
    len as c::size_t
}

pub(crate) fn msg_control_len(len: usize) -> c::size_t {
    // Same as above.
    len as c::size_t
}

/// Create a message header intended to receive a datagram.
pub(crate) fn with_recv_msghdr<R>(
    name: &mut MaybeUninit<c::sockaddr_storage>,
    iov: &mut [IoSliceMut<'_>],
    control: &mut RecvAncillaryBuffer<'_>,
    f: impl FnOnce(&mut c::msghdr) -> io::Result<R>,
) -> io::Result<R> {
    control.clear();

    let namelen = size_of::<c::sockaddr_storage>() as c::c_int;
    let mut msghdr = c::msghdr {
        msg_name: name.as_mut_ptr().cast(),
        msg_namelen: namelen,
        msg_iov: iov.as_mut_ptr().cast(),
        msg_iovlen: msg_iov_len(iov.len()),
        msg_control: control.as_control_ptr().cast(),
        msg_controllen: msg_control_len(control.control_len()),
        msg_flags: 0,
    };

    let res = f(&mut msghdr);

    // Reset the control length.
    if res.is_ok() {
        unsafe {
            control.set_control_len(msghdr.msg_controllen.try_into().unwrap_or(usize::MAX));
        }
    }

    res
}

/// Create a message header intended to send without an address.
pub(crate) fn with_noaddr_msghdr<R>(
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    f: impl FnOnce(c::msghdr) -> R,
) -> R {
    f(c::msghdr {
        msg_name: null_mut(),
        msg_namelen: 0,
        msg_iov: iov.as_ptr() as _,
        msg_iovlen: msg_iov_len(iov.len()),
        msg_control: control.as_control_ptr().cast(),
        msg_controllen: msg_control_len(control.control_len()),
        msg_flags: 0,
    })
}

/// Create a message header intended to send with an IPv4 address.
pub(crate) fn with_v4_msghdr<R>(
    addr: &SocketAddrV4,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    f: impl FnOnce(c::msghdr) -> R,
) -> R {
    let encoded = encode_sockaddr_v4(addr);

    f(c::msghdr {
        msg_name: as_ptr(&encoded) as _,
        msg_namelen: size_of::<SocketAddrV4>() as _,
        msg_iov: iov.as_ptr() as _,
        msg_iovlen: msg_iov_len(iov.len()),
        msg_control: control.as_control_ptr().cast(),
        msg_controllen: msg_control_len(control.control_len()),
        msg_flags: 0,
    })
}

/// Create a message header intended to send with an IPv6 address.
pub(crate) fn with_v6_msghdr<R>(
    addr: &SocketAddrV6,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    f: impl FnOnce(c::msghdr) -> R,
) -> R {
    let encoded = encode_sockaddr_v6(addr);

    f(c::msghdr {
        msg_name: as_ptr(&encoded) as _,
        msg_namelen: size_of::<SocketAddrV6>() as _,
        msg_iov: iov.as_ptr() as _,
        msg_iovlen: msg_iov_len(iov.len()),
        msg_control: control.as_control_ptr().cast(),
        msg_controllen: msg_control_len(control.control_len()),
        msg_flags: 0,
    })
}

/// Create a message header intended to send with a Unix address.
pub(crate) fn with_unix_msghdr<R>(
    addr: &crate::net::SocketAddrUnix,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    f: impl FnOnce(c::msghdr) -> R,
) -> R {
    f(c::msghdr {
        msg_name: as_ptr(&addr.unix) as _,
        msg_namelen: addr.addr_len() as _,
        msg_iov: iov.as_ptr() as _,
        msg_iovlen: msg_iov_len(iov.len()),
        msg_control: control.as_control_ptr().cast(),
        msg_controllen: msg_control_len(control.control_len()),
        msg_flags: 0,
    })
}

/// Create a message header intended to send with an XDP address.
#[cfg(target_os = "linux")]
pub(crate) fn with_xdp_msghdr<R>(
    addr: &SocketAddrXdp,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    f: impl FnOnce(c::msghdr) -> R,
) -> R {
    let encoded = encode_sockaddr_xdp(addr);

    f(c::msghdr {
        msg_name: as_ptr(&encoded) as _,
        msg_namelen: size_of::<SocketAddrXdp>() as _,
        msg_iov: iov.as_ptr() as _,
        msg_iovlen: msg_iov_len(iov.len()),
        msg_control: control.as_control_ptr().cast(),
        msg_controllen: msg_control_len(control.control_len()),
        msg_flags: 0,
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
