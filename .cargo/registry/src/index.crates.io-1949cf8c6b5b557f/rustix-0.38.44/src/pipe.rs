//! `pipe` and related APIs.
//!
//! # Safety
//!
//! `vmsplice` is an unsafe function.

#![allow(unsafe_code)]

use crate::fd::OwnedFd;
use crate::{backend, io};
#[cfg(not(any(
    solarish,
    windows,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
use backend::c;
#[cfg(linux_kernel)]
use backend::fd::AsFd;

#[cfg(not(apple))]
pub use backend::pipe::types::PipeFlags;

#[cfg(linux_kernel)]
pub use backend::pipe::types::{IoSliceRaw, SpliceFlags};

/// `PIPE_BUF`—The maximum length at which writes to a pipe are atomic.
///
/// # References
///  - [Linux]
///  - [POSIX]
///
/// [Linux]: https://man7.org/linux/man-pages/man7/pipe.7.html
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/write.html
#[cfg(not(any(
    solarish,
    windows,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "hurd",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
pub const PIPE_BUF: usize = c::PIPE_BUF;

/// `pipe()`—Creates a pipe.
///
/// This function creates a pipe and returns two file descriptors, for the
/// reading and writing ends of the pipe, respectively.
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
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/pipe.html
/// [Linux]: https://man7.org/linux/man-pages/man2/pipe.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/pipe.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=pipe&sektion=2
/// [NetBSD]: https://man.netbsd.org/pipe.2
/// [OpenBSD]: https://man.openbsd.org/pipe.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=pipe&section=2
/// [illumos]: https://illumos.org/man/2/pipe
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Creating-a-Pipe.html
#[inline]
pub fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    backend::pipe::syscalls::pipe()
}

/// `pipe2(flags)`—Creates a pipe, with flags.
///
/// This function creates a pipe and returns two file descriptors, for the
/// reading and writing ends of the pipe, respectively.
///
/// # References
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/pipe2.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=pipe2&sektion=2
/// [NetBSD]: https://man.netbsd.org/pipe2.2
/// [OpenBSD]: https://man.openbsd.org/pipe2.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=pipe2&section=2
/// [illumos]: https://illumos.org/man/2/pipe2
#[cfg(not(any(
    apple,
    target_os = "aix",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "nto"
)))]
#[inline]
#[doc(alias = "pipe2")]
pub fn pipe_with(flags: PipeFlags) -> io::Result<(OwnedFd, OwnedFd)> {
    backend::pipe::syscalls::pipe_with(flags)
}

/// `splice(fd_in, off_in, fd_out, off_out, len, flags)`—Transfer data
/// between a file and a pipe.
///
/// This function transfers up to `len` bytes of data from the file descriptor
/// `fd_in` to the file descriptor `fd_out`, where one of the file descriptors
/// must refer to a pipe.
///
/// `off_*` must be `None` if the corresponding fd refers to a pipe. Otherwise
/// its value points to the starting offset to the file, from which the data is
/// read/written. On success, the number of bytes read/written is added to the
/// offset.
///
/// Passing `None` causes the read/write to start from the file offset, and the
/// file offset is adjusted appropriately.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/splice.2.html
#[cfg(linux_kernel)]
#[inline]
pub fn splice<FdIn: AsFd, FdOut: AsFd>(
    fd_in: FdIn,
    off_in: Option<&mut u64>,
    fd_out: FdOut,
    off_out: Option<&mut u64>,
    len: usize,
    flags: SpliceFlags,
) -> io::Result<usize> {
    backend::pipe::syscalls::splice(fd_in.as_fd(), off_in, fd_out.as_fd(), off_out, len, flags)
}

/// `vmsplice(fd, bufs, flags)`—Transfer data between memory and a pipe.
///
/// If `fd` is the write end of the pipe, the function maps the memory pointer
/// at by `bufs` to the pipe.
///
/// If `fd` is the read end of the pipe, the function writes data from the pipe
/// to said memory.
///
/// # Safety
///
/// If the memory must not be mutated (such as when `bufs` were originally
/// immutable slices), it is up to the caller to ensure that the write end of
/// the pipe is placed in `fd`.
///
/// Additionally if `SpliceFlags::GIFT` is set, the caller must also ensure
/// that the contents of `bufs` in never modified following the call, and that
/// all of the pointers in `bufs` are page aligned, and the lengths are
/// multiples of a page size in bytes.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/vmsplice.2.html
#[cfg(linux_kernel)]
#[inline]
pub unsafe fn vmsplice<PipeFd: AsFd>(
    fd: PipeFd,
    bufs: &[IoSliceRaw<'_>],
    flags: SpliceFlags,
) -> io::Result<usize> {
    backend::pipe::syscalls::vmsplice(fd.as_fd(), bufs, flags)
}

/// `tee(fd_in, fd_out, len, flags)`—Copy data between pipes without
/// consuming it.
///
/// This reads up to `len` bytes from `in_fd` without consuming them, and
/// writes them to `out_fd`.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/tee.2.html
#[cfg(linux_kernel)]
#[inline]
pub fn tee<FdIn: AsFd, FdOut: AsFd>(
    fd_in: FdIn,
    fd_out: FdOut,
    len: usize,
    flags: SpliceFlags,
) -> io::Result<usize> {
    backend::pipe::syscalls::tee(fd_in.as_fd(), fd_out.as_fd(), len, flags)
}

/// `fnctl(fd, F_GETPIPE_SZ)`—Return the buffer capacity of a pipe.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/fcntl.2.html
#[cfg(linux_kernel)]
#[inline]
pub fn fcntl_getpipe_size<Fd: AsFd>(fd: Fd) -> io::Result<usize> {
    backend::pipe::syscalls::fcntl_getpipe_size(fd.as_fd())
}

/// `fnctl(fd, F_SETPIPE_SZ)`—Set the buffer capacity of a pipe.
///
/// The OS may decide to use a larger size than `size`. To know the precise
/// size, call [`fcntl_getpipe_size`] after setting the size. In future
/// versions of rustix, this function will return the new size.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/fcntl.2.html
#[cfg(linux_kernel)]
#[inline]
pub fn fcntl_setpipe_size<Fd: AsFd>(fd: Fd, size: usize) -> io::Result<()> {
    backend::pipe::syscalls::fcntl_setpipe_size(fd.as_fd(), size)
}
