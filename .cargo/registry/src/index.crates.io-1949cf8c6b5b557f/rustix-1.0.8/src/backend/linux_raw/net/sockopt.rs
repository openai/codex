//! linux_raw syscalls supporting `rustix::net::sockopt`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::c;
use crate::backend::conv::{by_mut, c_uint, ret, socklen_t};
use crate::fd::BorrowedFd;
#[cfg(feature = "alloc")]
use crate::ffi::CStr;
use crate::io;
use crate::net::sockopt::Timeout;
#[cfg(target_os = "linux")]
use crate::net::xdp::{XdpMmapOffsets, XdpOptionsFlags, XdpRingOffset, XdpStatistics, XdpUmemReg};
use crate::net::{
    AddressFamily, Ipv4Addr, Ipv6Addr, Protocol, RawProtocol, SocketAddrBuf, SocketAddrV4,
    SocketAddrV6, SocketType, UCred,
};
#[cfg(feature = "alloc")]
use alloc::borrow::ToOwned as _;
#[cfg(feature = "alloc")]
use alloc::string::String;
use core::mem::{size_of, MaybeUninit};
use core::time::Duration;
use linux_raw_sys::general::{__kernel_old_timeval, __kernel_sock_timeval};
use linux_raw_sys::net::{IPV6_MTU, IPV6_MULTICAST_IF, IP_MTU, IP_MULTICAST_IF};
#[cfg(target_os = "linux")]
use linux_raw_sys::xdp::{xdp_mmap_offsets, xdp_statistics, xdp_statistics_v1};
#[cfg(target_arch = "x86")]
use {
    crate::backend::conv::{slice_just_addr, x86_sys},
    crate::backend::reg::{ArgReg, SocketArg},
    linux_raw_sys::net::{SYS_GETSOCKOPT, SYS_SETSOCKOPT},
};

#[inline]
fn getsockopt<T: Copy>(fd: BorrowedFd<'_>, level: u32, optname: u32) -> io::Result<T> {
    let mut optlen: c::socklen_t = size_of::<T>().try_into().unwrap();
    debug_assert!(
        optlen as usize >= size_of::<c::c_int>(),
        "Socket APIs don't ever use `bool` directly"
    );

    let mut value = MaybeUninit::<T>::uninit();
    getsockopt_raw(fd, level, optname, &mut value, &mut optlen)?;

    assert_eq!(
        optlen as usize,
        size_of::<T>(),
        "unexpected getsockopt size"
    );

    unsafe { Ok(value.assume_init()) }
}

#[inline]
fn getsockopt_raw<T>(
    fd: BorrowedFd<'_>,
    level: u32,
    optname: u32,
    value: &mut MaybeUninit<T>,
    optlen: &mut c::socklen_t,
) -> io::Result<()> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall!(
            __NR_getsockopt,
            fd,
            c_uint(level),
            c_uint(optname),
            value,
            by_mut(optlen)
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall!(
            __NR_socketcall,
            x86_sys(SYS_GETSOCKOPT),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                c_uint(level),
                c_uint(optname),
                value.into(),
                by_mut(optlen),
            ])
        ))
    }
}

#[inline]
fn setsockopt<T: Copy>(fd: BorrowedFd<'_>, level: u32, optname: u32, value: T) -> io::Result<()> {
    let optlen = size_of::<T>().try_into().unwrap();
    debug_assert!(
        optlen as usize >= size_of::<c::c_int>(),
        "Socket APIs don't ever use `bool` directly"
    );
    setsockopt_raw(fd, level, optname, &value, optlen)
}

#[inline]
fn setsockopt_raw<T>(
    fd: BorrowedFd<'_>,
    level: u32,
    optname: u32,
    ptr: *const T,
    optlen: c::socklen_t,
) -> io::Result<()> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_setsockopt,
            fd,
            c_uint(level),
            c_uint(optname),
            ptr,
            socklen_t(optlen)
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_SETSOCKOPT),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                c_uint(level),
                c_uint(optname),
                ptr.into(),
                socklen_t(optlen),
            ])
        ))
    }
}

#[inline]
pub(crate) fn socket_type(fd: BorrowedFd<'_>) -> io::Result<SocketType> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_TYPE)
}

#[inline]
pub(crate) fn set_socket_reuseaddr(fd: BorrowedFd<'_>, reuseaddr: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_REUSEADDR, from_bool(reuseaddr))
}

#[inline]
pub(crate) fn socket_reuseaddr(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_REUSEADDR).map(to_bool)
}

#[inline]
pub(crate) fn set_socket_broadcast(fd: BorrowedFd<'_>, broadcast: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_BROADCAST, from_bool(broadcast))
}

#[inline]
pub(crate) fn socket_broadcast(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_BROADCAST).map(to_bool)
}

#[inline]
pub(crate) fn set_socket_linger(fd: BorrowedFd<'_>, linger: Option<Duration>) -> io::Result<()> {
    // Convert `linger` to seconds, rounding up.
    let l_linger = if let Some(linger) = linger {
        duration_to_secs(linger)?
    } else {
        0
    };
    let linger = c::linger {
        l_onoff: c::c_int::from(linger.is_some()),
        l_linger,
    };
    setsockopt(fd, c::SOL_SOCKET, c::SO_LINGER, linger)
}

#[inline]
pub(crate) fn socket_linger(fd: BorrowedFd<'_>) -> io::Result<Option<Duration>> {
    let linger: c::linger = getsockopt(fd, c::SOL_SOCKET, c::SO_LINGER)?;
    Ok((linger.l_onoff != 0).then(|| Duration::from_secs(linger.l_linger as u64)))
}

#[inline]
pub(crate) fn set_socket_passcred(fd: BorrowedFd<'_>, passcred: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_PASSCRED, from_bool(passcred))
}

#[inline]
pub(crate) fn socket_passcred(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_PASSCRED).map(to_bool)
}

#[inline]
pub(crate) fn set_socket_timeout(
    fd: BorrowedFd<'_>,
    id: Timeout,
    timeout: Option<Duration>,
) -> io::Result<()> {
    let time = duration_to_linux_sock_timeval(timeout)?;
    let optname = match id {
        Timeout::Recv => c::SO_RCVTIMEO_NEW,
        Timeout::Send => c::SO_SNDTIMEO_NEW,
    };
    match setsockopt(fd, c::SOL_SOCKET, optname, time) {
        Err(io::Errno::NOPROTOOPT) if c::SO_RCVTIMEO_NEW != c::SO_RCVTIMEO_OLD => {
            set_socket_timeout_old(fd, id, timeout)
        }
        otherwise => otherwise,
    }
}

/// Same as `set_socket_timeout` but uses `__kernel_old_timeval` instead of
/// `__kernel_sock_timeval` and `_OLD` constants instead of `_NEW`.
fn set_socket_timeout_old(
    fd: BorrowedFd<'_>,
    id: Timeout,
    timeout: Option<Duration>,
) -> io::Result<()> {
    let time = duration_to_linux_old_timeval(timeout)?;
    let optname = match id {
        Timeout::Recv => c::SO_RCVTIMEO_OLD,
        Timeout::Send => c::SO_SNDTIMEO_OLD,
    };
    setsockopt(fd, c::SOL_SOCKET, optname, time)
}

#[inline]
pub(crate) fn socket_timeout(fd: BorrowedFd<'_>, id: Timeout) -> io::Result<Option<Duration>> {
    let optname = match id {
        Timeout::Recv => c::SO_RCVTIMEO_NEW,
        Timeout::Send => c::SO_SNDTIMEO_NEW,
    };
    let time: __kernel_sock_timeval = match getsockopt(fd, c::SOL_SOCKET, optname) {
        Err(io::Errno::NOPROTOOPT) if c::SO_RCVTIMEO_NEW != c::SO_RCVTIMEO_OLD => {
            return socket_timeout_old(fd, id)
        }
        otherwise => otherwise?,
    };
    Ok(duration_from_linux_sock_timeval(time))
}

/// Same as `get_socket_timeout` but uses `__kernel_old_timeval` instead of
/// `__kernel_sock_timeval` and `_OLD` constants instead of `_NEW`.
fn socket_timeout_old(fd: BorrowedFd<'_>, id: Timeout) -> io::Result<Option<Duration>> {
    let optname = match id {
        Timeout::Recv => c::SO_RCVTIMEO_OLD,
        Timeout::Send => c::SO_SNDTIMEO_OLD,
    };
    let time: __kernel_old_timeval = getsockopt(fd, c::SOL_SOCKET, optname)?;
    Ok(duration_from_linux_old_timeval(time))
}

/// Convert a `__linux_sock_timeval` to a Rust `Option<Duration>`.
#[inline]
fn duration_from_linux_sock_timeval(time: __kernel_sock_timeval) -> Option<Duration> {
    if time.tv_sec == 0 && time.tv_usec == 0 {
        None
    } else {
        Some(Duration::from_secs(time.tv_sec as u64) + Duration::from_micros(time.tv_usec as u64))
    }
}

/// Like `duration_from_linux_sock_timeval` but uses Linux's old 32-bit
/// `__kernel_old_timeval`.
fn duration_from_linux_old_timeval(time: __kernel_old_timeval) -> Option<Duration> {
    if time.tv_sec == 0 && time.tv_usec == 0 {
        None
    } else {
        Some(Duration::from_secs(time.tv_sec as u64) + Duration::from_micros(time.tv_usec as u64))
    }
}

/// Convert a Rust `Option<Duration>` to a `__kernel_sock_timeval`.
#[inline]
fn duration_to_linux_sock_timeval(timeout: Option<Duration>) -> io::Result<__kernel_sock_timeval> {
    Ok(match timeout {
        Some(timeout) => {
            if timeout == Duration::ZERO {
                return Err(io::Errno::INVAL);
            }
            // `subsec_micros` rounds down, so we use `subsec_nanos` and
            // manually round up.
            let mut timeout = __kernel_sock_timeval {
                tv_sec: timeout.as_secs().try_into().unwrap_or(i64::MAX),
                tv_usec: ((timeout.subsec_nanos() + 999) / 1000) as _,
            };
            if timeout.tv_sec == 0 && timeout.tv_usec == 0 {
                timeout.tv_usec = 1;
            }
            timeout
        }
        None => __kernel_sock_timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
    })
}

/// Like `duration_to_linux_sock_timeval` but uses Linux's old 32-bit
/// `__kernel_old_timeval`.
fn duration_to_linux_old_timeval(timeout: Option<Duration>) -> io::Result<__kernel_old_timeval> {
    Ok(match timeout {
        Some(timeout) => {
            if timeout == Duration::ZERO {
                return Err(io::Errno::INVAL);
            }

            // `subsec_micros` rounds down, so we use `subsec_nanos` and
            // manually round up.
            let mut timeout = __kernel_old_timeval {
                tv_sec: timeout.as_secs().try_into().unwrap_or(c::c_long::MAX),
                tv_usec: ((timeout.subsec_nanos() + 999) / 1000) as _,
            };
            if timeout.tv_sec == 0 && timeout.tv_usec == 0 {
                timeout.tv_usec = 1;
            }
            timeout
        }
        None => __kernel_old_timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
    })
}

#[inline]
pub(crate) fn socket_error(fd: BorrowedFd<'_>) -> io::Result<Result<(), io::Errno>> {
    let err: c::c_int = getsockopt(fd, c::SOL_SOCKET, c::SO_ERROR)?;
    Ok(if err == 0 {
        Ok(())
    } else {
        Err(io::Errno::from_raw_os_error(err))
    })
}

#[inline]
pub(crate) fn set_socket_keepalive(fd: BorrowedFd<'_>, keepalive: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_KEEPALIVE, from_bool(keepalive))
}

#[inline]
pub(crate) fn socket_keepalive(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_KEEPALIVE).map(to_bool)
}

#[inline]
pub(crate) fn set_socket_recv_buffer_size(fd: BorrowedFd<'_>, size: usize) -> io::Result<()> {
    let size: c::c_int = size.try_into().map_err(|_| io::Errno::INVAL)?;
    setsockopt(fd, c::SOL_SOCKET, c::SO_RCVBUF, size)
}

#[inline]
pub(crate) fn set_socket_recv_buffer_size_force(fd: BorrowedFd<'_>, size: usize) -> io::Result<()> {
    let size: c::c_int = size.try_into().map_err(|_| io::Errno::INVAL)?;
    setsockopt(fd, c::SOL_SOCKET, c::SO_RCVBUFFORCE, size)
}

#[inline]
pub(crate) fn socket_recv_buffer_size(fd: BorrowedFd<'_>) -> io::Result<usize> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_RCVBUF).map(|size: u32| size as usize)
}

#[inline]
pub(crate) fn set_socket_send_buffer_size(fd: BorrowedFd<'_>, size: usize) -> io::Result<()> {
    let size: c::c_int = size.try_into().map_err(|_| io::Errno::INVAL)?;
    setsockopt(fd, c::SOL_SOCKET, c::SO_SNDBUF, size)
}

#[inline]
pub(crate) fn set_socket_send_buffer_size_force(fd: BorrowedFd<'_>, size: usize) -> io::Result<()> {
    let size: c::c_int = size.try_into().map_err(|_| io::Errno::INVAL)?;
    setsockopt(fd, c::SOL_SOCKET, c::SO_SNDBUFFORCE, size)
}

#[inline]
pub(crate) fn socket_send_buffer_size(fd: BorrowedFd<'_>) -> io::Result<usize> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_SNDBUF).map(|size: u32| size as usize)
}

#[inline]
pub(crate) fn socket_domain(fd: BorrowedFd<'_>) -> io::Result<AddressFamily> {
    let domain: c::c_int = getsockopt(fd, c::SOL_SOCKET, c::SO_DOMAIN)?;
    Ok(AddressFamily(
        domain.try_into().map_err(|_| io::Errno::OPNOTSUPP)?,
    ))
}

#[inline]
pub(crate) fn socket_acceptconn(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_ACCEPTCONN).map(to_bool)
}

#[inline]
pub(crate) fn set_socket_oobinline(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_OOBINLINE, from_bool(value))
}

#[inline]
pub(crate) fn socket_oobinline(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_OOBINLINE).map(to_bool)
}

#[inline]
pub(crate) fn set_socket_reuseport(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_REUSEPORT, from_bool(value))
}

#[inline]
pub(crate) fn socket_reuseport(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_REUSEPORT).map(to_bool)
}

#[inline]
pub(crate) fn socket_protocol(fd: BorrowedFd<'_>) -> io::Result<Option<Protocol>> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_PROTOCOL)
        .map(|raw: u32| RawProtocol::new(raw).map(Protocol::from_raw))
}

#[inline]
pub(crate) fn socket_cookie(fd: BorrowedFd<'_>) -> io::Result<u64> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_COOKIE)
}

#[inline]
pub(crate) fn socket_incoming_cpu(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_INCOMING_CPU)
}

#[inline]
pub(crate) fn set_socket_incoming_cpu(fd: BorrowedFd<'_>, value: u32) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_INCOMING_CPU, value)
}

#[inline]
pub(crate) fn set_ip_ttl(fd: BorrowedFd<'_>, ttl: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IP_TTL, ttl)
}

#[inline]
pub(crate) fn ip_ttl(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_IP, c::IP_TTL)
}

#[inline]
pub(crate) fn set_ipv6_v6only(fd: BorrowedFd<'_>, only_v6: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_V6ONLY, from_bool(only_v6))
}

#[inline]
pub(crate) fn ipv6_v6only(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_V6ONLY).map(to_bool)
}

#[inline]
pub(crate) fn ip_mtu(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_IP, IP_MTU)
}

#[inline]
pub(crate) fn ipv6_mtu(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_IPV6, IPV6_MTU)
}

#[inline]
pub(crate) fn set_ip_multicast_if_with_ifindex(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv4Addr,
    address: &Ipv4Addr,
    ifindex: u32,
) -> io::Result<()> {
    let mreqn = to_ip_mreqn(multiaddr, address, ifindex as i32);
    setsockopt(fd, c::IPPROTO_IP, IP_MULTICAST_IF, mreqn)
}

#[inline]
pub(crate) fn set_ip_multicast_if(fd: BorrowedFd<'_>, value: &Ipv4Addr) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, IP_MULTICAST_IF, to_imr_addr(value))
}

#[inline]
pub(crate) fn ip_multicast_if(fd: BorrowedFd<'_>) -> io::Result<Ipv4Addr> {
    getsockopt(fd, c::IPPROTO_IP, IP_MULTICAST_IF).map(from_in_addr)
}

#[inline]
pub(crate) fn set_ipv6_multicast_if(fd: BorrowedFd<'_>, value: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IPV6, IPV6_MULTICAST_IF, value as c::c_int)
}

#[inline]
pub(crate) fn ipv6_multicast_if(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_IPV6, IPV6_MULTICAST_IF)
}

#[inline]
pub(crate) fn set_ip_multicast_loop(fd: BorrowedFd<'_>, multicast_loop: bool) -> io::Result<()> {
    setsockopt(
        fd,
        c::IPPROTO_IP,
        c::IP_MULTICAST_LOOP,
        from_bool(multicast_loop),
    )
}

#[inline]
pub(crate) fn ip_multicast_loop(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IP, c::IP_MULTICAST_LOOP).map(to_bool)
}

#[inline]
pub(crate) fn set_ip_multicast_ttl(fd: BorrowedFd<'_>, multicast_ttl: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IP_MULTICAST_TTL, multicast_ttl)
}

#[inline]
pub(crate) fn ip_multicast_ttl(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_IP, c::IP_MULTICAST_TTL)
}

#[inline]
pub(crate) fn set_ipv6_multicast_loop(fd: BorrowedFd<'_>, multicast_loop: bool) -> io::Result<()> {
    setsockopt(
        fd,
        c::IPPROTO_IPV6,
        c::IPV6_MULTICAST_LOOP,
        from_bool(multicast_loop),
    )
}

#[inline]
pub(crate) fn ipv6_multicast_loop(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_MULTICAST_LOOP).map(to_bool)
}

#[inline]
pub(crate) fn set_ipv6_multicast_hops(fd: BorrowedFd<'_>, multicast_hops: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IPV6_MULTICAST_HOPS, multicast_hops)
}

#[inline]
pub(crate) fn ipv6_multicast_hops(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_IP, c::IPV6_MULTICAST_HOPS)
}

#[inline]
pub(crate) fn set_ip_add_membership(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv4Addr,
    interface: &Ipv4Addr,
) -> io::Result<()> {
    let mreq = to_ip_mreq(multiaddr, interface);
    setsockopt(fd, c::IPPROTO_IP, c::IP_ADD_MEMBERSHIP, mreq)
}

#[inline]
pub(crate) fn set_ip_add_membership_with_ifindex(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv4Addr,
    address: &Ipv4Addr,
    ifindex: u32,
) -> io::Result<()> {
    let mreqn = to_ip_mreqn(multiaddr, address, ifindex as i32);
    setsockopt(fd, c::IPPROTO_IP, c::IP_ADD_MEMBERSHIP, mreqn)
}

#[inline]
pub(crate) fn set_ip_add_source_membership(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv4Addr,
    interface: &Ipv4Addr,
    sourceaddr: &Ipv4Addr,
) -> io::Result<()> {
    let mreq_source = to_imr_source(multiaddr, interface, sourceaddr);
    setsockopt(fd, c::IPPROTO_IP, c::IP_ADD_SOURCE_MEMBERSHIP, mreq_source)
}

#[inline]
pub(crate) fn set_ip_drop_source_membership(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv4Addr,
    interface: &Ipv4Addr,
    sourceaddr: &Ipv4Addr,
) -> io::Result<()> {
    let mreq_source = to_imr_source(multiaddr, interface, sourceaddr);
    setsockopt(fd, c::IPPROTO_IP, c::IP_DROP_SOURCE_MEMBERSHIP, mreq_source)
}

#[inline]
pub(crate) fn set_ipv6_add_membership(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv6Addr,
    interface: u32,
) -> io::Result<()> {
    let mreq = to_ipv6mr(multiaddr, interface);
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_ADD_MEMBERSHIP, mreq)
}

#[inline]
pub(crate) fn set_ip_drop_membership(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv4Addr,
    interface: &Ipv4Addr,
) -> io::Result<()> {
    let mreq = to_ip_mreq(multiaddr, interface);
    setsockopt(fd, c::IPPROTO_IP, c::IP_DROP_MEMBERSHIP, mreq)
}

#[inline]
pub(crate) fn set_ip_drop_membership_with_ifindex(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv4Addr,
    address: &Ipv4Addr,
    ifindex: u32,
) -> io::Result<()> {
    let mreqn = to_ip_mreqn(multiaddr, address, ifindex as i32);
    setsockopt(fd, c::IPPROTO_IP, c::IP_DROP_MEMBERSHIP, mreqn)
}

#[inline]
pub(crate) fn set_ipv6_drop_membership(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv6Addr,
    interface: u32,
) -> io::Result<()> {
    let mreq = to_ipv6mr(multiaddr, interface);
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_DROP_MEMBERSHIP, mreq)
}

#[inline]
pub(crate) fn ipv6_unicast_hops(fd: BorrowedFd<'_>) -> io::Result<u8> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_UNICAST_HOPS).map(|hops: c::c_int| hops as u8)
}

#[inline]
pub(crate) fn set_ipv6_unicast_hops(fd: BorrowedFd<'_>, hops: Option<u8>) -> io::Result<()> {
    let hops = match hops {
        Some(hops) => hops.into(),
        None => -1,
    };
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_UNICAST_HOPS, hops)
}

#[inline]
pub(crate) fn set_ip_tos(fd: BorrowedFd<'_>, value: u8) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IP_TOS, i32::from(value))
}

#[inline]
pub(crate) fn ip_tos(fd: BorrowedFd<'_>) -> io::Result<u8> {
    let value: i32 = getsockopt(fd, c::IPPROTO_IP, c::IP_TOS)?;
    Ok(value as u8)
}

#[inline]
pub(crate) fn set_ip_recvtos(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IP_RECVTOS, from_bool(value))
}

#[inline]
pub(crate) fn ip_recvtos(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IP, c::IP_RECVTOS).map(to_bool)
}

#[inline]
pub(crate) fn set_ipv6_recvtclass(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_RECVTCLASS, from_bool(value))
}

#[inline]
pub(crate) fn ipv6_recvtclass(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_RECVTCLASS).map(to_bool)
}

#[inline]
pub(crate) fn set_ip_freebind(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IP_FREEBIND, from_bool(value))
}

#[inline]
pub(crate) fn ip_freebind(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IP, c::IP_FREEBIND).map(to_bool)
}

#[inline]
pub(crate) fn set_ipv6_freebind(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_FREEBIND, from_bool(value))
}

#[inline]
pub(crate) fn ipv6_freebind(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_FREEBIND).map(to_bool)
}

#[inline]
pub(crate) fn ip_original_dst(fd: BorrowedFd<'_>) -> io::Result<SocketAddrV4> {
    let level = c::IPPROTO_IP;
    let optname = c::SO_ORIGINAL_DST;
    let mut addr = SocketAddrBuf::new();

    getsockopt_raw(fd, level, optname, &mut addr.storage, &mut addr.len)?;

    Ok(unsafe { addr.into_any() }.try_into().unwrap())
}

#[inline]
pub(crate) fn ipv6_original_dst(fd: BorrowedFd<'_>) -> io::Result<SocketAddrV6> {
    let level = c::IPPROTO_IPV6;
    let optname = c::IP6T_SO_ORIGINAL_DST;
    let mut addr = SocketAddrBuf::new();

    getsockopt_raw(fd, level, optname, &mut addr.storage, &mut addr.len)?;

    Ok(unsafe { addr.into_any() }.try_into().unwrap())
}

#[inline]
pub(crate) fn set_ipv6_tclass(fd: BorrowedFd<'_>, value: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_TCLASS, value)
}

#[inline]
pub(crate) fn ipv6_tclass(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_TCLASS)
}

#[inline]
pub(crate) fn set_tcp_nodelay(fd: BorrowedFd<'_>, nodelay: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_NODELAY, from_bool(nodelay))
}

#[inline]
pub(crate) fn tcp_nodelay(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_NODELAY).map(to_bool)
}

#[inline]
pub(crate) fn set_tcp_keepcnt(fd: BorrowedFd<'_>, count: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_KEEPCNT, count)
}

#[inline]
pub(crate) fn tcp_keepcnt(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_KEEPCNT)
}

#[inline]
pub(crate) fn set_tcp_keepidle(fd: BorrowedFd<'_>, duration: Duration) -> io::Result<()> {
    let secs: c::c_uint = duration_to_secs(duration)?;
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_KEEPIDLE, secs)
}

#[inline]
pub(crate) fn tcp_keepidle(fd: BorrowedFd<'_>) -> io::Result<Duration> {
    let secs: c::c_uint = getsockopt(fd, c::IPPROTO_TCP, c::TCP_KEEPIDLE)?;
    Ok(Duration::from_secs(secs.into()))
}

#[inline]
pub(crate) fn set_tcp_keepintvl(fd: BorrowedFd<'_>, duration: Duration) -> io::Result<()> {
    let secs: c::c_uint = duration_to_secs(duration)?;
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_KEEPINTVL, secs)
}

#[inline]
pub(crate) fn tcp_keepintvl(fd: BorrowedFd<'_>) -> io::Result<Duration> {
    let secs: c::c_uint = getsockopt(fd, c::IPPROTO_TCP, c::TCP_KEEPINTVL)?;
    Ok(Duration::from_secs(secs.into()))
}

#[inline]
pub(crate) fn set_tcp_user_timeout(fd: BorrowedFd<'_>, value: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_USER_TIMEOUT, value)
}

#[inline]
pub(crate) fn tcp_user_timeout(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_USER_TIMEOUT)
}

#[inline]
pub(crate) fn set_tcp_quickack(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_QUICKACK, from_bool(value))
}

#[inline]
pub(crate) fn tcp_quickack(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_QUICKACK).map(to_bool)
}

#[inline]
pub(crate) fn set_tcp_congestion(fd: BorrowedFd<'_>, value: &str) -> io::Result<()> {
    let level = c::IPPROTO_TCP;
    let optname = c::TCP_CONGESTION;
    let optlen = value.len().try_into().unwrap();
    setsockopt_raw(fd, level, optname, value.as_ptr(), optlen)
}

#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn tcp_congestion(fd: BorrowedFd<'_>) -> io::Result<String> {
    const OPTLEN: c::socklen_t = 16;

    let level = c::IPPROTO_TCP;
    let optname = c::TCP_CONGESTION;
    let mut value = MaybeUninit::<[MaybeUninit<u8>; OPTLEN as usize]>::uninit();
    let mut optlen = OPTLEN;
    getsockopt_raw(fd, level, optname, &mut value, &mut optlen)?;
    unsafe {
        let value = value.assume_init();
        let slice: &[u8] = core::mem::transmute(&value[..optlen as usize]);
        assert!(slice.contains(&b'\0'));
        Ok(
            core::str::from_utf8(CStr::from_ptr(slice.as_ptr().cast()).to_bytes())
                .unwrap()
                .to_owned(),
        )
    }
}

#[inline]
pub(crate) fn set_tcp_thin_linear_timeouts(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(
        fd,
        c::IPPROTO_TCP,
        c::TCP_THIN_LINEAR_TIMEOUTS,
        from_bool(value),
    )
}

#[inline]
pub(crate) fn tcp_thin_linear_timeouts(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_THIN_LINEAR_TIMEOUTS).map(to_bool)
}

#[inline]
pub(crate) fn set_tcp_cork(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_CORK, from_bool(value))
}

#[inline]
pub(crate) fn tcp_cork(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_CORK).map(to_bool)
}

#[inline]
pub(crate) fn socket_peercred(fd: BorrowedFd<'_>) -> io::Result<UCred> {
    getsockopt(fd, c::SOL_SOCKET, linux_raw_sys::net::SO_PEERCRED)
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn set_xdp_umem_reg(fd: BorrowedFd<'_>, value: XdpUmemReg) -> io::Result<()> {
    setsockopt(fd, c::SOL_XDP, c::XDP_UMEM_REG, value)
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn set_xdp_umem_fill_ring_size(fd: BorrowedFd<'_>, value: u32) -> io::Result<()> {
    setsockopt(fd, c::SOL_XDP, c::XDP_UMEM_FILL_RING, value)
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn set_xdp_umem_completion_ring_size(fd: BorrowedFd<'_>, value: u32) -> io::Result<()> {
    setsockopt(fd, c::SOL_XDP, c::XDP_UMEM_COMPLETION_RING, value)
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn set_xdp_tx_ring_size(fd: BorrowedFd<'_>, value: u32) -> io::Result<()> {
    setsockopt(fd, c::SOL_XDP, c::XDP_TX_RING, value)
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn set_xdp_rx_ring_size(fd: BorrowedFd<'_>, value: u32) -> io::Result<()> {
    setsockopt(fd, c::SOL_XDP, c::XDP_RX_RING, value)
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn xdp_mmap_offsets(fd: BorrowedFd<'_>) -> io::Result<XdpMmapOffsets> {
    // The kernel will write `xdp_mmap_offsets` or `xdp_mmap_offsets_v1` to the
    // supplied pointer, depending on the kernel version. Both structs only
    // contain u64 values. By using the larger of both as the parameter, we can
    // shuffle the values to the non-v1 version returned by
    // `get_xdp_mmap_offsets` while keeping the return type unaffected by the
    // kernel version. This works because C will layout all struct members one
    // after the other.

    let mut optlen = size_of::<xdp_mmap_offsets>().try_into().unwrap();
    debug_assert!(
        optlen as usize >= size_of::<c::c_int>(),
        "Socket APIs don't ever use `bool` directly"
    );
    let mut value = MaybeUninit::<xdp_mmap_offsets>::zeroed();
    getsockopt_raw(fd, c::SOL_XDP, c::XDP_MMAP_OFFSETS, &mut value, &mut optlen)?;

    if optlen as usize == size_of::<c::xdp_mmap_offsets_v1>() {
        // SAFETY: All members of xdp_mmap_offsets are `u64` and thus are
        // correctly initialized by `MaybeUninit::<xdp_statistics>::zeroed()`.
        let xpd_mmap_offsets = unsafe { value.assume_init() };
        Ok(XdpMmapOffsets {
            rx: XdpRingOffset {
                producer: xpd_mmap_offsets.rx.producer,
                consumer: xpd_mmap_offsets.rx.consumer,
                desc: xpd_mmap_offsets.rx.desc,
                flags: None,
            },
            tx: XdpRingOffset {
                producer: xpd_mmap_offsets.rx.flags,
                consumer: xpd_mmap_offsets.tx.producer,
                desc: xpd_mmap_offsets.tx.consumer,
                flags: None,
            },
            fr: XdpRingOffset {
                producer: xpd_mmap_offsets.tx.desc,
                consumer: xpd_mmap_offsets.tx.flags,
                desc: xpd_mmap_offsets.fr.producer,
                flags: None,
            },
            cr: XdpRingOffset {
                producer: xpd_mmap_offsets.fr.consumer,
                consumer: xpd_mmap_offsets.fr.desc,
                desc: xpd_mmap_offsets.fr.flags,
                flags: None,
            },
        })
    } else {
        assert_eq!(
            optlen as usize,
            size_of::<xdp_mmap_offsets>(),
            "unexpected getsockopt size"
        );
        // SAFETY: All members of xdp_mmap_offsets are `u64` and thus are
        // correctly initialized by `MaybeUninit::<xdp_statistics>::zeroed()`.
        let xpd_mmap_offsets = unsafe { value.assume_init() };
        Ok(XdpMmapOffsets {
            rx: XdpRingOffset {
                producer: xpd_mmap_offsets.rx.producer,
                consumer: xpd_mmap_offsets.rx.consumer,
                desc: xpd_mmap_offsets.rx.desc,
                flags: Some(xpd_mmap_offsets.rx.flags),
            },
            tx: XdpRingOffset {
                producer: xpd_mmap_offsets.tx.producer,
                consumer: xpd_mmap_offsets.tx.consumer,
                desc: xpd_mmap_offsets.tx.desc,
                flags: Some(xpd_mmap_offsets.tx.flags),
            },
            fr: XdpRingOffset {
                producer: xpd_mmap_offsets.fr.producer,
                consumer: xpd_mmap_offsets.fr.consumer,
                desc: xpd_mmap_offsets.fr.desc,
                flags: Some(xpd_mmap_offsets.fr.flags),
            },
            cr: XdpRingOffset {
                producer: xpd_mmap_offsets.cr.producer,
                consumer: xpd_mmap_offsets.cr.consumer,
                desc: xpd_mmap_offsets.cr.desc,
                flags: Some(xpd_mmap_offsets.cr.flags),
            },
        })
    }
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn xdp_statistics(fd: BorrowedFd<'_>) -> io::Result<XdpStatistics> {
    let mut optlen = size_of::<xdp_statistics>().try_into().unwrap();
    debug_assert!(
        optlen as usize >= size_of::<c::c_int>(),
        "Socket APIs don't ever use `bool` directly"
    );
    let mut value = MaybeUninit::<xdp_statistics>::zeroed();
    getsockopt_raw(fd, c::SOL_XDP, c::XDP_STATISTICS, &mut value, &mut optlen)?;

    if optlen as usize == size_of::<xdp_statistics_v1>() {
        // SAFETY: All members of xdp_statistics are `u64` and thus are
        // correctly initialized by `MaybeUninit::<xdp_statistics>::zeroed()`.
        let xdp_statistics = unsafe { value.assume_init() };
        Ok(XdpStatistics {
            rx_dropped: xdp_statistics.rx_dropped,
            rx_invalid_descs: xdp_statistics.rx_dropped,
            tx_invalid_descs: xdp_statistics.rx_dropped,
            rx_ring_full: None,
            rx_fill_ring_empty_descs: None,
            tx_ring_empty_descs: None,
        })
    } else {
        assert_eq!(
            optlen as usize,
            size_of::<xdp_statistics>(),
            "unexpected getsockopt size"
        );
        // SAFETY: All members of xdp_statistics are `u64` and thus are
        // correctly initialized by `MaybeUninit::<xdp_statistics>::zeroed()`.
        let xdp_statistics = unsafe { value.assume_init() };
        Ok(XdpStatistics {
            rx_dropped: xdp_statistics.rx_dropped,
            rx_invalid_descs: xdp_statistics.rx_invalid_descs,
            tx_invalid_descs: xdp_statistics.tx_invalid_descs,
            rx_ring_full: Some(xdp_statistics.rx_ring_full),
            rx_fill_ring_empty_descs: Some(xdp_statistics.rx_fill_ring_empty_descs),
            tx_ring_empty_descs: Some(xdp_statistics.tx_ring_empty_descs),
        })
    }
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn xdp_options(fd: BorrowedFd<'_>) -> io::Result<XdpOptionsFlags> {
    getsockopt(fd, c::SOL_XDP, c::XDP_OPTIONS)
}

#[inline]
fn from_in_addr(in_addr: c::in_addr) -> Ipv4Addr {
    Ipv4Addr::from(in_addr.s_addr.to_ne_bytes())
}

#[inline]
fn to_ip_mreq(multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> c::ip_mreq {
    c::ip_mreq {
        imr_multiaddr: to_imr_addr(multiaddr),
        imr_interface: to_imr_addr(interface),
    }
}

#[inline]
fn to_ip_mreqn(multiaddr: &Ipv4Addr, address: &Ipv4Addr, ifindex: i32) -> c::ip_mreqn {
    c::ip_mreqn {
        imr_multiaddr: to_imr_addr(multiaddr),
        imr_address: to_imr_addr(address),
        imr_ifindex: ifindex,
    }
}

#[inline]
fn to_imr_source(
    multiaddr: &Ipv4Addr,
    interface: &Ipv4Addr,
    sourceaddr: &Ipv4Addr,
) -> c::ip_mreq_source {
    c::ip_mreq_source {
        imr_multiaddr: to_imr_addr(multiaddr).s_addr,
        imr_interface: to_imr_addr(interface).s_addr,
        imr_sourceaddr: to_imr_addr(sourceaddr).s_addr,
    }
}

#[inline]
fn to_imr_addr(addr: &Ipv4Addr) -> c::in_addr {
    c::in_addr {
        s_addr: u32::from_ne_bytes(addr.octets()),
    }
}

#[inline]
fn to_ipv6mr(multiaddr: &Ipv6Addr, interface: u32) -> c::ipv6_mreq {
    c::ipv6_mreq {
        ipv6mr_multiaddr: to_ipv6mr_multiaddr(multiaddr),
        ipv6mr_ifindex: to_ipv6mr_interface(interface),
    }
}

#[inline]
fn to_ipv6mr_multiaddr(multiaddr: &Ipv6Addr) -> c::in6_addr {
    c::in6_addr {
        in6_u: linux_raw_sys::net::in6_addr__bindgen_ty_1 {
            u6_addr8: multiaddr.octets(),
        },
    }
}

#[inline]
fn to_ipv6mr_interface(interface: u32) -> c::c_int {
    interface as c::c_int
}

#[inline]
fn from_bool(value: bool) -> c::c_uint {
    c::c_uint::from(value)
}

#[inline]
fn to_bool(value: c::c_uint) -> bool {
    value != 0
}

/// Convert to seconds, rounding up if necessary.
#[inline]
fn duration_to_secs<T: TryFrom<u64>>(duration: Duration) -> io::Result<T> {
    let mut secs = duration.as_secs();
    if duration.subsec_nanos() != 0 {
        secs = secs.checked_add(1).ok_or(io::Errno::INVAL)?;
    }
    T::try_from(secs).map_err(|_e| io::Errno::INVAL)
}
