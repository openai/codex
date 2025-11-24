use crate::{backend, io};

pub use crate::timespec::{Nsecs, Secs, Timespec};

/// `clockid_t`
#[cfg(not(target_os = "wasi"))]
pub use crate::clockid::{ClockId, DynamicClockId};

/// `clock_getres(id)`—Returns the resolution of a clock.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html
/// [Linux]: https://man7.org/linux/man-pages/man2/clock_getres.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=clock_getres&sektion=2
/// [NetBSD]: https://man.netbsd.org/clock_getres.2
/// [OpenBSD]: https://man.openbsd.org/clock_getres.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=clock_getres&section=2
/// [illumos]: https://illumos.org/man/3C/clock_getres
#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
#[inline]
#[must_use]
pub fn clock_getres(id: ClockId) -> Timespec {
    backend::time::syscalls::clock_getres(id)
}

/// `clock_gettime(id)`—Returns the current value of a clock.
///
/// This function uses `ClockId` which only contains clocks which are known to
/// always be supported at runtime, allowing this function to be infallible.
/// For a greater set of clocks and dynamic clock support, see
/// [`clock_gettime_dynamic`].
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_gettime.html
/// [Linux]: https://man7.org/linux/man-pages/man2/clock_gettime.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=clock_getres&sektion=2
/// [NetBSD]: https://man.netbsd.org/clock_getres.2
/// [OpenBSD]: https://man.openbsd.org/clock_getres.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=clock_getres&section=2
/// [illumos]: https://illumos.org/man/3C/clock_gettime
#[cfg(not(target_os = "wasi"))]
#[inline]
#[must_use]
pub fn clock_gettime(id: ClockId) -> Timespec {
    backend::time::syscalls::clock_gettime(id)
}

/// Like [`clock_gettime`] but with support for dynamic clocks.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_gettime.html
/// [Linux]: https://man7.org/linux/man-pages/man2/clock_gettime.2.html
#[cfg(not(target_os = "wasi"))]
#[inline]
pub fn clock_gettime_dynamic(id: DynamicClockId<'_>) -> io::Result<Timespec> {
    backend::time::syscalls::clock_gettime_dynamic(id)
}

/// `clock_settime(id, timespec)`—Sets the current value of a settable clock.
///
/// This fails with [`io::Errno::INVAL`] if the clock is not settable, and
/// [`io::Errno::ACCESS`] if the current process does not have permission to
/// set it.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_settime.html
/// [Linux]: https://man7.org/linux/man-pages/man2/clock_settime.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=clock_settime&sektion=2
/// [NetBSD]: https://man.netbsd.org/clock_settime.2
/// [OpenBSD]: https://man.openbsd.org/clock_settime.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=clock_settime&section=2
/// [illumos]: https://illumos.org/man/3C/clock_settime
#[cfg(not(any(
    target_os = "redox",
    target_os = "wasi",
    all(apple, not(target_os = "macos"))
)))]
#[inline]
pub fn clock_settime(id: ClockId, timespec: Timespec) -> io::Result<()> {
    backend::time::syscalls::clock_settime(id, timespec)
}
