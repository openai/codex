//! Linux `statx`.

use crate::fd::AsFd;
use crate::fs::AtFlags;
use crate::{backend, io, path};
use backend::fs::types::{Statx, StatxFlags};

#[cfg(feature = "linux_4_11")]
use backend::fs::syscalls::statx as _statx;
#[cfg(not(feature = "linux_4_11"))]
use compat::statx as _statx;

/// `statx(dirfd, path, flags, mask, statxbuf)`
///
/// This function returns [`io::Errno::NOSYS`] if `statx` is not available on
/// the platform, such as Linux before 4.11. This also includes older Docker
/// versions where the actual syscall fails with different error codes; rustix
/// handles this and translates them into `NOSYS`.
///
/// # References
///  - [Linux]
///
/// # Examples
///
/// ```
/// # use std::path::Path;
/// # use std::io;
/// # use rustix::fs::{AtFlags, StatxFlags};
/// # use rustix::fd::BorrowedFd;
/// /// Try to determine if the provided path is a mount root. Will return
/// /// `Ok(None)` if the kernel is not new enough to support `statx` or
/// /// [`libc::STATX_ATTR_MOUNT_ROOT`].
/// fn is_mountpoint(root: BorrowedFd<'_>, path: &Path) -> io::Result<Option<bool>> {
///     use rustix::fs::{AtFlags, StatxFlags};
///
///     let mountroot_flag = libc::STATX_ATTR_MOUNT_ROOT as u64;
///     match rustix::fs::statx(
///         root,
///         path,
///         AtFlags::NO_AUTOMOUNT | AtFlags::SYMLINK_NOFOLLOW,
///         StatxFlags::empty(),
///     ) {
///         Ok(r) => {
///             let present = (r.stx_attributes_mask & mountroot_flag) > 0;
///             Ok(present.then(|| r.stx_attributes & mountroot_flag > 0))
///         }
///         Err(rustix::io::Errno::NOSYS) => Ok(None),
///         Err(e) => Err(e.into()),
///     }
/// }
/// ```
///
/// [Linux]: https://man7.org/linux/man-pages/man2/statx.2.html
#[inline]
pub fn statx<P: path::Arg, Fd: AsFd>(
    dirfd: Fd,
    path: P,
    flags: AtFlags,
    mask: StatxFlags,
) -> io::Result<Statx> {
    path.into_with_c_str(|path| _statx(dirfd.as_fd(), path, flags, mask))
}

#[cfg(not(feature = "linux_4_11"))]
mod compat {
    use crate::fd::BorrowedFd;
    use crate::ffi::CStr;
    use crate::fs::AtFlags;
    use crate::{backend, io};
    use core::sync::atomic::{AtomicU8, Ordering};

    use backend::fs::types::{Statx, StatxFlags};

    // Linux kernel prior to 4.11 and old versions of Docker don't support
    // `statx`. We store the availability in a global to avoid unnecessary
    // syscalls.
    //
    // 0: Unknown
    // 1: Not available
    // 2: Available
    static STATX_STATE: AtomicU8 = AtomicU8::new(0);

    #[inline]
    pub fn statx(
        dirfd: BorrowedFd<'_>,
        path: &CStr,
        flags: AtFlags,
        mask: StatxFlags,
    ) -> io::Result<Statx> {
        match STATX_STATE.load(Ordering::Relaxed) {
            0 => statx_init(dirfd, path, flags, mask),
            1 => Err(io::Errno::NOSYS),
            _ => backend::fs::syscalls::statx(dirfd, path, flags, mask),
        }
    }

    /// The first `statx` call. We don't know if `statx` is available yet.
    fn statx_init(
        dirfd: BorrowedFd<'_>,
        path: &CStr,
        flags: AtFlags,
        mask: StatxFlags,
    ) -> io::Result<Statx> {
        match backend::fs::syscalls::statx(dirfd, path, flags, mask) {
            Err(err) => statx_error(err),
            result => {
                STATX_STATE.store(2, Ordering::Relaxed);
                result
            }
        }
    }

    /// The first `statx` call failed. We can get a variety of error codes
    /// from seccomp configs or faulty FUSE drivers, so we don't trust
    /// `ENOSYS` or `EPERM` to tell us whether statx is available.
    #[cold]
    fn statx_error(err: io::Errno) -> io::Result<Statx> {
        if backend::fs::syscalls::is_statx_available() {
            // Statx is available. Record this, and fail with the error
            // code of the initial `statx` call.
            STATX_STATE.store(2, Ordering::Relaxed);
            Err(err)
        } else {
            // Statx is not available. Record this, and fail with `NOSYS`.
            STATX_STATE.store(1, Ordering::Relaxed);
            Err(io::Errno::NOSYS)
        }
    }
}
