//! linux_raw syscalls supporting `rustix::fs`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code)]
#![allow(clippy::undocumented_unsafe_blocks)]

use crate::backend::c;
use crate::backend::conv::fs::oflags_for_open_how;
#[cfg(any(
    not(feature = "linux_4_11"),
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "mips",
    target_arch = "mips32r6",
))]
use crate::backend::conv::zero;
use crate::backend::conv::{
    by_ref, c_int, c_uint, dev_t, opt_mut, pass_usize, raw_fd, ret, ret_c_int, ret_c_uint,
    ret_infallible, ret_owned_fd, ret_usize, size_of, slice, slice_mut,
};
#[cfg(target_pointer_width = "64")]
use crate::backend::conv::{loff_t, loff_t_from_u64, ret_u64};
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "mips64",
    target_arch = "mips64r6",
    target_pointer_width = "32",
))]
use crate::fd::AsFd;
use crate::fd::{BorrowedFd, OwnedFd};
use crate::ffi::CStr;
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
use crate::fs::CWD;
use crate::fs::{
    inotify, Access, Advice, AtFlags, FallocateFlags, FileType, FlockOperation, Gid, MemfdFlags,
    Mode, OFlags, RenameFlags, ResolveFlags, SealFlags, SeekFrom, Stat, StatFs, StatVfs,
    StatVfsMountFlags, StatxFlags, Timestamps, Uid, XattrFlags,
};
use crate::io;
use core::mem::MaybeUninit;
#[cfg(any(target_arch = "mips64", target_arch = "mips64r6"))]
use linux_raw_sys::general::stat as linux_stat64;
use linux_raw_sys::general::{
    __kernel_fsid_t, open_how, statx, AT_EACCESS, AT_FDCWD, AT_REMOVEDIR, AT_SYMLINK_NOFOLLOW,
    F_ADD_SEALS, F_GETFL, F_GET_SEALS, F_SETFL, SEEK_CUR, SEEK_DATA, SEEK_END, SEEK_HOLE, SEEK_SET,
    STATX__RESERVED,
};
#[cfg(target_pointer_width = "32")]
use {
    crate::backend::conv::{hi, lo, slice_just_addr},
    linux_raw_sys::general::stat64 as linux_stat64,
    linux_raw_sys::general::timespec as __kernel_old_timespec,
};

#[inline]
pub(crate) fn open(path: &CStr, flags: OFlags, mode: Mode) -> io::Result<OwnedFd> {
    // Always enable support for large files.
    let flags = flags | OFlags::LARGEFILE;

    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    {
        openat(CWD.as_fd(), path, flags, mode)
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
    unsafe {
        ret_owned_fd(syscall_readonly!(__NR_open, path, flags, mode))
    }
}

#[inline]
pub(crate) fn openat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    flags: OFlags,
    mode: Mode,
) -> io::Result<OwnedFd> {
    // Always enable support for large files.
    let flags = flags | OFlags::LARGEFILE;

    unsafe { ret_owned_fd(syscall_readonly!(__NR_openat, dirfd, path, flags, mode)) }
}

#[inline]
pub(crate) fn openat2(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    mut flags: OFlags,
    mode: Mode,
    resolve: ResolveFlags,
) -> io::Result<OwnedFd> {
    // Enable support for large files, but not with `O_PATH` because
    // `openat2` doesn't like those flags together.
    if !flags.contains(OFlags::PATH) {
        flags |= OFlags::from_bits_retain(c::O_LARGEFILE);
    }

    unsafe {
        ret_owned_fd(syscall_readonly!(
            __NR_openat2,
            dirfd,
            path,
            by_ref(&open_how {
                flags: oflags_for_open_how(flags),
                mode: u64::from(mode.bits()),
                resolve: resolve.bits(),
            }),
            size_of::<open_how, _>()
        ))
    }
}

#[inline]
pub(crate) fn chmod(path: &CStr, mode: Mode) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_fchmodat,
            raw_fd(AT_FDCWD),
            path,
            mode
        ))
    }
}

#[inline]
pub(crate) fn chmodat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    mode: Mode,
    flags: AtFlags,
) -> io::Result<()> {
    if flags == AtFlags::SYMLINK_NOFOLLOW {
        return Err(io::Errno::OPNOTSUPP);
    }
    if !flags.is_empty() {
        return Err(io::Errno::INVAL);
    }
    unsafe { ret(syscall_readonly!(__NR_fchmodat, dirfd, path, mode)) }
}

#[inline]
pub(crate) fn fchmod(fd: BorrowedFd<'_>, mode: Mode) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_fchmod, fd, mode)) }
}

#[inline]
pub(crate) fn chownat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    owner: Option<Uid>,
    group: Option<Gid>,
    flags: AtFlags,
) -> io::Result<()> {
    unsafe {
        let (ow, gr) = crate::ugid::translate_fchown_args(owner, group);
        ret(syscall_readonly!(
            __NR_fchownat,
            dirfd,
            path,
            c_uint(ow),
            c_uint(gr),
            flags
        ))
    }
}

#[inline]
pub(crate) fn chown(path: &CStr, owner: Option<Uid>, group: Option<Gid>) -> io::Result<()> {
    // Most architectures have a `chown` syscall.
    #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
    unsafe {
        let (ow, gr) = crate::ugid::translate_fchown_args(owner, group);
        ret(syscall_readonly!(__NR_chown, path, c_uint(ow), c_uint(gr)))
    }

    // Aarch64 and RISC-V don't, so use `fchownat`.
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    unsafe {
        let (ow, gr) = crate::ugid::translate_fchown_args(owner, group);
        ret(syscall_readonly!(
            __NR_fchownat,
            raw_fd(AT_FDCWD),
            path,
            c_uint(ow),
            c_uint(gr),
            zero()
        ))
    }
}

#[inline]
pub(crate) fn fchown(fd: BorrowedFd<'_>, owner: Option<Uid>, group: Option<Gid>) -> io::Result<()> {
    unsafe {
        let (ow, gr) = crate::ugid::translate_fchown_args(owner, group);
        ret(syscall_readonly!(__NR_fchown, fd, c_uint(ow), c_uint(gr)))
    }
}

#[inline]
pub(crate) fn mknodat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    file_type: FileType,
    mode: Mode,
    dev: u64,
) -> io::Result<()> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall_readonly!(
            __NR_mknodat,
            dirfd,
            path,
            (mode, file_type),
            dev_t(dev)?
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall_readonly!(
            __NR_mknodat,
            dirfd,
            path,
            (mode, file_type),
            dev_t(dev)
        ))
    }
}

#[inline]
pub(crate) fn seek(fd: BorrowedFd<'_>, pos: SeekFrom) -> io::Result<u64> {
    let (whence, offset) = match pos {
        SeekFrom::Start(pos) => {
            let pos: u64 = pos;
            // Silently cast; we'll get `EINVAL` if the value is negative.
            (SEEK_SET, pos as i64)
        }
        SeekFrom::End(offset) => (SEEK_END, offset),
        SeekFrom::Current(offset) => (SEEK_CUR, offset),
        SeekFrom::Data(offset) => (SEEK_DATA, offset),
        SeekFrom::Hole(offset) => (SEEK_HOLE, offset),
    };
    _seek(fd, offset, whence)
}

#[inline]
pub(crate) fn _seek(fd: BorrowedFd<'_>, offset: i64, whence: c::c_uint) -> io::Result<u64> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<u64>::uninit();
        ret(syscall!(
            __NR__llseek,
            fd,
            // Don't use the hi/lo functions here because Linux's llseek
            // takes its 64-bit argument differently from everything else.
            pass_usize((offset >> 32) as usize),
            pass_usize(offset as usize),
            &mut result,
            c_uint(whence)
        ))?;
        Ok(result.assume_init())
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_u64(syscall_readonly!(
            __NR_lseek,
            fd,
            loff_t(offset),
            c_uint(whence)
        ))
    }
}

#[inline]
pub(crate) fn tell(fd: BorrowedFd<'_>) -> io::Result<u64> {
    _seek(fd, 0, SEEK_CUR).map(|x| x as u64)
}

#[inline]
pub(crate) fn ftruncate(fd: BorrowedFd<'_>, length: u64) -> io::Result<()> {
    // <https://github.com/torvalds/linux/blob/fcadab740480e0e0e9fa9bd272acd409884d431a/arch/arm64/kernel/sys32.c#L81-L83>
    #[cfg(all(
        target_pointer_width = "32",
        any(
            target_arch = "arm",
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "powerpc"
        ),
    ))]
    unsafe {
        ret(syscall_readonly!(
            __NR_ftruncate64,
            fd,
            zero(),
            hi(length),
            lo(length)
        ))
    }
    #[cfg(all(
        target_pointer_width = "32",
        not(any(
            target_arch = "arm",
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "powerpc"
        )),
    ))]
    unsafe {
        ret(syscall_readonly!(
            __NR_ftruncate64,
            fd,
            hi(length),
            lo(length)
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall_readonly!(
            __NR_ftruncate,
            fd,
            loff_t_from_u64(length)
        ))
    }
}

#[inline]
pub(crate) fn fallocate(
    fd: BorrowedFd<'_>,
    mode: FallocateFlags,
    offset: u64,
    len: u64,
) -> io::Result<()> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall_readonly!(
            __NR_fallocate,
            fd,
            mode,
            hi(offset),
            lo(offset),
            hi(len),
            lo(len)
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall_readonly!(
            __NR_fallocate,
            fd,
            mode,
            loff_t_from_u64(offset),
            loff_t_from_u64(len)
        ))
    }
}

#[inline]
pub(crate) fn fadvise(fd: BorrowedFd<'_>, pos: u64, len: u64, advice: Advice) -> io::Result<()> {
    // On ARM, the arguments are reordered so that the `len` and `pos` argument
    // pairs are aligned. And ARM has a custom syscall code for this.
    #[cfg(target_arch = "arm")]
    unsafe {
        ret(syscall_readonly!(
            __NR_arm_fadvise64_64,
            fd,
            advice,
            hi(pos),
            lo(pos),
            hi(len),
            lo(len)
        ))
    }

    // On powerpc, the arguments are reordered as on ARM.
    #[cfg(target_arch = "powerpc")]
    unsafe {
        ret(syscall_readonly!(
            __NR_fadvise64_64,
            fd,
            advice,
            hi(pos),
            lo(pos),
            hi(len),
            lo(len)
        ))
    }

    // On mips, the arguments are not reordered, and padding is inserted
    // instead to ensure alignment.
    #[cfg(any(target_arch = "mips", target_arch = "mips32r6"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_fadvise64,
            fd,
            zero(),
            hi(pos),
            lo(pos),
            hi(len),
            lo(len),
            advice
        ))
    }

    // For all other 32-bit architectures, use `fadvise64_64` so that we get a
    // 64-bit length.
    #[cfg(all(
        target_pointer_width = "32",
        not(any(
            target_arch = "arm",
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "powerpc"
        )),
    ))]
    unsafe {
        ret(syscall_readonly!(
            __NR_fadvise64_64,
            fd,
            hi(pos),
            lo(pos),
            hi(len),
            lo(len),
            advice
        ))
    }

    // On 64-bit architectures, use `fadvise64` which is sufficient.
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall_readonly!(
            __NR_fadvise64,
            fd,
            loff_t_from_u64(pos),
            loff_t_from_u64(len),
            advice
        ))
    }
}

#[inline]
pub(crate) fn fsync(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_fsync, fd)) }
}

#[inline]
pub(crate) fn fdatasync(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_fdatasync, fd)) }
}

#[inline]
pub(crate) fn flock(fd: BorrowedFd<'_>, operation: FlockOperation) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_flock,
            fd,
            c_uint(operation as c::c_uint)
        ))
    }
}

#[inline]
pub(crate) fn syncfs(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_syncfs, fd)) }
}

#[inline]
pub(crate) fn sync() {
    unsafe { ret_infallible(syscall_readonly!(__NR_sync)) }
}

#[inline]
pub(crate) fn fstat(fd: BorrowedFd<'_>) -> io::Result<Stat> {
    // 32-bit and mips64 Linux: `struct stat64` is not y2038 compatible; use
    // `statx`.
    //
    // And, some old platforms don't support `statx`, and some fail with a
    // confusing error code, so we call `crate::fs::statx` to handle that. If
    // `statx` isn't available, fall back to the buggy system call.
    #[cfg(any(
        target_pointer_width = "32",
        target_arch = "mips64",
        target_arch = "mips64r6"
    ))]
    {
        match crate::fs::statx(fd, cstr!(""), AtFlags::EMPTY_PATH, StatxFlags::BASIC_STATS) {
            Ok(x) => statx_to_stat(x),
            Err(io::Errno::NOSYS) => fstat_old(fd),
            Err(err) => Err(err),
        }
    }

    #[cfg(all(
        target_pointer_width = "64",
        not(target_arch = "mips64"),
        not(target_arch = "mips64r6")
    ))]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall!(__NR_fstat, fd, &mut result))?;
        Ok(result.assume_init())
    }
}

#[cfg(any(
    target_pointer_width = "32",
    target_arch = "mips64",
    target_arch = "mips64r6",
))]
fn fstat_old(fd: BorrowedFd<'_>) -> io::Result<Stat> {
    let mut result = MaybeUninit::<linux_stat64>::uninit();

    #[cfg(any(target_arch = "mips64", target_arch = "mips64r6"))]
    unsafe {
        ret(syscall!(__NR_fstat, fd, &mut result))?;
        stat_to_stat(result.assume_init())
    }

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall!(__NR_fstat64, fd, &mut result))?;
        stat_to_stat(result.assume_init())
    }
}

#[inline]
pub(crate) fn stat(path: &CStr) -> io::Result<Stat> {
    // See the comments in `fstat` about using `crate::fs::statx` here.
    #[cfg(any(
        target_pointer_width = "32",
        target_arch = "mips64",
        target_arch = "mips64r6"
    ))]
    {
        match crate::fs::statx(
            crate::fs::CWD.as_fd(),
            path,
            AtFlags::empty(),
            StatxFlags::BASIC_STATS,
        ) {
            Ok(x) => statx_to_stat(x),
            Err(io::Errno::NOSYS) => stat_old(path),
            Err(err) => Err(err),
        }
    }

    #[cfg(all(
        target_pointer_width = "64",
        not(target_arch = "mips64"),
        not(target_arch = "mips64r6"),
    ))]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall!(
            __NR_newfstatat,
            raw_fd(AT_FDCWD),
            path,
            &mut result,
            c_uint(0)
        ))?;
        Ok(result.assume_init())
    }
}

#[cfg(any(
    target_pointer_width = "32",
    target_arch = "mips64",
    target_arch = "mips64r6"
))]
fn stat_old(path: &CStr) -> io::Result<Stat> {
    let mut result = MaybeUninit::<linux_stat64>::uninit();

    #[cfg(any(target_arch = "mips64", target_arch = "mips64r6"))]
    unsafe {
        ret(syscall!(
            __NR_newfstatat,
            raw_fd(AT_FDCWD),
            path,
            &mut result,
            c_uint(0)
        ))?;
        stat_to_stat(result.assume_init())
    }

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall!(
            __NR_fstatat64,
            raw_fd(AT_FDCWD),
            path,
            &mut result,
            c_uint(0)
        ))?;
        stat_to_stat(result.assume_init())
    }
}

#[inline]
pub(crate) fn statat(dirfd: BorrowedFd<'_>, path: &CStr, flags: AtFlags) -> io::Result<Stat> {
    // See the comments in `fstat` about using `crate::fs::statx` here.
    #[cfg(any(
        target_pointer_width = "32",
        target_arch = "mips64",
        target_arch = "mips64r6"
    ))]
    {
        match crate::fs::statx(dirfd, path, flags, StatxFlags::BASIC_STATS) {
            Ok(x) => statx_to_stat(x),
            Err(io::Errno::NOSYS) => statat_old(dirfd, path, flags),
            Err(err) => Err(err),
        }
    }

    #[cfg(all(
        target_pointer_width = "64",
        not(target_arch = "mips64"),
        not(target_arch = "mips64r6"),
    ))]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall!(__NR_newfstatat, dirfd, path, &mut result, flags))?;
        Ok(result.assume_init())
    }
}

#[cfg(any(
    target_pointer_width = "32",
    target_arch = "mips64",
    target_arch = "mips64r6"
))]
fn statat_old(dirfd: BorrowedFd<'_>, path: &CStr, flags: AtFlags) -> io::Result<Stat> {
    let mut result = MaybeUninit::<linux_stat64>::uninit();

    #[cfg(any(target_arch = "mips64", target_arch = "mips64r6"))]
    unsafe {
        ret(syscall!(__NR_newfstatat, dirfd, path, &mut result, flags))?;
        stat_to_stat(result.assume_init())
    }

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall!(__NR_fstatat64, dirfd, path, &mut result, flags))?;
        stat_to_stat(result.assume_init())
    }
}

#[inline]
pub(crate) fn lstat(path: &CStr) -> io::Result<Stat> {
    // See the comments in `fstat` about using `crate::fs::statx` here.
    #[cfg(any(target_pointer_width = "32", target_arch = "mips64"))]
    {
        match crate::fs::statx(
            crate::fs::CWD.as_fd(),
            path,
            AtFlags::SYMLINK_NOFOLLOW,
            StatxFlags::BASIC_STATS,
        ) {
            Ok(x) => statx_to_stat(x),
            Err(io::Errno::NOSYS) => lstat_old(path),
            Err(err) => Err(err),
        }
    }

    #[cfg(all(target_pointer_width = "64", not(target_arch = "mips64")))]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall!(
            __NR_newfstatat,
            raw_fd(AT_FDCWD),
            path,
            &mut result,
            c_uint(AT_SYMLINK_NOFOLLOW)
        ))?;
        Ok(result.assume_init())
    }
}

#[cfg(any(target_pointer_width = "32", target_arch = "mips64"))]
fn lstat_old(path: &CStr) -> io::Result<Stat> {
    let mut result = MaybeUninit::<linux_stat64>::uninit();

    #[cfg(any(target_arch = "mips64", target_arch = "mips64r6"))]
    unsafe {
        ret(syscall!(
            __NR_newfstatat,
            raw_fd(AT_FDCWD),
            path,
            &mut result,
            c_uint(AT_SYMLINK_NOFOLLOW)
        ))?;
        stat_to_stat(result.assume_init())
    }

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall!(
            __NR_fstatat64,
            raw_fd(AT_FDCWD),
            path,
            &mut result,
            c_uint(AT_SYMLINK_NOFOLLOW)
        ))?;
        stat_to_stat(result.assume_init())
    }
}

/// Convert from a Linux `statx` value to rustix's `Stat`.
#[cfg(any(
    target_pointer_width = "32",
    target_arch = "mips64",
    target_arch = "mips64r6"
))]
#[allow(deprecated)] // for `st_[amc]time` u64->i64 transition
fn statx_to_stat(x: crate::fs::Statx) -> io::Result<Stat> {
    Ok(Stat {
        st_dev: crate::fs::makedev(x.stx_dev_major, x.stx_dev_minor),
        st_mode: x.stx_mode.into(),
        st_nlink: x.stx_nlink.into(),
        st_uid: x.stx_uid.into(),
        st_gid: x.stx_gid.into(),
        st_rdev: crate::fs::makedev(x.stx_rdev_major, x.stx_rdev_minor),
        st_size: x.stx_size.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_blksize: x.stx_blksize.into(),
        st_blocks: x.stx_blocks.into(),
        st_atime: bitcast!(i64::from(x.stx_atime.tv_sec)),
        st_atime_nsec: x.stx_atime.tv_nsec.into(),
        st_mtime: bitcast!(i64::from(x.stx_mtime.tv_sec)),
        st_mtime_nsec: x.stx_mtime.tv_nsec.into(),
        st_ctime: bitcast!(i64::from(x.stx_ctime.tv_sec)),
        st_ctime_nsec: x.stx_ctime.tv_nsec.into(),
        st_ino: x.stx_ino.into(),
    })
}

/// Convert from a Linux `stat64` value to rustix's `Stat`.
#[cfg(target_pointer_width = "32")]
#[allow(deprecated)] // for `st_[amc]time` u64->i64 transition
fn stat_to_stat(s64: linux_raw_sys::general::stat64) -> io::Result<Stat> {
    Ok(Stat {
        st_dev: s64.st_dev.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_mode: s64.st_mode.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_nlink: s64.st_nlink.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_uid: s64.st_uid.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_gid: s64.st_gid.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_rdev: s64.st_rdev.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_size: s64.st_size.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_blksize: s64.st_blksize.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_blocks: s64.st_blocks.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_atime: bitcast!(i64::from(s64.st_atime)),
        st_atime_nsec: s64
            .st_atime_nsec
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        st_mtime: bitcast!(i64::from(s64.st_mtime)),
        st_mtime_nsec: s64
            .st_mtime_nsec
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        st_ctime: bitcast!(i64::from(s64.st_ctime)),
        st_ctime_nsec: s64
            .st_ctime_nsec
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        st_ino: s64.st_ino.try_into().map_err(|_| io::Errno::OVERFLOW)?,
    })
}

/// Convert from a Linux `stat` value to rustix's `Stat`.
#[cfg(any(target_arch = "mips64", target_arch = "mips64r6"))]
fn stat_to_stat(s: linux_raw_sys::general::stat) -> io::Result<Stat> {
    Ok(Stat {
        st_dev: s.st_dev.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_mode: s.st_mode.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_nlink: s.st_nlink.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_uid: s.st_uid.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_gid: s.st_gid.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_rdev: s.st_rdev.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_size: s.st_size.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_blksize: s.st_blksize.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_blocks: s.st_blocks.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_atime: bitcast!(i64::from(s.st_atime)),
        st_atime_nsec: s
            .st_atime_nsec
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        st_mtime: bitcast!(i64::from(s.st_mtime)),
        st_mtime_nsec: s
            .st_mtime_nsec
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        st_ctime: bitcast!(i64::from(s.st_ctime)),
        st_ctime_nsec: s
            .st_ctime_nsec
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        st_ino: s.st_ino.try_into().map_err(|_| io::Errno::OVERFLOW)?,
    })
}

#[inline]
pub(crate) fn statx(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    flags: AtFlags,
    mask: StatxFlags,
) -> io::Result<statx> {
    // If a future Linux kernel adds more fields to `struct statx` and users
    // passing flags unknown to rustix in `StatxFlags`, we could end up
    // writing outside of the buffer. To prevent this possibility, we mask off
    // any flags that we don't know about.
    //
    // This includes `STATX__RESERVED`, which has a value that we know, but
    // which could take on arbitrary new meaning in the future. Linux currently
    // rejects this flag with `EINVAL`, so we do the same.
    //
    // This doesn't rely on `STATX_ALL` because [it's deprecated] and already
    // doesn't represent all the known flags.
    //
    // [it's deprecated]: https://patchwork.kernel.org/project/linux-fsdevel/patch/20200505095915.11275-7-mszeredi@redhat.com/
    if (mask.bits() & STATX__RESERVED) == STATX__RESERVED {
        return Err(io::Errno::INVAL);
    }
    let mask = mask & StatxFlags::all();

    unsafe {
        let mut statx_buf = MaybeUninit::<statx>::uninit();
        ret(syscall!(
            __NR_statx,
            dirfd,
            path,
            flags,
            mask,
            &mut statx_buf
        ))?;
        Ok(statx_buf.assume_init())
    }
}

#[cfg(not(feature = "linux_4_11"))]
#[inline]
pub(crate) fn is_statx_available() -> bool {
    unsafe {
        // Call `statx` with null pointers so that if it fails for any reason
        // other than `EFAULT`, we know it's not supported. This can use
        // "readonly" because we don't pass it a buffer to mutate.
        matches!(
            ret(syscall_readonly!(
                __NR_statx,
                raw_fd(AT_FDCWD),
                zero(),
                zero(),
                zero(),
                zero()
            )),
            Err(io::Errno::FAULT)
        )
    }
}

#[inline]
pub(crate) fn fstatfs(fd: BorrowedFd<'_>) -> io::Result<StatFs> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<StatFs>::uninit();
        ret(syscall!(
            __NR_fstatfs64,
            fd,
            size_of::<StatFs, _>(),
            &mut result
        ))?;
        Ok(result.assume_init())
    }

    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut result = MaybeUninit::<StatFs>::uninit();
        ret(syscall!(__NR_fstatfs, fd, &mut result))?;
        Ok(result.assume_init())
    }
}

#[inline]
pub(crate) fn fstatvfs(fd: BorrowedFd<'_>) -> io::Result<StatVfs> {
    // Linux doesn't have an `fstatvfs` syscall; we have to do `fstatfs` and
    // translate the fields as best we can.
    let statfs = fstatfs(fd)?;

    Ok(statfs_to_statvfs(statfs))
}

#[inline]
pub(crate) fn statfs(path: &CStr) -> io::Result<StatFs> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<StatFs>::uninit();
        ret(syscall!(
            __NR_statfs64,
            path,
            size_of::<StatFs, _>(),
            &mut result
        ))?;
        Ok(result.assume_init())
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut result = MaybeUninit::<StatFs>::uninit();
        ret(syscall!(__NR_statfs, path, &mut result))?;
        Ok(result.assume_init())
    }
}

#[inline]
pub(crate) fn statvfs(path: &CStr) -> io::Result<StatVfs> {
    // Linux doesn't have a `statvfs` syscall; we have to do `statfs` and
    // translate the fields as best we can.
    let statfs = statfs(path)?;

    Ok(statfs_to_statvfs(statfs))
}

fn statfs_to_statvfs(statfs: StatFs) -> StatVfs {
    let __kernel_fsid_t { val } = statfs.f_fsid;
    let [f_fsid_val0, f_fsid_val1]: [i32; 2] = val;

    StatVfs {
        f_bsize: statfs.f_bsize as u64,
        f_frsize: if statfs.f_frsize != 0 {
            statfs.f_frsize
        } else {
            statfs.f_bsize
        } as u64,
        f_blocks: statfs.f_blocks as u64,
        f_bfree: statfs.f_bfree as u64,
        f_bavail: statfs.f_bavail as u64,
        f_files: statfs.f_files as u64,
        f_ffree: statfs.f_ffree as u64,
        f_favail: statfs.f_ffree as u64,
        f_fsid: u64::from(f_fsid_val0 as u32) | u64::from(f_fsid_val1 as u32) << 32,
        f_flag: StatVfsMountFlags::from_bits_retain(statfs.f_flags as u64),
        f_namemax: statfs.f_namelen as u64,
    }
}

#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn readlink(path: &CStr, buf: &mut [u8]) -> io::Result<usize> {
    let (buf_addr_mut, buf_len) = slice_mut(buf);
    unsafe {
        ret_usize(syscall!(
            __NR_readlinkat,
            raw_fd(AT_FDCWD),
            path,
            buf_addr_mut,
            buf_len
        ))
    }
}

#[inline]
pub(crate) fn readlinkat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    buf: &mut [MaybeUninit<u8>],
) -> io::Result<usize> {
    let (buf_addr_mut, buf_len) = slice_mut(buf);
    unsafe {
        ret_usize(syscall!(
            __NR_readlinkat,
            dirfd,
            path,
            buf_addr_mut,
            buf_len
        ))
    }
}

#[inline]
pub(crate) fn fcntl_getfl(fd: BorrowedFd<'_>) -> io::Result<OFlags> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_c_uint(syscall_readonly!(__NR_fcntl64, fd, c_uint(F_GETFL)))
            .map(OFlags::from_bits_retain)
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_c_uint(syscall_readonly!(__NR_fcntl, fd, c_uint(F_GETFL))).map(OFlags::from_bits_retain)
    }
}

#[inline]
pub(crate) fn fcntl_setfl(fd: BorrowedFd<'_>, flags: OFlags) -> io::Result<()> {
    // Always enable support for large files.
    let flags = flags | OFlags::from_bits_retain(c::O_LARGEFILE);

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall_readonly!(__NR_fcntl64, fd, c_uint(F_SETFL), flags))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall_readonly!(__NR_fcntl, fd, c_uint(F_SETFL), flags))
    }
}

#[inline]
pub(crate) fn fcntl_get_seals(fd: BorrowedFd<'_>) -> io::Result<SealFlags> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_c_int(syscall_readonly!(__NR_fcntl64, fd, c_uint(F_GET_SEALS)))
            .map(|seals| SealFlags::from_bits_retain(seals as u32))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_c_int(syscall_readonly!(__NR_fcntl, fd, c_uint(F_GET_SEALS)))
            .map(|seals| SealFlags::from_bits_retain(seals as u32))
    }
}

#[inline]
pub(crate) fn fcntl_add_seals(fd: BorrowedFd<'_>, seals: SealFlags) -> io::Result<()> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall_readonly!(
            __NR_fcntl64,
            fd,
            c_uint(F_ADD_SEALS),
            seals
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall_readonly!(
            __NR_fcntl,
            fd,
            c_uint(F_ADD_SEALS),
            seals
        ))
    }
}

#[inline]
pub(crate) fn fcntl_lock(fd: BorrowedFd<'_>, operation: FlockOperation) -> io::Result<()> {
    #[cfg(target_pointer_width = "64")]
    use linux_raw_sys::general::{flock, F_SETLK, F_SETLKW};
    #[cfg(target_pointer_width = "32")]
    use linux_raw_sys::general::{flock64 as flock, F_SETLK64 as F_SETLK, F_SETLKW64 as F_SETLKW};
    use linux_raw_sys::general::{F_RDLCK, F_UNLCK, F_WRLCK};

    let (cmd, l_type) = match operation {
        FlockOperation::LockShared => (F_SETLKW, F_RDLCK),
        FlockOperation::LockExclusive => (F_SETLKW, F_WRLCK),
        FlockOperation::Unlock => (F_SETLKW, F_UNLCK),
        FlockOperation::NonBlockingLockShared => (F_SETLK, F_RDLCK),
        FlockOperation::NonBlockingLockExclusive => (F_SETLK, F_WRLCK),
        FlockOperation::NonBlockingUnlock => (F_SETLK, F_UNLCK),
    };

    let lock = flock {
        l_type: l_type as _,

        // When `l_len` is zero, this locks all the bytes from
        // `l_whence`/`l_start` to the end of the file, even as the
        // file grows dynamically.
        l_whence: SEEK_SET as _,
        l_start: 0,
        l_len: 0,

        // Unused.
        l_pid: 0,
    };

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall_readonly!(
            __NR_fcntl64,
            fd,
            c_uint(cmd),
            by_ref(&lock)
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall_readonly!(
            __NR_fcntl,
            fd,
            c_uint(cmd),
            by_ref(&lock)
        ))
    }
}

#[inline]
pub(crate) fn rename(old_path: &CStr, new_path: &CStr) -> io::Result<()> {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        ret(syscall_readonly!(
            __NR_renameat2,
            raw_fd(AT_FDCWD),
            old_path,
            raw_fd(AT_FDCWD),
            new_path,
            c_uint(0)
        ))
    }
    #[cfg(not(target_arch = "riscv64"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_renameat,
            raw_fd(AT_FDCWD),
            old_path,
            raw_fd(AT_FDCWD),
            new_path
        ))
    }
}

#[inline]
pub(crate) fn renameat(
    old_dirfd: BorrowedFd<'_>,
    old_path: &CStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &CStr,
) -> io::Result<()> {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        ret(syscall_readonly!(
            __NR_renameat2,
            old_dirfd,
            old_path,
            new_dirfd,
            new_path,
            c_uint(0)
        ))
    }
    #[cfg(not(target_arch = "riscv64"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_renameat,
            old_dirfd,
            old_path,
            new_dirfd,
            new_path
        ))
    }
}

#[inline]
pub(crate) fn renameat2(
    old_dirfd: BorrowedFd<'_>,
    old_path: &CStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &CStr,
    flags: RenameFlags,
) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_renameat2,
            old_dirfd,
            old_path,
            new_dirfd,
            new_path,
            flags
        ))
    }
}

#[inline]
pub(crate) fn unlink(path: &CStr) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_unlinkat,
            raw_fd(AT_FDCWD),
            path,
            c_uint(0)
        ))
    }
}

#[inline]
pub(crate) fn unlinkat(dirfd: BorrowedFd<'_>, path: &CStr, flags: AtFlags) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_unlinkat, dirfd, path, flags)) }
}

#[inline]
pub(crate) fn rmdir(path: &CStr) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_unlinkat,
            raw_fd(AT_FDCWD),
            path,
            c_uint(AT_REMOVEDIR)
        ))
    }
}

#[inline]
pub(crate) fn link(old_path: &CStr, new_path: &CStr) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_linkat,
            raw_fd(AT_FDCWD),
            old_path,
            raw_fd(AT_FDCWD),
            new_path,
            c_uint(0)
        ))
    }
}

#[inline]
pub(crate) fn linkat(
    old_dirfd: BorrowedFd<'_>,
    old_path: &CStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &CStr,
    flags: AtFlags,
) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_linkat,
            old_dirfd,
            old_path,
            new_dirfd,
            new_path,
            flags
        ))
    }
}

#[inline]
pub(crate) fn symlink(old_path: &CStr, new_path: &CStr) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_symlinkat,
            old_path,
            raw_fd(AT_FDCWD),
            new_path
        ))
    }
}

#[inline]
pub(crate) fn symlinkat(old_path: &CStr, dirfd: BorrowedFd<'_>, new_path: &CStr) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_symlinkat, old_path, dirfd, new_path)) }
}

#[inline]
pub(crate) fn mkdir(path: &CStr, mode: Mode) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_mkdirat,
            raw_fd(AT_FDCWD),
            path,
            mode
        ))
    }
}

#[inline]
pub(crate) fn mkdirat(dirfd: BorrowedFd<'_>, path: &CStr, mode: Mode) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_mkdirat, dirfd, path, mode)) }
}

#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn getdents(fd: BorrowedFd<'_>, dirent: &mut [u8]) -> io::Result<usize> {
    let (dirent_addr_mut, dirent_len) = slice_mut(dirent);

    unsafe { ret_usize(syscall!(__NR_getdents64, fd, dirent_addr_mut, dirent_len)) }
}

#[inline]
pub(crate) fn getdents_uninit(
    fd: BorrowedFd<'_>,
    dirent: &mut [MaybeUninit<u8>],
) -> io::Result<usize> {
    let (dirent_addr_mut, dirent_len) = slice_mut(dirent);

    unsafe { ret_usize(syscall!(__NR_getdents64, fd, dirent_addr_mut, dirent_len)) }
}

#[inline]
pub(crate) fn utimensat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    times: &Timestamps,
    flags: AtFlags,
) -> io::Result<()> {
    _utimensat(dirfd, Some(path), times, flags)
}

#[inline]
fn _utimensat(
    dirfd: BorrowedFd<'_>,
    path: Option<&CStr>,
    times: &Timestamps,
    flags: AtFlags,
) -> io::Result<()> {
    // `utimensat_time64` was introduced in Linux 5.1. The old `utimensat`
    // syscall is not y2038-compatible on 32-bit architectures.
    #[cfg(target_pointer_width = "32")]
    unsafe {
        match ret(syscall_readonly!(
            __NR_utimensat_time64,
            dirfd,
            path,
            by_ref(times),
            flags
        )) {
            Err(io::Errno::NOSYS) => _utimensat_old(dirfd, path, times, flags),
            otherwise => otherwise,
        }
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall_readonly!(
            __NR_utimensat,
            dirfd,
            path,
            by_ref(times),
            flags
        ))
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn _utimensat_old(
    dirfd: BorrowedFd<'_>,
    path: Option<&CStr>,
    times: &Timestamps,
    flags: AtFlags,
) -> io::Result<()> {
    // See the comments in `rustix_clock_gettime_via_syscall` about
    // emulation.
    let old_times = [
        __kernel_old_timespec {
            tv_sec: times
                .last_access
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: times
                .last_access
                .tv_nsec
                .try_into()
                .map_err(|_| io::Errno::INVAL)?,
        },
        __kernel_old_timespec {
            tv_sec: times
                .last_modification
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: times
                .last_modification
                .tv_nsec
                .try_into()
                .map_err(|_| io::Errno::INVAL)?,
        },
    ];
    // The length of the array is fixed and not passed into the syscall.
    let old_times_addr = slice_just_addr(&old_times);
    ret(syscall_readonly!(
        __NR_utimensat,
        dirfd,
        path,
        old_times_addr,
        flags
    ))
}

#[inline]
pub(crate) fn futimens(fd: BorrowedFd<'_>, times: &Timestamps) -> io::Result<()> {
    _utimensat(fd, None, times, AtFlags::empty())
}

#[inline]
pub(crate) fn access(path: &CStr, access: Access) -> io::Result<()> {
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    {
        accessat_noflags(CWD.as_fd(), path, access)
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
    unsafe {
        ret(syscall_readonly!(__NR_access, path, access))
    }
}

pub(crate) fn accessat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    access: Access,
    flags: AtFlags,
) -> io::Result<()> {
    if !flags
        .difference(AtFlags::EACCESS | AtFlags::SYMLINK_NOFOLLOW)
        .is_empty()
    {
        return Err(io::Errno::INVAL);
    }

    // Linux's `faccessat` syscall doesn't have a flags argument, so if we have
    // any flags, use the newer `faccessat2` introduced in Linux 5.8 which
    // does. Unless we're on Android where using newer system calls can cause
    // seccomp to abort the process.
    #[cfg(not(target_os = "android"))]
    if !flags.is_empty() {
        unsafe {
            match ret(syscall_readonly!(
                __NR_faccessat2,
                dirfd,
                path,
                access,
                flags
            )) {
                Ok(()) => return Ok(()),
                Err(io::Errno::NOSYS) => {}
                Err(other) => return Err(other),
            }
        }
    }

    // Linux's `faccessat` doesn't have a flags parameter. If we have
    // `AT_EACCESS` and we're not setuid or setgid, we can emulate it.
    if flags.is_empty()
        || (flags.bits() == AT_EACCESS
            && crate::backend::ugid::syscalls::getuid()
                == crate::backend::ugid::syscalls::geteuid()
            && crate::backend::ugid::syscalls::getgid()
                == crate::backend::ugid::syscalls::getegid())
    {
        return accessat_noflags(dirfd, path, access);
    }

    Err(io::Errno::NOSYS)
}

#[inline]
fn accessat_noflags(dirfd: BorrowedFd<'_>, path: &CStr, access: Access) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_faccessat, dirfd, path, access)) }
}

#[inline]
pub(crate) fn copy_file_range(
    fd_in: BorrowedFd<'_>,
    off_in: Option<&mut u64>,
    fd_out: BorrowedFd<'_>,
    off_out: Option<&mut u64>,
    len: usize,
) -> io::Result<usize> {
    unsafe {
        ret_usize(syscall!(
            __NR_copy_file_range,
            fd_in,
            opt_mut(off_in),
            fd_out,
            opt_mut(off_out),
            pass_usize(len),
            c_uint(0)
        ))
    }
}

#[inline]
pub(crate) fn memfd_create(name: &CStr, flags: MemfdFlags) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(syscall_readonly!(__NR_memfd_create, name, flags)) }
}

#[inline]
pub(crate) fn sendfile(
    out_fd: BorrowedFd<'_>,
    in_fd: BorrowedFd<'_>,
    offset: Option<&mut u64>,
    count: usize,
) -> io::Result<usize> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_usize(syscall!(
            __NR_sendfile64,
            out_fd,
            in_fd,
            opt_mut(offset),
            pass_usize(count)
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall!(
            __NR_sendfile,
            out_fd,
            in_fd,
            opt_mut(offset),
            pass_usize(count)
        ))
    }
}

#[inline]
pub(crate) fn inotify_init1(flags: inotify::CreateFlags) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(syscall_readonly!(__NR_inotify_init1, flags)) }
}

#[inline]
pub(crate) fn inotify_add_watch(
    infd: BorrowedFd<'_>,
    path: &CStr,
    flags: inotify::WatchFlags,
) -> io::Result<i32> {
    unsafe { ret_c_int(syscall_readonly!(__NR_inotify_add_watch, infd, path, flags)) }
}

#[inline]
pub(crate) fn inotify_rm_watch(infd: BorrowedFd<'_>, wfd: i32) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_inotify_rm_watch, infd, c_int(wfd))) }
}

#[inline]
pub(crate) fn getxattr(path: &CStr, name: &CStr, value: &mut [u8]) -> io::Result<usize> {
    let (value_addr_mut, value_len) = slice_mut(value);
    unsafe {
        ret_usize(syscall!(
            __NR_getxattr,
            path,
            name,
            value_addr_mut,
            value_len
        ))
    }
}

#[inline]
pub(crate) fn lgetxattr(path: &CStr, name: &CStr, value: &mut [u8]) -> io::Result<usize> {
    let (value_addr_mut, value_len) = slice_mut(value);
    unsafe {
        ret_usize(syscall!(
            __NR_lgetxattr,
            path,
            name,
            value_addr_mut,
            value_len
        ))
    }
}

#[inline]
pub(crate) fn fgetxattr(fd: BorrowedFd<'_>, name: &CStr, value: &mut [u8]) -> io::Result<usize> {
    let (value_addr_mut, value_len) = slice_mut(value);
    unsafe {
        ret_usize(syscall!(
            __NR_fgetxattr,
            fd,
            name,
            value_addr_mut,
            value_len
        ))
    }
}

#[inline]
pub(crate) fn setxattr(
    path: &CStr,
    name: &CStr,
    value: &[u8],
    flags: XattrFlags,
) -> io::Result<()> {
    let (value_addr, value_len) = slice(value);
    unsafe {
        ret(syscall_readonly!(
            __NR_setxattr,
            path,
            name,
            value_addr,
            value_len,
            flags
        ))
    }
}

#[inline]
pub(crate) fn lsetxattr(
    path: &CStr,
    name: &CStr,
    value: &[u8],
    flags: XattrFlags,
) -> io::Result<()> {
    let (value_addr, value_len) = slice(value);
    unsafe {
        ret(syscall_readonly!(
            __NR_lsetxattr,
            path,
            name,
            value_addr,
            value_len,
            flags
        ))
    }
}

#[inline]
pub(crate) fn fsetxattr(
    fd: BorrowedFd<'_>,
    name: &CStr,
    value: &[u8],
    flags: XattrFlags,
) -> io::Result<()> {
    let (value_addr, value_len) = slice(value);
    unsafe {
        ret(syscall_readonly!(
            __NR_fsetxattr,
            fd,
            name,
            value_addr,
            value_len,
            flags
        ))
    }
}

#[inline]
pub(crate) fn listxattr(path: &CStr, list: &mut [c::c_char]) -> io::Result<usize> {
    let (list_addr_mut, list_len) = slice_mut(list);
    unsafe { ret_usize(syscall!(__NR_listxattr, path, list_addr_mut, list_len)) }
}

#[inline]
pub(crate) fn llistxattr(path: &CStr, list: &mut [c::c_char]) -> io::Result<usize> {
    let (list_addr_mut, list_len) = slice_mut(list);
    unsafe { ret_usize(syscall!(__NR_llistxattr, path, list_addr_mut, list_len)) }
}

#[inline]
pub(crate) fn flistxattr(fd: BorrowedFd<'_>, list: &mut [c::c_char]) -> io::Result<usize> {
    let (list_addr_mut, list_len) = slice_mut(list);
    unsafe { ret_usize(syscall!(__NR_flistxattr, fd, list_addr_mut, list_len)) }
}

#[inline]
pub(crate) fn removexattr(path: &CStr, name: &CStr) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_removexattr, path, name)) }
}

#[inline]
pub(crate) fn lremovexattr(path: &CStr, name: &CStr) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_lremovexattr, path, name)) }
}

#[inline]
pub(crate) fn fremovexattr(fd: BorrowedFd<'_>, name: &CStr) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_fremovexattr, fd, name)) }
}

#[test]
fn test_sizes() {
    assert_eq_size!(linux_raw_sys::general::__kernel_loff_t, u64);

    // Assert that `Timestamps` has the expected layout.
    assert_eq_size!([linux_raw_sys::general::__kernel_timespec; 2], Timestamps);
}
