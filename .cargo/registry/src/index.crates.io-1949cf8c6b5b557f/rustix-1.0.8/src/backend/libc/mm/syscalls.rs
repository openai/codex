//! libc syscalls supporting `rustix::mm`.

#[cfg(not(target_os = "redox"))]
use super::types::Advice;
#[cfg(any(linux_kernel, freebsdlike, netbsdlike))]
use super::types::MlockAllFlags;
#[cfg(any(target_os = "emscripten", target_os = "linux"))]
use super::types::MremapFlags;
use super::types::{MapFlags, MprotectFlags, MsyncFlags, ProtFlags};
#[cfg(linux_kernel)]
use super::types::{MlockFlags, UserfaultfdFlags};
use crate::backend::c;
#[cfg(linux_kernel)]
use crate::backend::conv::ret_owned_fd;
use crate::backend::conv::{borrowed_fd, no_fd, ret};
use crate::fd::BorrowedFd;
#[cfg(linux_kernel)]
use crate::fd::OwnedFd;
use crate::io;

#[cfg(not(target_os = "redox"))]
pub(crate) fn madvise(addr: *mut c::c_void, len: usize, advice: Advice) -> io::Result<()> {
    // On Linux platforms, `MADV_DONTNEED` has the same value as
    // `POSIX_MADV_DONTNEED` but different behavior. We remap it to a different
    // value, and check for it here.
    #[cfg(target_os = "linux")]
    if let Advice::LinuxDontNeed = advice {
        return unsafe { ret(c::madvise(addr, len, c::MADV_DONTNEED)) };
    }

    #[cfg(not(target_os = "android"))]
    {
        let err = unsafe { c::posix_madvise(addr, len, advice as c::c_int) };

        // `posix_madvise` returns its error status rather than using `errno`.
        if err == 0 {
            Ok(())
        } else {
            Err(io::Errno(err))
        }
    }

    #[cfg(target_os = "android")]
    {
        if let Advice::DontNeed = advice {
            // Do nothing. Linux's `MADV_DONTNEED` isn't the same as
            // `POSIX_MADV_DONTNEED`, so just discard `MADV_DONTNEED`.
            Ok(())
        } else {
            unsafe { ret(c::madvise(addr, len, advice as c::c_int)) }
        }
    }
}

pub(crate) unsafe fn msync(addr: *mut c::c_void, len: usize, flags: MsyncFlags) -> io::Result<()> {
    let err = c::msync(addr, len, bitflags_bits!(flags));

    // `msync` returns its error status rather than using `errno`.
    if err == 0 {
        Ok(())
    } else {
        Err(io::Errno(err))
    }
}

/// # Safety
///
/// `mmap` is primarily unsafe due to the `addr` parameter, as anything working
/// with memory pointed to by raw pointers is unsafe.
pub(crate) unsafe fn mmap(
    ptr: *mut c::c_void,
    len: usize,
    prot: ProtFlags,
    flags: MapFlags,
    fd: BorrowedFd<'_>,
    offset: u64,
) -> io::Result<*mut c::c_void> {
    let res = c::mmap(
        ptr,
        len,
        bitflags_bits!(prot),
        bitflags_bits!(flags),
        borrowed_fd(fd),
        offset as i64,
    );
    if res == c::MAP_FAILED {
        Err(io::Errno::last_os_error())
    } else {
        Ok(res)
    }
}

/// # Safety
///
/// `mmap` is primarily unsafe due to the `addr` parameter, as anything working
/// with memory pointed to by raw pointers is unsafe.
pub(crate) unsafe fn mmap_anonymous(
    ptr: *mut c::c_void,
    len: usize,
    prot: ProtFlags,
    flags: MapFlags,
) -> io::Result<*mut c::c_void> {
    let res = c::mmap(
        ptr,
        len,
        bitflags_bits!(prot),
        bitflags_bits!(flags | MapFlags::from_bits_retain(bitcast!(c::MAP_ANONYMOUS))),
        no_fd(),
        0,
    );
    if res == c::MAP_FAILED {
        Err(io::Errno::last_os_error())
    } else {
        Ok(res)
    }
}

pub(crate) unsafe fn mprotect(
    ptr: *mut c::c_void,
    len: usize,
    flags: MprotectFlags,
) -> io::Result<()> {
    ret(c::mprotect(ptr, len, bitflags_bits!(flags)))
}

pub(crate) unsafe fn munmap(ptr: *mut c::c_void, len: usize) -> io::Result<()> {
    ret(c::munmap(ptr, len))
}

/// # Safety
///
/// `mremap` is primarily unsafe due to the `old_address` parameter, as
/// anything working with memory pointed to by raw pointers is unsafe.
#[cfg(any(target_os = "emscripten", target_os = "linux"))]
pub(crate) unsafe fn mremap(
    old_address: *mut c::c_void,
    old_size: usize,
    new_size: usize,
    flags: MremapFlags,
) -> io::Result<*mut c::c_void> {
    let res = c::mremap(old_address, old_size, new_size, bitflags_bits!(flags));
    if res == c::MAP_FAILED {
        Err(io::Errno::last_os_error())
    } else {
        Ok(res)
    }
}

/// # Safety
///
/// `mremap_fixed` is primarily unsafe due to the `old_address` and
/// `new_address` parameters, as anything working with memory pointed to by raw
/// pointers is unsafe.
#[cfg(any(target_os = "emscripten", target_os = "linux"))]
pub(crate) unsafe fn mremap_fixed(
    old_address: *mut c::c_void,
    old_size: usize,
    new_size: usize,
    flags: MremapFlags,
    new_address: *mut c::c_void,
) -> io::Result<*mut c::c_void> {
    let res = c::mremap(
        old_address,
        old_size,
        new_size,
        bitflags_bits!(flags | MremapFlags::from_bits_retain(bitcast!(c::MAP_FIXED))),
        new_address,
    );
    if res == c::MAP_FAILED {
        Err(io::Errno::last_os_error())
    } else {
        Ok(res)
    }
}

/// # Safety
///
/// `mlock` operates on raw pointers and may round out to the nearest page
/// boundaries.
#[inline]
pub(crate) unsafe fn mlock(addr: *mut c::c_void, length: usize) -> io::Result<()> {
    ret(c::mlock(addr, length))
}

/// # Safety
///
/// `mlock_with` operates on raw pointers and may round out to the nearest page
/// boundaries.
#[cfg(linux_kernel)]
#[inline]
pub(crate) unsafe fn mlock_with(
    addr: *mut c::c_void,
    length: usize,
    flags: MlockFlags,
) -> io::Result<()> {
    weak_or_syscall! {
        fn mlock2(
            addr: *const c::c_void,
            len: c::size_t,
            flags: c::c_int
        ) via SYS_mlock2 -> c::c_int
    }

    ret(mlock2(addr, length, bitflags_bits!(flags)))
}

/// # Safety
///
/// `munlock` operates on raw pointers and may round out to the nearest page
/// boundaries.
#[inline]
pub(crate) unsafe fn munlock(addr: *mut c::c_void, length: usize) -> io::Result<()> {
    ret(c::munlock(addr, length))
}

#[cfg(linux_kernel)]
pub(crate) unsafe fn userfaultfd(flags: UserfaultfdFlags) -> io::Result<OwnedFd> {
    syscall! {
        fn userfaultfd(
            flags: c::c_int
        ) via SYS_userfaultfd -> c::c_int
    }
    ret_owned_fd(userfaultfd(bitflags_bits!(flags)))
}

/// Locks all pages mapped into the address space of the calling process.
///
/// This includes the pages of the code, data, and stack segment, as well as
/// shared libraries, user space kernel data, shared memory, and memory-mapped
/// files. All mapped pages are guaranteed to be resident in RAM when the call
/// returns successfully; the pages are guaranteed to stay in RAM until later
/// unlocked.
#[inline]
#[cfg(any(linux_kernel, freebsdlike, netbsdlike))]
pub(crate) fn mlockall(flags: MlockAllFlags) -> io::Result<()> {
    unsafe { ret(c::mlockall(bitflags_bits!(flags))) }
}

/// Unlocks all pages mapped into the address space of the calling process.
#[inline]
#[cfg(any(linux_kernel, freebsdlike, netbsdlike))]
pub(crate) fn munlockall() -> io::Result<()> {
    unsafe { ret(c::munlockall()) }
}
