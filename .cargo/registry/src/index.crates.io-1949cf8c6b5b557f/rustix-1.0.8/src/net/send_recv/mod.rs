//! `recv`, `send`, and variants.

#![allow(unsafe_code)]

use crate::buffer::Buffer;
use crate::net::addr::SocketAddrArg;
use crate::net::SocketAddrAny;
use crate::{backend, io};
use backend::fd::AsFd;
use core::cmp::min;

pub use backend::net::send_recv::{RecvFlags, ReturnFlags, SendFlags};

#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
mod msg;

#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
pub use msg::*;

/// `recv(fd, buf, flags)`—Reads data from a socket.
///
/// In addition to the `Buffer::Output` return value, this also returns the
/// number of bytes received before any truncation due to the
/// [`RecvFlags::TRUNC`] flag.
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
#[allow(clippy::type_complexity)]
pub fn recv<Fd: AsFd, Buf: Buffer<u8>>(
    fd: Fd,
    mut buf: Buf,
    flags: RecvFlags,
) -> io::Result<(Buf::Output, usize)> {
    let (ptr, len) = buf.parts_mut();
    // SAFETY: `recv` behaves.
    let recv_len = unsafe { backend::net::syscalls::recv(fd.as_fd(), (ptr, len), flags)? };
    // If the `TRUNC` flag is set, the returned `length` may be longer than the
    // buffer length.
    let min_len = min(len, recv_len);
    // SAFETY: `recv` behaves.
    unsafe { Ok((buf.assume_init(min_len), recv_len)) }
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
/// In addition to the `Buffer::Output` return value, this also returns the
/// number of bytes received before any truncation due to the
/// [`RecvFlags::TRUNC`] flag.
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
pub fn recvfrom<Fd: AsFd, Buf: Buffer<u8>>(
    fd: Fd,
    mut buf: Buf,
    flags: RecvFlags,
) -> io::Result<(Buf::Output, usize, Option<SocketAddrAny>)> {
    let (ptr, len) = buf.parts_mut();
    // SAFETY: `recvfrom` behaves.
    let (recv_len, addr) =
        unsafe { backend::net::syscalls::recvfrom(fd.as_fd(), (ptr, len), flags)? };
    // If the `TRUNC` flag is set, the returned `length` may be longer than the
    // buffer length.
    let min_len = min(len, recv_len);
    // SAFETY: `recvfrom` behaves.
    unsafe { Ok((buf.assume_init(min_len), recv_len, addr)) }
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
    addr: &impl SocketAddrArg,
) -> io::Result<usize> {
    backend::net::syscalls::sendto(fd.as_fd(), buf, flags, addr)
}
