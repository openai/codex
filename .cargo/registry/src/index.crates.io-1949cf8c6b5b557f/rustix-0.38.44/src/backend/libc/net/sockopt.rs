//! libc syscalls supporting `rustix::net::sockopt`.

use super::ext::{in6_addr_new, in_addr_new};
use crate::backend::c;
use crate::backend::conv::{borrowed_fd, ret};
use crate::fd::BorrowedFd;
#[cfg(feature = "alloc")]
#[cfg(any(
    linux_like,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "illumos"
))]
use crate::ffi::CStr;
use crate::io;
use crate::net::sockopt::Timeout;
#[cfg(target_os = "linux")]
use crate::net::xdp::{XdpMmapOffsets, XdpOptionsFlags, XdpRingOffset, XdpStatistics, XdpUmemReg};
#[cfg(not(any(
    apple,
    windows,
    target_os = "aix",
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "netbsd",
    target_os = "nto",
    target_os = "vita",
)))]
use crate::net::AddressFamily;
#[cfg(any(
    linux_kernel,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "openbsd",
    target_os = "redox",
    target_env = "newlib"
))]
use crate::net::Protocol;
#[cfg(any(
    linux_kernel,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "openbsd",
    target_os = "redox",
    target_env = "newlib"
))]
use crate::net::RawProtocol;
use crate::net::{Ipv4Addr, Ipv6Addr, SocketType};
#[cfg(any(linux_kernel, target_os = "fuchsia"))]
use crate::net::{SocketAddrAny, SocketAddrStorage, SocketAddrV4};
#[cfg(linux_kernel)]
use crate::net::{SocketAddrV6, UCred};
use crate::utils::as_mut_ptr;
#[cfg(feature = "alloc")]
#[cfg(any(
    linux_like,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "illumos"
))]
use alloc::borrow::ToOwned;
#[cfg(feature = "alloc")]
#[cfg(any(
    linux_like,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "illumos"
))]
use alloc::string::String;
#[cfg(apple)]
use c::TCP_KEEPALIVE as TCP_KEEPIDLE;
#[cfg(not(any(apple, target_os = "openbsd", target_os = "haiku", target_os = "nto")))]
use c::TCP_KEEPIDLE;
use core::mem::{size_of, MaybeUninit};
use core::time::Duration;
#[cfg(target_os = "linux")]
use linux_raw_sys::xdp::{xdp_mmap_offsets, xdp_statistics, xdp_statistics_v1};
#[cfg(windows)]
use windows_sys::Win32::Foundation::BOOL;

#[inline]
fn getsockopt<T: Copy>(fd: BorrowedFd<'_>, level: i32, optname: i32) -> io::Result<T> {
    let mut optlen = core::mem::size_of::<T>().try_into().unwrap();
    debug_assert!(
        optlen as usize >= core::mem::size_of::<c::c_int>(),
        "Socket APIs don't ever use `bool` directly"
    );

    let mut value = MaybeUninit::<T>::zeroed();
    getsockopt_raw(fd, level, optname, &mut value, &mut optlen)?;

    // On Windows at least, `getsockopt` has been observed writing 1
    // byte on at least (`IPPROTO_TCP`, `TCP_NODELAY`), even though
    // Windows' documentation says that should write a 4-byte `BOOL`.
    // So, we initialize the memory to zeros above, and just assert
    // that `getsockopt` doesn't write too many bytes here.
    assert!(
        optlen as usize <= size_of::<T>(),
        "unexpected getsockopt size"
    );

    unsafe { Ok(value.assume_init()) }
}

#[inline]
fn getsockopt_raw<T>(
    fd: BorrowedFd<'_>,
    level: i32,
    optname: i32,
    value: &mut MaybeUninit<T>,
    optlen: &mut c::socklen_t,
) -> io::Result<()> {
    unsafe {
        ret(c::getsockopt(
            borrowed_fd(fd),
            level,
            optname,
            as_mut_ptr(value).cast(),
            optlen,
        ))
    }
}

#[inline]
fn setsockopt<T: Copy>(fd: BorrowedFd<'_>, level: i32, optname: i32, value: T) -> io::Result<()> {
    let optlen = core::mem::size_of::<T>().try_into().unwrap();
    debug_assert!(
        optlen as usize >= core::mem::size_of::<c::c_int>(),
        "Socket APIs don't ever use `bool` directly"
    );
    setsockopt_raw(fd, level, optname, &value, optlen)
}

#[inline]
fn setsockopt_raw<T>(
    fd: BorrowedFd<'_>,
    level: i32,
    optname: i32,
    ptr: *const T,
    optlen: c::socklen_t,
) -> io::Result<()> {
    unsafe {
        ret(c::setsockopt(
            borrowed_fd(fd),
            level,
            optname,
            ptr.cast(),
            optlen,
        ))
    }
}

#[inline]
pub(crate) fn get_socket_type(fd: BorrowedFd<'_>) -> io::Result<SocketType> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_TYPE)
}

#[inline]
pub(crate) fn set_socket_reuseaddr(fd: BorrowedFd<'_>, reuseaddr: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_REUSEADDR, from_bool(reuseaddr))
}

#[inline]
pub(crate) fn get_socket_reuseaddr(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_REUSEADDR).map(to_bool)
}

#[inline]
pub(crate) fn set_socket_broadcast(fd: BorrowedFd<'_>, broadcast: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_BROADCAST, from_bool(broadcast))
}

#[inline]
pub(crate) fn get_socket_broadcast(fd: BorrowedFd<'_>) -> io::Result<bool> {
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
        l_onoff: linger.is_some().into(),
        l_linger,
    };
    setsockopt(fd, c::SOL_SOCKET, c::SO_LINGER, linger)
}

#[inline]
pub(crate) fn get_socket_linger(fd: BorrowedFd<'_>) -> io::Result<Option<Duration>> {
    let linger: c::linger = getsockopt(fd, c::SOL_SOCKET, c::SO_LINGER)?;
    Ok((linger.l_onoff != 0).then(|| Duration::from_secs(linger.l_linger as u64)))
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn set_socket_passcred(fd: BorrowedFd<'_>, passcred: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_PASSCRED, from_bool(passcred))
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn get_socket_passcred(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_PASSCRED).map(to_bool)
}

#[inline]
pub(crate) fn set_socket_timeout(
    fd: BorrowedFd<'_>,
    id: Timeout,
    timeout: Option<Duration>,
) -> io::Result<()> {
    let optname = match id {
        Timeout::Recv => c::SO_RCVTIMEO,
        Timeout::Send => c::SO_SNDTIMEO,
    };

    #[cfg(not(windows))]
    let timeout = match timeout {
        Some(timeout) => {
            if timeout == Duration::ZERO {
                return Err(io::Errno::INVAL);
            }

            // Rust's musl libc bindings deprecated `time_t` while they
            // transition to 64-bit `time_t`. What we want here is just
            // “whatever type `timeval`'s `tv_sec` is”, so we're ok using
            // the deprecated type.
            #[allow(deprecated)]
            let tv_sec = timeout.as_secs().try_into().unwrap_or(c::time_t::MAX);

            // `subsec_micros` rounds down, so we use `subsec_nanos` and
            // manually round up.
            let mut timeout = c::timeval {
                tv_sec,
                tv_usec: ((timeout.subsec_nanos() + 999) / 1000) as _,
            };
            if timeout.tv_sec == 0 && timeout.tv_usec == 0 {
                timeout.tv_usec = 1;
            }
            timeout
        }
        None => c::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
    };

    #[cfg(windows)]
    let timeout: u32 = match timeout {
        Some(timeout) => {
            if timeout == Duration::ZERO {
                return Err(io::Errno::INVAL);
            }

            // `as_millis` rounds down, so we use `as_nanos` and
            // manually round up.
            let mut timeout: u32 = ((timeout.as_nanos() + 999_999) / 1_000_000)
                .try_into()
                .map_err(|_convert_err| io::Errno::INVAL)?;
            if timeout == 0 {
                timeout = 1;
            }
            timeout
        }
        None => 0,
    };

    setsockopt(fd, c::SOL_SOCKET, optname, timeout)
}

#[inline]
pub(crate) fn get_socket_timeout(fd: BorrowedFd<'_>, id: Timeout) -> io::Result<Option<Duration>> {
    let optname = match id {
        Timeout::Recv => c::SO_RCVTIMEO,
        Timeout::Send => c::SO_SNDTIMEO,
    };

    #[cfg(not(windows))]
    {
        let timeout: c::timeval = getsockopt(fd, c::SOL_SOCKET, optname)?;
        if timeout.tv_sec == 0 && timeout.tv_usec == 0 {
            Ok(None)
        } else {
            Ok(Some(
                Duration::from_secs(timeout.tv_sec as u64)
                    + Duration::from_micros(timeout.tv_usec as u64),
            ))
        }
    }

    #[cfg(windows)]
    {
        let timeout: u32 = getsockopt(fd, c::SOL_SOCKET, optname)?;
        if timeout == 0 {
            Ok(None)
        } else {
            Ok(Some(Duration::from_millis(timeout as u64)))
        }
    }
}

#[cfg(any(apple, freebsdlike, target_os = "netbsd"))]
#[inline]
pub(crate) fn get_socket_nosigpipe(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_NOSIGPIPE).map(to_bool)
}

#[cfg(any(apple, freebsdlike, target_os = "netbsd"))]
#[inline]
pub(crate) fn set_socket_nosigpipe(fd: BorrowedFd<'_>, val: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_NOSIGPIPE, from_bool(val))
}

#[inline]
pub(crate) fn get_socket_error(fd: BorrowedFd<'_>) -> io::Result<Result<(), io::Errno>> {
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
pub(crate) fn get_socket_keepalive(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_KEEPALIVE).map(to_bool)
}

#[inline]
pub(crate) fn set_socket_recv_buffer_size(fd: BorrowedFd<'_>, size: usize) -> io::Result<()> {
    let size: c::c_int = size.try_into().map_err(|_| io::Errno::INVAL)?;
    setsockopt(fd, c::SOL_SOCKET, c::SO_RCVBUF, size)
}

#[cfg(any(linux_kernel, target_os = "fuchsia", target_os = "redox"))]
#[inline]
pub(crate) fn set_socket_recv_buffer_size_force(fd: BorrowedFd<'_>, size: usize) -> io::Result<()> {
    let size: c::c_int = size.try_into().map_err(|_| io::Errno::INVAL)?;
    setsockopt(fd, c::SOL_SOCKET, c::SO_RCVBUFFORCE, size)
}

#[inline]
pub(crate) fn get_socket_recv_buffer_size(fd: BorrowedFd<'_>) -> io::Result<usize> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_RCVBUF).map(|size: u32| size as usize)
}

#[inline]
pub(crate) fn set_socket_send_buffer_size(fd: BorrowedFd<'_>, size: usize) -> io::Result<()> {
    let size: c::c_int = size.try_into().map_err(|_| io::Errno::INVAL)?;
    setsockopt(fd, c::SOL_SOCKET, c::SO_SNDBUF, size)
}

#[inline]
pub(crate) fn get_socket_send_buffer_size(fd: BorrowedFd<'_>) -> io::Result<usize> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_SNDBUF).map(|size: u32| size as usize)
}

#[inline]
#[cfg(not(any(
    apple,
    windows,
    target_os = "aix",
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "hurd",
    target_os = "netbsd",
    target_os = "nto",
    target_os = "vita",
)))]
pub(crate) fn get_socket_domain(fd: BorrowedFd<'_>) -> io::Result<AddressFamily> {
    let domain: c::c_int = getsockopt(fd, c::SOL_SOCKET, c::SO_DOMAIN)?;
    Ok(AddressFamily(
        domain.try_into().map_err(|_| io::Errno::OPNOTSUPP)?,
    ))
}

#[inline]
#[cfg(not(apple))] // Apple platforms declare the constant, but do not actually implement it.
pub(crate) fn get_socket_acceptconn(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_ACCEPTCONN).map(to_bool)
}

#[inline]
pub(crate) fn set_socket_oobinline(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_OOBINLINE, from_bool(value))
}

#[inline]
pub(crate) fn get_socket_oobinline(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_OOBINLINE).map(to_bool)
}

#[cfg(not(any(solarish, windows)))]
#[inline]
pub(crate) fn set_socket_reuseport(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_REUSEPORT, from_bool(value))
}

#[cfg(not(any(solarish, windows)))]
#[inline]
pub(crate) fn get_socket_reuseport(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_REUSEPORT).map(to_bool)
}

#[cfg(target_os = "freebsd")]
#[inline]
pub(crate) fn set_socket_reuseport_lb(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_REUSEPORT_LB, from_bool(value))
}

#[cfg(target_os = "freebsd")]
#[inline]
pub(crate) fn get_socket_reuseport_lb(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_REUSEPORT_LB).map(to_bool)
}

#[cfg(any(
    linux_kernel,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "openbsd",
    target_os = "redox",
    target_env = "newlib"
))]
#[inline]
pub(crate) fn get_socket_protocol(fd: BorrowedFd<'_>) -> io::Result<Option<Protocol>> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_PROTOCOL)
        .map(|raw| RawProtocol::new(raw).map(Protocol::from_raw))
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn get_socket_cookie(fd: BorrowedFd<'_>) -> io::Result<u64> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_COOKIE)
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn get_socket_incoming_cpu(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_INCOMING_CPU)
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn set_socket_incoming_cpu(fd: BorrowedFd<'_>, value: u32) -> io::Result<()> {
    setsockopt(fd, c::SOL_SOCKET, c::SO_INCOMING_CPU, value)
}

#[inline]
pub(crate) fn set_ip_ttl(fd: BorrowedFd<'_>, ttl: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IP_TTL, ttl)
}

#[inline]
pub(crate) fn get_ip_ttl(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_IP, c::IP_TTL)
}

#[inline]
pub(crate) fn set_ipv6_v6only(fd: BorrowedFd<'_>, only_v6: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_V6ONLY, from_bool(only_v6))
}

#[inline]
pub(crate) fn get_ipv6_v6only(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_V6ONLY).map(to_bool)
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
pub(crate) fn get_ip_multicast_loop(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IP, c::IP_MULTICAST_LOOP).map(to_bool)
}

#[inline]
pub(crate) fn set_ip_multicast_ttl(fd: BorrowedFd<'_>, multicast_ttl: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IP_MULTICAST_TTL, multicast_ttl)
}

#[inline]
pub(crate) fn get_ip_multicast_ttl(fd: BorrowedFd<'_>) -> io::Result<u32> {
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
pub(crate) fn get_ipv6_multicast_loop(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_MULTICAST_LOOP).map(to_bool)
}

#[inline]
pub(crate) fn set_ipv6_multicast_hops(fd: BorrowedFd<'_>, multicast_hops: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IPV6_MULTICAST_HOPS, multicast_hops)
}

#[inline]
pub(crate) fn get_ipv6_multicast_hops(fd: BorrowedFd<'_>) -> io::Result<u32> {
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

#[cfg(any(
    apple,
    freebsdlike,
    linux_like,
    target_os = "fuchsia",
    target_os = "openbsd"
))]
#[inline]
pub(crate) fn set_ip_add_membership_with_ifindex(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv4Addr,
    address: &Ipv4Addr,
    ifindex: i32,
) -> io::Result<()> {
    let mreqn = to_ip_mreqn(multiaddr, address, ifindex);
    setsockopt(fd, c::IPPROTO_IP, c::IP_ADD_MEMBERSHIP, mreqn)
}

#[cfg(any(apple, freebsdlike, linux_like, solarish, target_os = "aix"))]
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

#[cfg(any(apple, freebsdlike, linux_like, solarish, target_os = "aix"))]
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
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "haiku",
        target_os = "l4re",
        target_os = "nto"
    )))]
    use c::IPV6_ADD_MEMBERSHIP;
    #[cfg(any(
        bsd,
        solarish,
        target_os = "haiku",
        target_os = "l4re",
        target_os = "nto"
    ))]
    use c::IPV6_JOIN_GROUP as IPV6_ADD_MEMBERSHIP;

    let mreq = to_ipv6mr(multiaddr, interface);
    setsockopt(fd, c::IPPROTO_IPV6, IPV6_ADD_MEMBERSHIP, mreq)
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

#[cfg(any(
    apple,
    freebsdlike,
    linux_like,
    target_os = "fuchsia",
    target_os = "openbsd"
))]
#[inline]
pub(crate) fn set_ip_drop_membership_with_ifindex(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv4Addr,
    address: &Ipv4Addr,
    ifindex: i32,
) -> io::Result<()> {
    let mreqn = to_ip_mreqn(multiaddr, address, ifindex);
    setsockopt(fd, c::IPPROTO_IP, c::IP_DROP_MEMBERSHIP, mreqn)
}

#[inline]
pub(crate) fn set_ipv6_drop_membership(
    fd: BorrowedFd<'_>,
    multiaddr: &Ipv6Addr,
    interface: u32,
) -> io::Result<()> {
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "haiku",
        target_os = "l4re",
        target_os = "nto"
    )))]
    use c::IPV6_DROP_MEMBERSHIP;
    #[cfg(any(
        bsd,
        solarish,
        target_os = "haiku",
        target_os = "l4re",
        target_os = "nto"
    ))]
    use c::IPV6_LEAVE_GROUP as IPV6_DROP_MEMBERSHIP;

    let mreq = to_ipv6mr(multiaddr, interface);
    setsockopt(fd, c::IPPROTO_IPV6, IPV6_DROP_MEMBERSHIP, mreq)
}

#[inline]
pub(crate) fn get_ipv6_unicast_hops(fd: BorrowedFd<'_>) -> io::Result<u8> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_UNICAST_HOPS).map(|hops: c::c_int| hops as u8)
}

#[inline]
pub(crate) fn set_ipv6_unicast_hops(fd: BorrowedFd<'_>, hops: Option<u8>) -> io::Result<()> {
    let hops = match hops {
        Some(hops) => hops as c::c_int,
        None => -1,
    };
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_UNICAST_HOPS, hops)
}

#[cfg(any(
    bsd,
    linux_like,
    target_os = "aix",
    target_os = "fuchsia",
    target_os = "haiku",
    target_os = "nto",
    target_env = "newlib"
))]
#[inline]
pub(crate) fn set_ip_tos(fd: BorrowedFd<'_>, value: u8) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IP_TOS, i32::from(value))
}

#[cfg(any(
    bsd,
    linux_like,
    target_os = "aix",
    target_os = "fuchsia",
    target_os = "haiku",
    target_os = "nto",
    target_env = "newlib"
))]
#[inline]
pub(crate) fn get_ip_tos(fd: BorrowedFd<'_>) -> io::Result<u8> {
    let value: i32 = getsockopt(fd, c::IPPROTO_IP, c::IP_TOS)?;
    Ok(value as u8)
}

#[cfg(any(apple, linux_like, target_os = "freebsd", target_os = "fuchsia"))]
#[inline]
pub(crate) fn set_ip_recvtos(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IP_RECVTOS, from_bool(value))
}

#[cfg(any(apple, linux_like, target_os = "freebsd", target_os = "fuchsia"))]
#[inline]
pub(crate) fn get_ip_recvtos(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IP, c::IP_RECVTOS).map(to_bool)
}

#[cfg(any(
    bsd,
    linux_like,
    target_os = "aix",
    target_os = "fuchsia",
    target_os = "nto"
))]
#[inline]
pub(crate) fn set_ipv6_recvtclass(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_RECVTCLASS, from_bool(value))
}

#[cfg(any(
    bsd,
    linux_like,
    target_os = "aix",
    target_os = "fuchsia",
    target_os = "nto"
))]
#[inline]
pub(crate) fn get_ipv6_recvtclass(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_RECVTCLASS).map(to_bool)
}

#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[inline]
pub(crate) fn set_ip_freebind(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IP, c::IP_FREEBIND, from_bool(value))
}

#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[inline]
pub(crate) fn get_ip_freebind(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IP, c::IP_FREEBIND).map(to_bool)
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn set_ipv6_freebind(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_FREEBIND, from_bool(value))
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn get_ipv6_freebind(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_FREEBIND).map(to_bool)
}

#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[inline]
pub(crate) fn get_ip_original_dst(fd: BorrowedFd<'_>) -> io::Result<SocketAddrV4> {
    let level = c::IPPROTO_IP;
    let optname = c::SO_ORIGINAL_DST;
    let mut value = MaybeUninit::<SocketAddrStorage>::uninit();
    let mut optlen = core::mem::size_of_val(&value).try_into().unwrap();

    getsockopt_raw(fd, level, optname, &mut value, &mut optlen)?;

    let any = unsafe { SocketAddrAny::read(value.as_ptr(), optlen as usize)? };
    match any {
        SocketAddrAny::V4(v4) => Ok(v4),
        _ => unreachable!(),
    }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn get_ipv6_original_dst(fd: BorrowedFd<'_>) -> io::Result<SocketAddrV6> {
    let level = c::IPPROTO_IPV6;
    let optname = c::IP6T_SO_ORIGINAL_DST;
    let mut value = MaybeUninit::<SocketAddrStorage>::uninit();
    let mut optlen = core::mem::size_of_val(&value).try_into().unwrap();

    getsockopt_raw(fd, level, optname, &mut value, &mut optlen)?;

    let any = unsafe { SocketAddrAny::read(value.as_ptr(), optlen as usize)? };
    match any {
        SocketAddrAny::V6(v6) => Ok(v6),
        _ => unreachable!(),
    }
}

#[cfg(not(any(
    solarish,
    windows,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "vita"
)))]
#[inline]
pub(crate) fn set_ipv6_tclass(fd: BorrowedFd<'_>, value: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_IPV6, c::IPV6_TCLASS, value)
}

#[cfg(not(any(
    solarish,
    windows,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "vita"
)))]
#[inline]
pub(crate) fn get_ipv6_tclass(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_IPV6, c::IPV6_TCLASS)
}

#[inline]
pub(crate) fn set_tcp_nodelay(fd: BorrowedFd<'_>, nodelay: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_NODELAY, from_bool(nodelay))
}

#[inline]
pub(crate) fn get_tcp_nodelay(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_NODELAY).map(to_bool)
}

#[inline]
#[cfg(not(any(target_os = "openbsd", target_os = "haiku", target_os = "nto")))]
pub(crate) fn set_tcp_keepcnt(fd: BorrowedFd<'_>, count: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_KEEPCNT, count)
}

#[inline]
#[cfg(not(any(target_os = "openbsd", target_os = "haiku", target_os = "nto")))]
pub(crate) fn get_tcp_keepcnt(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_KEEPCNT)
}

#[inline]
#[cfg(not(any(target_os = "openbsd", target_os = "haiku", target_os = "nto")))]
pub(crate) fn set_tcp_keepidle(fd: BorrowedFd<'_>, duration: Duration) -> io::Result<()> {
    let secs: c::c_uint = duration_to_secs(duration)?;
    setsockopt(fd, c::IPPROTO_TCP, TCP_KEEPIDLE, secs)
}

#[inline]
#[cfg(not(any(target_os = "openbsd", target_os = "haiku", target_os = "nto")))]
pub(crate) fn get_tcp_keepidle(fd: BorrowedFd<'_>) -> io::Result<Duration> {
    let secs: c::c_uint = getsockopt(fd, c::IPPROTO_TCP, TCP_KEEPIDLE)?;
    Ok(Duration::from_secs(secs as u64))
}

#[inline]
#[cfg(not(any(target_os = "openbsd", target_os = "haiku", target_os = "nto")))]
pub(crate) fn set_tcp_keepintvl(fd: BorrowedFd<'_>, duration: Duration) -> io::Result<()> {
    let secs: c::c_uint = duration_to_secs(duration)?;
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_KEEPINTVL, secs)
}

#[inline]
#[cfg(not(any(target_os = "openbsd", target_os = "haiku", target_os = "nto")))]
pub(crate) fn get_tcp_keepintvl(fd: BorrowedFd<'_>) -> io::Result<Duration> {
    let secs: c::c_uint = getsockopt(fd, c::IPPROTO_TCP, c::TCP_KEEPINTVL)?;
    Ok(Duration::from_secs(secs as u64))
}

#[inline]
#[cfg(any(linux_like, target_os = "fuchsia"))]
pub(crate) fn set_tcp_user_timeout(fd: BorrowedFd<'_>, value: u32) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_USER_TIMEOUT, value)
}

#[inline]
#[cfg(any(linux_like, target_os = "fuchsia"))]
pub(crate) fn get_tcp_user_timeout(fd: BorrowedFd<'_>) -> io::Result<u32> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_USER_TIMEOUT)
}

#[cfg(any(linux_like, target_os = "fuchsia"))]
#[inline]
pub(crate) fn set_tcp_quickack(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_QUICKACK, from_bool(value))
}

#[cfg(any(linux_like, target_os = "fuchsia"))]
#[inline]
pub(crate) fn get_tcp_quickack(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_QUICKACK).map(to_bool)
}

#[cfg(any(
    linux_like,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "illumos"
))]
#[inline]
pub(crate) fn set_tcp_congestion(fd: BorrowedFd<'_>, value: &str) -> io::Result<()> {
    let level = c::IPPROTO_TCP;
    let optname = c::TCP_CONGESTION;
    let optlen = value.len().try_into().unwrap();
    setsockopt_raw(fd, level, optname, value.as_ptr(), optlen)
}

#[cfg(feature = "alloc")]
#[cfg(any(
    linux_like,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "illumos"
))]
#[inline]
pub(crate) fn get_tcp_congestion(fd: BorrowedFd<'_>) -> io::Result<String> {
    const OPTLEN: c::socklen_t = 16;

    let level = c::IPPROTO_TCP;
    let optname = c::TCP_CONGESTION;
    let mut value = MaybeUninit::<[MaybeUninit<u8>; OPTLEN as usize]>::uninit();
    let mut optlen = OPTLEN;
    getsockopt_raw(fd, level, optname, &mut value, &mut optlen)?;
    unsafe {
        let value = value.assume_init();
        let slice: &[u8] = core::mem::transmute(&value[..optlen as usize]);
        assert!(slice.iter().any(|b| *b == b'\0'));
        Ok(
            core::str::from_utf8(CStr::from_ptr(slice.as_ptr().cast()).to_bytes())
                .unwrap()
                .to_owned(),
        )
    }
}

#[cfg(any(linux_like, target_os = "fuchsia"))]
#[inline]
pub(crate) fn set_tcp_thin_linear_timeouts(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(
        fd,
        c::IPPROTO_TCP,
        c::TCP_THIN_LINEAR_TIMEOUTS,
        from_bool(value),
    )
}

#[cfg(any(linux_like, target_os = "fuchsia"))]
#[inline]
pub(crate) fn get_tcp_thin_linear_timeouts(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_THIN_LINEAR_TIMEOUTS).map(to_bool)
}

#[cfg(any(linux_like, solarish, target_os = "fuchsia"))]
#[inline]
pub(crate) fn set_tcp_cork(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    setsockopt(fd, c::IPPROTO_TCP, c::TCP_CORK, from_bool(value))
}

#[cfg(any(linux_like, solarish, target_os = "fuchsia"))]
#[inline]
pub(crate) fn get_tcp_cork(fd: BorrowedFd<'_>) -> io::Result<bool> {
    getsockopt(fd, c::IPPROTO_TCP, c::TCP_CORK).map(to_bool)
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn get_socket_peercred(fd: BorrowedFd<'_>) -> io::Result<UCred> {
    getsockopt(fd, c::SOL_SOCKET, c::SO_PEERCRED)
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
pub(crate) fn get_xdp_mmap_offsets(fd: BorrowedFd<'_>) -> io::Result<XdpMmapOffsets> {
    // The kernel will write `xdp_mmap_offsets` or `xdp_mmap_offsets_v1` to the
    // supplied pointer, depending on the kernel version. Both structs only
    // contain u64 values. By using the larger of both as the parameter, we can
    // shuffle the values to the non-v1 version returned by
    // `get_xdp_mmap_offsets` while keeping the return type unaffected by the
    // kernel version. This works because C will layout all struct members one
    // after the other.

    let mut optlen = core::mem::size_of::<xdp_mmap_offsets>().try_into().unwrap();
    debug_assert!(
        optlen as usize >= core::mem::size_of::<c::c_int>(),
        "Socket APIs don't ever use `bool` directly"
    );
    let mut value = MaybeUninit::<xdp_mmap_offsets>::zeroed();
    getsockopt_raw(fd, c::SOL_XDP, c::XDP_MMAP_OFFSETS, &mut value, &mut optlen)?;

    if optlen as usize == core::mem::size_of::<c::xdp_mmap_offsets_v1>() {
        // Safety: All members of xdp_mmap_offsets are u64 and thus are correctly
        // initialized by `MaybeUninit::<xdp_statistics>::zeroed()`
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
            core::mem::size_of::<xdp_mmap_offsets>(),
            "unexpected getsockopt size"
        );
        // Safety: All members of xdp_mmap_offsets are u64 and thus are correctly
        // initialized by `MaybeUninit::<xdp_statistics>::zeroed()`
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
pub(crate) fn get_xdp_statistics(fd: BorrowedFd<'_>) -> io::Result<XdpStatistics> {
    let mut optlen = core::mem::size_of::<xdp_statistics>().try_into().unwrap();
    debug_assert!(
        optlen as usize >= core::mem::size_of::<c::c_int>(),
        "Socket APIs don't ever use `bool` directly"
    );
    let mut value = MaybeUninit::<xdp_statistics>::zeroed();
    getsockopt_raw(fd, c::SOL_XDP, c::XDP_STATISTICS, &mut value, &mut optlen)?;

    if optlen as usize == core::mem::size_of::<xdp_statistics_v1>() {
        // Safety: All members of xdp_statistics are u64 and thus are correctly
        // initialized by `MaybeUninit::<xdp_statistics>::zeroed()`
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
            core::mem::size_of::<xdp_statistics>(),
            "unexpected getsockopt size"
        );
        // Safety: All members of xdp_statistics are u64 and thus are correctly
        // initialized by `MaybeUninit::<xdp_statistics>::zeroed()`
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
pub(crate) fn get_xdp_options(fd: BorrowedFd<'_>) -> io::Result<XdpOptionsFlags> {
    getsockopt(fd, c::SOL_XDP, c::XDP_OPTIONS)
}

#[inline]
fn to_ip_mreq(multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> c::ip_mreq {
    c::ip_mreq {
        imr_multiaddr: to_imr_addr(multiaddr),
        imr_interface: to_imr_addr(interface),
    }
}

#[cfg(any(
    apple,
    freebsdlike,
    linux_like,
    target_os = "fuchsia",
    target_os = "openbsd"
))]
#[inline]
fn to_ip_mreqn(multiaddr: &Ipv4Addr, address: &Ipv4Addr, ifindex: i32) -> c::ip_mreqn {
    c::ip_mreqn {
        imr_multiaddr: to_imr_addr(multiaddr),
        imr_address: to_imr_addr(address),
        imr_ifindex: ifindex,
    }
}

#[cfg(any(apple, freebsdlike, linux_like, solarish, target_os = "aix"))]
#[inline]
fn to_imr_source(
    multiaddr: &Ipv4Addr,
    interface: &Ipv4Addr,
    sourceaddr: &Ipv4Addr,
) -> c::ip_mreq_source {
    c::ip_mreq_source {
        imr_multiaddr: to_imr_addr(multiaddr),
        imr_interface: to_imr_addr(interface),
        imr_sourceaddr: to_imr_addr(sourceaddr),
    }
}

#[inline]
fn to_imr_addr(addr: &Ipv4Addr) -> c::in_addr {
    in_addr_new(u32::from_ne_bytes(addr.octets()))
}

#[inline]
fn to_ipv6mr(multiaddr: &Ipv6Addr, interface: u32) -> c::ipv6_mreq {
    c::ipv6_mreq {
        ipv6mr_multiaddr: to_ipv6mr_multiaddr(multiaddr),
        ipv6mr_interface: to_ipv6mr_interface(interface),
    }
}

#[inline]
fn to_ipv6mr_multiaddr(multiaddr: &Ipv6Addr) -> c::in6_addr {
    in6_addr_new(multiaddr.octets())
}

#[cfg(target_os = "android")]
#[inline]
fn to_ipv6mr_interface(interface: u32) -> c::c_int {
    interface as c::c_int
}

#[cfg(not(target_os = "android"))]
#[inline]
fn to_ipv6mr_interface(interface: u32) -> c::c_uint {
    interface as c::c_uint
}

// `getsockopt` and `setsockopt` represent boolean values as integers.
#[cfg(not(windows))]
type RawSocketBool = c::c_int;
#[cfg(windows)]
type RawSocketBool = BOOL;

// Wrap `RawSocketBool` in a newtype to discourage misuse.
#[repr(transparent)]
#[derive(Copy, Clone)]
struct SocketBool(RawSocketBool);

// Convert from a `bool` to a `SocketBool`.
#[inline]
fn from_bool(value: bool) -> SocketBool {
    SocketBool(value.into())
}

// Convert from a `SocketBool` to a `bool`.
#[inline]
fn to_bool(value: SocketBool) -> bool {
    value.0 != 0
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
