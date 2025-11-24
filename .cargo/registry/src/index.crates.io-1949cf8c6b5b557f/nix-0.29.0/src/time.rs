//! Sleep, query system clocks, and set system clock
use crate::sys::time::TimeSpec;
#[cfg(any(freebsdlike, linux_android, target_os = "emscripten"))]
#[cfg(feature = "process")]
use crate::unistd::Pid;
use crate::{Errno, Result};
use libc::{self, clockid_t};
use std::mem::MaybeUninit;

/// Clock identifier
///
/// Newtype pattern around [`libc::clockid_t`].
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ClockId(clockid_t);

impl ClockId {
    /// Creates `ClockId` from raw `clockid_t`
    pub const fn from_raw(clk_id: clockid_t) -> Self {
        ClockId(clk_id)
    }

    feature! {
    #![feature = "process"]
    /// Returns `ClockId` of a `pid` CPU-time clock
    #[cfg(any(freebsdlike, linux_android, target_os = "emscripten"))]
    pub fn pid_cpu_clock_id(pid: Pid) -> Result<Self> {
        clock_getcpuclockid(pid)
    }
    }

    /// Returns resolution of the clock id
    #[cfg(not(target_os = "redox"))]
    pub fn res(self) -> Result<TimeSpec> {
        clock_getres(self)
    }

    /// Returns the current time on the clock id
    pub fn now(self) -> Result<TimeSpec> {
        clock_gettime(self)
    }

    /// Sets time to `timespec` on the clock id
    #[cfg(not(any(
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "redox",
        target_os = "hermit"
    )))]
    pub fn set_time(self, timespec: TimeSpec) -> Result<()> {
        clock_settime(self, timespec)
    }

    /// Gets the raw `clockid_t` wrapped by `self`
    pub const fn as_raw(self) -> clockid_t {
        self.0
    }

    #[cfg(any(linux_android, target_os = "emscripten", target_os = "fuchsia"))]
    /// Starts at zero when the kernel boots and increments monotonically in SI seconds while the
    /// machine is running.
    pub const CLOCK_BOOTTIME: ClockId = ClockId(libc::CLOCK_BOOTTIME);
    /// Like [`CLOCK_BOOTTIME`](ClockId::CLOCK_BOOTTIME), but will wake the system if it is
    /// suspended..
    #[cfg(any(linux_android, target_os = "emscripten", target_os = "fuchsia"))]
    pub const CLOCK_BOOTTIME_ALARM: ClockId =
        ClockId(libc::CLOCK_BOOTTIME_ALARM);
    /// Increments in SI seconds.
    pub const CLOCK_MONOTONIC: ClockId = ClockId(libc::CLOCK_MONOTONIC);
    /// Like [`CLOCK_MONOTONIC`](ClockId::CLOCK_MONOTONIC), but optimized for execution time at the expense of accuracy.
    #[cfg(any(linux_android, target_os = "emscripten", target_os = "fuchsia"))]
    pub const CLOCK_MONOTONIC_COARSE: ClockId =
        ClockId(libc::CLOCK_MONOTONIC_COARSE);
    #[cfg(freebsdlike)]
    /// Like [`CLOCK_MONOTONIC`](ClockId::CLOCK_MONOTONIC), but optimized for execution time at the expense of accuracy.
    pub const CLOCK_MONOTONIC_FAST: ClockId =
        ClockId(libc::CLOCK_MONOTONIC_FAST);
    #[cfg(freebsdlike)]
    /// Like [`CLOCK_MONOTONIC`](ClockId::CLOCK_MONOTONIC), but optimized for accuracy at the expense of execution time.
    pub const CLOCK_MONOTONIC_PRECISE: ClockId =
        ClockId(libc::CLOCK_MONOTONIC_PRECISE);
    /// Similar to [`CLOCK_MONOTONIC`](ClockId::CLOCK_MONOTONIC), but provides access to a raw
    /// hardware-based time that is not subject to NTP adjustments.
    #[cfg(any(linux_android, target_os = "emscripten", target_os = "fuchsia"))]
    pub const CLOCK_MONOTONIC_RAW: ClockId = ClockId(libc::CLOCK_MONOTONIC_RAW);
    #[cfg(any(
        linux_android,
        apple_targets,
        freebsdlike,
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "redox",
    ))]
    /// Returns the execution time of the calling process.
    pub const CLOCK_PROCESS_CPUTIME_ID: ClockId =
        ClockId(libc::CLOCK_PROCESS_CPUTIME_ID);
    #[cfg(freebsdlike)]
    /// Increments when the CPU is running in user or kernel mode
    pub const CLOCK_PROF: ClockId = ClockId(libc::CLOCK_PROF);
    /// Increments as a wall clock should.
    pub const CLOCK_REALTIME: ClockId = ClockId(libc::CLOCK_REALTIME);
    /// Like [`CLOCK_REALTIME`](ClockId::CLOCK_REALTIME), but not settable.
    #[cfg(any(linux_android, target_os = "emscripten", target_os = "fuchsia"))]
    pub const CLOCK_REALTIME_ALARM: ClockId =
        ClockId(libc::CLOCK_REALTIME_ALARM);
    /// Like [`CLOCK_REALTIME`](ClockId::CLOCK_REALTIME), but optimized for execution time at the expense of accuracy.
    #[cfg(any(linux_android, target_os = "emscripten", target_os = "fuchsia"))]
    pub const CLOCK_REALTIME_COARSE: ClockId =
        ClockId(libc::CLOCK_REALTIME_COARSE);
    #[cfg(freebsdlike)]
    /// Like [`CLOCK_REALTIME`](ClockId::CLOCK_REALTIME), but optimized for execution time at the expense of accuracy.
    pub const CLOCK_REALTIME_FAST: ClockId = ClockId(libc::CLOCK_REALTIME_FAST);
    #[cfg(freebsdlike)]
    /// Like [`CLOCK_REALTIME`](ClockId::CLOCK_REALTIME), but optimized for accuracy at the expense of execution time.
    pub const CLOCK_REALTIME_PRECISE: ClockId =
        ClockId(libc::CLOCK_REALTIME_PRECISE);
    #[cfg(freebsdlike)]
    /// Returns the current second without performing a full time counter query, using an in-kernel
    /// cached value of the current second.
    pub const CLOCK_SECOND: ClockId = ClockId(libc::CLOCK_SECOND);
    #[allow(missing_docs)] // Undocumented on Linux!
    #[cfg(any(
        target_os = "emscripten",
        target_os = "fuchsia",
        all(target_os = "linux", target_env = "musl")
    ))]
    pub const CLOCK_SGI_CYCLE: ClockId = ClockId(libc::CLOCK_SGI_CYCLE);
    /// International Atomic Time.
    ///
    /// A nonsettable system-wide clock derived from wall-clock time but ignoring leap seconds.
    #[cfg(any(linux_android, target_os = "emscripten", target_os = "fuchsia"))]
    pub const CLOCK_TAI: ClockId = ClockId(libc::CLOCK_TAI);
    #[cfg(any(
        linux_android,
        apple_targets,
        freebsdlike,
        target_os = "emscripten",
        target_os = "fuchsia",
    ))]
    /// Returns the execution time of the calling thread.
    pub const CLOCK_THREAD_CPUTIME_ID: ClockId =
        ClockId(libc::CLOCK_THREAD_CPUTIME_ID);
    #[cfg(freebsdlike)]
    /// Starts at zero when the kernel boots and increments monotonically in SI seconds while the
    /// machine is running.
    pub const CLOCK_UPTIME: ClockId = ClockId(libc::CLOCK_UPTIME);
    #[cfg(freebsdlike)]
    /// Like [`CLOCK_UPTIME`](ClockId::CLOCK_UPTIME), but optimized for execution time at the expense of accuracy.
    pub const CLOCK_UPTIME_FAST: ClockId = ClockId(libc::CLOCK_UPTIME_FAST);
    #[cfg(freebsdlike)]
    /// Like [`CLOCK_UPTIME`](ClockId::CLOCK_UPTIME), but optimized for accuracy at the expense of execution time.
    pub const CLOCK_UPTIME_PRECISE: ClockId =
        ClockId(libc::CLOCK_UPTIME_PRECISE);
    #[cfg(freebsdlike)]
    /// Increments only when the CPU is running in user mode on behalf of the calling process.
    pub const CLOCK_VIRTUAL: ClockId = ClockId(libc::CLOCK_VIRTUAL);
}

impl From<ClockId> for clockid_t {
    fn from(clock_id: ClockId) -> Self {
        clock_id.as_raw()
    }
}

impl From<clockid_t> for ClockId {
    fn from(clk_id: clockid_t) -> Self {
        ClockId::from_raw(clk_id)
    }
}

impl std::fmt::Display for ClockId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

/// Get the resolution of the specified clock, (see
/// [clock_getres(2)](https://pubs.opengroup.org/onlinepubs/7908799/xsh/clock_getres.html)).
#[cfg(not(target_os = "redox"))]
pub fn clock_getres(clock_id: ClockId) -> Result<TimeSpec> {
    let mut c_time: MaybeUninit<libc::timespec> = MaybeUninit::uninit();
    let ret =
        unsafe { libc::clock_getres(clock_id.as_raw(), c_time.as_mut_ptr()) };
    Errno::result(ret)?;
    let res = unsafe { c_time.assume_init() };
    Ok(TimeSpec::from(res))
}

/// Get the time of the specified clock, (see
/// [clock_gettime(2)](https://pubs.opengroup.org/onlinepubs/7908799/xsh/clock_gettime.html)).
pub fn clock_gettime(clock_id: ClockId) -> Result<TimeSpec> {
    let mut c_time: MaybeUninit<libc::timespec> = MaybeUninit::uninit();
    let ret =
        unsafe { libc::clock_gettime(clock_id.as_raw(), c_time.as_mut_ptr()) };
    Errno::result(ret)?;
    let res = unsafe { c_time.assume_init() };
    Ok(TimeSpec::from(res))
}

/// Set the time of the specified clock, (see
/// [clock_settime(2)](https://pubs.opengroup.org/onlinepubs/7908799/xsh/clock_settime.html)).
#[cfg(not(any(
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "redox",
    target_os = "hermit"
)))]
pub fn clock_settime(clock_id: ClockId, timespec: TimeSpec) -> Result<()> {
    let ret =
        unsafe { libc::clock_settime(clock_id.as_raw(), timespec.as_ref()) };
    Errno::result(ret).map(drop)
}

/// Get the clock id of the specified process id, (see
/// [clock_getcpuclockid(3)](https://pubs.opengroup.org/onlinepubs/009695399/functions/clock_getcpuclockid.html)).
#[cfg(any(freebsdlike, linux_android, target_os = "emscripten"))]
#[cfg(feature = "process")]
#[cfg_attr(docsrs, doc(cfg(feature = "process")))]
pub fn clock_getcpuclockid(pid: Pid) -> Result<ClockId> {
    let mut clk_id: MaybeUninit<libc::clockid_t> = MaybeUninit::uninit();
    let ret =
        unsafe { libc::clock_getcpuclockid(pid.into(), clk_id.as_mut_ptr()) };
    if ret == 0 {
        let res = unsafe { clk_id.assume_init() };
        Ok(ClockId::from(res))
    } else {
        Err(Errno::from_raw(ret))
    }
}

#[cfg(any(
    linux_android,
    solarish,
    freebsdlike,
    target_os = "netbsd",
    target_os = "hurd",
    target_os = "aix"
))]
libc_bitflags! {
    /// Flags that are used for arming the timer.
    pub struct ClockNanosleepFlags: libc::c_int {
        /// Indicates that a requested time value should be treated as absolute instead of
        /// relative.
        TIMER_ABSTIME;
    }
}

/// Suspend execution of this thread for the amount of time specified by `request`
/// and measured against the clock speficied by `clock_id`.
///
/// If `flags` is [`TIMER_ABSTIME`](ClockNanosleepFlags::TIMER_ABSTIME), this function will suspend
/// execution until the time value of clock_id reaches the absolute time specified by `request`. If
/// a signal is caught by a signal-catching function, or a signal causes the process to terminate,
/// this sleep is interrrupted.
///
/// see also [man 3 clock_nanosleep](https://pubs.opengroup.org/onlinepubs/009695399/functions/clock_nanosleep.html)
#[cfg(any(
    linux_android,
    solarish,
    freebsdlike,
    target_os = "netbsd",
    target_os = "hurd",
    target_os = "aix"
))]
pub fn clock_nanosleep(
    clock_id: ClockId,
    flags: ClockNanosleepFlags,
    request: &TimeSpec,
) -> Result<TimeSpec> {
    let mut remain = TimeSpec::new(0, 0);
    let ret = unsafe {
        libc::clock_nanosleep(
            clock_id.as_raw(),
            flags.bits(),
            request.as_ref() as *const _,
            remain.as_mut() as *mut _,
        )
    };
    if ret == 0 {
        Ok(remain)
    } else {
        Err(Errno::from_raw(ret))
    }
}
