//! linux_raw syscalls supporting `rustix::time`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::conv::{ret, ret_infallible};
use crate::clockid::ClockId;
use crate::io;
use crate::timespec::Timespec;
use core::mem::MaybeUninit;
#[cfg(all(feature = "time", target_pointer_width = "32"))]
use linux_raw_sys::general::itimerspec as __kernel_old_itimerspec;
#[cfg(target_pointer_width = "32")]
use linux_raw_sys::general::timespec as __kernel_old_timespec;
#[cfg(feature = "time")]
use {
    crate::backend::conv::{by_ref, ret_owned_fd},
    crate::fd::BorrowedFd,
    crate::fd::OwnedFd,
    crate::time::{Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags},
};

// `clock_gettime` has special optimizations via the vDSO.
#[cfg(feature = "time")]
pub(crate) use crate::backend::vdso_wrappers::{clock_gettime, clock_gettime_dynamic};

#[inline]
pub(crate) fn clock_getres(which_clock: ClockId) -> Timespec {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<Timespec>::uninit();
        if let Err(err) = ret(syscall!(__NR_clock_getres_time64, which_clock, &mut result)) {
            // See the comments in `rustix_clock_gettime_via_syscall` about
            // emulation.
            debug_assert_eq!(err, io::Errno::NOSYS);
            clock_getres_old(which_clock, &mut result);
        }
        result.assume_init()
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut result = MaybeUninit::<Timespec>::uninit();
        ret_infallible(syscall!(__NR_clock_getres, which_clock, &mut result));
        result.assume_init()
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn clock_getres_old(which_clock: ClockId, result: &mut MaybeUninit<Timespec>) {
    let mut old_result = MaybeUninit::<__kernel_old_timespec>::uninit();
    ret_infallible(syscall!(__NR_clock_getres, which_clock, &mut old_result));
    let old_result = old_result.assume_init();
    result.write(Timespec {
        tv_sec: old_result.tv_sec.into(),
        tv_nsec: old_result.tv_nsec.into(),
    });
}

#[cfg(feature = "time")]
#[inline]
pub(crate) fn clock_settime(which_clock: ClockId, timespec: Timespec) -> io::Result<()> {
    // `clock_settime64` was introduced in Linux 5.1. The old `clock_settime`
    // syscall is not y2038-compatible on 32-bit architectures.
    #[cfg(target_pointer_width = "32")]
    unsafe {
        match ret(syscall_readonly!(
            __NR_clock_settime64,
            which_clock,
            by_ref(&timespec)
        )) {
            Err(io::Errno::NOSYS) => clock_settime_old(which_clock, timespec),
            otherwise => otherwise,
        }
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall_readonly!(
            __NR_clock_settime,
            which_clock,
            by_ref(&timespec)
        ))
    }
}

#[cfg(feature = "time")]
#[cfg(target_pointer_width = "32")]
unsafe fn clock_settime_old(which_clock: ClockId, timespec: Timespec) -> io::Result<()> {
    let old_timespec = __kernel_old_timespec {
        tv_sec: timespec
            .tv_sec
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        tv_nsec: timespec.tv_nsec as _,
    };
    ret(syscall_readonly!(
        __NR_clock_settime,
        which_clock,
        by_ref(&old_timespec)
    ))
}

#[cfg(feature = "time")]
#[inline]
pub(crate) fn timerfd_create(clockid: TimerfdClockId, flags: TimerfdFlags) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(syscall_readonly!(__NR_timerfd_create, clockid, flags)) }
}

#[cfg(feature = "time")]
#[inline]
pub(crate) fn timerfd_settime(
    fd: BorrowedFd<'_>,
    flags: TimerfdTimerFlags,
    new_value: &Itimerspec,
) -> io::Result<Itimerspec> {
    let mut result = MaybeUninit::<Itimerspec>::uninit();

    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall!(
            __NR_timerfd_settime,
            fd,
            flags,
            by_ref(new_value),
            &mut result
        ))?;
        Ok(result.assume_init())
    }

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall!(
            __NR_timerfd_settime64,
            fd,
            flags,
            by_ref(new_value),
            &mut result
        ))
        .or_else(|err| {
            // See the comments in `rustix_clock_gettime_via_syscall` about
            // emulation.
            if err == io::Errno::NOSYS {
                timerfd_settime_old(fd, flags, new_value, &mut result)
            } else {
                Err(err)
            }
        })?;
        Ok(result.assume_init())
    }
}

#[cfg(feature = "time")]
#[cfg(target_pointer_width = "32")]
unsafe fn timerfd_settime_old(
    fd: BorrowedFd<'_>,
    flags: TimerfdTimerFlags,
    new_value: &Itimerspec,
    result: &mut MaybeUninit<Itimerspec>,
) -> io::Result<()> {
    let mut old_result = MaybeUninit::<__kernel_old_itimerspec>::uninit();

    // Convert `new_value` to the old `__kernel_old_itimerspec` format.
    let old_new_value = __kernel_old_itimerspec {
        it_interval: __kernel_old_timespec {
            tv_sec: new_value
                .it_interval
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: new_value
                .it_interval
                .tv_nsec
                .try_into()
                .map_err(|_| io::Errno::INVAL)?,
        },
        it_value: __kernel_old_timespec {
            tv_sec: new_value
                .it_value
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: new_value
                .it_value
                .tv_nsec
                .try_into()
                .map_err(|_| io::Errno::INVAL)?,
        },
    };
    ret(syscall!(
        __NR_timerfd_settime,
        fd,
        flags,
        by_ref(&old_new_value),
        &mut old_result
    ))?;
    let old_result = old_result.assume_init();
    result.write(Itimerspec {
        it_interval: Timespec {
            tv_sec: old_result.it_interval.tv_sec.into(),
            tv_nsec: old_result.it_interval.tv_nsec.into(),
        },
        it_value: Timespec {
            tv_sec: old_result.it_value.tv_sec.into(),
            tv_nsec: old_result.it_value.tv_nsec.into(),
        },
    });
    Ok(())
}

#[cfg(feature = "time")]
#[inline]
pub(crate) fn timerfd_gettime(fd: BorrowedFd<'_>) -> io::Result<Itimerspec> {
    let mut result = MaybeUninit::<Itimerspec>::uninit();

    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall!(__NR_timerfd_gettime, fd, &mut result))?;
        Ok(result.assume_init())
    }

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall!(__NR_timerfd_gettime64, fd, &mut result)).or_else(|err| {
            // See the comments in `rustix_clock_gettime_via_syscall` about
            // emulation.
            if err == io::Errno::NOSYS {
                timerfd_gettime_old(fd, &mut result)
            } else {
                Err(err)
            }
        })?;
        Ok(result.assume_init())
    }
}

#[cfg(feature = "time")]
#[cfg(target_pointer_width = "32")]
unsafe fn timerfd_gettime_old(
    fd: BorrowedFd<'_>,
    result: &mut MaybeUninit<Itimerspec>,
) -> io::Result<()> {
    let mut old_result = MaybeUninit::<__kernel_old_itimerspec>::uninit();
    ret(syscall!(__NR_timerfd_gettime, fd, &mut old_result))?;
    let old_result = old_result.assume_init();
    result.write(Itimerspec {
        it_interval: Timespec {
            tv_sec: old_result.it_interval.tv_sec.into(),
            tv_nsec: old_result.it_interval.tv_nsec.into(),
        },
        it_value: Timespec {
            tv_sec: old_result.it_value.tv_sec.into(),
            tv_nsec: old_result.it_value.tv_nsec.into(),
        },
    });
    Ok(())
}
