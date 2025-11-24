//! The unsafe `close` for raw file descriptors.
//!
//! # Safety
//!
//! Operating on raw file descriptors is unsafe.
#![allow(unsafe_code)]

use crate::backend;
use backend::fd::RawFd;

/// `close(raw_fd)`—Closes a `RawFd` directly.
///
/// Most users won't need to use this, as `OwnedFd` automatically closes its
/// file descriptor on `Drop`.
///
/// This function does not return a `Result`, as it is the [responsibility] of
/// filesystem designers to not return errors from `close`. Users who chose to
/// use NFS or similar filesystems should take care to monitor for problems
/// externally.
///
/// [responsibility]: https://lwn.net/Articles/576518/
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/close.html
/// [Linux]: https://man7.org/linux/man-pages/man2/close.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/close.2.html#//apple_ref/doc/man/2/close
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-closesocket
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=close&sektion=2
/// [NetBSD]: https://man.netbsd.org/close.2
/// [OpenBSD]: https://man.openbsd.org/close.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=close&section=2
/// [illumos]: https://illumos.org/man/2/close
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Opening-and-Closing-Files.html#index-close
///
/// # Safety
///
/// This function takes a `RawFd`, which must be valid before the call, and is
/// not valid after the call.
#[inline]
pub unsafe fn close(raw_fd: RawFd) {
    backend::io::syscalls::close(raw_fd)
}

/// `close(raw_fd)`—Closes a `RawFd` directly, and report any errors
/// returned by the OS.
///
/// The rustix developers do not intend the existence of this feature to imply
/// that anyone should use it.
#[cfg(feature = "try_close")]
pub unsafe fn try_close(raw_fd: RawFd) -> crate::io::Result<()> {
    backend::io::syscalls::try_close(raw_fd)
}
