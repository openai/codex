//! The BSD sockets API requires us to read the `sa_family` field before we can
//! interpret the rest of a `sockaddr` produced by the kernel.
#![allow(unsafe_code)]

use crate::backend::c;
use crate::net::{SocketAddrV4, SocketAddrV6};

pub(crate) fn encode_sockaddr_v4(v4: &SocketAddrV4) -> c::sockaddr_in {
    c::sockaddr_in {
        sin_family: c::AF_INET as _,
        sin_port: u16::to_be(v4.port()),
        sin_addr: c::in_addr {
            s_addr: u32::from_ne_bytes(v4.ip().octets()),
        },
        __pad: [0_u8; 8],
    }
}

pub(crate) fn encode_sockaddr_v6(v6: &SocketAddrV6) -> c::sockaddr_in6 {
    c::sockaddr_in6 {
        sin6_family: c::AF_INET6 as _,
        sin6_port: u16::to_be(v6.port()),
        sin6_flowinfo: u32::to_be(v6.flowinfo()),
        sin6_addr: c::in6_addr {
            in6_u: linux_raw_sys::net::in6_addr__bindgen_ty_1 {
                u6_addr8: v6.ip().octets(),
            },
        },
        sin6_scope_id: v6.scope_id(),
    }
}
