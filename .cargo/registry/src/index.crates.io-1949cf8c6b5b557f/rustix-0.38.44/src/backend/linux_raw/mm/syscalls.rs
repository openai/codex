//! linux_raw syscalls supporting `rustix::io`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code)]
#![allow(clippy::undocumented_unsafe_blocks)]

use super::types::{
    Advice, MapFlags, MlockAllFlags, MlockFlags, MprotectFlags, MremapFlags, MsyncFlags, ProtFlags,
    UserfaultfdFlags,
};
use crate::backend::c;
#[cfg(target_pointer_width = "64")]
use crate::backend::conv::loff_t_from_u64;
use crate::backend::conv::{c_uint, no_fd, pass_usize, ret, ret_owned_fd, ret_void_star};
use crate::fd::{BorrowedFd, OwnedFd};
use crate::io;
use linux_raw_sys::general::{MAP_ANONYMOUS, MREMAP_FIXED};

#[inline]
pub(crate) fn madvise(addr: *mut c::c_void, len: usize, advice: Advice) -> io::Result<()> {
    unsafe {
        ret(syscall!(
            __NR_madvise,
            addr,
            pass_usize(len),
            c_uint(advice as c::c_uint)
        ))
    }
}

#[inline]
pub(crate) unsafe fn msync(addr: *mut c::c_void, len: usize, flags: MsyncFlags) -> io::Result<()> {
    ret(syscall!(__NR_msync, addr, pass_usize(len), flags))
}

/// # Safety
///
/// `mmap` is primarily unsafe due to the `addr` parameter, as anything working
/// with memory pointed to by raw pointers is unsafe.
#[inline]
pub(crate) unsafe fn mmap(
    addr: *mut c::c_void,
    length: usize,
    prot: ProtFlags,
    flags: MapFlags,
    fd: BorrowedFd<'_>,
    offset: u64,
) -> io::Result<*mut c::c_void> {
    #[cfg(target_pointer_width = "32")]
    {
        ret_void_star(syscall!(
            __NR_mmap2,
            addr,
            pass_usize(length),
            prot,
            flags,
            fd,
            (offset / 4096)
                .try_into()
                .map(pass_usize)
                .map_err(|_| io::Errno::INVAL)?
        ))
    }
    #[cfg(target_pointer_width = "64")]
    {
        ret_void_star(syscall!(
            __NR_mmap,
            addr,
            pass_usize(length),
            prot,
            flags,
            fd,
            loff_t_from_u64(offset)
        ))
    }
}

/// # Safety
///
/// `mmap` is primarily unsafe due to the `addr` parameter, as anything working
/// with memory pointed to by raw pointers is unsafe.
#[inline]
pub(crate) unsafe fn mmap_anonymous(
    addr: *mut c::c_void,
    length: usize,
    prot: ProtFlags,
    flags: MapFlags,
) -> io::Result<*mut c::c_void> {
    #[cfg(target_pointer_width = "32")]
    {
        ret_void_star(syscall!(
            __NR_mmap2,
            addr,
            pass_usize(length),
            prot,
            c_uint(flags.bits() | MAP_ANONYMOUS),
            no_fd(),
            pass_usize(0)
        ))
    }
    #[cfg(target_pointer_width = "64")]
    {
        ret_void_star(syscall!(
            __NR_mmap,
            addr,
            pass_usize(length),
            prot,
            c_uint(flags.bits() | MAP_ANONYMOUS),
            no_fd(),
            loff_t_from_u64(0)
        ))
    }
}

#[inline]
pub(crate) unsafe fn mprotect(
    ptr: *mut c::c_void,
    len: usize,
    flags: MprotectFlags,
) -> io::Result<()> {
    ret(syscall!(__NR_mprotect, ptr, pass_usize(len), flags))
}

/// # Safety
///
/// `munmap` is primarily unsafe due to the `addr` parameter, as anything
/// working with memory pointed to by raw pointers is unsafe.
#[inline]
pub(crate) unsafe fn munmap(addr: *mut c::c_void, length: usize) -> io::Result<()> {
    ret(syscall!(__NR_munmap, addr, pass_usize(length)))
}

/// # Safety
///
/// `mremap` is primarily unsafe due to the `old_address` parameter, as
/// anything working with memory pointed to by raw pointers is unsafe.
#[inline]
pub(crate) unsafe fn mremap(
    old_address: *mut c::c_void,
    old_size: usize,
    new_size: usize,
    flags: MremapFlags,
) -> io::Result<*mut c::c_void> {
    ret_void_star(syscall!(
        __NR_mremap,
        old_address,
        pass_usize(old_size),
        pass_usize(new_size),
        flags
    ))
}

/// # Safety
///
/// `mremap_fixed` is primarily unsafe due to the `old_address` and
/// `new_address` parameters, as anything working with memory pointed to by raw
/// pointers is unsafe.
#[inline]
pub(crate) unsafe fn mremap_fixed(
    old_address: *mut c::c_void,
    old_size: usize,
    new_size: usize,
    flags: MremapFlags,
    new_address: *mut c::c_void,
) -> io::Result<*mut c::c_void> {
    ret_void_star(syscall!(
        __NR_mremap,
        old_address,
        pass_usize(old_size),
        pass_usize(new_size),
        c_uint(flags.bits() | MREMAP_FIXED),
        new_address
    ))
}

/// # Safety
///
/// `mlock` operates on raw pointers and may round out to the nearest page
/// boundaries.
#[inline]
pub(crate) unsafe fn mlock(addr: *mut c::c_void, length: usize) -> io::Result<()> {
    ret(syscall!(__NR_mlock, addr, pass_usize(length)))
}

/// # Safety
///
/// `mlock_with` operates on raw pointers and may round out to the nearest page
/// boundaries.
#[inline]
pub(crate) unsafe fn mlock_with(
    addr: *mut c::c_void,
    length: usize,
    flags: MlockFlags,
) -> io::Result<()> {
    ret(syscall!(__NR_mlock2, addr, pass_usize(length), flags))
}

/// # Safety
///
/// `munlock` operates on raw pointers and may round out to the nearest page
/// boundaries.
#[inline]
pub(crate) unsafe fn munlock(addr: *mut c::c_void, length: usize) -> io::Result<()> {
    ret(syscall!(__NR_munlock, addr, pass_usize(length)))
}

#[inline]
pub(crate) unsafe fn userfaultfd(flags: UserfaultfdFlags) -> io::Result<OwnedFd> {
    ret_owned_fd(syscall_readonly!(__NR_userfaultfd, flags))
}

/// Locks all pages mapped into the address space of the calling process.
///
/// This includes the pages of the code, data, and stack segment, as well as
/// shared libraries, user space kernel data, shared memory, and memory-mapped
/// files. All mapped pages are guaranteed to be resident in RAM when the call
/// returns successfully; the pages are guaranteed to stay in RAM until later
/// unlocked.
#[inline]
pub(crate) fn mlockall(flags: MlockAllFlags) -> io::Result<()> {
    // When `mlockall` is used with `MCL_ONFAULT | MCL_FUTURE`, the ordering
    // of `mlockall` with respect to arbitrary loads may be significant,
    // because if a load happens and evokes a fault before the `mlockall`,
    // the memory doesn't get locked, but if the load and therefore
    // the fault happens after, then the memory does get locked.
    // So to be conservative in this regard, we use `syscall` instead
    // of `syscall_readonly`
    unsafe { ret(syscall!(__NR_mlockall, flags)) }
}

/// Unlocks all pages mapped into the address space of the calling process.
#[inline]
pub(crate) fn munlockall() -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_munlockall)) }
}
