use crate::backend;

/// `pause()`
///
/// The POSIX `pause` interface returns an error code, but the only thing
/// `pause` does is sleep until interrupted by a signal, so it always
/// returns the same thing with the same error code, so in rustix, the
/// return value is omitted.
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
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/pause.html
/// [Linux]: https://man7.org/linux/man-pages/man2/pause.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/pause.3.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=pause&sektion=3
/// [NetBSD]: https://man.netbsd.org/pause.3
/// [OpenBSD]: https://man.openbsd.org/pause.3
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=pause&section=3
/// [illumos]: https://illumos.org/man/2/pause
#[inline]
pub fn pause() {
    backend::event::syscalls::pause()
}
