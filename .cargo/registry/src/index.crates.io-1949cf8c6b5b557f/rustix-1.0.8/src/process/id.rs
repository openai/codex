//! Unix user, group, and process identifiers.
//!
//! # Safety
//!
//! The `Uid`, `Gid`, and `Pid` types can be constructed from raw integers,
//! which is marked unsafe because actual OS's assign special meaning to some
//! integer values.
#![allow(unsafe_code)]

use crate::{backend, io};
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub use crate::pid::{Pid, RawPid};
pub use crate::ugid::{Gid, RawGid, RawUid, Uid};

/// `getuid()`—Returns the process' real user ID.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [illumos]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getuid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getuid.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=getuid&sektion=2
/// [illumos]: https://www.illumos.org/man/2/getuid
/// [NetBSD]: https://man.netbsd.org/getuid.2
#[inline]
#[must_use]
pub fn getuid() -> Uid {
    backend::ugid::syscalls::getuid()
}

/// `geteuid()`—Returns the process' effective user ID.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [illumos]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/geteuid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/geteuid.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=geteuid&sektion=2
/// [illumos]: https://www.illumos.org/man/2/geteuid
/// [NetBSD]: https://man.netbsd.org/geteuid.2
#[inline]
#[must_use]
pub fn geteuid() -> Uid {
    backend::ugid::syscalls::geteuid()
}

/// `getgid()`—Returns the process' real group ID.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [illumos]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getgid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getgid.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=getgid&sektion=2
/// [illumos]: https://www.illumos.org/man/2/getgid
/// [NetBSD]: https://man.netbsd.org/getgid.2
#[inline]
#[must_use]
pub fn getgid() -> Gid {
    backend::ugid::syscalls::getgid()
}

/// `getegid()`—Returns the process' effective group ID.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [illumos]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getegid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getegid.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=getegid&sektion=2
/// [illumos]: https://www.illumos.org/man/2/getegid
/// [NetBSD]: https://man.netbsd.org/getegid.2
#[inline]
#[must_use]
pub fn getegid() -> Gid {
    backend::ugid::syscalls::getegid()
}

/// `getpid()`—Returns the process' ID.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [illumos]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getpid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getpid.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=getpid&sektion=2
/// [illumos]: https://www.illumos.org/man/2/getpid
/// [NetBSD]: https://man.netbsd.org/getpid.2
#[inline]
#[must_use]
pub fn getpid() -> Pid {
    backend::pid::syscalls::getpid()
}

/// `getppid()`—Returns the parent process' ID.
///
/// This will return `None` if the current process has no parent (or no parent
/// accessible in the current PID namespace), such as if the current process is
/// the init process ([`Pid::INIT`]).
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [illumos]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getppid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getppid.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=getppid&sektion=2
/// [illumos]: https://www.illumos.org/man/2/getppid
/// [NetBSD]: https://man.netbsd.org/getppid.2
#[inline]
#[must_use]
pub fn getppid() -> Option<Pid> {
    backend::process::syscalls::getppid()
}

/// `getpgid(pid)`—Returns the process group ID of the given process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [illumos]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getpgid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getpgid.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=getpgid&sektion=2
/// [illumos]: https://www.illumos.org/man/2/getpgid
/// [NetBSD]: https://man.netbsd.org/getpgid.2
#[inline]
pub fn getpgid(pid: Option<Pid>) -> io::Result<Pid> {
    backend::process::syscalls::getpgid(pid)
}

/// `setpgid(pid, pgid)`—Sets the process group ID of the given process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [illumos]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/setpgid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/setpgid.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=setpgid&sektion=2
/// [illumos]: https://www.illumos.org/man/2/setpgid
/// [NetBSD]: https://man.netbsd.org/setpgid.2
#[inline]
pub fn setpgid(pid: Option<Pid>, pgid: Option<Pid>) -> io::Result<()> {
    backend::process::syscalls::setpgid(pid, pgid)
}

/// `getpgrp()`—Returns the process' group ID.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [illumos]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getpgrp.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getpgrp.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=getpgrp&sektion=2
/// [illumos]: https://www.illumos.org/man/2/getpgrp
/// [NetBSD]: https://man.netbsd.org/getpgrp.2
#[inline]
#[must_use]
pub fn getpgrp() -> Pid {
    backend::process::syscalls::getpgrp()
}

/// `getsid(pid)`—Get the session ID of the given process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [illumos]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getsid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getsid.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=getsid&sektion=2
/// [illumos]: https://www.illumos.org/man/2/getsid
/// [NetBSD]: https://man.netbsd.org/getsid.2
#[cfg(not(target_os = "redox"))]
#[inline]
pub fn getsid(pid: Option<Pid>) -> io::Result<Pid> {
    backend::process::syscalls::getsid(pid)
}

/// `setsid()`—Create a new session.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [illumos]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/setsid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/setsid.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=setsid&sektion=2
/// [illumos]: https://www.illumos.org/man/2/setsid
/// [NetBSD]: https://man.netbsd.org/setsid.2
#[inline]
pub fn setsid() -> io::Result<Pid> {
    backend::process::syscalls::setsid()
}

/// `getgroups()`—Return a list of the current user's groups.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getgroups.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getgroups.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=getgroups&sektion=2
/// [NetBSD]: https://man.netbsd.org/getgroups.2
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub fn getgroups() -> io::Result<Vec<Gid>> {
    // This code would benefit from having a better way to read into
    // uninitialized memory, but that requires `unsafe`.
    let mut buffer = Vec::with_capacity(0);
    let ngroups = backend::process::syscalls::getgroups(&mut buffer)?;
    buffer.resize(ngroups, Gid::ROOT);
    backend::process::syscalls::getgroups(&mut buffer)?;
    Ok(buffer)
}
