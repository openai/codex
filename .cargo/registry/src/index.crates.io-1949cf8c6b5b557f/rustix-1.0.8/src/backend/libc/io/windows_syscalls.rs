//! Windows system calls in the `io` module.

use crate::backend::c;
#[cfg(feature = "try_close")]
use crate::backend::conv::ret;
use crate::backend::conv::{borrowed_fd, ret_c_int, ret_send_recv, send_recv_len};
use crate::fd::{BorrowedFd, RawFd};
use crate::io;
use crate::ioctl::{IoctlOutput, Opcode};

pub(crate) unsafe fn read(fd: BorrowedFd<'_>, buf: (*mut u8, usize)) -> io::Result<usize> {
    // `read` on a socket is equivalent to `recv` with no flags.
    ret_send_recv(c::recv(
        borrowed_fd(fd),
        buf.0.cast(),
        send_recv_len(buf.1),
        0,
    ))
}

pub(crate) fn write(fd: BorrowedFd<'_>, buf: &[u8]) -> io::Result<usize> {
    // `write` on a socket is equivalent to `send` with no flags.
    unsafe {
        ret_send_recv(c::send(
            borrowed_fd(fd),
            buf.as_ptr().cast(),
            send_recv_len(buf.len()),
            0,
        ))
    }
}

pub(crate) unsafe fn close(raw_fd: RawFd) {
    let _ = c::closesocket(raw_fd as c::SOCKET);
}

#[cfg(feature = "try_close")]
pub(crate) unsafe fn try_close(raw_fd: RawFd) -> io::Result<()> {
    ret(c::closesocket(raw_fd as c::SOCKET))
}

#[inline]
pub(crate) unsafe fn ioctl(
    fd: BorrowedFd<'_>,
    request: Opcode,
    arg: *mut c::c_void,
) -> io::Result<IoctlOutput> {
    ret_c_int(c::ioctl(borrowed_fd(fd), request, arg.cast()))
}

#[inline]
pub(crate) unsafe fn ioctl_readonly(
    fd: BorrowedFd<'_>,
    request: Opcode,
    arg: *mut c::c_void,
) -> io::Result<IoctlOutput> {
    ioctl(fd, request, arg)
}
