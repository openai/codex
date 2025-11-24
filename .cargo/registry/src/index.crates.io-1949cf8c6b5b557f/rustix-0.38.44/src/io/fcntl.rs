//! The Unix `fcntl` function is effectively lots of different functions hidden
//! behind a single dynamic dispatch interface. In order to provide a type-safe
//! API, rustix makes them all separate functions so that they can have
//! dedicated static type signatures.
//!
//! `fcntl` functions which are not specific to files or directories live in
//! the [`io`] module instead.
//!
//! [`io`]: crate::io

use crate::{backend, io};
use backend::fd::{AsFd, OwnedFd, RawFd};

pub use backend::io::types::FdFlags;

/// `fcntl(fd, F_GETFD)`—Returns a file descriptor's flags.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fcntl.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fcntl.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=fcntl&sektion=2
/// [NetBSD]: https://man.netbsd.org/fcntl.2
/// [OpenBSD]: https://man.openbsd.org/fcntl.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=fcntl&section=2
/// [illumos]: https://illumos.org/man/2/fcntl
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Control-Operations.html#index-fcntl-function
#[inline]
#[doc(alias = "F_GETFD")]
pub fn fcntl_getfd<Fd: AsFd>(fd: Fd) -> io::Result<FdFlags> {
    backend::io::syscalls::fcntl_getfd(fd.as_fd())
}

/// `fcntl(fd, F_SETFD, flags)`—Sets a file descriptor's flags.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fcntl.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fcntl.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=fcntl&sektion=2
/// [NetBSD]: https://man.netbsd.org/fcntl.2
/// [OpenBSD]: https://man.openbsd.org/fcntl.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=fcntl&section=2
/// [illumos]: https://illumos.org/man/2/fcntl
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Control-Operations.html#index-fcntl-function
#[inline]
#[doc(alias = "F_SETFD")]
pub fn fcntl_setfd<Fd: AsFd>(fd: Fd, flags: FdFlags) -> io::Result<()> {
    backend::io::syscalls::fcntl_setfd(fd.as_fd(), flags)
}

/// `fcntl(fd, F_DUPFD_CLOEXEC)`—Creates a new `OwnedFd` instance, with value
/// at least `min`, that has `O_CLOEXEC` set and that shares the same
/// underlying [file description] as `fd`.
///
/// POSIX guarantees that `F_DUPFD_CLOEXEC` will use the lowest unused file
/// descriptor which is at least `min`, however it is not safe in general to
/// rely on this, as file descriptors may be unexpectedly allocated on other
/// threads or in libraries.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fcntl.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fcntl.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=fcntl&sektion=2
/// [NetBSD]: https://man.netbsd.org/fcntl.2
/// [OpenBSD]: https://man.openbsd.org/fcntl.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=fcntl&section=2
/// [illumos]: https://illumos.org/man/2/fcntl
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Control-Operations.html#index-fcntl-function
/// [file description]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap03.html#tag_03_258
#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
#[inline]
#[doc(alias = "F_DUPFD_CLOEXEC")]
pub fn fcntl_dupfd_cloexec<Fd: AsFd>(fd: Fd, min: RawFd) -> io::Result<OwnedFd> {
    backend::io::syscalls::fcntl_dupfd_cloexec(fd.as_fd(), min)
}

/// `fcntl(fd, F_DUPFD)`—Creates a new `OwnedFd` instance, with value at
/// least `min`, that shares the same underlying [file description] as `fd`.
///
/// POSIX guarantees that `F_DUPFD` will use the lowest unused file descriptor
/// which is at least `min`, however it is not safe in general to rely on this,
/// as file descriptors may be unexpectedly allocated on other threads or in
/// libraries.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fcntl.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fcntl.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=fcntl&sektion=2
/// [NetBSD]: https://man.netbsd.org/fcntl.2
/// [OpenBSD]: https://man.openbsd.org/fcntl.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=fcntl&section=2
/// [illumos]: https://illumos.org/man/2/fcntl
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Control-Operations.html#index-fcntl-function
/// [file description]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap03.html#tag_03_258
#[cfg(target_os = "espidf")]
#[inline]
#[doc(alias = "F_DUPFD")]
pub fn fcntl_dupfd<Fd: AsFd>(fd: Fd, min: RawFd) -> io::Result<OwnedFd> {
    backend::io::syscalls::fcntl_dupfd(fd.as_fd(), min)
}
