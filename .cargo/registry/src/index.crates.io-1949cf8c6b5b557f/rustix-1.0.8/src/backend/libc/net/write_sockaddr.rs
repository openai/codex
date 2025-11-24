//! The BSD sockets API requires us to read the `sa_family` field before we can
//! interpret the rest of a `sockaddr` produced by the kernel.

use super::ext::{in6_addr_new, in_addr_new, sockaddr_in6_new};
use crate::backend::c;
use crate::net::{SocketAddrV4, SocketAddrV6};

pub(crate) fn encode_sockaddr_v4(v4: &SocketAddrV4) -> c::sockaddr_in {
    c::sockaddr_in {
        #[cfg(any(
            bsd,
            target_os = "aix",
            target_os = "espidf",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "nto",
            target_os = "vita",
        ))]
        sin_len: core::mem::size_of::<c::sockaddr_in>() as _,
        sin_family: c::AF_INET as _,
        sin_port: u16::to_be(v4.port()),
        sin_addr: in_addr_new(u32::from_ne_bytes(v4.ip().octets())),
        #[cfg(not(any(target_os = "haiku", target_os = "vita")))]
        sin_zero: [0; 8_usize],
        #[cfg(target_os = "haiku")]
        sin_zero: [0; 24_usize],
        #[cfg(target_os = "vita")]
        sin_zero: [0; 6_usize],
        #[cfg(target_os = "vita")]
        sin_vport: 0,
    }
}

pub(crate) fn encode_sockaddr_v6(v6: &SocketAddrV6) -> c::sockaddr_in6 {
    #[cfg(any(
        bsd,
        target_os = "aix",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "nto",
        target_os = "vita"
    ))]
    {
        sockaddr_in6_new(
            core::mem::size_of::<c::sockaddr_in6>() as _,
            c::AF_INET6 as _,
            u16::to_be(v6.port()),
            u32::to_be(v6.flowinfo()),
            in6_addr_new(v6.ip().octets()),
            v6.scope_id(),
        )
    }
    #[cfg(not(any(
        bsd,
        target_os = "aix",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "nto",
        target_os = "vita"
    )))]
    {
        sockaddr_in6_new(
            c::AF_INET6 as _,
            u16::to_be(v6.port()),
            u32::to_be(v6.flowinfo()),
            in6_addr_new(v6.ip().octets()),
            v6.scope_id(),
        )
    }
}
