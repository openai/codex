use crate::ffi::CString;
use crate::{backend, io};
use backend::fd::AsFd;

/// `fcntl(fd, F_GETPATH)`
///
/// # References
///  - [Apple]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
#[inline]
pub fn getpath<Fd: AsFd>(fd: Fd) -> io::Result<CString> {
    backend::fs::syscalls::getpath(fd.as_fd())
}
