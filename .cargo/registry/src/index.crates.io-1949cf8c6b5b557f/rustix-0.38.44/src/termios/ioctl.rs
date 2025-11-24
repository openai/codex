//! Terminal-related `ioctl` functions.

#![allow(unsafe_code)]

use crate::fd::AsFd;
use crate::{backend, io, ioctl};
use backend::c;

/// `ioctl(fd, TIOCEXCL)`—Enables exclusive mode on a terminal.
///
/// # References
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///
/// [Linux]: https://man7.org/linux/man-pages/man4/tty_ioctl.4.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=tty&sektion=4
/// [NetBSD]: https://man.netbsd.org/tty.4
/// [OpenBSD]: https://man.openbsd.org/tty.4
#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
#[inline]
#[doc(alias = "TIOCEXCL")]
pub fn ioctl_tiocexcl<Fd: AsFd>(fd: Fd) -> io::Result<()> {
    // SAFETY: `TIOCEXCL` is a no-argument setter opcode.
    unsafe {
        let ctl = ioctl::NoArg::<ioctl::BadOpcode<{ c::TIOCEXCL as _ }>>::new();
        ioctl::ioctl(fd, ctl)
    }
}

/// `ioctl(fd, TIOCNXCL)`—Disables exclusive mode on a terminal.
///
/// # References
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///
/// [Linux]: https://man7.org/linux/man-pages/man4/tty_ioctl.4.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=tty&sektion=4
/// [NetBSD]: https://man.netbsd.org/tty.4
/// [OpenBSD]: https://man.openbsd.org/tty.4
#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
#[inline]
#[doc(alias = "TIOCNXCL")]
pub fn ioctl_tiocnxcl<Fd: AsFd>(fd: Fd) -> io::Result<()> {
    // SAFETY: `TIOCNXCL` is a no-argument setter opcode.
    unsafe {
        let ctl = ioctl::NoArg::<ioctl::BadOpcode<{ c::TIOCNXCL as _ }>>::new();
        ioctl::ioctl(fd, ctl)
    }
}
