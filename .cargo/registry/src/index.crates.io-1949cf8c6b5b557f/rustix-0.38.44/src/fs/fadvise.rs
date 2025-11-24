use crate::{backend, io};
use backend::fd::AsFd;
use backend::fs::types::Advice;

/// `posix_fadvise(fd, offset, len, advice)`—Declares an expected access
/// pattern for a file.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/posix_fadvise.html
/// [Linux]: https://man7.org/linux/man-pages/man2/posix_fadvise.2.html
#[inline]
#[doc(alias = "posix_fadvise")]
pub fn fadvise<Fd: AsFd>(fd: Fd, offset: u64, len: u64, advice: Advice) -> io::Result<()> {
    backend::fs::syscalls::fadvise(fd.as_fd(), offset, len, advice)
}
