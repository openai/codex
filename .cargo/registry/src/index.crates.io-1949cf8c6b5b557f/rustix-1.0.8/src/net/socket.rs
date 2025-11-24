use crate::fd::OwnedFd;
use crate::net::addr::SocketAddrArg;
use crate::net::SocketAddrAny;
use crate::{backend, io};
use backend::fd::AsFd;

pub use crate::net::{AddressFamily, Protocol, Shutdown, SocketFlags, SocketType};
#[cfg(unix)]
pub use backend::net::addr::SocketAddrUnix;

/// `socket(domain, type_, protocol)`—Creates a socket.
///
/// POSIX guarantees that `socket` will use the lowest unused file descriptor,
/// however it is not safe in general to rely on this, as file descriptors may
/// be unexpectedly allocated on other threads or in libraries.
///
/// To pass extra flags such as [`SocketFlags::CLOEXEC`] or
/// [`SocketFlags::NONBLOCK`], use [`socket_with`].
///
/// # References
///  - [Beej's Guide to Network Programming]
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#socket
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/socket.html
/// [Linux]: https://man7.org/linux/man-pages/man2/socket.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/socket.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-socket
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=socket&sektion=2
/// [NetBSD]: https://man.netbsd.org/socket.2
/// [OpenBSD]: https://man.openbsd.org/socket.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=socket&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/socket
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Creating-a-Socket.html
#[inline]
pub fn socket(
    domain: AddressFamily,
    type_: SocketType,
    protocol: Option<Protocol>,
) -> io::Result<OwnedFd> {
    backend::net::syscalls::socket(domain, type_, protocol)
}

/// `socket_with(domain, type_ | flags, protocol)`—Creates a socket, with
/// flags.
///
/// POSIX guarantees that `socket` will use the lowest unused file descriptor,
/// however it is not safe in general to rely on this, as file descriptors may
/// be unexpectedly allocated on other threads or in libraries.
///
/// `socket_with` is the same as [`socket`] but adds an additional flags
/// operand.
///
/// # References
///  - [Beej's Guide to Network Programming]
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#socket
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/socket.html
/// [Linux]: https://man7.org/linux/man-pages/man2/socket.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/socket.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-socket
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=socket&sektion=2
/// [NetBSD]: https://man.netbsd.org/socket.2
/// [OpenBSD]: https://man.openbsd.org/socket.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=socket&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/socket
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Creating-a-Socket.html
#[doc(alias("socket"))]
#[inline]
pub fn socket_with(
    domain: AddressFamily,
    type_: SocketType,
    flags: SocketFlags,
    protocol: Option<Protocol>,
) -> io::Result<OwnedFd> {
    backend::net::syscalls::socket_with(domain, type_, flags, protocol)
}

/// `bind(sockfd, addr)`—Binds a socket to an IP address.
///
/// # References
///  - [Beej's Guide to Network Programming]
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#bind
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/bind.html
/// [Linux]: https://man7.org/linux/man-pages/man2/bind.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/bind.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-bind
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=bind&sektion=2
/// [NetBSD]: https://man.netbsd.org/bind.2
/// [OpenBSD]: https://man.openbsd.org/bind.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=bind&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/bind
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Setting-Address.html
pub fn bind<Fd: AsFd>(sockfd: Fd, addr: &impl SocketAddrArg) -> io::Result<()> {
    backend::net::syscalls::bind(sockfd.as_fd(), addr)
}

/// `connect(sockfd, addr)`—Initiates a connection to an IP address.
///
/// On Windows, a non-blocking socket returns [`Errno::WOULDBLOCK`] if the
/// connection cannot be completed immediately, rather than
/// `Errno::INPROGRESS`.
///
/// # References
///  - [Beej's Guide to Network Programming]
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#connect
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/connect.html
/// [Linux]: https://man7.org/linux/man-pages/man2/connect.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/connect.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-connect
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=connect&sektion=2
/// [NetBSD]: https://man.netbsd.org/connect.2
/// [OpenBSD]: https://man.openbsd.org/connect.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=connect&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/connect
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Connecting.html
/// [`Errno::WOULDBLOCK`]: io::Errno::WOULDBLOCK
pub fn connect<Fd: AsFd>(sockfd: Fd, addr: &impl SocketAddrArg) -> io::Result<()> {
    backend::net::syscalls::connect(sockfd.as_fd(), addr)
}

/// `connect(sockfd, {.sa_family = AF_UNSPEC}, sizeof(struct sockaddr))`—
/// Dissolve the socket's association.
///
/// On UDP sockets, BSD platforms report [`Errno::AFNOSUPPORT`] or
/// [`Errno::INVAL`] even if the disconnect was successful.
///
/// # References
///  - [Beej's Guide to Network Programming]
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#connect
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/connect.html
/// [Linux]: https://man7.org/linux/man-pages/man2/connect.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/connect.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-connect
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=connect&sektion=2
/// [NetBSD]: https://man.netbsd.org/connect.2
/// [OpenBSD]: https://man.openbsd.org/connect.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=connect&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/connect
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Connecting.html
/// [`Errno::AFNOSUPPORT`]: io::Errno::AFNOSUPPORT
/// [`Errno::INVAL`]: io::Errno::INVAL
#[inline]
#[doc(alias = "connect")]
pub fn connect_unspec<Fd: AsFd>(sockfd: Fd) -> io::Result<()> {
    backend::net::syscalls::connect_unspec(sockfd.as_fd())
}

/// `listen(fd, backlog)`—Enables listening for incoming connections.
///
/// # References
///  - [Beej's Guide to Network Programming]
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#listen
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/listen.html
/// [Linux]: https://man7.org/linux/man-pages/man2/listen.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/listen.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-listen
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=listen&sektion=2
/// [NetBSD]: https://man.netbsd.org/listen.2
/// [OpenBSD]: https://man.openbsd.org/listen.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=listen&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/listen
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Listening.html
#[inline]
pub fn listen<Fd: AsFd>(sockfd: Fd, backlog: i32) -> io::Result<()> {
    backend::net::syscalls::listen(sockfd.as_fd(), backlog)
}

/// `accept(fd, NULL, NULL)`—Accepts an incoming connection.
///
/// Use [`acceptfrom`] to retrieve the peer address.
///
/// POSIX guarantees that `accept` will use the lowest unused file descriptor,
/// however it is not safe in general to rely on this, as file descriptors may
/// be unexpectedly allocated on other threads or in libraries.
///
/// See [`accept_with`] to pass additional flags.
///
/// # References
///  - [Beej's Guide to Network Programming]
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#acceptthank-you-for-calling-port-3490.
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/accept.html
/// [Linux]: https://man7.org/linux/man-pages/man2/accept.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/accept.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-accept
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=accept&sektion=2
/// [NetBSD]: https://man.netbsd.org/accept.2
/// [OpenBSD]: https://man.openbsd.org/accept.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=accept&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/accept
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Accepting-Connections.html
#[inline]
pub fn accept<Fd: AsFd>(sockfd: Fd) -> io::Result<OwnedFd> {
    backend::net::syscalls::accept(sockfd.as_fd())
}

/// `accept4(fd, NULL, NULL, flags)`—Accepts an incoming connection, with
/// flags.
///
/// Use [`acceptfrom_with`] to retrieve the peer address.
///
/// Even though POSIX guarantees that this will use the lowest unused file
/// descriptor, it is not safe in general to rely on this, as file descriptors
/// may be unexpectedly allocated on other threads or in libraries.
///
/// `accept_with` is the same as [`accept`] but adds an additional flags
/// operand.
///
/// # References
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/accept4.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=accept4&sektion=2
/// [NetBSD]: https://man.netbsd.org/accept4.2
/// [OpenBSD]: https://man.openbsd.org/accept4.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=accept4&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/accept4
#[inline]
#[doc(alias = "accept4")]
pub fn accept_with<Fd: AsFd>(sockfd: Fd, flags: SocketFlags) -> io::Result<OwnedFd> {
    backend::net::syscalls::accept_with(sockfd.as_fd(), flags)
}

/// `accept(fd, &addr, &len)`—Accepts an incoming connection and returns the
/// peer address.
///
/// Use [`accept`] if the peer address isn't needed.
///
/// See [`acceptfrom_with`] to pass additional flags.
///
/// # References
///  - [Beej's Guide to Network Programming]
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#acceptthank-you-for-calling-port-3490.
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/accept.html
/// [Linux]: https://man7.org/linux/man-pages/man2/accept.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/accept.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-accept
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=accept&sektion=2
/// [NetBSD]: https://man.netbsd.org/accept.2
/// [OpenBSD]: https://man.openbsd.org/accept.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=accept&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/accept
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Accepting-Connections.html
#[inline]
#[doc(alias = "accept")]
pub fn acceptfrom<Fd: AsFd>(sockfd: Fd) -> io::Result<(OwnedFd, Option<SocketAddrAny>)> {
    backend::net::syscalls::acceptfrom(sockfd.as_fd())
}

/// `accept4(fd, &addr, &len, flags)`—Accepts an incoming connection and
/// returns the peer address, with flags.
///
/// Use [`accept_with`] if the peer address isn't needed.
///
/// `acceptfrom_with` is the same as [`acceptfrom`] but adds an additional
/// flags operand.
///
/// # References
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/accept4.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=accept4&sektion=2
/// [NetBSD]: https://man.netbsd.org/accept4.2
/// [OpenBSD]: https://man.openbsd.org/accept4.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=accept4&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/accept4
#[inline]
#[doc(alias = "accept4")]
pub fn acceptfrom_with<Fd: AsFd>(
    sockfd: Fd,
    flags: SocketFlags,
) -> io::Result<(OwnedFd, Option<SocketAddrAny>)> {
    backend::net::syscalls::acceptfrom_with(sockfd.as_fd(), flags)
}

/// `shutdown(fd, how)`—Closes the read and/or write sides of a stream.
///
/// # References
///  - [Beej's Guide to Network Programming]
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#close-and-shutdownget-outta-my-face
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/shutdown.html
/// [Linux]: https://man7.org/linux/man-pages/man2/shutdown.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/shutdown.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-shutdown
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=shutdown&sektion=2
/// [NetBSD]: https://man.netbsd.org/shutdown.2
/// [OpenBSD]: https://man.openbsd.org/shutdown.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=shutdown&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/shutdown
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Closing-a-Socket.html
#[inline]
pub fn shutdown<Fd: AsFd>(sockfd: Fd, how: Shutdown) -> io::Result<()> {
    backend::net::syscalls::shutdown(sockfd.as_fd(), how)
}

/// `getsockname(fd, addr, len)`—Returns the address a socket is bound to.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getsockname.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getsockname.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getsockname.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-getsockname
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=getsockname&sektion=2
/// [NetBSD]: https://man.netbsd.org/getsockname.2
/// [OpenBSD]: https://man.openbsd.org/getsockname.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=getsockname&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/getsockname
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Reading-Address.html
#[inline]
pub fn getsockname<Fd: AsFd>(sockfd: Fd) -> io::Result<SocketAddrAny> {
    backend::net::syscalls::getsockname(sockfd.as_fd())
}

/// `getpeername(fd, addr, len)`—Returns the address a socket is connected
/// to.
///
/// # References
///  - [Beej's Guide to Network Programming]
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#getpeernamewho-are-you
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getpeername.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getpeername.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getpeername.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-getpeername
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=getpeername&sektion=2
/// [NetBSD]: https://man.netbsd.org/getpeername.2
/// [OpenBSD]: https://man.openbsd.org/getpeername.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=getpeername&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/getpeername
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Who-is-Connected.html
#[inline]
pub fn getpeername<Fd: AsFd>(sockfd: Fd) -> io::Result<Option<SocketAddrAny>> {
    backend::net::syscalls::getpeername(sockfd.as_fd())
}
