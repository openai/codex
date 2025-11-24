//! linux_raw syscalls supporting `rustix::io`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code)]
#![allow(clippy::undocumented_unsafe_blocks)]

#[cfg(target_pointer_width = "64")]
use crate::backend::conv::loff_t_from_u64;
#[cfg(all(
    target_pointer_width = "32",
    any(
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "powerpc"
    ),
))]
use crate::backend::conv::zero;
use crate::backend::conv::{
    c_uint, pass_usize, raw_fd, ret, ret_c_int, ret_c_uint, ret_discarded_fd, ret_owned_fd,
    ret_usize, slice,
};
#[cfg(target_pointer_width = "32")]
use crate::backend::conv::{hi, lo};
use crate::backend::{c, MAX_IOV};
use crate::fd::{AsFd as _, BorrowedFd, OwnedFd, RawFd};
use crate::io::{self, DupFlags, FdFlags, IoSlice, IoSliceMut, ReadWriteFlags};
use crate::ioctl::{IoctlOutput, Opcode};
use core::cmp;
use linux_raw_sys::general::{F_DUPFD_CLOEXEC, F_GETFD, F_SETFD};

#[inline]
pub(crate) unsafe fn read(fd: BorrowedFd<'_>, buf: (*mut u8, usize)) -> io::Result<usize> {
    ret_usize(syscall!(__NR_read, fd, buf.0, pass_usize(buf.1)))
}

#[inline]
pub(crate) unsafe fn pread(
    fd: BorrowedFd<'_>,
    buf: (*mut u8, usize),
    pos: u64,
) -> io::Result<usize> {
    // <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/arch/arm64/kernel/sys32.c?h=v6.13#n70>
    #[cfg(all(
        target_pointer_width = "32",
        any(
            target_arch = "arm",
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "powerpc"
        ),
    ))]
    {
        ret_usize(syscall!(
            __NR_pread64,
            fd,
            buf.0,
            pass_usize(buf.1),
            zero(),
            hi(pos),
            lo(pos)
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
    {
        ret_usize(syscall!(
            __NR_pread64,
            fd,
            buf.0,
            pass_usize(buf.1),
            hi(pos),
            lo(pos)
        ))
    }
    #[cfg(target_pointer_width = "64")]
    ret_usize(syscall!(
        __NR_pread64,
        fd,
        buf.0,
        pass_usize(buf.1),
        loff_t_from_u64(pos)
    ))
}

#[inline]
pub(crate) fn readv(fd: BorrowedFd<'_>, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), MAX_IOV)]);

    unsafe { ret_usize(syscall!(__NR_readv, fd, bufs_addr, bufs_len)) }
}

#[inline]
pub(crate) fn preadv(
    fd: BorrowedFd<'_>,
    bufs: &mut [IoSliceMut<'_>],
    pos: u64,
) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), MAX_IOV)]);

    // Unlike the plain "p" functions, the "pv" functions pass their offset in
    // an endian-independent way, and always in two registers.
    unsafe {
        ret_usize(syscall!(
            __NR_preadv,
            fd,
            bufs_addr,
            bufs_len,
            pass_usize(pos as usize),
            pass_usize((pos >> 32) as usize)
        ))
    }
}

#[inline]
pub(crate) fn preadv2(
    fd: BorrowedFd<'_>,
    bufs: &mut [IoSliceMut<'_>],
    pos: u64,
    flags: ReadWriteFlags,
) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), MAX_IOV)]);

    // Unlike the plain "p" functions, the "pv" functions pass their offset in
    // an endian-independent way, and always in two registers.
    unsafe {
        ret_usize(syscall!(
            __NR_preadv2,
            fd,
            bufs_addr,
            bufs_len,
            pass_usize(pos as usize),
            pass_usize((pos >> 32) as usize),
            flags
        ))
    }
}

#[inline]
pub(crate) fn write(fd: BorrowedFd<'_>, buf: &[u8]) -> io::Result<usize> {
    let (buf_addr, buf_len) = slice(buf);

    unsafe { ret_usize(syscall_readonly!(__NR_write, fd, buf_addr, buf_len)) }
}

#[inline]
pub(crate) fn pwrite(fd: BorrowedFd<'_>, buf: &[u8], pos: u64) -> io::Result<usize> {
    let (buf_addr, buf_len) = slice(buf);

    // <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/arch/arm64/kernel/sys32.c?h=v6.13#n76>
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
        ret_usize(syscall_readonly!(
            __NR_pwrite64,
            fd,
            buf_addr,
            buf_len,
            zero(),
            hi(pos),
            lo(pos)
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
        ret_usize(syscall_readonly!(
            __NR_pwrite64,
            fd,
            buf_addr,
            buf_len,
            hi(pos),
            lo(pos)
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_pwrite64,
            fd,
            buf_addr,
            buf_len,
            loff_t_from_u64(pos)
        ))
    }
}

#[inline]
pub(crate) fn writev(fd: BorrowedFd<'_>, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), MAX_IOV)]);

    unsafe { ret_usize(syscall_readonly!(__NR_writev, fd, bufs_addr, bufs_len)) }
}

#[inline]
pub(crate) fn pwritev(fd: BorrowedFd<'_>, bufs: &[IoSlice<'_>], pos: u64) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), MAX_IOV)]);

    // Unlike the plain "p" functions, the "pv" functions pass their offset in
    // an endian-independent way, and always in two registers.
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_pwritev,
            fd,
            bufs_addr,
            bufs_len,
            pass_usize(pos as usize),
            pass_usize((pos >> 32) as usize)
        ))
    }
}

#[inline]
pub(crate) fn pwritev2(
    fd: BorrowedFd<'_>,
    bufs: &[IoSlice<'_>],
    pos: u64,
    flags: ReadWriteFlags,
) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), MAX_IOV)]);

    // Unlike the plain "p" functions, the "pv" functions pass their offset in
    // an endian-independent way, and always in two registers.
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_pwritev2,
            fd,
            bufs_addr,
            bufs_len,
            pass_usize(pos as usize),
            pass_usize((pos >> 32) as usize),
            flags
        ))
    }
}

#[inline]
pub(crate) unsafe fn close(fd: RawFd) {
    // See the documentation for [`io::close`] for why errors are ignored.
    syscall_readonly!(__NR_close, raw_fd(fd)).decode_void();
}

#[cfg(feature = "try_close")]
#[inline]
pub(crate) unsafe fn try_close(fd: RawFd) -> io::Result<()> {
    ret(syscall_readonly!(__NR_close, raw_fd(fd)))
}

#[inline]
pub(crate) unsafe fn ioctl(
    fd: BorrowedFd<'_>,
    request: Opcode,
    arg: *mut c::c_void,
) -> io::Result<IoctlOutput> {
    ret_c_int(syscall!(__NR_ioctl, fd, c_uint(request), arg))
}

#[inline]
pub(crate) unsafe fn ioctl_readonly(
    fd: BorrowedFd<'_>,
    request: Opcode,
    arg: *mut c::c_void,
) -> io::Result<IoctlOutput> {
    ret_c_int(syscall_readonly!(__NR_ioctl, fd, c_uint(request), arg))
}

#[inline]
pub(crate) fn dup(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(syscall_readonly!(__NR_dup, fd)) }
}

#[allow(clippy::needless_pass_by_ref_mut)]
#[inline]
pub(crate) fn dup2(fd: BorrowedFd<'_>, new: &mut OwnedFd) -> io::Result<()> {
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    {
        // We don't need to worry about the difference between `dup2` and
        // `dup3` when the file descriptors are equal because we have an
        // `&mut OwnedFd` which means `fd` doesn't alias it.
        dup3(fd, new, DupFlags::empty())
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
    unsafe {
        ret_discarded_fd(syscall_readonly!(__NR_dup2, fd, new.as_fd()))
    }
}

#[allow(clippy::needless_pass_by_ref_mut)]
#[inline]
pub(crate) fn dup3(fd: BorrowedFd<'_>, new: &mut OwnedFd, flags: DupFlags) -> io::Result<()> {
    unsafe { ret_discarded_fd(syscall_readonly!(__NR_dup3, fd, new.as_fd(), flags)) }
}

#[inline]
pub(crate) fn fcntl_getfd(fd: BorrowedFd<'_>) -> io::Result<FdFlags> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_c_uint(syscall_readonly!(__NR_fcntl64, fd, c_uint(F_GETFD)))
            .map(FdFlags::from_bits_retain)
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_c_uint(syscall_readonly!(__NR_fcntl, fd, c_uint(F_GETFD)))
            .map(FdFlags::from_bits_retain)
    }
}

#[inline]
pub(crate) fn fcntl_setfd(fd: BorrowedFd<'_>, flags: FdFlags) -> io::Result<()> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall_readonly!(__NR_fcntl64, fd, c_uint(F_SETFD), flags))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall_readonly!(__NR_fcntl, fd, c_uint(F_SETFD), flags))
    }
}

#[inline]
pub(crate) fn fcntl_dupfd_cloexec(fd: BorrowedFd<'_>, min: RawFd) -> io::Result<OwnedFd> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_owned_fd(syscall_readonly!(
            __NR_fcntl64,
            fd,
            c_uint(F_DUPFD_CLOEXEC),
            raw_fd(min)
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_owned_fd(syscall_readonly!(
            __NR_fcntl,
            fd,
            c_uint(F_DUPFD_CLOEXEC),
            raw_fd(min)
        ))
    }
}
