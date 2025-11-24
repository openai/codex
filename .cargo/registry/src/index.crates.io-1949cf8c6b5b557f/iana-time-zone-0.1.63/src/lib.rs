#![warn(clippy::all)]
#![warn(clippy::cargo)]
#![warn(clippy::undocumented_unsafe_blocks)]
#![allow(unknown_lints)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]
#![warn(trivial_casts, trivial_numeric_casts)]
#![warn(unused_qualifications)]
#![warn(variant_size_differences)]

//! get the IANA time zone for the current system
//!
//! This small utility crate provides the
//! [`get_timezone()`](fn.get_timezone.html) function.
//!
//! ```rust
//! // Get the current time zone as a string.
//! let tz_str = iana_time_zone::get_timezone()?;
//! println!("The current time zone is: {}", tz_str);
//! # Ok::<(), iana_time_zone::GetTimezoneError>(())
//! ```
//!
//! The resulting string can be parsed to a
//! [`chrono-tz::Tz`](https://docs.rs/chrono-tz/latest/chrono_tz/enum.Tz.html)
//! variant like this:
//! ```rust
//! let tz_str = iana_time_zone::get_timezone()?;
//! let tz: chrono_tz::Tz = tz_str.parse()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(dead_code)]
mod ffi_utils;

#[cfg_attr(
    any(all(target_os = "linux", not(target_env = "ohos")), target_os = "hurd"),
    path = "tz_linux.rs"
)]
#[cfg_attr(all(target_os = "linux", target_env = "ohos"), path = "tz_ohos.rs")]
#[cfg_attr(target_os = "windows", path = "tz_windows.rs")]
#[cfg_attr(target_vendor = "apple", path = "tz_darwin.rs")]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    path = "tz_wasm32_unknown.rs"
)]
#[cfg_attr(
    any(target_os = "freebsd", target_os = "dragonfly"),
    path = "tz_freebsd.rs"
)]
#[cfg_attr(
    any(target_os = "netbsd", target_os = "openbsd"),
    path = "tz_netbsd.rs"
)]
#[cfg_attr(
    any(target_os = "illumos", target_os = "solaris"),
    path = "tz_illumos.rs"
)]
#[cfg_attr(target_os = "aix", path = "tz_aix.rs")]
#[cfg_attr(target_os = "android", path = "tz_android.rs")]
#[cfg_attr(target_os = "haiku", path = "tz_haiku.rs")]
mod platform;

/// Error types
#[derive(Debug)]
pub enum GetTimezoneError {
    /// Failed to parse
    FailedParsingString,
    /// Wrapped IO error
    IoError(std::io::Error),
    /// Platform-specific error from the operating system
    OsError,
}

impl std::error::Error for GetTimezoneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GetTimezoneError::FailedParsingString => None,
            GetTimezoneError::IoError(err) => Some(err),
            GetTimezoneError::OsError => None,
        }
    }
}

impl std::fmt::Display for GetTimezoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str(match self {
            GetTimezoneError::FailedParsingString => "GetTimezoneError::FailedParsingString",
            GetTimezoneError::IoError(err) => return err.fmt(f),
            GetTimezoneError::OsError => "OsError",
        })
    }
}

impl From<std::io::Error> for GetTimezoneError {
    fn from(orig: std::io::Error) -> Self {
        GetTimezoneError::IoError(orig)
    }
}

/// Get the current IANA time zone as a string.
///
/// See the module-level documentation for a usage example and more details
/// about this function.
#[inline]
pub fn get_timezone() -> Result<String, GetTimezoneError> {
    platform::get_timezone_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_current() {
        println!("current: {}", get_timezone().unwrap());
    }
}
