use core::mem::MaybeUninit;

use alloc::string::String;

use windows_sys::Win32::System::Time::{
    GetDynamicTimeZoneInformation, DYNAMIC_TIME_ZONE_INFORMATION,
    TIME_ZONE_ID_INVALID,
};

use crate::{
    error::{err, Error, ErrorContext},
    tz::{TimeZone, TimeZoneDatabase},
    util::utf8,
};

use self::windows_zones::WINDOWS_TO_IANA;

#[allow(dead_code)] // we don't currently read the version
mod windows_zones;

/// Attempts to find the default "system" time zone.
///
/// This works by querying `GetDynamicTimeZoneInformation` via the Windows
/// API, and mapping the time zone key name returned to an IANA time zone
/// name via the [CLDR XML data].
///
/// If the API call fails or a valid mapping could not be found, then `None`
/// is returned and some log messages are emitted.
///
/// Windows does provide a [WinRT GetTimeZone] call that will return the IANA
/// time zone name directly, but it looks like a mess to use WinRT from Rust
/// currently. And this approach enjoys wider platform support.
///
/// [CLDR XML data]: https://github.com/unicode-org/cldr/raw/main/common/supplemental/windowsZones.xml
/// [WinRT GetTimeZone]: https://learn.microsoft.com/en-us/uwp/api/windows.globalization.calendar.gettimezone?view=winrt-22621
pub(super) fn get(db: &TimeZoneDatabase) -> Option<TimeZone> {
    let tz_key_name = match get_tz_key_name() {
        Ok(tz_key_name) => tz_key_name,
        Err(_err) => {
            warn!(
                "failed to discover current time zone via \
                 winapi GetDynamicTimeZoneInformation: {_err}",
            );
            return None;
        }
    };
    let iana_name = match windows_to_iana(&tz_key_name) {
        Ok(iana_name) => iana_name,
        Err(_err) => {
            warn!("could not find IANA time zone name: {_err}");
            return None;
        }
    };
    let tz = match db.get(iana_name) {
        Ok(tz) => tz,
        Err(_err) => {
            warn!(
                "could not find mapped IANA time zone {iana_name} \
                 in zoneinfo database {db:?}: {_err}",
            );
            return None;
        }
    };
    Some(tz)
}

pub(super) fn read(_db: &TimeZoneDatabase, path: &str) -> Option<TimeZone> {
    match super::read_unnamed_tzif_file(path) {
        Ok(tz) => Some(tz),
        Err(_err) => {
            trace!("failed to read {path} as unnamed time zone: {_err}");
            None
        }
    }
}

fn windows_to_iana(tz_key_name: &str) -> Result<&'static str, Error> {
    let result = WINDOWS_TO_IANA.binary_search_by(|(win_name, _)| {
        utf8::cmp_ignore_ascii_case(win_name, &tz_key_name)
    });
    let Ok(index) = result else {
        return Err(err!(
            "found Windows time zone name {tz_key_name}, \
             but could not find a mapping for it to an \
             IANA time zone name",
        ));
    };
    let iana_name = WINDOWS_TO_IANA[index].1;
    trace!(
        "found Windows time zone name {tz_key_name}, and \
         successfully mapped it to IANA time zone {iana_name}",
    );
    Ok(iana_name)
}

fn get_tz_key_name() -> Result<String, Error> {
    let mut info: MaybeUninit<DYNAMIC_TIME_ZONE_INFORMATION> =
        MaybeUninit::uninit();
    // SAFETY: We pass a pointer to the expected input and signal it as
    // unitializaed to rustc via MaybeUninit.
    let rc = unsafe { GetDynamicTimeZoneInformation(info.as_mut_ptr()) };
    if rc == TIME_ZONE_ID_INVALID {
        return Err(Error::io(std::io::Error::last_os_error()));
    }
    // SAFETY: Windows API docs indicate that the pointer is correctly written
    // to unless it fails, and we check for failure above. So we're only here
    // when `info` is properly initialized.
    let info = unsafe { info.assume_init() };
    let tz_key_name = nul_terminated_utf16_to_string(&info.TimeZoneKeyName)
        .context(
            "could not get TimeZoneKeyName from \
             winapi DYNAMIC_TIME_ZONE_INFORMATION",
        )?;
    Ok(tz_key_name)
}

fn nul_terminated_utf16_to_string(
    code_units: &[u16],
) -> Result<String, Error> {
    let nul = code_units.iter().position(|&cu| cu == 0).ok_or_else(|| {
        err!("failed to convert u16 slice to UTF-8 (no NUL terminator found)")
    })?;
    let string = String::from_utf16(&code_units[..nul])
        .map_err(Error::adhoc)
        .with_context(|| {
            err!("failed to convert u16 slice to UTF-8 (invalid UTF-16)")
        })?;
    Ok(string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_time_zone_name_windows_dynamic_time_zone() {
        let _ = crate::logging::Logger::init();

        let db = crate::tz::db();
        if crate::tz::db().is_definitively_empty() {
            return;
        }
        let path = std::path::Path::new("/etc/localtime");
        if !path.exists() {
            return;
        }
        // It's hard to assert much other than that a time zone could be
        // successfully constructed. Presumably this may fail in certain
        // environments, but hopefully the `is_definitively_empty` check above
        // will filter most out.
        assert!(get(db).is_some());
    }
}
