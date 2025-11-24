//! linux_raw syscalls supporting `rustix::runtime`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::c;
#[cfg(target_arch = "x86")]
use crate::backend::conv::by_mut;
#[cfg(target_arch = "x86_64")]
use crate::backend::conv::c_uint;
use crate::backend::conv::{
    by_ref, c_int, opt_ref, ret, ret_c_int, ret_c_int_infallible, ret_error, ret_infallible,
    ret_void_star, size_of, zero,
};
#[cfg(feature = "fs")]
use crate::fd::BorrowedFd;
use crate::ffi::CStr;
#[cfg(feature = "fs")]
use crate::fs::AtFlags;
use crate::io;
use crate::pid::{Pid, RawPid};
use crate::runtime::{Fork, How, KernelSigSet, KernelSigaction, Siginfo, Stack};
use crate::signal::Signal;
use crate::timespec::Timespec;
use core::ffi::c_void;
use core::mem::MaybeUninit;
#[cfg(all(target_pointer_width = "32", not(feature = "linux_5_1")))]
use linux_raw_sys::general::__kernel_old_timespec;
#[cfg(target_arch = "x86_64")]
use linux_raw_sys::general::ARCH_SET_FS;

#[inline]
pub(crate) unsafe fn kernel_fork() -> io::Result<Fork> {
    let mut child_pid = MaybeUninit::<RawPid>::uninit();

    // Unix `fork` only returns the child PID in the parent; we'd like it in
    // the child too, so set `CLONE_CHILD_SETTID` and pass in the address of a
    // memory location to store it to in the child.
    //
    // Architectures differ on the order of the parameters.
    #[cfg(target_arch = "x86_64")]
    let pid = ret_c_int(syscall!(
        __NR_clone,
        c_int(c::SIGCHLD | c::CLONE_CHILD_SETTID),
        zero(),
        zero(),
        &mut child_pid,
        zero()
    ))?;
    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "riscv64",
        target_arch = "s390x",
        target_arch = "x86"
    ))]
    let pid = ret_c_int(syscall!(
        __NR_clone,
        c_int(c::SIGCHLD | c::CLONE_CHILD_SETTID),
        zero(),
        zero(),
        zero(),
        &mut child_pid
    ))?;

    Ok(if let Some(pid) = Pid::from_raw(pid) {
        Fork::ParentOf(pid)
    } else {
        Fork::Child(Pid::from_raw_unchecked(child_pid.assume_init()))
    })
}

#[cfg(feature = "fs")]
pub(crate) unsafe fn execveat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    args: *const *const u8,
    env_vars: *const *const u8,
    flags: AtFlags,
) -> io::Errno {
    ret_error(syscall_readonly!(
        __NR_execveat,
        dirfd,
        path,
        args,
        env_vars,
        flags
    ))
}

pub(crate) unsafe fn execve(
    path: &CStr,
    args: *const *const u8,
    env_vars: *const *const u8,
) -> io::Errno {
    ret_error(syscall_readonly!(__NR_execve, path, args, env_vars))
}

pub(crate) mod tls {
    use super::*;
    #[cfg(target_arch = "x86")]
    use crate::backend::runtime::tls::UserDesc;

    #[cfg(target_arch = "x86")]
    #[inline]
    pub(crate) unsafe fn set_thread_area(u_info: &mut UserDesc) -> io::Result<()> {
        ret(syscall!(__NR_set_thread_area, by_mut(u_info)))
    }

    #[cfg(target_arch = "arm")]
    #[inline]
    pub(crate) unsafe fn arm_set_tls(data: *mut c::c_void) -> io::Result<()> {
        ret(syscall_readonly!(__ARM_NR_set_tls, data))
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    pub(crate) unsafe fn set_fs(data: *mut c::c_void) {
        ret_infallible(syscall_readonly!(
            __NR_arch_prctl,
            c_uint(ARCH_SET_FS),
            data,
            zero(),
            zero(),
            zero()
        ))
    }

    #[inline]
    pub(crate) unsafe fn set_tid_address(data: *mut c::c_void) -> Pid {
        let tid: i32 = ret_c_int_infallible(syscall_readonly!(__NR_set_tid_address, data));
        Pid::from_raw_unchecked(tid)
    }

    #[inline]
    pub(crate) fn exit_thread(code: c::c_int) -> ! {
        unsafe { syscall_noreturn!(__NR_exit, c_int(code)) }
    }
}

#[inline]
pub(crate) unsafe fn kernel_sigaction(
    signal: Signal,
    new: Option<KernelSigaction>,
) -> io::Result<KernelSigaction> {
    let mut old = MaybeUninit::<KernelSigaction>::uninit();
    let new = opt_ref(new.as_ref());
    ret(syscall!(
        __NR_rt_sigaction,
        signal,
        new,
        &mut old,
        size_of::<KernelSigSet, _>()
    ))?;
    Ok(old.assume_init())
}

#[inline]
pub(crate) unsafe fn kernel_sigaltstack(new: Option<Stack>) -> io::Result<Stack> {
    let mut old = MaybeUninit::<Stack>::uninit();
    let new = opt_ref(new.as_ref());
    ret(syscall!(__NR_sigaltstack, new, &mut old))?;
    Ok(old.assume_init())
}

#[inline]
pub(crate) unsafe fn tkill(tid: Pid, sig: Signal) -> io::Result<()> {
    ret(syscall_readonly!(__NR_tkill, tid, sig))
}

#[inline]
pub(crate) unsafe fn kernel_sigprocmask(
    how: How,
    new: Option<&KernelSigSet>,
) -> io::Result<KernelSigSet> {
    let mut old = MaybeUninit::<KernelSigSet>::uninit();
    let new = opt_ref(new);
    ret(syscall!(
        __NR_rt_sigprocmask,
        how,
        new,
        &mut old,
        size_of::<KernelSigSet, _>()
    ))?;
    Ok(old.assume_init())
}

#[inline]
pub(crate) fn kernel_sigpending() -> KernelSigSet {
    let mut pending = MaybeUninit::<KernelSigSet>::uninit();
    unsafe {
        ret_infallible(syscall!(
            __NR_rt_sigpending,
            &mut pending,
            size_of::<KernelSigSet, _>()
        ));
        pending.assume_init()
    }
}

#[inline]
pub(crate) fn kernel_sigsuspend(set: &KernelSigSet) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_rt_sigsuspend,
            by_ref(set),
            size_of::<KernelSigSet, _>()
        ))
    }
}

#[inline]
pub(crate) unsafe fn kernel_sigwait(set: &KernelSigSet) -> io::Result<Signal> {
    Ok(Signal::from_raw_unchecked(ret_c_int(syscall_readonly!(
        __NR_rt_sigtimedwait,
        by_ref(set),
        zero(),
        zero(),
        size_of::<KernelSigSet, _>()
    ))?))
}

#[inline]
pub(crate) unsafe fn kernel_sigwaitinfo(set: &KernelSigSet) -> io::Result<Siginfo> {
    let mut info = MaybeUninit::<Siginfo>::uninit();
    let _signum = ret_c_int(syscall!(
        __NR_rt_sigtimedwait,
        by_ref(set),
        &mut info,
        zero(),
        size_of::<KernelSigSet, _>()
    ))?;
    Ok(info.assume_init())
}

#[inline]
pub(crate) unsafe fn kernel_sigtimedwait(
    set: &KernelSigSet,
    timeout: Option<&Timespec>,
) -> io::Result<Siginfo> {
    let mut info = MaybeUninit::<Siginfo>::uninit();

    // `rt_sigtimedwait_time64` was introduced in Linux 5.1. The old
    // `rt_sigtimedwait` syscall is not y2038-compatible on 32-bit
    // architectures.
    #[cfg(target_pointer_width = "32")]
    {
        // If we don't have Linux 5.1, and the timeout fits in a
        // `__kernel_old_timespec`, use plain `rt_sigtimedwait`.
        //
        // We do this unconditionally, rather than trying
        // `rt_sigtimedwait_time64` and falling back on `Errno::NOSYS`, because
        // seccomp configurations will sometimes abort the process on syscalls
        // they don't recognize.
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
                return ret_c_int(syscall!(
                    __NR_rt_sigtimedwait,
                    by_ref(set),
                    &mut info,
                    opt_ref(old_timeout.as_ref()),
                    size_of::<KernelSigSet, _>()
                ))
                .map(|sig| {
                    debug_assert_eq!(
                        sig,
                        info.assume_init_ref()
                            .__bindgen_anon_1
                            .__bindgen_anon_1
                            .si_signo
                    );
                    info.assume_init()
                });
            }
        }

        ret_c_int(syscall!(
            __NR_rt_sigtimedwait_time64,
            by_ref(set),
            &mut info,
            opt_ref(timeout),
            size_of::<KernelSigSet, _>()
        ))
        .map(|sig| {
            debug_assert_eq!(
                sig,
                info.assume_init_ref()
                    .__bindgen_anon_1
                    .__bindgen_anon_1
                    .si_signo
            );
            info.assume_init()
        })
    }

    #[cfg(target_pointer_width = "64")]
    {
        let _signum = ret_c_int(syscall!(
            __NR_rt_sigtimedwait,
            by_ref(set),
            &mut info,
            opt_ref(timeout),
            size_of::<KernelSigSet, _>()
        ))?;
        Ok(info.assume_init())
    }
}

#[inline]
pub(crate) fn exit_group(code: c::c_int) -> ! {
    unsafe { syscall_noreturn!(__NR_exit_group, c_int(code)) }
}

#[inline]
pub(crate) unsafe fn kernel_brk(addr: *mut c::c_void) -> io::Result<*mut c_void> {
    // This is non-`readonly`, to prevent loads from being reordered past it.
    ret_void_star(syscall!(__NR_brk, addr))
}
