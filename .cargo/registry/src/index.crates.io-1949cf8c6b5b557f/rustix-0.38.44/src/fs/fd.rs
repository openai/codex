//! Functions which operate on file descriptors.

#[cfg(not(target_os = "wasi"))]
use crate::fs::Mode;
#[cfg(not(target_os = "wasi"))]
use crate::fs::{Gid, Uid};
use crate::fs::{OFlags, SeekFrom, Timespec};
use crate::{backend, io};
use backend::fd::{AsFd, BorrowedFd};
#[cfg(not(any(
    netbsdlike,
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
)))]
use backend::fs::types::FallocateFlags;
#[cfg(not(any(
    target_os = "espidf",
    target_os = "solaris",
    target_os = "vita",
    target_os = "wasi"
)))]
use backend::fs::types::FlockOperation;
#[cfg(linux_kernel)]
use backend::fs::types::FsWord;
use backend::fs::types::Stat;
#[cfg(not(any(
    solarish,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "netbsd",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
use backend::fs::types::StatFs;
#[cfg(not(any(target_os = "haiku", target_os = "redox", target_os = "wasi")))]
use backend::fs::types::StatVfs;
use core::fmt;

/// Timestamps used by [`utimensat`] and [`futimens`].
///
/// [`utimensat`]: crate::fs::utimensat
/// [`futimens`]: crate::fs::futimens
// This is `repr(C)` and specifically laid out to match the representation used
// by `utimensat` and `futimens`, which expect 2-element arrays of timestamps.
#[repr(C)]
#[derive(Clone)]
pub struct Timestamps {
    /// The timestamp of the last access to a filesystem object.
    pub last_access: Timespec,

    /// The timestamp of the last modification of a filesystem object.
    pub last_modification: Timespec,
}

impl fmt::Debug for Timestamps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Timestamps")
            .field("last_access.tv_sec", &self.last_access.tv_sec)
            .field("last_access.tv_nsec", &self.last_access.tv_nsec)
            .field("last_modification.tv_sec", &self.last_modification.tv_sec)
            .field("last_modification.tv_nsec", &self.last_modification.tv_nsec)
            .finish()
    }
}

/// The filesystem magic number for procfs.
///
/// See [the `fstatfs` manual page] for more information.
///
/// [the `fstatfs` manual page]: https://man7.org/linux/man-pages/man2/fstatfs.2.html#DESCRIPTION
#[cfg(linux_kernel)]
pub const PROC_SUPER_MAGIC: FsWord = backend::c::PROC_SUPER_MAGIC as FsWord;

/// The filesystem magic number for NFS.
///
/// See [the `fstatfs` manual page] for more information.
///
/// [the `fstatfs` manual page]: https://man7.org/linux/man-pages/man2/fstatfs.2.html#DESCRIPTION
#[cfg(linux_kernel)]
pub const NFS_SUPER_MAGIC: FsWord = backend::c::NFS_SUPER_MAGIC as FsWord;

/// `lseek(fd, offset, whence)`—Repositions a file descriptor within a file.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/lseek.html
/// [Linux]: https://man7.org/linux/man-pages/man2/lseek.2.html
#[inline]
#[doc(alias = "lseek")]
pub fn seek<Fd: AsFd>(fd: Fd, pos: SeekFrom) -> io::Result<u64> {
    backend::fs::syscalls::seek(fd.as_fd(), pos)
}

/// `lseek(fd, 0, SEEK_CUR)`—Returns the current position within a file.
///
/// Return the current position of the file descriptor. This is a subset of
/// the functionality of `seek`, but this interface makes it easier for users
/// to declare their intent not to mutate any state.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/lseek.html
/// [Linux]: https://man7.org/linux/man-pages/man2/lseek.2.html
#[inline]
#[doc(alias = "lseek")]
pub fn tell<Fd: AsFd>(fd: Fd) -> io::Result<u64> {
    backend::fs::syscalls::tell(fd.as_fd())
}

/// `fchmod(fd, mode)`—Sets open file or directory permissions.
///
/// This implementation does not support [`OFlags::PATH`] file descriptors,
/// even on platforms where the host libc emulates it.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fchmod.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fchmod.2.html
#[cfg(not(target_os = "wasi"))]
#[inline]
pub fn fchmod<Fd: AsFd>(fd: Fd, mode: Mode) -> io::Result<()> {
    backend::fs::syscalls::fchmod(fd.as_fd(), mode)
}

/// `fchown(fd, owner, group)`—Sets open file or directory ownership.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fchown.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fchown.2.html
#[cfg(not(target_os = "wasi"))]
#[inline]
pub fn fchown<Fd: AsFd>(fd: Fd, owner: Option<Uid>, group: Option<Gid>) -> io::Result<()> {
    backend::fs::syscalls::fchown(fd.as_fd(), owner, group)
}

/// `fstat(fd)`—Queries metadata for an open file or directory.
///
/// [`Mode::from_raw_mode`] and [`FileType::from_raw_mode`] may be used to
/// interpret the `st_mode` field.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fstat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fstat.2.html
/// [`Mode::from_raw_mode`]: Mode::from_raw_mode
/// [`FileType::from_raw_mode`]: crate::fs::FileType::from_raw_mode
#[inline]
pub fn fstat<Fd: AsFd>(fd: Fd) -> io::Result<Stat> {
    backend::fs::syscalls::fstat(fd.as_fd())
}

/// `fstatfs(fd)`—Queries filesystem statistics for an open file or directory.
///
/// Compared to [`fstatvfs`], this function often provides more information,
/// though it's less portable.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/fstatfs.2.html
#[cfg(not(any(
    solarish,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "netbsd",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
#[inline]
pub fn fstatfs<Fd: AsFd>(fd: Fd) -> io::Result<StatFs> {
    backend::fs::syscalls::fstatfs(fd.as_fd())
}

/// `fstatvfs(fd)`—Queries filesystem statistics for an open file or
/// directory, POSIX version.
///
/// Compared to [`fstatfs`], this function often provides less information,
/// but it is more portable. But even so, filesystems are very diverse and not
/// all the fields are meaningful for every filesystem. And `f_fsid` doesn't
/// seem to have a clear meaning anywhere.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fstatvfs.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fstatvfs.2.html
#[cfg(not(any(target_os = "haiku", target_os = "redox", target_os = "wasi")))]
#[inline]
pub fn fstatvfs<Fd: AsFd>(fd: Fd) -> io::Result<StatVfs> {
    backend::fs::syscalls::fstatvfs(fd.as_fd())
}

/// `futimens(fd, times)`—Sets timestamps for an open file or directory.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/futimens.html
/// [Linux]: https://man7.org/linux/man-pages/man2/utimensat.2.html
#[cfg(not(any(target_os = "espidf", target_os = "vita")))]
#[inline]
pub fn futimens<Fd: AsFd>(fd: Fd, times: &Timestamps) -> io::Result<()> {
    backend::fs::syscalls::futimens(fd.as_fd(), times)
}

/// `fallocate(fd, mode, offset, len)`—Adjusts file allocation.
///
/// This is a more general form of `posix_fallocate`, adding a `mode` argument
/// which modifies the behavior. On platforms which only support
/// `posix_fallocate` and not the more general form, no `FallocateFlags` values
/// are defined so it will always be empty.
///
/// # References
///  - [POSIX]
///  - [Linux `fallocate`]
///  - [Linux `posix_fallocate`]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/posix_fallocate.html
/// [Linux `fallocate`]: https://man7.org/linux/man-pages/man2/fallocate.2.html
/// [Linux `posix_fallocate`]: https://man7.org/linux/man-pages/man3/posix_fallocate.3.html
#[cfg(not(any(
    netbsdlike,
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
)))] // not implemented in libc for netbsd yet
#[inline]
#[doc(alias = "posix_fallocate")]
pub fn fallocate<Fd: AsFd>(fd: Fd, mode: FallocateFlags, offset: u64, len: u64) -> io::Result<()> {
    backend::fs::syscalls::fallocate(fd.as_fd(), mode, offset, len)
}

/// `fcntl(fd, F_GETFL) & O_ACCMODE`
///
/// Returns a pair of booleans indicating whether the file descriptor is
/// readable and/or writable, respectively. This is only reliable on files; for
/// example, it doesn't reflect whether sockets have been shut down; for
/// general I/O handle support, use [`io::is_read_write`].
#[inline]
pub fn is_file_read_write<Fd: AsFd>(fd: Fd) -> io::Result<(bool, bool)> {
    _is_file_read_write(fd.as_fd())
}

pub(crate) fn _is_file_read_write(fd: BorrowedFd<'_>) -> io::Result<(bool, bool)> {
    let mode = backend::fs::syscalls::fcntl_getfl(fd)?;

    // Check for `O_PATH`.
    #[cfg(any(linux_kernel, target_os = "emscripten", target_os = "fuchsia"))]
    if mode.contains(OFlags::PATH) {
        return Ok((false, false));
    }

    // Use `RWMODE` rather than `ACCMODE` as `ACCMODE` may include `O_PATH`.
    // We handled `O_PATH` above.
    match mode & OFlags::RWMODE {
        OFlags::RDONLY => Ok((true, false)),
        OFlags::RDWR => Ok((true, true)),
        OFlags::WRONLY => Ok((false, true)),
        _ => unreachable!(),
    }
}

/// `fsync(fd)`—Ensures that file data and metadata is written to the
/// underlying storage device.
///
/// On iOS and macOS this isn't sufficient to ensure that data has reached
/// persistent storage; use [`fcntl_fullfsync`] to ensure that.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fsync.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fsync.2.html
/// [`fcntl_fullfsync`]: https://docs.rs/rustix/*/x86_64-apple-darwin/rustix/fs/fn.fcntl_fullfsync.html
#[inline]
pub fn fsync<Fd: AsFd>(fd: Fd) -> io::Result<()> {
    backend::fs::syscalls::fsync(fd.as_fd())
}

/// `fdatasync(fd)`—Ensures that file data is written to the underlying
/// storage device.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fdatasync.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fdatasync.2.html
#[cfg(not(any(
    apple,
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "redox",
    target_os = "vita",
)))]
#[inline]
pub fn fdatasync<Fd: AsFd>(fd: Fd) -> io::Result<()> {
    backend::fs::syscalls::fdatasync(fd.as_fd())
}

/// `ftruncate(fd, length)`—Sets the length of a file.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/ftruncate.html
/// [Linux]: https://man7.org/linux/man-pages/man2/ftruncate.2.html
#[inline]
pub fn ftruncate<Fd: AsFd>(fd: Fd, length: u64) -> io::Result<()> {
    backend::fs::syscalls::ftruncate(fd.as_fd(), length)
}

/// `flock(fd, operation)`—Acquire or release an advisory lock on an open file.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/flock.2.html
#[cfg(not(any(
    target_os = "espidf",
    target_os = "solaris",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub fn flock<Fd: AsFd>(fd: Fd, operation: FlockOperation) -> io::Result<()> {
    backend::fs::syscalls::flock(fd.as_fd(), operation)
}

/// `syncfs(fd)`—Flush cached filesystem data.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/syncfs.2.html
#[cfg(linux_kernel)]
#[inline]
pub fn syncfs<Fd: AsFd>(fd: Fd) -> io::Result<()> {
    backend::fs::syscalls::syncfs(fd.as_fd())
}
