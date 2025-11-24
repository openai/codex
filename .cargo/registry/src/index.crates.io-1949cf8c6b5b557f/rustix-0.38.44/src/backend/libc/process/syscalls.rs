//! libc syscalls supporting `rustix::process`.

#[cfg(any(freebsdlike, linux_kernel, target_os = "fuchsia"))]
use super::types::RawCpuSet;
use crate::backend::c;
#[cfg(not(any(target_os = "wasi", target_os = "fuchsia")))]
use crate::backend::conv::borrowed_fd;
#[cfg(any(target_os = "linux", feature = "fs"))]
use crate::backend::conv::c_str;
#[cfg(all(feature = "alloc", feature = "fs", not(target_os = "wasi")))]
use crate::backend::conv::ret_discarded_char_ptr;
#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
use crate::backend::conv::ret_infallible;
#[cfg(not(target_os = "wasi"))]
use crate::backend::conv::ret_pid_t;
#[cfg(linux_kernel)]
use crate::backend::conv::ret_u32;
#[cfg(all(feature = "alloc", not(target_os = "wasi")))]
use crate::backend::conv::ret_usize;
use crate::backend::conv::{ret, ret_c_int};
#[cfg(not(any(target_os = "wasi", target_os = "fuchsia")))]
use crate::fd::BorrowedFd;
#[cfg(target_os = "linux")]
use crate::fd::{AsRawFd, OwnedFd, RawFd};
#[cfg(any(target_os = "linux", feature = "fs"))]
use crate::ffi::CStr;
#[cfg(feature = "fs")]
use crate::fs::Mode;
use crate::io;
#[cfg(all(feature = "alloc", not(target_os = "wasi")))]
use crate::process::Gid;
#[cfg(not(target_os = "wasi"))]
use crate::process::Pid;
#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
use crate::process::Signal;
#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "vita",
    target_os = "wasi"
)))]
use crate::process::Uid;
#[cfg(linux_kernel)]
use crate::process::{Cpuid, MembarrierCommand, MembarrierQuery};
#[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "wasi")))]
use crate::process::{RawPid, WaitOptions, WaitStatus};
#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
use crate::process::{Resource, Rlimit};
#[cfg(not(any(
    target_os = "espidf",
    target_os = "redox",
    target_os = "openbsd",
    target_os = "vita",
    target_os = "wasi"
)))]
use crate::process::{WaitId, WaitidOptions, WaitidStatus};
use core::mem::MaybeUninit;
#[cfg(target_os = "linux")]
use {
    super::super::conv::ret_owned_fd, crate::process::PidfdFlags, crate::process::PidfdGetfdFlags,
};

#[cfg(any(linux_kernel, target_os = "dragonfly"))]
#[inline]
pub(crate) fn sched_getcpu() -> usize {
    let r = unsafe { libc::sched_getcpu() };
    debug_assert!(r >= 0);
    r as usize
}

#[cfg(feature = "fs")]
#[cfg(not(target_os = "wasi"))]
pub(crate) fn chdir(path: &CStr) -> io::Result<()> {
    unsafe { ret(c::chdir(c_str(path))) }
}

#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))]
pub(crate) fn fchdir(dirfd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(c::fchdir(borrowed_fd(dirfd))) }
}

#[cfg(feature = "fs")]
#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))]
pub(crate) fn chroot(path: &CStr) -> io::Result<()> {
    unsafe { ret(c::chroot(c_str(path))) }
}

#[cfg(all(feature = "alloc", feature = "fs"))]
#[cfg(not(target_os = "wasi"))]
pub(crate) fn getcwd(buf: &mut [MaybeUninit<u8>]) -> io::Result<()> {
    unsafe { ret_discarded_char_ptr(c::getcwd(buf.as_mut_ptr().cast(), buf.len())) }
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

#[cfg(not(target_os = "wasi"))]
#[inline]
#[must_use]
pub(crate) fn getppid() -> Option<Pid> {
    unsafe {
        let pid: i32 = c::getppid();
        Pid::from_raw(pid)
    }
}

#[cfg(not(target_os = "wasi"))]
#[inline]
pub(crate) fn getpgid(pid: Option<Pid>) -> io::Result<Pid> {
    unsafe {
        let pgid = ret_pid_t(c::getpgid(Pid::as_raw(pid) as _))?;
        Ok(Pid::from_raw_unchecked(pgid))
    }
}

#[cfg(not(target_os = "wasi"))]
#[inline]
pub(crate) fn setpgid(pid: Option<Pid>, pgid: Option<Pid>) -> io::Result<()> {
    unsafe { ret(c::setpgid(Pid::as_raw(pid) as _, Pid::as_raw(pgid) as _)) }
}

#[cfg(not(target_os = "wasi"))]
#[inline]
#[must_use]
pub(crate) fn getpgrp() -> Pid {
    unsafe {
        let pgid = c::getpgrp();
        Pid::from_raw_unchecked(pgid)
    }
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

#[cfg(not(target_os = "wasi"))]
#[cfg(feature = "fs")]
#[inline]
pub(crate) fn umask(mask: Mode) -> Mode {
    unsafe { Mode::from_bits_retain(c::umask(mask.bits() as c::mode_t).into()) }
}

#[cfg(not(any(target_os = "fuchsia", target_os = "vita", target_os = "wasi")))]
#[inline]
pub(crate) fn nice(inc: i32) -> io::Result<i32> {
    libc_errno::set_errno(libc_errno::Errno(0));
    let r = unsafe { c::nice(inc) };
    if libc_errno::errno().0 != 0 {
        ret_c_int(r)
    } else {
        Ok(r)
    }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub(crate) fn getpriority_user(uid: Uid) -> io::Result<i32> {
    libc_errno::set_errno(libc_errno::Errno(0));
    let r = unsafe { c::getpriority(c::PRIO_USER, uid.as_raw() as _) };
    if libc_errno::errno().0 != 0 {
        ret_c_int(r)
    } else {
        Ok(r)
    }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub(crate) fn getpriority_pgrp(pgid: Option<Pid>) -> io::Result<i32> {
    libc_errno::set_errno(libc_errno::Errno(0));
    let r = unsafe { c::getpriority(c::PRIO_PGRP, Pid::as_raw(pgid) as _) };
    if libc_errno::errno().0 != 0 {
        ret_c_int(r)
    } else {
        Ok(r)
    }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub(crate) fn getpriority_process(pid: Option<Pid>) -> io::Result<i32> {
    libc_errno::set_errno(libc_errno::Errno(0));
    let r = unsafe { c::getpriority(c::PRIO_PROCESS, Pid::as_raw(pid) as _) };
    if libc_errno::errno().0 != 0 {
        ret_c_int(r)
    } else {
        Ok(r)
    }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub(crate) fn setpriority_user(uid: Uid, priority: i32) -> io::Result<()> {
    unsafe { ret(c::setpriority(c::PRIO_USER, uid.as_raw() as _, priority)) }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub(crate) fn setpriority_pgrp(pgid: Option<Pid>, priority: i32) -> io::Result<()> {
    unsafe {
        ret(c::setpriority(
            c::PRIO_PGRP,
            Pid::as_raw(pgid) as _,
            priority,
        ))
    }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub(crate) fn setpriority_process(pid: Option<Pid>, priority: i32) -> io::Result<()> {
    unsafe {
        ret(c::setpriority(
            c::PRIO_PROCESS,
            Pid::as_raw(pid) as _,
            priority,
        ))
    }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub(crate) fn getrlimit(limit: Resource) -> Rlimit {
    let mut result = MaybeUninit::<c::rlimit>::uninit();
    unsafe {
        ret_infallible(c::getrlimit(limit as _, result.as_mut_ptr()));
        rlimit_from_libc(result.assume_init())
    }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub(crate) fn setrlimit(limit: Resource, new: Rlimit) -> io::Result<()> {
    let lim = rlimit_to_libc(new)?;
    unsafe { ret(c::setrlimit(limit as _, &lim)) }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn prlimit(pid: Option<Pid>, limit: Resource, new: Rlimit) -> io::Result<Rlimit> {
    let lim = rlimit_to_libc(new)?;
    let mut result = MaybeUninit::<c::rlimit>::uninit();
    unsafe {
        ret(c::prlimit(
            Pid::as_raw(pid),
            limit as _,
            &lim,
            result.as_mut_ptr(),
        ))?;
        Ok(rlimit_from_libc(result.assume_init()))
    }
}

/// Convert a C `c::rlimit` to a Rust `Rlimit`.
#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
fn rlimit_from_libc(lim: c::rlimit) -> Rlimit {
    let current = if lim.rlim_cur == c::RLIM_INFINITY {
        None
    } else {
        Some(lim.rlim_cur.try_into().unwrap())
    };
    let maximum = if lim.rlim_max == c::RLIM_INFINITY {
        None
    } else {
        Some(lim.rlim_max.try_into().unwrap())
    };
    Rlimit { current, maximum }
}

/// Convert a Rust [`Rlimit`] to a C `c::rlimit`.
#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
fn rlimit_to_libc(lim: Rlimit) -> io::Result<c::rlimit> {
    let Rlimit { current, maximum } = lim;
    let rlim_cur = match current {
        Some(r) => r.try_into().map_err(|_e| io::Errno::INVAL)?,
        None => c::RLIM_INFINITY as _,
    };
    let rlim_max = match maximum {
        Some(r) => r.try_into().map_err(|_e| io::Errno::INVAL)?,
        None => c::RLIM_INFINITY as _,
    };
    Ok(c::rlimit { rlim_cur, rlim_max })
}

#[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "wasi")))]
#[inline]
pub(crate) fn wait(waitopts: WaitOptions) -> io::Result<Option<(Pid, WaitStatus)>> {
    _waitpid(!0, waitopts)
}

#[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "wasi")))]
#[inline]
pub(crate) fn waitpid(
    pid: Option<Pid>,
    waitopts: WaitOptions,
) -> io::Result<Option<(Pid, WaitStatus)>> {
    _waitpid(Pid::as_raw(pid), waitopts)
}

#[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "wasi")))]
#[inline]
pub(crate) fn waitpgid(pgid: Pid, waitopts: WaitOptions) -> io::Result<Option<(Pid, WaitStatus)>> {
    _waitpid(-pgid.as_raw_nonzero().get(), waitopts)
}

#[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "wasi")))]
#[inline]
pub(crate) fn _waitpid(
    pid: RawPid,
    waitopts: WaitOptions,
) -> io::Result<Option<(Pid, WaitStatus)>> {
    unsafe {
        let mut status: c::c_int = 0;
        let pid = ret_c_int(c::waitpid(pid as _, &mut status, waitopts.bits() as _))?;
        Ok(Pid::from_raw(pid).map(|pid| (pid, WaitStatus::new(status as _))))
    }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "redox",
    target_os = "openbsd",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub(crate) fn waitid(id: WaitId<'_>, options: WaitidOptions) -> io::Result<Option<WaitidStatus>> {
    // Get the id to wait on.
    match id {
        WaitId::All => _waitid_all(options),
        WaitId::Pid(pid) => _waitid_pid(pid, options),
        WaitId::Pgid(pgid) => _waitid_pgid(pgid, options),
        #[cfg(target_os = "linux")]
        WaitId::PidFd(fd) => _waitid_pidfd(fd, options),
        #[cfg(not(target_os = "linux"))]
        WaitId::__EatLifetime(_) => unreachable!(),
    }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "redox",
    target_os = "openbsd",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
fn _waitid_all(options: WaitidOptions) -> io::Result<Option<WaitidStatus>> {
    // `waitid` can return successfully without initializing the struct (no
    // children found when using `WNOHANG`)
    let mut status = MaybeUninit::<c::siginfo_t>::zeroed();
    unsafe {
        ret(c::waitid(
            c::P_ALL,
            0,
            status.as_mut_ptr(),
            options.bits() as _,
        ))?
    };

    Ok(unsafe { cvt_waitid_status(status) })
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "redox",
    target_os = "openbsd",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
fn _waitid_pid(pid: Pid, options: WaitidOptions) -> io::Result<Option<WaitidStatus>> {
    // `waitid` can return successfully without initializing the struct (no
    // children found when using `WNOHANG`)
    let mut status = MaybeUninit::<c::siginfo_t>::zeroed();
    unsafe {
        ret(c::waitid(
            c::P_PID,
            Pid::as_raw(Some(pid)) as _,
            status.as_mut_ptr(),
            options.bits() as _,
        ))?
    };

    Ok(unsafe { cvt_waitid_status(status) })
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "redox",
    target_os = "openbsd",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
fn _waitid_pgid(pgid: Option<Pid>, options: WaitidOptions) -> io::Result<Option<WaitidStatus>> {
    // `waitid` can return successfully without initializing the struct (no
    // children found when using `WNOHANG`)
    let mut status = MaybeUninit::<c::siginfo_t>::zeroed();
    unsafe {
        ret(c::waitid(
            c::P_PGID,
            Pid::as_raw(pgid) as _,
            status.as_mut_ptr(),
            options.bits() as _,
        ))?
    };

    Ok(unsafe { cvt_waitid_status(status) })
}

#[cfg(target_os = "linux")]
#[inline]
fn _waitid_pidfd(fd: BorrowedFd<'_>, options: WaitidOptions) -> io::Result<Option<WaitidStatus>> {
    // `waitid` can return successfully without initializing the struct (no
    // children found when using `WNOHANG`)
    let mut status = MaybeUninit::<c::siginfo_t>::zeroed();
    unsafe {
        ret(c::waitid(
            c::P_PIDFD,
            fd.as_raw_fd() as _,
            status.as_mut_ptr(),
            options.bits() as _,
        ))?
    };

    Ok(unsafe { cvt_waitid_status(status) })
}

/// Convert a `siginfo_t` to a `WaitidStatus`.
///
/// # Safety
///
/// The caller must ensure that `status` is initialized and that `waitid`
/// returned successfully.
#[cfg(not(any(
    target_os = "espidf",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
unsafe fn cvt_waitid_status(status: MaybeUninit<c::siginfo_t>) -> Option<WaitidStatus> {
    let status = status.assume_init();
    // `si_pid` is supposedly the better way to check that the struct has been
    // filled, e.g. the Linux manual page says about the `WNOHANG` case “zero
    // out the si_pid field before the call and check for a nonzero value”.
    // But e.g. NetBSD/OpenBSD don't have it exposed in the libc crate for now,
    // and some platforms don't have it at all. For simplicity, always check
    // `si_signo`. We have zero-initialized the whole struct, and all kernels
    // should set `SIGCHLD` here.
    if status.si_signo == 0 {
        None
    } else {
        Some(WaitidStatus(status))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
#[inline]
pub(crate) fn getsid(pid: Option<Pid>) -> io::Result<Pid> {
    unsafe {
        let pid = ret_pid_t(c::getsid(Pid::as_raw(pid) as _))?;
        Ok(Pid::from_raw_unchecked(pid))
    }
}

#[cfg(not(target_os = "wasi"))]
#[inline]
pub(crate) fn setsid() -> io::Result<Pid> {
    unsafe {
        let pid = ret_c_int(c::setsid())?;
        Ok(Pid::from_raw_unchecked(pid))
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
#[inline]
pub(crate) fn kill_process(pid: Pid, sig: Signal) -> io::Result<()> {
    unsafe { ret(c::kill(pid.as_raw_nonzero().get(), sig as i32)) }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
#[inline]
pub(crate) fn kill_process_group(pid: Pid, sig: Signal) -> io::Result<()> {
    unsafe {
        ret(c::kill(
            pid.as_raw_nonzero().get().wrapping_neg(),
            sig as i32,
        ))
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
#[inline]
pub(crate) fn kill_current_process_group(sig: Signal) -> io::Result<()> {
    unsafe { ret(c::kill(0, sig as i32)) }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
pub(crate) fn test_kill_process(pid: Pid) -> io::Result<()> {
    unsafe { ret(c::kill(pid.as_raw_nonzero().get(), 0)) }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
#[inline]
pub(crate) fn test_kill_process_group(pid: Pid) -> io::Result<()> {
    unsafe { ret(c::kill(pid.as_raw_nonzero().get().wrapping_neg(), 0)) }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
#[inline]
pub(crate) fn test_kill_current_process_group() -> io::Result<()> {
    unsafe { ret(c::kill(0, 0)) }
}

#[cfg(freebsdlike)]
#[inline]
pub(crate) unsafe fn procctl(
    idtype: c::idtype_t,
    id: c::id_t,
    option: c::c_int,
    data: *mut c::c_void,
) -> io::Result<()> {
    ret(c::procctl(idtype, id, option, data))
}

#[cfg(target_os = "linux")]
pub(crate) fn pidfd_open(pid: Pid, flags: PidfdFlags) -> io::Result<OwnedFd> {
    syscall! {
        fn pidfd_open(
            pid: c::pid_t,
            flags: c::c_uint
        ) via SYS_pidfd_open -> c::c_int
    }
    unsafe {
        ret_owned_fd(pidfd_open(
            pid.as_raw_nonzero().get(),
            bitflags_bits!(flags),
        ))
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn pidfd_send_signal(pidfd: BorrowedFd<'_>, sig: Signal) -> io::Result<()> {
    syscall! {
        fn pidfd_send_signal(
            pid: c::pid_t,
            sig: c::c_int,
            info: *const c::siginfo_t,
            flags: c::c_int
        ) via SYS_pidfd_send_signal -> c::c_int
    }
    unsafe {
        ret(pidfd_send_signal(
            borrowed_fd(pidfd),
            sig as c::c_int,
            core::ptr::null(),
            0,
        ))
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn pidfd_getfd(
    pidfd: BorrowedFd<'_>,
    targetfd: RawFd,
    flags: PidfdGetfdFlags,
) -> io::Result<OwnedFd> {
    syscall! {
        fn pidfd_getfd(
            pidfd: c::c_int,
            targetfd: c::c_int,
            flags: c::c_uint
        ) via SYS_pidfd_getfd -> c::c_int
    }
    unsafe {
        ret_owned_fd(pidfd_getfd(
            borrowed_fd(pidfd),
            targetfd,
            bitflags_bits!(flags),
        ))
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn pivot_root(new_root: &CStr, put_old: &CStr) -> io::Result<()> {
    syscall! {
        fn pivot_root(
            new_root: *const c::c_char,
            put_old: *const c::c_char
        ) via SYS_pivot_root -> c::c_int
    }
    unsafe { ret(pivot_root(c_str(new_root), c_str(put_old))) }
}

#[cfg(all(feature = "alloc", not(target_os = "wasi")))]
pub(crate) fn getgroups(buf: &mut [Gid]) -> io::Result<usize> {
    let len = buf.len().try_into().map_err(|_| io::Errno::NOMEM)?;

    unsafe { ret_usize(c::getgroups(len, buf.as_mut_ptr().cast()) as isize) }
}
