//! Utility functions, including updating time zone information.

pub use time_core::util::{days_in_year, is_leap_year, weeks_in_year};

use crate::Month;

/// Whether to adjust the date, and in which direction. Useful when implementing arithmetic.
pub(crate) enum DateAdjustment {
    /// The previous day should be used.
    Previous,
    /// The next day should be used.
    Next,
    /// The date should be used as-is.
    None,
}

/// Get the number of days in the month of a given year.
///
/// ```rust
/// # use time::{Month, util};
/// assert_eq!(util::days_in_month(Month::February, 2020), 29);
/// ```
#[inline]
pub const fn days_in_month(month: Month, year: i32) -> u8 {
    time_core::util::days_in_month(month as u8, year)
}

/// Get the number of days in the month of a given year.
///
/// ```rust
/// # #![expect(deprecated)]
/// # use time::{Month, util};
/// assert_eq!(util::days_in_year_month(2020, Month::February), 29);
/// ```
#[deprecated(
    since = "0.3.37",
    note = "use `days_in_month` or `Month::length` instead"
)]
#[inline]
pub const fn days_in_year_month(year: i32, month: Month) -> u8 {
    days_in_month(month, year)
}

/// Update time zone information from the system.
///
/// For a version of this function that is guaranteed to be sound, see [`refresh_tz`].
///
/// # Safety
///
/// This is a system call with specific requirements. The following is from POSIX's description of
/// `tzset`:
///
/// > If a thread accesses `tzname`, `daylight`, or `timezone` directly while another thread is in a
/// > call to `tzset()`, or to any function that is required or allowed to set timezone information
/// > as if by calling `tzset()`, the behavior is undefined.
///
/// Effectively, this translates to the requirement that at least one of the following must be true:
///
/// - The operating system provides a thread-safe environment.
/// - The process is single-threaded.
/// - The process is multi-threaded **and** no other thread is mutating the environment in any way
///   at the same time a call to a method that obtains the local UTC offset. This includes adding,
///   removing, or modifying an environment variable.
///
/// ## Soundness is global
///
/// You must not only verify this safety conditions for your code, but for **all** code that will be
/// included in the final binary. Notably, it applies to both direct and transitive dependencies and
/// to both Rust and non-Rust code. **For this reason it is not possible for a library to soundly
/// call this method.**
///
/// ## Forward compatibility
///
/// This currently only does anything on Unix-like systems. On other systems, it is a no-op. This
/// may change in the future if necessary, expanding the safety requirements. It is expected that,
/// at a minimum, calling this method when the process is single-threaded will remain sound.
#[cfg(feature = "local-offset")]
#[inline]
pub unsafe fn refresh_tz_unchecked() {
    // Safety: The caller must uphold the safety requirements.
    unsafe { crate::sys::refresh_tz_unchecked() };
}

/// Attempt to update time zone information from the system.
///
/// Returns `None` if the call is not known to be sound.
#[cfg(feature = "local-offset")]
#[inline]
pub fn refresh_tz() -> Option<()> {
    crate::sys::refresh_tz()
}

#[doc(hidden)]
#[cfg(feature = "local-offset")]
#[expect(
    clippy::missing_const_for_fn,
    reason = "no longer used; original implementation was not const"
)]
#[deprecated(since = "0.3.37", note = "no longer needed; TZ is refreshed manually")]
pub mod local_offset {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Soundness {
        Sound,
        Unsound,
    }

    #[inline]
    pub unsafe fn set_soundness(_: Soundness) {}

    #[inline]
    pub fn get_soundness() -> Soundness {
        Soundness::Sound
    }
}
