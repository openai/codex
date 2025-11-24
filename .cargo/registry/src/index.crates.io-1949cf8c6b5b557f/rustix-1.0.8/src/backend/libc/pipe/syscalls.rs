use crate::backend::c;
use crate::backend::conv::ret;
use crate::fd::OwnedFd;
use crate::io;
#[cfg(not(any(
    apple,
    target_os = "aix",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "nto",
    target_os = "wasi"
)))]
use crate::pipe::PipeFlags;
use core::mem::MaybeUninit;
#[cfg(linux_kernel)]
use {
    crate::backend::conv::{borrowed_fd, ret_c_int, ret_usize},
    crate::backend::MAX_IOV,
    crate::fd::BorrowedFd,
    crate::pipe::{IoSliceRaw, SpliceFlags},
    crate::utils::option_as_mut_ptr,
    core::cmp::min,
};

#[cfg(not(target_os = "wasi"))]
pub(crate) fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    unsafe {
        let mut result = MaybeUninit::<[OwnedFd; 2]>::uninit();
        ret(c::pipe(result.as_mut_ptr().cast::<i32>()))?;
        let [p0, p1] = result.assume_init();
        Ok((p0, p1))
    }
}

#[cfg(not(any(
    apple,
    target_os = "aix",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "nto",
    target_os = "wasi"
)))]
pub(crate) fn pipe_with(flags: PipeFlags) -> io::Result<(OwnedFd, OwnedFd)> {
    unsafe {
        let mut result = MaybeUninit::<[OwnedFd; 2]>::uninit();
        ret(c::pipe2(
            result.as_mut_ptr().cast::<i32>(),
            bitflags_bits!(flags),
        ))?;
        let [p0, p1] = result.assume_init();
        Ok((p0, p1))
    }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn splice(
    fd_in: BorrowedFd<'_>,
    off_in: Option<&mut u64>,
    fd_out: BorrowedFd<'_>,
    off_out: Option<&mut u64>,
    len: usize,
    flags: SpliceFlags,
) -> io::Result<usize> {
    let off_in = option_as_mut_ptr(off_in).cast();
    let off_out = option_as_mut_ptr(off_out).cast();

    unsafe {
        ret_usize(c::splice(
            borrowed_fd(fd_in),
            off_in,
            borrowed_fd(fd_out),
            off_out,
            len,
            flags.bits(),
        ))
    }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) unsafe fn vmsplice(
    fd: BorrowedFd<'_>,
    bufs: &[IoSliceRaw<'_>],
    flags: SpliceFlags,
) -> io::Result<usize> {
    ret_usize(c::vmsplice(
        borrowed_fd(fd),
        bufs.as_ptr().cast::<c::iovec>(),
        min(bufs.len(), MAX_IOV),
        flags.bits(),
    ))
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn tee(
    fd_in: BorrowedFd<'_>,
    fd_out: BorrowedFd<'_>,
    len: usize,
    flags: SpliceFlags,
) -> io::Result<usize> {
    unsafe {
        ret_usize(c::tee(
            borrowed_fd(fd_in),
            borrowed_fd(fd_out),
            len,
            flags.bits(),
        ))
    }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn fcntl_getpipe_size(fd: BorrowedFd<'_>) -> io::Result<usize> {
    unsafe { ret_c_int(c::fcntl(borrowed_fd(fd), c::F_GETPIPE_SZ)).map(|size| size as usize) }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn fcntl_setpipe_size(fd: BorrowedFd<'_>, size: usize) -> io::Result<usize> {
    let size: c::c_int = size.try_into().map_err(|_| io::Errno::PERM)?;

    unsafe { ret_c_int(c::fcntl(borrowed_fd(fd), c::F_SETPIPE_SZ, size)).map(|size| size as usize) }
}
