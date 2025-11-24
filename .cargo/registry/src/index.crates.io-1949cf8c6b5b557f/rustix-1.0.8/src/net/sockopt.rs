//! `getsockopt` and `setsockopt` functions.
//!
//! In the rustix API, there is a separate function for each option, so that it
//! can be given an option-specific type signature.
//!
//! # References for all getter functions:
//!
//!  - [POSIX `getsockopt`]
//!  - [Linux `getsockopt`]
//!  - [Winsock `getsockopt`]
//!  - [Apple `getsockopt`]
//!  - [FreeBSD `getsockopt`]
//!  - [NetBSD `getsockopt`]
//!  - [OpenBSD `getsockopt`]
//!  - [DragonFly BSD `getsockopt`]
//!  - [illumos `getsockopt`]
//!  - [glibc `getsockopt`]
//!
//! [POSIX `getsockopt`]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getsockopt.html
//! [Linux `getsockopt`]: https://man7.org/linux/man-pages/man2/getsockopt.2.html
//! [Winsock `getsockopt`]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-getsockopt
//! [Apple `getsockopt`]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getsockopt.2.html
//! [FreeBSD `getsockopt`]: https://man.freebsd.org/cgi/man.cgi?query=getsockopt&sektion=2
//! [NetBSD `getsockopt`]: https://man.netbsd.org/getsockopt.2
//! [OpenBSD `getsockopt`]: https://man.openbsd.org/getsockopt.2
//! [DragonFly BSD `getsockopt`]: https://man.dragonflybsd.org/?command=getsockopt&section=2
//! [illumos `getsockopt`]: https://illumos.org/man/3SOCKET/getsockopt
//! [glibc `getsockopt`]: https://sourceware.org/glibc/manual/latest/html_node/Socket-Option-Functions.html
//!
//! # References for all `set_*` functions:
//!
//!  - [POSIX `setsockopt`]
//!  - [Linux `setsockopt`]
//!  - [Winsock `setsockopt`]
//!  - [Apple `setsockopt`]
//!  - [FreeBSD `setsockopt`]
//!  - [NetBSD `setsockopt`]
//!  - [OpenBSD `setsockopt`]
//!  - [DragonFly BSD `setsockopt`]
//!  - [illumos `setsockopt`]
//!  - [glibc `setsockopt`]
//!
//! [POSIX `setsockopt`]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/setsockopt.html
//! [Linux `setsockopt`]: https://man7.org/linux/man-pages/man2/setsockopt.2.html
//! [Winsock `setsockopt`]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-setsockopt
//! [Apple `setsockopt`]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/setsockopt.2.html
//! [FreeBSD `setsockopt`]: https://man.freebsd.org/cgi/man.cgi?query=setsockopt&sektion=2
//! [NetBSD `setsockopt`]: https://man.netbsd.org/setsockopt.2
//! [OpenBSD `setsockopt`]: https://man.openbsd.org/setsockopt.2
//! [DragonFly BSD `setsockopt`]: https://man.dragonflybsd.org/?command=setsockopt&section=2
//! [illumos `setsockopt`]: https://illumos.org/man/3SOCKET/setsockopt
//! [glibc `setsockopt`]: https://sourceware.org/glibc/manual/latest/html_node/Socket-Option-Functions.html
//!
//! # References for `get_socket_*` and `set_socket_*` functions:
//!
//!  - [References for all getter functions]
//!  - [References for all `set_*` functions]
//!  - [POSIX `sys/socket.h`]
//!  - [Linux `socket`]
//!  - [Winsock `SOL_SOCKET` options]
//!  - [glibc `SOL_SOCKET` Options]
//!
//! [POSIX `sys/socket.h`]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/sys_socket.h.html
//! [Linux `socket`]: https://man7.org/linux/man-pages/man7/socket.7.html
//! [Winsock `SOL_SOCKET` options]: https://docs.microsoft.com/en-us/windows/win32/winsock/sol-socket-socket-options
//! [glibc `SOL_SOCKET` options]: https://sourceware.org/glibc/manual/latest/html_node/Socket_002dLevel-Options.html
//!
//! # References for `get_ip_*` and `set_ip_*` functions:
//!
//!  - [References for all getter functions]
//!  - [References for all `set_*` functions]
//!  - [POSIX `netinet/in.h`]
//!  - [Linux `ip`]
//!  - [Winsock `IPPROTO_IP` options]
//!  - [Apple `ip`]
//!  - [FreeBSD `ip`]
//!  - [NetBSD `ip`]
//!  - [OpenBSD `ip`]
//!  - [DragonFly BSD `ip`]
//!  - [illumos `ip`]
//!
//! [POSIX `netinet/in.h`]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/netinet_in.h.html
//! [Linux `ip`]: https://man7.org/linux/man-pages/man7/ip.7.html
//! [Winsock `IPPROTO_IP` options]: https://docs.microsoft.com/en-us/windows/win32/winsock/ipproto-ip-socket-options
//! [Apple `ip`]: https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man4/ip.4
//! [FreeBSD `ip`]: https://man.freebsd.org/cgi/man.cgi?query=ip&sektion=4
//! [NetBSD `ip`]: https://man.netbsd.org/ip.4
//! [OpenBSD `ip`]: https://man.openbsd.org/ip.4
//! [DragonFly BSD `ip`]: https://man.dragonflybsd.org/?command=ip&section=4
//! [illumos `ip`]: https://illumos.org/man/4P/ip
//!
//! # References for `get_ipv6_*` and `set_ipv6_*` functions:
//!
//!  - [References for all getter functions]
//!  - [References for all `set_*` functions]
//!  - [POSIX `netinet/in.h`]
//!  - [Linux `ipv6`]
//!  - [Winsock `IPPROTO_IPV6` options]
//!  - [Apple `ip6`]
//!  - [FreeBSD `ip6`]
//!  - [NetBSD `ip6`]
//!  - [OpenBSD `ip6`]
//!  - [DragonFly BSD `ip6`]
//!  - [illumos `ip6`]
//!
//! [POSIX `netinet/in.h`]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/netinet_in.h.html
//! [Linux `ipv6`]: https://man7.org/linux/man-pages/man7/ipv6.7.html
//! [Winsock `IPPROTO_IPV6` options]: https://docs.microsoft.com/en-us/windows/win32/winsock/ipproto-ipv6-socket-options
//! [Apple `ip6`]: https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man4/ip6.4
//! [FreeBSD `ip6`]: https://man.freebsd.org/cgi/man.cgi?query=ip6&sektion=4
//! [NetBSD `ip6`]: https://man.netbsd.org/ip6.4
//! [OpenBSD `ip6`]: https://man.openbsd.org/ip6.4
//! [DragonFly BSD `ip6`]: https://man.dragonflybsd.org/?command=ip6&section=4
//! [illumos `ip6`]: https://illumos.org/man/4P/ip6
//!
//! # References for `get_tcp_*` and `set_tcp_*` functions:
//!
//!  - [References for all getter functions]
//!  - [References for all `set_*` functions]
//!  - [POSIX `netinet/tcp.h`]
//!  - [Linux `tcp`]
//!  - [Winsock `IPPROTO_TCP` options]
//!  - [Apple `tcp`]
//!  - [FreeBSD `tcp`]
//!  - [NetBSD `tcp`]
//!  - [OpenBSD `tcp`]
//!  - [DragonFly BSD `tcp`]
//!  - [illumos `tcp`]
//!
//! [POSIX `netinet/tcp.h`]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/netinet_tcp.h.html
//! [Linux `tcp`]: https://man7.org/linux/man-pages/man7/tcp.7.html
//! [Winsock `IPPROTO_TCP` options]: https://docs.microsoft.com/en-us/windows/win32/winsock/ipproto-tcp-socket-options
//! [Apple `tcp`]: https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man4/tcp.4
//! [FreeBSD `tcp`]: https://man.freebsd.org/cgi/man.cgi?query=tcp&sektion=4
//! [NetBSD `tcp`]: https://man.netbsd.org/tcp.4
//! [OpenBSD `tcp`]: https://man.openbsd.org/tcp.4
//! [DragonFly BSD `tcp`]: https://man.dragonflybsd.org/?command=tcp&section=4
//! [illumos `tcp`]: https://illumos.org/man/4P/tcp
//!
//! [References for all getter functions]: #references-for-all-getter-functions
//! [References for all `set_*` functions]: #references-for-all-set_-functions

#![doc(alias = "getsockopt")]
#![doc(alias = "setsockopt")]

#[cfg(target_os = "linux")]
use crate::net::xdp::{XdpMmapOffsets, XdpOptionsFlags, XdpStatistics, XdpUmemReg};
#[cfg(not(any(
    apple,
    windows,
    target_os = "aix",
    target_os = "cygwin",
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
#[cfg(any(linux_kernel, target_os = "fuchsia"))]
use crate::net::SocketAddrV4;
#[cfg(linux_kernel)]
use crate::net::SocketAddrV6;
use crate::net::{Ipv4Addr, Ipv6Addr, SocketType};
use crate::{backend, io};
#[cfg(feature = "alloc")]
#[cfg(any(
    linux_like,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "illumos"
))]
use alloc::string::String;
use backend::c;
use backend::fd::AsFd;
use core::time::Duration;

/// Timeout identifier for use with [`set_socket_timeout`] and
/// [`socket_timeout`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum Timeout {
    /// `SO_RCVTIMEO`—Timeout for receiving.
    Recv = c::SO_RCVTIMEO as _,

    /// `SO_SNDTIMEO`—Timeout for sending.
    Send = c::SO_SNDTIMEO as _,
}

/// `getsockopt(fd, SOL_SOCKET, SO_TYPE)`—Returns the type of a socket.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_TYPE")]
pub fn socket_type<Fd: AsFd>(fd: Fd) -> io::Result<SocketType> {
    backend::net::sockopt::socket_type(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, value)`—Set whether local
/// addresses may be reused in `bind`.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_REUSEADDR")]
pub fn set_socket_reuseaddr<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_socket_reuseaddr(fd.as_fd(), value)
}

/// `getsockopt(fd, SOL_SOCKET, SO_REUSEADDR)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_REUSEADDR")]
pub fn socket_reuseaddr<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::socket_reuseaddr(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, SO_BROADCAST, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_BROADCAST")]
pub fn set_socket_broadcast<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_socket_broadcast(fd.as_fd(), value)
}

/// `getsockopt(fd, SOL_SOCKET, SO_BROADCAST)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_BROADCAST")]
pub fn socket_broadcast<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::socket_broadcast(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, SO_LINGER, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_LINGER")]
pub fn set_socket_linger<Fd: AsFd>(fd: Fd, value: Option<Duration>) -> io::Result<()> {
    backend::net::sockopt::set_socket_linger(fd.as_fd(), value)
}

/// `getsockopt(fd, SOL_SOCKET, SO_LINGER)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_LINGER")]
pub fn socket_linger<Fd: AsFd>(fd: Fd) -> io::Result<Option<Duration>> {
    backend::net::sockopt::socket_linger(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, SO_PASSCRED, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(linux_kernel)]
#[inline]
#[doc(alias = "SO_PASSCRED")]
pub fn set_socket_passcred<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_socket_passcred(fd.as_fd(), value)
}

/// `getsockopt(fd, SOL_SOCKET, SO_PASSCRED)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(linux_kernel)]
#[inline]
#[doc(alias = "SO_PASSCRED")]
pub fn socket_passcred<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::socket_passcred(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, id, value)`—Set the sending or receiving
/// timeout.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_RCVTIMEO")]
#[doc(alias = "SO_SNDTIMEO")]
pub fn set_socket_timeout<Fd: AsFd>(
    fd: Fd,
    id: Timeout,
    value: Option<Duration>,
) -> io::Result<()> {
    backend::net::sockopt::set_socket_timeout(fd.as_fd(), id, value)
}

/// `getsockopt(fd, SOL_SOCKET, id)`—Get the sending or receiving timeout.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_RCVTIMEO")]
#[doc(alias = "SO_SNDTIMEO")]
pub fn socket_timeout<Fd: AsFd>(fd: Fd, id: Timeout) -> io::Result<Option<Duration>> {
    backend::net::sockopt::socket_timeout(fd.as_fd(), id)
}

/// `getsockopt(fd, SOL_SOCKET, SO_ERROR)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_ERROR")]
pub fn socket_error<Fd: AsFd>(fd: Fd) -> io::Result<Result<(), io::Errno>> {
    backend::net::sockopt::socket_error(fd.as_fd())
}

/// `getsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(any(apple, freebsdlike, target_os = "netbsd"))]
#[doc(alias = "SO_NOSIGPIPE")]
#[inline]
pub fn socket_nosigpipe<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::socket_nosigpipe(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(any(apple, freebsdlike, target_os = "netbsd"))]
#[doc(alias = "SO_NOSIGPIPE")]
#[inline]
pub fn set_socket_nosigpipe<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_socket_nosigpipe(fd.as_fd(), value)
}

/// `setsockopt(fd, SOL_SOCKET, SO_KEEPALIVE, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_KEEPALIVE")]
pub fn set_socket_keepalive<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_socket_keepalive(fd.as_fd(), value)
}

/// `getsockopt(fd, SOL_SOCKET, SO_KEEPALIVE)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_KEEPALIVE")]
pub fn socket_keepalive<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::socket_keepalive(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, SO_RCVBUF, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_RCVBUF")]
pub fn set_socket_recv_buffer_size<Fd: AsFd>(fd: Fd, value: usize) -> io::Result<()> {
    backend::net::sockopt::set_socket_recv_buffer_size(fd.as_fd(), value)
}

/// `setsockopt(fd, SOL_SOCKET, SO_RCVBUFFORCE, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(any(linux_kernel, target_os = "fuchsia", target_os = "redox"))]
#[inline]
#[doc(alias = "SO_RCVBUFFORCE")]
pub fn set_socket_recv_buffer_size_force<Fd: AsFd>(fd: Fd, value: usize) -> io::Result<()> {
    backend::net::sockopt::set_socket_recv_buffer_size_force(fd.as_fd(), value)
}

/// `getsockopt(fd, SOL_SOCKET, SO_RCVBUF)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_RCVBUF")]
pub fn socket_recv_buffer_size<Fd: AsFd>(fd: Fd) -> io::Result<usize> {
    backend::net::sockopt::socket_recv_buffer_size(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, SO_SNDBUF, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_SNDBUF")]
pub fn set_socket_send_buffer_size<Fd: AsFd>(fd: Fd, value: usize) -> io::Result<()> {
    backend::net::sockopt::set_socket_send_buffer_size(fd.as_fd(), value)
}

/// `setsockopt(fd, SOL_SOCKET, SO_SNDBUFFORCE, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(any(linux_kernel, target_os = "fuchsia", target_os = "redox"))]
#[inline]
#[doc(alias = "SO_SNDBUFFORCE")]
pub fn set_socket_send_buffer_size_force<Fd: AsFd>(fd: Fd, value: usize) -> io::Result<()> {
    backend::net::sockopt::set_socket_send_buffer_size_force(fd.as_fd(), value)
}

/// `getsockopt(fd, SOL_SOCKET, SO_SNDBUF)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_SNDBUF")]
pub fn socket_send_buffer_size<Fd: AsFd>(fd: Fd) -> io::Result<usize> {
    backend::net::sockopt::socket_send_buffer_size(fd.as_fd())
}

/// `getsockopt(fd, SOL_SOCKET, SO_DOMAIN)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(not(any(
    apple,
    windows,
    target_os = "aix",
    target_os = "cygwin",
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "hurd",
    target_os = "netbsd",
    target_os = "nto",
    target_os = "vita",
)))]
#[inline]
#[doc(alias = "SO_DOMAIN")]
pub fn socket_domain<Fd: AsFd>(fd: Fd) -> io::Result<AddressFamily> {
    backend::net::sockopt::socket_domain(fd.as_fd())
}

/// `getsockopt(fd, SOL_SOCKET, SO_ACCEPTCONN)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(not(apple))] // Apple platforms declare the constant, but do not actually implement it.
#[inline]
#[doc(alias = "SO_ACCEPTCONN")]
pub fn socket_acceptconn<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::socket_acceptconn(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, SO_OOBINLINE, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_OOBINLINE")]
pub fn set_socket_oobinline<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_socket_oobinline(fd.as_fd(), value)
}

/// `getsockopt(fd, SOL_SOCKET, SO_OOBINLINE)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "SO_OOBINLINE")]
pub fn socket_oobinline<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::socket_oobinline(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, SO_REUSEPORT, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(not(any(solarish, windows, target_os = "cygwin")))]
#[cfg(not(windows))]
#[inline]
#[doc(alias = "SO_REUSEPORT")]
pub fn set_socket_reuseport<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_socket_reuseport(fd.as_fd(), value)
}

/// `getsockopt(fd, SOL_SOCKET, SO_REUSEPORT)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(not(any(solarish, windows, target_os = "cygwin")))]
#[inline]
#[doc(alias = "SO_REUSEPORT")]
pub fn socket_reuseport<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::socket_reuseport(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, SO_REUSEPORT_LB, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(target_os = "freebsd")]
#[inline]
#[doc(alias = "SO_REUSEPORT_LB")]
pub fn set_socket_reuseport_lb<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_socket_reuseport_lb(fd.as_fd(), value)
}

/// `getsockopt(fd, SOL_SOCKET, SO_REUSEPORT_LB)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(target_os = "freebsd")]
#[inline]
#[doc(alias = "SO_REUSEPORT_LB")]
pub fn socket_reuseport_lb<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::socket_reuseport_lb(fd.as_fd())
}

/// `getsockopt(fd, SOL_SOCKET, SO_PROTOCOL)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(any(
    linux_kernel,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "openbsd",
    target_os = "redox",
    target_env = "newlib"
))]
#[inline]
#[doc(alias = "SO_PROTOCOL")]
pub fn socket_protocol<Fd: AsFd>(fd: Fd) -> io::Result<Option<Protocol>> {
    backend::net::sockopt::socket_protocol(fd.as_fd())
}

/// `getsockopt(fd, SOL_SOCKET, SO_COOKIE)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(target_os = "linux")]
#[inline]
#[doc(alias = "SO_COOKIE")]
pub fn socket_cookie<Fd: AsFd>(fd: Fd) -> io::Result<u64> {
    backend::net::sockopt::socket_cookie(fd.as_fd())
}

/// `getsockopt(fd, SOL_SOCKET, SO_INCOMING_CPU)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(target_os = "linux")]
#[inline]
#[doc(alias = "SO_INCOMING_CPU")]
pub fn socket_incoming_cpu<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    backend::net::sockopt::socket_incoming_cpu(fd.as_fd())
}

/// `setsockopt(fd, SOL_SOCKET, SO_INCOMING_CPU, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[cfg(target_os = "linux")]
#[inline]
#[doc(alias = "SO_INCOMING_CPU")]
pub fn set_socket_incoming_cpu<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_socket_incoming_cpu(fd.as_fd(), value)
}

/// `setsockopt(fd, IPPROTO_IP, IP_TTL, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_socket_-and-set_socket_-functions
#[inline]
#[doc(alias = "IP_TTL")]
pub fn set_ip_ttl<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_ip_ttl(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IP, IP_TTL)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[doc(alias = "IP_TTL")]
pub fn ip_ttl<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    backend::net::sockopt::ip_ttl(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IPV6, IPV6_V6ONLY, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[inline]
#[doc(alias = "IPV6_V6ONLY")]
pub fn set_ipv6_v6only<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_ipv6_v6only(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IPV6, IPV6_V6ONLY)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[inline]
#[doc(alias = "IPV6_V6ONLY")]
pub fn ipv6_v6only<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::ipv6_v6only(fd.as_fd())
}

/// `getsockopt(fd, IPPROTO_IP, IP_MTU)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[cfg(any(linux_kernel, target_os = "cygwin"))]
#[doc(alias = "IP_MTU")]
pub fn ip_mtu<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    backend::net::sockopt::ip_mtu(fd.as_fd())
}

/// `getsockopt(fd, IPPROTO_IPV6, IPV6_MTU)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[cfg(any(linux_kernel, target_os = "cygwin"))]
#[doc(alias = "IPV6_MTU")]
pub fn ipv6_mtu<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    backend::net::sockopt::ipv6_mtu(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IP, IP_MULTICAST_IF, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[doc(alias = "IP_MULTICAST_IF")]
pub fn set_ip_multicast_if<Fd: AsFd>(fd: Fd, value: &Ipv4Addr) -> io::Result<()> {
    backend::net::sockopt::set_ip_multicast_if(fd.as_fd(), value)
}

/// `setsockopt(fd, IPPROTO_IP, IP_MULTICAST_IF, multiaddr, address,
/// ifindex)`
///
/// This is similar to [`set_ip_multicast_if`] but additionally allows an
/// `ifindex` value to be given.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[cfg(any(
    apple,
    freebsdlike,
    linux_like,
    target_os = "fuchsia",
    target_os = "openbsd"
))]
#[inline]
#[doc(alias = "IP_MULTICAST_IF")]
pub fn set_ip_multicast_if_with_ifindex<Fd: AsFd>(
    fd: Fd,
    multiaddr: &Ipv4Addr,
    address: &Ipv4Addr,
    ifindex: u32,
) -> io::Result<()> {
    backend::net::sockopt::set_ip_multicast_if_with_ifindex(fd.as_fd(), multiaddr, address, ifindex)
}

/// `getsockopt(fd, IPPROTO_IP, IP_MULTICAST_IF)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[doc(alias = "IP_MULTICAST_IF")]
pub fn ip_multicast_if<Fd: AsFd>(fd: Fd) -> io::Result<Ipv4Addr> {
    backend::net::sockopt::ip_multicast_if(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IPV6, IPV6_MULTICAST_IF, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[doc(alias = "IPV6_MULTICAST_IF")]
pub fn set_ipv6_multicast_if<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_ipv6_multicast_if(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IPV6, IPV6_MULTICAST_IF)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[doc(alias = "IPV6_MULTICAST_IF")]
pub fn ipv6_multicast_if<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    backend::net::sockopt::ipv6_multicast_if(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IP, IP_MULTICAST_LOOP, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[doc(alias = "IP_MULTICAST_LOOP")]
pub fn set_ip_multicast_loop<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_ip_multicast_loop(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IP, IP_MULTICAST_LOOP)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[doc(alias = "IP_MULTICAST_LOOP")]
pub fn ip_multicast_loop<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::ip_multicast_loop(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IP, IP_MULTICAST_TTL, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[doc(alias = "IP_MULTICAST_TTL")]
pub fn set_ip_multicast_ttl<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_ip_multicast_ttl(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IP, IP_MULTICAST_TTL)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[doc(alias = "IP_MULTICAST_TTL")]
pub fn ip_multicast_ttl<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    backend::net::sockopt::ip_multicast_ttl(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IPV6, IPV6_MULTICAST_LOOP, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[inline]
#[doc(alias = "IPV6_MULTICAST_LOOP")]
pub fn set_ipv6_multicast_loop<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_ipv6_multicast_loop(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IPV6, IPV6_MULTICAST_LOOP)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[inline]
#[doc(alias = "IPV6_MULTICAST_LOOP")]
pub fn ipv6_multicast_loop<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::ipv6_multicast_loop(fd.as_fd())
}

/// `getsockopt(fd, IPPROTO_IPV6, IPV6_UNICAST_HOPS)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[inline]
#[doc(alias = "IPV6_UNICAST_HOPS")]
pub fn ipv6_unicast_hops<Fd: AsFd>(fd: Fd) -> io::Result<u8> {
    backend::net::sockopt::ipv6_unicast_hops(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IPV6, IPV6_UNICAST_HOPS, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[inline]
#[doc(alias = "IPV6_UNICAST_HOPS")]
pub fn set_ipv6_unicast_hops<Fd: AsFd>(fd: Fd, value: Option<u8>) -> io::Result<()> {
    backend::net::sockopt::set_ipv6_unicast_hops(fd.as_fd(), value)
}

/// `setsockopt(fd, IPPROTO_IPV6, IPV6_MULTICAST_HOPS, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[inline]
#[doc(alias = "IPV6_MULTICAST_HOPS")]
pub fn set_ipv6_multicast_hops<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_ipv6_multicast_hops(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IPV6, IPV6_MULTICAST_HOPS)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[inline]
#[doc(alias = "IPV6_MULTICAST_HOPS")]
pub fn ipv6_multicast_hops<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    backend::net::sockopt::ipv6_multicast_hops(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IP, IP_ADD_MEMBERSHIP, multiaddr, interface)`
///
/// This is similar to [`set_ip_add_membership`] but always sets the `ifindex`
/// value to zero. See [`set_ip_add_membership_with_ifindex`] instead to also
/// give the `ifindex` value.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[doc(alias = "IP_ADD_MEMBERSHIP")]
pub fn set_ip_add_membership<Fd: AsFd>(
    fd: Fd,
    multiaddr: &Ipv4Addr,
    interface: &Ipv4Addr,
) -> io::Result<()> {
    backend::net::sockopt::set_ip_add_membership(fd.as_fd(), multiaddr, interface)
}

/// `setsockopt(fd, IPPROTO_IP, IP_ADD_MEMBERSHIP, multiaddr, address,
/// ifindex)`
///
/// This is similar to [`set_ip_add_membership`] but additionally allows an
/// `ifindex` value to be given.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[cfg(any(
    apple,
    freebsdlike,
    linux_like,
    target_os = "fuchsia",
    target_os = "openbsd"
))]
#[inline]
#[doc(alias = "IP_ADD_MEMBERSHIP")]
pub fn set_ip_add_membership_with_ifindex<Fd: AsFd>(
    fd: Fd,
    multiaddr: &Ipv4Addr,
    address: &Ipv4Addr,
    ifindex: u32,
) -> io::Result<()> {
    backend::net::sockopt::set_ip_add_membership_with_ifindex(
        fd.as_fd(),
        multiaddr,
        address,
        ifindex,
    )
}

/// `setsockopt(fd, IPPROTO_IP, IP_ADD_SOURCE_MEMBERSHIP, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[cfg(any(apple, freebsdlike, linux_like, solarish, target_os = "aix"))]
#[inline]
#[doc(alias = "IP_ADD_SOURCE_MEMBERSHIP")]
pub fn set_ip_add_source_membership<Fd: AsFd>(
    fd: Fd,
    multiaddr: &Ipv4Addr,
    interface: &Ipv4Addr,
    sourceaddr: &Ipv4Addr,
) -> io::Result<()> {
    backend::net::sockopt::set_ip_add_source_membership(
        fd.as_fd(),
        multiaddr,
        interface,
        sourceaddr,
    )
}

/// `setsockopt(fd, IPPROTO_IP, IP_DROP_SOURCE_MEMBERSHIP, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[cfg(any(apple, freebsdlike, linux_like, solarish, target_os = "aix"))]
#[inline]
#[doc(alias = "IP_DROP_SOURCE_MEMBERSHIP")]
pub fn set_ip_drop_source_membership<Fd: AsFd>(
    fd: Fd,
    multiaddr: &Ipv4Addr,
    interface: &Ipv4Addr,
    sourceaddr: &Ipv4Addr,
) -> io::Result<()> {
    backend::net::sockopt::set_ip_drop_source_membership(
        fd.as_fd(),
        multiaddr,
        interface,
        sourceaddr,
    )
}

/// `setsockopt(fd, IPPROTO_IPV6, IPV6_ADD_MEMBERSHIP, multiaddr, interface)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[inline]
#[doc(alias = "IPV6_JOIN_GROUP")]
#[doc(alias = "IPV6_ADD_MEMBERSHIP")]
pub fn set_ipv6_add_membership<Fd: AsFd>(
    fd: Fd,
    multiaddr: &Ipv6Addr,
    interface: u32,
) -> io::Result<()> {
    backend::net::sockopt::set_ipv6_add_membership(fd.as_fd(), multiaddr, interface)
}

/// `setsockopt(fd, IPPROTO_IP, IP_DROP_MEMBERSHIP, multiaddr, interface)`
///
/// This is similar to [`set_ip_drop_membership`] but always sets `ifindex`
/// value to zero.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[inline]
#[doc(alias = "IP_DROP_MEMBERSHIP")]
pub fn set_ip_drop_membership<Fd: AsFd>(
    fd: Fd,
    multiaddr: &Ipv4Addr,
    interface: &Ipv4Addr,
) -> io::Result<()> {
    backend::net::sockopt::set_ip_drop_membership(fd.as_fd(), multiaddr, interface)
}

/// `setsockopt(fd, IPPROTO_IP, IP_DROP_MEMBERSHIP, multiaddr, interface)`
///
/// This is similar to [`set_ip_drop_membership_with_ifindex`] but additionally
/// allows a `ifindex` value to be given.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[cfg(any(
    apple,
    freebsdlike,
    linux_like,
    target_os = "fuchsia",
    target_os = "openbsd"
))]
#[inline]
#[doc(alias = "IP_DROP_MEMBERSHIP")]
pub fn set_ip_drop_membership_with_ifindex<Fd: AsFd>(
    fd: Fd,
    multiaddr: &Ipv4Addr,
    address: &Ipv4Addr,
    ifindex: u32,
) -> io::Result<()> {
    backend::net::sockopt::set_ip_drop_membership_with_ifindex(
        fd.as_fd(),
        multiaddr,
        address,
        ifindex,
    )
}

/// `setsockopt(fd, IPPROTO_IPV6, IPV6_DROP_MEMBERSHIP, multiaddr, interface)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[inline]
#[doc(alias = "IPV6_LEAVE_GROUP")]
#[doc(alias = "IPV6_DROP_MEMBERSHIP")]
pub fn set_ipv6_drop_membership<Fd: AsFd>(
    fd: Fd,
    multiaddr: &Ipv6Addr,
    interface: u32,
) -> io::Result<()> {
    backend::net::sockopt::set_ipv6_drop_membership(fd.as_fd(), multiaddr, interface)
}

/// `setsockopt(fd, IPPROTO_IP, IP_TOS, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
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
#[doc(alias = "IP_TOS")]
pub fn set_ip_tos<Fd: AsFd>(fd: Fd, value: u8) -> io::Result<()> {
    backend::net::sockopt::set_ip_tos(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IP, IP_TOS)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
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
#[doc(alias = "IP_TOS")]
pub fn ip_tos<Fd: AsFd>(fd: Fd) -> io::Result<u8> {
    backend::net::sockopt::ip_tos(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IP, IP_RECVTOS, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[cfg(any(
    apple,
    linux_like,
    target_os = "cygwin",
    target_os = "freebsd",
    target_os = "fuchsia",
))]
#[inline]
#[doc(alias = "IP_RECVTOS")]
pub fn set_ip_recvtos<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_ip_recvtos(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IP, IP_RECVTOS)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ip_-and-set_ip_-functions
#[cfg(any(
    apple,
    linux_like,
    target_os = "cygwin",
    target_os = "freebsd",
    target_os = "fuchsia",
))]
#[inline]
#[doc(alias = "IP_RECVTOS")]
pub fn ip_recvtos<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::ip_recvtos(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IPV6, IPV6_RECVTCLASS, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[cfg(any(
    bsd,
    linux_like,
    target_os = "aix",
    target_os = "fuchsia",
    target_os = "nto"
))]
#[inline]
#[doc(alias = "IPV6_RECVTCLASS")]
pub fn set_ipv6_recvtclass<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_ipv6_recvtclass(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IPV6, IPV6_RECVTCLASS)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[cfg(any(
    bsd,
    linux_like,
    target_os = "aix",
    target_os = "fuchsia",
    target_os = "nto"
))]
#[inline]
#[doc(alias = "IPV6_RECVTCLASS")]
pub fn ipv6_recvtclass<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::ipv6_recvtclass(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IP, IP_FREEBIND, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[inline]
#[doc(alias = "IP_FREEBIND")]
pub fn set_ip_freebind<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_ip_freebind(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IP, IP_FREEBIND)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[inline]
#[doc(alias = "IP_FREEBIND")]
pub fn ip_freebind<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::ip_freebind(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IPV6, IPV6_FREEBIND, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[cfg(linux_kernel)]
#[inline]
#[doc(alias = "IPV6_FREEBIND")]
pub fn set_ipv6_freebind<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_ipv6_freebind(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IPV6, IPV6_FREEBIND)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[cfg(linux_kernel)]
#[inline]
#[doc(alias = "IPV6_FREEBIND")]
pub fn ipv6_freebind<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::ipv6_freebind(fd.as_fd())
}

/// `getsockopt(fd, IPPROTO_IP, SO_ORIGINAL_DST)`
///
/// Even though this corresponds to a `SO_*` constant, it is an `IPPROTO_IP`
/// option.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[inline]
#[doc(alias = "SO_ORIGINAL_DST")]
pub fn ip_original_dst<Fd: AsFd>(fd: Fd) -> io::Result<SocketAddrV4> {
    backend::net::sockopt::ip_original_dst(fd.as_fd())
}

/// `getsockopt(fd, IPPROTO_IPV6, IP6T_SO_ORIGINAL_DST)`
///
/// Even though this corresponds to a `IP6T_*` constant, it is an
/// `IPPROTO_IPV6` option.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[cfg(linux_kernel)]
#[inline]
#[doc(alias = "IP6T_SO_ORIGINAL_DST")]
pub fn ipv6_original_dst<Fd: AsFd>(fd: Fd) -> io::Result<SocketAddrV6> {
    backend::net::sockopt::ipv6_original_dst(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_IPV6, IPV6_TCLASS, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[cfg(not(any(
    solarish,
    windows,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
#[inline]
#[doc(alias = "IPV6_TCLASS")]
pub fn set_ipv6_tclass<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_ipv6_tclass(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_IPV6, IPV6_TCLASS)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_ipv6_-and-set_ipv6_-functions
#[cfg(not(any(
    solarish,
    windows,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
#[inline]
#[doc(alias = "IPV6_TCLASS")]
pub fn ipv6_tclass<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    backend::net::sockopt::ipv6_tclass(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[inline]
#[doc(alias = "TCP_NODELAY")]
pub fn set_tcp_nodelay<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_tcp_nodelay(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_TCP, TCP_NODELAY)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[inline]
#[doc(alias = "TCP_NODELAY")]
pub fn tcp_nodelay<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::tcp_nodelay(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_TCP, TCP_KEEPCNT, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(not(any(
    target_os = "haiku",
    target_os = "nto",
    target_os = "openbsd",
    target_os = "redox"
)))]
#[inline]
#[doc(alias = "TCP_KEEPCNT")]
pub fn set_tcp_keepcnt<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_tcp_keepcnt(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_TCP, TCP_KEEPCNT)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(not(any(
    target_os = "haiku",
    target_os = "nto",
    target_os = "openbsd",
    target_os = "redox"
)))]
#[inline]
#[doc(alias = "TCP_KEEPCNT")]
pub fn tcp_keepcnt<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    backend::net::sockopt::tcp_keepcnt(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_TCP, TCP_KEEPIDLE, value)`
///
/// `TCP_KEEPALIVE` on Apple platforms.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(not(any(target_os = "haiku", target_os = "nto", target_os = "openbsd")))]
#[inline]
#[doc(alias = "TCP_KEEPIDLE")]
pub fn set_tcp_keepidle<Fd: AsFd>(fd: Fd, value: Duration) -> io::Result<()> {
    backend::net::sockopt::set_tcp_keepidle(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_TCP, TCP_KEEPIDLE)`
///
/// `TCP_KEEPALIVE` on Apple platforms.
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(not(any(target_os = "haiku", target_os = "nto", target_os = "openbsd")))]
#[inline]
#[doc(alias = "TCP_KEEPIDLE")]
pub fn tcp_keepidle<Fd: AsFd>(fd: Fd) -> io::Result<Duration> {
    backend::net::sockopt::tcp_keepidle(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_TCP, TCP_KEEPINTVL, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(not(any(
    target_os = "haiku",
    target_os = "nto",
    target_os = "openbsd",
    target_os = "redox"
)))]
#[inline]
#[doc(alias = "TCP_KEEPINTVL")]
pub fn set_tcp_keepintvl<Fd: AsFd>(fd: Fd, value: Duration) -> io::Result<()> {
    backend::net::sockopt::set_tcp_keepintvl(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_TCP, TCP_KEEPINTVL)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(not(any(
    target_os = "haiku",
    target_os = "nto",
    target_os = "openbsd",
    target_os = "redox"
)))]
#[inline]
#[doc(alias = "TCP_KEEPINTVL")]
pub fn tcp_keepintvl<Fd: AsFd>(fd: Fd) -> io::Result<Duration> {
    backend::net::sockopt::tcp_keepintvl(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_TCP, TCP_USER_TIMEOUT, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(any(linux_like, target_os = "fuchsia"))]
#[inline]
#[doc(alias = "TCP_USER_TIMEOUT")]
pub fn set_tcp_user_timeout<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_tcp_user_timeout(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_TCP, TCP_USER_TIMEOUT)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(any(linux_like, target_os = "fuchsia"))]
#[inline]
#[doc(alias = "TCP_USER_TIMEOUT")]
pub fn tcp_user_timeout<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    backend::net::sockopt::tcp_user_timeout(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_TCP, TCP_QUICKACK, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(any(linux_like, target_os = "fuchsia"))]
#[inline]
#[doc(alias = "TCP_QUICKACK")]
pub fn set_tcp_quickack<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_tcp_quickack(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_TCP, TCP_QUICKACK)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(any(linux_like, target_os = "fuchsia"))]
#[inline]
#[doc(alias = "TCP_QUICKACK")]
pub fn tcp_quickack<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::tcp_quickack(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_TCP, TCP_CONGESTION, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(any(
    linux_like,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "illumos"
))]
#[inline]
#[doc(alias = "TCP_CONGESTION")]
pub fn set_tcp_congestion<Fd: AsFd>(fd: Fd, value: &str) -> io::Result<()> {
    backend::net::sockopt::set_tcp_congestion(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_TCP, TCP_CONGESTION)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(feature = "alloc")]
#[cfg(any(
    linux_like,
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "illumos"
))]
#[inline]
#[doc(alias = "TCP_CONGESTION")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub fn tcp_congestion<Fd: AsFd>(fd: Fd) -> io::Result<String> {
    backend::net::sockopt::tcp_congestion(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_TCP, TCP_THIN_LINEAR_TIMEOUTS, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(any(linux_like, target_os = "fuchsia"))]
#[inline]
#[doc(alias = "TCP_THIN_LINEAR_TIMEOUTS")]
pub fn set_tcp_thin_linear_timeouts<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_tcp_thin_linear_timeouts(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_TCP, TCP_THIN_LINEAR_TIMEOUTS)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(any(linux_like, target_os = "fuchsia"))]
#[inline]
#[doc(alias = "TCP_THIN_LINEAR_TIMEOUTS")]
pub fn tcp_thin_linear_timeouts<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::tcp_thin_linear_timeouts(fd.as_fd())
}

/// `setsockopt(fd, IPPROTO_TCP, TCP_CORK, value)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(any(linux_like, solarish, target_os = "fuchsia"))]
#[inline]
#[doc(alias = "TCP_CORK")]
pub fn set_tcp_cork<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::net::sockopt::set_tcp_cork(fd.as_fd(), value)
}

/// `getsockopt(fd, IPPROTO_TCP, TCP_CORK)`
///
/// See the [module-level documentation] for more.
///
/// [module-level documentation]: self#references-for-get_tcp_-and-set_tcp_-functions
#[cfg(any(linux_like, solarish, target_os = "fuchsia"))]
#[inline]
#[doc(alias = "TCP_CORK")]
pub fn tcp_cork<Fd: AsFd>(fd: Fd) -> io::Result<bool> {
    backend::net::sockopt::tcp_cork(fd.as_fd())
}

/// `getsockopt(fd, SOL_SOCKET, SO_PEERCRED)`—Get credentials of Unix domain
/// socket peer process.
///
/// # References
///  - [Linux `unix`]
///
/// [Linux `unix`]: https://man7.org/linux/man-pages/man7/unix.7.html
#[cfg(linux_kernel)]
#[doc(alias = "SO_PEERCRED")]
pub fn socket_peercred<Fd: AsFd>(fd: Fd) -> io::Result<super::UCred> {
    backend::net::sockopt::socket_peercred(fd.as_fd())
}

/// `setsockopt(fd, SOL_XDP, XDP_UMEM_REG, value)`
///
/// On kernel versions only supporting v1, the flags are ignored.
///
/// # References
///   - [Linux]
///
/// [Linux]: https://www.kernel.org/doc/html/next/networking/af_xdp.html#xdp-umem-reg-setsockopt
#[cfg(target_os = "linux")]
#[doc(alias = "XDP_UMEM_REG")]
pub fn set_xdp_umem_reg<Fd: AsFd>(fd: Fd, value: XdpUmemReg) -> io::Result<()> {
    backend::net::sockopt::set_xdp_umem_reg(fd.as_fd(), value)
}

/// `setsockopt(fd, SOL_XDP, XDP_UMEM_FILL_RING, value)`
///
/// # References
///   - [Linux]
///
/// [Linux]: https://www.kernel.org/doc/html/next/networking/af_xdp.html#xdp-rx-tx-umem-fill-umem-completion-ring-setsockopts
#[cfg(target_os = "linux")]
#[doc(alias = "XDP_UMEM_FILL_RING")]
pub fn set_xdp_umem_fill_ring_size<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_xdp_umem_fill_ring_size(fd.as_fd(), value)
}

/// `setsockopt(fd, SOL_XDP, XDP_UMEM_COMPLETION_RING, value)`
///
/// # References
///   - [Linux]
///
/// [Linux]: https://www.kernel.org/doc/html/next/networking/af_xdp.html#xdp-rx-tx-umem-fill-umem-completion-ring-setsockopts
#[cfg(target_os = "linux")]
#[doc(alias = "XDP_UMEM_COMPLETION_RING")]
pub fn set_xdp_umem_completion_ring_size<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_xdp_umem_completion_ring_size(fd.as_fd(), value)
}

/// `setsockopt(fd, SOL_XDP, XDP_TX_RING, value)`
///
/// # References
///   - [Linux]
///
/// [Linux]: https://www.kernel.org/doc/html/next/networking/af_xdp.html#xdp-rx-tx-umem-fill-umem-completion-ring-setsockopts
#[cfg(target_os = "linux")]
#[doc(alias = "XDP_TX_RING")]
pub fn set_xdp_tx_ring_size<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_xdp_tx_ring_size(fd.as_fd(), value)
}

/// `setsockopt(fd, SOL_XDP, XDP_RX_RING, value)`
///
/// # References
///   - [Linux]
///
/// [Linux]: https://www.kernel.org/doc/html/next/networking/af_xdp.html#xdp-rx-tx-umem-fill-umem-completion-ring-setsockopts
#[cfg(target_os = "linux")]
#[doc(alias = "XDP_RX_RING")]
pub fn set_xdp_rx_ring_size<Fd: AsFd>(fd: Fd, value: u32) -> io::Result<()> {
    backend::net::sockopt::set_xdp_rx_ring_size(fd.as_fd(), value)
}

/// `getsockopt(fd, SOL_XDP, XDP_MMAP_OFFSETS)`
///
/// # References
///   - [Linux]
///
/// [Linux]: https://www.kernel.org/doc/html/next/networking/af_xdp.html
#[cfg(target_os = "linux")]
#[doc(alias = "XDP_MMAP_OFFSETS")]
pub fn xdp_mmap_offsets<Fd: AsFd>(fd: Fd) -> io::Result<XdpMmapOffsets> {
    backend::net::sockopt::xdp_mmap_offsets(fd.as_fd())
}

/// `getsockopt(fd, SOL_XDP, XDP_STATISTICS)`
///
/// # References
///   - [Linux]
///
/// [Linux]: https://www.kernel.org/doc/html/next/networking/af_xdp.html#xdp-statistics-getsockopt
#[cfg(target_os = "linux")]
#[doc(alias = "XDP_STATISTICS")]
pub fn xdp_statistics<Fd: AsFd>(fd: Fd) -> io::Result<XdpStatistics> {
    backend::net::sockopt::xdp_statistics(fd.as_fd())
}

/// `getsockopt(fd, SOL_XDP, XDP_OPTIONS)`
///
/// # References
///   - [Linux]
///
/// [Linux]: https://www.kernel.org/doc/html/next/networking/af_xdp.html#xdp-options-getsockopt
#[cfg(target_os = "linux")]
#[doc(alias = "XDP_OPTIONS")]
pub fn xdp_options<Fd: AsFd>(fd: Fd) -> io::Result<XdpOptionsFlags> {
    backend::net::sockopt::xdp_options(fd.as_fd())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sizes() {
        use c::c_int;

        // Backend code needs to cast these to `c_int` so make sure that cast
        // isn't lossy.
        assert_eq_size!(Timeout, c_int);
    }
}
