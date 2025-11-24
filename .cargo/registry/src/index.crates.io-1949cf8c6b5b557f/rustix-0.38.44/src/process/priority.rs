#[cfg(not(target_os = "espidf"))]
use crate::process::{Pid, Uid};
use crate::{backend, io};

/// `nice(inc)`—Adjust the scheduling priority of the current process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/nice.html
/// [Linux]: https://man7.org/linux/man-pages/man2/nice.2.html
#[inline]
pub fn nice(inc: i32) -> io::Result<i32> {
    backend::process::syscalls::nice(inc)
}

/// `getpriority(PRIO_USER, uid)`—Get the scheduling priority of the given
/// user.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getpriority.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getpriority.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/setpriority.2.html
#[cfg(not(target_os = "espidf"))]
#[inline]
#[doc(alias = "getpriority")]
pub fn getpriority_user(uid: Uid) -> io::Result<i32> {
    backend::process::syscalls::getpriority_user(uid)
}

/// `getpriority(PRIO_PGRP, gid)`—Get the scheduling priority of the given
/// process group.
///
/// A `pgid` of `None` means the process group of the calling process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getpriority.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getpriority.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/setpriority.2.html
#[cfg(not(target_os = "espidf"))]
#[inline]
#[doc(alias = "getpriority")]
pub fn getpriority_pgrp(pgid: Option<Pid>) -> io::Result<i32> {
    backend::process::syscalls::getpriority_pgrp(pgid)
}

/// `getpriority(PRIO_PROCESS, pid)`—Get the scheduling priority of the given
/// process.
///
/// A `pid` of `None` means the calling process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getpriority.html
/// [Linux]: https://man7.org/linux/man-pages/man2/getpriority.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/setpriority.2.html
#[cfg(not(target_os = "espidf"))]
#[inline]
#[doc(alias = "getpriority")]
pub fn getpriority_process(pid: Option<Pid>) -> io::Result<i32> {
    backend::process::syscalls::getpriority_process(pid)
}

/// `setpriority(PRIO_USER, uid)`—Get the scheduling priority of the given
/// user.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/setpriority.html
/// [Linux]: https://man7.org/linux/man-pages/man2/setpriority.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/setpriority.2.html
#[cfg(not(target_os = "espidf"))]
#[inline]
#[doc(alias = "setpriority")]
pub fn setpriority_user(uid: Uid, priority: i32) -> io::Result<()> {
    backend::process::syscalls::setpriority_user(uid, priority)
}

/// `setpriority(PRIO_PGRP, pgid)`—Get the scheduling priority of the given
/// process group.
///
/// A `pgid` of `None` means the process group of the calling process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/setpriority.html
/// [Linux]: https://man7.org/linux/man-pages/man2/setpriority.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/setpriority.2.html
#[cfg(not(target_os = "espidf"))]
#[inline]
#[doc(alias = "setpriority")]
pub fn setpriority_pgrp(pgid: Option<Pid>, priority: i32) -> io::Result<()> {
    backend::process::syscalls::setpriority_pgrp(pgid, priority)
}

/// `setpriority(PRIO_PROCESS, pid)`—Get the scheduling priority of the given
/// process.
///
/// A `pid` of `None` means the calling process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/setpriority.html
/// [Linux]: https://man7.org/linux/man-pages/man2/setpriority.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/setpriority.2.html
#[cfg(not(target_os = "espidf"))]
#[inline]
#[doc(alias = "setpriority")]
pub fn setpriority_process(pid: Option<Pid>, priority: i32) -> io::Result<()> {
    backend::process::syscalls::setpriority_process(pid, priority)
}
