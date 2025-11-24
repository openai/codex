use crate::{backend, io};
use backend::fd::AsFd;

/// `sendfile(out_fd, in_fd, offset, count)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/sendfile.2.html
#[cfg(linux_kernel)]
#[inline]
pub fn sendfile<OutFd: AsFd, InFd: AsFd>(
    out_fd: OutFd,
    in_fd: InFd,
    offset: Option<&mut u64>,
    count: usize,
) -> io::Result<usize> {
    backend::fs::syscalls::sendfile(out_fd.as_fd(), in_fd.as_fd(), offset, count)
}
