//! The Unix `ioctl` function is effectively lots of different functions hidden
//! behind a single dynamic dispatch interface. In order to provide a type-safe
//! API, rustix makes them all separate functions so that they can have
//! dedicated static type signatures.
//!
//! Some ioctls, such as those related to filesystems, terminals, and
//! processes, live in other top-level API modules.

#![allow(unsafe_code)]

use crate::{backend, io, ioctl};
use backend::c;
use backend::fd::AsFd;

/// `ioctl(fd, FIOCLEX, NULL)`—Set the close-on-exec flag.
///
/// This is similar to `fcntl(fd, F_SETFD, FD_CLOEXEC)`, except that it avoids
/// clearing any other flags that might be set.
#[cfg(apple)]
#[inline]
#[doc(alias = "FIOCLEX")]
#[doc(alias = "FD_CLOEXEC")]
pub fn ioctl_fioclex<Fd: AsFd>(fd: Fd) -> io::Result<()> {
    // SAFETY: `FIOCLEX` is a no-argument setter opcode.
    unsafe {
        let ctl = ioctl::NoArg::<ioctl::BadOpcode<{ c::FIOCLEX }>>::new();
        ioctl::ioctl(fd, ctl)
    }
}

/// `ioctl(fd, FIONBIO, &value)`—Enables or disables non-blocking mode.
///
/// # References
///  - [Winsock]
///  - [NetBSD]
///  - [OpenBSD]
///
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/winsock/winsock-ioctls#unix-ioctl-codes
/// [NetBSD]: https://man.netbsd.org/ioctl.2#GENERIC%20IOCTLS
/// [OpenBSD]: https://man.openbsd.org/ioctl.2#GENERIC_IOCTLS
#[inline]
#[doc(alias = "FIONBIO")]
pub fn ioctl_fionbio<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    // SAFETY: `FIONBIO` is a pointer setter opcode.
    unsafe {
        let ctl = ioctl::Setter::<ioctl::BadOpcode<{ c::FIONBIO }>, c::c_int>::new(value.into());
        ioctl::ioctl(fd, ctl)
    }
}

/// `ioctl(fd, FIONREAD)`—Returns the number of bytes ready to be read.
///
/// The result of this function gets silently coerced into a C `int` by the OS,
/// so it may contain a wrapped value.
///
/// # References
///  - [Linux]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/ioctl_tty.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/winsock/winsock-ioctls#unix-ioctl-codes
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=ioctl&sektion=2#GENERIC%09IOCTLS
/// [NetBSD]: https://man.netbsd.org/ioctl.2#GENERIC%20IOCTLS
/// [OpenBSD]: https://man.openbsd.org/ioctl.2#GENERIC_IOCTLS
#[cfg(not(any(target_os = "espidf", target_os = "horizon", target_os = "vita")))]
#[inline]
#[doc(alias = "FIONREAD")]
pub fn ioctl_fionread<Fd: AsFd>(fd: Fd) -> io::Result<u64> {
    // SAFETY: `FIONREAD` is a getter opcode that gets a `c_int`.
    unsafe {
        let ctl = ioctl::Getter::<ioctl::BadOpcode<{ c::FIONREAD }>, c::c_int>::new();
        ioctl::ioctl(fd, ctl).map(|n| n as u64)
    }
}
