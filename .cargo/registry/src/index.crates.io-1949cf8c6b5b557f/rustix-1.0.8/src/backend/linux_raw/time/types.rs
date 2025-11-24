use crate::ffi;
use bitflags::bitflags;

bitflags! {
    /// `TFD_*` flags for use with [`timerfd_create`].
    ///
    /// [`timerfd_create`]: crate::time::timerfd_create
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct TimerfdFlags: ffi::c_uint {
        /// `TFD_NONBLOCK`
        #[doc(alias = "TFD_NONBLOCK")]
        const NONBLOCK = linux_raw_sys::general::TFD_NONBLOCK;

        /// `TFD_CLOEXEC`
        #[doc(alias = "TFD_CLOEXEC")]
        const CLOEXEC = linux_raw_sys::general::TFD_CLOEXEC;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `TFD_TIMER_*` flags for use with [`timerfd_settime`].
    ///
    /// [`timerfd_settime`]: crate::time::timerfd_settime
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct TimerfdTimerFlags: ffi::c_uint {
        /// `TFD_TIMER_ABSTIME`
        #[doc(alias = "TFD_TIMER_ABSTIME")]
        const ABSTIME = linux_raw_sys::general::TFD_TIMER_ABSTIME;

        /// `TFD_TIMER_CANCEL_ON_SET`
        #[doc(alias = "TFD_TIMER_CANCEL_ON_SET")]
        const CANCEL_ON_SET = linux_raw_sys::general::TFD_TIMER_CANCEL_ON_SET;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `CLOCK_*` constants for use with [`timerfd_create`].
///
/// [`timerfd_create`]: crate::time::timerfd_create
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u32)]
#[non_exhaustive]
pub enum TimerfdClockId {
    /// `CLOCK_REALTIME`—A clock that tells the “real” time.
    ///
    /// This is a clock that tells the amount of time elapsed since the Unix
    /// epoch, 1970-01-01T00:00:00Z. The clock is externally settable, so it is
    /// not monotonic. Successive reads may see decreasing times, so it isn't
    /// reliable for measuring durations.
    #[doc(alias = "CLOCK_REALTIME")]
    Realtime = linux_raw_sys::general::CLOCK_REALTIME,

    /// `CLOCK_MONOTONIC`—A clock that tells an abstract time.
    ///
    /// Unlike `Realtime`, this clock is not based on a fixed known epoch, so
    /// individual times aren't meaningful. However, since it isn't settable,
    /// it is reliable for measuring durations.
    ///
    /// This clock does not advance while the system is suspended; see
    /// `Boottime` for a clock that does.
    #[doc(alias = "CLOCK_MONOTONIC")]
    Monotonic = linux_raw_sys::general::CLOCK_MONOTONIC,

    /// `CLOCK_BOOTTIME`—Like `Monotonic`, but advances while suspended.
    ///
    /// This clock is similar to `Monotonic`, but does advance while the system
    /// is suspended.
    #[doc(alias = "CLOCK_BOOTTIME")]
    Boottime = linux_raw_sys::general::CLOCK_BOOTTIME,

    /// `CLOCK_REALTIME_ALARM`—Like `Realtime`, but wakes a suspended system.
    ///
    /// This clock is like `Realtime`, but can wake up a suspended system.
    ///
    /// Use of this clock requires the `CAP_WAKE_ALARM` Linux capability.
    #[doc(alias = "CLOCK_REALTIME_ALARM")]
    RealtimeAlarm = linux_raw_sys::general::CLOCK_REALTIME_ALARM,

    /// `CLOCK_BOOTTIME_ALARM`—Like `Boottime`, but wakes a suspended system.
    ///
    /// This clock is like `Boottime`, but can wake up a suspended system.
    ///
    /// Use of this clock requires the `CAP_WAKE_ALARM` Linux capability.
    #[doc(alias = "CLOCK_BOOTTIME_ALARM")]
    BoottimeAlarm = linux_raw_sys::general::CLOCK_BOOTTIME_ALARM,
}
