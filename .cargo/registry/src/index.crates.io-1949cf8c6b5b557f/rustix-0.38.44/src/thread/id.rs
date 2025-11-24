use crate::{backend, io};

pub use crate::pid::{Pid, RawPid};
pub use crate::ugid::{Gid, RawGid, RawUid, Uid};

/// `gettid()`—Returns the thread ID.
///
/// This returns the OS thread ID, which is not necessarily the same as the
/// `rust::thread::Thread::id` or the pthread ID.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/gettid.2.html
#[inline]
#[must_use]
pub fn gettid() -> Pid {
    backend::thread::syscalls::gettid()
}

/// `setuid(uid)`
///
/// # Warning
///
/// This is not the setxid you are looking for… POSIX requires xids to be
/// process granular, but on Linux they are per-thread. Thus, this call only
/// changes the xid for the current *thread*, not the entire process even
/// though that is in violation of the POSIX standard.
///
/// For details on this distinction, see the C library vs. kernel differences
/// in the [manual page][linux_notes]. This call implements the kernel
/// behavior.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/setuid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/setuid.2.html
/// [linux_notes]: https://man7.org/linux/man-pages/man2/setuid.2.html#NOTES
#[inline]
pub fn set_thread_uid(uid: Uid) -> io::Result<()> {
    backend::thread::syscalls::setuid_thread(uid)
}

/// `setresuid(ruid, euid, suid)`
///
/// # Warning
///
/// This is not the setresxid you are looking for… POSIX requires xids to be
/// process granular, but on Linux they are per-thread. Thus, this call only
/// changes the xid for the current *thread*, not the entire process even
/// though that is in violation of the POSIX standard.
///
/// For details on this distinction, see the C library vs. kernel differences
/// in the [manual page][linux_notes] and the notes in [`set_thread_uid`]. This
/// call implements the kernel behavior.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/setresuid.2.html
/// [linux_notes]: https://man7.org/linux/man-pages/man2/setresuid.2.html#NOTES
#[inline]
pub fn set_thread_res_uid(ruid: Uid, euid: Uid, suid: Uid) -> io::Result<()> {
    backend::thread::syscalls::setresuid_thread(ruid, euid, suid)
}

/// `setgid(gid)`
///
/// # Warning
///
/// This is not the setxid you are looking for… POSIX requires xids to be
/// process granular, but on Linux they are per-thread. Thus, this call only
/// changes the xid for the current *thread*, not the entire process even
/// though that is in violation of the POSIX standard.
///
/// For details on this distinction, see the C library vs. kernel differences
/// in the [manual page][linux_notes]. This call implements the kernel
/// behavior.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/setgid.html
/// [Linux]: https://man7.org/linux/man-pages/man2/setgid.2.html
/// [linux_notes]: https://man7.org/linux/man-pages/man2/setgid.2.html#NOTES
#[inline]
pub fn set_thread_gid(gid: Gid) -> io::Result<()> {
    backend::thread::syscalls::setgid_thread(gid)
}

/// `setresgid(rgid, egid, sgid)`
///
/// # Warning
///
/// This is not the setresxid you are looking for… POSIX requires xids to be
/// process granular, but on Linux they are per-thread. Thus, this call only
/// changes the xid for the current *thread*, not the entire process even
/// though that is in violation of the POSIX standard.
///
/// For details on this distinction, see the C library vs. kernel differences
/// in the [manual page][linux_notes] and the notes in [`set_thread_gid`]. This
/// call implements the kernel behavior.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/setresgid.2.html
/// [linux_notes]: https://man7.org/linux/man-pages/man2/setresgid.2.html#NOTES
#[inline]
pub fn set_thread_res_gid(rgid: Gid, egid: Gid, sgid: Gid) -> io::Result<()> {
    backend::thread::syscalls::setresgid_thread(rgid, egid, sgid)
}

/// `setgroups(groups)`-Sets the supplementary group IDs for the calling
/// thread.
///
/// # Warning
///
/// This is not the `setgroups` you are looking for… POSIX requires gids to be
/// process granular, but on Linux they are per-thread. Thus, this call only
/// changes the gids for the current *thread*, not the entire process even
/// though that is in violation of the POSIX standard.
///
/// For details on this distinction, see the C library vs. kernel differences
/// in the [manual page][linux_notes]. This call implements the kernel
/// behavior.
///
/// # References
/// - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/setgroups.2.html
/// [linux_notes]: https://man7.org/linux/man-pages/man2/setgroups.2.html#NOTES
#[cfg(linux_kernel)]
#[inline]
pub fn set_thread_groups(groups: &[Gid]) -> io::Result<()> {
    backend::thread::syscalls::setgroups_thread(groups)
}
