//! Get the system's UTC offset on Unix.

use core::mem::MaybeUninit;

use crate::{OffsetDateTime, UtcOffset};

/// Convert the given Unix timestamp to a `libc::tm`. Returns `None` on any error.
#[inline]
fn timestamp_to_tm(timestamp: i64) -> Option<libc::tm> {
    #[allow(
        clippy::useless_conversion,
        reason = "the exact type of `timestamp` beforehand can vary, so this conversion is \
                  necessary"
    )]
    let timestamp = timestamp.try_into().ok()?;

    let mut tm = MaybeUninit::uninit();

    // Safety: We are calling a system API, which mutates the `tm` variable. If a null
    // pointer is returned, an error occurred.
    let tm_ptr = unsafe { libc::localtime_r(&timestamp, tm.as_mut_ptr()) };

    if tm_ptr.is_null() {
        None
    } else {
        // Safety: The value was initialized, as we no longer have a null pointer.
        Some(unsafe { tm.assume_init() })
    }
}

/// Convert a `libc::tm` to a `UtcOffset`. Returns `None` on any error.
// This is available to any target known to have the `tm_gmtoff` extension.
#[cfg(any(
    target_os = "redox",
    target_os = "linux",
    target_os = "l4re",
    target_os = "android",
    target_os = "emscripten",
    target_os = "macos",
    target_os = "ios",
    target_os = "watchos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "haiku",
))]
#[inline]
fn tm_to_offset(_unix_timestamp: i64, tm: libc::tm) -> Option<UtcOffset> {
    let seconds = tm.tm_gmtoff.try_into().ok()?;
    UtcOffset::from_whole_seconds(seconds).ok()
}

/// Convert a `libc::tm` to a `UtcOffset`. Returns `None` on any error.
///
/// This method can return an incorrect value, as it only approximates the `tm_gmtoff` field. The
/// reason for this is that daylight saving time does not start on the same date every year, nor are
/// the rules for daylight saving time the same for every year. This implementation assumes 1970 is
/// equivalent to every other year, which is not always the case.
#[cfg(not(any(
    target_os = "redox",
    target_os = "linux",
    target_os = "l4re",
    target_os = "android",
    target_os = "emscripten",
    target_os = "macos",
    target_os = "ios",
    target_os = "watchos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "haiku",
)))]
#[inline]
fn tm_to_offset(unix_timestamp: i64, tm: libc::tm) -> Option<UtcOffset> {
    use crate::Date;

    let mut tm = tm;
    if tm.tm_sec == 60 {
        // Leap seconds are not currently supported.
        tm.tm_sec = 59;
    }

    let local_timestamp =
        Date::from_ordinal_date(1900 + tm.tm_year, u16::try_from(tm.tm_yday).ok()? + 1)
            .ok()?
            .with_hms(
                tm.tm_hour.try_into().ok()?,
                tm.tm_min.try_into().ok()?,
                tm.tm_sec.try_into().ok()?,
            )
            .ok()?
            .assume_utc()
            .unix_timestamp();

    let diff_secs = (local_timestamp - unix_timestamp).try_into().ok()?;

    UtcOffset::from_whole_seconds(diff_secs).ok()
}

/// Obtain the system's UTC offset.
#[inline]
pub(super) fn local_offset_at(datetime: OffsetDateTime) -> Option<UtcOffset> {
    let unix_timestamp = datetime.unix_timestamp();
    let tm = timestamp_to_tm(unix_timestamp)?;
    tm_to_offset(unix_timestamp, tm)
}
