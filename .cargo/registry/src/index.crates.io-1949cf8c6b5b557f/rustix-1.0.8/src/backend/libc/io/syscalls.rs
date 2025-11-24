//! libc syscalls supporting `rustix::io`.

use crate::backend::c;
#[cfg(not(target_os = "wasi"))]
use crate::backend::conv::ret_discarded_fd;
use crate::backend::conv::{borrowed_fd, ret, ret_c_int, ret_owned_fd, ret_usize};
use crate::fd::{AsFd as _, BorrowedFd, OwnedFd, RawFd};
#[cfg(not(any(
    target_os = "aix",
    target_os = "espidf",
    target_os = "nto",
    target_os = "vita",
    target_os = "wasi"
)))]
use crate::io::DupFlags;
#[cfg(linux_kernel)]
use crate::io::ReadWriteFlags;
use crate::io::{self, FdFlags};
use crate::ioctl::{IoctlOutput, Opcode};
use core::cmp::min;
#[cfg(not(any(target_os = "espidf", target_os = "horizon")))]
use {
    crate::backend::MAX_IOV,
    crate::io::{IoSlice, IoSliceMut},
};

pub(crate) unsafe fn read(fd: BorrowedFd<'_>, buf: (*mut u8, usize)) -> io::Result<usize> {
    ret_usize(c::read(
        borrowed_fd(fd),
        buf.0.cast(),
        min(buf.1, READ_LIMIT),
    ))
}

pub(crate) fn write(fd: BorrowedFd<'_>, buf: &[u8]) -> io::Result<usize> {
    unsafe {
        ret_usize(c::write(
            borrowed_fd(fd),
            buf.as_ptr().cast(),
            min(buf.len(), READ_LIMIT),
        ))
    }
}

pub(crate) unsafe fn pread(
    fd: BorrowedFd<'_>,
    buf: (*mut u8, usize),
    offset: u64,
) -> io::Result<usize> {
    let len = min(buf.1, READ_LIMIT);

    // Silently cast; we'll get `EINVAL` if the value is negative.
    let offset = offset as i64;

    // ESP-IDF and Vita don't support 64-bit offsets, for example.
    let offset = offset.try_into().map_err(|_| io::Errno::OVERFLOW)?;

    ret_usize(c::pread(borrowed_fd(fd), buf.0.cast(), len, offset))
}

pub(crate) fn pwrite(fd: BorrowedFd<'_>, buf: &[u8], offset: u64) -> io::Result<usize> {
    let len = min(buf.len(), READ_LIMIT);

    // Silently cast; we'll get `EINVAL` if the value is negative.
    let offset = offset as i64;

    // ESP-IDF and Vita don't support 64-bit offsets, for example.
    let offset = offset.try_into().map_err(|_| io::Errno::OVERFLOW)?;

    unsafe { ret_usize(c::pwrite(borrowed_fd(fd), buf.as_ptr().cast(), len, offset)) }
}

#[cfg(not(any(target_os = "espidf", target_os = "horizon")))]
pub(crate) fn readv(fd: BorrowedFd<'_>, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
    unsafe {
        ret_usize(c::readv(
            borrowed_fd(fd),
            bufs.as_ptr().cast::<c::iovec>(),
            min(bufs.len(), MAX_IOV) as c::c_int,
        ))
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "horizon")))]
pub(crate) fn writev(fd: BorrowedFd<'_>, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
    unsafe {
        ret_usize(c::writev(
            borrowed_fd(fd),
            bufs.as_ptr().cast::<c::iovec>(),
            min(bufs.len(), MAX_IOV) as c::c_int,
        ))
    }
}

#[cfg(not(any(
    target_os = "cygwin",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "nto",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
)))]
pub(crate) fn preadv(
    fd: BorrowedFd<'_>,
    bufs: &mut [IoSliceMut<'_>],
    offset: u64,
) -> io::Result<usize> {
    // Silently cast; we'll get `EINVAL` if the value is negative.
    let offset = offset as i64;

    // ESP-IDF and Vita don't support 64-bit offsets, for example.
    let offset = offset.try_into().map_err(|_| io::Errno::OVERFLOW)?;

    unsafe {
        ret_usize(c::preadv(
            borrowed_fd(fd),
            bufs.as_ptr().cast::<c::iovec>(),
            min(bufs.len(), MAX_IOV) as c::c_int,
            offset,
        ))
    }
}

#[cfg(not(any(
    target_os = "cygwin",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "nto",
    target_os = "horizon",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
)))]
pub(crate) fn pwritev(fd: BorrowedFd<'_>, bufs: &[IoSlice<'_>], offset: u64) -> io::Result<usize> {
    // Silently cast; we'll get `EINVAL` if the value is negative.
    let offset = offset as i64;

    // ESP-IDF and Vita don't support 64-bit offsets, for example.
    let offset = offset.try_into().map_err(|_| io::Errno::OVERFLOW)?;

    unsafe {
        ret_usize(c::pwritev(
            borrowed_fd(fd),
            bufs.as_ptr().cast::<c::iovec>(),
            min(bufs.len(), MAX_IOV) as c::c_int,
            offset,
        ))
    }
}

#[cfg(linux_kernel)]
pub(crate) fn preadv2(
    fd: BorrowedFd<'_>,
    bufs: &mut [IoSliceMut<'_>],
    offset: u64,
    flags: ReadWriteFlags,
) -> io::Result<usize> {
    // Silently cast; we'll get `EINVAL` if the value is negative.
    let offset = offset as i64;
    unsafe {
        ret_usize(c::preadv2(
            borrowed_fd(fd),
            bufs.as_ptr().cast::<c::iovec>(),
            min(bufs.len(), MAX_IOV) as c::c_int,
            offset,
            bitflags_bits!(flags),
        ))
    }
}

#[cfg(linux_kernel)]
pub(crate) fn pwritev2(
    fd: BorrowedFd<'_>,
    bufs: &[IoSlice<'_>],
    offset: u64,
    flags: ReadWriteFlags,
) -> io::Result<usize> {
    // Silently cast; we'll get `EINVAL` if the value is negative.
    let offset = offset as i64;
    unsafe {
        ret_usize(c::pwritev2(
            borrowed_fd(fd),
            bufs.as_ptr().cast::<c::iovec>(),
            min(bufs.len(), MAX_IOV) as c::c_int,
            offset,
            bitflags_bits!(flags),
        ))
    }
}

// These functions are derived from Rust's library/std/src/sys/unix/fd.rs at
// revision 326ef470a8b379a180d6dc4bbef08990698a737a.

// The maximum read limit on most POSIX-like systems is `SSIZE_MAX`, with the
// manual page quoting that if the count of bytes to read is greater than
// `SSIZE_MAX` the result is “unspecified”.
//
// On macOS, however, apparently the 64-bit libc is either buggy or
// intentionally showing odd behavior by rejecting any read with a size larger
// than or equal to `INT_MAX`. To handle both of these the read size is capped
// on both platforms.
#[cfg(target_os = "macos")]
const READ_LIMIT: usize = c::c_int::MAX as usize - 1;
#[cfg(not(target_os = "macos"))]
const READ_LIMIT: usize = c::ssize_t::MAX as usize;

pub(crate) unsafe fn close(raw_fd: RawFd) {
    let _ = c::close(raw_fd as c::c_int);
}

#[cfg(feature = "try_close")]
pub(crate) unsafe fn try_close(raw_fd: RawFd) -> io::Result<()> {
    ret(c::close(raw_fd as c::c_int))
}

#[inline]
pub(crate) unsafe fn ioctl(
    fd: BorrowedFd<'_>,
    request: Opcode,
    arg: *mut c::c_void,
) -> io::Result<IoctlOutput> {
    ret_c_int(c::ioctl(borrowed_fd(fd), request, arg))
}

#[inline]
pub(crate) unsafe fn ioctl_readonly(
    fd: BorrowedFd<'_>,
    request: Opcode,
    arg: *mut c::c_void,
) -> io::Result<IoctlOutput> {
    ioctl(fd, request, arg)
}

pub(crate) fn fcntl_getfd(fd: BorrowedFd<'_>) -> io::Result<FdFlags> {
    let flags = unsafe { ret_c_int(c::fcntl(borrowed_fd(fd), c::F_GETFD))? };
    Ok(FdFlags::from_bits_retain(bitcast!(flags)))
}

pub(crate) fn fcntl_setfd(fd: BorrowedFd<'_>, flags: FdFlags) -> io::Result<()> {
    unsafe { ret(c::fcntl(borrowed_fd(fd), c::F_SETFD, flags.bits())) }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
pub(crate) fn fcntl_dupfd_cloexec(fd: BorrowedFd<'_>, min: RawFd) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(c::fcntl(borrowed_fd(fd), c::F_DUPFD_CLOEXEC, min)) }
}

#[cfg(target_os = "espidf")]
pub(crate) fn fcntl_dupfd(fd: BorrowedFd<'_>, min: RawFd) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(c::fcntl(borrowed_fd(fd), c::F_DUPFD, min)) }
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn dup(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(c::dup(borrowed_fd(fd))) }
}

#[allow(clippy::needless_pass_by_ref_mut)]
#[cfg(not(target_os = "wasi"))]
pub(crate) fn dup2(fd: BorrowedFd<'_>, new: &mut OwnedFd) -> io::Result<()> {
    unsafe { ret_discarded_fd(c::dup2(borrowed_fd(fd), borrowed_fd(new.as_fd()))) }
}

#[allow(clippy::needless_pass_by_ref_mut)]
#[cfg(not(any(
    apple,
    target_os = "aix",
    target_os = "android",
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
pub(crate) fn dup3(fd: BorrowedFd<'_>, new: &mut OwnedFd, flags: DupFlags) -> io::Result<()> {
    unsafe {
        ret_discarded_fd(c::dup3(
            borrowed_fd(fd),
            borrowed_fd(new.as_fd()),
            bitflags_bits!(flags),
        ))
    }
}

#[cfg(any(
    apple,
    target_os = "android",
    target_os = "dragonfly",
    target_os = "haiku",
    target_os = "redox",
))]
pub(crate) fn dup3(fd: BorrowedFd<'_>, new: &mut OwnedFd, _flags: DupFlags) -> io::Result<()> {
    // Android 5.0 has `dup3`, but libc doesn't have bindings. Emulate it
    // using `dup2`. We don't need to worry about the difference between
    // `dup2` and `dup3` when the file descriptors are equal because we
    // have an `&mut OwnedFd` which means `fd` doesn't alias it.
    dup2(fd, new)
}
