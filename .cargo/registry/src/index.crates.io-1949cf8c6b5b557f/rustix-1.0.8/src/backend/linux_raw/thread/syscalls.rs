//! linux_raw syscalls supporting `rustix::thread`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use super::types::RawCpuSet;
use crate::backend::c;
use crate::backend::conv::{
    by_mut, by_ref, c_int, c_uint, opt_ref, ret, ret_c_int, ret_c_int_infallible, ret_c_uint,
    ret_usize, size_of, slice, slice_just_addr, slice_just_addr_mut, zero,
};
use crate::fd::BorrowedFd;
use crate::io;
use crate::pid::Pid;
use crate::thread::{
    futex, ClockId, Cpuid, MembarrierCommand, MembarrierQuery, NanosleepRelativeResult, Timespec,
};
use crate::utils::as_mut_ptr;
use core::mem::MaybeUninit;
use core::sync::atomic::AtomicU32;
#[cfg(target_pointer_width = "32")]
use linux_raw_sys::general::timespec as __kernel_old_timespec;
use linux_raw_sys::general::{membarrier_cmd, membarrier_cmd_flag, TIMER_ABSTIME};

#[inline]
pub(crate) fn clock_nanosleep_relative(id: ClockId, req: &Timespec) -> NanosleepRelativeResult {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut rem = MaybeUninit::<Timespec>::uninit();
        match ret(syscall!(
            __NR_clock_nanosleep_time64,
            id,
            c_int(0),
            by_ref(req),
            &mut rem
        ))
        .or_else(|err| {
            // See the comments in `clock_gettime_via_syscall` about emulation.
            if err == io::Errno::NOSYS {
                clock_nanosleep_relative_old(id, req, &mut rem)
            } else {
                Err(err)
            }
        }) {
            Ok(()) => NanosleepRelativeResult::Ok,
            Err(io::Errno::INTR) => NanosleepRelativeResult::Interrupted(rem.assume_init()),
            Err(err) => NanosleepRelativeResult::Err(err),
        }
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut rem = MaybeUninit::<Timespec>::uninit();
        match ret(syscall!(
            __NR_clock_nanosleep,
            id,
            c_int(0),
            by_ref(req),
            &mut rem
        )) {
            Ok(()) => NanosleepRelativeResult::Ok,
            Err(io::Errno::INTR) => NanosleepRelativeResult::Interrupted(rem.assume_init()),
            Err(err) => NanosleepRelativeResult::Err(err),
        }
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn clock_nanosleep_relative_old(
    id: ClockId,
    req: &Timespec,
    rem: &mut MaybeUninit<Timespec>,
) -> io::Result<()> {
    let old_req = __kernel_old_timespec {
        tv_sec: req.tv_sec.try_into().map_err(|_| io::Errno::INVAL)?,
        tv_nsec: req.tv_nsec.try_into().map_err(|_| io::Errno::INVAL)?,
    };
    let mut old_rem = MaybeUninit::<__kernel_old_timespec>::uninit();
    ret(syscall!(
        __NR_clock_nanosleep,
        id,
        c_int(0),
        by_ref(&old_req),
        &mut old_rem
    ))?;
    let old_rem = old_rem.assume_init();
    rem.write(Timespec {
        tv_sec: old_rem.tv_sec.into(),
        tv_nsec: old_rem.tv_nsec.into(),
    });
    Ok(())
}

#[inline]
pub(crate) fn clock_nanosleep_absolute(id: ClockId, req: &Timespec) -> io::Result<()> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall_readonly!(
            __NR_clock_nanosleep_time64,
            id,
            c_uint(TIMER_ABSTIME),
            by_ref(req),
            zero()
        ))
        .or_else(|err| {
            // See the comments in `clock_gettime_via_syscall` about emulation.
            if err == io::Errno::NOSYS {
                clock_nanosleep_absolute_old(id, req)
            } else {
                Err(err)
            }
        })
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall_readonly!(
            __NR_clock_nanosleep,
            id,
            c_uint(TIMER_ABSTIME),
            by_ref(req),
            zero()
        ))
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn clock_nanosleep_absolute_old(id: ClockId, req: &Timespec) -> io::Result<()> {
    let old_req = __kernel_old_timespec {
        tv_sec: req.tv_sec.try_into().map_err(|_| io::Errno::INVAL)?,
        tv_nsec: req.tv_nsec.try_into().map_err(|_| io::Errno::INVAL)?,
    };
    ret(syscall_readonly!(
        __NR_clock_nanosleep,
        id,
        c_int(0),
        by_ref(&old_req),
        zero()
    ))
}

#[inline]
pub(crate) fn nanosleep(req: &Timespec) -> NanosleepRelativeResult {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut rem = MaybeUninit::<Timespec>::uninit();
        match ret(syscall!(
            __NR_clock_nanosleep_time64,
            ClockId::Realtime,
            c_int(0),
            by_ref(req),
            &mut rem
        ))
        .or_else(|err| {
            // See the comments in `clock_gettime_via_syscall` about emulation.
            if err == io::Errno::NOSYS {
                nanosleep_old(req, &mut rem)
            } else {
                Err(err)
            }
        }) {
            Ok(()) => NanosleepRelativeResult::Ok,
            Err(io::Errno::INTR) => NanosleepRelativeResult::Interrupted(rem.assume_init()),
            Err(err) => NanosleepRelativeResult::Err(err),
        }
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut rem = MaybeUninit::<Timespec>::uninit();
        match ret(syscall!(__NR_nanosleep, by_ref(req), &mut rem)) {
            Ok(()) => NanosleepRelativeResult::Ok,
            Err(io::Errno::INTR) => NanosleepRelativeResult::Interrupted(rem.assume_init()),
            Err(err) => NanosleepRelativeResult::Err(err),
        }
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn nanosleep_old(req: &Timespec, rem: &mut MaybeUninit<Timespec>) -> io::Result<()> {
    let old_req = __kernel_old_timespec {
        tv_sec: req.tv_sec.try_into().map_err(|_| io::Errno::INVAL)?,
        tv_nsec: req.tv_nsec.try_into().map_err(|_| io::Errno::INVAL)?,
    };
    let mut old_rem = MaybeUninit::<__kernel_old_timespec>::uninit();
    ret(syscall!(__NR_nanosleep, by_ref(&old_req), &mut old_rem))?;
    let old_rem = old_rem.assume_init();
    rem.write(Timespec {
        tv_sec: old_rem.tv_sec.into(),
        tv_nsec: old_rem.tv_nsec.into(),
    });
    Ok(())
}

#[inline]
#[must_use]
pub(crate) fn gettid() -> Pid {
    unsafe {
        let tid = ret_c_int_infallible(syscall_readonly!(__NR_gettid));
        Pid::from_raw_unchecked(tid)
    }
}

/// # Safety
///
/// The raw pointers must point to valid aligned memory.
#[inline]
pub(crate) unsafe fn futex_val2(
    uaddr: *const AtomicU32,
    op: super::futex::Operation,
    flags: futex::Flags,
    val: u32,
    val2: u32,
    uaddr2: *const AtomicU32,
    val3: u32,
) -> io::Result<usize> {
    // Pass `val2` in the least-significant bytes of the `timeout` argument.
    // [“the kernel casts the timeout value first to unsigned long, then to
    // uint32_t”], so we perform that exact conversion in reverse to create
    // the pointer.
    //
    // [“the kernel casts the timeout value first to unsigned long, then to uint32_t”]: https://man7.org/linux/man-pages/man2/futex.2.html
    let timeout = val2 as usize as *const Timespec;

    #[cfg(target_pointer_width = "32")]
    {
        // Linux 5.1 added `futex_time64`; if we have that, use it. We don't
        // need it here, because `timeout` is just passing `val2` and not a
        // real timeout, but it's nice to use `futex_time64` for consistency
        // with the other futex calls that do.
        #[cfg(feature = "linux_5_1")]
        {
            ret_usize(syscall!(
                __NR_futex_time64,
                uaddr,
                (op, flags),
                c_uint(val),
                timeout,
                uaddr2,
                c_uint(val3)
            ))
        }

        // If we don't have Linux 5.1, use plain `futex`.
        #[cfg(not(feature = "linux_5_1"))]
        {
            ret_usize(syscall!(
                __NR_futex,
                uaddr,
                (op, flags),
                c_uint(val),
                timeout,
                uaddr2,
                c_uint(val3)
            ))
        }
    }
    #[cfg(target_pointer_width = "64")]
    ret_usize(syscall!(
        __NR_futex,
        uaddr,
        (op, flags),
        c_uint(val),
        timeout,
        uaddr2,
        c_uint(val3)
    ))
}

/// # Safety
///
/// The raw pointers must point to valid aligned memory.
#[inline]
pub(crate) unsafe fn futex_timeout(
    uaddr: *const AtomicU32,
    op: super::futex::Operation,
    flags: futex::Flags,
    val: u32,
    timeout: Option<&Timespec>,
    uaddr2: *const AtomicU32,
    val3: u32,
) -> io::Result<usize> {
    #[cfg(target_pointer_width = "32")]
    {
        // If we don't have Linux 5.1, and the timeout fits in a
        // `__kernel_old_timespec`, use plain `futex`.
        //
        // We do this unconditionally, rather than trying `futex_time64` and
        // falling back on `Errno::NOSYS`, because seccomp configurations will
        // sometimes abort the process on syscalls they don't recognize.
        #[cfg(not(feature = "linux_5_1"))]
        {
            // If we don't have a timeout, or if we can convert the timeout to
            // a `__kernel_old_timespec`, the use `__NR_futex`.
            fn convert(timeout: &Timespec) -> Option<__kernel_old_timespec> {
                Some(__kernel_old_timespec {
                    tv_sec: timeout.tv_sec.try_into().ok()?,
                    tv_nsec: timeout.tv_nsec.try_into().ok()?,
                })
            }
            let old_timeout = if let Some(timeout) = timeout {
                match convert(timeout) {
                    // Could not convert timeout.
                    None => None,
                    // Could convert timeout. Ok!
                    Some(old_timeout) => Some(Some(old_timeout)),
                }
            } else {
                // No timeout. Ok!
                Some(None)
            };
            if let Some(old_timeout) = old_timeout {
                return ret_usize(syscall!(
                    __NR_futex,
                    uaddr,
                    (op, flags),
                    c_uint(val),
                    opt_ref(old_timeout.as_ref()),
                    uaddr2,
                    c_uint(val3)
                ));
            }
        }

        // We either have Linux 5.1 or the timeout didn't fit in
        // `__kernel_old_timespec` so `__NR_futex_time64` will either succeed
        // or fail due to our having no other options.
        ret_usize(syscall!(
            __NR_futex_time64,
            uaddr,
            (op, flags),
            c_uint(val),
            opt_ref(timeout),
            uaddr2,
            c_uint(val3)
        ))
    }
    #[cfg(target_pointer_width = "64")]
    ret_usize(syscall!(
        __NR_futex,
        uaddr,
        (op, flags),
        c_uint(val),
        opt_ref(timeout),
        uaddr2,
        c_uint(val3)
    ))
}

#[inline]
pub(crate) fn futex_waitv(
    waiters: &[futex::Wait],
    flags: futex::WaitvFlags,
    timeout: Option<&Timespec>,
    clockid: ClockId,
) -> io::Result<usize> {
    let (waiters_addr, waiters_len) = slice(waiters);
    unsafe {
        ret_usize(syscall!(
            __NR_futex_waitv,
            waiters_addr,
            waiters_len,
            c_uint(flags.bits()),
            opt_ref(timeout),
            clockid
        ))
    }
}

#[inline]
pub(crate) fn setns(fd: BorrowedFd<'_>, nstype: c::c_int) -> io::Result<c::c_int> {
    unsafe { ret_c_int(syscall_readonly!(__NR_setns, fd, c_int(nstype))) }
}

#[inline]
pub(crate) unsafe fn unshare(flags: crate::thread::UnshareFlags) -> io::Result<()> {
    ret(syscall_readonly!(__NR_unshare, flags))
}

#[inline]
pub(crate) fn capget(
    header: &mut linux_raw_sys::general::__user_cap_header_struct,
    data: &mut [MaybeUninit<linux_raw_sys::general::__user_cap_data_struct>],
) -> io::Result<()> {
    unsafe {
        ret(syscall!(
            __NR_capget,
            by_mut(header),
            slice_just_addr_mut(data)
        ))
    }
}

#[inline]
pub(crate) fn capset(
    header: &mut linux_raw_sys::general::__user_cap_header_struct,
    data: &[linux_raw_sys::general::__user_cap_data_struct],
) -> io::Result<()> {
    unsafe { ret(syscall!(__NR_capset, by_mut(header), slice_just_addr(data))) }
}

#[inline]
pub(crate) fn setuid_thread(uid: crate::ugid::Uid) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_setuid, uid)) }
}

#[inline]
pub(crate) fn setresuid_thread(
    ruid: crate::ugid::Uid,
    euid: crate::ugid::Uid,
    suid: crate::ugid::Uid,
) -> io::Result<()> {
    #[cfg(any(target_arch = "x86", target_arch = "arm", target_arch = "sparc"))]
    unsafe {
        ret(syscall_readonly!(__NR_setresuid32, ruid, euid, suid))
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "arm", target_arch = "sparc")))]
    unsafe {
        ret(syscall_readonly!(__NR_setresuid, ruid, euid, suid))
    }
}

#[inline]
pub(crate) fn setgid_thread(gid: crate::ugid::Gid) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_setgid, gid)) }
}

#[inline]
pub(crate) fn setresgid_thread(
    rgid: crate::ugid::Gid,
    egid: crate::ugid::Gid,
    sgid: crate::ugid::Gid,
) -> io::Result<()> {
    #[cfg(any(target_arch = "x86", target_arch = "arm", target_arch = "sparc"))]
    unsafe {
        ret(syscall_readonly!(__NR_setresgid32, rgid, egid, sgid))
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "arm", target_arch = "sparc")))]
    unsafe {
        ret(syscall_readonly!(__NR_setresgid, rgid, egid, sgid))
    }
}

#[inline]
pub(crate) fn setgroups_thread(gids: &[crate::ugid::Gid]) -> io::Result<()> {
    let (addr, len) = slice(gids);
    unsafe { ret(syscall_readonly!(__NR_setgroups, len, addr)) }
}

// `sched_getcpu` has special optimizations via the vDSO on some architectures.
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "riscv64",
    target_arch = "powerpc",
    target_arch = "powerpc64",
    target_arch = "s390x"
))]
pub(crate) use crate::backend::vdso_wrappers::sched_getcpu;

// `sched_getcpu` on platforms without a vDSO entry for it.
#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "riscv64",
    target_arch = "powerpc",
    target_arch = "powerpc64",
    target_arch = "s390x"
)))]
#[inline]
pub(crate) fn sched_getcpu() -> usize {
    let mut cpu = MaybeUninit::<u32>::uninit();
    unsafe {
        let r = ret(syscall!(__NR_getcpu, &mut cpu, zero(), zero()));
        debug_assert!(r.is_ok());
        cpu.assume_init() as usize
    }
}

#[inline]
pub(crate) fn sched_getaffinity(pid: Option<Pid>, cpuset: &mut RawCpuSet) -> io::Result<()> {
    unsafe {
        // The raw Linux syscall returns the size (in bytes) of the `cpumask_t`
        // data type that is used internally by the kernel to represent the CPU
        // set bit mask.
        let size = ret_usize(syscall!(
            __NR_sched_getaffinity,
            c_int(Pid::as_raw(pid)),
            size_of::<RawCpuSet, _>(),
            by_mut(&mut cpuset.bits)
        ))?;
        let bytes = as_mut_ptr(cpuset).cast::<u8>();
        let rest = bytes.wrapping_add(size);
        // Zero every byte in the cpuset not set by the kernel.
        rest.write_bytes(0, core::mem::size_of::<RawCpuSet>() - size);
        Ok(())
    }
}

#[inline]
pub(crate) fn sched_setaffinity(pid: Option<Pid>, cpuset: &RawCpuSet) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_sched_setaffinity,
            c_int(Pid::as_raw(pid)),
            size_of::<RawCpuSet, _>(),
            slice_just_addr(&cpuset.bits)
        ))
    }
}

#[inline]
pub(crate) fn sched_yield() {
    unsafe {
        // See the documentation for [`crate::thread::sched_yield`] for why
        // errors are ignored.
        syscall_readonly!(__NR_sched_yield).decode_void();
    }
}

#[inline]
pub(crate) fn membarrier_query() -> MembarrierQuery {
    unsafe {
        match ret_c_uint(syscall!(
            __NR_membarrier,
            c_int(membarrier_cmd::MEMBARRIER_CMD_QUERY as _),
            c_uint(0)
        )) {
            Ok(query) => MembarrierQuery::from_bits_retain(query),
            Err(_) => MembarrierQuery::empty(),
        }
    }
}

#[inline]
pub(crate) fn membarrier(cmd: MembarrierCommand) -> io::Result<()> {
    unsafe { ret(syscall!(__NR_membarrier, cmd, c_uint(0))) }
}

#[inline]
pub(crate) fn membarrier_cpu(cmd: MembarrierCommand, cpu: Cpuid) -> io::Result<()> {
    unsafe {
        ret(syscall!(
            __NR_membarrier,
            cmd,
            c_uint(membarrier_cmd_flag::MEMBARRIER_CMD_FLAG_CPU as _),
            cpu
        ))
    }
}
