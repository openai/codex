//! Filesystem-oriented `ioctl` functions.

#![allow(unsafe_code)]

#[cfg(linux_kernel)]
use {
    crate::backend::c,
    crate::fd::AsFd,
    crate::{backend, ffi, io, ioctl},
};

use bitflags::bitflags;

#[cfg(all(linux_kernel, not(any(target_arch = "sparc", target_arch = "sparc64"))))]
use crate::fd::{AsRawFd as _, BorrowedFd};

/// `ioctl(fd, BLKSSZGET)`—Returns the logical block size of a block device.
///
/// This is mentioned in the [Linux `openat` manual page].
///
/// [Linux `openat` manual page]: https://man7.org/linux/man-pages/man2/openat.2.html
#[cfg(linux_kernel)]
#[inline]
#[doc(alias = "BLKSSZGET")]
pub fn ioctl_blksszget<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    // SAFETY: `BLZSSZGET` is a getter opcode that gets a `u32`.
    unsafe {
        let ctl = ioctl::Getter::<{ c::BLKSSZGET }, c::c_uint>::new();
        ioctl::ioctl(fd, ctl)
    }
}

/// `ioctl(fd, BLKPBSZGET)`—Returns the physical block size of a block device.
#[cfg(linux_kernel)]
#[inline]
#[doc(alias = "BLKPBSZGET")]
pub fn ioctl_blkpbszget<Fd: AsFd>(fd: Fd) -> io::Result<u32> {
    // SAFETY: `BLKPBSZGET` is a getter opcode that gets a `u32`.
    unsafe {
        let ctl = ioctl::Getter::<{ c::BLKPBSZGET }, c::c_uint>::new();
        ioctl::ioctl(fd, ctl)
    }
}

/// `ioctl(fd, FICLONE, src_fd)`—Share data between open files.
///
/// This ioctl is not available on SPARC platforms.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/ioctl_ficlone.2.html
#[cfg(all(linux_kernel, not(any(target_arch = "sparc", target_arch = "sparc64"))))]
#[inline]
#[doc(alias = "FICLONE")]
pub fn ioctl_ficlone<Fd: AsFd, SrcFd: AsFd>(fd: Fd, src_fd: SrcFd) -> io::Result<()> {
    unsafe { ioctl::ioctl(fd, Ficlone(src_fd.as_fd())) }
}

/// `ioctl(fd, EXT4_IOC_RESIZE_FS, blocks)`—Resize ext4 filesystem on fd.
#[cfg(linux_kernel)]
#[inline]
#[doc(alias = "EXT4_IOC_RESIZE_FS")]
pub fn ext4_ioc_resize_fs<Fd: AsFd>(fd: Fd, blocks: u64) -> io::Result<()> {
    // SAFETY: `EXT4_IOC_RESIZE_FS` is a pointer setter opcode.
    unsafe {
        let ctl = ioctl::Setter::<{ backend::fs::EXT4_IOC_RESIZE_FS }, u64>::new(blocks);
        ioctl::ioctl(fd, ctl)
    }
}

#[cfg(all(linux_kernel, not(any(target_arch = "sparc", target_arch = "sparc64"))))]
struct Ficlone<'a>(BorrowedFd<'a>);

#[cfg(all(linux_kernel, not(any(target_arch = "sparc", target_arch = "sparc64"))))]
unsafe impl ioctl::Ioctl for Ficlone<'_> {
    type Output = ();

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> ioctl::Opcode {
        c::FICLONE as ioctl::Opcode
    }

    fn as_ptr(&mut self) -> *mut c::c_void {
        self.0.as_raw_fd() as *mut c::c_void
    }

    unsafe fn output_from_ptr(
        _: ioctl::IoctlOutput,
        _: *mut c::c_void,
    ) -> io::Result<Self::Output> {
        Ok(())
    }
}

#[cfg(linux_kernel)]
bitflags! {
    /// `FS_*` constants for use with [`ioctl_getflags`].
    ///
    /// [`ioctl_getflags`]: crate::fs::ioctl::ioctl_getflags
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IFlags: ffi::c_uint {
        /// `FS_APPEND_FL`
        const APPEND = linux_raw_sys::general::FS_APPEND_FL;
        /// `FS_COMPR_FL`
        const COMPRESSED = linux_raw_sys::general::FS_COMPR_FL;
        /// `FS_DIRSYNC_FL`
        const DIRSYNC = linux_raw_sys::general::FS_DIRSYNC_FL;
        /// `FS_IMMUTABLE_FL`
        const IMMUTABLE = linux_raw_sys::general::FS_IMMUTABLE_FL;
        /// `FS_JOURNAL_DATA_FL`
        const JOURNALING = linux_raw_sys::general::FS_JOURNAL_DATA_FL;
        /// `FS_NOATIME_FL`
        const NOATIME = linux_raw_sys::general::FS_NOATIME_FL;
        /// `FS_NOCOW_FL`
        const NOCOW = linux_raw_sys::general::FS_NOCOW_FL;
        /// `FS_NODUMP_FL`
        const NODUMP = linux_raw_sys::general::FS_NODUMP_FL;
        /// `FS_NOTAIL_FL`
        const NOTAIL = linux_raw_sys::general::FS_NOTAIL_FL;
        /// `FS_PROJINHERIT_FL`
        const PROJECT_INHERIT = linux_raw_sys::general::FS_PROJINHERIT_FL;
        /// `FS_SECRM_FL`
        const SECURE_REMOVAL = linux_raw_sys::general::FS_SECRM_FL;
        /// `FS_SYNC_FL`
        const SYNC = linux_raw_sys::general::FS_SYNC_FL;
        /// `FS_TOPDIR_FL`
        const TOPDIR = linux_raw_sys::general::FS_TOPDIR_FL;
        /// `FS_UNRM_FL`
        const UNRM = linux_raw_sys::general::FS_UNRM_FL;
    }
}

/// `ioctl(fd, FS_IOC_GETFLAGS)`—Returns the [inode flags] attributes
///
/// [inode flags]: https://man7.org/linux/man-pages/man2/ioctl_iflags.2.html
#[cfg(linux_kernel)]
#[inline]
#[doc(alias = "FS_IOC_GETFLAGS")]
pub fn ioctl_getflags<Fd: AsFd>(fd: Fd) -> io::Result<IFlags> {
    unsafe {
        #[cfg(target_pointer_width = "32")]
        let ctl = ioctl::Getter::<{ c::FS_IOC32_GETFLAGS }, u32>::new();
        #[cfg(target_pointer_width = "64")]
        let ctl = ioctl::Getter::<{ c::FS_IOC_GETFLAGS }, u32>::new();

        ioctl::ioctl(fd, ctl).map(IFlags::from_bits_retain)
    }
}

/// `ioctl(fd, FS_IOC_SETFLAGS)`—Modify the [inode flags] attributes
///
/// [inode flags]: https://man7.org/linux/man-pages/man2/ioctl_iflags.2.html
#[cfg(linux_kernel)]
#[inline]
#[doc(alias = "FS_IOC_SETFLAGS")]
pub fn ioctl_setflags<Fd: AsFd>(fd: Fd, flags: IFlags) -> io::Result<()> {
    unsafe {
        #[cfg(target_pointer_width = "32")]
        let ctl = ioctl::Setter::<{ c::FS_IOC32_SETFLAGS }, u32>::new(flags.bits());

        #[cfg(target_pointer_width = "64")]
        let ctl = ioctl::Setter::<{ c::FS_IOC_SETFLAGS }, u32>::new(flags.bits());

        ioctl::ioctl(fd, ctl)
    }
}
