use crate::{backend, io};
use backend::fd::AsFd;

/// `fcntl(fd, F_RDADVISE, radvisory { offset, len })`
///
/// # References
///  - [Apple]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
#[doc(alias = "F_RDADVISE")]
#[inline]
pub fn fcntl_rdadvise<Fd: AsFd>(fd: Fd, offset: u64, len: u64) -> io::Result<()> {
    backend::fs::syscalls::fcntl_rdadvise(fd.as_fd(), offset, len)
}

/// `fcntl(fd, F_FULLFSYNC)`
///
/// # References
///  - [Apple]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
#[doc(alias = "F_FULLSYNC")]
#[inline]
pub fn fcntl_fullfsync<Fd: AsFd>(fd: Fd) -> io::Result<()> {
    backend::fs::syscalls::fcntl_fullfsync(fd.as_fd())
}

/// `fcntl(fd, F_NOCACHE, value)`—Turn data caching off or on for a file
/// descriptor.
///
/// See [this mailing list post] for additional information about the meanings
/// of `F_NOCACHE` and `F_GLOBAL_NOCACHE`.
///
/// [this mailing list post]: https://lists.apple.com/archives/filesystem-dev/2007/Sep/msg00010.html
///
/// See also [`fcntl_global_nocache`].
///
/// # References
///  - [Apple]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
#[doc(alias = "F_NOCACHE")]
#[inline]
pub fn fcntl_nocache<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::fs::syscalls::fcntl_nocache(fd.as_fd(), value)
}

/// `fcntl(fd, F_GLOBAL_NOCACHE, value)`—Turn data caching off or on for all
/// file descriptors.
///
/// See [this mailing list post] for additional information about the meanings
/// of `F_NOCACHE` and `F_GLOBAL_NOCACHE`.
///
/// [this mailing list post]: https://lists.apple.com/archives/filesystem-dev/2007/Sep/msg00010.html
///
/// See also [`fcntl_nocache`].
///
/// # References
///  - [Apple]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
#[doc(alias = "F_GLOBAL_NOCACHE")]
#[inline]
pub fn fcntl_global_nocache<Fd: AsFd>(fd: Fd, value: bool) -> io::Result<()> {
    backend::fs::syscalls::fcntl_global_nocache(fd.as_fd(), value)
}
