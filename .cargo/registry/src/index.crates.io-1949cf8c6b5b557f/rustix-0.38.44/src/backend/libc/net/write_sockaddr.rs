//! The BSD sockets API requires us to read the `ss_family` field before we can
//! interpret the rest of a `sockaddr` produced by the kernel.

use super::addr::SocketAddrStorage;
#[cfg(unix)]
use super::addr::SocketAddrUnix;
use super::ext::{in6_addr_new, in_addr_new, sockaddr_in6_new};
use crate::backend::c;
#[cfg(target_os = "linux")]
use crate::net::xdp::SocketAddrXdp;
use crate::net::{SocketAddrAny, SocketAddrV4, SocketAddrV6};
use core::mem::size_of;

pub(crate) unsafe fn write_sockaddr(
    addr: &SocketAddrAny,
    storage: *mut SocketAddrStorage,
) -> usize {
    match addr {
        SocketAddrAny::V4(v4) => write_sockaddr_v4(v4, storage),
        SocketAddrAny::V6(v6) => write_sockaddr_v6(v6, storage),
        #[cfg(unix)]
        SocketAddrAny::Unix(unix) => write_sockaddr_unix(unix, storage),
        #[cfg(target_os = "linux")]
        SocketAddrAny::Xdp(xdp) => write_sockaddr_xdp(xdp, storage),
    }
}

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
        sin_len: size_of::<c::sockaddr_in>() as _,
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

unsafe fn write_sockaddr_v4(v4: &SocketAddrV4, storage: *mut SocketAddrStorage) -> usize {
    let encoded = encode_sockaddr_v4(v4);
    core::ptr::write(storage.cast(), encoded);
    size_of::<c::sockaddr_in>()
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
            size_of::<c::sockaddr_in6>() as _,
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

unsafe fn write_sockaddr_v6(v6: &SocketAddrV6, storage: *mut SocketAddrStorage) -> usize {
    let encoded = encode_sockaddr_v6(v6);
    core::ptr::write(storage.cast(), encoded);
    size_of::<c::sockaddr_in6>()
}

#[cfg(unix)]
unsafe fn write_sockaddr_unix(unix: &SocketAddrUnix, storage: *mut SocketAddrStorage) -> usize {
    core::ptr::write(storage.cast(), unix.unix);
    unix.len()
}

#[cfg(target_os = "linux")]
pub(crate) fn encode_sockaddr_xdp(xdp: &SocketAddrXdp) -> c::sockaddr_xdp {
    c::sockaddr_xdp {
        sxdp_family: c::AF_XDP as _,
        sxdp_flags: xdp.flags().bits(),
        sxdp_ifindex: xdp.interface_index(),
        sxdp_queue_id: xdp.queue_id(),
        sxdp_shared_umem_fd: xdp.shared_umem_fd(),
    }
}

#[cfg(target_os = "linux")]
unsafe fn write_sockaddr_xdp(xdp: &SocketAddrXdp, storage: *mut SocketAddrStorage) -> usize {
    let encoded = encode_sockaddr_xdp(xdp);
    core::ptr::write(storage.cast(), encoded);
    size_of::<c::sockaddr_xdp>()
}
