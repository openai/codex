//! `read` and `write`, optionally positioned, optionally vectored.

#![allow(unsafe_code)]

use crate::buffer::Buffer;
use crate::{backend, io};
use backend::fd::AsFd;

// Declare `IoSlice` and `IoSliceMut`.
#[cfg(not(windows))]
pub use crate::maybe_polyfill::io::{IoSlice, IoSliceMut};

#[cfg(linux_kernel)]
pub use backend::io::types::ReadWriteFlags;

/// `read(fd, buf)`—Reads from a stream.
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/read.html
/// [Linux]: https://man7.org/linux/man-pages/man2/read.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/read.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=read&sektion=2
/// [NetBSD]: https://man.netbsd.org/read.2
/// [OpenBSD]: https://man.openbsd.org/read.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=read&section=2
/// [illumos]: https://illumos.org/man/2/read
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/I_002fO-Primitives.html#index-reading-from-a-file-descriptor
#[inline]
pub fn read<Fd: AsFd, Buf: Buffer<u8>>(fd: Fd, mut buf: Buf) -> io::Result<Buf::Output> {
    // SAFETY: `read` behaves.
    let len = unsafe { backend::io::syscalls::read(fd.as_fd(), buf.parts_mut())? };
    // SAFETY: `read` behaves.
    unsafe { Ok(buf.assume_init(len)) }
}

/// `write(fd, buf)`—Writes to a stream.
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/write.html
/// [Linux]: https://man7.org/linux/man-pages/man2/write.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/write.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=write&sektion=2
/// [NetBSD]: https://man.netbsd.org/write.2
/// [OpenBSD]: https://man.openbsd.org/write.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=write&section=2
/// [illumos]: https://illumos.org/man/2/write
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/I_002fO-Primitives.html#index-writing-to-a-file-descriptor
#[inline]
pub fn write<Fd: AsFd>(fd: Fd, buf: &[u8]) -> io::Result<usize> {
    backend::io::syscalls::write(fd.as_fd(), buf)
}

/// `pread(fd, buf, offset)`—Reads from a file at a given position.
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/pread.html
/// [Linux]: https://man7.org/linux/man-pages/man2/pread.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/pread.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=pread&sektion=2
/// [NetBSD]: https://man.netbsd.org/pread.2
/// [OpenBSD]: https://man.openbsd.org/pread.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=pread&section=2
/// [illumos]: https://illumos.org/man/2/pread
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/I_002fO-Primitives.html#index-pread64
#[cfg(not(windows))]
#[inline]
pub fn pread<Fd: AsFd, Buf: Buffer<u8>>(
    fd: Fd,
    mut buf: Buf,
    offset: u64,
) -> io::Result<Buf::Output> {
    // SAFETY: `pread` behaves.
    let len = unsafe { backend::io::syscalls::pread(fd.as_fd(), buf.parts_mut(), offset)? };
    // SAFETY: `pread` behaves.
    unsafe { Ok(buf.assume_init(len)) }
}

/// `pwrite(fd, bufs)`—Writes to a file at a given position.
///
/// Contrary to POSIX, on many popular platforms including Linux and FreeBSD,
/// if the file is opened in append mode, this ignores the offset appends the
/// data to the end of the file.
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/pwrite.html
/// [Linux]: https://man7.org/linux/man-pages/man2/pwrite.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/pwrite.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=pwrite&sektion=2
/// [NetBSD]: https://man.netbsd.org/pwrite.2
/// [OpenBSD]: https://man.openbsd.org/pwrite.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=pwrite&section=2
/// [illumos]: https://illumos.org/man/2/pwrite
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/I_002fO-Primitives.html#index-pwrite64
#[cfg(not(windows))]
#[inline]
pub fn pwrite<Fd: AsFd>(fd: Fd, buf: &[u8], offset: u64) -> io::Result<usize> {
    backend::io::syscalls::pwrite(fd.as_fd(), buf, offset)
}

/// `readv(fd, bufs)`—Reads from a stream into multiple buffers.
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/readv.html
/// [Linux]: https://man7.org/linux/man-pages/man2/readv.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/readv.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=readv&sektion=2
/// [NetBSD]: https://man.netbsd.org/readv.2
/// [OpenBSD]: https://man.openbsd.org/readv.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=readv&section=2
/// [illumos]: https://illumos.org/man/2/readv
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Scatter_002dGather.html#index-readv
#[cfg(not(any(windows, target_os = "espidf", target_os = "horizon")))]
#[inline]
pub fn readv<Fd: AsFd>(fd: Fd, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
    backend::io::syscalls::readv(fd.as_fd(), bufs)
}

/// `writev(fd, bufs)`—Writes to a stream from multiple buffers.
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/writev.html
/// [Linux]: https://man7.org/linux/man-pages/man2/writev.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/writev.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=writev&sektion=2
/// [NetBSD]: https://man.netbsd.org/writev.2
/// [OpenBSD]: https://man.openbsd.org/writev.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=writev&section=2
/// [illumos]: https://illumos.org/man/2/writev
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Scatter_002dGather.html#index-writev
#[cfg(not(any(windows, target_os = "espidf", target_os = "horizon")))]
#[inline]
pub fn writev<Fd: AsFd>(fd: Fd, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
    backend::io::syscalls::writev(fd.as_fd(), bufs)
}

/// `preadv(fd, bufs, offset)`—Reads from a file at a given position into
/// multiple buffers.
///
/// # References
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/preadv.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=preadv&sektion=2
/// [NetBSD]: https://man.netbsd.org/preadv.2
/// [OpenBSD]: https://man.openbsd.org/preadv.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=preadv&section=2
/// [illumos]: https://illumos.org/man/2/preadv
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Scatter_002dGather.html#index-preadv64
#[cfg(not(any(
    windows,
    target_os = "cygwin",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "nto",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
)))]
#[inline]
pub fn preadv<Fd: AsFd>(fd: Fd, bufs: &mut [IoSliceMut<'_>], offset: u64) -> io::Result<usize> {
    backend::io::syscalls::preadv(fd.as_fd(), bufs, offset)
}

/// `pwritev(fd, bufs, offset)`—Writes to a file at a given position from
/// multiple buffers.
///
/// Contrary to POSIX, on many popular platforms including Linux and FreeBSD,
/// if the file is opened in append mode, this ignores the offset appends the
/// data to the end of the file.
///
/// # References
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/pwritev.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=pwritev&sektion=2
/// [NetBSD]: https://man.netbsd.org/pwritev.2
/// [OpenBSD]: https://man.openbsd.org/pwritev.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=pwritev&section=2
/// [illumos]: https://illumos.org/man/2/pwritev
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/I_002fO-Primitives.html#index-pwrite64
#[cfg(not(any(
    windows,
    target_os = "cygwin",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "nto",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
)))]
#[inline]
pub fn pwritev<Fd: AsFd>(fd: Fd, bufs: &[IoSlice<'_>], offset: u64) -> io::Result<usize> {
    backend::io::syscalls::pwritev(fd.as_fd(), bufs, offset)
}

/// `preadv2(fd, bufs, offset, flags)`—Reads data, with several options.
///
/// An `offset` of `u64::MAX` means to use and update the current file offset.
///
/// # References
///  - [Linux]
///  - [glibc]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/preadv2.2.html
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Scatter_002dGather.html#index-preadv64v2
#[cfg(linux_kernel)]
#[inline]
pub fn preadv2<Fd: AsFd>(
    fd: Fd,
    bufs: &mut [IoSliceMut<'_>],
    offset: u64,
    flags: ReadWriteFlags,
) -> io::Result<usize> {
    backend::io::syscalls::preadv2(fd.as_fd(), bufs, offset, flags)
}

/// `pwritev2(fd, bufs, offset, flags)`—Writes data, with several options.
///
/// An `offset` of `u64::MAX` means to use and update the current file offset.
///
/// # References
///  - [Linux]
///  - [glibc]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/pwritev2.2.html
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Scatter_002dGather.html#index-pwritev64v2
#[cfg(linux_kernel)]
#[inline]
pub fn pwritev2<Fd: AsFd>(
    fd: Fd,
    bufs: &[IoSlice<'_>],
    offset: u64,
    flags: ReadWriteFlags,
) -> io::Result<usize> {
    backend::io::syscalls::pwritev2(fd.as_fd(), bufs, offset, flags)
}
