//! linux_raw syscalls supporting `rustix::process`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::c;
#[cfg(all(feature = "alloc", feature = "fs"))]
use crate::backend::conv::slice_mut;
use crate::backend::conv::{
    by_mut, by_ref, c_int, c_uint, negative_pid, pass_usize, raw_fd, ret, ret_c_int,
    ret_c_int_infallible, ret_infallible, ret_owned_fd, zero,
};
use crate::fd::{AsRawFd as _, BorrowedFd, OwnedFd, RawFd};
#[cfg(feature = "fs")]
use crate::ffi::CStr;
use crate::io;
use crate::pid::RawPid;
use crate::process::{
    Flock, Pid, PidfdFlags, PidfdGetfdFlags, Resource, Rlimit, Uid, WaitId, WaitIdOptions,
    WaitIdStatus, WaitOptions, WaitStatus,
};
use crate::signal::Signal;
use core::mem::MaybeUninit;
use core::ptr::{null, null_mut};
use linux_raw_sys::general::{rlimit64, PRIO_PGRP, PRIO_PROCESS, PRIO_USER, RLIM64_INFINITY};
#[cfg(feature = "fs")]
use {crate::backend::conv::ret_c_uint_infallible, crate::fs::Mode};
#[cfg(feature = "alloc")]
use {
    crate::backend::conv::{ret_usize, slice_just_addr_mut},
    crate::process::Gid,
};

#[cfg(feature = "fs")]
#[inline]
pub(crate) fn chdir(filename: &CStr) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_chdir, filename)) }
}

#[inline]
pub(crate) fn fchdir(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_fchdir, fd)) }
}

#[cfg(feature = "fs")]
#[inline]
pub(crate) fn chroot(filename: &CStr) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_chroot, filename)) }
}

#[cfg(all(feature = "alloc", feature = "fs"))]
#[inline]
pub(crate) fn getcwd(buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
    let (buf_addr_mut, buf_len) = slice_mut(buf);
    unsafe { ret_usize(syscall!(__NR_getcwd, buf_addr_mut, buf_len)) }
}

#[inline]
#[must_use]
pub(crate) fn getppid() -> Option<Pid> {
    unsafe {
        let ppid = ret_c_int_infallible(syscall_readonly!(__NR_getppid));
        Pid::from_raw(ppid)
    }
}

#[inline]
pub(crate) fn getpgid(pid: Option<Pid>) -> io::Result<Pid> {
    unsafe {
        let pgid = ret_c_int(syscall_readonly!(__NR_getpgid, c_int(Pid::as_raw(pid))))?;
        debug_assert!(pgid > 0);
        Ok(Pid::from_raw_unchecked(pgid))
    }
}

#[inline]
pub(crate) fn setpgid(pid: Option<Pid>, pgid: Option<Pid>) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_setpgid,
            c_int(Pid::as_raw(pid)),
            c_int(Pid::as_raw(pgid))
        ))
    }
}

#[inline]
#[must_use]
pub(crate) fn getpgrp() -> Pid {
    // Use the `getpgrp` syscall if available.
    #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
    unsafe {
        let pgid = ret_c_int_infallible(syscall_readonly!(__NR_getpgrp));
        debug_assert!(pgid > 0);
        Pid::from_raw_unchecked(pgid)
    }

    // Otherwise use `getpgrp` and pass it zero.
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    unsafe {
        let pgid = ret_c_int_infallible(syscall_readonly!(__NR_getpgid, c_uint(0)));
        debug_assert!(pgid > 0);
        Pid::from_raw_unchecked(pgid)
    }
}

#[cfg(feature = "fs")]
#[inline]
pub(crate) fn umask(mode: Mode) -> Mode {
    unsafe { Mode::from_bits_retain(ret_c_uint_infallible(syscall_readonly!(__NR_umask, mode))) }
}

#[inline]
pub(crate) fn nice(inc: i32) -> io::Result<i32> {
    let priority = (if inc > -40 && inc < 40 {
        inc + getpriority_process(None)?
    } else {
        inc
    })
    .clamp(-20, 19);
    setpriority_process(None, priority)?;
    Ok(priority)
}

#[inline]
pub(crate) fn getpriority_user(uid: Uid) -> io::Result<i32> {
    unsafe {
        Ok(20
            - ret_c_int(syscall_readonly!(
                __NR_getpriority,
                c_uint(PRIO_USER),
                c_uint(uid.as_raw())
            ))?)
    }
}

#[inline]
pub(crate) fn getpriority_pgrp(pgid: Option<Pid>) -> io::Result<i32> {
    unsafe {
        Ok(20
            - ret_c_int(syscall_readonly!(
                __NR_getpriority,
                c_uint(PRIO_PGRP),
                c_int(Pid::as_raw(pgid))
            ))?)
    }
}

#[inline]
pub(crate) fn getpriority_process(pid: Option<Pid>) -> io::Result<i32> {
    unsafe {
        Ok(20
            - ret_c_int(syscall_readonly!(
                __NR_getpriority,
                c_uint(PRIO_PROCESS),
                c_int(Pid::as_raw(pid))
            ))?)
    }
}

#[inline]
pub(crate) fn setpriority_user(uid: Uid, priority: i32) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_setpriority,
            c_uint(PRIO_USER),
            c_uint(uid.as_raw()),
            c_int(priority)
        ))
    }
}

#[inline]
pub(crate) fn setpriority_pgrp(pgid: Option<Pid>, priority: i32) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_setpriority,
            c_uint(PRIO_PGRP),
            c_int(Pid::as_raw(pgid)),
            c_int(priority)
        ))
    }
}

#[inline]
pub(crate) fn setpriority_process(pid: Option<Pid>, priority: i32) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_setpriority,
            c_uint(PRIO_PROCESS),
            c_int(Pid::as_raw(pid)),
            c_int(priority)
        ))
    }
}

#[inline]
pub(crate) fn getrlimit(limit: Resource) -> Rlimit {
    let mut result = MaybeUninit::<rlimit64>::uninit();
    unsafe {
        ret_infallible(syscall!(
            __NR_prlimit64,
            c_uint(0),
            limit,
            null::<c::c_void>(),
            &mut result
        ));
        rlimit_from_linux(result.assume_init())
    }
}

#[inline]
pub(crate) fn setrlimit(limit: Resource, new: Rlimit) -> io::Result<()> {
    unsafe {
        let lim = rlimit_to_linux(new);
        match ret(syscall_readonly!(
            __NR_prlimit64,
            c_uint(0),
            limit,
            by_ref(&lim),
            null_mut::<c::c_void>()
        )) {
            Ok(()) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

#[inline]
pub(crate) fn prlimit(pid: Option<Pid>, limit: Resource, new: Rlimit) -> io::Result<Rlimit> {
    let lim = rlimit_to_linux(new);
    let mut result = MaybeUninit::<rlimit64>::uninit();
    unsafe {
        match ret(syscall!(
            __NR_prlimit64,
            c_int(Pid::as_raw(pid)),
            limit,
            by_ref(&lim),
            &mut result
        )) {
            Ok(()) => Ok(rlimit_from_linux(result.assume_init())),
            Err(err) => Err(err),
        }
    }
}

/// Convert a C `rlimit64` to a Rust `Rlimit`.
#[inline]
fn rlimit_from_linux(lim: rlimit64) -> Rlimit {
    let current = if lim.rlim_cur == RLIM64_INFINITY as u64 {
        None
    } else {
        Some(lim.rlim_cur)
    };
    let maximum = if lim.rlim_max == RLIM64_INFINITY as u64 {
        None
    } else {
        Some(lim.rlim_max)
    };
    Rlimit { current, maximum }
}

/// Convert a Rust [`Rlimit`] to a C `rlimit64`.
#[inline]
fn rlimit_to_linux(lim: Rlimit) -> rlimit64 {
    let rlim_cur = match lim.current {
        Some(r) => r,
        None => RLIM64_INFINITY as _,
    };
    let rlim_max = match lim.maximum {
        Some(r) => r,
        None => RLIM64_INFINITY as _,
    };
    rlimit64 { rlim_cur, rlim_max }
}

#[inline]
pub(crate) fn wait(waitopts: WaitOptions) -> io::Result<Option<(Pid, WaitStatus)>> {
    _waitpid(!0, waitopts)
}

#[inline]
pub(crate) fn waitpid(
    pid: Option<Pid>,
    waitopts: WaitOptions,
) -> io::Result<Option<(Pid, WaitStatus)>> {
    _waitpid(Pid::as_raw(pid), waitopts)
}

#[inline]
pub(crate) fn waitpgid(pgid: Pid, waitopts: WaitOptions) -> io::Result<Option<(Pid, WaitStatus)>> {
    _waitpid(-pgid.as_raw_nonzero().get(), waitopts)
}

#[inline]
pub(crate) fn _waitpid(
    pid: RawPid,
    waitopts: WaitOptions,
) -> io::Result<Option<(Pid, WaitStatus)>> {
    unsafe {
        let mut status = MaybeUninit::<i32>::uninit();
        let pid = ret_c_int(syscall!(
            __NR_wait4,
            c_int(pid as _),
            &mut status,
            c_int(waitopts.bits() as _),
            zero()
        ))?;
        Ok(Pid::from_raw(pid).map(|pid| (pid, WaitStatus::new(status.assume_init()))))
    }
}

#[inline]
pub(crate) fn waitid(id: WaitId<'_>, options: WaitIdOptions) -> io::Result<Option<WaitIdStatus>> {
    // Get the id to wait on.
    match id {
        WaitId::All => _waitid_all(options),
        WaitId::Pid(pid) => _waitid_pid(pid, options),
        WaitId::Pgid(pid) => _waitid_pgid(pid, options),
        WaitId::PidFd(fd) => _waitid_pidfd(fd, options),
    }
}

#[inline]
fn _waitid_all(options: WaitIdOptions) -> io::Result<Option<WaitIdStatus>> {
    // `waitid` can return successfully without initializing the struct (no
    // children found when using `WNOHANG`)
    let mut status = MaybeUninit::<c::siginfo_t>::zeroed();
    unsafe {
        ret(syscall!(
            __NR_waitid,
            c_uint(c::P_ALL),
            c_uint(0),
            &mut status,
            c_int(options.bits() as _),
            zero()
        ))?
    };

    Ok(unsafe { cvt_waitid_status(status) })
}

#[inline]
fn _waitid_pid(pid: Pid, options: WaitIdOptions) -> io::Result<Option<WaitIdStatus>> {
    // `waitid` can return successfully without initializing the struct (no
    // children found when using `WNOHANG`)
    let mut status = MaybeUninit::<c::siginfo_t>::zeroed();
    unsafe {
        ret(syscall!(
            __NR_waitid,
            c_uint(c::P_PID),
            c_int(Pid::as_raw(Some(pid))),
            &mut status,
            c_int(options.bits() as _),
            zero()
        ))?
    };

    Ok(unsafe { cvt_waitid_status(status) })
}

#[inline]
fn _waitid_pgid(pgid: Option<Pid>, options: WaitIdOptions) -> io::Result<Option<WaitIdStatus>> {
    // `waitid` can return successfully without initializing the struct (no
    // children found when using `WNOHANG`)
    let mut status = MaybeUninit::<c::siginfo_t>::zeroed();
    unsafe {
        ret(syscall!(
            __NR_waitid,
            c_uint(c::P_PGID),
            c_int(Pid::as_raw(pgid)),
            &mut status,
            c_int(options.bits() as _),
            zero()
        ))?
    };

    Ok(unsafe { cvt_waitid_status(status) })
}

#[inline]
fn _waitid_pidfd(fd: BorrowedFd<'_>, options: WaitIdOptions) -> io::Result<Option<WaitIdStatus>> {
    // `waitid` can return successfully without initializing the struct (no
    // children found when using `WNOHANG`)
    let mut status = MaybeUninit::<c::siginfo_t>::zeroed();
    unsafe {
        ret(syscall!(
            __NR_waitid,
            c_uint(c::P_PIDFD),
            c_uint(fd.as_raw_fd() as _),
            &mut status,
            c_int(options.bits() as _),
            zero()
        ))?
    };

    Ok(unsafe { cvt_waitid_status(status) })
}

/// Convert a `siginfo_t` to a `WaitIdStatus`.
///
/// # Safety
///
/// The caller must ensure that `status` is initialized and that `waitid`
/// returned successfully.
#[inline]
unsafe fn cvt_waitid_status(status: MaybeUninit<c::siginfo_t>) -> Option<WaitIdStatus> {
    let status = status.assume_init();
    if status
        .__bindgen_anon_1
        .__bindgen_anon_1
        ._sifields
        ._sigchld
        ._pid
        == 0
    {
        None
    } else {
        Some(WaitIdStatus(status))
    }
}

#[inline]
pub(crate) fn getsid(pid: Option<Pid>) -> io::Result<Pid> {
    unsafe {
        let pid = ret_c_int(syscall_readonly!(__NR_getsid, c_int(Pid::as_raw(pid))))?;
        Ok(Pid::from_raw_unchecked(pid))
    }
}

#[inline]
pub(crate) fn setsid() -> io::Result<Pid> {
    unsafe {
        let pid = ret_c_int(syscall_readonly!(__NR_setsid))?;
        Ok(Pid::from_raw_unchecked(pid))
    }
}

#[inline]
pub(crate) fn kill_process(pid: Pid, sig: Signal) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_kill, pid, sig)) }
}

#[inline]
pub(crate) fn kill_process_group(pid: Pid, sig: Signal) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_kill, negative_pid(pid), sig)) }
}

#[inline]
pub(crate) fn kill_current_process_group(sig: Signal) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_kill, pass_usize(0), sig)) }
}

#[inline]
pub(crate) fn test_kill_process(pid: Pid) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_kill, pid, pass_usize(0))) }
}

#[inline]
pub(crate) fn test_kill_process_group(pid: Pid) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_kill,
            negative_pid(pid),
            pass_usize(0)
        ))
    }
}

#[inline]
pub(crate) fn test_kill_current_process_group() -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_kill, pass_usize(0), pass_usize(0))) }
}

#[inline]
pub(crate) fn pidfd_getfd(
    pidfd: BorrowedFd<'_>,
    targetfd: RawFd,
    flags: PidfdGetfdFlags,
) -> io::Result<OwnedFd> {
    unsafe {
        ret_owned_fd(syscall_readonly!(
            __NR_pidfd_getfd,
            pidfd,
            raw_fd(targetfd),
            c_int(flags.bits() as _)
        ))
    }
}

#[inline]
pub(crate) fn pidfd_open(pid: Pid, flags: PidfdFlags) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(syscall_readonly!(__NR_pidfd_open, pid, flags)) }
}

#[inline]
pub(crate) fn pidfd_send_signal(fd: BorrowedFd<'_>, sig: Signal) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_pidfd_send_signal,
            fd,
            sig,
            pass_usize(0),
            pass_usize(0)
        ))
    }
}

#[cfg(feature = "fs")]
#[inline]
pub(crate) fn pivot_root(new_root: &CStr, put_old: &CStr) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_pivot_root, new_root, put_old)) }
}

#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn getgroups(buf: &mut [Gid]) -> io::Result<usize> {
    let len = buf.len().try_into().map_err(|_| io::Errno::NOMEM)?;

    unsafe {
        ret_usize(syscall!(
            __NR_getgroups,
            c_int(len),
            slice_just_addr_mut(buf)
        ))
    }
}

#[inline]
pub(crate) fn fcntl_getlk(fd: BorrowedFd<'_>, lock: &Flock) -> io::Result<Option<Flock>> {
    let mut curr_lock: c::flock = lock.as_raw();
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall!(
            __NR_fcntl64,
            fd,
            c_uint(c::F_GETLK64),
            by_mut(&mut curr_lock)
        ))?
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall!(
            __NR_fcntl,
            fd,
            c_uint(c::F_GETLK),
            by_mut(&mut curr_lock)
        ))?
    }

    // If no blocking lock is found, `fcntl(GETLK, ..)` sets `l_type` to
    // `F_UNLCK`.
    if curr_lock.l_type == c::F_UNLCK as _ {
        Ok(None)
    } else {
        Ok(Some(unsafe { Flock::from_raw_unchecked(curr_lock) }))
    }
}
