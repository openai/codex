//! The Unix `fcntl` function is effectively lots of different functions hidden
//! behind a single dynamic dispatch interface. In order to provide a type-safe
//! API, rustix makes them all separate functions so that they can have
//! dedicated static type signatures.

#[cfg(not(any(
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
use crate::fs::FlockOperation;
use crate::{backend, io};
use backend::fd::AsFd;
use backend::fs::types::OFlags;

/// `fcntl(fd, F_GETFL)`—Returns a file descriptor's access mode and status.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fcntl.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fcntl.2.html
#[inline]
#[doc(alias = "F_GETFL")]
pub fn fcntl_getfl<Fd: AsFd>(fd: Fd) -> io::Result<OFlags> {
    backend::fs::syscalls::fcntl_getfl(fd.as_fd())
}

/// `fcntl(fd, F_SETFL, flags)`—Sets a file descriptor's status.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fcntl.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fcntl.2.html
#[inline]
#[doc(alias = "F_SETFL")]
pub fn fcntl_setfl<Fd: AsFd>(fd: Fd, flags: OFlags) -> io::Result<()> {
    backend::fs::syscalls::fcntl_setfl(fd.as_fd(), flags)
}

/// `fcntl(fd, F_GET_SEALS)`—Return the seals for `fd`'s inode.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/fcntl.2.html
#[cfg(any(linux_kernel, target_os = "freebsd", target_os = "fuchsia"))]
#[inline]
#[doc(alias = "F_GET_SEALS")]
pub fn fcntl_get_seals<Fd: AsFd>(fd: Fd) -> io::Result<SealFlags> {
    backend::fs::syscalls::fcntl_get_seals(fd.as_fd())
}

#[cfg(any(linux_kernel, target_os = "freebsd", target_os = "fuchsia"))]
use backend::fs::types::SealFlags;

/// `fcntl(fd, F_ADD_SEALS)`—Add seals to `fd`'s inode.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/fcntl.2.html
#[cfg(any(linux_kernel, target_os = "freebsd", target_os = "fuchsia"))]
#[inline]
#[doc(alias = "F_ADD_SEALS")]
pub fn fcntl_add_seals<Fd: AsFd>(fd: Fd, seals: SealFlags) -> io::Result<()> {
    backend::fs::syscalls::fcntl_add_seals(fd.as_fd(), seals)
}

/// `fcntl(fd, F_SETLK)`—Acquire or release an `fcntl`-style lock.
///
/// This function doesn't currently have an offset or len; it currently always
/// sets the `l_len` field to 0, which is a special case that means the entire
/// file should be locked.
///
/// Unlike `flock`-style locks, `fcntl`-style locks are process-associated,
/// meaning that they don't guard against being acquired by two threads in the
/// same process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fcntl.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fcntl.2.html
#[cfg(not(any(
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
#[doc(alias = "F_SETLK")]
#[doc(alias = "F_SETLKW")]
pub fn fcntl_lock<Fd: AsFd>(fd: Fd, operation: FlockOperation) -> io::Result<()> {
    backend::fs::syscalls::fcntl_lock(fd.as_fd(), operation)
}
