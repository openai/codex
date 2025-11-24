//! `recv`, `send`, and variants.

#![allow(unsafe_code)]

use crate::buffer::split_init;
#[cfg(target_os = "linux")]
use crate::net::xdp::SocketAddrXdp;
#[cfg(unix)]
use crate::net::SocketAddrUnix;
use crate::net::{SocketAddr, SocketAddrAny, SocketAddrV4, SocketAddrV6};
use crate::{backend, io};
use backend::fd::{AsFd, BorrowedFd};
use core::cmp::min;
use core::mem::MaybeUninit;

pub use backend::net::send_recv::{RecvFlags, SendFlags};

#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
mod msg;

#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub use msg::*;

/// `recv(fd, buf, flags)`—Reads data from a socket.
///
/// This takes a `&mut [u8]` which Rust requires to contain initialized memory.
/// To use an uninitialized buffer, use [`recv_uninit`].
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
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#sendrecv
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/recv.html
/// [Linux]: https://man7.org/linux/man-pages/man2/recv.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/recv.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-recv
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=recv&sektion=2
/// [NetBSD]: https://man.netbsd.org/recv.2
/// [OpenBSD]: https://man.openbsd.org/recv.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=recv&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/recv
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Receiving-Data.html
#[inline]
pub fn recv<Fd: AsFd>(fd: Fd, buf: &mut [u8], flags: RecvFlags) -> io::Result<usize> {
    unsafe { backend::net::syscalls::recv(fd.as_fd(), buf.as_mut_ptr(), buf.len(), flags) }
}

/// `recv(fd, buf, flags)`—Reads data from a socket.
///
/// This is equivalent to [`recv`], except that it can read into uninitialized
/// memory. It returns the slice that was initialized by this function and the
/// slice that remains uninitialized.
///
/// Because this interface returns the length via the returned slice, it's
/// unsable to return the untruncated length that would be returned when the
/// `RecvFlags::TRUNC` flag is used. If you need the untruncated length, use
/// [`recv`].
#[inline]
pub fn recv_uninit<Fd: AsFd>(
    fd: Fd,
    buf: &mut [MaybeUninit<u8>],
    flags: RecvFlags,
) -> io::Result<(&mut [u8], &mut [MaybeUninit<u8>])> {
    let length = unsafe {
        backend::net::syscalls::recv(fd.as_fd(), buf.as_mut_ptr().cast::<u8>(), buf.len(), flags)?
    };

    // If the `TRUNC` flag is set, the returned `length` may be longer than the
    // buffer length.
    Ok(unsafe { split_init(buf, min(length, buf.len())) })
}

/// `send(fd, buf, flags)`—Writes data to a socket.
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
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#sendrecv
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/send.html
/// [Linux]: https://man7.org/linux/man-pages/man2/send.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/send.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-send
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=send&sektion=2
/// [NetBSD]: https://man.netbsd.org/send.2
/// [OpenBSD]: https://man.openbsd.org/send.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=send&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/send
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Sending-Data.html
#[inline]
pub fn send<Fd: AsFd>(fd: Fd, buf: &[u8], flags: SendFlags) -> io::Result<usize> {
    backend::net::syscalls::send(fd.as_fd(), buf, flags)
}

/// `recvfrom(fd, buf, flags, addr, len)`—Reads data from a socket and
/// returns the sender address.
///
/// This takes a `&mut [u8]` which Rust requires to contain initialized memory.
/// To use an uninitialized buffer, use [`recvfrom_uninit`].
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
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#sendtorecv
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/recvfrom.html
/// [Linux]: https://man7.org/linux/man-pages/man2/recvfrom.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/recvfrom.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-recvfrom
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=recvfrom&sektion=2
/// [NetBSD]: https://man.netbsd.org/recvfrom.2
/// [OpenBSD]: https://man.openbsd.org/recvfrom.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=recvfrom&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/recvfrom
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Receiving-Datagrams.html
#[inline]
pub fn recvfrom<Fd: AsFd>(
    fd: Fd,
    buf: &mut [u8],
    flags: RecvFlags,
) -> io::Result<(usize, Option<SocketAddrAny>)> {
    unsafe { backend::net::syscalls::recvfrom(fd.as_fd(), buf.as_mut_ptr(), buf.len(), flags) }
}

/// `recvfrom(fd, buf, flags, addr, len)`—Reads data from a socket and
/// returns the sender address.
///
/// This is equivalent to [`recvfrom`], except that it can read into
/// uninitialized memory. It returns the slice that was initialized by this
/// function and the slice that remains uninitialized.
///
/// Because this interface returns the length via the returned slice, it's
/// unsable to return the untruncated length that would be returned when the
/// `RecvFlags::TRUNC` flag is used. If you need the untruncated length, use
/// [`recvfrom`].
#[allow(clippy::type_complexity)]
#[inline]
pub fn recvfrom_uninit<Fd: AsFd>(
    fd: Fd,
    buf: &mut [MaybeUninit<u8>],
    flags: RecvFlags,
) -> io::Result<(&mut [u8], &mut [MaybeUninit<u8>], Option<SocketAddrAny>)> {
    let (length, addr) = unsafe {
        backend::net::syscalls::recvfrom(
            fd.as_fd(),
            buf.as_mut_ptr().cast::<u8>(),
            buf.len(),
            flags,
        )?
    };

    // If the `TRUNC` flag is set, the returned `length` may be longer than the
    // buffer length.
    let (init, uninit) = unsafe { split_init(buf, min(length, buf.len())) };
    Ok((init, uninit, addr))
}

/// `sendto(fd, buf, flags, addr)`—Writes data to a socket to a specific IP
/// address.
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
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#sendtorecv
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/sendto.html
/// [Linux]: https://man7.org/linux/man-pages/man2/sendto.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/sendto.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-sendto
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=sendto&sektion=2
/// [NetBSD]: https://man.netbsd.org/sendto.2
/// [OpenBSD]: https://man.openbsd.org/sendto.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=sendto&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/sendto
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Sending-Datagrams.html
pub fn sendto<Fd: AsFd>(
    fd: Fd,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddr,
) -> io::Result<usize> {
    _sendto(fd.as_fd(), buf, flags, addr)
}

fn _sendto(
    fd: BorrowedFd<'_>,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddr,
) -> io::Result<usize> {
    match addr {
        SocketAddr::V4(v4) => backend::net::syscalls::sendto_v4(fd, buf, flags, v4),
        SocketAddr::V6(v6) => backend::net::syscalls::sendto_v6(fd, buf, flags, v6),
    }
}

/// `sendto(fd, buf, flags, addr)`—Writes data to a socket to a specific
/// address.
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
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#sendtorecv
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/sendto.html
/// [Linux]: https://man7.org/linux/man-pages/man2/sendto.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/sendto.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-sendto
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=sendto&sektion=2
/// [NetBSD]: https://man.netbsd.org/sendto.2
/// [OpenBSD]: https://man.openbsd.org/sendto.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=sendto&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/sendto
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Sending-Datagrams.html
pub fn sendto_any<Fd: AsFd>(
    fd: Fd,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrAny,
) -> io::Result<usize> {
    _sendto_any(fd.as_fd(), buf, flags, addr)
}

fn _sendto_any(
    fd: BorrowedFd<'_>,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrAny,
) -> io::Result<usize> {
    match addr {
        SocketAddrAny::V4(v4) => backend::net::syscalls::sendto_v4(fd, buf, flags, v4),
        SocketAddrAny::V6(v6) => backend::net::syscalls::sendto_v6(fd, buf, flags, v6),
        #[cfg(unix)]
        SocketAddrAny::Unix(unix) => backend::net::syscalls::sendto_unix(fd, buf, flags, unix),
        #[cfg(target_os = "linux")]
        SocketAddrAny::Xdp(xdp) => backend::net::syscalls::sendto_xdp(fd, buf, flags, xdp),
    }
}

/// `sendto(fd, buf, flags, addr, sizeof(struct sockaddr_in))`—Writes data to
/// a socket to a specific IPv4 address.
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
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#sendtorecv
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/sendto.html
/// [Linux]: https://man7.org/linux/man-pages/man2/sendto.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/sendto.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-sendto
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=sendto&sektion=2
/// [NetBSD]: https://man.netbsd.org/sendto.2
/// [OpenBSD]: https://man.openbsd.org/sendto.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=sendto&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/sendto
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Sending-Datagrams.html
#[inline]
#[doc(alias = "sendto")]
pub fn sendto_v4<Fd: AsFd>(
    fd: Fd,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrV4,
) -> io::Result<usize> {
    backend::net::syscalls::sendto_v4(fd.as_fd(), buf, flags, addr)
}

/// `sendto(fd, buf, flags, addr, sizeof(struct sockaddr_in6))`—Writes data
/// to a socket to a specific IPv6 address.
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
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#sendtorecv
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/sendto.html
/// [Linux]: https://man7.org/linux/man-pages/man2/sendto.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/sendto.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-sendto
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=sendto&sektion=2
/// [NetBSD]: https://man.netbsd.org/sendto.2
/// [OpenBSD]: https://man.openbsd.org/sendto.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=sendto&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/sendto
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Sending-Datagrams.html
#[inline]
#[doc(alias = "sendto")]
pub fn sendto_v6<Fd: AsFd>(
    fd: Fd,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrV6,
) -> io::Result<usize> {
    backend::net::syscalls::sendto_v6(fd.as_fd(), buf, flags, addr)
}

/// `sendto(fd, buf, flags, addr, sizeof(struct sockaddr_un))`—Writes data to
/// a socket to a specific Unix-domain socket address.
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
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/system-calls-or-bust.html#sendtorecv
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/sendto.html
/// [Linux]: https://man7.org/linux/man-pages/man2/sendto.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/sendto.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-sendto
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=sendto&sektion=2
/// [NetBSD]: https://man.netbsd.org/sendto.2
/// [OpenBSD]: https://man.openbsd.org/sendto.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=sendto&section=2
/// [illumos]: https://illumos.org/man/3SOCKET/sendto
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Sending-Datagrams.html
#[cfg(unix)]
#[inline]
#[doc(alias = "sendto")]
pub fn sendto_unix<Fd: AsFd>(
    fd: Fd,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrUnix,
) -> io::Result<usize> {
    backend::net::syscalls::sendto_unix(fd.as_fd(), buf, flags, addr)
}

/// `sendto(fd, buf, flags, addr, sizeof(struct sockaddr_xdp))`—Writes data
/// to a socket to a specific XDP address.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/sendto.2.html
#[cfg(target_os = "linux")]
#[inline]
#[doc(alias = "sendto")]
pub fn sendto_xdp<Fd: AsFd>(
    fd: Fd,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrXdp,
) -> io::Result<usize> {
    backend::net::syscalls::sendto_xdp(fd.as_fd(), buf, flags, addr)
}
