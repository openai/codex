use crate::{backend, io};
use backend::fd::AsFd;
use backend::fs::types::Advice;
use core::num::NonZeroU64;

/// `posix_fadvise(fd, offset, len, advice)`—Declares an expected access
/// pattern for a file.
///
/// If `len` is `None`, the advice extends to the end of the file.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/posix_fadvise.html
/// [Linux]: https://man7.org/linux/man-pages/man2/posix_fadvise.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=posix_fadvise&sektion=2
#[inline]
#[doc(alias = "posix_fadvise")]
pub fn fadvise<Fd: AsFd>(
    fd: Fd,
    offset: u64,
    len: Option<NonZeroU64>,
    advice: Advice,
) -> io::Result<()> {
    backend::fs::syscalls::fadvise(fd.as_fd(), offset, len, advice)
}
