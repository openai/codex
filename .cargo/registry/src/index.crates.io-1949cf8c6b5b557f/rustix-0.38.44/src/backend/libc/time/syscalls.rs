//! libc syscalls supporting `rustix::time`.

use crate::backend::c;
use crate::backend::conv::ret;
#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[cfg(feature = "time")]
#[cfg(any(all(target_env = "gnu", fix_y2038), not(fix_y2038)))]
use crate::backend::time::types::LibcItimerspec;
#[cfg(not(target_os = "wasi"))]
use crate::clockid::{ClockId, DynamicClockId};
use crate::io;
#[cfg(all(target_env = "gnu", fix_y2038))]
use crate::timespec::LibcTimespec;
use crate::timespec::Timespec;
use core::mem::MaybeUninit;
#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[cfg(feature = "time")]
use {
    crate::backend::conv::{borrowed_fd, ret_owned_fd},
    crate::fd::{BorrowedFd, OwnedFd},
    crate::time::{Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags},
};

#[cfg(all(target_env = "gnu", fix_y2038))]
weak!(fn __clock_gettime64(c::clockid_t, *mut LibcTimespec) -> c::c_int);
#[cfg(all(target_env = "gnu", fix_y2038))]
weak!(fn __clock_settime64(c::clockid_t, *const LibcTimespec) -> c::c_int);
#[cfg(all(target_env = "gnu", fix_y2038))]
weak!(fn __clock_getres64(c::clockid_t, *mut LibcTimespec) -> c::c_int);
#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[cfg(all(target_env = "gnu", fix_y2038))]
#[cfg(feature = "time")]
weak!(fn __timerfd_gettime64(c::c_int, *mut LibcItimerspec) -> c::c_int);
#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[cfg(all(target_env = "gnu", fix_y2038))]
#[cfg(feature = "time")]
weak!(fn __timerfd_settime64(c::c_int, c::c_int, *const LibcItimerspec, *mut LibcItimerspec) -> c::c_int);

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
#[inline]
#[must_use]
pub(crate) fn clock_getres(id: ClockId) -> Timespec {
    // Old 32-bit version: libc has `clock_getres` but it is not y2038 safe by
    // default. But there may be a `__clock_getres64` we can use.
    #[cfg(fix_y2038)]
    {
        #[cfg(target_env = "gnu")]
        if let Some(libc_clock_getres) = __clock_getres64.get() {
            let mut timespec = MaybeUninit::<LibcTimespec>::uninit();
            unsafe {
                ret(libc_clock_getres(id as c::clockid_t, timespec.as_mut_ptr())).unwrap();
                return timespec.assume_init().into();
            }
        }

        clock_getres_old(id)
    }

    // Main version: libc is y2038 safe and has `clock_getres`.
    #[cfg(not(fix_y2038))]
    unsafe {
        let mut timespec = MaybeUninit::<Timespec>::uninit();
        let _ = c::clock_getres(id as c::clockid_t, timespec.as_mut_ptr());
        timespec.assume_init()
    }
}

#[cfg(fix_y2038)]
#[must_use]
fn clock_getres_old(id: ClockId) -> Timespec {
    let mut old_timespec = MaybeUninit::<c::timespec>::uninit();

    let old_timespec = unsafe {
        ret(c::clock_getres(
            id as c::clockid_t,
            old_timespec.as_mut_ptr(),
        ))
        .unwrap();
        old_timespec.assume_init()
    };

    Timespec {
        tv_sec: old_timespec.tv_sec.into(),
        tv_nsec: old_timespec.tv_nsec.into(),
    }
}

#[cfg(not(target_os = "wasi"))]
#[inline]
#[must_use]
pub(crate) fn clock_gettime(id: ClockId) -> Timespec {
    // Old 32-bit version: libc has `clock_gettime` but it is not y2038 safe by
    // default. But there may be a `__clock_gettime64` we can use.
    #[cfg(fix_y2038)]
    {
        #[cfg(target_env = "gnu")]
        if let Some(libc_clock_gettime) = __clock_gettime64.get() {
            let mut timespec = MaybeUninit::<LibcTimespec>::uninit();
            unsafe {
                ret(libc_clock_gettime(
                    id as c::clockid_t,
                    timespec.as_mut_ptr(),
                ))
                .unwrap();
                return timespec.assume_init().into();
            }
        }

        clock_gettime_old(id)
    }

    // Use `.unwrap()` here because `clock_getres` can fail if the clock itself
    // overflows a number of seconds, but if that happens, the monotonic clocks
    // can't maintain their invariants, or the realtime clocks aren't properly
    // configured.
    #[cfg(not(fix_y2038))]
    unsafe {
        let mut timespec = MaybeUninit::<Timespec>::uninit();
        ret(c::clock_gettime(id as c::clockid_t, timespec.as_mut_ptr())).unwrap();
        timespec.assume_init()
    }
}

#[cfg(fix_y2038)]
#[must_use]
fn clock_gettime_old(id: ClockId) -> Timespec {
    let mut old_timespec = MaybeUninit::<c::timespec>::uninit();

    let old_timespec = unsafe {
        ret(c::clock_gettime(
            id as c::clockid_t,
            old_timespec.as_mut_ptr(),
        ))
        .unwrap();
        old_timespec.assume_init()
    };

    Timespec {
        tv_sec: old_timespec.tv_sec.into(),
        tv_nsec: old_timespec.tv_nsec.into(),
    }
}

#[cfg(not(target_os = "wasi"))]
#[inline]
pub(crate) fn clock_gettime_dynamic(id: DynamicClockId<'_>) -> io::Result<Timespec> {
    let id: c::clockid_t = match id {
        DynamicClockId::Known(id) => id as c::clockid_t,

        #[cfg(linux_kernel)]
        DynamicClockId::Dynamic(fd) => {
            use crate::fd::AsRawFd;
            const CLOCKFD: i32 = 3;
            (!fd.as_raw_fd() << 3) | CLOCKFD
        }

        #[cfg(not(linux_kernel))]
        DynamicClockId::Dynamic(_fd) => {
            // Dynamic clocks are not supported on this platform.
            return Err(io::Errno::INVAL);
        }

        #[cfg(linux_kernel)]
        DynamicClockId::RealtimeAlarm => c::CLOCK_REALTIME_ALARM,

        #[cfg(linux_kernel)]
        DynamicClockId::Tai => c::CLOCK_TAI,

        #[cfg(any(
            linux_kernel,
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "openbsd"
        ))]
        DynamicClockId::Boottime => c::CLOCK_BOOTTIME,

        #[cfg(any(linux_kernel, target_os = "fuchsia"))]
        DynamicClockId::BoottimeAlarm => c::CLOCK_BOOTTIME_ALARM,
    };

    // Old 32-bit version: libc has `clock_gettime` but it is not y2038
    // safe by default. But there may be a `__clock_gettime64` we can use.
    #[cfg(fix_y2038)]
    {
        #[cfg(target_env = "gnu")]
        if let Some(libc_clock_gettime) = __clock_gettime64.get() {
            let mut timespec = MaybeUninit::<LibcTimespec>::uninit();
            unsafe {
                ret(libc_clock_gettime(
                    id as c::clockid_t,
                    timespec.as_mut_ptr(),
                ))?;

                return Ok(timespec.assume_init().into());
            }
        }

        clock_gettime_dynamic_old(id)
    }

    // Main version: libc is y2038 safe and has `clock_gettime`.
    #[cfg(not(fix_y2038))]
    unsafe {
        let mut timespec = MaybeUninit::<Timespec>::uninit();

        ret(c::clock_gettime(id as c::clockid_t, timespec.as_mut_ptr()))?;

        Ok(timespec.assume_init())
    }
}

#[cfg(fix_y2038)]
#[inline]
fn clock_gettime_dynamic_old(id: c::clockid_t) -> io::Result<Timespec> {
    let mut old_timespec = MaybeUninit::<c::timespec>::uninit();

    let old_timespec = unsafe {
        ret(c::clock_gettime(
            id as c::clockid_t,
            old_timespec.as_mut_ptr(),
        ))?;

        old_timespec.assume_init()
    };

    Ok(Timespec {
        tv_sec: old_timespec.tv_sec.into(),
        tv_nsec: old_timespec.tv_nsec.into(),
    })
}

#[cfg(not(any(
    target_os = "redox",
    target_os = "wasi",
    all(apple, not(target_os = "macos"))
)))]
#[inline]
pub(crate) fn clock_settime(id: ClockId, timespec: Timespec) -> io::Result<()> {
    // Old 32-bit version: libc has `clock_gettime` but it is not y2038 safe by
    // default. But there may be a `__clock_settime64` we can use.
    #[cfg(fix_y2038)]
    {
        #[cfg(target_env = "gnu")]
        if let Some(libc_clock_settime) = __clock_settime64.get() {
            unsafe {
                let mut new_timespec = core::mem::zeroed::<LibcTimespec>();
                new_timespec.tv_sec = timespec.tv_sec;
                new_timespec.tv_nsec = timespec.tv_nsec as _;
                return ret(libc_clock_settime(id as c::clockid_t, &new_timespec));
            }
        }

        clock_settime_old(id, timespec)
    }

    // Main version: libc is y2038 safe and has `clock_settime`.
    #[cfg(not(fix_y2038))]
    unsafe {
        ret(c::clock_settime(id as c::clockid_t, &timespec))
    }
}

#[cfg(not(any(
    target_os = "redox",
    target_os = "wasi",
    all(apple, not(target_os = "macos"))
)))]
#[cfg(fix_y2038)]
fn clock_settime_old(id: ClockId, timespec: Timespec) -> io::Result<()> {
    let old_timespec = c::timespec {
        tv_sec: timespec
            .tv_sec
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        tv_nsec: timespec.tv_nsec as _,
    };

    unsafe { ret(c::clock_settime(id as c::clockid_t, &old_timespec)) }
}

#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[cfg(feature = "time")]
pub(crate) fn timerfd_create(id: TimerfdClockId, flags: TimerfdFlags) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(c::timerfd_create(id as c::clockid_t, bitflags_bits!(flags))) }
}

#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[cfg(feature = "time")]
pub(crate) fn timerfd_settime(
    fd: BorrowedFd<'_>,
    flags: TimerfdTimerFlags,
    new_value: &Itimerspec,
) -> io::Result<Itimerspec> {
    // Old 32-bit version: libc has `timerfd_settime` but it is not y2038 safe
    // by default. But there may be a `__timerfd_settime64` we can use.
    #[cfg(fix_y2038)]
    {
        #[cfg(target_env = "gnu")]
        if let Some(libc_timerfd_settime) = __timerfd_settime64.get() {
            let mut result = MaybeUninit::<LibcItimerspec>::uninit();
            unsafe {
                ret(libc_timerfd_settime(
                    borrowed_fd(fd),
                    bitflags_bits!(flags),
                    &new_value.clone().into(),
                    result.as_mut_ptr(),
                ))?;
                return Ok(result.assume_init().into());
            }
        }

        timerfd_settime_old(fd, flags, new_value)
    }

    #[cfg(not(fix_y2038))]
    unsafe {
        let mut result = MaybeUninit::<LibcItimerspec>::uninit();
        ret(c::timerfd_settime(
            borrowed_fd(fd),
            bitflags_bits!(flags),
            new_value,
            result.as_mut_ptr(),
        ))?;
        Ok(result.assume_init())
    }
}

#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[cfg(fix_y2038)]
#[cfg(feature = "time")]
fn timerfd_settime_old(
    fd: BorrowedFd<'_>,
    flags: TimerfdTimerFlags,
    new_value: &Itimerspec,
) -> io::Result<Itimerspec> {
    let mut old_result = MaybeUninit::<c::itimerspec>::uninit();

    // Convert `new_value` to the old `itimerspec` format.
    let old_new_value = c::itimerspec {
        it_interval: c::timespec {
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
        it_value: c::timespec {
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

    let old_result = unsafe {
        ret(c::timerfd_settime(
            borrowed_fd(fd),
            bitflags_bits!(flags),
            &old_new_value,
            old_result.as_mut_ptr(),
        ))?;
        old_result.assume_init()
    };

    Ok(Itimerspec {
        it_interval: Timespec {
            tv_sec: old_result
                .it_interval
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: old_result.it_interval.tv_nsec as _,
        },
        it_value: Timespec {
            tv_sec: old_result
                .it_interval
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: old_result.it_interval.tv_nsec as _,
        },
    })
}

#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[cfg(feature = "time")]
pub(crate) fn timerfd_gettime(fd: BorrowedFd<'_>) -> io::Result<Itimerspec> {
    // Old 32-bit version: libc has `timerfd_gettime` but it is not y2038 safe
    // by default. But there may be a `__timerfd_gettime64` we can use.
    #[cfg(fix_y2038)]
    {
        #[cfg(target_env = "gnu")]
        if let Some(libc_timerfd_gettime) = __timerfd_gettime64.get() {
            let mut result = MaybeUninit::<LibcItimerspec>::uninit();
            unsafe {
                ret(libc_timerfd_gettime(borrowed_fd(fd), result.as_mut_ptr()))?;
                return Ok(result.assume_init().into());
            }
        }

        timerfd_gettime_old(fd)
    }

    #[cfg(not(fix_y2038))]
    unsafe {
        let mut result = MaybeUninit::<LibcItimerspec>::uninit();
        ret(c::timerfd_gettime(borrowed_fd(fd), result.as_mut_ptr()))?;
        Ok(result.assume_init())
    }
}

#[cfg(any(linux_kernel, target_os = "fuchsia"))]
#[cfg(fix_y2038)]
#[cfg(feature = "time")]
fn timerfd_gettime_old(fd: BorrowedFd<'_>) -> io::Result<Itimerspec> {
    let mut old_result = MaybeUninit::<c::itimerspec>::uninit();

    let old_result = unsafe {
        ret(c::timerfd_gettime(borrowed_fd(fd), old_result.as_mut_ptr()))?;
        old_result.assume_init()
    };

    Ok(Itimerspec {
        it_interval: Timespec {
            tv_sec: old_result
                .it_interval
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: old_result.it_interval.tv_nsec as _,
        },
        it_value: Timespec {
            tv_sec: old_result
                .it_interval
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: old_result.it_interval.tv_nsec as _,
        },
    })
}
