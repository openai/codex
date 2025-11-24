use crate::backend::c;
use crate::fd::BorrowedFd;

/// `CLOCK_*` constants for use with [`clock_gettime`].
///
/// These constants are always supported at runtime, so `clock_gettime` never
/// has to fail with `INVAL` due to an unsupported clock. See
/// [`DynamicClockId`] for a greater set of clocks, with the caveat that not
/// all of them are always supported.
///
/// [`clock_gettime`]: crate::time::clock_gettime
#[cfg(not(any(apple, target_os = "wasi")))]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    not(any(target_os = "aix", target_os = "cygwin", target_os = "dragonfly")),
    repr(i32)
)]
#[cfg_attr(any(target_os = "cygwin", target_os = "dragonfly"), repr(u64))]
#[cfg_attr(target_os = "aix", repr(i64))]
#[non_exhaustive]
pub enum ClockId {
    /// `CLOCK_REALTIME`
    #[doc(alias = "CLOCK_REALTIME")]
    Realtime = bitcast!(c::CLOCK_REALTIME),

    /// `CLOCK_MONOTONIC`
    #[doc(alias = "CLOCK_MONOTONIC")]
    Monotonic = bitcast!(c::CLOCK_MONOTONIC),

    /// `CLOCK_UPTIME`
    ///
    /// On FreeBSD, this is an alias for [`Self::Boottime`].
    ///
    /// On OpenBSD, this differs from `Self::Boottime`; it only advances when
    /// the system is not suspended.
    ///
    /// [`Self::Uptime`]: https://docs.rs/rustix/*/x86_64-unknown-freebsd/rustix/time/enum.ClockId.html#variant.Uptime
    #[cfg(any(freebsdlike, target_os = "openbsd"))]
    #[doc(alias = "CLOCK_UPTIME")]
    Uptime = c::CLOCK_UPTIME,

    /// `CLOCK_PROCESS_CPUTIME_ID`
    #[cfg(not(any(
        solarish,
        target_os = "horizon",
        target_os = "netbsd",
        target_os = "redox",
        target_os = "vita"
    )))]
    #[doc(alias = "CLOCK_PROCESS_CPUTIME_ID")]
    ProcessCPUTime = c::CLOCK_PROCESS_CPUTIME_ID,

    /// `CLOCK_THREAD_CPUTIME_ID`
    #[cfg(not(any(
        solarish,
        target_os = "horizon",
        target_os = "netbsd",
        target_os = "redox",
        target_os = "vita"
    )))]
    #[doc(alias = "CLOCK_THREAD_CPUTIME_ID")]
    ThreadCPUTime = c::CLOCK_THREAD_CPUTIME_ID,

    /// `CLOCK_REALTIME_COARSE`
    #[cfg(any(linux_kernel, target_os = "freebsd"))]
    #[doc(alias = "CLOCK_REALTIME_COARSE")]
    RealtimeCoarse = c::CLOCK_REALTIME_COARSE,

    /// `CLOCK_MONOTONIC_COARSE`
    #[cfg(any(linux_kernel, target_os = "freebsd"))]
    #[doc(alias = "CLOCK_MONOTONIC_COARSE")]
    MonotonicCoarse = c::CLOCK_MONOTONIC_COARSE,

    /// `CLOCK_MONOTONIC_RAW`
    #[cfg(linux_kernel)]
    #[doc(alias = "CLOCK_MONOTONIC_RAW")]
    MonotonicRaw = c::CLOCK_MONOTONIC_RAW,

    /// `CLOCK_REALTIME_ALARM`
    #[cfg(linux_kernel)]
    #[doc(alias = "CLOCK_REALTIME_ALARM")]
    RealtimeAlarm = bitcast!(c::CLOCK_REALTIME_ALARM),

    /// `CLOCK_TAI`, available on Linux ≥ 3.10
    #[cfg(all(linux_kernel, feature = "linux_4_11"))]
    #[doc(alias = "CLOCK_TAI")]
    Tai = bitcast!(c::CLOCK_TAI),

    /// `CLOCK_BOOTTIME`
    #[cfg(any(linux_kernel, target_os = "fuchsia", target_os = "openbsd"))]
    #[doc(alias = "CLOCK_BOOTTIME")]
    Boottime = bitcast!(c::CLOCK_BOOTTIME),

    /// `CLOCK_BOOTTIME_ALARM`
    #[cfg(any(linux_kernel, target_os = "fuchsia"))]
    #[doc(alias = "CLOCK_BOOTTIME_ALARM")]
    BoottimeAlarm = bitcast!(c::CLOCK_BOOTTIME_ALARM),
}

/// `CLOCK_*` constants for use with [`clock_gettime`].
///
/// These constants are always supported at runtime, so `clock_gettime` never
/// has to fail with `INVAL` due to an unsupported clock. See
/// [`DynamicClockId`] for a greater set of clocks, with the caveat that not
/// all of them are always supported.
///
/// [`clock_gettime`]: crate::time::clock_gettime
#[cfg(apple)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u32)]
#[non_exhaustive]
pub enum ClockId {
    /// `CLOCK_REALTIME`
    #[doc(alias = "CLOCK_REALTIME")]
    Realtime = c::CLOCK_REALTIME,

    /// `CLOCK_MONOTONIC`
    #[doc(alias = "CLOCK_MONOTONIC")]
    Monotonic = c::CLOCK_MONOTONIC,

    /// `CLOCK_PROCESS_CPUTIME_ID`
    #[doc(alias = "CLOCK_PROCESS_CPUTIME_ID")]
    ProcessCPUTime = c::CLOCK_PROCESS_CPUTIME_ID,

    /// `CLOCK_THREAD_CPUTIME_ID`
    #[doc(alias = "CLOCK_THREAD_CPUTIME_ID")]
    ThreadCPUTime = c::CLOCK_THREAD_CPUTIME_ID,
}

/// `CLOCK_*` constants for use with [`clock_gettime_dynamic`].
///
/// These constants may be unsupported at runtime, depending on the OS version,
/// and `clock_gettime_dynamic` may fail with `INVAL`. See [`ClockId`] for
/// clocks which are always supported at runtime.
///
/// [`clock_gettime_dynamic`]: crate::time::clock_gettime_dynamic
#[cfg(not(target_os = "wasi"))]
#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub enum DynamicClockId<'a> {
    /// `ClockId` values that are always supported at runtime.
    Known(ClockId),

    /// Linux dynamic clocks.
    Dynamic(BorrowedFd<'a>),

    /// `CLOCK_REALTIME_ALARM`
    #[cfg(linux_kernel)]
    #[doc(alias = "CLOCK_REALTIME_ALARM")]
    RealtimeAlarm,

    /// `CLOCK_TAI`, available on Linux ≥ 3.10
    #[cfg(linux_kernel)]
    #[doc(alias = "CLOCK_TAI")]
    Tai,

    /// `CLOCK_BOOTTIME`
    #[cfg(any(
        linux_kernel,
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "openbsd"
    ))]
    #[doc(alias = "CLOCK_BOOTTIME")]
    Boottime,

    /// `CLOCK_BOOTTIME_ALARM`
    #[cfg(any(linux_kernel, target_os = "fuchsia"))]
    #[doc(alias = "CLOCK_BOOTTIME_ALARM")]
    BoottimeAlarm,
}
