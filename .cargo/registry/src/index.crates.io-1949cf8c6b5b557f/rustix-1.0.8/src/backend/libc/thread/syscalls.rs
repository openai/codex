//! libc syscalls supporting `rustix::thread`.

#[cfg(any(freebsdlike, linux_kernel, target_os = "fuchsia"))]
use super::types::RawCpuSet;
use crate::backend::c;
use crate::backend::conv::ret;
use crate::io;
#[cfg(any(freebsdlike, linux_kernel, target_os = "fuchsia"))]
use crate::pid::Pid;
#[cfg(not(any(
    apple,
    freebsdlike,
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
use crate::thread::ClockId;
#[cfg(linux_kernel)]
use crate::thread::{Cpuid, MembarrierCommand, MembarrierQuery};
#[cfg(not(target_os = "redox"))]
use crate::thread::{NanosleepRelativeResult, Timespec};
#[cfg(all(target_env = "gnu", fix_y2038))]
use crate::timespec::LibcTimespec;
#[cfg(not(fix_y2038))]
use crate::timespec::{as_libc_timespec_mut_ptr, as_libc_timespec_ptr};
#[cfg(linux_kernel)]
use crate::utils::option_as_ptr;
use core::mem::MaybeUninit;
#[cfg(linux_kernel)]
use core::sync::atomic::AtomicU32;
#[cfg(linux_kernel)]
use {
    crate::backend::conv::{borrowed_fd, ret_c_int, ret_u32, ret_usize},
    crate::fd::BorrowedFd,
    crate::thread::futex,
    crate::utils::as_mut_ptr,
};

#[cfg(all(target_env = "gnu", fix_y2038))]
weak!(fn __clock_nanosleep_time64(c::clockid_t, c::c_int, *const LibcTimespec, *mut LibcTimespec) -> c::c_int);
#[cfg(all(target_env = "gnu", fix_y2038))]
weak!(fn __nanosleep64(*const LibcTimespec, *mut LibcTimespec) -> c::c_int);

#[cfg(not(any(
    apple,
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "freebsd", // FreeBSD 12 has clock_nanosleep, but libc targets FreeBSD 11.
    target_os = "haiku",
    target_os = "horizon",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
#[inline]
pub(crate) fn clock_nanosleep_relative(id: ClockId, request: &Timespec) -> NanosleepRelativeResult {
    // Old 32-bit version: libc has `clock_nanosleep` but it is not y2038 safe
    // by default. But there may be a `__clock_nanosleep_time64` we can use.
    #[cfg(fix_y2038)]
    {
        #[cfg(target_env = "gnu")]
        if let Some(libc_clock_nanosleep) = __clock_nanosleep_time64.get() {
            let flags = 0;
            let mut remain = MaybeUninit::<LibcTimespec>::uninit();

            unsafe {
                return match libc_clock_nanosleep(
                    id as c::clockid_t,
                    flags,
                    &request.clone().into(),
                    remain.as_mut_ptr(),
                ) {
                    0 => NanosleepRelativeResult::Ok,
                    err if err == io::Errno::INTR.0 => {
                        NanosleepRelativeResult::Interrupted(remain.assume_init().into())
                    }
                    err => NanosleepRelativeResult::Err(io::Errno(err)),
                };
            }
        }

        clock_nanosleep_relative_old(id, request)
    }

    // Main version: libc is y2038 safe and has `clock_nanosleep`.
    #[cfg(not(fix_y2038))]
    unsafe {
        let flags = 0;
        let mut remain = MaybeUninit::<Timespec>::uninit();

        match c::clock_nanosleep(
            id as c::clockid_t,
            flags,
            as_libc_timespec_ptr(request),
            as_libc_timespec_mut_ptr(&mut remain),
        ) {
            0 => NanosleepRelativeResult::Ok,
            err if err == io::Errno::INTR.0 => {
                NanosleepRelativeResult::Interrupted(remain.assume_init())
            }
            err => NanosleepRelativeResult::Err(io::Errno(err)),
        }
    }
}

#[cfg(all(
    fix_y2038,
    not(any(
        apple,
        target_os = "emscripten",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "vita"
    ))
))]
fn clock_nanosleep_relative_old(
    id: crate::clockid::ClockId,
    request: &Timespec,
) -> NanosleepRelativeResult {
    let tv_sec = match request.tv_sec.try_into() {
        Ok(tv_sec) => tv_sec,
        Err(_) => return NanosleepRelativeResult::Err(io::Errno::OVERFLOW),
    };
    let tv_nsec = match request.tv_nsec.try_into() {
        Ok(tv_nsec) => tv_nsec,
        Err(_) => return NanosleepRelativeResult::Err(io::Errno::INVAL),
    };
    let old_request = c::timespec { tv_sec, tv_nsec };
    let mut old_remain = MaybeUninit::<c::timespec>::uninit();
    let flags = 0;

    unsafe {
        match c::clock_nanosleep(
            id as c::clockid_t,
            flags,
            &old_request,
            old_remain.as_mut_ptr(),
        ) {
            0 => NanosleepRelativeResult::Ok,
            err if err == io::Errno::INTR.0 => {
                let old_remain = old_remain.assume_init();
                let remain = Timespec {
                    tv_sec: old_remain.tv_sec.into(),
                    tv_nsec: old_remain.tv_nsec.into(),
                };
                NanosleepRelativeResult::Interrupted(remain)
            }
            err => NanosleepRelativeResult::Err(io::Errno(err)),
        }
    }
}

#[cfg(not(any(
    apple,
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "freebsd", // FreeBSD 12 has clock_nanosleep, but libc targets FreeBSD 11.
    target_os = "haiku",
    target_os = "horizon",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
#[inline]
pub(crate) fn clock_nanosleep_absolute(id: ClockId, request: &Timespec) -> io::Result<()> {
    // Old 32-bit version: libc has `clock_nanosleep` but it is not y2038 safe
    // by default. But there may be a `__clock_nanosleep_time64` we can use.
    #[cfg(fix_y2038)]
    {
        #[cfg(target_env = "gnu")]
        if let Some(libc_clock_nanosleep) = __clock_nanosleep_time64.get() {
            let flags = c::TIMER_ABSTIME;
            unsafe {
                return match libc_clock_nanosleep(
                    id as c::clockid_t,
                    flags,
                    &request.clone().into(),
                    core::ptr::null_mut(),
                ) {
                    0 => Ok(()),
                    err => Err(io::Errno(err)),
                };
            }
        }

        clock_nanosleep_absolute_old(id, request)
    }

    // Main version: libc is y2038 safe and has `clock_nanosleep`.
    #[cfg(not(fix_y2038))]
    {
        let flags = c::TIMER_ABSTIME;

        match unsafe {
            c::clock_nanosleep(
                id as c::clockid_t,
                flags as _,
                as_libc_timespec_ptr(request),
                core::ptr::null_mut(),
            )
        } {
            0 => Ok(()),
            err => Err(io::Errno(err)),
        }
    }
}

#[cfg(all(
    fix_y2038,
    not(any(
        apple,
        target_os = "emscripten",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "vita"
    ))
))]
fn clock_nanosleep_absolute_old(id: crate::clockid::ClockId, request: &Timespec) -> io::Result<()> {
    let flags = c::TIMER_ABSTIME;

    let old_request = c::timespec {
        tv_sec: request.tv_sec.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        tv_nsec: request.tv_nsec.try_into().map_err(|_| io::Errno::INVAL)?,
    };
    match unsafe {
        c::clock_nanosleep(
            id as c::clockid_t,
            flags,
            &old_request,
            core::ptr::null_mut(),
        )
    } {
        0 => Ok(()),
        err => Err(io::Errno(err)),
    }
}

#[cfg(not(target_os = "redox"))]
#[inline]
pub(crate) fn nanosleep(request: &Timespec) -> NanosleepRelativeResult {
    // Old 32-bit version: libc has `nanosleep` but it is not y2038 safe by
    // default. But there may be a `__nanosleep64` we can use.
    #[cfg(fix_y2038)]
    {
        #[cfg(target_env = "gnu")]
        if let Some(libc_nanosleep) = __nanosleep64.get() {
            let mut remain = MaybeUninit::<LibcTimespec>::uninit();
            unsafe {
                return match ret(libc_nanosleep(&request.clone().into(), remain.as_mut_ptr())) {
                    Ok(()) => NanosleepRelativeResult::Ok,
                    Err(io::Errno::INTR) => {
                        NanosleepRelativeResult::Interrupted(remain.assume_init().into())
                    }
                    Err(err) => NanosleepRelativeResult::Err(err),
                };
            }
        }

        nanosleep_old(request)
    }

    // Main version: libc is y2038 safe and has `nanosleep`.
    #[cfg(not(fix_y2038))]
    unsafe {
        let mut remain = MaybeUninit::<Timespec>::uninit();

        match ret(c::nanosleep(
            as_libc_timespec_ptr(request),
            as_libc_timespec_mut_ptr(&mut remain),
        )) {
            Ok(()) => NanosleepRelativeResult::Ok,
            Err(io::Errno::INTR) => NanosleepRelativeResult::Interrupted(remain.assume_init()),
            Err(err) => NanosleepRelativeResult::Err(err),
        }
    }
}

#[cfg(fix_y2038)]
fn nanosleep_old(request: &Timespec) -> NanosleepRelativeResult {
    let tv_sec = match request.tv_sec.try_into() {
        Ok(tv_sec) => tv_sec,
        Err(_) => return NanosleepRelativeResult::Err(io::Errno::OVERFLOW),
    };
    let tv_nsec = match request.tv_nsec.try_into() {
        Ok(tv_nsec) => tv_nsec,
        Err(_) => return NanosleepRelativeResult::Err(io::Errno::INVAL),
    };
    let old_request = c::timespec { tv_sec, tv_nsec };
    let mut old_remain = MaybeUninit::<c::timespec>::uninit();

    unsafe {
        match ret(c::nanosleep(&old_request, old_remain.as_mut_ptr())) {
            Ok(()) => NanosleepRelativeResult::Ok,
            Err(io::Errno::INTR) => {
                let old_remain = old_remain.assume_init();
                let remain = Timespec {
                    tv_sec: old_remain.tv_sec.into(),
                    tv_nsec: old_remain.tv_nsec.into(),
                };
                NanosleepRelativeResult::Interrupted(remain)
            }
            Err(err) => NanosleepRelativeResult::Err(err),
        }
    }
}

#[cfg(linux_kernel)]
#[inline]
#[must_use]
pub(crate) fn gettid() -> Pid {
    // `gettid` wasn't supported in glibc until 2.30, and musl until 1.2.2,
    // so use `syscall`.
    // <https://sourceware.org/bugzilla/show_bug.cgi?id=6399#c62>
    weak_or_syscall! {
        fn gettid() via SYS_gettid -> c::pid_t
    }

    unsafe {
        let tid = gettid();
        Pid::from_raw_unchecked(tid)
    }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn setns(fd: BorrowedFd<'_>, nstype: c::c_int) -> io::Result<c::c_int> {
    // `setns` wasn't supported in glibc until 2.14, and musl until 0.9.5,
    // so use `syscall`.
    weak_or_syscall! {
        fn setns(fd: c::c_int, nstype: c::c_int) via SYS_setns -> c::c_int
    }

    unsafe { ret_c_int(setns(borrowed_fd(fd), nstype)) }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) unsafe fn unshare(flags: crate::thread::UnshareFlags) -> io::Result<()> {
    ret(c::unshare(flags.bits() as i32))
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn capget(
    header: &mut linux_raw_sys::general::__user_cap_header_struct,
    data: &mut [MaybeUninit<linux_raw_sys::general::__user_cap_data_struct>],
) -> io::Result<()> {
    syscall! {
        fn capget(
            hdrp: *mut linux_raw_sys::general::__user_cap_header_struct,
            data: *mut linux_raw_sys::general::__user_cap_data_struct
        ) via SYS_capget -> c::c_int
    }

    unsafe {
        ret(capget(
            as_mut_ptr(header),
            data.as_mut_ptr()
                .cast::<linux_raw_sys::general::__user_cap_data_struct>(),
        ))
    }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn capset(
    header: &mut linux_raw_sys::general::__user_cap_header_struct,
    data: &[linux_raw_sys::general::__user_cap_data_struct],
) -> io::Result<()> {
    syscall! {
        fn capset(
            hdrp: *mut linux_raw_sys::general::__user_cap_header_struct,
            data: *const linux_raw_sys::general::__user_cap_data_struct
        ) via SYS_capset -> c::c_int
    }

    unsafe { ret(capset(as_mut_ptr(header), data.as_ptr())) }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn setuid_thread(uid: crate::ugid::Uid) -> io::Result<()> {
    syscall! {
        fn setuid(uid: c::uid_t) via SYS_setuid -> c::c_int
    }

    unsafe { ret(setuid(uid.as_raw())) }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn setresuid_thread(
    ruid: crate::ugid::Uid,
    euid: crate::ugid::Uid,
    suid: crate::ugid::Uid,
) -> io::Result<()> {
    #[cfg(any(target_arch = "x86", target_arch = "arm", target_arch = "sparc"))]
    const SYS: c::c_long = c::SYS_setresuid32 as c::c_long;
    #[cfg(not(any(target_arch = "x86", target_arch = "arm", target_arch = "sparc")))]
    const SYS: c::c_long = c::SYS_setresuid as c::c_long;

    syscall! {
        fn setresuid(ruid: c::uid_t, euid: c::uid_t, suid: c::uid_t) via SYS -> c::c_int
    }

    unsafe { ret(setresuid(ruid.as_raw(), euid.as_raw(), suid.as_raw())) }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn setgid_thread(gid: crate::ugid::Gid) -> io::Result<()> {
    syscall! {
        fn setgid(gid: c::gid_t) via SYS_setgid -> c::c_int
    }

    unsafe { ret(setgid(gid.as_raw())) }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn setresgid_thread(
    rgid: crate::ugid::Gid,
    egid: crate::ugid::Gid,
    sgid: crate::ugid::Gid,
) -> io::Result<()> {
    #[cfg(any(target_arch = "x86", target_arch = "arm", target_arch = "sparc"))]
    const SYS: c::c_long = c::SYS_setresgid32 as c::c_long;
    #[cfg(not(any(target_arch = "x86", target_arch = "arm", target_arch = "sparc")))]
    const SYS: c::c_long = c::SYS_setresgid as c::c_long;

    syscall! {
        fn setresgid(rgid: c::gid_t, egid: c::gid_t, sgid: c::gid_t) via SYS -> c::c_int
    }

    unsafe { ret(setresgid(rgid.as_raw(), egid.as_raw(), sgid.as_raw())) }
}

/// # Safety
///
/// The raw pointers must point to valid aligned memory.
#[cfg(linux_kernel)]
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

    #[cfg(all(
        target_pointer_width = "32",
        not(any(target_arch = "aarch64", target_arch = "x86_64"))
    ))]
    {
        // TODO: Upstream this to the libc crate.
        #[allow(non_upper_case_globals)]
        const SYS_futex_time64: i32 = linux_raw_sys::general::__NR_futex_time64 as i32;

        syscall! {
            fn futex_time64(
                uaddr: *const AtomicU32,
                futex_op: c::c_int,
                val: u32,
                timeout: *const Timespec,
                uaddr2: *const AtomicU32,
                val3: u32
            ) via SYS_futex_time64 -> c::ssize_t
        }

        ret_usize(futex_time64(
            uaddr,
            op as i32 | flags.bits() as i32,
            val,
            timeout,
            uaddr2,
            val3,
        ))
    }

    #[cfg(any(
        target_pointer_width = "64",
        target_arch = "aarch64",
        target_arch = "x86_64"
    ))]
    {
        syscall! {
            fn futex(
                uaddr: *const AtomicU32,
                futex_op: c::c_int,
                val: u32,
                timeout: *const Timespec,
                uaddr2: *const AtomicU32,
                val3: u32
            ) via SYS_futex -> c::c_long
        }

        ret_usize(futex(
            uaddr,
            op as i32 | flags.bits() as i32,
            val,
            timeout.cast(),
            uaddr2,
            val3,
        ) as isize)
    }
}

/// # Safety
///
/// The raw pointers must point to valid aligned memory.
#[cfg(linux_kernel)]
pub(crate) unsafe fn futex_timeout(
    uaddr: *const AtomicU32,
    op: super::futex::Operation,
    flags: futex::Flags,
    val: u32,
    timeout: Option<&Timespec>,
    uaddr2: *const AtomicU32,
    val3: u32,
) -> io::Result<usize> {
    #[cfg(all(
        target_pointer_width = "32",
        not(any(target_arch = "aarch64", target_arch = "x86_64"))
    ))]
    {
        // TODO: Upstream this to the libc crate.
        #[allow(non_upper_case_globals)]
        const SYS_futex_time64: i32 = linux_raw_sys::general::__NR_futex_time64 as i32;

        syscall! {
            fn futex_time64(
                uaddr: *const AtomicU32,
                futex_op: c::c_int,
                val: u32,
                timeout: *const Timespec,
                uaddr2: *const AtomicU32,
                val3: u32
            ) via SYS_futex_time64 -> c::ssize_t
        }

        ret_usize(futex_time64(
            uaddr,
            op as i32 | flags.bits() as i32,
            val,
            option_as_ptr(timeout),
            uaddr2,
            val3,
        ))
        .or_else(|err| {
            // See the comments in `clock_gettime_via_syscall` about emulation.
            if err == io::Errno::NOSYS {
                futex_old_timespec(uaddr, op, flags, val, timeout, uaddr2, val3)
            } else {
                Err(err)
            }
        })
    }

    #[cfg(any(
        target_pointer_width = "64",
        target_arch = "aarch64",
        target_arch = "x86_64"
    ))]
    {
        syscall! {
            fn futex(
                uaddr: *const AtomicU32,
                futex_op: c::c_int,
                val: u32,
                timeout: *const Timespec,
                uaddr2: *const AtomicU32,
                val3: u32
            ) via SYS_futex -> c::c_long
        }

        ret_usize(futex(
            uaddr,
            op as i32 | flags.bits() as i32,
            val,
            option_as_ptr(timeout).cast(),
            uaddr2,
            val3,
        ) as isize)
    }
}

/// # Safety
///
/// The raw pointers must point to valid aligned memory.
#[cfg(linux_kernel)]
#[cfg(all(
    target_pointer_width = "32",
    not(any(target_arch = "aarch64", target_arch = "x86_64"))
))]
unsafe fn futex_old_timespec(
    uaddr: *const AtomicU32,
    op: super::futex::Operation,
    flags: futex::Flags,
    val: u32,
    timeout: Option<&Timespec>,
    uaddr2: *const AtomicU32,
    val3: u32,
) -> io::Result<usize> {
    syscall! {
        fn futex(
            uaddr: *const AtomicU32,
            futex_op: c::c_int,
            val: u32,
            timeout: *const linux_raw_sys::general::__kernel_old_timespec,
            uaddr2: *const AtomicU32,
            val3: u32
        ) via SYS_futex -> c::c_long
    }

    let old_timeout = if let Some(timeout) = timeout {
        Some(linux_raw_sys::general::__kernel_old_timespec {
            tv_sec: timeout.tv_sec.try_into().map_err(|_| io::Errno::INVAL)?,
            tv_nsec: timeout.tv_nsec.try_into().map_err(|_| io::Errno::INVAL)?,
        })
    } else {
        None
    };
    ret_usize(futex(
        uaddr,
        op as i32 | flags.bits() as i32,
        val,
        option_as_ptr(old_timeout.as_ref()),
        uaddr2,
        val3,
    ) as isize)
}

#[cfg(linux_kernel)]
pub(crate) fn futex_waitv(
    waiters: &[futex::Wait],
    flags: futex::WaitvFlags,
    timeout: Option<&Timespec>,
    clockid: ClockId,
) -> io::Result<usize> {
    use futex::Wait as FutexWait;
    use linux_raw_sys::general::__kernel_clockid_t as clockid_t;
    syscall! {
        fn futex_waitv(
            waiters: *const FutexWait,
            nr_futexes: c::c_uint,
            flags: c::c_uint,
            timeout: *const Timespec,
            clockid: clockid_t
        ) via SYS_futex_waitv -> c::c_int
    }

    let nr_futexes: c::c_uint = waiters.len().try_into().map_err(|_| io::Errno::INVAL)?;

    unsafe {
        ret_c_int(futex_waitv(
            waiters.as_ptr(),
            nr_futexes,
            flags.bits(),
            option_as_ptr(timeout).cast(),
            clockid as _,
        ))
        .map(|n| n as usize)
    }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn setgroups_thread(groups: &[crate::ugid::Gid]) -> io::Result<()> {
    syscall! {
        fn setgroups(size: c::size_t, list: *const c::gid_t) via SYS_setgroups -> c::c_int
    }
    ret(unsafe { setgroups(groups.len(), groups.as_ptr().cast()) })
}

#[cfg(any(linux_kernel, target_os = "dragonfly"))]
#[inline]
pub(crate) fn sched_getcpu() -> usize {
    let r = unsafe { c::sched_getcpu() };
    debug_assert!(r >= 0);
    r as usize
}

#[cfg(any(freebsdlike, linux_kernel, target_os = "fuchsia"))]
#[inline]
pub(crate) fn sched_getaffinity(pid: Option<Pid>, cpuset: &mut RawCpuSet) -> io::Result<()> {
    unsafe {
        ret(c::sched_getaffinity(
            Pid::as_raw(pid) as _,
            core::mem::size_of::<RawCpuSet>(),
            cpuset,
        ))
    }
}

#[cfg(any(freebsdlike, linux_kernel, target_os = "fuchsia"))]
#[inline]
pub(crate) fn sched_setaffinity(pid: Option<Pid>, cpuset: &RawCpuSet) -> io::Result<()> {
    unsafe {
        ret(c::sched_setaffinity(
            Pid::as_raw(pid) as _,
            core::mem::size_of::<RawCpuSet>(),
            cpuset,
        ))
    }
}

#[inline]
pub(crate) fn sched_yield() {
    unsafe {
        let _ = c::sched_yield();
    }
}

// The `membarrier` syscall has a third argument, but it's only used when
// the `flags` argument is `MEMBARRIER_CMD_FLAG_CPU`.
#[cfg(linux_kernel)]
syscall! {
    fn membarrier_all(
        cmd: c::c_int,
        flags: c::c_uint
    ) via SYS_membarrier -> c::c_int
}

#[cfg(linux_kernel)]
pub(crate) fn membarrier_query() -> MembarrierQuery {
    // glibc does not have a wrapper for `membarrier`; [the documentation]
    // says to use `syscall`.
    //
    // [the documentation]: https://man7.org/linux/man-pages/man2/membarrier.2.html#NOTES
    const MEMBARRIER_CMD_QUERY: u32 = 0;
    unsafe {
        match ret_u32(membarrier_all(MEMBARRIER_CMD_QUERY as i32, 0)) {
            Ok(query) => MembarrierQuery::from_bits_retain(query),
            Err(_) => MembarrierQuery::empty(),
        }
    }
}

#[cfg(linux_kernel)]
pub(crate) fn membarrier(cmd: MembarrierCommand) -> io::Result<()> {
    unsafe { ret(membarrier_all(cmd as i32, 0)) }
}

#[cfg(linux_kernel)]
pub(crate) fn membarrier_cpu(cmd: MembarrierCommand, cpu: Cpuid) -> io::Result<()> {
    const MEMBARRIER_CMD_FLAG_CPU: u32 = 1;

    syscall! {
        fn membarrier_cpu(
            cmd: c::c_int,
            flags: c::c_uint,
            cpu_id: c::c_int
        ) via SYS_membarrier -> c::c_int
    }

    unsafe {
        ret(membarrier_cpu(
            cmd as i32,
            MEMBARRIER_CMD_FLAG_CPU,
            bitcast!(cpu.as_raw()),
        ))
    }
}
