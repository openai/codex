//! The `CWD` and `ABS` constants, representing the current working directory
//! and absolute-only paths, respectively.
//!
//! # Safety
//!
//! This file uses `AT_FDCWD`, which is a raw file descriptor, but which is
//! always valid, and `-EBADF`, which is an undocumented by commonly used
//! convention of passing a value which will always fail if the accompanying
//! path isn't absolute.

#![allow(unsafe_code)]

use crate::backend;
use backend::c;
use backend::fd::{BorrowedFd, RawFd};

/// `AT_FDCWD`—A handle representing the current working directory.
///
/// This is a file descriptor which refers to the process current directory
/// which can be used as the directory argument in `*at` functions such as
/// [`openat`].
///
/// # References
///  - [POSIX]
///
/// [`openat`]: crate::fs::openat
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/fcntl.h.html
// SAFETY: `AT_FDCWD` is a reserved value that is never dynamically
// allocated, so it'll remain valid for the duration of `'static`.
#[cfg(not(target_os = "horizon"))]
#[doc(alias = "AT_FDCWD")]
pub const CWD: BorrowedFd<'static> =
    unsafe { BorrowedFd::<'static>::borrow_raw(c::AT_FDCWD as RawFd) };

/// `-EBADF`—A handle that requires paths to be absolute.
///
/// This is a file descriptor which refers to no directory, which can be used
/// as the directory argument in `*at` functions such as [`openat`], which
/// causes them to fail with [`BADF`] if the accompanying path is not absolute.
///
/// This corresponds to the undocumented by commonly used convention of
/// passing `-EBADF` as the `dirfd` argument, which is ignored if the path is
/// absolute, and evokes an `EBADF` error otherwise.
///
/// [`openat`]: crate::fs::openat
/// [`BADF`]: crate::io::Errno::BADF
// SAFETY: This `-EBADF` convention is commonly used, such as in lxc, so OS's
// aren't going to break it.
pub const ABS: BorrowedFd<'static> =
    unsafe { BorrowedFd::<'static>::borrow_raw(c::EBADF.wrapping_neg() as RawFd) };

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fd::AsRawFd as _;

    #[test]
    fn test_cwd() {
        assert!(CWD.as_raw_fd() != -1);
        assert!(CWD.as_raw_fd() != c::STDIN_FILENO);
        assert!(CWD.as_raw_fd() != c::STDOUT_FILENO);
        assert!(CWD.as_raw_fd() != c::STDERR_FILENO);
        #[cfg(linux_kernel)]
        #[cfg(feature = "io_uring")]
        assert!(CWD.as_raw_fd() != crate::io_uring::IORING_REGISTER_FILES_SKIP.as_raw_fd());
    }

    #[test]
    fn test_abs() {
        assert!(ABS.as_raw_fd() < 0);
        assert!(ABS.as_raw_fd() != -1);
        assert!(ABS.as_raw_fd() != c::AT_FDCWD);
        assert!(ABS.as_raw_fd() != c::STDIN_FILENO);
        assert!(ABS.as_raw_fd() != c::STDOUT_FILENO);
        assert!(ABS.as_raw_fd() != c::STDERR_FILENO);
        #[cfg(linux_kernel)]
        #[cfg(feature = "io_uring")]
        assert!(ABS.as_raw_fd() != crate::io_uring::IORING_REGISTER_FILES_SKIP.as_raw_fd());
    }
}
