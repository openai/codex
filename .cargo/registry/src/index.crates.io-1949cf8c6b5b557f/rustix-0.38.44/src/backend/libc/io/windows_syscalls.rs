//! Windows system calls in the `io` module.

use crate::backend::c;
#[cfg(feature = "try_close")]
use crate::backend::conv::ret;
use crate::backend::conv::{borrowed_fd, ret_c_int};
use crate::backend::fd::LibcFd;
use crate::fd::{BorrowedFd, RawFd};
use crate::io;
use crate::ioctl::{IoctlOutput, RawOpcode};

pub(crate) unsafe fn close(raw_fd: RawFd) {
    let _ = c::close(raw_fd as LibcFd);
}

#[cfg(feature = "try_close")]
pub(crate) unsafe fn try_close(raw_fd: RawFd) -> io::Result<()> {
    ret(c::close(raw_fd as LibcFd))
}

#[inline]
pub(crate) unsafe fn ioctl(
    fd: BorrowedFd<'_>,
    request: RawOpcode,
    arg: *mut c::c_void,
) -> io::Result<IoctlOutput> {
    ret_c_int(c::ioctl(borrowed_fd(fd), request, arg.cast()))
}

#[inline]
pub(crate) unsafe fn ioctl_readonly(
    fd: BorrowedFd<'_>,
    request: RawOpcode,
    arg: *mut c::c_void,
) -> io::Result<IoctlOutput> {
    ioctl(fd, request, arg)
}
