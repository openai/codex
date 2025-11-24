//! POSIX-style `*at` functions.
//!
//! The `dirfd` argument to these functions may be a file descriptor for a
//! directory, the special value [`CWD`], or the special value [`ABS`].
//!
//! [`CWD`]: crate::fs::CWD
//! [`ABS`]: crate::fs::ABS

#![allow(unsafe_code)]

use crate::buffer::Buffer;
use crate::fd::OwnedFd;
#[cfg(not(any(target_os = "espidf", target_os = "horizon", target_os = "vita")))]
use crate::fs::Access;
#[cfg(not(target_os = "espidf"))]
use crate::fs::AtFlags;
#[cfg(apple)]
use crate::fs::CloneFlags;
#[cfg(any(linux_kernel, apple))]
use crate::fs::RenameFlags;
#[cfg(not(target_os = "espidf"))]
use crate::fs::Stat;
#[cfg(not(any(apple, target_os = "espidf", target_os = "vita", target_os = "wasi")))]
use crate::fs::{Dev, FileType};
#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
use crate::fs::{Gid, Uid};
use crate::fs::{Mode, OFlags};
use crate::{backend, io, path};
use backend::fd::AsFd;
#[cfg(feature = "alloc")]
use {
    crate::ffi::{CStr, CString},
    crate::path::SMALL_PATH_BUFFER_SIZE,
    alloc::vec::Vec,
    backend::fd::BorrowedFd,
};
#[cfg(not(any(target_os = "espidf", target_os = "vita")))]
use {crate::fs::Timestamps, crate::timespec::Nsecs};

/// `UTIME_NOW` for use with [`utimensat`].
///
/// [`utimensat`]: crate::fs::utimensat
#[cfg(not(any(
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
pub const UTIME_NOW: Nsecs = backend::c::UTIME_NOW as Nsecs;

/// `UTIME_OMIT` for use with [`utimensat`].
///
/// [`utimensat`]: crate::fs::utimensat
#[cfg(not(any(
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
pub const UTIME_OMIT: Nsecs = backend::c::UTIME_OMIT as Nsecs;

/// `openat(dirfd, path, oflags, mode)`—Opens a file.
///
/// POSIX guarantees that `openat` will use the lowest unused file descriptor,
/// however it is not safe in general to rely on this, as file descriptors may
/// be unexpectedly allocated on other threads or in libraries.
///
/// The `Mode` argument is only significant when creating a file.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/openat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/openat.2.html
#[inline]
pub fn openat<P: path::Arg, Fd: AsFd>(
    dirfd: Fd,
    path: P,
    oflags: OFlags,
    create_mode: Mode,
) -> io::Result<OwnedFd> {
    path.into_with_c_str(|path| {
        backend::fs::syscalls::openat(dirfd.as_fd(), path, oflags, create_mode)
    })
}

/// `readlinkat(fd, path)`—Reads the contents of a symlink.
///
/// If `reuse` already has available capacity, reuse it if possible.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/readlinkat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/readlinkat.2.html
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[inline]
pub fn readlinkat<P: path::Arg, Fd: AsFd, B: Into<Vec<u8>>>(
    dirfd: Fd,
    path: P,
    reuse: B,
) -> io::Result<CString> {
    path.into_with_c_str(|path| _readlinkat(dirfd.as_fd(), path, reuse.into()))
}

#[cfg(feature = "alloc")]
#[allow(unsafe_code)]
fn _readlinkat(dirfd: BorrowedFd<'_>, path: &CStr, mut buffer: Vec<u8>) -> io::Result<CString> {
    buffer.clear();
    buffer.reserve(SMALL_PATH_BUFFER_SIZE);

    loop {
        let buf = buffer.spare_capacity_mut();

        // SAFETY: `readlinkat` behaves.
        let nread = unsafe {
            backend::fs::syscalls::readlinkat(
                dirfd.as_fd(),
                path,
                (buf.as_mut_ptr().cast(), buf.len()),
            )?
        };

        debug_assert!(nread <= buffer.capacity());
        if nread < buffer.capacity() {
            // SAFETY: From the [documentation]: “On success, these calls
            // return the number of bytes placed in buf.”
            //
            // [documentation]: https://man7.org/linux/man-pages/man2/readlinkat.2.html
            unsafe {
                buffer.set_len(nread);
            }

            // SAFETY:
            // - “readlink places the contents of the symbolic link pathname
            //   in the buffer buf”
            // - [POSIX definition 3.271: Pathname]: “A string that is used
            //   to identify a file.”
            // - [POSIX definition 3.375: String]: “A contiguous sequence of
            //   bytes terminated by and including the first null byte.”
            // - “readlink does not append a terminating null byte to buf.”
            //
            // Thus, there will be no NUL bytes in the string.
            //
            // [POSIX definition 3.271: Pathname]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap03.html#tag_03_271
            // [POSIX definition 3.375: String]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap03.html#tag_03_375
            unsafe {
                return Ok(CString::from_vec_unchecked(buffer));
            }
        }

        // Use `Vec` reallocation strategy to grow capacity exponentially.
        buffer.reserve(buffer.capacity() + 1);
    }
}

/// `readlinkat(fd, path)`—Reads the contents of a symlink, without
/// allocating.
///
/// This is the "raw" version which avoids allocating, but which truncates the
/// string if it doesn't fit in the provided buffer, and doesn't NUL-terminate
/// the string.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/readlinkat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/readlinkat.2.html
#[inline]
pub fn readlinkat_raw<P: path::Arg, Fd: AsFd, Buf: Buffer<u8>>(
    dirfd: Fd,
    path: P,
    mut buf: Buf,
) -> io::Result<Buf::Output> {
    // SAFETY: `readlinkat` behaves.
    let len = path.into_with_c_str(|path| unsafe {
        backend::fs::syscalls::readlinkat(dirfd.as_fd(), path, buf.parts_mut())
    })?;
    // SAFETY: `readlinkat` behaves.
    unsafe { Ok(buf.assume_init(len)) }
}

/// `mkdirat(fd, path, mode)`—Creates a directory.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/mkdirat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/mkdirat.2.html
#[inline]
pub fn mkdirat<P: path::Arg, Fd: AsFd>(dirfd: Fd, path: P, mode: Mode) -> io::Result<()> {
    path.into_with_c_str(|path| backend::fs::syscalls::mkdirat(dirfd.as_fd(), path, mode))
}

/// `linkat(old_dirfd, old_path, new_dirfd, new_path, flags)`—Creates a hard
/// link.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/linkat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/linkat.2.html
#[cfg(not(target_os = "espidf"))]
#[inline]
pub fn linkat<P: path::Arg, Q: path::Arg, PFd: AsFd, QFd: AsFd>(
    old_dirfd: PFd,
    old_path: P,
    new_dirfd: QFd,
    new_path: Q,
    flags: AtFlags,
) -> io::Result<()> {
    old_path.into_with_c_str(|old_path| {
        new_path.into_with_c_str(|new_path| {
            backend::fs::syscalls::linkat(
                old_dirfd.as_fd(),
                old_path,
                new_dirfd.as_fd(),
                new_path,
                flags,
            )
        })
    })
}

/// `unlinkat(fd, path, flags)`—Unlinks a file or remove a directory.
///
/// With the [`REMOVEDIR`] flag, this removes a directory. This is in place of
/// a `rmdirat` function.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [`REMOVEDIR`]: AtFlags::REMOVEDIR
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/unlinkat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/unlinkat.2.html
#[cfg(not(target_os = "espidf"))]
#[inline]
pub fn unlinkat<P: path::Arg, Fd: AsFd>(dirfd: Fd, path: P, flags: AtFlags) -> io::Result<()> {
    path.into_with_c_str(|path| backend::fs::syscalls::unlinkat(dirfd.as_fd(), path, flags))
}

/// `renameat(old_dirfd, old_path, new_dirfd, new_path)`—Renames a file or
/// directory.
///
/// See [`renameat_with`] to pass additional flags.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/renameat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/renameat.2.html
#[inline]
pub fn renameat<P: path::Arg, Q: path::Arg, PFd: AsFd, QFd: AsFd>(
    old_dirfd: PFd,
    old_path: P,
    new_dirfd: QFd,
    new_path: Q,
) -> io::Result<()> {
    old_path.into_with_c_str(|old_path| {
        new_path.into_with_c_str(|new_path| {
            backend::fs::syscalls::renameat(
                old_dirfd.as_fd(),
                old_path,
                new_dirfd.as_fd(),
                new_path,
            )
        })
    })
}

/// `renameat2(old_dirfd, old_path, new_dirfd, new_path, flags)`—Renames a
/// file or directory.
///
/// `renameat_with` is the same as [`renameat`] but adds an additional
/// flags operand.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/renameat2.2.html
#[cfg(any(apple, linux_kernel))]
#[inline]
#[doc(alias = "renameat2")]
#[doc(alias = "renameatx_np")]
pub fn renameat_with<P: path::Arg, Q: path::Arg, PFd: AsFd, QFd: AsFd>(
    old_dirfd: PFd,
    old_path: P,
    new_dirfd: QFd,
    new_path: Q,
    flags: RenameFlags,
) -> io::Result<()> {
    old_path.into_with_c_str(|old_path| {
        new_path.into_with_c_str(|new_path| {
            backend::fs::syscalls::renameat2(
                old_dirfd.as_fd(),
                old_path,
                new_dirfd.as_fd(),
                new_path,
                flags,
            )
        })
    })
}

/// `symlinkat(old_path, new_dirfd, new_path)`—Creates a symlink.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/symlinkat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/symlinkat.2.html
#[inline]
pub fn symlinkat<P: path::Arg, Q: path::Arg, Fd: AsFd>(
    old_path: P,
    new_dirfd: Fd,
    new_path: Q,
) -> io::Result<()> {
    old_path.into_with_c_str(|old_path| {
        new_path.into_with_c_str(|new_path| {
            backend::fs::syscalls::symlinkat(old_path, new_dirfd.as_fd(), new_path)
        })
    })
}

/// `fstatat(dirfd, path, flags)`—Queries metadata for a file or directory.
///
/// [`Mode::from_raw_mode`] and [`FileType::from_raw_mode`] may be used to
/// interpret the `st_mode` field.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fstatat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fstatat.2.html
/// [`Mode::from_raw_mode`]: crate::fs::Mode::from_raw_mode
/// [`FileType::from_raw_mode`]: crate::fs::FileType::from_raw_mode
#[cfg(not(target_os = "espidf"))]
#[inline]
#[doc(alias = "fstatat")]
pub fn statat<P: path::Arg, Fd: AsFd>(dirfd: Fd, path: P, flags: AtFlags) -> io::Result<Stat> {
    path.into_with_c_str(|path| backend::fs::syscalls::statat(dirfd.as_fd(), path, flags))
}

/// `faccessat(dirfd, path, access, flags)`—Tests permissions for a file or
/// directory.
///
/// On Linux before 5.8, this function uses the `faccessat` system call which
/// doesn't support any flags. This function emulates support for the
/// [`AtFlags::EACCESS`] flag by checking whether the uid and gid of the
/// process match the effective uid and gid, in which case the `EACCESS` flag
/// can be ignored. In Linux 5.8 and beyond `faccessat2` is used, which
/// supports flags.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/faccessat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/faccessat.2.html
#[cfg(not(any(target_os = "espidf", target_os = "horizon", target_os = "vita")))]
#[inline]
#[doc(alias = "faccessat")]
pub fn accessat<P: path::Arg, Fd: AsFd>(
    dirfd: Fd,
    path: P,
    access: Access,
    flags: AtFlags,
) -> io::Result<()> {
    path.into_with_c_str(|path| backend::fs::syscalls::accessat(dirfd.as_fd(), path, access, flags))
}

/// `utimensat(dirfd, path, times, flags)`—Sets file or directory timestamps.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/utimensat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/utimensat.2.html
#[cfg(not(any(target_os = "espidf", target_os = "horizon", target_os = "vita")))]
#[inline]
pub fn utimensat<P: path::Arg, Fd: AsFd>(
    dirfd: Fd,
    path: P,
    times: &Timestamps,
    flags: AtFlags,
) -> io::Result<()> {
    path.into_with_c_str(|path| backend::fs::syscalls::utimensat(dirfd.as_fd(), path, times, flags))
}

/// `fchmodat(dirfd, path, mode, flags)`—Sets file or directory permissions.
///
/// Platform support for flags varies widely, for example on Linux
/// [`AtFlags::SYMLINK_NOFOLLOW`] is not implemented and therefore
/// [`io::Errno::OPNOTSUPP`] will be returned.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fchmodat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fchmodat.2.html
#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
#[inline]
#[doc(alias = "fchmodat")]
pub fn chmodat<P: path::Arg, Fd: AsFd>(
    dirfd: Fd,
    path: P,
    mode: Mode,
    flags: AtFlags,
) -> io::Result<()> {
    path.into_with_c_str(|path| backend::fs::syscalls::chmodat(dirfd.as_fd(), path, mode, flags))
}

/// `fclonefileat(src, dst_dir, dst, flags)`—Efficiently copies between files.
///
/// # References
///  - [Apple]
///
/// [Apple]: https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man2/clonefile.2
#[cfg(apple)]
#[inline]
pub fn fclonefileat<Fd: AsFd, DstFd: AsFd, P: path::Arg>(
    src: Fd,
    dst_dir: DstFd,
    dst: P,
    flags: CloneFlags,
) -> io::Result<()> {
    dst.into_with_c_str(|dst| {
        backend::fs::syscalls::fclonefileat(src.as_fd(), dst_dir.as_fd(), dst, flags)
    })
}

/// `mknodat(dirfd, path, mode, dev)`—Creates special or normal files.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/mknodat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/mknodat.2.html
#[cfg(not(any(
    apple,
    target_os = "espidf",
    target_os = "horizon",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub fn mknodat<P: path::Arg, Fd: AsFd>(
    dirfd: Fd,
    path: P,
    file_type: FileType,
    mode: Mode,
    dev: Dev,
) -> io::Result<()> {
    path.into_with_c_str(|path| {
        backend::fs::syscalls::mknodat(dirfd.as_fd(), path, file_type, mode, dev)
    })
}

/// `fchownat(dirfd, path, owner, group, flags)`—Sets file or directory
/// ownership.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fchownat.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fchownat.2.html
#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
#[inline]
#[doc(alias = "fchownat")]
pub fn chownat<P: path::Arg, Fd: AsFd>(
    dirfd: Fd,
    path: P,
    owner: Option<Uid>,
    group: Option<Gid>,
    flags: AtFlags,
) -> io::Result<()> {
    path.into_with_c_str(|path| {
        backend::fs::syscalls::chownat(dirfd.as_fd(), path, owner, group, flags)
    })
}
