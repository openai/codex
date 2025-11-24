//! POSIX-style filesystem functions which operate on bare paths.

use crate::fd::OwnedFd;
#[cfg(not(any(target_os = "espidf", target_os = "vita")))]
use crate::fs::Access;
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
use crate::fs::StatFs;
#[cfg(not(any(target_os = "haiku", target_os = "redox", target_os = "wasi")))]
use crate::fs::StatVfs;
use crate::fs::{Mode, OFlags, Stat};
#[cfg(not(target_os = "wasi"))]
use crate::ugid::{Gid, Uid};
use crate::{backend, io, path};
#[cfg(feature = "alloc")]
use {
    crate::ffi::{CStr, CString},
    crate::path::SMALL_PATH_BUFFER_SIZE,
    alloc::vec::Vec,
};

/// `open(path, oflags, mode)`—Opens a file.
///
/// POSIX guarantees that `open` will use the lowest unused file descriptor,
/// however it is not safe in general to rely on this, as file descriptors may
/// be unexpectedly allocated on other threads or in libraries.
///
/// The `Mode` argument is only significant when creating a file.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/open.html
/// [Linux]: https://man7.org/linux/man-pages/man2/open.2.html
#[inline]
pub fn open<P: path::Arg>(path: P, flags: OFlags, mode: Mode) -> io::Result<OwnedFd> {
    path.into_with_c_str(|path| backend::fs::syscalls::open(path, flags, mode))
}

/// `chmod(path, mode)`—Sets file or directory permissions.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/chmod.html
/// [Linux]: https://man7.org/linux/man-pages/man2/chmod.2.html
#[cfg(not(target_os = "wasi"))]
#[inline]
pub fn chmod<P: path::Arg>(path: P, mode: Mode) -> io::Result<()> {
    path.into_with_c_str(|path| backend::fs::syscalls::chmod(path, mode))
}

/// `stat(path)`—Queries metadata for a file or directory.
///
/// [`Mode::from_raw_mode`] and [`FileType::from_raw_mode`] may be used to
/// interpret the `st_mode` field.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/stat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/stat.2.html
/// [`Mode::from_raw_mode`]: crate::fs::Mode::from_raw_mode
/// [`FileType::from_raw_mode`]: crate::fs::FileType::from_raw_mode
#[inline]
pub fn stat<P: path::Arg>(path: P) -> io::Result<Stat> {
    path.into_with_c_str(backend::fs::syscalls::stat)
}

/// `lstat(path)`—Queries metadata for a file or directory, without following
/// symlinks.
///
/// [`Mode::from_raw_mode`] and [`FileType::from_raw_mode`] may be used to
/// interpret the `st_mode` field.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/lstat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/lstat.2.html
/// [`Mode::from_raw_mode`]: crate::fs::Mode::from_raw_mode
/// [`FileType::from_raw_mode`]: crate::fs::FileType::from_raw_mode
#[inline]
pub fn lstat<P: path::Arg>(path: P) -> io::Result<Stat> {
    path.into_with_c_str(backend::fs::syscalls::lstat)
}

/// `readlink(path)`—Reads the contents of a symlink.
///
/// If `reuse` is non-empty, reuse its buffer to store the result if possible.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/readlink.html
/// [Linux]: https://man7.org/linux/man-pages/man2/readlink.2.html
#[cfg(feature = "alloc")]
#[inline]
pub fn readlink<P: path::Arg, B: Into<Vec<u8>>>(path: P, reuse: B) -> io::Result<CString> {
    path.into_with_c_str(|path| _readlink(path, reuse.into()))
}

#[cfg(feature = "alloc")]
fn _readlink(path: &CStr, mut buffer: Vec<u8>) -> io::Result<CString> {
    // This code would benefit from having a better way to read into
    // uninitialized memory, but that requires `unsafe`.
    buffer.clear();
    buffer.reserve(SMALL_PATH_BUFFER_SIZE);
    buffer.resize(buffer.capacity(), 0_u8);

    loop {
        let nread = backend::fs::syscalls::readlink(path, &mut buffer)?;

        let nread = nread as usize;
        assert!(nread <= buffer.len());
        if nread < buffer.len() {
            buffer.resize(nread, 0_u8);
            return Ok(CString::new(buffer).unwrap());
        }
        // Use `Vec` reallocation strategy to grow capacity exponentially.
        buffer.reserve(1);
        buffer.resize(buffer.capacity(), 0_u8);
    }
}

/// `rename(old_path, new_path)`—Renames a file or directory.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/rename.html
/// [Linux]: https://man7.org/linux/man-pages/man2/rename.2.html
#[inline]
pub fn rename<P: path::Arg, Q: path::Arg>(old_path: P, new_path: Q) -> io::Result<()> {
    old_path.into_with_c_str(|old_path| {
        new_path.into_with_c_str(|new_path| backend::fs::syscalls::rename(old_path, new_path))
    })
}

/// `unlink(path)`—Unlinks a file.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/unlink.html
/// [Linux]: https://man7.org/linux/man-pages/man2/unlink.2.html
#[inline]
pub fn unlink<P: path::Arg>(path: P) -> io::Result<()> {
    path.into_with_c_str(backend::fs::syscalls::unlink)
}

/// `rmdir(path)`—Removes a directory.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/rmdir.html
/// [Linux]: https://man7.org/linux/man-pages/man2/rmdir.2.html
#[inline]
pub fn rmdir<P: path::Arg>(path: P) -> io::Result<()> {
    path.into_with_c_str(backend::fs::syscalls::rmdir)
}

/// `link(old_path, new_path)`—Creates a hard link.
///
/// POSIX leaves it implementation-defined whether `link` follows a symlink in
/// `old_path`, or creates a new link to the symbolic link itself. On platforms
/// which have it, [`linkat`] avoids this problem since it has an [`AtFlags`]
/// parameter and the [`AtFlags::SYMLINK_FOLLOW`] flag determines whether
/// symlinks should be followed.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/link.html
/// [Linux]: https://man7.org/linux/man-pages/man2/link.2.html
/// [`linkat`]: crate::fs::linkat
/// [`AtFlags`]: crate::fs::AtFlags
/// [`AtFlags::SYMLINK_FOLLOW`]: crate::fs::AtFlags::SYMLINK_FOLLOW
#[inline]
pub fn link<P: path::Arg, Q: path::Arg>(old_path: P, new_path: Q) -> io::Result<()> {
    old_path.into_with_c_str(|old_path| {
        new_path.into_with_c_str(|new_path| backend::fs::syscalls::link(old_path, new_path))
    })
}

/// `symlink(old_path, new_path)`—Creates a symlink.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/symlink.html
/// [Linux]: https://man7.org/linux/man-pages/man2/symlink.2.html
#[inline]
pub fn symlink<P: path::Arg, Q: path::Arg>(old_path: P, new_path: Q) -> io::Result<()> {
    old_path.into_with_c_str(|old_path| {
        new_path.into_with_c_str(|new_path| backend::fs::syscalls::symlink(old_path, new_path))
    })
}

/// `mkdir(path, mode)`—Creates a directory.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/mkdir.html
/// [Linux]: https://man7.org/linux/man-pages/man2/mkdir.2.html
#[inline]
pub fn mkdir<P: path::Arg>(path: P, mode: Mode) -> io::Result<()> {
    path.into_with_c_str(|path| backend::fs::syscalls::mkdir(path, mode))
}

/// `access(path, access)`—Tests permissions for a file or directory.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/access.html
/// [Linux]: https://man7.org/linux/man-pages/man2/access.2.html
#[cfg(not(any(target_os = "espidf", target_os = "vita")))]
#[inline]
pub fn access<P: path::Arg>(path: P, access: Access) -> io::Result<()> {
    path.into_with_c_str(|path| backend::fs::syscalls::access(path, access))
}

/// `statfs`—Queries filesystem metadata.
///
/// Compared to [`statvfs`], this function often provides more information,
/// though it's less portable.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/statfs.2.html
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
pub fn statfs<P: path::Arg>(path: P) -> io::Result<StatFs> {
    path.into_with_c_str(backend::fs::syscalls::statfs)
}

/// `statvfs`—Queries filesystem metadata, POSIX version.
///
/// Compared to [`statfs`], this function often provides less information, but
/// it is more portable. But even so, filesystems are very diverse and not all
/// the fields are meaningful for every filesystem. And `f_fsid` doesn't seem
/// to have a clear meaning anywhere.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/statvfs.html
/// [Linux]: https://man7.org/linux/man-pages/man2/statvfs.2.html
#[cfg(not(any(target_os = "haiku", target_os = "redox", target_os = "wasi")))]
#[inline]
pub fn statvfs<P: path::Arg>(path: P) -> io::Result<StatVfs> {
    path.into_with_c_str(backend::fs::syscalls::statvfs)
}

/// `chown(path, owner, group)`—Sets open file or directory ownership.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/chown.html
/// [Linux]: https://man7.org/linux/man-pages/man2/chown.2.html
#[cfg(not(target_os = "wasi"))]
#[inline]
pub fn chown<P: path::Arg>(path: P, owner: Option<Uid>, group: Option<Gid>) -> io::Result<()> {
    path.into_with_c_str(|path| backend::fs::syscalls::chown(path, owner, group))
}
