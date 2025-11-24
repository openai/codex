//! Linux `statx`.

use crate::fd::AsFd;
use crate::fs::AtFlags;
use crate::{backend, io, path};
use backend::c;
use bitflags::bitflags;

#[cfg(feature = "linux_4_11")]
use backend::fs::syscalls::statx as _statx;
#[cfg(not(feature = "linux_4_11"))]
use compat::statx as _statx;

/// `struct statx` for use with [`statx`].
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(missing_docs)]
#[non_exhaustive]
pub struct Statx {
    pub stx_mask: u32,
    pub stx_blksize: u32,
    pub stx_attributes: StatxAttributes,
    pub stx_nlink: u32,
    pub stx_uid: u32,
    pub stx_gid: u32,
    pub stx_mode: u16,
    pub(crate) __spare0: [u16; 1],
    pub stx_ino: u64,
    pub stx_size: u64,
    pub stx_blocks: u64,
    pub stx_attributes_mask: StatxAttributes,
    pub stx_atime: StatxTimestamp,
    pub stx_btime: StatxTimestamp,
    pub stx_ctime: StatxTimestamp,
    pub stx_mtime: StatxTimestamp,
    pub stx_rdev_major: u32,
    pub stx_rdev_minor: u32,
    pub stx_dev_major: u32,
    pub stx_dev_minor: u32,
    pub stx_mnt_id: u64,
    pub stx_dio_mem_align: u32,
    pub stx_dio_offset_align: u32,
    pub stx_subvol: u64,
    pub stx_atomic_write_unit_min: u32,
    pub stx_atomic_write_unit_max: u32,
    pub stx_atomic_write_segments_max: u32,
    pub(crate) __spare1: [u32; 1],
    pub(crate) __spare3: [u64; 9],
}

/// `struct statx_timestamp` for use with [`Statx`].
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub struct StatxTimestamp {
    /// Seconds.
    pub tv_sec: i64,

    /// Nanoseconds. Must be less than 1_000_000_000.
    pub tv_nsec: u32,

    pub(crate) __reserved: i32,
}

bitflags! {
    /// `STATX_*` constants for use with [`statx`].
    ///
    /// [`statx`]: crate::fs::statx
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct StatxFlags: u32 {
        /// `STATX_TYPE`
        const TYPE = c::STATX_TYPE;

        /// `STATX_MODE`
        const MODE = c::STATX_MODE;

        /// `STATX_NLINK`
        const NLINK = c::STATX_NLINK;

        /// `STATX_UID`
        const UID = c::STATX_UID;

        /// `STATX_GID`
        const GID = c::STATX_GID;

        /// `STATX_ATIME`
        const ATIME = c::STATX_ATIME;

        /// `STATX_MTIME`
        const MTIME = c::STATX_MTIME;

        /// `STATX_CTIME`
        const CTIME = c::STATX_CTIME;

        /// `STATX_INO`
        const INO = c::STATX_INO;

        /// `STATX_SIZE`
        const SIZE = c::STATX_SIZE;

        /// `STATX_BLOCKS`
        const BLOCKS = c::STATX_BLOCKS;

        /// `STATX_BASIC_STATS`
        const BASIC_STATS = c::STATX_BASIC_STATS;

        /// `STATX_BTIME`
        const BTIME = c::STATX_BTIME;

        /// `STATX_MNT_ID` (since Linux 5.8)
        const MNT_ID = c::STATX_MNT_ID;

        /// `STATX_DIOALIGN` (since Linux 6.1)
        const DIOALIGN = c::STATX_DIOALIGN;

        /// `STATX_ALL`
        const ALL = c::STATX_ALL;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `STATX_ATTR_*` flags for use with [`Statx`].
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct StatxAttributes: u64 {
        /// `STATX_ATTR_COMPRESSED`
        const COMPRESSED = c::STATX_ATTR_COMPRESSED as u64;

        /// `STATX_ATTR_IMMUTABLE`
        const IMMUTABLE = c::STATX_ATTR_IMMUTABLE as u64;

        /// `STATX_ATTR_APPEND`
        const APPEND = c::STATX_ATTR_APPEND as u64;

        /// `STATX_ATTR_NODUMP`
        const NODUMP = c::STATX_ATTR_NODUMP as u64;

        /// `STATX_ATTR_ENCRYPTED`
        const ENCRYPTED = c::STATX_ATTR_ENCRYPTED as u64;

        /// `STATX_ATTR_AUTOMOUNT`
        const AUTOMOUNT = c::STATX_ATTR_AUTOMOUNT as u64;

        /// `STATX_ATTR_MOUNT_ROOT`
        const MOUNT_ROOT = c::STATX_ATTR_MOUNT_ROOT as u64;

        /// `STATX_ATTR_VERITY`
        const VERITY = c::STATX_ATTR_VERITY as u64;

        /// `STATX_ATTR_DAX`
        const DAX = c::STATX_ATTR_DAX as u64;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `statx(dirfd, path, flags, mask, statxbuf)`—Extended `stat`.
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
/// /// [`StatxAttributes::MOUNT_ROOT`].
/// fn is_mountpoint(root: BorrowedFd<'_>, path: &Path) -> io::Result<Option<bool>> {
///     use rustix::fs::{AtFlags, StatxAttributes, StatxFlags};
///
///     match rustix::fs::statx(
///         root,
///         path,
///         AtFlags::NO_AUTOMOUNT | AtFlags::SYMLINK_NOFOLLOW,
///         StatxFlags::empty(),
///     ) {
///         Ok(r) => {
///             let present = r.stx_attributes_mask.contains(StatxAttributes::MOUNT_ROOT);
///             Ok(present.then(|| r.stx_attributes.contains(StatxAttributes::MOUNT_ROOT)))
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
    use crate::fs::{AtFlags, Statx, StatxFlags};
    use crate::{backend, io};
    use core::sync::atomic::{AtomicU8, Ordering};

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
