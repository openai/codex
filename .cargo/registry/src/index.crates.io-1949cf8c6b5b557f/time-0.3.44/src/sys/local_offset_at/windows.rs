//! Get the system's UTC offset on Windows.

use core::mem::MaybeUninit;

use num_conv::prelude::*;

use crate::convert::*;
use crate::{OffsetDateTime, UtcOffset};

// ffi: WINAPI FILETIME struct
#[repr(C)]
#[expect(non_snake_case, reason = "system API")]
#[expect(clippy::missing_docs_in_private_items)]
struct FileTime {
    dwLowDateTime: u32,
    dwHighDateTime: u32,
}

// ffi: WINAPI SYSTEMTIME struct
#[repr(C)]
#[expect(non_snake_case, reason = "system API")]
#[expect(clippy::missing_docs_in_private_items)]
struct SystemTime {
    wYear: u16,
    wMonth: u16,
    wDayOfWeek: u16,
    wDay: u16,
    wHour: u16,
    wMinute: u16,
    wSecond: u16,
    wMilliseconds: u16,
}

#[link(name = "kernel32")]
extern "system" {
    // https://docs.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-systemtimetofiletime
    fn SystemTimeToFileTime(lpSystemTime: *const SystemTime, lpFileTime: *mut FileTime) -> i32;

    // https://docs.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-systemtimetotzspecificlocaltime
    fn SystemTimeToTzSpecificLocalTime(
        lpTimeZoneInformation: *const core::ffi::c_void, // We only pass `nullptr` here
        lpUniversalTime: *const SystemTime,
        lpLocalTime: *mut SystemTime,
    ) -> i32;
}

/// Convert a `SYSTEMTIME` to a `FILETIME`. Returns `None` if any error occurred.
#[inline]
fn systemtime_to_filetime(systime: &SystemTime) -> Option<FileTime> {
    let mut ft = MaybeUninit::uninit();

    // Safety: `SystemTimeToFileTime` is thread-safe.
    if 0 == unsafe { SystemTimeToFileTime(systime, ft.as_mut_ptr()) } {
        // failed
        None
    } else {
        // Safety: The call succeeded.
        Some(unsafe { ft.assume_init() })
    }
}

/// Convert a `FILETIME` to an `i64`, representing a number of seconds.
#[inline]
fn filetime_to_secs(filetime: &FileTime) -> i64 {
    /// FILETIME represents 100-nanosecond intervals
    const FT_TO_SECS: u64 = Nanosecond::per_t::<u64>(Second) / 100;
    ((filetime.dwHighDateTime.extend::<u64>() << 32 | filetime.dwLowDateTime.extend::<u64>())
        / FT_TO_SECS) as i64
}

/// Convert an [`OffsetDateTime`] to a `SYSTEMTIME`.
#[inline]
fn offset_to_systemtime(datetime: OffsetDateTime) -> SystemTime {
    let (_, month, day_of_month) = datetime.to_offset(UtcOffset::UTC).date().to_calendar_date();
    SystemTime {
        wYear: datetime.year().cast_unsigned().truncate(),
        wMonth: u8::from(month).extend(),
        wDay: day_of_month.extend(),
        wDayOfWeek: 0, // ignored
        wHour: datetime.hour().extend(),
        wMinute: datetime.minute().extend(),
        wSecond: datetime.second().extend(),
        wMilliseconds: datetime.millisecond(),
    }
}

/// Obtain the system's UTC offset.
#[inline]
pub(super) fn local_offset_at(datetime: OffsetDateTime) -> Option<UtcOffset> {
    // This function falls back to UTC if any system call fails.
    let systime_utc = offset_to_systemtime(datetime.to_offset(UtcOffset::UTC));

    // Safety: `local_time` is only read if it is properly initialized, and
    // `SystemTimeToTzSpecificLocalTime` is thread-safe.
    let systime_local = unsafe {
        let mut local_time = MaybeUninit::uninit();

        if 0 == SystemTimeToTzSpecificLocalTime(
            core::ptr::null(), // use system's current timezone
            &systime_utc,
            local_time.as_mut_ptr(),
        ) {
            // call failed
            return None;
        } else {
            local_time.assume_init()
        }
    };

    // Convert SYSTEMTIMEs to FILETIMEs so we can perform arithmetic on them.
    let ft_system = systemtime_to_filetime(&systime_utc)?;
    let ft_local = systemtime_to_filetime(&systime_local)?;

    let diff_secs = (filetime_to_secs(&ft_local) - filetime_to_secs(&ft_system))
        .try_into()
        .ok()?;

    UtcOffset::from_whole_seconds(diff_secs).ok()
}
