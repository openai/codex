//! CPU and thread identifiers.
//!
//! # Safety
//!
//! The `Cpuid`, type can be constructed from raw integers, which is marked
//! unsafe because actual OS's assign special meaning to some integer values.

#![allow(unsafe_code)]
use crate::{backend, io};
#[cfg(linux_kernel)]
use backend::thread::types::RawCpuid;

pub use crate::pid::{Pid, RawPid};
pub use crate::ugid::{Gid, RawGid, RawUid, Uid};

/// A Linux CPU ID.
#[cfg(linux_kernel)]
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct Cpuid(RawCpuid);

#[cfg(linux_kernel)]
impl Cpuid {
    /// Converts a `RawCpuid` into a `Cpuid`.
    ///
    /// # Safety
    ///
    /// `raw` must be the value of a valid Linux CPU ID.
    #[inline]
    pub const unsafe fn from_raw(raw: RawCpuid) -> Self {
        Self(raw)
    }

    /// Converts a `Cpuid` into a `RawCpuid`.
    #[inline]
    pub const fn as_raw(self) -> RawCpuid {
        self.0
    }
}

/// `gettid()`—Returns the thread ID.
///
/// This returns the OS thread ID, which is not necessarily the same as the
/// Rust's `std::thread::Thread::id` or the pthread ID.
///
/// This function always does a system call. To avoid this overhead, ask the
/// thread runtime for the ID instead, for example using [`libc::gettid`] or
/// [`origin::thread::current_id`].
///
/// [`libc::gettid`]: https://docs.rs/libc/*/libc/fn.gettid.html
/// [`origin::thread::current_id`]: https://docs.rs/origin/*/origin/thread/fn.current_id.html
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

/// `setuid(uid)`—Sets the effective user ID of the calling thread.
///
/// # Warning
///
/// This is not the `setuid` you are looking for… POSIX requires uids to be
/// process granular, but on Linux they are per-thread. Thus, this call only
/// changes the uid for the current *thread*, not the entire process even
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

/// `setresuid(ruid, euid, suid)`—Sets the real, effective, and saved user ID
/// of the calling thread.
///
/// # Warning
///
/// This is not the `setresuid` you are looking for… POSIX requires uids to be
/// process granular, but on Linux they are per-thread. Thus, this call only
/// changes the uid for the current *thread*, not the entire process even
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

/// `setgid(gid)`—Sets the effective group ID of the current thread.
///
/// # Warning
///
/// This is not the `setgid` you are looking for… POSIX requires gids to be
/// process granular, but on Linux they are per-thread. Thus, this call only
/// changes the gid for the current *thread*, not the entire process even
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

/// `setresgid(rgid, egid, sgid)`—Sets the real, effective, and saved group
/// ID of the current thread.
///
/// # Warning
///
/// This is not the `setresgid` you are looking for… POSIX requires gids to be
/// process granular, but on Linux they are per-thread. Thus, this call only
/// changes the gid for the current *thread*, not the entire process even
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

/// `setgroups(groups)`—Sets the supplementary group IDs for the calling
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
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/setgroups.2.html
/// [linux_notes]: https://man7.org/linux/man-pages/man2/setgroups.2.html#NOTES
#[cfg(linux_kernel)]
#[inline]
pub fn set_thread_groups(groups: &[Gid]) -> io::Result<()> {
    backend::thread::syscalls::setgroups_thread(groups)
}
